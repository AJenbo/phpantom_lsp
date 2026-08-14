/// Function/method/static call return-type resolution: resolves
/// function calls, method calls, and static calls to their return
/// types, including template substitution for `@template` parameters
/// and conditional return type evaluation.
use std::collections::HashMap;
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::cst::*;

use crate::Backend;
use crate::atom::{atom, bytes_to_str};
use crate::php_type::{CallableParam, PhpType, TypeKind};
use crate::types::{ClassInfo, ResolvedType};
use crate::virtual_members::laravel::validated_shape;

use crate::type_engine::call_resolution::MethodReturnCtx;
use crate::type_engine::resolver::{Loaders, VarResolutionCtx};
use crate::type_engine::variable::resolution::build_var_resolver_from_ctx;

use super::array_access::{class_string_inner_binding, insert_or_union};
use super::instantiation::{
    TemplateBindingMode, candidate_binding_modes, classify_template_binding,
    extract_array_position, extract_generic_arg_from_ancestor,
};
use super::{
    extract_closure_or_arrow_return_type, resolve_rhs_expression, resolve_var_types,
    resolved_type_with_lookup,
};

/// Apply one binding mode for `tpl_name`, recording whatever it resolves
/// into `subs`.
///
/// Returns without touching `subs` when the mode cannot bind the argument
/// it was given, which is what lets a union `@param` try its alternatives
/// in turn (see [`candidate_binding_modes`]).
#[allow(clippy::too_many_arguments)]
fn apply_template_binding_mode(
    subs: &mut HashMap<String, PhpType>,
    binding_mode: &TemplateBindingMode,
    tpl_name: &str,
    arg_text: &str,
    param_hint: Option<&PhpType>,
    rctx: &crate::type_engine::resolver::ResolutionCtx<'_>,
) {
    match *binding_mode {
        TemplateBindingMode::Direct => {
            if let Some(resolved_type) = Backend::resolve_arg_text_to_type(arg_text, rctx) {
                // An array-literal argument resolves only to the bare
                // `array` keyword, which erases its own keys. Bound
                // directly (no wrapping hint to unify against), that
                // erased shape is all downstream `key-of<T>`/
                // `value-of<T>` would have to work with, so build the
                // literal's real key/value shape instead.
                let bound_type = if resolved_type.is_bare_array() {
                    crate::type_engine::call_resolution::array_literal_shape_type(arg_text, rctx)
                        .unwrap_or(resolved_type)
                } else {
                    resolved_type
                };
                insert_or_union(subs, tpl_name.to_string(), bound_type);
            }
        }
        TemplateBindingMode::CallableReturnType => {
            if let Some(bound) = crate::type_engine::call_resolution::bind_callable_return_template(
                arg_text, param_hint, tpl_name, rctx,
            ) {
                insert_or_union(subs, tpl_name.to_string(), bound);
            }
        }
        TemplateBindingMode::CallableReturnArrayPosition(position) => {
            // `@param callable(...): array<TKey, TValue> $cb` — bind
            // from the key/value of the callback's array-shaped
            // return, not the whole return type.
            if let Some(extracted) = Backend::infer_closure_return_type(arg_text, rctx)
                .and_then(|ret_type| extract_array_position(&ret_type, position))
            {
                insert_or_union(subs, tpl_name.to_string(), extracted);
            }
        }
        TemplateBindingMode::CallableParamType(position) => {
            // `@param Closure(T): void $cb` — extract the closure's
            // parameter type annotation at the given position.
            if let Some(param_type) =
                crate::type_engine::call_resolution::bind_callable_param_template(
                    arg_text, position, rctx,
                )
            {
                insert_or_union(subs, tpl_name.to_string(), param_type);
            }
        }
        TemplateBindingMode::ArrayElement => {
            // `@param T[] $items` — resolve individual array elements.
            if arg_text.starts_with('[') && arg_text.ends_with(']') {
                let inner = arg_text[1..arg_text.len() - 1].trim();
                if inner.is_empty() {
                    // Empty array `[]` → element type is `never`.
                    insert_or_union(subs, tpl_name.to_string(), PhpType::never());
                } else {
                    let first_elem =
                        crate::type_engine::conditional_resolution::split_text_args(inner);
                    if let Some(elem) = first_elem.first()
                        && let Some(resolved_type) =
                            Backend::resolve_arg_text_to_type(elem.trim(), rctx)
                    {
                        insert_or_union(subs, tpl_name.to_string(), resolved_type);
                    }
                }
            } else if let Some(resolved_type) = Backend::resolve_arg_text_to_type(arg_text, rctx)
                .or_else(|| resolve_arg_call_raw_type(arg_text, rctx))
            {
                // Extract the element type from array-like types
                // so we bind T to the element, not the whole array.
                // The call-expression fallback covers arguments whose
                // declared return type is an array (`getConfigs()`
                // returning `array<string, Config>`) — those carry no
                // class info, so the general resolver yields nothing.
                if let Some(elem_type) = resolved_type.extract_value_type(false) {
                    insert_or_union(subs, tpl_name.to_string(), elem_type.clone());
                } else if !resolved_type.is_array_like() {
                    // The argument resolved to a genuine (non-array)
                    // type — bind it directly.  A bare array-like
                    // container whose element type can't be extracted
                    // is left unbound so `T` falls back to its bound
                    // (or `mixed`) rather than binding `T` to `array`.
                    insert_or_union(subs, tpl_name.to_string(), resolved_type);
                }
            }
        }
        TemplateBindingMode::ClassStringInner => {
            if let Some(binding) = class_string_inner_binding(arg_text, rctx) {
                insert_or_union(subs, tpl_name.to_string(), binding);
            }
        }
        TemplateBindingMode::GenericWrapper(ref wrapper_name, tpl_position) => {
            // When the argument is a closure and the param hint
            // union contains a Callable variant, try yield inference
            // before array-like or hierarchy extraction.
            if let Some(concrete) = Backend::try_closure_return_type_for_template(
                arg_text,
                tpl_name,
                tpl_position,
                param_hint,
                rctx,
            ) {
                insert_or_union(subs, tpl_name.to_string(), concrete);
                return;
            }
            // For `@param array<TKey, TValue> $value`, resolve the
            // argument's raw iterable type — from a variable's
            // annotations/assignments (`$users` as `array<int, User>`)
            // or from a call expression's declared return type
            // (`$this->getUsers()` returning `array<int, User>`) —
            // and extract the positional generic argument.
            if is_array_like_wrapper(wrapper_name)
                && let Some(resolved) = resolve_arg_iterable_raw_type(arg_text, rctx)
                && let Some(concrete) = extract_array_type_at_position(&resolved, tpl_position)
            {
                insert_or_union(subs, tpl_name.to_string(), concrete);
                return;
            }
            // Array literal argument for array-like wrappers:
            // `[1, 2, 3]` for `@param array<T>` → infer T from elements.
            if is_array_like_wrapper(wrapper_name)
                && arg_text.starts_with('[')
                && arg_text.ends_with(']')
            {
                let inner = arg_text[1..arg_text.len() - 1].trim();
                if inner.is_empty() {
                    // Empty array `[]` → element type is `never`.
                    insert_or_union(subs, tpl_name.to_string(), PhpType::never());
                    return;
                } else {
                    let elems = crate::type_engine::conditional_resolution::split_text_args(inner);
                    // For `array<T>` (position 0 with 1 generic arg) or
                    // `array<K, V>` (position 1 = value), infer from
                    // element values.  For position 0 in a 2-arg generic
                    // (the key), infer from keys if available.
                    if let Some(elem) = elems.first()
                        && let Some(resolved_type) =
                            Backend::resolve_arg_text_to_type(elem.trim(), rctx)
                    {
                        insert_or_union(subs, tpl_name.to_string(), resolved_type);
                        return;
                    }
                }
            }
            // Special case: unwrap class-string<class-string<T>> to class-string<T>
            if wrapper_name == "class-string"
                && tpl_position == 0
                && let Some(resolved_type) = Backend::resolve_arg_text_to_type(arg_text, rctx)
            {
                if let Some(inner) = resolved_type.unwrap_class_string_inner() {
                    insert_or_union(subs, tpl_name.to_string(), inner.clone());
                } else {
                    insert_or_union(subs, tpl_name.to_string(), resolved_type);
                }
            }
            // ── Class generic wrapper resolution ────────────────
            // For `@param Container<TItem> $c` where the argument
            // is a subclass like `FooContainer extends Container<Foo>`,
            // resolve the argument type and walk its @extends chain
            // to find the wrapper class's generic arg at the right
            // position.
            if !is_array_like_wrapper(wrapper_name)
                && wrapper_name != "class-string"
                && let Some(resolved_type) = Backend::resolve_arg_text_to_type(arg_text, rctx)
                && let Some(concrete) = extract_generic_arg_from_ancestor(
                    &resolved_type,
                    wrapper_name,
                    tpl_position,
                    rctx,
                )
            {
                insert_or_union(subs, tpl_name.to_string(), concrete);
            }
            // When array-type extraction fails (e.g. bare `array`
            // property without generic annotation), do NOT fall back
            // to a Direct resolve — that would bind the template
            // param to the whole argument type instead of its
            // positional generic arg.  Leave it unbound so the
            // "fill in unbound" code below maps it to its declared
            // upper bound or `mixed`.
        }
    }
}

/// Build a template substitution map for a function-level `@template` call.
///
/// Uses the function's `template_bindings` to match template parameters to
/// their concrete types inferred from the call-site arguments.  Handles:
///   - Direct type: `@param T $bar` + `func(new Baz())` → `T = Baz`
///   - Array type: `@param T[] $items` + `func([new X()])` → `T = X`
///   - Generic wrapper: `@param array<TKey, TValue> $v` + `func($users)` →
///     positional resolution through the wrapper's generic arguments.
///
/// Every binding site unions into the substitution rather than
/// overwriting it, so a template bound from several parameters resolves
/// to what all of its arguments have in common: `@param T[] $a, T[] $b`
/// with `combine([1], ['x'])` binds `T` to `int|string`.  Letting the
/// last binding site win would leave every other argument measured
/// against a type taken from one of its siblings.
pub(crate) fn build_function_template_subs(
    func_info: &crate::types::FunctionInfo,
    arg_texts: &[String],
    rctx: &crate::type_engine::resolver::ResolutionCtx<'_>,
) -> HashMap<String, PhpType> {
    let mut subs = HashMap::new();

    // Bind the raw source-order argument texts to parameters by PHP's rules
    // so a named argument (`id: Foo::class`) is routed to the parameter it
    // targets rather than its ordinal slot, and its `name:` prefix is
    // stripped off the value.
    let arg_refs: Vec<&str> = arg_texts.iter().map(|s| s.as_str()).collect();
    let bound = crate::call_args::bind_text_args_to_params(&func_info.parameters, &arg_refs);

    for (tpl_name, param_name) in &func_info.template_bindings {
        let param_idx = match func_info
            .parameters
            .iter()
            .position(|p| p.name == param_name.as_str())
        {
            Some(idx) => idx,
            None => continue,
        };

        let provided_arg = bound.get(param_idx).and_then(|o| o.as_deref());

        // Determine the binding mode by inspecting the parameter's
        // docblock type hint.  The type hint tells us how the template
        // param is embedded in the `@param` annotation.
        let param_hint = func_info
            .parameters
            .get(param_idx)
            .and_then(|p| p.type_hint.as_ref());
        let binding_mode = classify_template_binding(tpl_name, param_hint);

        // Fall back to the parameter's default value only for binding
        // modes where the default is meaningful (class-string<T> with
        // a `Foo::class` default, or direct bindings with `::class`).
        let default_value = func_info
            .parameters
            .get(param_idx)
            .and_then(|p| p.default_value.as_deref());
        let tpl_bound = func_info
            .template_param_bounds
            .get(&crate::atom::atom(tpl_name));
        let arg_text: &str = match provided_arg {
            Some(text) => text,
            // A template bounded by a type operator resolves against the
            // one literal it binds to, and an omitted argument has such a
            // literal whenever the parameter declares a scalar default —
            // known at the declaration site exactly as an explicit
            // argument is known at the call site.
            None => match default_value {
                Some(d)
                    if crate::type_engine::call_resolution::type_operator_bound_literal(
                        tpl_bound, d,
                    )
                    .is_some() =>
                {
                    d
                }
                _ => match &binding_mode {
                    TemplateBindingMode::ClassStringInner => match default_value {
                        Some(d) => d,
                        None => continue,
                    },
                    TemplateBindingMode::Direct => match default_value {
                        Some(d) if d.ends_with("::class") => d,
                        _ => continue,
                    },
                    _ => continue,
                },
            },
        };

        if let Some(literal) =
            crate::type_engine::call_resolution::type_operator_bound_literal(tpl_bound, arg_text)
        {
            insert_or_union(&mut subs, tpl_name.to_string(), literal);
            continue;
        }

        // A union `@param` names one binding site per alternative
        // (`Collection<TKey, TValue>|array<TKey, TValue>`), and the one the
        // argument's own shape matches is the one that should bind. Try
        // them in order and stop at the first that resolves; a non-union
        // hint yields a single mode, which is the path every other
        // parameter takes.
        let before = subs.get(tpl_name.as_str()).cloned();
        for mode in candidate_binding_modes(tpl_name, param_hint) {
            apply_template_binding_mode(&mut subs, &mode, tpl_name, arg_text, param_hint, rctx);
            if subs.get(tpl_name.as_str()) != before.as_ref() {
                break;
            }
        }
    }

    crate::type_engine::call_resolution::finish_template_subs(
        &mut subs,
        &func_info.template_params,
        &func_info.template_param_bounds,
        func_info.return_type.as_ref(),
        rctx,
    );

    subs
}

/// Fill in any function-level `@template` parameter a resolved type still
/// names, using the bindings the call-site arguments provide.
///
/// A conditional return type is evaluated on its own, before the general
/// substitution path runs, so its winning branch can still carry a bare
/// template name (`tap()` returns `TValue`). The arguments are only resolved
/// when the type actually names one, which keeps the common case free.
pub(crate) fn substitute_function_templates(
    func_info: &crate::types::FunctionInfo,
    ty: PhpType,
    arg_texts: &[String],
    rctx: &crate::type_engine::resolver::ResolutionCtx<'_>,
) -> PhpType {
    if !ty.references_any_name(&func_info.template_params) {
        return ty;
    }
    let subs = build_function_template_subs(func_info, arg_texts, rctx);
    if subs.is_empty() {
        return ty;
    }
    ty.substitute(&subs)
}

/// Resolve a variable argument to its raw type string.
///
/// For `$pens` with `/** @var Pen[] $pens */`, returns `Some("Pen[]")`.
/// For `$users` with `/** @var array<int, User> $users */`, returns
/// `Some("array<int, User>")`.
///
/// Tries docblock annotations first, then falls back to AST-based
/// raw type inference.
pub(crate) fn resolve_arg_variable_raw_type(
    arg_text: &str,
    rctx: &crate::type_engine::resolver::ResolutionCtx<'_>,
) -> Option<PhpType> {
    let var_name = arg_text.trim();
    // A property chain is read from its base expression below, whatever the
    // base is spelled as (`$this`, `Labels::MEDICAL`, …); everything after
    // that looks the argument up as a variable, so it has to be one.
    if !var_name.starts_with('$') && !var_name.contains("->") {
        return None;
    }

    // ── Property chain: `$this->items`, `$obj->prop` ────────────
    // When the argument is a property access chain, resolve the base
    // object's type and look up the property's type hint.  This is
    // needed for template substitution in calls like
    // `array_any($this->items, fn($item) => …)` where `$this->items`
    // is `array<int, PurchaseFileProduct>` after generic substitution.
    if let Some(arrow_pos) = var_name.find("->") {
        let base = &var_name[..arrow_pos];
        let prop = &var_name[arrow_pos + 2..];
        // Only handle simple single-level property access for now.
        if !prop.is_empty() && !prop.contains("->") && !prop.contains('(') {
            let base_classes = ResolvedType::into_arced_classes(
                crate::type_engine::resolver::resolve_target_classes(
                    base,
                    crate::types::AccessKind::Arrow,
                    rctx,
                ),
            );
            for cls in &base_classes {
                if let Some(hint) =
                    crate::inheritance::resolve_property_type_hint(cls, prop, rctx.class_loader)
                {
                    return Some(hint);
                }
            }
        }
    }

    // Past this point every lookup is keyed on a variable name, so a chain
    // whose property could not be read has nothing left to answer it.
    if !var_name.starts_with('$') {
        return None;
    }

    // 1. Try docblock annotation (@var).
    if let Some(raw) = crate::docblock::find_iterable_raw_type_in_source(
        rctx.content,
        rctx.cursor_offset as usize,
        var_name,
    )
    .map(|t| crate::util::resolve_php_type_names(&t, rctx.class_loader))
    {
        return Some(raw);
    }

    // 2. When the diagnostic scope cache is active (and not still being
    //    built), read the variable's type from the pre-computed forward-
    //    walked scope snapshots.  This avoids hitting the backward
    //    scanner during diagnostic collection.
    if crate::type_engine::variable::forward_walk::is_diagnostic_scope_active()
        && !crate::type_engine::variable::forward_walk::is_building_scopes()
    {
        let prefixed = if var_name.starts_with('$') {
            var_name.to_string()
        } else {
            format!("${}", var_name)
        };
        if let Some(types) = crate::type_engine::variable::forward_walk::lookup_diagnostic_scope(
            &prefixed,
            rctx.cursor_offset,
        ) {
            return Some(ResolvedType::types_joined(&types));
        }
    }

    // 3. When a scope_var_resolver is available (forward walker is
    //    active on either diagnostic or completion path), read from
    //    the in-progress ScopeState.  If the variable isn't there,
    //    it hasn't been assigned yet — return None rather than
    //    falling through to resolve_variable_types which would
    //    re-enter the forward walker and cause stack overflow.
    if let Some(resolver) = rctx.scope_var_resolver {
        let prefixed = if var_name.starts_with('$') {
            var_name.to_string()
        } else {
            format!("${}", var_name)
        };
        let from_scope = resolver(&prefixed);
        if from_scope.is_empty() {
            return None;
        }
        return Some(ResolvedType::types_joined(&from_scope));
    }

    // 4. During the build phase, the forward walker is the authority.
    //    If the variable isn't in the scope cache, don't fall through
    //    to the backward scanner — return None so the caller treats
    //    it as unresolved.
    if crate::type_engine::variable::forward_walk::is_building_scopes() {
        return None;
    }

    // 5. Fall back to unified variable resolution pipeline (backward
    //    scanner).  This path is only reached for interactive features
    //    (hover, completion, goto-def) where no scope cache is active
    //    and no scope_var_resolver was provided.
    //
    // Guard: resolve_variable_types is designed for bare `$variable`
    // names.  Complex expressions (array access like `$arr['key']`,
    // comparisons like `$x === 'foo'`, boolean chains, null coalescing)
    // are not variable names and will never match a scope entry.
    // Skip them to avoid wasted backward scans and fallthrough noise.
    if var_name.contains("->")
        || var_name.contains("::")
        || var_name.contains('[')
        || var_name.contains("===")
        || var_name.contains("&&")
        || var_name.contains("??")
        || var_name.contains("||")
    {
        return None;
    }

    let default_class;
    let current_class = match rctx.current_class {
        Some(cc) => cc,
        None => {
            default_class =
                crate::class_lookup::class_context_placeholder(rctx.content, rctx.cursor_offset);
            &default_class
        }
    };
    let resolved = crate::type_engine::variable::resolution::resolve_variable_types(
        var_name,
        current_class,
        rctx.all_classes,
        rctx.content,
        rctx.cursor_offset,
        rctx.class_loader,
        rctx.backend,
        Loaders::with_function(rctx.function_loader),
    );
    if resolved.is_empty() {
        None
    } else {
        Some(ResolvedType::types_joined(&resolved))
    }
}

/// Resolve a call-expression argument (`$obj->method()`, `self::method()`,
/// `helper()`) to its declared return type, preserving generic arguments
/// that don't resolve to loadable classes (e.g. `array<string, Config>`).
///
/// Routes through the shared call-resolution pipeline
/// (`resolve_call_return_types_expr_with_hint`) so class-level and
/// method-level template substitutions apply to the returned type.
/// Returns `None` when the text is not a call expression or the callee
/// has no declared return type.
pub(super) fn resolve_arg_call_raw_type(
    arg_text: &str,
    rctx: &crate::type_engine::resolver::ResolutionCtx<'_>,
) -> Option<PhpType> {
    let trimmed = arg_text.trim();
    if !trimmed.ends_with(')') {
        return None;
    }
    // Closure/arrow-function literals also end with `)` but are not
    // call expressions — their types are handled by the callable
    // binding modes, not here.
    if crate::completion::source::helpers::is_closure_like_text(trimmed) {
        return None;
    }
    let expr = crate::type_engine::subject_expr::SubjectExpr::parse(trimmed);
    let crate::type_engine::subject_expr::SubjectExpr::CallExpr { callee, args_text } = &expr
    else {
        return None;
    };
    let mut hint: Option<PhpType> = None;
    Backend::resolve_call_return_types_expr_with_hint(callee, args_text, rctx, Some(&mut hint));
    hint
}

/// Resolve an argument's raw iterable type for positional generic
/// extraction, regardless of the argument's syntax shape.
///
/// Variables and property chains resolve through
/// [`resolve_arg_variable_raw_type`] (docblock annotations, forward-walk
/// scope, assignment scanning); call expressions resolve through the
/// shared call return-type pipeline via [`resolve_arg_call_raw_type`].
pub(super) fn resolve_arg_iterable_raw_type(
    arg_text: &str,
    rctx: &crate::type_engine::resolver::ResolutionCtx<'_>,
) -> Option<PhpType> {
    resolve_arg_variable_raw_type(arg_text, rctx)
        .or_else(|| resolve_arg_call_raw_type(arg_text, rctx))
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
pub(super) fn extract_array_type_at_position(ty: &PhpType, position: usize) -> Option<PhpType> {
    match position {
        // A `list<V>` writes no key argument but is not silent about its
        // keys: PHP defines a list as sequentially `int`-indexed from 0.
        // Without this, `array_keys(list<User>)` binds `TKey` to nothing
        // and reports `list<mixed>`. A single-argument `array<V>` is a
        // different case and deliberately absent: its key is `array-key`,
        // which is what the unbound declaration already says.
        0 => ty
            .extract_key_type(false)
            .cloned()
            .or_else(|| implicit_list_key_type(ty)),
        1 => ty.extract_value_type(false).cloned(),
        _ => None,
    }
}

/// The `int` key type a `list<V>` implies, or `None` for any other type.
fn implicit_list_key_type(ty: &PhpType) -> Option<PhpType> {
    match ty.kind() {
        TypeKind::Generic(g)
            if g.args.len() == 1
                && matches!(
                    g.name.to_ascii_lowercase().as_str(),
                    "list" | "non-empty-list"
                ) =>
        {
            Some(PhpType::int())
        }
        TypeKind::Nullable(inner) => implicit_list_key_type(inner),
        TypeKind::Union(members) => members.iter().find_map(implicit_list_key_type),
        _ => None,
    }
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
pub(crate) fn is_array_like_wrapper(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "array" | "list" | "non-empty-array" | "non-empty-list" | "iterable"
    ) || crate::util::short_name(name).eq_ignore_ascii_case("arrayable")
}

/// Resolve function, method, and static method calls to their return
/// types.
pub(super) fn resolve_rhs_call<'b>(
    call: &'b Call<'b>,
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ResolvedType> {
    let mut resolved = match call {
        Call::Function(func_call) => resolve_rhs_function_call(func_call, expr, ctx),
        Call::Method(method_call) => resolve_rhs_method_call_inner(
            method_call.object,
            &method_call.method,
            &method_call.argument_list,
            ctx,
        ),
        Call::NullSafeMethod(method_call) => resolve_rhs_method_call_inner(
            method_call.object,
            &method_call.method,
            &method_call.argument_list,
            ctx,
        ),
        Call::StaticMethod(static_call) => resolve_rhs_static_call(static_call, ctx),
    };

    // A `@return value-of<ID_TABLE>` arrives here with the operator still
    // standing: the docblock parser saw a name it could not read, and only
    // the template path reads the constant behind it.  Finish it so the
    // caller gets the value union the table describes rather than a type
    // expression that widens to `mixed`.
    if resolved
        .iter()
        .any(|rt| rt.type_string.contains_unevaluated_operator())
    {
        let rctx = ctx.as_resolution_ctx();
        for rt in &mut resolved {
            if let Some(evaluated) = crate::type_engine::call_resolution::evaluate_constant_operands(
                &rt.type_string,
                &rctx,
            ) {
                rt.type_string = evaluated;
            }
        }
    }

    resolved
}

pub(crate) fn infer_closure_literal_type(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> PhpType {
    let explicit_or_yield = {
        let span = expr.span();
        let start = (span.start.offset as usize).min(ctx.content.len());
        let end = (span.end.offset as usize).min(ctx.content.len());
        ctx.content.get(start..end).and_then(|text| {
            crate::completion::source::helpers::extract_closure_return_type_from_text(text).or_else(
                || {
                    crate::completion::source::helpers::infer_generator_type_from_closure_yields(
                        text,
                    )
                },
            )
        })
    };

    let inferred_return = explicit_or_yield.or_else(|| match expr {
        Expression::ArrowFunction(arrow) => {
            let resolved = resolve_rhs_expression(arrow.expression, ctx);
            if resolved.is_empty() {
                None
            } else {
                Some(ResolvedType::types_joined(&resolved))
            }
        }
        // First-class callable syntax: `strlen(...)`, `$this->method(...)`,
        // `ClassName::method(...)`.  Resolve the underlying function/method's
        // return type from the callable's own source text.
        Expression::PartialApplication(_) => {
            let span = expr.span();
            let start = (span.start.offset as usize).min(ctx.content.len());
            let end = (span.end.offset as usize).min(ctx.content.len());
            ctx.content.get(start..end).and_then(|text| {
                let rctx = ctx.as_resolution_ctx();
                crate::completion::source::helpers::resolve_first_class_callable_return_type(
                    text, &rctx,
                )
            })
        }
        _ => None,
    });

    let params = declared_closure_params(expr, ctx);
    if inferred_return.is_some() || !params.is_empty() {
        PhpType::callable_spec("Closure", params, inferred_return)
    } else {
        PhpType::closure()
    }
}

/// The parameter list a closure or arrow function literal declares, as
/// callable-signature parameters.
///
/// A literal's arity and parameter types are part of the type it produces:
/// `fn (BrandView $b) => …` is a `Closure(BrandView): …`, and dropping the
/// parameters makes it fail every declared `Closure(BrandView): …` it is
/// handed to. A parameter with no native hint contributes `mixed`, which a
/// contravariant check accepts from any expected parameter type; a hinted
/// one goes through the class loader so the short name written in the
/// literal matches the fully-qualified name in the expectation.
fn declared_closure_params(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<CallableParam> {
    let parameter_list = match expr {
        Expression::Closure(closure) => &closure.parameter_list,
        Expression::ArrowFunction(arrow) => &arrow.parameter_list,
        _ => return Vec::new(),
    };

    parameter_list
        .parameters
        .iter()
        .map(|param| CallableParam {
            type_hint: param
                .hint
                .as_ref()
                .map(|hint| {
                    crate::util::resolve_php_type_names(
                        &crate::parser::extract_hint_type(hint),
                        ctx.class_loader,
                    )
                })
                .unwrap_or_else(PhpType::mixed),
            optional: param.default_value.is_some(),
            variadic: param.ellipsis.is_some(),
        })
        .collect()
}

/// Resolve a plain function call: `someFunc()`, array functions, variable
/// invocations (`$fn()`), and conditional return types.
pub(super) fn resolve_rhs_function_call<'b>(
    func_call: &'b FunctionCall<'b>,
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ResolvedType> {
    let current_class_name: &str = &ctx.current_class.name;
    let all_classes = ctx.all_classes;
    let content = ctx.content;
    let class_loader = ctx.class_loader;
    let function_loader = ctx.function_loader();

    // ── First-class callable invocation: `Foo::method(...)()` ───
    // When the callee is a partial application (first-class callable),
    // invoking it with `()` returns the underlying method's return
    // type.  Delegate to the matching call-resolution path.
    if let Expression::PartialApplication(pa) = func_call.function {
        use mago_syntax::cst::partial_application::PartialApplication;
        match pa {
            PartialApplication::StaticMethod(sma) => {
                // Build a synthetic StaticMethodCall and resolve it.
                let synthetic = mago_syntax::cst::call::StaticMethodCall {
                    class: sma.class,
                    double_colon: sma.double_colon,
                    method: sma.method.clone(),
                    argument_list: func_call.argument_list.clone(),
                };
                return resolve_rhs_static_call(&synthetic, ctx);
            }
            PartialApplication::Method(ma) => {
                return resolve_rhs_method_call_inner(
                    ma.object,
                    &ma.method,
                    &func_call.argument_list,
                    ctx,
                );
            }
            PartialApplication::Function(fa) => {
                // `strlen(...)()` — resolve the inner function name.
                if let Expression::Identifier(ident) = fa.function {
                    let name = bytes_to_str(ident.value()).to_string();
                    let name_offset = ident.span().start.offset;
                    let function_loader = ctx.function_loader();
                    if let Some(fl) = function_loader
                        && let Some(func_info) = fl(&name, name_offset)
                        && let Some(ref ret) = func_info.return_type
                    {
                        let resolved =
                            crate::type_engine::type_resolution::type_hint_to_classes_typed(
                                ret,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            );
                        if !resolved.is_empty() {
                            return ResolvedType::from_classes_with_hint(resolved, ret.clone());
                        }
                        return vec![resolved_type_with_lookup(
                            ret.clone(),
                            &ctx.current_class.name,
                            ctx.all_classes,
                            ctx.class_loader,
                        )];
                    }
                }
            }
        }
    }

    let func_name = match func_call.function {
        Expression::Identifier(ident) => Some(bytes_to_str(ident.value()).to_string()),
        _ => None,
    };
    // Byte offset of the function-name identifier, so the loader can
    // consult mago-names' per-offset resolution.  This is what lets a
    // call resolve to a function declared in a *different* `namespace`
    // block of the same file (the file-level namespace guess would miss).
    let func_name_offset = func_call.function.span().start.offset;

    // ── Laravel container string binding ────────────────
    // `$var = app('blade.compiler')` / `$var = resolve('cache')` bind a
    // plain string to a concrete class via the framework's container
    // alias table. Mirrors the direct-call-subject interception in
    // call_resolution.rs so the binding survives being assigned to a
    // variable instead of being chained off the call directly.
    if let Some(ref name) = func_name {
        let normalized_func = name.trim_start_matches('\\');
        if matches!(normalized_func, "app" | "resolve") {
            let arg_texts =
                crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                    &func_call.argument_list,
                    content,
                );
            if let Some(first_arg) = arg_texts.first()
                && let Some(alias) = crate::util::unescape_php_string_literal(first_arg.trim())
                && let Some(cls) = (ctx.class_loader)(&alias)
            {
                return ResolvedType::from_classes(vec![cls]);
            }
        }

        // ── now() / today() → configured Laravel date class ──
        // Laravel's `now()`/`today()` helpers are declared to return
        // `CarbonInterface`, but they instantiate the concrete class selected
        // by Laravel's date factory.
        // Resolving to the interface loses the concrete type and
        // produces spurious mismatches when the value flows into a
        // `DateTime`/`DateTimeImmutable` declaration.  Map both to the
        // concrete class.
        //
        // This is not strictly sound (the helpers' declared type is the
        // interface), but it mirrors Larastan's `NowAndTodayExtension`.
        // The Laravel/Carbon ecosystem is written against that model, so
        // real codebases assume the concrete type; matching it avoids a
        // flood of mismatches that only exist because the declared types
        // are looser than reality.
        if matches!(
            normalized_func,
            "now" | "today" | "Illuminate\\Support\\now" | "Illuminate\\Support\\today"
        ) && let Some(cls) =
            (ctx.class_loader)(crate::virtual_members::laravel::CONFIGURED_DATE_CLASS_FQN)
        {
            return ResolvedType::from_classes(vec![cls]);
        }

        // ── view('name') → concrete Illuminate\View\View ──
        // The helper's conditional return type names the *contract*, but
        // the factory always builds the concrete view object.  Mirrors
        // Larastan's `view()` stub.
        if normalized_func.trim_start_matches('\\') == "view" {
            let arg_texts =
                crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                    &func_call.argument_list,
                    content,
                );
            if crate::virtual_members::laravel::view_helper_returns_view(
                normalized_func,
                arg_texts.first().map(String::as_str).unwrap_or(""),
            ) && let Some(cls) = (ctx.class_loader)(crate::virtual_members::laravel::VIEW_FQN)
            {
                return ResolvedType::from_classes(vec![cls]);
            }
        }
    }

    // ── Laravel config() return type inference ───────
    if let Some(ref name) = func_name {
        let normalized_func = name.trim_start_matches('\\');
        if matches!(normalized_func, "config" | "Illuminate\\Support\\config") {
            let arg_texts =
                crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                    &func_call.argument_list,
                    content,
                );
            if let Some(first_arg) = arg_texts.first()
                && let Some(key) = crate::util::unescape_php_string_literal(first_arg.trim())
                && !key.is_empty()
                && !key.contains('$')
                && let Some(resolver) = ctx.loaders.config_resolver
                && let Some(ty) = resolver(&key)
            {
                return vec![ResolvedType::from_type_string(ty)];
            }
        }
    }

    // ── Laravel translation helper return type narrowing ─────
    // `trans()`/`__()` declare a return type of `string|array|null` because
    // a translation key may name a whole group, and the keyless form hands
    // the key straight back. The key decides which branch a call takes:
    // a leaf entry is a `string`, a group is the array beneath it, and
    // `__()` with no key at all is the `null`.
    if let Some(ref name) = func_name
        && let Some(resolver) = ctx.loaders.trans_resolver
    {
        let normalized_func = name.trim_start_matches('\\');
        if matches!(normalized_func, "trans" | "__") {
            let arg_texts =
                crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                    &func_call.argument_list,
                    content,
                );
            match arg_texts.first() {
                Some(first_arg) => {
                    let ty = crate::util::unescape_php_string_literal(first_arg.trim())
                        .filter(|key| !key.is_empty() && !key.contains('$'))
                        .and_then(|key| resolver(&key))
                        .unwrap_or_else(crate::virtual_members::laravel::unresolved_trans_type);
                    return vec![ResolvedType::from_type_string(ty)];
                }
                // `trans()` with no key hands back the translator itself, a
                // shape this narrowing does not model; `__()` returns the
                // null it was given.
                None if normalized_func == "__" => {
                    return vec![ResolvedType::from_type_string(PhpType::null())];
                }
                None => {}
            }
        }
    }

    // ── Known array functions ────────────────────────
    // For element-extracting functions (array_pop, etc.)
    // resolve to the element ClassInfo directly.
    if let Some(ref name) = func_name
        && let Some(element_type) =
            crate::type_engine::variable::raw_type_inference::resolve_array_func_element_type(
                name,
                &func_call.argument_list,
                ctx,
            )
    {
        let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
            &element_type,
            current_class_name,
            all_classes,
            class_loader,
        );
        if !resolved.is_empty() {
            return ResolvedType::from_classes_with_hint(resolved, element_type);
        }
        // The element type is not class-like (`array_pop` on a
        // `list<list<int>>` yields `list<int>`), but it is still the right
        // answer. Returning it as a type-string-only result keeps the one
        // level of unwrapping; falling through to the raw-type branch below
        // would hand back the container type unchanged.
        return vec![resolved_type_with_lookup(
            element_type,
            current_class_name,
            all_classes,
            class_loader,
        )];
    }

    // For type-preserving functions (array_filter, array_values, etc.)
    // the output has the same iterable type as the input array.
    // Return the full type string (e.g. `list<User>`) so that
    // downstream consumers (foreach, array access, hover) see the
    // element type without needing the raw-type pipeline's fallback.
    if let Some(ref name) = func_name
        && let Some(raw_type) =
            crate::type_engine::variable::raw_type_inference::resolve_array_func_raw_type(
                name,
                &func_call.argument_list,
                ctx,
            )
    {
        let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
            &raw_type,
            current_class_name,
            all_classes,
            class_loader,
        );
        if !resolved.is_empty() {
            return ResolvedType::from_classes_with_hint(resolved, raw_type);
        }
        // The type string is informative (e.g. `list<User>`) but
        // doesn't resolve to a class — return as type-string-only.
        return vec![resolved_type_with_lookup(
            raw_type,
            current_class_name,
            all_classes,
            class_loader,
        )];
    }

    if let Some(ref name) = func_name
        && let Some(fl) = function_loader
        && let Some(func_info) = fl(name, func_name_offset)
    {
        // A return type the call site decides (a conditional keyed on an
        // argument, or a branch the flags argument rules out) is
        // authoritative, so it is tried before the declared type.
        if func_info.conditional_return.is_some()
            || crate::type_engine::types::flag_returns::has_flag_dependent_return(name)
        {
            let var_resolver = build_var_resolver_from_ctx(ctx);
            let rctx = ctx.as_resolution_ctx();
            // `is <Type>` conditions on an argument that isn't a literal
            // (`preg_replace($p, $r, $subject)`) are decided by the
            // argument's resolved type, so the branch a call takes matches
            // what it was actually handed.
            let arg_ty_resolver = |t: &str| Backend::resolve_arg_text_to_type(t, &rctx);
            let text_args =
                crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                    &func_call.argument_list,
                    content,
                )
                .join(", ");
            let resolved_type = func_info.conditional_return.as_ref().and_then(|cond| {
                let tpl = crate::type_engine::types::conditional::TemplateContext {
                    defaults: None,
                    params: &func_info.template_params,
                    bindings: &func_info.template_bindings,
                    arg_type_resolver: Some(&arg_ty_resolver),
                };
                crate::type_engine::conditional_resolution::resolve_conditional_with_text_args_and_defaults(
                    cond,
                    &func_info.parameters,
                    &text_args,
                    Some(&var_resolver),
                    crate::type_engine::conditional_resolution::ConditionalClassContext {
                        calling: Some(current_class_name),
                        declaring: None,
                    },
                    class_loader,
                    &tpl,
                )
            })
            // A branch the flags argument rules out (`json_encode(…,
            // JSON_THROW_ON_ERROR)` never returning `false`) is decided the
            // same way: at the call site, from the declared return type.
            .or_else(|| {
                crate::type_engine::types::flag_returns::flag_narrowed_return_type(
                    name,
                    &func_info.parameters,
                    &text_args,
                    func_info.return_type.as_ref()?,
                    Some(&arg_ty_resolver),
                )
            });
            if let Some(ty) = resolved_type {
                // The winning branch can name a function-level `@template`
                // (`tap()` returns `TValue`), which only the call-site
                // arguments fill in.
                let ty = substitute_function_templates(
                    &func_info,
                    ty,
                    &crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                        &func_call.argument_list,
                        content,
                    ),
                    &rctx,
                );
                let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                    &ty,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return ResolvedType::from_classes_with_hint(resolved, ty.clone());
                }
                // The conditional resolved to a non-class type (e.g.
                // `list<string>`, `int`).  Return it as a type-string-only
                // entry so downstream consumers see the resolved type.
                return vec![resolved_type_with_lookup(
                    ty,
                    current_class_name,
                    all_classes,
                    class_loader,
                )];
            }
        }

        // ── Function-level @template substitution ────────────
        // When the function has template params and bindings,
        // infer concrete types from the arguments and apply
        // substitution to the return type before resolving.
        if !func_info.template_params.is_empty() && func_info.return_type.is_some() {
            let arg_texts =
                crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                    &func_call.argument_list,
                    content,
                );
            let rctx = ctx.as_resolution_ctx();
            let subs = build_function_template_subs(&func_info, &arg_texts, &rctx);
            if !subs.is_empty()
                && let Some(ref ret) = func_info.return_type
            {
                let substituted = ret.substitute(&subs);
                let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                    &substituted,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return ResolvedType::from_classes_with_hint(resolved, substituted);
                }
                // The substituted type didn't resolve to any classes
                // (e.g. `mixed|null`, `int|null`, `array-key|null`).
                // Return it as a type-string-only entry so that
                // downstream consumers see the substituted type
                // instead of the raw template name.
                return vec![ResolvedType::from_type_string(substituted)];
            }
        }

        if let Some(ref ret) = func_info.return_type {
            let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                ret,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                return ResolvedType::from_classes_with_hint(resolved, ret.clone());
            }
            // The function has a return type string but
            // `type_hint_to_classes_typed` found no matching class (e.g.
            // `list<Widget>`, `int`, `array{name: string}`).  Return a
            // type-string-only entry so that consumers reading
            // `.type_string` still get the information.
            //
            // When the return type is `void`, PHP yields `null` at
            // runtime — mirror that so the variable type is correct.
            if *ret == PhpType::void() {
                return vec![ResolvedType::from_type_string(PhpType::null())];
            }
            return vec![resolved_type_with_lookup(
                ret.clone(),
                current_class_name,
                all_classes,
                class_loader,
            )];
        }
    }

    // ── Variable invocation: $fn() ──────────────────
    // When the callee is a variable (not a named function),
    // resolve the variable's type annotation for a
    // callable/Closure return type, or look for a
    // closure/arrow-function literal in the assignment.
    if let Expression::Variable(Variable::Direct(dv)) = func_call.function {
        let var_name = bytes_to_str(dv.name).to_string();
        let offset = expr.span().start.offset as usize;

        // 1. Try docblock annotation:
        //    `@var Closure(): User $fn` or
        //    `@param callable(int): Response $fn`
        if let Some(raw_type) =
            crate::docblock::find_iterable_raw_type_in_source(content, offset, &var_name)
                .map(|t| crate::util::resolve_php_type_names(&t, class_loader))
            && let Some(ret_type) = raw_type.callable_return_type()
        {
            let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                ret_type,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                return ResolvedType::from_classes_with_hint(resolved, ret_type.clone());
            }
        }

        // 2. Resolve the variable's own type.  Closures, arrow functions,
        //    and first-class callables are all inferred by
        //    `resolve_rhs_expression` as a `TypeKind::Callable` (see
        //    `infer_closure_literal_type`), so `$fn`'s embedded return
        //    type covers `$fn = function(): T {}`, `$fn = fn(): T => …`,
        //    and `$fn = strlen(...)` / `$fn = $obj->method(...)` alike.
        let var_types = resolve_var_types(&var_name, ctx, ctx.cursor_offset);
        for rt in &var_types {
            if let Some(ret_type) = rt.type_string.callable_return_type() {
                let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                    ret_type,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return ResolvedType::from_classes_with_hint(resolved, ret_type.clone());
                }
            }
        }

        // 3. Check for __invoke().  When $f holds an object with an
        //    __invoke() method, $f() should return __invoke()'s return
        //    type.
        let var_classes = ResolvedType::into_arced_classes(var_types);
        for owner in &var_classes {
            if let Some(invoke) = owner.get_method("__invoke")
                && let Some(ref ret) = invoke.return_type
            {
                let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                    ret,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return ResolvedType::from_classes_with_hint(resolved, ret.clone());
                }
                // When type_hint_to_classes_typed can't resolve the return
                // type (e.g. `Item[]` where the `[]` suffix prevents
                // class lookup), emit a type-string-only entry so that
                // callers like foreach resolution can still extract the
                // element type via `PhpType::extract_value_type`.
                if !ret.is_empty() {
                    return vec![resolved_type_with_lookup(
                        ret.clone(),
                        current_class_name,
                        all_classes,
                        class_loader,
                    )];
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
        if let Some(parsed_ret_type) = extract_closure_or_arrow_return_type(callee_expr) {
            let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                &parsed_ret_type,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                return ResolvedType::from_classes_with_hint(resolved, parsed_ret_type);
            }
        }

        let callee_results = resolve_rhs_expression(callee_expr, ctx);
        for rt in &callee_results {
            if let Some(ref owner_cls) = rt.class_info
                && let Some(invoke) = owner_cls.get_method("__invoke")
                && let Some(ref ret) = invoke.return_type
            {
                let resolved = crate::type_engine::type_resolution::type_hint_to_classes_typed(
                    ret,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    return ResolvedType::from_classes_with_hint(resolved, ret.clone());
                }
                if !ret.is_empty() {
                    return vec![resolved_type_with_lookup(
                        ret.clone(),
                        current_class_name,
                        all_classes,
                        class_loader,
                    )];
                }
            }
        }
    }

    vec![]
}

/// A method call's receiver, already resolved: the candidate owner classes
/// plus the `ResolvedType` values they came from.
///
/// The full `ResolvedType` is kept alongside the classes so that the
/// receiver's generic type string (e.g. `Builder<Article>`) is available
/// when the method returns `static`/`self`/`$this`.
pub(super) type MethodReceiver = (Vec<Arc<ClassInfo>>, Vec<ResolvedType>);

/// Resolve a method call's object expression to its receiver.
///
/// `$this` resolves to the enclosing class, a bare variable through the
/// variable pipeline (honouring `match(true)` arm narrowing), and anything
/// else — `(new Factory())`, `getService()`, a chain link — by resolving the
/// expression.
fn resolve_method_receiver<'b>(
    object: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> MethodReceiver {
    if let Expression::Variable(Variable::Direct(dv)) = object
        && dv.name == b"$this"
    {
        let classes: Vec<Arc<ClassInfo>> = ctx
            .all_classes
            .iter()
            .find(|c| c.name == ctx.current_class.name)
            .map(Arc::clone)
            .into_iter()
            .collect();
        return (classes, vec![]);
    }
    if let Expression::Variable(Variable::Direct(dv)) = object {
        let var = bytes_to_str(dv.name).to_string();
        // Check match-arm narrowing override first — when inside
        // a match(true) arm, the variable may be narrowed to a
        // specific class by the arm's instanceof condition.
        let resolved = match ctx.match_arm_narrowing.get(&var).cloned() {
            Some(overridden) => overridden,
            None => resolve_var_types(&var, ctx, object.span().end.offset),
        };
        if !resolved.is_empty() {
            let classes = ResolvedType::into_arced_classes(resolved.clone());
            return (classes, resolved);
        }
        // Fall back to resolve_target_classes when the variable
        // resolution pipeline returns nothing (e.g. for parameters
        // that are resolved through the completion pipeline's subject
        // resolution).
        let classes: Vec<Arc<ClassInfo>> =
            ResolvedType::into_arced_classes(crate::type_engine::resolver::resolve_target_classes(
                &var,
                crate::types::AccessKind::Arrow,
                &ctx.as_resolution_ctx(),
            ));
        return (classes, vec![]);
    }
    let resolved = resolve_rhs_expression(object, ctx);
    let classes = ResolvedType::into_arced_classes(resolved.clone());
    (classes, resolved)
}

/// Resolve a method call (regular or null-safe) from its constituent parts:
/// the object expression (`$this`, a variable, or an arbitrary chained
/// expression), the method selector, and the argument list.
///
/// Both `$obj->method()` and `$obj?->method()` share the same resolution
/// logic — the null-safe operator only affects whether `null` propagates
/// at runtime, not which class the method belongs to.
pub(super) fn resolve_rhs_method_call_inner<'b>(
    object: &'b Expression<'b>,
    method: &'b ClassLikeMemberSelector<'b>,
    argument_list: &'b ArgumentList<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ResolvedType> {
    resolve_method_call_on_receiver(object, method, argument_list, None, ctx)
}

/// Resolve a method call whose receiver may already be known.
///
/// A `Some(receiver)` skips resolving `object`, which is how a fluent chain
/// is walked outward from its base without recursing into each link (see
/// `resolve_method_chain` in the parent module).  `object` is still needed
/// for the parts of resolution that read the receiver's *syntax* rather
/// than its type: whether the call forwards late static binding, and which
/// request a Laravel validation shape belongs to.
pub(super) fn resolve_method_call_on_receiver<'b>(
    object: &'b Expression<'b>,
    method: &'b ClassLikeMemberSelector<'b>,
    argument_list: &'b ArgumentList<'b>,
    receiver: Option<MethodReceiver>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ResolvedType> {
    let method_name = match method {
        ClassLikeMemberSelector::Identifier(ident) => bytes_to_str(ident.value).to_string(),
        // Variable method name (`$obj->$method()`) — can't resolve statically.
        _ => return vec![],
    };
    let (owner_classes, receiver_resolved) =
        receiver.unwrap_or_else(|| resolve_method_receiver(object, ctx));

    let arg_texts = crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
        argument_list,
        ctx.content,
    );
    let arg_refs: Vec<&str> = arg_texts.iter().map(|s| s.as_str()).collect();
    let rctx = ctx.as_resolution_ctx();

    // ── Expand union generic receivers ──────────────────────────
    // When the receiver is a union type like `C<A>|C<B>`, the variable
    // resolution pipeline returns a single ResolvedType with a Union
    // type_string and one class_info.  To resolve the method on each
    // branch separately (so `->get()` yields `A|B` not just `A`),
    // expand the union into separate owner entries with per-branch
    // generic substitutions applied.
    let (owner_classes, receiver_resolved) =
        expand_union_generic_owners(owner_classes, receiver_resolved, ctx);

    // Laravel validated input: the rules that guard this request describe the
    // array it hands back, so `$data = $request->validated()` gets a shape
    // rather than plain `array`.  Classifying the call does not depend on the
    // receiver, so it happens once rather than once per owner.
    //
    // `validate([…])` reads its rules from its own argument, so it works
    // wherever it is written; every other form asks the scope for them and is
    // a no-op without the server state that reads them.
    let shape_call = validated_shape::shape_bearing_method(&method_name)
        .filter(|call| *call == validated_shape::ShapeCall::Validate || ctx.backend.is_some());

    // Laravel request input: `header('X', '')`, `query()`, `file('photo')`
    // and the rest declare one union spanning every way of calling them,
    // and the arguments say which of those ways this is.  Classified once
    // for the same reason the shape call above is.
    let input_accessor =
        crate::virtual_members::laravel::request_input::input_accessor(&method_name);

    for owner in &owner_classes {
        if let Some(result) =
            try_resolve_config_method_type(&owner.fqn(), &method_name, argument_list, ctx)
        {
            return result;
        }
        if let Some(result) =
            try_resolve_trans_method_type(&owner.fqn(), &method_name, argument_list, ctx)
        {
            return result;
        }
        if let Some(result) =
            try_resolve_command_accessor_type(owner, &method_name, argument_list, ctx)
        {
            return result;
        }
        if let Some(call) = shape_call
            && let Some(shape) = try_resolve_validated_shape(owner, call, object, &arg_refs, ctx)
        {
            return vec![ResolvedType::from_type_string(shape)];
        }
        if let Some(accessor) = input_accessor
            && let Some(result) =
                try_resolve_request_accessor_type(owner, accessor, argument_list, &arg_refs, ctx)
        {
            return result;
        }
    }

    // Laravel factory count state: `create()`/`make()` build a single
    // model, or a collection of them when the chain set a count
    // (`factory(3)`, `count(3)`, `times(3)`).  Laravel declares both
    // outcomes as one `Collection<int, TModel>|TModel` return type, so
    // without this the union survives into every assignment, argument and
    // property write that a factory chain feeds.
    if let Some((classes, hint)) = crate::virtual_members::laravel::resolve_factory_count_return_ast(
        object,
        &method_name,
        &receiver_resolved,
        ctx.content,
        &rctx,
    ) {
        return vec![match classes.first() {
            Some(class) => ResolvedType::from_both_arc(hint, Arc::clone(class)),
            None => ResolvedType::from_type_string(hint),
        }];
    }

    // A fluent factory call (`state()`, `for()`, `hasPosts()`, `count()`)
    // returns the factory itself, so the count state the chain has built
    // up so far travels on to the result — which is what lets a `create()`
    // several statements and one variable later still know what it builds.
    let fluent_count = crate::virtual_members::laravel::fluent_factory_count(
        &receiver_resolved,
        &method_name,
        arg_refs.first().copied(),
        &rctx,
    );

    let receiver_is_this = matches!(
        object,
        Expression::Variable(Variable::Direct(dv)) if dv.name == b"$this"
    );
    let lsb_class = lsb_class_for_call(receiver_is_this, &receiver_resolved, ctx);

    let is_union = owner_classes.len() > 1;
    let mut union_results: Vec<ResolvedType> = Vec::new();

    for (idx, owner) in owner_classes.iter().enumerate() {
        // Build class-level template substitutions from the receiver's
        // generic type string (e.g. `Collection<int, User>` maps
        // `TKey => int, TValue => User`), merge in method-level
        // substitutions bound from the call's arguments, then override
        // with any `@psalm-if-this-is` inference from the receiver's
        // concrete type.
        let receiver_type = receiver_resolved
            .get(idx)
            .or_else(|| receiver_resolved.first())
            .map(|rt| &rt.type_string);
        let template_subs = crate::type_engine::call_resolution::build_call_template_subs(
            owner,
            &method_name,
            &arg_refs,
            receiver_type,
            &rctx,
        );

        // When the return type contains `static`/`self`/`$this` and the
        // receiver was resolved with generic parameters, use the
        // receiver's full type (e.g. `Builder<Article>`) for
        // substitution so the generics are preserved; otherwise fall
        // back to a plain FQN swap.
        let owner_key = owner.fqn();
        let self_replace =
            |ty: &PhpType| match receiver_type_for_owner(&receiver_resolved, &owner_key) {
                Some(rt) => ty.replace_self_with_type(&rt),
                None => ty.replace_self_bound(&owner_key, lsb_class.as_deref()),
            };

        let mut owner_results = resolve_owner_method_call(
            owner,
            &method_name,
            argument_list,
            ctx,
            false,
            &template_subs,
            &self_replace,
        );
        if let Some(count) = fluent_count {
            crate::virtual_members::laravel::carry_factory_count(
                &mut owner_results,
                &receiver_resolved,
                count,
            );
        }
        if !is_union {
            return owner_results;
        }
        ResolvedType::extend_unique(&mut union_results, owner_results);
    }

    // For intersection types, filter out `mixed` when concrete types exist.
    // When a receiver is an intersection like `IChild&IParent<C>`, each member
    // resolves the method independently: the unparameterized interface may
    // return `mixed` while the parameterized one returns `C`.  In an
    // intersection the most specific type wins, so discard `mixed` entries
    // when at least one non-mixed result is present.
    if union_results.len() > 1 {
        let has_non_mixed = union_results.iter().any(|rt| !rt.type_string.is_mixed());
        if has_non_mixed {
            union_results.retain(|rt| !rt.type_string.is_mixed());
        }
    }

    union_results
}

/// Expand union generic receiver types into separate owner entries.
///
/// When a variable has type `C<A>|C<B>`, the resolution pipeline produces
/// a single `ResolvedType` with `type_string = Union(Generic("C",[A]), Generic("C",[B]))`
/// and one `class_info` (the base class `C`).  Calling a method on such
/// a union should resolve each branch independently: `->get()` on
/// `C<A>|C<B>` where `get()` returns `T` should yield `A|B`.
///
/// This function detects such union-of-generics patterns and expands them
/// into separate owner classes, each with the appropriate template
/// substitutions applied.
pub(super) fn expand_union_generic_owners(
    owner_classes: Vec<Arc<ClassInfo>>,
    receiver_resolved: Vec<ResolvedType>,
    ctx: &VarResolutionCtx<'_>,
) -> (Vec<Arc<ClassInfo>>, Vec<ResolvedType>) {
    // Only expand when we have exactly one owner and the type_string
    // is a union with generic branches referencing the same base class.
    if owner_classes.len() != 1 || receiver_resolved.len() != 1 {
        return (owner_classes, receiver_resolved);
    }
    let rt = &receiver_resolved[0];
    let union_members = match &rt.type_string.kind() {
        TypeKind::Union(members) => members,
        _ => return (owner_classes, receiver_resolved),
    };

    // Check that at least two branches are generic types of the same
    // base class, and the class has template parameters.
    let base_cls = &owner_classes[0];
    if base_cls.template_params.is_empty() {
        return (owner_classes, receiver_resolved);
    }

    let base_fqn = base_cls.fqn();
    let base_short = base_cls.name.as_str();
    let is_same_base = |name: &str| -> bool {
        name == base_short
            || name == base_fqn.as_str()
            || crate::util::short_name(name) == base_short
    };
    let generic_branches: Vec<&PhpType> = union_members
        .iter()
        .filter(|m| matches!(m.kind(), TypeKind::Generic(g) if is_same_base(&g.name)))
        .collect();
    if generic_branches.len() < 2 {
        return (owner_classes, receiver_resolved);
    }

    // Expand: for each generic branch, apply the type args to produce
    // a substituted ClassInfo.
    let mut expanded_owners: Vec<Arc<ClassInfo>> = Vec::new();
    let mut expanded_resolved: Vec<ResolvedType> = Vec::new();

    for member in union_members {
        match member.kind() {
            TypeKind::Generic(g) if is_same_base(&g.name) => {
                let arc = crate::virtual_members::resolve_class_fully_with_type_args(
                    base_cls,
                    ctx.class_loader,
                    ctx.resolved_class_cache,
                    &g.args,
                );
                expanded_resolved.push(ResolvedType::from_both_arc(
                    member.clone(),
                    Arc::clone(&arc),
                ));
                expanded_owners.push(arc);
            }
            // Non-generic union members (e.g. scalars in `C<A>|int`)
            // are kept as type-string-only entries in receiver_resolved
            // but don't contribute an owner class.
            other => {
                expanded_resolved.push(ResolvedType::from_type_string(other.clone().into()));
            }
        }
    }

    (expanded_owners, expanded_resolved)
}

/// Find the receiver's type string that matches the given owner class name.
///
/// Scans `receiver_resolved` for a `ResolvedType` whose `class_info`
/// matches `owner_name` (short name or FQN) and whose `type_string` is a
/// `Generic` (i.e. carries generic parameters like `Builder<Article>`).
/// Returns the matching `PhpType` so that `replace_self_with_type` can
/// preserve those generic parameters when the method returns
/// `static`/`self`/`$this`.
///
/// Matching by short name alone is ambiguous for Laravel's dual
/// `Eloquent\Builder` / `Query\Builder` classes; FQN is preferred when
/// available so Query-mixin fluents like `lockForUpdate()` keep the
/// Eloquent receiver's `Builder<TModel>` type.
pub(super) fn receiver_type_for_owner(
    receiver_resolved: &[ResolvedType],
    owner_name: &str,
) -> Option<PhpType> {
    let owner_short = crate::util::short_name(owner_name);
    let mut short_match = None;
    for rt in receiver_resolved {
        let Some(ci) = rt.class_info.as_ref() else {
            continue;
        };
        if !matches!(rt.type_string.kind(), TypeKind::Generic(_)) {
            continue;
        }
        if ci.fqn().as_str() == owner_name || ci.name.as_str() == owner_name {
            return Some(rt.type_string.clone());
        }
        if short_match.is_none() && ci.name.as_str() == owner_short {
            short_match = Some(rt.type_string.clone());
        }
    }
    short_match
}

/// Resolve a method's PHPStan conditional return type against the call-site
/// arguments, returning the winning branch's type when it is definite and
/// informative.
///
/// The returned type has template substitutions applied, `self`/`static`/
/// `$this` replaced (via the `replace_self` closure, which differs between the
/// instance and static call paths), and any conditionals nested inside the
/// winning branch collapsed.  Returns `None` when the method has no
/// conditional return type, the condition cannot be decided from the
/// arguments, or the winning branch is uninformative (a bare `mixed`/`array`
/// else-branch) — in which case the caller falls back to the native return
/// type so the full union (including scalar/`array` members) is preserved.
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_conditional_return_for_call(
    method_ref: Option<&crate::types::MethodInfo>,
    text_args: &str,
    var_resolver: crate::type_engine::conditional_resolution::VarClassStringResolver<'_>,
    calling_class_name: &str,
    declaring_class_name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    template_subs: &HashMap<String, PhpType>,
    arg_type_resolver: crate::type_engine::conditional_resolution::ArgTypeResolver<'_>,
    replace_self: impl Fn(&PhpType) -> PhpType,
) -> Option<PhpType> {
    let method = method_ref?;
    let cond = method.conditional_return.as_ref()?;
    let params = method.parameters.as_slice();
    let class_ctx = crate::type_engine::conditional_resolution::ConditionalClassContext {
        calling: Some(calling_class_name),
        declaring: Some(declaring_class_name),
    };
    let tpl = crate::type_engine::conditional_resolution::TemplateContext {
        defaults: None,
        params: method.template_params.as_slice(),
        bindings: method.template_bindings.as_slice(),
        arg_type_resolver,
    };
    let resolved =
        crate::type_engine::conditional_resolution::resolve_conditional_with_text_args_and_defaults(
            cond,
            params,
            text_args,
            var_resolver,
            class_ctx,
            class_loader,
            &tpl,
        )?;
    let substituted = if template_subs.is_empty() {
        resolved
    } else {
        resolved.substitute(template_subs)
    };
    let substituted = if substituted.contains_self_ref() {
        replace_self(&substituted)
    } else {
        substituted
    };
    // Collapse any conditionals nested inside the winning branch.
    let collapsed = if substituted.contains_conditional() {
        let tpl2 = crate::type_engine::conditional_resolution::TemplateContext {
            defaults: Some(template_subs),
            params: method.template_params.as_slice(),
            bindings: method.template_bindings.as_slice(),
            arg_type_resolver,
        };
        crate::type_engine::conditional_resolution::evaluate_nested_conditionals_text(
            &substituted,
            params,
            text_args,
            var_resolver,
            class_ctx,
            class_loader,
            &tpl2,
        )
    } else {
        substituted
    };
    if collapsed.is_uninformative_return() {
        None
    } else {
        Some(collapsed)
    }
}

/// Resolve an authoritative return type (e.g. a call-site-narrowed
/// conditional branch) to `ResolvedType` values.
///
/// Prefers class-backed results when the type names concrete classes, keeping
/// the full type string as the hint (so generics like `Collection<int, User>`
/// survive).  When the type names no class (a bare `array<…>`, `list<…>`,
/// scalar, or shape) a type-string-only entry is returned so consumers that
/// read `.type_string` still see it.  `void` collapses to `null`.
pub(super) fn resolve_from_authoritative_type(
    ty: PhpType,
    current_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Vec<ResolvedType> {
    let classes = crate::type_engine::type_resolution::type_hint_to_classes_typed(
        &ty,
        current_class_name,
        all_classes,
        class_loader,
    );
    if !classes.is_empty() {
        return ResolvedType::from_classes_with_hint(classes, ty);
    }
    if ty == PhpType::void() {
        return vec![ResolvedType::from_type_string(PhpType::null())];
    }
    vec![resolved_type_with_lookup(
        ty,
        current_class_name,
        all_classes,
        class_loader,
    )]
}

/// Whether `method_name` on `class_info` is declared `static`, looking through
/// the inheritance merge so an inherited or `@method static` member counts.
///
/// A method the merge cannot find is treated as static: an unknown target
/// reached through `ClassName::` is a static call as far as anything we can
/// still say about it goes.
fn method_is_static(class_info: &ClassInfo, method_name: &str, ctx: &VarResolutionCtx<'_>) -> bool {
    if let Some(method) = class_info.get_method_ci(method_name) {
        return method.is_static;
    }
    let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
        class_info,
        ctx.class_loader,
        ctx.resolved_class_cache,
    );
    merged
        .get_method_ci(method_name)
        .is_none_or(|method| method.is_static)
}

/// The class a `@return static` / `@return $this` annotation binds to for this
/// call, or `None` when the call fixes the class and the keyword collapses.
///
/// PHP only carries late static binding across a *forwarding* call: `$this->`,
/// `self::`, `static::`, and `parent::`.  Writing the class out — `A::create()`,
/// `(new A)->create()`, a variable declared `A` — pins it, so `static` there is
/// exactly `A` however `A` is subclassed.  PHPStan and Psalm both collapse
/// those, and keeping a bounded `static(A)` would claim an openness the call
/// does not have.
///
/// A receiver that is *itself* still bounded keeps the chain open, so
/// `$this->self()->self()` stays bound to the class the chain started from
/// rather than collapsing at the second hop.
fn lsb_class_for_call(
    receiver_is_this: bool,
    receiver_resolved: &[ResolvedType],
    ctx: &VarResolutionCtx<'_>,
) -> Option<crate::atom::Atom> {
    if receiver_is_this {
        return Some(ctx.current_class.fqn());
    }
    receiver_resolved
        .iter()
        .find_map(|rt| match rt.type_string.kind() {
            TypeKind::StaticType(bound) | TypeKind::ThisType(bound) => Some(*bound),
            _ => None,
        })
}

/// Resolve a method call's return type against a single, fully determined
/// owner class: template substitution, `@psalm-if-this-is` narrowing (via
/// the caller-supplied `template_subs`), PHPStan conditional return types,
/// and body-return-type inference, in that order.
///
/// Shared by instance method calls (called once per union-receiver branch)
/// and static method calls (a single owner, no receiver-derived generics).
/// `self_replace` maps `static`/`self`/`$this` in a resolved return type to
/// the owner's concrete type: generic-aware (via [`receiver_type_for_owner`])
/// for instance calls, a plain FQN swap for static calls, which have no
/// receiver expression to carry generics.
pub(super) fn resolve_owner_method_call(
    owner: &ClassInfo,
    method_name: &str,
    argument_list: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
    is_static: bool,
    template_subs: &HashMap<String, PhpType>,
    self_replace: &dyn Fn(&PhpType) -> PhpType,
) -> Vec<ResolvedType> {
    let current_class_name: &str = &ctx.current_class.name;
    let arg_texts = crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
        argument_list,
        ctx.content,
    );
    let text_args = arg_texts.join(", ");
    let rctx = ctx.as_resolution_ctx();
    let var_resolver = build_var_resolver_from_ctx(ctx);
    let mr_ctx = MethodReturnCtx {
        all_classes: ctx.all_classes,
        class_loader: ctx.class_loader,
        backend: ctx.backend,
        template_subs,
        var_resolver: Some(&var_resolver),
        cache: ctx.resolved_class_cache,
        calling_class_name: Some(&ctx.current_class.name),
        is_static,
    };

    // Try the owner directly first — it may already be fully resolved with
    // generic substitutions applied.  The cache is keyed by bare FQN and
    // returns the un-substituted base class, so prefer the owner's own
    // method to preserve template substitutions.
    let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
        owner,
        ctx.class_loader,
        ctx.resolved_class_cache,
    );
    if let Some((date_class, date_return_type)) =
        Backend::configured_laravel_date_return(&merged, method_name, ctx.class_loader)
    {
        return ResolvedType::from_classes_with_hint(vec![date_class], date_return_type);
    }

    let owner_method = owner.get_method_ci(method_name);
    let merged_method = merged.get_method_ci(method_name);
    // Prefer the merged method's return type when the owner's method has no
    // docblock override (return_type == native_return_type).  The merged
    // method carries inherited types from interfaces/parents with template
    // substitutions already applied (e.g. `V|null` → `User|null` from
    // `@implements Collection<string, User>`).
    let method_ref = match (owner_method, merged_method) {
        (Some(om), Some(mm))
            if om.return_type == om.native_return_type
                && mm.return_type != mm.native_return_type =>
        {
            Some(mm)
        }
        (Some(om), _) => Some(om),
        (None, Some(mm)) => Some(mm),
        // Method not found — fall back to the magic method's return type.
        (None, None) => merged.get_method_ci(if is_static { "__callStatic" } else { "__call" }),
    };
    // Recover the effective return type string from the method and replace
    // `static`/`self`/`$this` with the owner's concrete type so that e.g.
    // `static[]` becomes `Country[]`.
    let native_ret_type_string = method_ref.and_then(|m| m.return_type.as_ref()).map(|ret| {
        let substituted = if !template_subs.is_empty() {
            ret.substitute(template_subs).simplified()
        } else {
            ret.clone()
        };
        // Resolve `parent` to the concrete parent class name before any
        // self/static replacement so that downstream consumers see a real
        // FQN instead of the keyword.
        let substituted = if substituted.is_parent_ref() {
            owner
                .parent_class
                .as_ref()
                .map(|p| PhpType::named(atom(p.as_ref())))
                .unwrap_or(substituted)
        } else {
            substituted
        };
        let resolved = if substituted.contains_self_ref() {
            self_replace(&substituted)
        } else {
            substituted
        };
        // Eloquent's own `get()`/`all()`/relation accessors are annotated
        // with the base `Collection<int, TModel>`; once TModel is concrete
        // we know which collection subclass the model really builds.
        crate::virtual_members::laravel::replace_eloquent_collections_in_type(
            &resolved,
            ctx.class_loader,
        )
        .unwrap_or(resolved)
    });

    // Resolver from an argument's source text to its type, used to evaluate
    // `is <Type>` conditions whose argument is an expression (a method-call
    // chain, property access, …) rather than a literal.
    let arg_ty_resolver = |t: &str| Backend::resolve_arg_text_to_type(t, &rctx);

    // Resolve the PHPStan conditional return type against the call-site
    // arguments, if the method declares one.  When it yields an informative
    // type it is *authoritative*: the branch it selects (e.g.
    // `list<\stdClass>` from `PDOStatement::fetchAll`, or `array<TKey,
    // static>` for a literal-array argument) supersedes the method's broad
    // native union return type.  Resolving classes from the native union
    // instead would both ignore the call-site narrowing and silently drop
    // scalar or `array` members the union carries.
    let conditional_ret = resolve_conditional_return_for_call(
        method_ref,
        &text_args,
        Some(&var_resolver),
        current_class_name,
        owner.fqn().as_str(),
        ctx.class_loader,
        template_subs,
        Some(&arg_ty_resolver),
        self_replace,
    );

    // Collapse any conditionals nested inside the (template-substituted)
    // native return type against the call arguments, so a generic wrapper
    // like `Collection<($groupBy is array|string ? array-key : …), …>`
    // yields a concrete key type instead of carrying a raw conditional that
    // later gets compared against — and printed in — an argument-type
    // diagnostic.
    let native_ret_type_string = native_ret_type_string.map(|ty| {
        if ty.contains_conditional() {
            let params = method_ref.map(|m| m.parameters.as_slice()).unwrap_or(&[]);
            let tpl = crate::type_engine::conditional_resolution::TemplateContext {
                defaults: Some(template_subs),
                params: method_ref
                    .map(|m| m.template_params.as_slice())
                    .unwrap_or(&[]),
                bindings: method_ref
                    .map(|m| m.template_bindings.as_slice())
                    .unwrap_or(&[]),
                arg_type_resolver: Some(&arg_ty_resolver),
            };
            crate::type_engine::conditional_resolution::evaluate_nested_conditionals_text(
                &ty,
                params,
                &text_args,
                Some(&var_resolver),
                crate::type_engine::conditional_resolution::ConditionalClassContext {
                    calling: Some(current_class_name),
                    declaring: Some(owner.fqn().as_str()),
                },
                ctx.class_loader,
                &tpl,
            )
        } else {
            ty
        }
    });

    // When the conditional resolved to a definite, informative type, it
    // wins — resolve the result classes from it directly.
    if let Some(cond_ty) = conditional_ret {
        return resolve_from_authoritative_type(
            cond_ty,
            current_class_name,
            ctx.all_classes,
            ctx.class_loader,
        );
    }

    let ret_type_string = native_ret_type_string;

    let results =
        Backend::resolve_method_return_types_with_args(owner, method_name, &text_args, &mr_ctx);
    if !results.is_empty() {
        return match ret_type_string {
            Some(hint) => ResolvedType::from_classes_with_hint(results, hint),
            None => ResolvedType::from_classes(results),
        };
    }

    // The method has a return type string but `type_hint_to_classes_typed`
    // found no matching class (e.g. `list<Widget>`, `int`, `array{name:
    // string}`).  Return a type-string-only entry so that consumers reading
    // `.type_string` (hover, foreach resolution, null-coalesce stripping)
    // still get the information.
    //
    // Return the type string even for non-informative types like `array` or
    // `mixed` — a correct-but-vague type is better than keeping the
    // previous (wrong) type after reassignment.  Skip only `void` (void
    // methods don't produce a value).  Also expand type aliases before
    // returning so that `@phpstan-type UserList array<int, User>` with
    // `@return UserList` is expanded to its concrete type.
    if let Some(hint) = ret_type_string {
        let expanded = crate::type_engine::type_resolution::resolve_type_alias_typed(
            &hint,
            &owner.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        let parsed_effective = expanded.unwrap_or(hint);
        if parsed_effective == PhpType::void() {
            return vec![ResolvedType::from_type_string(PhpType::null())];
        }
        return vec![resolved_type_with_lookup(
            parsed_effective,
            current_class_name,
            ctx.all_classes,
            ctx.class_loader,
        )];
    }

    // Body return type inference fallback: when the method has no declared
    // return type and no @return docblock, try to infer the return type
    // from the method body.  This handles non-class types (list<Foo>, int,
    // array shapes) that resolve_method_return_types_with_args cannot
    // represent.
    if method_ref.is_some_and(|m| m.return_type.is_none() && m.name_offset != 0 && !m.is_virtual)
        && let Some(backend) = ctx.backend
        && let Some(inferred) = crate::type_engine::call_resolution::try_infer_body_return_type(
            backend,
            &owner.fqn(),
            method_ref.unwrap(),
        )
        && !inferred.is_void()
        && !inferred.is_mixed()
    {
        return vec![resolved_type_with_lookup(
            inferred,
            current_class_name,
            ctx.all_classes,
            ctx.class_loader,
        )];
    }

    vec![]
}

/// Resolve a static method call: `ClassName::method()`, `self::method()`,
/// `static::method()`.
pub(super) fn resolve_rhs_static_call(
    static_call: &StaticMethodCall<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ResolvedType> {
    let current_class_name: &str = &ctx.current_class.name;

    // `self::`, `static::`, and `parent::` all forward late static binding, so
    // `@return static` on the target stays bound to the class the call is made
    // *from* — including `parent::`, which reads the annotation off the parent
    // but still resolves `static` to the current class.
    let forwards_lsb = matches!(
        static_call.class,
        Expression::Self_(_) | Expression::Static(_) | Expression::Parent(_)
    );

    let class_name = match static_call.class {
        Expression::Self_(_) => Some(current_class_name.to_string()),
        Expression::Static(_) => Some(current_class_name.to_string()),
        Expression::Parent(_) => ctx.current_class.parent_class.map(|a| a.to_string()),
        Expression::Identifier(ident) => Some(bytes_to_str(ident.value()).to_string()),
        // ── `$var::method()` where `$var` holds a class-string ──
        Expression::Variable(Variable::Direct(dv)) => {
            let var_name = bytes_to_str(dv.name).to_string();
            let targets =
                crate::type_engine::variable::class_string_resolution::resolve_class_string_targets(
                    &var_name,
                    ctx.current_class,
                    ctx.all_classes,
                    ctx.content,
                    ctx.cursor_offset,
                    ctx.class_loader,
                    ctx.backend,
                );
            // When there are multiple possible class targets (union class-string),
            // resolve the method return type through each and union the results.
            if targets.len() > 1 {
                if let ClassLikeMemberSelector::Identifier(ident) = &static_call.method {
                    let method_name_str = bytes_to_str(ident.value).to_string();
                    let mut union_types: Vec<PhpType> = Vec::new();
                    let mut union_classes: Vec<ResolvedType> = Vec::new();
                    for target in &targets {
                        let arg_texts = crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                            &static_call.argument_list,
                            ctx.content,
                        );
                        let arg_refs: Vec<&str> = arg_texts.iter().map(|s| s.as_str()).collect();
                        let text_args = arg_texts.join(", ");
                        let rctx = ctx.as_resolution_ctx();
                        let template_subs = Backend::build_method_template_subs(
                            target,
                            &method_name_str,
                            &arg_refs,
                            &rctx,
                        );
                        let var_resolver = build_var_resolver_from_ctx(ctx);
                        let mr_ctx = MethodReturnCtx {
                            all_classes: ctx.all_classes,
                            class_loader: ctx.class_loader,
                            backend: ctx.backend,
                            template_subs: &template_subs,
                            var_resolver: Some(&var_resolver),
                            cache: ctx.resolved_class_cache,
                            calling_class_name: Some(&ctx.current_class.name),
                            is_static: true,
                        };
                        // Get the method's return type string.
                        let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                            target,
                            ctx.class_loader,
                            ctx.resolved_class_cache,
                        );
                        let method_ref = target
                            .get_method_ci(&method_name_str)
                            .or_else(|| merged.get_method_ci(&method_name_str));
                        if let Some(m) = method_ref {
                            if let Some(ref ret) = m.return_type {
                                let substituted = if !template_subs.is_empty() {
                                    ret.substitute(&template_subs)
                                } else {
                                    ret.clone()
                                };
                                // Each target is a concrete class-string, so
                                // late static binding has nothing left to
                                // resolve on this branch.
                                let resolved = substituted.replace_self_bound(&target.fqn(), None);
                                union_types.push(resolved);
                            }
                        } else {
                            let results = Backend::resolve_method_return_types_with_args(
                                target,
                                &method_name_str,
                                &text_args,
                                &mr_ctx,
                            );
                            for r in results {
                                union_classes.push(ResolvedType::from_both_arc(
                                    PhpType::named(atom(r.name.as_ref())),
                                    r,
                                ));
                            }
                        }
                    }
                    if !union_types.is_empty() || !union_classes.is_empty() {
                        // Build a unified type from all resolved return types.
                        let combined = if union_types.len() == 1 && union_classes.is_empty() {
                            union_types.remove(0)
                        } else if union_types.is_empty() && !union_classes.is_empty() {
                            return union_classes;
                        } else {
                            PhpType::union(union_types)
                        };
                        let resolved_classes =
                            crate::type_engine::type_resolution::type_hint_to_classes_typed(
                                &combined,
                                current_class_name,
                                ctx.all_classes,
                                ctx.class_loader,
                            );
                        if !resolved_classes.is_empty() {
                            return ResolvedType::from_classes_with_hint(
                                resolved_classes,
                                combined,
                            );
                        }
                        return vec![ResolvedType::from_type_string(combined)];
                    }
                }
                // None of the targets yielded a return type.
                return vec![];
            }
            if let Some(first) = targets.first() {
                Some(first.name.to_string())
            } else {
                // Fallback: resolve the variable's type and extract the
                // inner type from `class-string<T>`.  This handles
                // parameters typed as `@param class-string<Foo> $var`
                // where there is no `$var = Foo::class` assignment.
                let resolved = resolve_var_types(&var_name, ctx, ctx.cursor_offset);
                resolved
                    .iter()
                    .find_map(|rt| match &rt.type_string.kind() {
                        TypeKind::ClassString(Some(inner)) => {
                            inner.base_name().map(|s| s.to_string())
                        }
                        TypeKind::Nullable(inner) => match inner.kind() {
                            TypeKind::ClassString(Some(cs_inner)) => {
                                cs_inner.base_name().map(|s| s.to_string())
                            }
                            _ => None,
                        },
                        TypeKind::Union(members) => members.iter().find_map(|m| match m.kind() {
                            TypeKind::ClassString(Some(inner)) => {
                                inner.base_name().map(|s| s.to_string())
                            }
                            TypeKind::Nullable(inner) => match inner.kind() {
                                TypeKind::ClassString(Some(cs_inner)) => {
                                    cs_inner.base_name().map(|s| s.to_string())
                                }
                                _ => None,
                            },
                            _ => None,
                        }),
                        _ => None,
                    })
                    .or_else(|| {
                        // Final fallback: `$var::method()` where `$var` is an
                        // object instance (not a class-string). In PHP you can
                        // call static methods on an instance reference.
                        resolved
                            .iter()
                            .find_map(|rt| rt.type_string.base_name().map(|s| s.to_string()))
                    })
            }
        }
        _ => None,
    };
    if let Some(cls_name) = class_name
        && let ClassLikeMemberSelector::Identifier(ident) = &static_call.method
    {
        let method_name = bytes_to_str(ident.value).to_string();
        let owner = (ctx.class_loader)(&cls_name)
            .map(Arc::unwrap_or_clone)
            .or_else(|| {
                ctx.all_classes
                    .iter()
                    .find(|c| c.name == cls_name)
                    .map(|c| ClassInfo::clone(c))
            });
        if let Some(ref owner) = owner {
            let concrete_owner = crate::type_engine::call_resolution::facade_concrete_owner(
                owner,
                &method_name,
                ctx.class_loader,
                ctx.resolved_class_cache,
                ctx.backend,
            );
            let owner = concrete_owner.as_ref().unwrap_or(owner);

            if let Some(result) = try_resolve_config_method_type(
                &owner.fqn(),
                &method_name,
                &static_call.argument_list,
                ctx,
            ) {
                return result;
            }

            if let Some(result) = try_resolve_trans_method_type(
                &owner.fqn(),
                &method_name,
                &static_call.argument_list,
                ctx,
            ) {
                return result;
            }

            let arg_texts =
                crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
                    &static_call.argument_list,
                    ctx.content,
                );
            let arg_refs: Vec<&str> = arg_texts.iter().map(|s| s.as_str()).collect();
            let rctx = ctx.as_resolution_ctx();
            let template_subs =
                Backend::build_method_template_subs(owner, &method_name, &arg_refs, &rctx);
            let owner_key = owner.fqn();
            // An explicit `A::` on a non-static method is PHP's pre-8
            // instance-forwarding form, which keeps `$this` (and with it late
            // static binding) bound, so only a `static` method written out
            // fixes the class.
            let target_is_static = method_is_static(owner, &method_name, ctx);
            let lsb_class = (forwards_lsb || !target_is_static).then(|| ctx.current_class.fqn());
            let self_replace =
                |ty: &PhpType| ty.replace_self_bound(&owner_key, lsb_class.as_deref());

            let mut results = resolve_owner_method_call(
                owner,
                &method_name,
                &static_call.argument_list,
                ctx,
                true,
                &template_subs,
                &self_replace,
            );
            // `Model::factory(…)`, `UserFactory::times(3)` and
            // `UserFactory::new()` open a factory chain, and what they
            // were opened with is what the `create()` at the far end of
            // it builds.  `factory($count)` only settles that once its
            // argument is resolved, which is why the type is fetched
            // lazily rather than for every static call in the file.
            let first_arg_type = || {
                let arg = static_call.argument_list.arguments.first()?;
                let resolved = resolve_rhs_expression(arg.value(), ctx);
                (!resolved.is_empty()).then(|| ResolvedType::types_joined(&resolved))
            };
            crate::virtual_members::laravel::tag_static_factory_call(
                &mut results,
                &method_name,
                arg_refs.first().copied(),
                &first_arg_type,
                &rctx,
            );
            return results;
        }
    }
    vec![]
}

/// The array shape a Laravel `validated()` / `validate()` /
/// `safe()->only()` call assigns, given the validation rules in scope.
///
/// `object` is the expression the method was called on, which is what tells
/// a `ValidatedInput` receiver apart from the request it came from.
fn try_resolve_validated_shape(
    owner: &ClassInfo,
    call: validated_shape::ShapeCall,
    object: &Expression<'_>,
    arg_refs: &[&str],
    ctx: &VarResolutionCtx<'_>,
) -> Option<PhpType> {
    validated_shape::resolve_shape_at_call(
        owner,
        call,
        arg_refs,
        &|| safe_source_owner(object, ctx),
        ctx.content,
        object.span().end.offset,
        ctx.as_resolution_ctx().class_loader,
        ctx.backend,
    )
}

/// The request class behind a `ValidatedInput` receiver.
///
/// Covers both the direct chain (`$request->safe()->only(…)`) and the
/// two-step form (`$safe = $request->safe(); $safe->only(…)`), which
/// `resolve_var_types`'s assignment tracing already resolves back to the
/// request variable.
fn safe_source_owner(
    object: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<Arc<ClassInfo>> {
    let variable = match object {
        // `$request->safe()->only(…)` — the receiver is the `safe()` call.
        Expression::Call(_) => {
            crate::virtual_members::laravel::safe_call_receiver_variable(object)?
        }
        // `$safe->only(…)` — trace the assignment that produced `$safe`.
        Expression::Variable(Variable::Direct(dv)) => {
            crate::virtual_members::laravel::safe_source_variable(
                ctx.content,
                object.span().end.offset as usize,
                bytes_to_str(dv.name),
            )?
        }
        _ => return None,
    };
    let resolved = resolve_var_types(&variable, ctx, object.span().end.offset);
    ResolvedType::into_arced_classes(resolved)
        .into_iter()
        .next()
}

fn try_resolve_config_method_type(
    owner_fqn: &str,
    method_name: &str,
    argument_list: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    const CONFIG_FQNS: &[&str] = &[
        "Illuminate\\Config\\Repository",
        "Illuminate\\Support\\Facades\\Config",
        "Config",
    ];
    if !matches!(method_name, "get" | "array") {
        return None;
    }
    let normalized = owner_fqn.strip_prefix('\\').unwrap_or(owner_fqn);
    if !CONFIG_FQNS
        .iter()
        .any(|fqn| normalized.eq_ignore_ascii_case(fqn))
    {
        return None;
    }
    let resolver = ctx.loaders.config_resolver?;
    let arg_texts = crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
        argument_list,
        ctx.content,
    );
    let first_arg = arg_texts.first()?;
    let key = crate::util::unescape_php_string_literal(first_arg.trim())?;
    if key.is_empty() || key.contains('$') {
        return None;
    }
    let ty = resolver(&key)?;
    if method_name == "array" {
        if ty.is_array_like() {
            return Some(vec![ResolvedType::from_type_string(ty)]);
        }
        return None;
    }
    Some(vec![ResolvedType::from_type_string(ty)])
}

/// Narrow `Illuminate\Translation\Translator::get()` (the `Lang::get()`
/// facade method) to `string` when its literal key argument names a
/// scalar translation entry.  Mirrors [`try_resolve_config_method_type`].
fn try_resolve_trans_method_type(
    owner_fqn: &str,
    method_name: &str,
    argument_list: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    const TRANS_FQNS: &[&str] = &[
        "Illuminate\\Translation\\Translator",
        "Illuminate\\Support\\Facades\\Lang",
        "Lang",
    ];
    if method_name != "get" {
        return None;
    }
    let normalized = owner_fqn.strip_prefix('\\').unwrap_or(owner_fqn);
    if !TRANS_FQNS
        .iter()
        .any(|fqn| normalized.eq_ignore_ascii_case(fqn))
    {
        return None;
    }
    let resolver = ctx.loaders.trans_resolver?;
    let arg_texts = crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
        argument_list,
        ctx.content,
    );
    let first_arg = arg_texts.first()?;
    let ty = crate::util::unescape_php_string_literal(first_arg.trim())
        .filter(|key| !key.is_empty() && !key.contains('$'))
        .and_then(|key| resolver(&key))
        .unwrap_or_else(crate::virtual_members::laravel::unresolved_trans_type);
    Some(vec![ResolvedType::from_type_string(ty)])
}

/// Resolve a request input accessor (`header()`, `query()`, `input()`,
/// `cookie()`, `post()`, `file()`) to what the call's own arguments say it
/// returns, rather than the union that covers every way of calling it.
fn try_resolve_request_accessor_type(
    owner: &ClassInfo,
    accessor: crate::virtual_members::laravel::request_input::InputAccessor,
    argument_list: &ArgumentList<'_>,
    arg_refs: &[&str],
    ctx: &VarResolutionCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    use crate::virtual_members::laravel::request_input;

    // The default only decides the missing-key branch, so it is resolved
    // only once a keyed call has been established.
    let default_type = || {
        let argument = argument_list.arguments.get(1)?;
        let resolved = resolve_rhs_expression(argument.value(), ctx);
        (!resolved.is_empty()).then(|| ResolvedType::types_joined(&resolved))
    };
    let ty = request_input::resolve_accessor_type(
        owner,
        accessor,
        &request_input::AccessorArgs {
            key: arg_refs.first().copied(),
            default_type: &default_type,
        },
        ctx.content,
        ctx.cursor_offset,
        ctx.class_loader,
        ctx.backend,
    )?;
    // The whole union travels on one entry, carrying the class of its
    // object half: splitting it into an entry per member would leave the
    // array half as a class-less entry that a later `instanceof` guard has
    // no way to rule out.
    let classes = crate::type_engine::type_resolution::type_hint_to_classes_typed(
        &ty,
        &owner.fqn(),
        ctx.all_classes,
        ctx.class_loader,
    );
    Some(vec![match classes.first() {
        Some(class) => ResolvedType::from_both_arc(ty, Arc::clone(class)),
        None => ResolvedType::from_type_string(ty),
    }])
}

/// Narrow `$this->argument('user')` / `$this->option('queue')` on an Artisan
/// command to the type its own `$signature` declares for that parameter.
///
/// The framework's declared union spans every parameter shape at once, so
/// without this a value-less `{--flag}` reads as `array|string|int|bool|null`
/// wherever it is used.
fn try_resolve_command_accessor_type(
    class: &ClassInfo,
    method_name: &str,
    argument_list: &ArgumentList<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<Vec<ResolvedType>> {
    if !crate::virtual_members::laravel::is_command_accessor(method_name) {
        return None;
    }
    let arg_texts = crate::type_engine::variable::raw_type_inference::extract_arg_texts_from_ast(
        argument_list,
        ctx.content,
    );
    let key = match arg_texts.first() {
        // A key built at runtime names no parameter we can look up.
        Some(text) => Some(crate::util::unescape_php_string_literal(text.trim())?),
        None => None,
    };
    let ty = crate::virtual_members::laravel::resolve_command_accessor_type(
        class,
        method_name,
        key.as_deref(),
        ctx.class_loader,
        ctx.backend,
    )?;
    Some(vec![ResolvedType::from_type_string(ty)])
}
