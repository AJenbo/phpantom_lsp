/// Variable type resolution for completion subjects.
///
/// This module resolves the type of a `$variable` by re-parsing the file
/// and walking the method / function body that contains the cursor.  It
/// examines:
///
///   - Assignments: `$var = new ClassName(…)`, `$var = $obj->method()`, etc.
///   - Method/function parameter type hints
///   - Inline `/** @var Type */` docblock overrides
///   - Conditional branches (if/else, try/catch, loops) — collects all
///     possible types when the variable is assigned differently in each
///     branch.
///   - Match expressions: `$var = match(…) { … => new A(), … => new B() }`
///     collects all possible types from all arms.
///   - Ternary expressions: `$var = $cond ? new A() : new B()` collects
///     types from both branches.  Short ternary `$a ?: new B()` and
///     null-coalescing `$a ?? new B()` are also supported.
///   - Foreach value variables: when iterating over a variable annotated
///     with a generic iterable type (e.g. `@var list<User>`, `@param
///     list<User>`, `User[]`), the foreach value variable is resolved to
///     the element type.
///   - Foreach key variables: when iterating over a two-parameter generic
///     (e.g. `SplObjectStorage<Request, Response>`), the key variable is
///     resolved to the first type parameter.
///   - Array destructuring: `[$a, $b] = getUsers()` and `list($a, $b) = $var`
///     infer element types from the RHS's generic iterable annotation
///     (function return types, variable/property annotations, inline @var).
///
/// Type narrowing (instanceof, assert, custom type guards) is delegated
/// to the [`crate::completion::type_narrowing`] module.  Closure/arrow-function scope
/// handling is delegated to [`super::closure_resolution`].
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::completion::types::narrowing;
use crate::docblock;
use crate::parser::{extract_hint_string, with_parsed_program};
use crate::types::ClassInfo;

use crate::completion::resolver::{FunctionLoaderFn, VarResolutionCtx};

/// Build a [`VarClassStringResolver`] closure from a [`VarResolutionCtx`].
///
/// The returned closure resolves a variable name (e.g. `"$requestType"`)
/// to the class names it holds as class-string values by delegating to
/// [`resolve_class_string_targets`](super::class_string_resolution::resolve_class_string_targets).
pub(in crate::completion) fn build_var_resolver_from_ctx<'a>(
    ctx: &'a VarResolutionCtx<'a>,
) -> impl Fn(&str) -> Vec<String> + 'a {
    move |var_name: &str| -> Vec<String> {
        super::class_string_resolution::resolve_class_string_targets(
            var_name,
            ctx.current_class,
            ctx.all_classes,
            ctx.content,
            ctx.cursor_offset,
            ctx.class_loader,
        )
        .iter()
        .map(|c| c.name.clone())
        .collect()
    }
}

/// Check whether a type hint should be enriched with generic args for
/// Eloquent scope method Builder parameters.
///
/// When `type_str` resolves to `Builder` (the Eloquent Builder, without
/// generic parameters) and the enclosing method is a scope on a class
/// that extends Eloquent Model, returns
/// `Some("Builder<EnclosingModelName>")`.  Otherwise returns `None`,
/// meaning the caller should use the original type string.
///
/// A method is considered a scope when it uses the `scopeX` naming
/// convention (name starts with `scope`, len > 5) **or** when
/// `has_scope_attr` is `true` (the method has `#[Scope]`).
fn enrich_builder_type_in_scope(
    type_str: &str,
    method_name: &str,
    has_scope_attr: bool,
    current_class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    use crate::virtual_members::laravel::{ELOQUENT_BUILDER_FQN, extends_eloquent_model};

    // Only applies inside scope methods: either the scopeX naming
    // convention or the #[Scope] attribute.
    let is_convention_scope = method_name.starts_with("scope") && method_name.len() > 5;
    if !is_convention_scope && !has_scope_attr {
        return None;
    }

    // Only applies when the enclosing class extends Eloquent Model.
    if !extends_eloquent_model(current_class, class_loader) {
        return None;
    }

    // Check if the type is the Eloquent Builder (without generic args).
    // Accept both the FQN and the short name `Builder` (common in use
    // imports).  If the type already has generic args (e.g.
    // `Builder<User>`), do not enrich — the user-supplied generics
    // should be used as-is.
    if type_str.contains('<') {
        return None;
    }
    let is_eloquent_builder = type_str == ELOQUENT_BUILDER_FQN || type_str == "Builder";
    if !is_eloquent_builder {
        return None;
    }

    // Build the enriched type with the enclosing model as the generic arg.
    Some(format!("{type_str}<{}>", current_class.name))
}

/// Resolve the type of `$variable` by re-parsing the file and walking
/// the method body that contains `cursor_offset`.
///
/// Looks at:
///   1. Assignments: `$var = new ClassName(…)` / `new self` / `new static`
///   2. Assignments from function calls: `$var = app()` → look up return type
///   3. Method parameter type hints
///
/// Returns all possible types when the variable is assigned different
/// types in conditional branches.
pub(crate) fn resolve_variable_types(
    var_name: &str,
    current_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    content: &str,
    cursor_offset: u32,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoaderFn<'_>,
) -> Vec<ClassInfo> {
    with_parsed_program(content, "resolve_variable_types", |program, _content| {
        let ctx = VarResolutionCtx {
            var_name,
            current_class,
            all_classes,
            content,
            cursor_offset,
            class_loader,
            function_loader,
            resolved_class_cache: None,
            enclosing_return_type: None,
        };

        // Walk top-level (and namespace-nested) statements to find the
        // class + method containing the cursor.
        resolve_variable_in_statements(program.statements.iter(), &ctx)
    })
}

/// Walk a sequence of top-level statements to find the class or
/// function body that contains the cursor, then resolve the target
/// variable's type within that scope.
pub(in crate::completion) fn resolve_variable_in_statements<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    // Collect so we can iterate twice: once to check class bodies,
    // once (if needed) to walk top-level statements.
    let stmts: Vec<&Statement> = statements.collect();

    for &stmt in &stmts {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if ctx.cursor_offset < start || ctx.cursor_offset > end {
                    continue;
                }
                // The cursor is inside this class body.  PHP method
                // scopes are isolated — they cannot access variables
                // from enclosing or top-level code.  Return whatever
                // the member scan found (even if empty, e.g. after
                // `unset($var)`), and never fall through to the
                // top-level walk.
                return resolve_variable_in_members(class.members.iter(), ctx);
            }
            Statement::Interface(iface) => {
                let start = iface.left_brace.start.offset;
                let end = iface.right_brace.end.offset;
                if ctx.cursor_offset < start || ctx.cursor_offset > end {
                    continue;
                }
                return resolve_variable_in_members(iface.members.iter(), ctx);
            }
            Statement::Enum(enum_def) => {
                let start = enum_def.left_brace.start.offset;
                let end = enum_def.right_brace.end.offset;
                if ctx.cursor_offset < start || ctx.cursor_offset > end {
                    continue;
                }
                return resolve_variable_in_members(enum_def.members.iter(), ctx);
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if ctx.cursor_offset < start || ctx.cursor_offset > end {
                    continue;
                }
                return resolve_variable_in_members(trait_def.members.iter(), ctx);
            }
            Statement::Namespace(ns) => {
                let results = resolve_variable_in_statements(ns.statements().iter(), ctx);
                if !results.is_empty() {
                    return results;
                }
            }
            // ── Top-level function declarations ──
            // If the cursor is inside a `function foo(Type $p) { … }`
            // at the top level, resolve the variable from its params
            // and walk its body.
            Statement::Function(func) => {
                if let Some(results) = try_resolve_in_function(func, ctx) {
                    return results;
                }
            }
            // ── Functions inside if-guards / blocks ──
            // The common PHP pattern `if (! function_exists('foo'))
            // { function foo(Type $p) { … } }` nests the function
            // declaration inside an if body.  Recurse into blocks
            // and if-bodies so the function's parameters and body
            // assignments are still resolved.
            Statement::If(_) | Statement::Block(_) => {
                if let Some(results) = try_resolve_in_nested_function(stmt, ctx) {
                    return results;
                }
            }
            _ => {}
        }
    }

    // The cursor is not inside any class/interface/enum body — it must
    // be in top-level code.  Walk all top-level statements to find
    // variable assignments (e.g. `$user = new User(…);`).
    let mut results: Vec<ClassInfo> = Vec::new();
    walk_statements_for_assignments(stmts.into_iter(), ctx, &mut results, false);
    results
}

/// Try to resolve the target variable inside a `Function` declaration.
///
/// Returns `Some(results)` when the cursor falls inside the function body
/// (the function introduces an isolated scope, so we always return even
/// when the result vec is empty).  Returns `None` when the cursor is
/// outside this function.
fn try_resolve_in_function(
    func: &Function<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<Vec<ClassInfo>> {
    let body_start = func.body.left_brace.start.offset;
    let body_end = func.body.right_brace.end.offset;
    if ctx.cursor_offset < body_start || ctx.cursor_offset > body_end {
        return None;
    }
    // Extract the enclosing function's @return type for generator
    // yield inference inside the body.  Use body_start + 1 (just
    // past the opening `{`) so the backward brace scan in
    // find_enclosing_return_type immediately finds the function's
    // own `{` and does NOT get confused by intermediate `{`/`}`
    // from nested control-flow.
    let enclosing_ret =
        crate::docblock::find_enclosing_return_type(ctx.content, (body_start + 1) as usize);
    let body_ctx = ctx.with_enclosing_return_type(enclosing_ret);
    // The cursor is inside this function body.  PHP function scopes
    // are isolated, so return the result directly (even if empty
    // after `unset`).
    let mut results: Vec<ClassInfo> = Vec::new();
    super::closure_resolution::resolve_closure_params(
        &func.parameter_list,
        &body_ctx,
        &mut results,
    );
    walk_statements_for_assignments(func.body.statements.iter(), &body_ctx, &mut results, false);
    if !results.is_empty() {
        return Some(results);
    }

    // Generator yield reverse inference for top-level functions.
    if let Some(ref ret_type) = body_ctx.enclosing_return_type {
        let yield_results =
            super::raw_type_inference::try_infer_from_generator_yield(ret_type, &body_ctx);
        if !yield_results.is_empty() {
            return Some(yield_results);
        }
    }

    Some(results)
}

/// Recursively search a statement for a nested `Function` declaration
/// whose body contains the cursor.
///
/// This handles the common PHP pattern where functions are wrapped in
/// `if (! function_exists('name')) { function name(…) { … } }` guards.
/// The function may be nested inside `Block`, `If`, or other compound
/// statements.
fn try_resolve_in_nested_function(
    stmt: &Statement<'_>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<Vec<ClassInfo>> {
    // Quick span check — skip if cursor is outside this statement entirely.
    let span = stmt.span();
    if ctx.cursor_offset < span.start.offset || ctx.cursor_offset > span.end.offset {
        return None;
    }
    match stmt {
        Statement::Function(func) => try_resolve_in_function(func, ctx),
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                if let Some(results) = try_resolve_in_nested_function(inner, ctx) {
                    return Some(results);
                }
            }
            None
        }
        Statement::If(if_stmt) => {
            match &if_stmt.body {
                IfBody::Statement(body) => {
                    if let Some(results) = try_resolve_in_nested_function(body.statement, ctx) {
                        return Some(results);
                    }
                    for else_if in body.else_if_clauses.iter() {
                        if let Some(results) =
                            try_resolve_in_nested_function(else_if.statement, ctx)
                        {
                            return Some(results);
                        }
                    }
                    if let Some(else_clause) = &body.else_clause
                        && let Some(results) =
                            try_resolve_in_nested_function(else_clause.statement, ctx)
                    {
                        return Some(results);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        if let Some(results) = try_resolve_in_nested_function(inner, ctx) {
                            return Some(results);
                        }
                    }
                    for else_if in body.else_if_clauses.iter() {
                        for inner in else_if.statements.iter() {
                            if let Some(results) = try_resolve_in_nested_function(inner, ctx) {
                                return Some(results);
                            }
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        for inner in else_clause.statements.iter() {
                            if let Some(results) = try_resolve_in_nested_function(inner, ctx) {
                                return Some(results);
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Resolve a variable's type by scanning class-like members for parameter
/// type hints and assignment expressions.
///
/// Shared between `Statement::Class` and `Statement::Interface`.
///
/// Returns all possible types when the variable is assigned different
/// types in conditional branches.
fn resolve_variable_in_members<'b>(
    members: impl Iterator<Item = &'b ClassLikeMember<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    for member in members {
        if let ClassLikeMember::Method(method) = member {
            // Collect parameter type hint as initial candidate set.
            // We no longer return early here so that the method body
            // can be scanned for instanceof narrowing / reassignments.
            let mut param_results: Vec<ClassInfo> = Vec::new();
            for param in method.parameter_list.parameters.iter() {
                let pname = param.variable.name.to_string();
                if pname == ctx.var_name {
                    // Try the native AST type hint first.
                    let native_type_str = param.hint.as_ref().map(|h| extract_hint_string(h));

                    // ── Eloquent scope Builder inference ────────
                    // When the enclosing method is a scope on an
                    // Eloquent Model and the parameter type is
                    // `Builder` (without generics), enrich it to
                    // `Builder<EnclosingModel>` so that the
                    // generic-args path injects scope methods.
                    let enriched_type_str = native_type_str.as_deref().and_then(|ts| {
                        let method_name = method.name.value.to_string();
                        // Check whether the method has a #[Scope]
                        // attribute so that the enrichment also
                        // applies to attribute-style scopes.
                        let has_scope_attr = method.attribute_lists.iter().any(|al| {
                            al.attributes
                                .iter()
                                .any(|a| a.name.last_segment() == "Scope")
                        });
                        enrich_builder_type_in_scope(
                            ts,
                            &method_name,
                            has_scope_attr,
                            ctx.current_class,
                            ctx.class_loader,
                        )
                    });

                    let type_str_for_resolution =
                        enriched_type_str.as_deref().or(native_type_str.as_deref());

                    let resolved_from_native = type_str_for_resolution
                        .map(|ts| {
                            crate::completion::type_resolution::type_hint_to_classes(
                                ts,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            )
                        })
                        .unwrap_or_default();

                    if !resolved_from_native.is_empty() {
                        param_results = resolved_from_native;
                        break;
                    }

                    // Native hint didn't resolve (e.g. `object`, `mixed`).
                    // Fall back to the `@param` docblock annotation which
                    // may carry a more specific type such as
                    // `object{foo: int, bar: string}`.
                    let method_start = method.span().start.offset as usize;
                    if let Some(raw_docblock_type) =
                        crate::docblock::find_iterable_raw_type_in_source(
                            ctx.content,
                            method_start,
                            ctx.var_name,
                        )
                    {
                        let resolved = crate::completion::type_resolution::type_hint_to_classes(
                            &raw_docblock_type,
                            &ctx.current_class.name,
                            ctx.all_classes,
                            ctx.class_loader,
                        );
                        if !resolved.is_empty() {
                            param_results = resolved;
                            break;
                        }
                    }

                    // Neither native hint nor docblock resolved.
                    // Check the fully-resolved class (with interface
                    // members merged and `@implements` generics applied)
                    // for a more specific parameter type.  This handles
                    // cases where the class declares `map(object $entity)`
                    // but the interface has `@param TEntity $entity` with
                    // `@implements Interface<Boo>` substituting `TEntity`.
                    let method_name = method.name.value.to_string();
                    let merged = crate::virtual_members::resolve_class_fully_maybe_cached(
                        ctx.current_class,
                        ctx.class_loader,
                        ctx.resolved_class_cache,
                    );
                    if let Some(merged_method) =
                        merged.methods.iter().find(|m| m.name == method_name)
                    {
                        // Find the matching parameter by name.
                        // ParameterInfo.name includes the `$` prefix.
                        if let Some(merged_param) = merged_method
                            .parameters
                            .iter()
                            .find(|p| p.name == ctx.var_name)
                            && let Some(ref hint) = merged_param.type_hint
                        {
                            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                                hint,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            );
                            if !resolved.is_empty() {
                                param_results = resolved;
                                break;
                            }
                        }
                    }
                }
            }

            if let MethodBody::Concrete(block) = &method.body {
                let blk_start = block.left_brace.start.offset;
                let blk_end = block.right_brace.end.offset;
                if ctx.cursor_offset >= blk_start && ctx.cursor_offset <= blk_end {
                    // Extract the enclosing method's @return type for
                    // generator yield inference inside the body.
                    // Use blk_start + 1 (just past the opening `{`)
                    // so the backward brace scan in
                    // find_enclosing_return_type immediately finds
                    // the method's own `{` and does NOT get confused
                    // by intermediate `{`/`}` from nested control-
                    // flow (if, while, foreach, etc.) that would sit
                    // between the cursor and the method brace when
                    // cursor_offset is used.
                    let enclosing_ret = crate::docblock::find_enclosing_return_type(
                        ctx.content,
                        (blk_start + 1) as usize,
                    );
                    let body_ctx = ctx.with_enclosing_return_type(enclosing_ret);
                    // Seed the result set with the parameter type hint
                    // (if any) so that instanceof narrowing and
                    // unconditional reassignments can refine it.
                    let mut results = param_results.clone();
                    walk_statements_for_assignments(
                        block.statements.iter(),
                        &body_ctx,
                        &mut results,
                        false,
                    );
                    if !results.is_empty() {
                        return results;
                    }

                    // ── Generator yield reverse inference ──
                    // If no type was found through normal resolution
                    // and the enclosing method returns a Generator,
                    // check whether our variable appears as the
                    // operand of a `yield` statement and infer its
                    // type from the Generator's TValue parameter.
                    if let Some(ref ret_type) = body_ctx.enclosing_return_type {
                        let yield_results =
                            super::raw_type_inference::try_infer_from_generator_yield(
                                ret_type, &body_ctx,
                            );
                        if !yield_results.is_empty() {
                            return yield_results;
                        }
                    }
                } else {
                    // Cursor is not inside this method's body —
                    // skip to the next method so we don't
                    // accidentally return parameter types from
                    // a different method that happens to share
                    // the same parameter name.
                    continue;
                }
            }

            // Abstract method (no concrete body) — return the
            // parameter type hint only when the cursor falls
            // within the method's overall span (signature region).
            let method_start = method.span().start.offset;
            let method_end = method.span().end.offset;
            if !param_results.is_empty()
                && ctx.cursor_offset >= method_start
                && ctx.cursor_offset <= method_end
            {
                return param_results;
            }
        }
    }
    vec![]
}

/// Walk statements collecting variable assignment types.
///
/// The `conditional` flag indicates whether we are inside a conditional
/// block (if/else, try/catch, loop).  When `conditional` is `false`,
/// a new assignment **replaces** all previous candidates (the variable
/// is being unconditionally reassigned).  When `conditional` is `true`,
/// a new assignment **adds** to the list (the variable *might* be this
/// type).
pub(in crate::completion) fn walk_statements_for_assignments<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    conditional: bool,
) {
    /// Return the sorted set of class names in `results`.
    fn result_names(results: &[ClassInfo]) -> Vec<String> {
        let mut names: Vec<String> = results.iter().map(|c| c.name.clone()).collect();
        names.sort();
        names
    }

    // Accumulator for sequential `assert($x instanceof ...)` calls.
    // Each assert narrows to a single type; this vec collects them
    // so that two asserts in a row produce a union (intersection type).
    let mut assert_narrowed_types: Vec<ClassInfo> = Vec::new();

    for stmt in statements {
        // ── Closure / arrow-function scope ──
        // If the cursor falls *inside* this statement, check whether
        // it is (or contains) a closure / arrow function whose body
        // encloses the cursor.  Closures introduce a new variable
        // scope, so we resolve entirely within that scope and stop.
        let stmt_span = stmt.span();
        if ctx.cursor_offset >= stmt_span.start.offset
            && ctx.cursor_offset <= stmt_span.end.offset
            && super::closure_resolution::try_resolve_in_closure_stmt(stmt, ctx, results)
        {
            return;
        }

        // Only consider statements whose start is before the cursor
        if stmt.span().start.offset >= ctx.cursor_offset {
            continue;
        }

        match stmt {
            Statement::Expression(expr_stmt) => {
                // Try inline `/** @var Type */` override first.
                // If the docblock resolves successfully (and passes
                // the same override check we apply to @return), use
                // it and skip normal resolution for this statement.
                let pre_assign_names = result_names(results);
                if !try_inline_var_override(
                    expr_stmt.expression,
                    stmt.span().start.offset as usize,
                    ctx,
                    results,
                    conditional,
                ) {
                    check_expression_for_assignment(
                        expr_stmt.expression,
                        ctx,
                        results,
                        conditional,
                    );
                }
                // If an assignment (or @var override) changed the
                // results, the variable was reassigned — any prior
                // assert-narrowed types are no longer valid.
                if result_names(results) != pre_assign_names {
                    assert_narrowed_types.clear();
                }

                // ── Pass-by-reference parameter type inference ──
                // When a function call passes our variable to a
                // parameter declared as `Type &$param`, the variable
                // acquires that type after the call.
                try_apply_pass_by_reference_type(expr_stmt.expression, ctx, results, conditional);

                // ── assert($var instanceof ClassName) narrowing ──
                // When `assert($var instanceof Foo)` appears before
                // the cursor, narrow the variable to `Foo` for the
                // remainder of the current scope.
                //
                // Sequential asserts accumulate an intersection type:
                //   assert($x instanceof A);
                //   assert($x instanceof B);
                // → results contains members from both A and B.
                //
                // Save the pre-assert state so we can detect when
                // the assert actually narrowed, and merge with any
                // prior assert-narrowed types.
                let pre_assert_names = result_names(results);
                narrowing::try_apply_assert_instanceof_narrowing(
                    expr_stmt.expression,
                    ctx,
                    results,
                );
                let changed = result_names(results) != pre_assert_names;
                // If the assert changed results AND we had prior
                // assert-narrowed types, merge the old narrowed
                // types back in (accumulate the intersection).
                if changed && !assert_narrowed_types.is_empty() {
                    for cls in &assert_narrowed_types {
                        if !results.iter().any(|c| c.name == cls.name) {
                            results.push(cls.clone());
                        }
                    }
                }
                // Track types that came from assert narrowing.
                if changed {
                    assert_narrowed_types.clone_from(results);
                }

                // ── @phpstan-assert / @psalm-assert narrowing ──
                // When a function with `@phpstan-assert Type $param`
                // is called as a standalone statement, narrow the
                // corresponding argument variable unconditionally.
                narrowing::try_apply_custom_assert_narrowing(expr_stmt.expression, ctx, results);

                // ── match(true) { $var instanceof Foo => … } narrowing ──
                narrowing::try_apply_match_true_narrowing(expr_stmt.expression, ctx, results);

                // ── ternary instanceof narrowing ──
                // `$var instanceof Foo ? $var->method() : …`
                // When the cursor is inside a ternary whose condition
                // checks instanceof, narrow accordingly.
                narrowing::try_apply_ternary_instanceof_narrowing(
                    expr_stmt.expression,
                    ctx,
                    results,
                );

                // ── inline && narrowing ──
                // `$var instanceof Foo && $var->method()`
                // When the cursor is inside the RHS of `&&` whose
                // LHS checks instanceof, narrow accordingly.
                narrowing::try_apply_inline_and_narrowing(expr_stmt.expression, ctx, results);
            }
            // Recurse into blocks — these are just `{ … }` groupings,
            // not conditional, so preserve the current `conditional` flag.
            Statement::Block(block) => {
                walk_statements_for_assignments(block.statements.iter(), ctx, results, conditional);
            }
            Statement::If(if_stmt) => {
                walk_if_statement(if_stmt, stmt, ctx, results);
            }
            Statement::Foreach(foreach) => {
                walk_foreach_statement(foreach, ctx, results, conditional);
            }
            Statement::While(while_stmt) => {
                walk_while_statement(while_stmt, ctx, results);
            }
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(inner) => {
                    check_statement_for_assignments(inner, ctx, results, true);
                }
                ForBody::ColonDelimited(body) => {
                    walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
                }
            },
            Statement::DoWhile(dw) => {
                check_statement_for_assignments(dw.statement, ctx, results, true);
            }
            Statement::Try(try_stmt) => {
                walk_try_statement(try_stmt, ctx, results);
            }
            Statement::Switch(switch) => {
                for case in switch.body.cases().iter() {
                    walk_statements_for_assignments(case.statements().iter(), ctx, results, true);
                }
            }
            // ── unset($var) tracking ──
            // When `unset($var)` appears unconditionally before the
            // cursor, the variable no longer has a type.  Clear all
            // previously accumulated results so that `$var->` does
            // not resolve to the type it had before the unset.
            //
            // Inside conditional branches (`conditional == true`)
            // the variable *might* still exist, so we leave the
            // results untouched.
            Statement::Unset(unset_stmt) => {
                if !conditional {
                    for val in unset_stmt.values.iter() {
                        if let Expression::Variable(Variable::Direct(dv)) = val
                            && dv.name == ctx.var_name
                        {
                            results.clear();
                        }
                    }
                }
            }
            // Recurse into namespace blocks so that assignments and
            // closures inside `namespace Foo { … }` or after
            // `namespace Foo;` are visible to the walker.
            Statement::Namespace(ns) => {
                walk_statements_for_assignments(ns.statements().iter(), ctx, results, conditional);
            }
            _ => {}
        }
    }
}

/// Handle `if` / `elseif` / `else` statements during variable
/// assignment walking.
///
/// Applies instanceof and `@phpstan-assert-if-true/false` narrowing
/// for each branch, recurses into the branch bodies with
/// `conditional = true`, and applies guard-clause narrowing when the
/// cursor is after the if-statement.
fn walk_if_statement<'b>(
    if_stmt: &'b If<'b>,
    enclosing_stmt: &'b Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    // ── Inline && narrowing inside the condition expression ──
    // When the cursor is inside the RHS of `&&` in the condition,
    // apply instanceof narrowing from the LHS so that e.g.
    // `if ($x instanceof Foo && $x->bar())` narrows `$x` to `Foo`
    // at the `$x->bar()` call site.
    narrowing::try_apply_inline_and_narrowing(if_stmt.condition, ctx, results);

    match &if_stmt.body {
        IfBody::Statement(body) => {
            // ── instanceof narrowing for then-body ──
            narrowing::try_apply_instanceof_narrowing(
                if_stmt.condition,
                body.statement.span(),
                ctx,
                results,
            );
            // ── @phpstan-assert-if-true/false narrowing for then-body ──
            narrowing::try_apply_assert_condition_narrowing(
                if_stmt.condition,
                body.statement.span(),
                ctx,
                results,
                false, // not inverted — this is the then-body
            );
            // ── in_array strict-mode narrowing for then-body ──
            narrowing::try_apply_in_array_narrowing(
                if_stmt.condition,
                body.statement.span(),
                ctx,
                results,
            );
            check_statement_for_assignments(body.statement, ctx, results, true);

            for else_if in body.else_if_clauses.iter() {
                // ── inline && narrowing for elseif condition ──
                narrowing::try_apply_inline_and_narrowing(else_if.condition, ctx, results);
                // ── instanceof narrowing for elseif-body ──
                narrowing::try_apply_instanceof_narrowing(
                    else_if.condition,
                    else_if.statement.span(),
                    ctx,
                    results,
                );
                narrowing::try_apply_assert_condition_narrowing(
                    else_if.condition,
                    else_if.statement.span(),
                    ctx,
                    results,
                    false,
                );
                narrowing::try_apply_in_array_narrowing(
                    else_if.condition,
                    else_if.statement.span(),
                    ctx,
                    results,
                );
                check_statement_for_assignments(else_if.statement, ctx, results, true);
            }
            if let Some(else_clause) = &body.else_clause {
                // ── inverse instanceof narrowing for else-body ──
                // `if ($v instanceof Foo) { … } else { ← here }`
                // means $v is NOT Foo in the else branch.
                let else_span = else_clause.statement.span();
                narrowing::try_apply_instanceof_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                narrowing::try_apply_assert_condition_narrowing(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                    true, // inverted — this is the else-body
                );
                narrowing::try_apply_in_array_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                // Also apply inverse narrowing for every elseif condition.
                // In the else branch, all preceding conditions were false,
                // so each elseif's condition is also inverted.
                for else_if in body.else_if_clauses.iter() {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                    narrowing::try_apply_assert_condition_narrowing(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                        true,
                    );
                    narrowing::try_apply_in_array_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                }
                check_statement_for_assignments(else_clause.statement, ctx, results, true);
            }
        }
        IfBody::ColonDelimited(body) => {
            // Determine the then-body span: from the colon to
            // the first elseif / else / endif keyword.
            let then_end = if !body.else_if_clauses.is_empty() {
                body.else_if_clauses
                    .first()
                    .unwrap()
                    .elseif
                    .span()
                    .start
                    .offset
            } else if let Some(ref ec) = body.else_clause {
                ec.r#else.span().start.offset
            } else {
                body.endif.span().start.offset
            };
            let then_span = mago_span::Span::new(
                body.colon.file_id,
                body.colon.start,
                mago_span::Position::new(then_end),
            );
            narrowing::try_apply_instanceof_narrowing(if_stmt.condition, then_span, ctx, results);
            narrowing::try_apply_assert_condition_narrowing(
                if_stmt.condition,
                then_span,
                ctx,
                results,
                false,
            );
            narrowing::try_apply_in_array_narrowing(if_stmt.condition, then_span, ctx, results);
            walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
            for else_if in body.else_if_clauses.iter() {
                // ── inline && narrowing for elseif condition ──
                narrowing::try_apply_inline_and_narrowing(else_if.condition, ctx, results);
                let ei_span = mago_span::Span::new(
                    else_if.colon.file_id,
                    else_if.colon.start,
                    mago_span::Position::new(
                        else_if
                            .statements
                            .span(else_if.colon.file_id, else_if.colon.end)
                            .end
                            .offset,
                    ),
                );
                narrowing::try_apply_instanceof_narrowing(else_if.condition, ei_span, ctx, results);
                narrowing::try_apply_assert_condition_narrowing(
                    else_if.condition,
                    ei_span,
                    ctx,
                    results,
                    false,
                );
                narrowing::try_apply_in_array_narrowing(else_if.condition, ei_span, ctx, results);
                walk_statements_for_assignments(else_if.statements.iter(), ctx, results, true);
            }
            if let Some(else_clause) = &body.else_clause {
                // ── inverse instanceof narrowing for else-body ──
                let else_span = mago_span::Span::new(
                    else_clause.colon.file_id,
                    else_clause.colon.start,
                    mago_span::Position::new(
                        else_clause
                            .statements
                            .span(else_clause.colon.file_id, else_clause.colon.end)
                            .end
                            .offset,
                    ),
                );
                narrowing::try_apply_instanceof_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                narrowing::try_apply_assert_condition_narrowing(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                    true, // inverted — else-body
                );
                narrowing::try_apply_in_array_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                // Also apply inverse narrowing for every elseif condition.
                for else_if in body.else_if_clauses.iter() {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                    narrowing::try_apply_assert_condition_narrowing(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                        true,
                    );
                    narrowing::try_apply_in_array_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                }
                walk_statements_for_assignments(else_clause.statements.iter(), ctx, results, true);
            }
        }
    }

    // ── Guard clause narrowing (early return / throw) ──
    // When the cursor is *after* a guard clause (an `if`
    // whose then-body unconditionally exits via return /
    // throw / continue / break, with no else / elseif),
    // apply the inverse narrowing so subsequent code sees
    // the narrowed type.
    //
    // Example:
    //   if (!$var instanceof Foo) { return; }
    //   $var-> // narrowed to Foo here
    if enclosing_stmt.span().end.offset < ctx.cursor_offset {
        narrowing::apply_guard_clause_narrowing(if_stmt, ctx, results);
        narrowing::apply_guard_clause_in_array_narrowing(if_stmt, ctx, results);
    }
}

/// Handle `foreach` statements during variable assignment walking.
///
/// Resolves the foreach value/key variable and recurses into the body
/// when the cursor is inside the loop body **or** on the foreach header
/// (the `foreach ($expr as $val)` part).  The header check is needed so
/// that hover on the binding variable at its definition site can resolve
/// the iteration type.
fn walk_foreach_statement<'b>(
    foreach: &'b Foreach<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    conditional: bool,
) {
    let body_span = foreach.body.span();
    let header_start = foreach.foreach.span().start.offset;
    if ctx.cursor_offset >= header_start && ctx.cursor_offset <= body_span.end.offset {
        // ── Foreach value/key type from generic iterables ──
        // When the variable we're resolving is the foreach
        // *value* variable, try to infer its type from the
        // iterated expression's generic type annotation.
        //
        // Example:
        //   /** @var list<User> $users */
        //   foreach ($users as $user) { $user-> }
        //
        // Here `$user` is resolved to `User`.
        //
        // Similarly, when the variable is the foreach *key*
        // variable, try to infer its type from the key
        // position of a two-parameter generic annotation.
        //
        // Example:
        //   /** @var SplObjectStorage<Request, Response> $storage */
        //   foreach ($storage as $req => $res) { $req-> }
        //
        // Here `$req` is resolved to `Request`.
        super::foreach_resolution::try_resolve_foreach_value_type(
            foreach,
            ctx,
            results,
            conditional,
        );
        super::foreach_resolution::try_resolve_foreach_key_type(foreach, ctx, results, conditional);

        match &foreach.body {
            ForeachBody::Statement(inner) => {
                check_statement_for_assignments(inner, ctx, results, true);
            }
            ForeachBody::ColonDelimited(body) => {
                walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
            }
        }
    }
}

/// Handle `while` statements during variable assignment walking.
///
/// Applies instanceof and `@phpstan-assert` narrowing based on the
/// while condition and recurses into the loop body.
fn walk_while_statement<'b>(
    while_stmt: &'b While<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    // ── Inline && narrowing inside the while condition ──
    narrowing::try_apply_inline_and_narrowing(while_stmt.condition, ctx, results);

    match &while_stmt.body {
        WhileBody::Statement(inner) => {
            narrowing::try_apply_instanceof_narrowing(
                while_stmt.condition,
                inner.span(),
                ctx,
                results,
            );
            narrowing::try_apply_assert_condition_narrowing(
                while_stmt.condition,
                inner.span(),
                ctx,
                results,
                false,
            );
            narrowing::try_apply_in_array_narrowing(
                while_stmt.condition,
                inner.span(),
                ctx,
                results,
            );
            check_statement_for_assignments(inner, ctx, results, true);
        }
        WhileBody::ColonDelimited(body) => {
            let body_span = mago_span::Span::new(
                body.colon.file_id,
                body.colon.start,
                mago_span::Position::new(body.end_while.span().start.offset),
            );
            narrowing::try_apply_instanceof_narrowing(
                while_stmt.condition,
                body_span,
                ctx,
                results,
            );
            narrowing::try_apply_assert_condition_narrowing(
                while_stmt.condition,
                body_span,
                ctx,
                results,
                false,
            );
            narrowing::try_apply_in_array_narrowing(while_stmt.condition, body_span, ctx, results);
            walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
        }
    }
}

/// Handle `try` / `catch` / `finally` statements during variable
/// assignment walking.
///
/// Recurses into the try block, seeds the catch variable's type from
/// the catch clause's type hint(s), recurses into each catch block,
/// and recurses into the finally block if present.
fn walk_try_statement<'b>(
    try_stmt: &'b Try<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    walk_statements_for_assignments(try_stmt.block.statements.iter(), ctx, results, true);
    for catch in try_stmt.catch_clauses.iter() {
        // Seed the catch variable's type from the catch
        // clause's type hint(s) before recursing into the
        // block.  Handles single types like
        // `catch (ValidationException $e)` and multi-catch
        // like `catch (TypeA | TypeB $e)`.
        if let Some(ref var) = catch.variable
            && var.name == ctx.var_name
        {
            let hint_str = extract_hint_string(&catch.hint);
            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                &hint_str,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            ClassInfo::extend_unique(results, resolved);
        }
        walk_statements_for_assignments(catch.block.statements.iter(), ctx, results, true);
    }
    if let Some(finally) = &try_stmt.finally_clause {
        walk_statements_for_assignments(finally.block.statements.iter(), ctx, results, true);
    }
}

/// Convenience wrapper that walks a single statement for assignments
/// to the target variable, delegating to `walk_statements_for_assignments`.
pub(in crate::completion) fn check_statement_for_assignments<'b>(
    stmt: &'b Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    conditional: bool,
) {
    walk_statements_for_assignments(std::iter::once(stmt), ctx, results, conditional);
}

/// Try to resolve a variable's type from an inline `/** @var … */`
/// docblock that immediately precedes the assignment statement.
///
/// Supports both formats:
///   - `/** @var TheType */`
///   - `/** @var TheType $var */`
///
/// When a variable name is present in the annotation, it must match
/// the variable being resolved.
///
/// The same override check used for `@return` is applied: the docblock
/// type only wins when `resolve_effective_type(native, docblock)` picks
/// the docblock.  If the native (RHS) type is a concrete scalar and the
/// docblock type is a class name, the override is rejected and the
/// method returns `false` so the caller falls through to normal
/// resolution.
///
/// Returns `true` when the override was applied (results updated) and
/// `false` when there is no applicable `@var` annotation.
pub(in crate::completion) fn try_inline_var_override<'b>(
    expr: &'b Expression<'b>,
    stmt_start: usize,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    conditional: bool,
) -> bool {
    // Must be an assignment to our target variable.
    let assignment = match expr {
        Expression::Assignment(a) if a.operator.is_assign() => a,
        _ => return false,
    };
    let lhs_name = match assignment.lhs {
        Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
        _ => return false,
    };
    if lhs_name != ctx.var_name {
        return false;
    }

    // Look for a `/** @var … */` docblock right before this statement.
    let (var_type, var_name) = match docblock::find_inline_var_docblock(ctx.content, stmt_start) {
        Some(pair) => pair,
        None => return false,
    };

    // If the annotation includes a variable name, it must match.
    if let Some(ref vn) = var_name
        && vn != ctx.var_name
    {
        return false;
    }

    // Determine the "native" return-type string from the RHS so we can
    // apply the same override check used for `@return` annotations.
    let native_type = extract_native_type_from_rhs(assignment.rhs, ctx);
    let effective = docblock::resolve_effective_type(native_type.as_deref(), Some(&var_type));

    let eff_type = match effective {
        Some(t) => t,
        None => return false,
    };

    let resolved = crate::completion::type_resolution::type_hint_to_classes(
        &eff_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );

    if resolved.is_empty() {
        return false;
    }

    // Apply the resolved type(s) with the same conditional semantics
    // used by `check_expression_for_assignment`.
    if !conditional {
        results.clear();
    }
    for cls in resolved {
        if !results.iter().any(|c| c.name == cls.name) {
            results.push(cls);
        }
    }
    true
}

/// Extract the "native" return-type string from the RHS of an assignment
/// expression, without resolving it to `ClassInfo`.
///
/// This is used by [`try_inline_var_override`] to feed
/// [`docblock::resolve_effective_type`] with the same kind of type
/// string that `@return` override checking uses.
///
/// Returns `None` when the native type cannot be determined (the
/// caller should treat this as "unknown", which lets the docblock type
/// win unconditionally).
fn extract_native_type_from_rhs<'b>(
    rhs: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    match rhs {
        // `new ClassName(…)` → the class name.
        Expression::Instantiation(inst) => match inst.class {
            Expression::Identifier(ident) => Some(ident.value().to_string()),
            Expression::Self_(_) => Some(ctx.current_class.name.clone()),
            Expression::Static(_) => Some(ctx.current_class.name.clone()),
            _ => None,
        },
        // Function / method calls → look up the return type.
        Expression::Call(call) => match call {
            Call::Function(func_call) => {
                let func_name = match func_call.function {
                    Expression::Identifier(ident) => Some(ident.value().to_string()),
                    _ => None,
                };
                func_name.and_then(|name| {
                    ctx.function_loader
                        .and_then(|fl| fl(&name))
                        .and_then(|fi| fi.return_type)
                })
            }
            Call::Method(method_call) => {
                if let Expression::Variable(Variable::Direct(dv)) = method_call.object
                    && dv.name == "$this"
                    && let ClassLikeMemberSelector::Identifier(ident) = &method_call.method
                {
                    let method_name = ident.value.to_string();
                    ctx.all_classes
                        .iter()
                        .find(|c| c.name == ctx.current_class.name)
                        .and_then(|cls| {
                            cls.methods
                                .iter()
                                .find(|m| m.name == method_name)
                                .and_then(|m| m.return_type.clone())
                        })
                } else {
                    None
                }
            }
            Call::StaticMethod(static_call) => {
                let class_name = match static_call.class {
                    Expression::Self_(_) | Expression::Static(_) => {
                        Some(ctx.current_class.name.clone())
                    }
                    Expression::Identifier(ident) => Some(ident.value().to_string()),
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
                    owner.and_then(|o| {
                        o.methods
                            .iter()
                            .find(|m| m.name == method_name)
                            .and_then(|m| m.return_type.clone())
                    })
                } else {
                    None
                }
            }
            _ => None,
        },
        // First-class callable syntax, closure literals, and arrow
        // functions always produce a Closure.
        Expression::PartialApplication(_)
        | Expression::Closure(_)
        | Expression::ArrowFunction(_) => Some("\\Closure".to_string()),
        _ => None,
    }
}

/// If `expr` is an assignment whose LHS matches `$var_name` and whose
/// RHS is a `new …` instantiation or a function/method call with a
/// known return type, resolve the class and add it to `results`.
///
/// When `conditional` is `false` (unconditional assignment), previous
/// candidates are cleared before adding the new type.  When `true`,
/// the new type is appended (the variable *might* be this type).
///
/// Duplicate class names are suppressed automatically.
pub(in crate::completion) fn check_expression_for_assignment<'b>(
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    conditional: bool,
) {
    let var_name = ctx.var_name;

    /// Push one or more resolved classes into `results`.
    ///
    /// * `conditional == false` → unconditional assignment: **clear**
    ///   previous candidates first, then add all new ones (handles
    ///   union return types like `A|B` from a single assignment).
    /// * `conditional == true` → conditional branch: **append**
    ///   without clearing (the variable *might* be these types).
    ///
    /// Duplicates (same class name) are always suppressed.
    fn push_results(results: &mut Vec<ClassInfo>, new_classes: Vec<ClassInfo>, conditional: bool) {
        if new_classes.is_empty() {
            return;
        }
        if !conditional {
            results.clear();
        }
        ClassInfo::extend_unique(results, new_classes);
    }

    if let Expression::Assignment(assignment) = expr {
        if !assignment.operator.is_assign() {
            return;
        }

        // ── Array destructuring: `[$a, $b] = …` / `list($a, $b) = …` ──
        // When the LHS is an Array or List expression, check whether
        // our target variable appears among its elements.  If so,
        // resolve the RHS's iterable element type.
        if matches!(assignment.lhs, Expression::Array(_) | Expression::List(_)) {
            super::foreach_resolution::try_resolve_destructured_type(
                assignment,
                ctx,
                results,
                conditional,
            );
            return;
        }

        // Check LHS is our variable
        let lhs_name = match assignment.lhs {
            Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
            _ => return,
        };
        if lhs_name != var_name {
            return;
        }

        // Delegate all RHS resolution to the shared helper.
        //
        // Use the assignment's own start offset as cursor_offset so
        // that any recursive variable resolution only considers
        // assignments *before* this one.  Without this, a
        // self-referential assignment like `$value = $value->value`
        // would infinitely recurse: resolving the RHS `$value`
        // would re-discover the same assignment, resolve its RHS
        // again, and so on until a stack overflow crashes the
        // process.
        let rhs_ctx = ctx.with_cursor_offset(assignment.span().start.offset);
        let resolved = super::rhs_resolution::resolve_rhs_expression(assignment.rhs, &rhs_ctx);
        push_results(results, resolved, conditional);
    }
}

// ── Array function type preservation helpers ─────────────────────────

/// Extract the first positional argument expression from an
/// argument list.
pub(in crate::completion) fn first_arg_expr<'b>(
    args: &'b ArgumentList<'b>,
) -> Option<&'b Expression<'b>> {
    args.arguments.first().map(|arg| match arg {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    })
}

/// Extract the nth positional argument expression (0-based).
pub(in crate::completion) fn nth_arg_expr<'b>(
    args: &'b ArgumentList<'b>,
    n: usize,
) -> Option<&'b Expression<'b>> {
    args.arguments.iter().nth(n).map(|arg| match arg {
        Argument::Positional(pos) => pos.value,
        Argument::Named(named) => named.value,
    })
}

/// Resolve the raw iterable type of an argument expression.
///
/// Handles `$variable` (via docblock scanning) and delegates to
/// `extract_rhs_iterable_raw_type` for method calls, property access,
/// etc.
pub(in crate::completion) fn resolve_arg_raw_type<'b>(
    arg_expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<String> {
    // Direct variable — scan for @var / @param annotation.
    if let Expression::Variable(Variable::Direct(dv)) = arg_expr {
        let var_text = dv.name.to_string();
        let offset = arg_expr.span().start.offset as usize;
        let from_docblock =
            docblock::find_iterable_raw_type_in_source(ctx.content, offset, &var_text);
        if from_docblock.is_some() {
            return from_docblock;
        }

        // No docblock — walk the AST for the variable's assignment
        // to extract the raw iterable type.  This handles cases like
        // `$users = $this->getUsers(); array_pop($users)` where
        // `$users` has no `@var` annotation but was assigned from a
        // method returning `list<User>`.
        let current_class = ctx
            .all_classes
            .iter()
            .find(|c| c.name == ctx.current_class.name)
            .map(|c| c.as_ref());
        if let Some(raw) = super::raw_type_inference::resolve_variable_assignment_raw_type(
            &var_text,
            ctx.content,
            offset as u32,
            current_class,
            ctx.all_classes,
            ctx.class_loader,
            ctx.function_loader,
        ) && docblock::types::extract_generic_value_type(&raw).is_some()
        {
            return Some(raw);
        }
    }
    // Fall back to structural extraction (method calls, etc.)
    super::foreach_resolution::extract_rhs_iterable_raw_type(arg_expr, ctx)
}

/// Check whether a call expression passes the target variable to a
/// pass-by-reference parameter with a type hint, and if so, push the
/// resolved type into `results`.
///
/// For example, given `function foo(Baz &$bar): void {}` and the call
/// `foo($bar)`, this function detects that `$bar` is passed to a `&`
/// parameter typed as `Baz` and resolves `$bar` to `Baz`.
///
/// Currently handles standalone function calls (via `function_loader`).
/// Method and static method calls with by-ref parameters are not yet
/// supported.
fn try_apply_pass_by_reference_type(
    expr: &Expression<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
    conditional: bool,
) {
    let (argument_list, parameters) = match expr {
        Expression::Call(Call::Function(func_call)) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return,
            };
            let fl = match ctx.function_loader {
                Some(fl) => fl,
                None => return,
            };
            let func_info = match fl(&func_name) {
                Some(fi) => fi,
                None => return,
            };
            // Borrow the argument list and clone the parameters so we
            // can iterate them together.
            (&func_call.argument_list, func_info.parameters)
        }
        _ => return,
    };

    for (i, arg) in argument_list.arguments.iter().enumerate() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };

        // Check if this argument is our target variable.
        let is_our_var = match arg_expr {
            Expression::Variable(Variable::Direct(dv)) => dv.name == ctx.var_name,
            _ => false,
        };
        if !is_our_var {
            continue;
        }

        // Check if the corresponding parameter is pass-by-reference
        // with a type hint.
        if let Some(param) = parameters.get(i)
            && param.is_reference
            && let Some(ref type_hint) = param.type_hint
        {
            let resolved = crate::completion::type_resolution::type_hint_to_classes(
                type_hint,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            if !resolved.is_empty() {
                if !conditional {
                    results.clear();
                }
                ClassInfo::extend_unique(results, resolved);
            }
        }
    }
}

#[cfg(test)]
#[path = "resolution_tests.rs"]
mod tests;
