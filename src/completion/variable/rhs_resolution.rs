/// Right-hand-side expression resolution for variable assignments.
///
/// This module resolves the type of the right-hand side of an assignment
/// (`$var = <expr>`) to zero or more `ClassInfo` values.  It handles:
///
///   - `new ClassName(…)` → the instantiated class
///   - Array access: `$arr[0]`, `$arr[$key]` → generic element type
///   - Function calls: `someFunc()` → return type
///   - Method calls: `$this->method()`, `$obj->method()` → return type
///   - Static calls: `ClassName::method()` → return type
///   - Property access: `$this->prop`, `$obj->prop` → property type
///   - Match expressions: union of all arm types
///   - Ternary / null-coalescing: union of both branches
///   - Clone: `clone $expr` → preserves the cloned expression's type
///
/// The entry point is [`resolve_rhs_expression`], which dispatches to
/// specialised helpers based on the AST node kind.
/// The only caller is
/// [`check_expression_for_assignment`](super::resolution::check_expression_for_assignment)
/// in `variable_resolution.rs`.
use std::collections::HashMap;
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::Backend;
use crate::docblock;
use crate::parser::extract_hint_string;
use crate::types::ClassInfo;

use super::resolution::build_var_resolver_from_ctx;
use crate::completion::call_resolution::MethodReturnCtx;
use crate::completion::conditional_resolution::resolve_conditional_with_args;
use crate::completion::resolver::VarResolutionCtx;

/// Resolve a right-hand-side expression to zero or more `ClassInfo`
/// values.
///
/// This is the single place where an arbitrary PHP expression is
/// resolved to class types.  It handles:
///
///   - `new ClassName(…)` → the instantiated class
///   - Array access: `$arr[0]`, `$arr[$key]` → generic element type
///   - Function calls: `someFunc()` → return type
///   - Method calls: `$this->method()`, `$obj->method()` → return type
///   - Static calls: `ClassName::method()` → return type
///   - Property access: `$this->prop`, `$obj->prop` → property type
///   - Match expressions: union of all arm types
///   - Ternary / null-coalescing: union of both branches
///   - Clone: `clone $expr` → preserves the cloned expression's type
///
/// Used by `check_expression_for_assignment` (for `$var = <expr>`)
/// and recursively by multi-branch constructs (match, ternary, `??`).
pub(in crate::completion) fn resolve_rhs_expression<'b>(
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    match expr {
        Expression::Instantiation(inst) => resolve_rhs_instantiation(inst, ctx),
        Expression::ArrayAccess(array_access) => resolve_rhs_array_access(array_access, expr, ctx),
        Expression::Call(call) => resolve_rhs_call(call, expr, ctx),
        Expression::Access(access) => resolve_rhs_property_access(access, ctx),
        Expression::Parenthesized(p) => resolve_rhs_expression(p.expression, ctx),
        Expression::Match(match_expr) => {
            let mut combined = Vec::new();
            for arm in match_expr.arms.iter() {
                let arm_results = resolve_rhs_expression(arm.expression(), ctx);
                ClassInfo::extend_unique(&mut combined, arm_results);
            }
            combined
        }
        Expression::Conditional(cond_expr) => {
            let mut combined = Vec::new();
            let then_expr = cond_expr.then.unwrap_or(cond_expr.condition);
            ClassInfo::extend_unique(&mut combined, resolve_rhs_expression(then_expr, ctx));
            ClassInfo::extend_unique(&mut combined, resolve_rhs_expression(cond_expr.r#else, ctx));
            combined
        }
        Expression::Binary(binary) if binary.operator.is_null_coalesce() => {
            let mut combined = Vec::new();
            ClassInfo::extend_unique(&mut combined, resolve_rhs_expression(binary.lhs, ctx));
            ClassInfo::extend_unique(&mut combined, resolve_rhs_expression(binary.rhs, ctx));
            combined
        }
        Expression::Clone(clone_expr) => resolve_rhs_clone(clone_expr, ctx),
        // ── Pipe operator (PHP 8.5): `$expr |> callable(...)` ──
        // The result type is the return type of the callable.
        // The callable is typically a first-class callable reference
        // (PartialApplication) such as `trim(...)` or `createDate(...)`.
        Expression::Pipe(pipe) => resolve_rhs_pipe(pipe, ctx),
        Expression::PartialApplication(_)
        | Expression::Closure(_)
        | Expression::ArrowFunction(_) => {
            // First-class callable syntax (`strlen(...)`),
            // closure literals (`function() { … }`), and
            // arrow functions (`fn() => …`) all produce a
            // `Closure` instance at runtime.
            // Use the fully-qualified name so that resolution
            // succeeds even inside a namespace block (unqualified
            // class names are prefixed with the current namespace
            // and do NOT fall back to the global scope in PHP).
            crate::completion::type_resolution::type_hint_to_classes(
                "\\Closure",
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            )
        }
        // ── Generator yield-assignment: `$var = yield $expr` ──
        // The value of a yield expression is the TSend type from
        // the enclosing function's `@return Generator<K, V, TSend, R>`.
        Expression::Yield(_) => {
            if let Some(ref ret_type) = ctx.enclosing_return_type
                && let Some(send_type) = crate::docblock::extract_generator_send_type(ret_type)
            {
                return crate::completion::type_resolution::type_hint_to_classes(
                    &send_type,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
            }
            vec![]
        }
        _ => vec![],
    }
}

/// Resolve a pipe expression `$input |> callable(...)` to the callable's
/// return type.
///
/// The pipe operator passes `$input` as the first argument to `callable`
/// and returns its result.  Chains like `$a |> f(...) |> g(...)` are
/// nested: the outer pipe's input is the inner pipe expression.
///
/// Currently handles function-level callables (e.g. `createDate(...)`).
/// Method and static method callables are not yet supported.
fn resolve_rhs_pipe(pipe: &Pipe<'_>, ctx: &VarResolutionCtx<'_>) -> Vec<ClassInfo> {
    // The callable determines the result type.
    // For `PartialApplication::Function`, extract the function name
    // and look up its return type.
    match pipe.callable {
        Expression::PartialApplication(PartialApplication::Function(fpa)) => {
            let func_name = match fpa.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return vec![],
            };
            if let Some(fl) = ctx.function_loader
                && let Some(func_info) = fl(&func_name)
                && let Some(ref ret) = func_info.return_type
            {
                return crate::completion::type_resolution::type_hint_to_classes(
                    ret,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
            }
            vec![]
        }
        // Method callable: `$input |> $obj->method(...)`
        // Static callable: `$input |> Class::method(...)`
        // Not yet supported — fall back to empty.
        _ => vec![],
    }
}

/// Resolve `new ClassName(…)` to the instantiated class.
fn resolve_rhs_instantiation(
    inst: &Instantiation<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let class_name = match inst.class {
        Expression::Self_(_) => Some("self"),
        Expression::Static(_) => Some("static"),
        Expression::Identifier(ident) => Some(ident.value()),
        _ => None,
    };
    if let Some(name) = class_name {
        let classes = crate::completion::type_resolution::type_hint_to_classes(
            name,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );

        // ── Constructor template inference ──────────────────────
        // When the class has `@template` params and the constructor
        // has `@param` bindings for them, infer concrete types from
        // the constructor arguments and apply the substitution to
        // the class so that methods returning `T` resolve correctly.
        if classes.len() == 1 && !classes[0].template_params.is_empty() {
            let cls = &classes[0];
            if let Some(ctor) = cls.methods.iter().find(|m| m.name == "__construct")
                && !ctor.template_bindings.is_empty()
                && let Some(ref arg_list) = inst.argument_list
            {
                let text_args =
                    super::raw_type_inference::extract_argument_text(arg_list, ctx.content);
                if !text_args.is_empty() {
                    let rctx = ctx.as_resolution_ctx();
                    let subs = build_constructor_template_subs(cls, ctor, &text_args, &rctx, ctx);
                    if !subs.is_empty() {
                        let type_args: Vec<&str> = cls
                            .template_params
                            .iter()
                            .map(|p| subs.get(p).map(|s| s.as_str()).unwrap_or(p.as_str()))
                            .collect();
                        let resolved =
                            crate::virtual_members::resolve_class_fully(cls, ctx.class_loader);
                        let substituted =
                            crate::inheritance::apply_generic_args(&resolved, &type_args);
                        return vec![substituted];
                    }
                }
            }
        }

        return classes;
    }

    // ── `new $var` where `$var` holds a class-string ────────────
    // When the class expression is a variable, resolve it to check
    // if it holds a class-string value (e.g. `$f = Foo::class;
    // new $f`).  Extract the class name from the class-string and
    // use it to resolve the instantiated type.
    if let Expression::Variable(Variable::Direct(dv)) = inst.class {
        let var_name = dv.name.to_string();
        let resolved =
            crate::completion::variable::class_string_resolution::resolve_class_string_targets(
                &var_name,
                ctx.current_class,
                ctx.all_classes,
                ctx.content,
                ctx.cursor_offset,
                ctx.class_loader,
            );
        if !resolved.is_empty() {
            return resolved;
        }
    }

    vec![]
}

/// Build a template substitution map from constructor arguments.
///
/// Uses the constructor's `template_bindings` (from `@param T $name`
/// annotations) to match template parameters to their concrete types
/// inferred from the call-site arguments.  Handles:
///   - Direct type: `@param T $bar` + `new Foo(new Baz())` → `T = Baz`
///   - Array type: `@param T[] $items` + `new Foo([new X()])` → `T = X`
///   - Generic wrapper: `@param Wrapper<T> $w` + `new Foo(new Wrapper(new X()))` → `T = X`
///     (by resolving the wrapper's constructor template params recursively)
fn build_constructor_template_subs(
    _class: &ClassInfo,
    ctor: &crate::types::MethodInfo,
    text_args: &str,
    rctx: &crate::completion::resolver::ResolutionCtx<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> HashMap<String, String> {
    let args = crate::completion::conditional_resolution::split_text_args(text_args);
    let mut subs = HashMap::new();

    for (tpl_name, param_name) in &ctor.template_bindings {
        // Find the parameter index for this binding.
        let param_idx = match ctor.parameters.iter().position(|p| p.name == *param_name) {
            Some(idx) => idx,
            None => continue,
        };

        // Get the corresponding argument text.
        let arg_text = match args.get(param_idx) {
            Some(text) => text.trim(),
            None => continue,
        };

        // Determine the binding mode by inspecting the parameter's
        // docblock type hint.  The type hint tells us how the template
        // param is embedded in the `@param` annotation.
        let param_hint = ctor
            .parameters
            .get(param_idx)
            .and_then(|p| p.type_hint.as_deref());
        let binding_mode = classify_template_binding(tpl_name, param_hint);

        match binding_mode {
            TemplateBindingMode::Direct => {
                // `@param T $bar` — the argument resolves directly to T.
                if let Some(type_name) = Backend::resolve_arg_text_to_type(arg_text, rctx) {
                    subs.insert(tpl_name.clone(), type_name);
                }
            }
            TemplateBindingMode::ArrayElement => {
                // `@param T[] $items` — resolve individual array elements.
                if arg_text.starts_with('[') && arg_text.ends_with(']') {
                    let inner = arg_text[1..arg_text.len() - 1].trim();
                    if !inner.is_empty() {
                        let first_elem =
                            crate::completion::conditional_resolution::split_text_args(inner);
                        if let Some(elem) = first_elem.first()
                            && let Some(type_name) =
                                Backend::resolve_arg_text_to_type(elem.trim(), rctx)
                        {
                            subs.insert(tpl_name.clone(), type_name);
                        }
                    }
                } else if let Some(type_name) = Backend::resolve_arg_text_to_type(arg_text, rctx) {
                    // Fallback: treat as direct if not an array literal.
                    subs.insert(tpl_name.clone(), type_name);
                }
            }
            TemplateBindingMode::GenericWrapper(wrapper_name, tpl_position) => {
                // `@param Wrapper<T> $a` — resolve the wrapper's constructor
                // template params to find the concrete type for T.
                if let Some(concrete) = resolve_generic_wrapper_template(
                    &wrapper_name,
                    tpl_position,
                    arg_text,
                    rctx,
                    ctx,
                ) {
                    subs.insert(tpl_name.clone(), concrete);
                }
            }
        }
    }

    subs
}

/// How a template parameter is referenced in a `@param` type annotation.
#[derive(Debug)]
pub(crate) enum TemplateBindingMode {
    /// `@param T $bar` — the whole type is the template param.
    Direct,
    /// `@param T[] $items` — the template param is the array element type.
    ArrayElement,
    /// `@param Wrapper<..., T, ...> $a` — the template param is a generic
    /// argument of the wrapper class at the given position.
    GenericWrapper(String, usize),
}

/// Classify how a template parameter name appears in a `@param` type hint.
///
/// Handles union types like `Arrayable<TKey, TValue>|iterable<TKey, TValue>|null`
/// by splitting at depth 0 first, then checking each union part individually.
pub(crate) fn classify_template_binding(
    tpl_name: &str,
    param_hint: Option<&str>,
) -> TemplateBindingMode {
    let hint = match param_hint {
        Some(h) => h,
        // No type hint — assume direct binding.
        None => return TemplateBindingMode::Direct,
    };

    // Strip nullable prefix.
    let hint = hint.strip_prefix('?').unwrap_or(hint);

    // Split the union at depth 0 (respecting `<…>` nesting) so that
    // `Arrayable<TKey, TValue>|iterable<TKey, TValue>|null` is split
    // into individual parts rather than treating the whole thing as one
    // type with a broken `<…>` span.
    let union_parts = split_union_at_depth0(hint);

    for part in &union_parts {
        let part = part.trim();
        if part == "null" || part.is_empty() {
            continue;
        }

        // Check for `T[]` pattern.
        if let Some(base) = part.strip_suffix("[]")
            && base == tpl_name
        {
            return TemplateBindingMode::ArrayElement;
        }

        // Check for direct `T`.
        if part == tpl_name {
            return TemplateBindingMode::Direct;
        }

        // Check for `Wrapper<..., T, ...>` pattern.
        if let Some(open) = part.find('<')
            && let Some(close) = part.rfind('>')
        {
            let wrapper_name = crate::docblock::types::clean_type(&part[..open]);
            let generic_part = &part[open + 1..close];
            let hint_args: Vec<&str> = generic_part.split(',').map(|s| s.trim()).collect();
            for (i, arg) in hint_args.iter().enumerate() {
                if *arg == tpl_name {
                    return TemplateBindingMode::GenericWrapper(wrapper_name, i);
                }
            }
        }
    }

    // Fallback to direct.
    TemplateBindingMode::Direct
}

/// Split a type string on `|` at depth 0, respecting `<…>` nesting.
///
/// `"Arrayable<TKey, TValue>|iterable<TKey, TValue>|null"` →
/// `["Arrayable<TKey, TValue>", "iterable<TKey, TValue>", "null"]`
fn split_union_at_depth0(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' | '(' | '{' => depth += 1,
            '>' | ')' | '}' => depth -= 1,
            '|' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Resolve a template param that appears inside a generic wrapper type.
///
/// For `@param Wrapper<T> $a` with argument `new Wrapper(new X())`,
/// recursively resolve the wrapper's constructor template params to
/// find the concrete type for the template param at `tpl_position`.
fn resolve_generic_wrapper_template(
    wrapper_name: &str,
    tpl_position: usize,
    arg_text: &str,
    rctx: &crate::completion::resolver::ResolutionCtx<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    // Load the wrapper class.
    let wrapper_cls = (ctx.class_loader)(wrapper_name)
        .map(Arc::unwrap_or_clone)
        .or_else(|| {
            ctx.all_classes
                .iter()
                .find(|c| crate::util::short_name(&c.name) == crate::util::short_name(wrapper_name))
                .map(|c| ClassInfo::clone(c))
        })?;

    // Find the wrapper's constructor and its template bindings.
    let wrapper_ctor = wrapper_cls
        .methods
        .iter()
        .find(|m| m.name == "__construct")?;
    if wrapper_ctor.template_bindings.is_empty() {
        return None;
    }

    // Extract the constructor arguments from the argument text.
    // e.g. from `new Foobar(new X())` extract `new X()`.
    let paren_start = arg_text.find('(')?;
    let paren_end = arg_text.rfind(')')?;
    let inner_args = arg_text[paren_start + 1..paren_end].trim();

    let wrapper_subs =
        build_constructor_template_subs(&wrapper_cls, wrapper_ctor, inner_args, rctx, ctx);

    // Find the wrapper's template param at the given position and
    // look it up in the substitution map.
    let wrapper_tpl = wrapper_cls.template_params.get(tpl_position)?;
    wrapper_subs.get(wrapper_tpl).cloned()
}

/// Resolve `$arr[0]` / `$arr[$key]` by extracting the generic element
/// type from the base array's annotation or assignment.
fn resolve_rhs_array_access<'b>(
    array_access: &ArrayAccess<'b>,
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    if let Expression::Variable(Variable::Direct(base_dv)) = array_access.array {
        let base_var = base_dv.name.to_string();
        let access_offset = expr.span().start.offset as usize;

        // Strategy 1: docblock annotation (`@var`, `@param`).
        if let Some(raw_type) =
            docblock::find_iterable_raw_type_in_source(ctx.content, access_offset, &base_var)
            && let Some(element_type) = docblock::types::extract_generic_value_type(&raw_type)
        {
            return crate::completion::type_resolution::type_hint_to_classes(
                &element_type,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
        }

        // Strategy 2: resolve the base variable's type via AST-based
        // assignment scanning and extract the iterable element type.
        // This handles cases like `$attrs = $ref->getAttributes();`
        // where there is no explicit `@var` annotation but the method
        // return type is `ReflectionAttribute[]`.
        let current_class = Some(ctx.current_class);
        if let Some(raw_type) = super::raw_type_inference::resolve_variable_assignment_raw_type(
            &base_var,
            ctx.content,
            access_offset as u32,
            current_class,
            ctx.all_classes,
            ctx.class_loader,
            ctx.function_loader,
        ) && let Some(element_type) = docblock::types::extract_generic_value_type(&raw_type)
        {
            return crate::completion::type_resolution::type_hint_to_classes(
                &element_type,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
        }
    }
    vec![]
}

/// Build a template substitution map for a function-level `@template` call.
///
/// Uses the function's `template_bindings` to match template parameters to
/// their concrete types inferred from the call-site arguments.  Handles:
///   - Direct type: `@param T $bar` + `func(new Baz())` → `T = Baz`
///   - Array type: `@param T[] $items` + `func([new X()])` → `T = X`
///   - Generic wrapper: `@param array<TKey, TValue> $v` + `func($users)` →
///     positional resolution through the wrapper's generic arguments.
pub(crate) fn build_function_template_subs(
    func_info: &crate::types::FunctionInfo,
    text_args: &str,
    rctx: &crate::completion::resolver::ResolutionCtx<'_>,
) -> HashMap<String, String> {
    let args = crate::completion::conditional_resolution::split_text_args(text_args);
    let mut subs = HashMap::new();

    for (tpl_name, param_name) in &func_info.template_bindings {
        let param_idx = match func_info
            .parameters
            .iter()
            .position(|p| p.name == *param_name)
        {
            Some(idx) => idx,
            None => continue,
        };

        let arg_text = match args.get(param_idx) {
            Some(text) => text.trim(),
            None => continue,
        };

        // Determine the binding mode by inspecting the parameter's
        // docblock type hint.  The type hint tells us how the template
        // param is embedded in the `@param` annotation.
        let param_hint = func_info
            .parameters
            .get(param_idx)
            .and_then(|p| p.type_hint.as_deref());
        let binding_mode = classify_template_binding(tpl_name, param_hint);

        match binding_mode {
            TemplateBindingMode::Direct => {
                if let Some(type_name) = Backend::resolve_arg_text_to_type(arg_text, rctx) {
                    subs.insert(tpl_name.clone(), type_name);
                }
            }
            TemplateBindingMode::ArrayElement => {
                // `@param T[] $items` — resolve individual array elements.
                if arg_text.starts_with('[') && arg_text.ends_with(']') {
                    let inner = arg_text[1..arg_text.len() - 1].trim();
                    if !inner.is_empty() {
                        let first_elem =
                            crate::completion::conditional_resolution::split_text_args(inner);
                        if let Some(elem) = first_elem.first()
                            && let Some(type_name) =
                                Backend::resolve_arg_text_to_type(elem.trim(), rctx)
                        {
                            subs.insert(tpl_name.clone(), type_name);
                        }
                    }
                } else if let Some(type_name) = Backend::resolve_arg_text_to_type(arg_text, rctx) {
                    // Fallback: treat as direct if not an array literal.
                    subs.insert(tpl_name.clone(), type_name);
                }
            }
            TemplateBindingMode::GenericWrapper(ref wrapper_name, tpl_position) => {
                // For `@param array<TKey, TValue> $value` with a variable
                // argument like `$users`, resolve the variable's raw type
                // string (e.g. `User[]`, `array<int, User>`) and extract
                // the positional generic argument.
                if is_array_like_wrapper(wrapper_name)
                    && arg_text.starts_with('$')
                    && let Some(resolved) = resolve_arg_variable_raw_type(arg_text, rctx)
                    && let Some(concrete) = extract_array_type_at_position(&resolved, tpl_position)
                {
                    subs.insert(tpl_name.clone(), concrete);
                    continue;
                }
                // Fall back to direct resolution for non-array wrappers
                // or when raw type extraction fails.
                if let Some(type_name) = Backend::resolve_arg_text_to_type(arg_text, rctx) {
                    subs.insert(tpl_name.clone(), type_name);
                }
            }
        }
    }

    subs
}

/// Resolve a variable argument to its raw type string.
///
/// For `$pens` with `/** @var Pen[] $pens */`, returns `Some("Pen[]")`.
/// For `$users` with `/** @var array<int, User> $users */`, returns
/// `Some("array<int, User>")`.
///
/// Tries docblock annotations first, then falls back to AST-based
/// raw type inference.
fn resolve_arg_variable_raw_type(
    arg_text: &str,
    rctx: &crate::completion::resolver::ResolutionCtx<'_>,
) -> Option<String> {
    let var_name = arg_text.trim();
    if !var_name.starts_with('$') {
        return None;
    }

    // 1. Try docblock annotation (@var).
    if let Some(raw) = crate::docblock::find_iterable_raw_type_in_source(
        rctx.content,
        rctx.cursor_offset as usize,
        var_name,
    ) {
        return Some(raw);
    }

    // 2. Fall back to AST-based raw type inference.
    let default_class = crate::types::ClassInfo::default();
    let current_class = rctx.current_class.unwrap_or(&default_class);
    super::raw_type_inference::resolve_variable_assignment_raw_type(
        var_name,
        rctx.content,
        rctx.cursor_offset,
        Some(current_class),
        rctx.all_classes,
        rctx.class_loader,
        rctx.function_loader,
    )
}

/// Extract the concrete type at `position` from an array type string.
///
/// For array types with two generic parameters (key + value):
/// - `array<int, User>` at position 0 → `"int"`, position 1 → `"User"`
/// - `User[]` at position 0 → `"int"` (implicit key), position 1 → `"User"`
/// - `list<User>` at position 0 → `"int"`, position 1 → `"User"`
///
/// For single-param forms:
/// - `array<User>` at position 0 → `"User"`
fn extract_array_type_at_position(raw_type: &str, position: usize) -> Option<String> {
    let trimmed = raw_type.trim();

    // `T[]` shorthand → key is int (position 0), value is T (position 1).
    if let Some(base) = trimmed.strip_suffix("[]") {
        let element = crate::docblock::types::clean_type(base);
        return match position {
            0 => Some("int".to_string()),
            1 => Some(element),
            _ => None,
        };
    }

    // `list<T>` → key is int, value is T.
    if trimmed.starts_with("list<") || trimmed.starts_with("non-empty-list<") {
        let (_base, args) = crate::docblock::generics::parse_generic_args(trimmed);
        return match position {
            0 => Some("int".to_string()),
            1 => args
                .first()
                .map(|a| crate::docblock::types::clean_type(a.trim())),
            _ => None,
        };
    }

    // `array<K, V>` or `array<V>`.
    if trimmed.starts_with("array<")
        || trimmed.starts_with("non-empty-array<")
        || trimmed.starts_with("iterable<")
    {
        let (_base, args) = crate::docblock::generics::parse_generic_args(trimmed);
        if args.len() == 2 {
            // `array<K, V>` — position maps directly.
            return args
                .get(position)
                .map(|a| crate::docblock::types::clean_type(a.trim()));
        } else if args.len() == 1 {
            // `array<V>` — position 0 = int (key), position 1 = V.
            return match position {
                0 => Some("int".to_string()),
                1 => args
                    .first()
                    .map(|a| crate::docblock::types::clean_type(a.trim())),
                _ => None,
            };
        }
    }

    None
}

/// Whether a wrapper type name should be treated as array-like for
/// positional generic argument extraction.
///
/// When `@param Wrapper<TKey, TValue> $value` binds a template param
/// via `GenericWrapper`, and the wrapper is an array-like type, we can
/// resolve the argument variable's raw type (e.g. `User[]`) and extract
/// the positional generic component (key at 0, value at 1).
///
/// This covers `array`, `iterable`, `list`, and common Laravel/PHPStan
/// collection interfaces whose generic args follow `<TKey, TValue>`.
fn is_array_like_wrapper(name: &str) -> bool {
    // Compare the short name (last segment after `\`) case-insensitively.
    let short = crate::util::short_name(name);
    matches!(
        short.to_ascii_lowercase().as_str(),
        "array" | "iterable" | "list" | "non-empty-array" | "non-empty-list" | "arrayable"
    )
}

/// Resolve function, method, and static method calls to their return
/// types.
fn resolve_rhs_call<'b>(
    call: &'b Call<'b>,
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    match call {
        Call::Function(func_call) => resolve_rhs_function_call(func_call, expr, ctx),
        Call::Method(method_call) => resolve_rhs_method_call(method_call, expr, ctx),
        Call::StaticMethod(static_call) => resolve_rhs_static_call(static_call, ctx),
        _ => vec![],
    }
}

/// Resolve a plain function call: `someFunc()`, array functions, variable
/// invocations (`$fn()`), and conditional return types.
fn resolve_rhs_function_call<'b>(
    func_call: &'b FunctionCall<'b>,
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let current_class_name: &str = &ctx.current_class.name;
    let all_classes = ctx.all_classes;
    let content = ctx.content;
    let class_loader = ctx.class_loader;
    let function_loader = ctx.function_loader;

    let func_name = match func_call.function {
        Expression::Identifier(ident) => Some(ident.value().to_string()),
        _ => None,
    };

    // ── Known array functions ────────────────────────
    // For element-extracting functions (array_pop, etc.)
    // resolve to the element ClassInfo directly.
    if let Some(ref name) = func_name
        && let Some(element_type) = super::raw_type_inference::resolve_array_func_element_type(
            name,
            &func_call.argument_list,
            ctx,
        )
    {
        let resolved = crate::completion::type_resolution::type_hint_to_classes(
            &element_type,
            current_class_name,
            all_classes,
            class_loader,
        );
        if !resolved.is_empty() {
            return resolved;
        }
    }

    if let Some(name) = func_name
        && let Some(fl) = function_loader
        && let Some(func_info) = fl(&name)
    {
        // Try conditional return type first
        if let Some(ref cond) = func_info.conditional_return {
            let var_resolver = build_var_resolver_from_ctx(ctx);
            let resolved_type = resolve_conditional_with_args(
                cond,
                &func_info.parameters,
                &func_call.argument_list,
                Some(&var_resolver),
            );
            if let Some(ref ty) = resolved_type {
                let resolved = crate::completion::type_resolution::type_hint_to_classes(
                    ty,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }

        // ── Function-level @template substitution ────────────
        // When the function has template params and bindings,
        // infer concrete types from the arguments and apply
        // substitution to the return type before resolving.
        if !func_info.template_params.is_empty()
            && !func_info.template_bindings.is_empty()
            && func_info.return_type.is_some()
        {
            let text_args =
                super::raw_type_inference::extract_argument_text(&func_call.argument_list, content);
            if !text_args.is_empty() {
                let rctx = ctx.as_resolution_ctx();
                let subs = build_function_template_subs(&func_info, &text_args, &rctx);
                if !subs.is_empty()
                    && let Some(ref ret) = func_info.return_type
                {
                    let substituted = crate::inheritance::apply_substitution(ret, &subs);
                    let resolved = crate::completion::type_resolution::type_hint_to_classes(
                        &substituted,
                        current_class_name,
                        all_classes,
                        class_loader,
                    );
                    if !resolved.is_empty() {
                        return resolved;
                    }
                }
            }
        }

        if let Some(ref ret) = func_info.return_type {
            return crate::completion::type_resolution::type_hint_to_classes(
                ret,
                current_class_name,
                all_classes,
                class_loader,
            );
        }
    }

    // ── Variable invocation: $fn() ──────────────────
    // When the callee is a variable (not a named function),
    // resolve the variable's type annotation for a
    // callable/Closure return type, or look for a
    // closure/arrow-function literal in the assignment.
    if let Expression::Variable(Variable::Direct(dv)) = func_call.function {
        let var_name = dv.name.to_string();
        let offset = expr.span().start.offset as usize;

        // 1. Try docblock annotation:
        //    `@var Closure(): User $fn` or
        //    `@param callable(int): Response $fn`
        if let Some(raw_type) =
            crate::docblock::find_iterable_raw_type_in_source(content, offset, &var_name)
            && let Some(ret) = crate::docblock::extract_callable_return_type(&raw_type)
        {
            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                &ret,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                return resolved;
            }
        }

        // 2. Scan for closure literal assignment and
        //    extract native return type hint.
        if let Some(ret) =
            crate::completion::source::helpers::extract_closure_return_type_from_assignment(
                &var_name,
                content,
                ctx.cursor_offset,
            )
        {
            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                &ret,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                return resolved;
            }
        }

        // 3. Scan backward for first-class callable assignment:
        //    `$fn = strlen(...)`, `$fn = $obj->method(...)`, or
        //    `$fn = ClassName::staticMethod(...)`.
        //    Resolve the underlying function/method's return type.
        let rctx = ctx.as_resolution_ctx();
        if let Some(ret) =
            crate::completion::source::helpers::extract_first_class_callable_return_type(
                &var_name, &rctx,
            )
        {
            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                &ret,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                return resolved;
            }
        }

        // 4. Resolve the variable's type and check for __invoke().
        //    When $f holds an object with an __invoke() method,
        //    $f() should return __invoke()'s return type.
        let rctx = ctx.as_resolution_ctx();
        let var_classes = crate::completion::resolver::resolve_target_classes(
            &var_name,
            crate::types::AccessKind::Arrow,
            &rctx,
        );
        for owner in &var_classes {
            if let Some(invoke) = owner.methods.iter().find(|m| m.name == "__invoke")
                && let Some(ref ret) = invoke.return_type
            {
                let resolved = crate::completion::type_resolution::type_hint_to_classes(
                    ret,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }
    }

    // ── General expression invocation: ($expr)() ────
    // When the callee is an arbitrary expression (e.g.
    // `($this->foo)()`, `(getFactory())()`, etc.), resolve
    // the expression to classes and check for __invoke().
    let callee_expr = match func_call.function {
        Expression::Parenthesized(p) => p.expression,
        other => other,
    };
    // Skip if we already handled it as a variable above.
    if !matches!(callee_expr, Expression::Variable(Variable::Direct(_))) {
        // ── Directly invoked closure / arrow function ────
        // `(fn (): Foo => …)()` or `(function (): Foo { … })()`
        // Extract the return type from the literal instead of going
        // through `__invoke()` on the generic `Closure` stub.
        if let Some(ret_type) = extract_closure_or_arrow_return_type(callee_expr) {
            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                &ret_type,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                return resolved;
            }
        }

        let callee_classes = resolve_rhs_expression(callee_expr, ctx);
        for owner in &callee_classes {
            if let Some(invoke) = owner.methods.iter().find(|m| m.name == "__invoke")
                && let Some(ref ret) = invoke.return_type
            {
                let resolved = crate::completion::type_resolution::type_hint_to_classes(
                    ret,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }
    }

    vec![]
}

/// Resolve an instance method call: `$this->method()`, `$var->method()`,
/// chained calls, and other object expressions via AST-based resolution.
fn resolve_rhs_method_call<'b>(
    method_call: &'b MethodCall<'b>,
    _expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let method_name = match &method_call.method {
        ClassLikeMemberSelector::Identifier(ident) => ident.value.to_string(),
        // Variable method name (`$obj->$method()`) — can't resolve statically.
        _ => return vec![],
    };
    // Resolve the object expression to candidate owner classes.
    let owner_classes: Vec<ClassInfo> = if let Expression::Variable(Variable::Direct(dv)) =
        method_call.object
        && dv.name == "$this"
    {
        ctx.all_classes
            .iter()
            .find(|c| c.name == ctx.current_class.name)
            .map(|c| ClassInfo::clone(c))
            .into_iter()
            .collect()
    } else if let Expression::Variable(Variable::Direct(dv)) = method_call.object {
        let var = dv.name.to_string();
        crate::completion::resolver::resolve_target_classes(
            &var,
            crate::types::AccessKind::Arrow,
            &ctx.as_resolution_ctx(),
        )
    } else {
        // Handle non-variable object expressions like
        // `(new Factory())->create()`, `getService()->method()`,
        // or chained calls by recursively resolving the expression.
        resolve_rhs_expression(method_call.object, ctx)
    };

    let text_args =
        super::raw_type_inference::extract_argument_text(&method_call.argument_list, ctx.content);
    let rctx = ctx.as_resolution_ctx();
    let var_resolver = build_var_resolver_from_ctx(ctx);

    for owner in &owner_classes {
        let template_subs = if !text_args.is_empty() {
            Backend::build_method_template_subs(owner, &method_name, &text_args, &rctx)
        } else {
            HashMap::new()
        };
        let mr_ctx = MethodReturnCtx {
            all_classes: ctx.all_classes,
            class_loader: ctx.class_loader,
            template_subs: &template_subs,
            var_resolver: Some(&var_resolver),
            cache: ctx.resolved_class_cache,
        };
        let results = Backend::resolve_method_return_types_with_args(
            owner,
            &method_name,
            &text_args,
            &mr_ctx,
        );
        if !results.is_empty() {
            return results;
        }
    }
    vec![]
}

/// Resolve a static method call: `ClassName::method()`, `self::method()`,
/// `static::method()`.
fn resolve_rhs_static_call(
    static_call: &StaticMethodCall<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let current_class_name: &str = &ctx.current_class.name;

    let class_name = match static_call.class {
        Expression::Self_(_) => Some(current_class_name.to_string()),
        Expression::Static(_) => Some(current_class_name.to_string()),
        Expression::Identifier(ident) => Some(ident.value().to_string()),
        // ── `$var::method()` where `$var` holds a class-string ──
        Expression::Variable(Variable::Direct(dv)) => {
            let var_name = dv.name.to_string();
            let targets =
                crate::completion::variable::class_string_resolution::resolve_class_string_targets(
                    &var_name,
                    ctx.current_class,
                    ctx.all_classes,
                    ctx.content,
                    ctx.cursor_offset,
                    ctx.class_loader,
                );
            targets.first().map(|c| c.name.clone())
        }
        _ => None,
    };
    if let Some(cls_name) = class_name
        && let ClassLikeMemberSelector::Identifier(ident) = &static_call.method
    {
        let method_name = ident.value.to_string();
        let owner = ctx
            .all_classes
            .iter()
            .find(|c| c.name == cls_name)
            .map(|c| ClassInfo::clone(c))
            .or_else(|| (ctx.class_loader)(&cls_name).map(Arc::unwrap_or_clone));
        if let Some(ref owner) = owner {
            let text_args = super::raw_type_inference::extract_argument_text(
                &static_call.argument_list,
                ctx.content,
            );
            let rctx = ctx.as_resolution_ctx();
            let template_subs = if !text_args.is_empty() {
                Backend::build_method_template_subs(owner, &method_name, &text_args, &rctx)
            } else {
                HashMap::new()
            };
            let var_resolver = build_var_resolver_from_ctx(ctx);
            let mr_ctx = MethodReturnCtx {
                all_classes: ctx.all_classes,
                class_loader: ctx.class_loader,
                template_subs: &template_subs,
                var_resolver: Some(&var_resolver),
                cache: ctx.resolved_class_cache,
            };
            return Backend::resolve_method_return_types_with_args(
                owner,
                &method_name,
                &text_args,
                &mr_ctx,
            );
        }
    }
    vec![]
}

/// Resolve property access: `$this->prop`, `$obj->prop`, `$obj?->prop`.
fn resolve_rhs_property_access(access: &Access<'_>, ctx: &VarResolutionCtx<'_>) -> Vec<ClassInfo> {
    let current_class_name: &str = &ctx.current_class.name;
    let all_classes = ctx.all_classes;
    let class_loader = ctx.class_loader;

    // ── Class constant / enum case access: `Foo::BAR` ──
    // When the RHS is a class constant access, resolve the class and
    // check whether the constant is an enum case (→ type is the enum
    // itself) or a typed constant (→ use its type_hint).
    if let Access::ClassConstant(cca) = access {
        let class_name = match cca.class {
            Expression::Identifier(ident) => Some(ident.value().to_string()),
            Expression::Self_(_) => Some(current_class_name.to_string()),
            Expression::Static(_) => Some(current_class_name.to_string()),
            _ => None,
        };
        if let Some(class_name) = class_name {
            let resolved_name = crate::docblock::types::clean_type(&class_name);
            let target_classes = crate::completion::type_resolution::type_hint_to_classes(
                &resolved_name,
                current_class_name,
                all_classes,
                class_loader,
            );

            let const_name = match &cca.constant {
                ClassLikeConstantSelector::Identifier(ident) => Some(ident.value.to_string()),
                _ => None,
            };

            if let Some(const_name) = const_name {
                for cls in &target_classes {
                    // Check if the constant is an enum case — the
                    // result type is the enum class itself.
                    if let Some(c) = cls.constants.iter().find(|c| c.name == const_name) {
                        if c.is_enum_case {
                            return target_classes;
                        }
                        // Typed class constant — resolve via type_hint.
                        if let Some(ref th) = c.type_hint {
                            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                                th,
                                current_class_name,
                                all_classes,
                                class_loader,
                            );
                            if !resolved.is_empty() {
                                return resolved;
                            }
                        }
                    }
                }
            }
        }
        return vec![];
    }

    let (object_expr, prop_selector) = match access {
        Access::Property(pa) => (Some(pa.object), Some(&pa.property)),
        Access::NullSafeProperty(pa) => (Some(pa.object), Some(&pa.property)),
        _ => (None, None),
    };
    if let Some(obj) = object_expr
        && let Some(sel) = prop_selector
    {
        let prop_name = match sel {
            ClassLikeMemberSelector::Identifier(ident) => Some(ident.value.to_string()),
            _ => None,
        };
        if let Some(prop_name) = prop_name {
            let owner_classes: Vec<ClassInfo> = if let Expression::Variable(Variable::Direct(dv)) =
                obj
                && dv.name == "$this"
            {
                all_classes
                    .iter()
                    .find(|c| c.name == current_class_name)
                    .map(|c| ClassInfo::clone(c))
                    .into_iter()
                    .collect()
            } else if let Expression::Variable(Variable::Direct(dv)) = obj {
                let var = dv.name.to_string();
                crate::completion::resolver::resolve_target_classes(
                    &var,
                    crate::types::AccessKind::Arrow,
                    &ctx.as_resolution_ctx(),
                )
            } else {
                // Handle non-variable object expressions like
                // `(new Canvas())->easel`, `getService()->prop`,
                // or `SomeClass::make()->prop` by recursively
                // resolving the expression type.
                resolve_rhs_expression(obj, ctx)
            };

            for owner in &owner_classes {
                let resolved = crate::completion::type_resolution::resolve_property_types(
                    &prop_name,
                    owner,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }
    }
    vec![]
}

/// Resolve `clone $expr` — preserves the cloned expression's type.
///
/// First tries resolving the inner expression structurally (handles
/// `clone new Foo()`, `clone $this->getConfig()`, ternary, etc.).
/// If that yields nothing, falls back to text-based resolution by
/// extracting the source text of the cloned expression and resolving
/// it as a subject string via `resolve_target_classes`.
fn resolve_rhs_clone(clone_expr: &Clone<'_>, ctx: &VarResolutionCtx<'_>) -> Vec<ClassInfo> {
    let structural = resolve_rhs_expression(clone_expr.object, ctx);
    if !structural.is_empty() {
        return structural;
    }
    // Fallback: extract source text of the cloned expression
    // and resolve it as a subject.  This handles cases like
    // `clone $original` where `$original`'s type was set by a
    // prior assignment or parameter type hint.
    let obj_span = clone_expr.object.span();
    let start = obj_span.start.offset as usize;
    let end = obj_span.end.offset as usize;
    if end <= ctx.content.len() {
        let obj_text = ctx.content[start..end].trim();
        if !obj_text.is_empty() {
            let rctx = ctx.as_resolution_ctx();
            return crate::completion::resolver::resolve_target_classes(
                obj_text,
                crate::types::AccessKind::Arrow,
                &rctx,
            );
        }
    }
    vec![]
}

/// Extract the return type hint from a closure or arrow function expression.
///
/// Returns the type-hint string when the expression is a `Closure` or
/// `ArrowFunction` with an explicit return type annotation, e.g.
/// `fn (): Foo => …` yields `"Foo"`.  Returns `None` otherwise.
fn extract_closure_or_arrow_return_type(expr: &Expression<'_>) -> Option<String> {
    match expr {
        Expression::ArrowFunction(arrow) => arrow
            .return_type_hint
            .as_ref()
            .map(|rth| extract_hint_string(&rth.hint)),
        Expression::Closure(closure) => closure
            .return_type_hint
            .as_ref()
            .map(|rth| extract_hint_string(&rth.hint)),
        _ => None,
    }
}
