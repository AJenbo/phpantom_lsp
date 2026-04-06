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
use std::cell::Cell;
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::completion::types::narrowing;
use crate::docblock;
use crate::parser::{extract_hint_string, with_parsed_program};
use crate::php_type::{PhpType, ShapeEntry};
use crate::types::{ClassInfo, ResolvedType};

use crate::completion::resolver::{Loaders, VarResolutionCtx};

// ── Re-entrancy guard for resolve_variable_types ────────────────────
//
// PHP allows `foreach ($x->method() as $x)` where the foreach value
// variable shadows the iterator receiver.  Resolving `$x` finds the
// foreach, which resolves the iterator expression `$x->method()`,
// which resolves `$x` again — infinite recursion.
//
// The guard lives here, at the very top of `resolve_variable_types`,
// so that the depth check executes before `with_parsed_program`
// allocates its large closure frame on the stack.  Placing the guard
// in a caller (e.g. `resolve_variable_fallback`) is insufficient
// because several other call sites invoke `resolve_variable_types`
// directly (diagnostics, hover, foreach resolution).
thread_local! {
    static VAR_RESOLUTION_DEPTH: Cell<u8> = const { Cell::new(0) };
    /// Sticky flag set when `resolve_variable_types` returns empty
    /// because the depth guard fired.  The flag stays set until the
    /// outermost `resolve_variable_types` call returns, so callers
    /// up the stack (e.g. `resolve_target_classes`) can detect that
    /// an empty result was caused by the depth limit — not because
    /// the variable is genuinely unresolvable — and skip caching it.
    static VAR_RESOLUTION_DEPTH_LIMITED: Cell<bool> = const { Cell::new(false) };
}

/// Maximum nesting depth for `resolve_variable_types` calls.
///
/// Four levels covers legitimate nested resolution such as:
///   depth 0: resolve `$arr` built via `$arr[$key] = ['k' => $var]`
///   depth 1: resolve `$var` (array element variable in the shape literal)
///   depth 2: resolve `$item->prop` (RHS of `$var = $item->prop`)
///   depth 3: resolve `$item` (foreach value binding → iterable param)
///
/// Cycles like `foreach ($x->method() as $x)` are caught by a
/// targeted check in `try_resolve_foreach_value_type` (which skips
/// resolution when the value variable shadows the iterator receiver)
/// rather than by this depth limit alone.  The depth limit is a
/// safety net for any remaining recursive patterns.
const MAX_VAR_RESOLUTION_DEPTH: u8 = 4;

/// Returns `true` when any `resolve_variable_types` call on the
/// current stack has returned empty because the depth guard fired.
///
/// The flag is sticky: once set, it stays `true` until the outermost
/// `resolve_variable_types` call returns and clears it.  This lets
/// callers further up the stack (e.g. `resolve_target_classes`) detect
/// that an empty result was caused by the depth limit — not because
/// the variable is genuinely unresolvable — and skip caching it.
pub(crate) fn is_var_resolution_depth_limited() -> bool {
    VAR_RESOLUTION_DEPTH_LIMITED.with(|f| f.get())
}

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
    if PhpType::parse(type_str).has_type_structure() {
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
    loaders: Loaders<'_>,
) -> Vec<ResolvedType> {
    // ── Depth guard ─────────────────────────────────────────────
    let depth = VAR_RESOLUTION_DEPTH.with(|d| {
        let cur = d.get();
        d.set(cur.saturating_add(1));
        cur
    });
    if depth >= MAX_VAR_RESOLUTION_DEPTH {
        VAR_RESOLUTION_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        VAR_RESOLUTION_DEPTH_LIMITED.with(|f| f.set(true));
        return vec![];
    }

    let result = with_parsed_program(content, "resolve_variable_types", |program, _content| {
        let ctx = VarResolutionCtx {
            var_name,
            current_class,
            all_classes,
            content,
            cursor_offset,
            class_loader,
            loaders,
            resolved_class_cache: None,
            enclosing_return_type: None,
            branch_aware: false,
        };

        resolve_variable_in_statements(program.statements.iter(), &ctx)
    });

    let new_depth = VAR_RESOLUTION_DEPTH.with(|d| {
        let v = d.get().saturating_sub(1);
        d.set(v);
        v
    });
    // Clear the depth-limited flag when the outermost call returns.
    // Inner calls leave it set so that every caller in the stack
    // can observe it.
    if new_depth == 0 {
        VAR_RESOLUTION_DEPTH_LIMITED.with(|f| f.set(false));
    }
    result
}

/// Resolve variable types with branch-aware if/else handling.
///
/// Like [`resolve_variable_types`], but when the cursor is inside a
/// specific if/else/elseif branch, only that branch's assignments
/// contribute to the result.  This produces the single type visible
/// at the cursor position, which is what hover needs (e.g. only `Lamp`
/// inside an if-branch, not `Lamp|Faucet`).
pub(crate) fn resolve_variable_types_branch_aware(
    var_name: &str,
    current_class: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    content: &str,
    cursor_offset: u32,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    loaders: Loaders<'_>,
) -> Vec<ResolvedType> {
    // ── Depth guard (same as resolve_variable_types) ────────────
    let depth = VAR_RESOLUTION_DEPTH.with(|d| {
        let cur = d.get();
        d.set(cur.saturating_add(1));
        cur
    });
    if depth >= MAX_VAR_RESOLUTION_DEPTH {
        VAR_RESOLUTION_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        VAR_RESOLUTION_DEPTH_LIMITED.with(|f| f.set(true));
        return vec![];
    }

    let result = with_parsed_program(
        content,
        "resolve_variable_types_branch_aware",
        |program, _content| {
            let ctx = VarResolutionCtx {
                var_name,
                current_class,
                all_classes,
                content,
                cursor_offset,
                class_loader,
                loaders,
                resolved_class_cache: None,
                enclosing_return_type: None,
                branch_aware: true,
            };

            resolve_variable_in_statements(program.statements.iter(), &ctx)
        },
    );

    let new_depth = VAR_RESOLUTION_DEPTH.with(|d| {
        let v = d.get().saturating_sub(1);
        d.set(v);
        v
    });
    if new_depth == 0 {
        VAR_RESOLUTION_DEPTH_LIMITED.with(|f| f.set(false));
    }
    result
}

/// Walk a sequence of top-level statements to find the class or
/// function body that contains the cursor, then resolve the target
/// variable's type within that scope.
pub(in crate::completion) fn resolve_variable_in_statements<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<ResolvedType> {
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

        // ── Anonymous classes inside expressions ──
        // Anonymous classes (`new class { … }`) appear as expressions
        // inside statements (e.g. `return new class extends Foo { … };`
        // or `$x = new class { … };`).  If the cursor falls inside one,
        // resolve variables from its member methods just like we do for
        // named classes above.
        let stmt_span = stmt.span();
        if ctx.cursor_offset >= stmt_span.start.offset
            && ctx.cursor_offset <= stmt_span.end.offset
            && let Some(anon) = find_anonymous_class_containing_cursor(stmt, ctx.cursor_offset)
        {
            return resolve_variable_in_members(anon.members.iter(), ctx);
        }
    }

    // The cursor is not inside any class/interface/enum body — it must
    // be in top-level code.  Walk all top-level statements to find
    // variable assignments (e.g. `$user = new User(…);`).
    let mut results: Vec<ResolvedType> = Vec::new();
    walk_statements_for_assignments(stmts.into_iter(), ctx, &mut results, false);
    results
}

/// Recursively walk a statement's expression tree looking for an
/// `AnonymousClass` whose body (between `{` and `}`) contains the
/// given cursor offset.  Returns a reference to the first matching
/// anonymous class node, or `None`.
fn find_anonymous_class_containing_cursor<'a>(
    stmt: &'a Statement<'a>,
    cursor_offset: u32,
) -> Option<&'a AnonymousClass<'a>> {
    /// Walk an expression tree for an anonymous class containing the cursor.
    fn walk_expr<'a>(expr: &'a Expression<'a>, cursor: u32) -> Option<&'a AnonymousClass<'a>> {
        let sp = expr.span();
        if cursor < sp.start.offset || cursor > sp.end.offset {
            return None;
        }
        match expr {
            Expression::AnonymousClass(anon) => {
                if cursor >= anon.left_brace.start.offset && cursor <= anon.right_brace.end.offset {
                    return Some(anon);
                }
                None
            }
            Expression::Parenthesized(p) => walk_expr(p.expression, cursor),
            Expression::Assignment(a) => {
                walk_expr(a.lhs, cursor).or_else(|| walk_expr(a.rhs, cursor))
            }
            Expression::Binary(b) => walk_expr(b.lhs, cursor).or_else(|| walk_expr(b.rhs, cursor)),
            Expression::Conditional(c) => walk_expr(c.condition, cursor)
                .or_else(|| c.then.and_then(|e| walk_expr(e, cursor)))
                .or_else(|| walk_expr(c.r#else, cursor)),
            Expression::Call(call) => match call {
                Call::Function(fc) => walk_args(&fc.argument_list.arguments, cursor),
                Call::Method(mc) => walk_expr(mc.object, cursor)
                    .or_else(|| walk_args(&mc.argument_list.arguments, cursor)),
                Call::NullSafeMethod(mc) => walk_expr(mc.object, cursor)
                    .or_else(|| walk_args(&mc.argument_list.arguments, cursor)),
                Call::StaticMethod(sc) => walk_expr(sc.class, cursor)
                    .or_else(|| walk_args(&sc.argument_list.arguments, cursor)),
            },
            Expression::Array(arr) => {
                for elem in arr.elements.iter() {
                    let found = match elem {
                        ArrayElement::KeyValue(kv) => {
                            walk_expr(kv.key, cursor).or_else(|| walk_expr(kv.value, cursor))
                        }
                        ArrayElement::Value(v) => walk_expr(v.value, cursor),
                        ArrayElement::Variadic(v) => walk_expr(v.value, cursor),
                        _ => None,
                    };
                    if found.is_some() {
                        return found;
                    }
                }
                None
            }
            Expression::LegacyArray(arr) => {
                for elem in arr.elements.iter() {
                    let found = match elem {
                        ArrayElement::KeyValue(kv) => {
                            walk_expr(kv.key, cursor).or_else(|| walk_expr(kv.value, cursor))
                        }
                        ArrayElement::Value(v) => walk_expr(v.value, cursor),
                        ArrayElement::Variadic(v) => walk_expr(v.value, cursor),
                        _ => None,
                    };
                    if found.is_some() {
                        return found;
                    }
                }
                None
            }
            Expression::Closure(closure) => {
                // The anonymous class could be inside a closure body.
                for inner in closure.body.statements.iter() {
                    if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor) {
                        return Some(anon);
                    }
                }
                None
            }
            Expression::ArrowFunction(arrow) => walk_expr(arrow.expression, cursor),
            Expression::Instantiation(inst) => {
                if let Some(ref args) = inst.argument_list {
                    walk_args(&args.arguments, cursor)
                } else {
                    None
                }
            }
            Expression::UnaryPrefix(u) => walk_expr(u.operand, cursor),
            Expression::UnaryPostfix(u) => walk_expr(u.operand, cursor),
            Expression::Throw(t) => walk_expr(t.exception, cursor),
            Expression::Clone(c) => walk_expr(c.object, cursor),
            Expression::Match(m) => {
                if let Some(found) = walk_expr(m.expression, cursor) {
                    return Some(found);
                }
                for arm in m.arms.iter() {
                    if let Some(found) = walk_expr(arm.expression(), cursor) {
                        return Some(found);
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Walk a list of call arguments.
    fn walk_args<'a>(
        arguments: &'a mago_syntax::ast::sequence::TokenSeparatedSequence<'a, Argument<'a>>,
        cursor: u32,
    ) -> Option<&'a AnonymousClass<'a>> {
        for arg in arguments.iter() {
            let arg_expr = match arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            if let Some(found) = walk_expr(arg_expr, cursor) {
                return Some(found);
            }
        }
        None
    }

    match stmt {
        Statement::Expression(expr_stmt) => walk_expr(expr_stmt.expression, cursor_offset),
        Statement::Return(ret) => ret.value.as_ref().and_then(|v| walk_expr(v, cursor_offset)),
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset) {
                    return Some(anon);
                }
            }
            None
        }
        Statement::If(if_stmt) => match &if_stmt.body {
            IfBody::Statement(body) => {
                find_anonymous_class_containing_cursor(body.statement, cursor_offset)
            }
            IfBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset)
                    {
                        return Some(anon);
                    }
                }
                None
            }
        },
        Statement::Foreach(foreach) => match &foreach.body {
            ForeachBody::Statement(inner) => {
                find_anonymous_class_containing_cursor(inner, cursor_offset)
            }
            ForeachBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset)
                    {
                        return Some(anon);
                    }
                }
                None
            }
        },
        Statement::While(while_stmt) => match &while_stmt.body {
            WhileBody::Statement(inner) => {
                find_anonymous_class_containing_cursor(inner, cursor_offset)
            }
            WhileBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset)
                    {
                        return Some(anon);
                    }
                }
                None
            }
        },
        Statement::For(for_stmt) => match &for_stmt.body {
            ForBody::Statement(inner) => {
                find_anonymous_class_containing_cursor(inner, cursor_offset)
            }
            ForBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset)
                    {
                        return Some(anon);
                    }
                }
                None
            }
        },
        Statement::DoWhile(dw) => {
            find_anonymous_class_containing_cursor(dw.statement, cursor_offset)
        }
        Statement::Try(try_stmt) => {
            for inner in try_stmt.block.statements.iter() {
                if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset) {
                    return Some(anon);
                }
            }
            for catch in try_stmt.catch_clauses.iter() {
                for inner in catch.block.statements.iter() {
                    if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset)
                    {
                        return Some(anon);
                    }
                }
            }
            if let Some(finally) = &try_stmt.finally_clause {
                for inner in finally.block.statements.iter() {
                    if let Some(anon) = find_anonymous_class_containing_cursor(inner, cursor_offset)
                    {
                        return Some(anon);
                    }
                }
            }
            None
        }
        _ => None,
    }
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
) -> Option<Vec<ResolvedType>> {
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
        crate::docblock::find_enclosing_return_type(ctx.content, (body_start + 1) as usize)
            .map(|s| PhpType::parse(&s));
    let body_ctx = ctx.with_enclosing_return_type(enclosing_ret);
    // The cursor is inside this function body.  PHP function scopes
    // are isolated, so return the result directly (even if empty
    // after `unset`).
    let mut results: Vec<ResolvedType> = Vec::new();
    super::closure_resolution::resolve_closure_params(
        &func.parameter_list,
        &body_ctx,
        &mut results,
    );

    // ── Substitute function-level template params with their
    // bounds for standalone function parameters ─────────────────
    // When the resolved parameter type is `class-string<T>` or a
    // bare template name `T` and `T` is a function-level template
    // with an upper bound, replace `T` with the bound so that
    // member access resolves against the bound type.
    let func_start = func.span().start.offset as usize;
    for rt in results.iter_mut() {
        if matches!(&rt.type_string, PhpType::ClassString(Some(inner)) if matches!(inner.as_ref(), PhpType::Named(_)))
        {
            rt.type_string = substitute_class_string_template_bounds(
                rt.type_string.clone(),
                ctx.content,
                func_start,
            );
        }
        // Bare template param → bound (e.g. `T` → `Builder|QueryBuilder`).
        rt.type_string =
            substitute_template_param_bounds(rt.type_string.clone(), ctx.content, func_start);
    }

    walk_statements_for_assignments(func.body.statements.iter(), &body_ctx, &mut results, false);
    if !results.is_empty() {
        return Some(results);
    }

    // Fall back to a standalone `@var` docblock scan when parameter
    // resolution and assignment walking yielded no type.
    super::closure_resolution::try_standalone_var_docblock(&body_ctx, &mut results);
    if !results.is_empty() {
        return Some(results);
    }

    // Generator yield reverse inference for top-level functions.
    if let Some(ref ret_type) = body_ctx.enclosing_return_type {
        let yield_results =
            super::raw_type_inference::try_infer_from_generator_yield(ret_type, &body_ctx);
        if !yield_results.is_empty() {
            return Some(ResolvedType::from_classes(yield_results));
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
) -> Option<Vec<ResolvedType>> {
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
) -> Vec<ResolvedType> {
    for member in members {
        if let ClassLikeMember::Method(method) = member {
            // Collect parameter type hint as initial candidate set.
            // We no longer return early here so that the method body
            // can be scanned for instanceof narrowing / reassignments.
            let mut param_results: Vec<ResolvedType> = Vec::new();
            let mut matched_param_is_variadic = false;
            for param in method.parameter_list.parameters.iter() {
                let pname = param.variable.name.to_string();
                if pname == ctx.var_name {
                    matched_param_is_variadic = param.ellipsis.is_some();
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

                    // Check the `@param` docblock annotation which may
                    // carry a more specific type than the native hint
                    // (e.g. `@param FuncCall $node` on `Node $node`).
                    let method_start = method.span().start.offset as usize;
                    let raw_docblock_type = crate::docblock::find_iterable_raw_type_in_source(
                        ctx.content,
                        method_start,
                        ctx.var_name,
                    );

                    // Pick the effective type: docblock overrides native
                    // when it is a compatible refinement.
                    let effective_type = crate::docblock::resolve_effective_type(
                        type_str_for_resolution,
                        raw_docblock_type.as_deref(),
                    );

                    // ── Substitute method-level template params with
                    // their bounds before class resolution ──────────
                    // When the effective type contains a bare template
                    // parameter name (e.g. `T` from `@template T of
                    // Builder|QueryBuilder`), replace it with the bound
                    // type so that `$query->where(...)` resolves members
                    // on `Builder|QueryBuilder` instead of failing with
                    // "subject type 'T' could not be resolved".
                    let effective_type = effective_type.map(|ty| {
                        substitute_template_param_bounds(
                            ty,
                            ctx.content,
                            method.span().start.offset as usize,
                        )
                    });

                    let resolved_from_effective = effective_type
                        .as_ref()
                        .map(|ty| {
                            crate::completion::type_resolution::type_hint_to_classes_typed(
                                ty,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            )
                        })
                        .unwrap_or_default();

                    if !resolved_from_effective.is_empty() {
                        param_results = ResolvedType::from_classes_with_hint(
                            resolved_from_effective,
                            effective_type.unwrap_or_else(|| {
                                PhpType::parse(type_str_for_resolution.unwrap_or(""))
                            }),
                        );
                        break;
                    }

                    // The effective type didn't resolve to a class (e.g.
                    // `object`, `mixed`, or an object shape). Fall back to
                    // the raw `@param` docblock annotation which may carry
                    // a more specific non-class type such as
                    // `object{foo: int, bar: string}`.
                    if let Some(ref raw_docblock_type) = raw_docblock_type {
                        let resolved = crate::completion::type_resolution::type_hint_to_classes(
                            raw_docblock_type,
                            &ctx.current_class.name,
                            ctx.all_classes,
                            ctx.class_loader,
                        );
                        if !resolved.is_empty() {
                            param_results = ResolvedType::from_classes_with_hint(
                                resolved,
                                PhpType::parse(raw_docblock_type),
                            );
                            break;
                        }
                    }

                    // Neither native hint nor docblock resolved to a class.
                    // Check the fully-resolved class (with interface
                    // members merged and `@implements` generics applied)
                    // for a more specific parameter type.  This handles
                    // cases where the class declares `map(object $entity)`
                    // but the interface has `@param TEntity $entity` with
                    // `@implements Interface<Boo>` substituting `TEntity`.
                    let mut merged_type_hint: Option<String> = None;
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
                            let hint_str = hint.to_string();
                            let resolved =
                                crate::completion::type_resolution::type_hint_to_classes_typed(
                                    hint,
                                    &ctx.current_class.name,
                                    ctx.all_classes,
                                    ctx.class_loader,
                                );
                            if !resolved.is_empty() {
                                param_results = ResolvedType::from_classes_with_hint(
                                    resolved,
                                    PhpType::parse(&hint_str),
                                );
                                break;
                            }
                            // The merged type hint is richer than the
                            // native hint (e.g. `list<Pen>` vs `array`)
                            // but didn't resolve to a class. Remember
                            // it so the type-string-only fallback below
                            // uses it instead of the bare native hint.
                            merged_type_hint = Some(hint_str);
                        }
                    }

                    // All class-resolution attempts failed.  Emit a
                    // type-string-only entry so that consumers like hover
                    // and diagnostics can see the parameter's type even
                    // when it's a scalar or PHPDoc pseudo-type.
                    //
                    // Prefer the docblock type (e.g. `class-string<BackedEnum>`)
                    // over the native type (e.g. `string`) when the
                    // docblock provides a more specific annotation.
                    // When the merged class provides a richer type from
                    // parent/interface inheritance (e.g. `list<Pen>` from
                    // a parent's `@param`), prefer that over the bare
                    // native hint.
                    let best_type_str = raw_docblock_type
                        .as_deref()
                        .or(merged_type_hint.as_deref())
                        .or(type_str_for_resolution);
                    if let Some(ts) = best_type_str {
                        let mut parsed = PhpType::parse(ts);

                        // ── Substitute method-level template params
                        // in `class-string<T>` with their bounds ────────
                        // When the parameter type is `class-string<T>` and
                        // `T` is a method-level template with an upper bound
                        // (e.g. `@template T of CustomerModel`), replace `T`
                        // with `CustomerModel` so that static member access
                        // (`$class::KEY`, `$class::from(...)`) resolves
                        // against the bound type.
                        parsed = substitute_class_string_template_bounds(
                            parsed,
                            ctx.content,
                            method.span().start.offset as usize,
                        );

                        param_results = vec![ResolvedType::from_type_string(parsed)];
                    }
                }
            }

            // ── Variadic parameter wrapping ─────────────────────────
            // When the matched parameter is variadic (e.g.
            // `HtmlString|int|string ...$placeholders`), the native
            // type hint describes the *element* type, but the variable
            // itself holds `list<ElementType>`.  Wrap the resolved
            // types so that foreach iteration can extract the element
            // type via `PhpType::extract_value_type`.
            if matched_param_is_variadic && !param_results.is_empty() {
                for rt in &mut param_results {
                    rt.type_string =
                        PhpType::Generic("list".to_string(), vec![rt.type_string.clone()]);
                    // The variable is now an array, not a class instance,
                    // so clear the class_info.
                    rt.class_info = None;
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
                    )
                    .map(|s| PhpType::parse(&s));
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

                    // Fall back to a standalone `@var` docblock scan
                    // when parameter resolution and assignment walking
                    // yielded no type.
                    super::closure_resolution::try_standalone_var_docblock(&body_ctx, &mut results);
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
                            return ResolvedType::from_classes(yield_results);
                        }
                    }

                    // The concrete body was walked but produced no
                    // results.  This can happen when instanceof
                    // narrowing targeted an unresolvable class (e.g.
                    // from a phar) and cleared the parameter type.
                    // Do NOT fall through to the abstract-method
                    // param fallback below — returning the un-narrowed
                    // parameter type would cause false-positive
                    // "unknown member" diagnostics for members that
                    // only exist on the narrowed (unresolvable) class.
                    return vec![];
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

/// Substitute method/function-level template parameter names with their
/// upper bounds from `@template T of Bound` annotations.
///
/// This handles the general case where a parameter type IS a template
/// parameter (e.g. `@param T $query` where `@template T of Builder`).
/// Without this substitution, `T` remains an unresolvable named type
/// and member access on `$query` fails with "subject type 'T' could not
/// be resolved".
///
/// Works on any `PhpType` structure — bare names, unions, intersections,
/// nullable wrappers, generics, etc. — via `PhpType::substitute`.
fn substitute_template_param_bounds(
    ty: PhpType,
    content: &str,
    method_start_offset: usize,
) -> PhpType {
    // Quick check: only act when the type contains at least one bare
    // identifier that could be a template parameter.  This avoids the
    // docblock parse for the common case where the type is a concrete
    // class name or scalar.
    if !type_may_contain_template_param(&ty) {
        return ty;
    }

    let before = &content[..method_start_offset];
    let docblock = extract_preceding_docblock(before);

    let Some(docblock) = docblock else {
        return ty;
    };

    let bounds = docblock::extract_template_params_with_bounds(docblock);
    if bounds.is_empty() {
        return ty;
    }

    let mut subs = std::collections::HashMap::new();
    for (name, bound) in bounds {
        if let Some(bound_str) = bound {
            subs.insert(name, PhpType::parse(&bound_str));
        }
    }

    if subs.is_empty() {
        return ty;
    }

    ty.substitute(&subs)
}

/// Check whether a `PhpType` tree may contain a bare template parameter
/// name — i.e. a `Named` variant whose value is not a well-known scalar
/// or pseudo-type.  This is a cheap pre-filter so that we only parse the
/// docblock when there is a realistic chance of finding a substitution.
fn type_may_contain_template_param(ty: &PhpType) -> bool {
    match ty {
        PhpType::Named(name) => {
            // Well-known scalars/pseudo-types are never template params.
            !matches!(
                name.as_str(),
                "int"
                    | "float"
                    | "string"
                    | "bool"
                    | "null"
                    | "void"
                    | "never"
                    | "mixed"
                    | "object"
                    | "array"
                    | "iterable"
                    | "callable"
                    | "resource"
                    | "true"
                    | "false"
                    | "self"
                    | "static"
                    | "$this"
                    | "parent"
                    | "class-string"
            )
        }
        PhpType::Union(members) | PhpType::Intersection(members) => {
            members.iter().any(type_may_contain_template_param)
        }
        PhpType::Nullable(inner) => type_may_contain_template_param(inner),
        PhpType::Generic(base, args) => {
            // Check if the base itself could be a template param, or any arg.
            type_may_contain_template_param(&PhpType::Named(base.clone()))
                || args.iter().any(type_may_contain_template_param)
        }
        _ => false,
    }
}

/// Substitute method-level template parameters inside `class-string<T>`
/// types with their upper bounds from `@template T of Bound` annotations.
///
/// This enables `$class::` static member access resolution when the
/// parameter is typed as `class-string<T>` and `T` is bounded by a
/// concrete class.  Without this substitution, `T` remains an
/// unresolvable named type and `$class::` yields no completions.
fn substitute_class_string_template_bounds(
    ty: PhpType,
    content: &str,
    method_start_offset: usize,
) -> PhpType {
    // Only act on class-string<T> where the inner type is a simple name
    // (i.e. a potential template parameter).
    let inner_name = match &ty {
        PhpType::ClassString(Some(inner)) => match inner.as_ref() {
            PhpType::Named(name) => Some(name.clone()),
            _ => None,
        },
        _ => None,
    };

    let Some(tpl_name) = inner_name else {
        return ty;
    };

    // Extract the method's docblock to find template parameter bounds.
    // The docblock sits immediately before the method declaration, so
    // we search backward from the method's start offset.
    let before = &content[..method_start_offset];
    let docblock = extract_preceding_docblock(before);

    let Some(docblock) = docblock else {
        return ty;
    };

    let bounds = docblock::extract_template_params_with_bounds(docblock);
    for (name, bound) in bounds {
        if name == tpl_name
            && let Some(bound_str) = bound
        {
            return PhpType::ClassString(Some(Box::new(PhpType::parse(&bound_str))));
        }
    }

    ty
}

/// Extract the docblock comment immediately preceding a given offset.
///
/// Scans backward from `before` (the source text up to the method start)
/// to find the closest `/** ... */` block.  Returns `None` when no
/// docblock is found or when there is non-whitespace between the
/// docblock and the method declaration.
fn extract_preceding_docblock(before: &str) -> Option<&str> {
    let trimmed = before.trim_end();
    if !trimmed.ends_with("*/") {
        return None;
    }
    let close_pos = trimmed.len();
    let open_pos = trimmed.rfind("/**")?;
    Some(&trimmed[open_pos..close_pos])
}

pub(in crate::completion) fn walk_statements_for_assignments<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
    conditional: bool,
) {
    /// Return the sorted set of class names in `results`.
    fn result_names(results: &[ResolvedType]) -> Vec<String> {
        let mut names: Vec<String> = results
            .iter()
            .filter_map(|rt| rt.class_info.as_ref().map(|c| c.name.clone()))
            .collect();
        names.sort();
        names
    }

    // Accumulator for sequential `assert($x instanceof ...)` calls.
    // Each assert narrows to a single type; this vec collects them
    // so that two asserts in a row produce a union (intersection type).
    let mut assert_narrowed_types: Vec<ResolvedType> = Vec::new();

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
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_assert_instanceof_narrowing(
                        expr_stmt.expression,
                        ctx,
                        classes,
                    );
                });
                let changed = result_names(results) != pre_assert_names;
                // If the assert changed results AND we had prior
                // assert-narrowed types, merge the old narrowed
                // types back in (accumulate the intersection).
                if changed && !assert_narrowed_types.is_empty() {
                    for rt in &assert_narrowed_types {
                        if !results
                            .iter()
                            .any(|existing| existing.type_string == rt.type_string)
                        {
                            results.push(rt.clone());
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
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_custom_assert_narrowing(
                        expr_stmt.expression,
                        ctx,
                        classes,
                    );
                });

                // ── match(true) { $var instanceof Foo => … } narrowing ──
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_match_true_narrowing(expr_stmt.expression, ctx, classes);
                });

                // ── ternary instanceof narrowing ──
                // `$var instanceof Foo ? $var->method() : …`
                // When the cursor is inside a ternary whose condition
                // checks instanceof, narrow accordingly.
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_ternary_instanceof_narrowing(
                        expr_stmt.expression,
                        ctx,
                        classes,
                    );
                });

                // ── inline && narrowing ──
                // `$var instanceof Foo && $var->method()`
                // When the cursor is inside the RHS of `&&` whose
                // LHS checks instanceof, narrow accordingly.
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_inline_and_narrowing(expr_stmt.expression, ctx, classes);
                });
                // ── inline && null narrowing ──
                // `$var !== null && $var->method()`
                // When the cursor is inside the RHS of `&&` whose
                // LHS checks for non-null, strip null from resolved types.
                narrowing::try_apply_inline_and_null_narrowing(expr_stmt.expression, ctx, results);
            }
            // ── Return statements ──
            // The return value expression can contain narrowing
            // constructs like `return $x instanceof Foo && $x->bar()`
            // or `return $x instanceof Foo ? $x->method() : null`.
            // Apply the same expression-level narrowing that we
            // apply to standalone expression statements.
            Statement::Return(ret) => {
                if let Some(val) = ret.value {
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_match_true_narrowing(val, ctx, classes);
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_ternary_instanceof_narrowing(val, ctx, classes);
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_inline_and_narrowing(val, ctx, classes);
                    });
                    narrowing::try_apply_inline_and_null_narrowing(val, ctx, results);
                }
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
            Statement::For(for_stmt) => {
                // ── Pre-scan for loop-carried assignments ──
                let for_body_span = for_stmt.body.span();
                let for_header_start = for_stmt.r#for.span().start.offset;
                let cursor_inside_for = ctx.cursor_offset >= for_header_start
                    && ctx.cursor_offset <= for_body_span.end.offset;
                if cursor_inside_for {
                    match &for_stmt.body {
                        ForBody::Statement(inner) => {
                            prescan_loop_body_for_assignments(
                                std::iter::once(*inner),
                                for_body_span.end.offset,
                                ctx,
                                results,
                            );
                        }
                        ForBody::ColonDelimited(body) => {
                            prescan_loop_body_for_assignments(
                                body.statements.iter(),
                                for_body_span.end.offset,
                                ctx,
                                results,
                            );
                        }
                    }
                }
                match &for_stmt.body {
                    ForBody::Statement(inner) => {
                        check_statement_for_assignments(inner, ctx, results, true);
                    }
                    ForBody::ColonDelimited(body) => {
                        walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
                    }
                }
            }
            Statement::DoWhile(dw) => {
                // ── Pre-scan for loop-carried assignments ──
                let dw_body_span = dw.statement.span();
                let dw_header_start = dw.r#do.span().start.offset;
                let cursor_inside_dw = ctx.cursor_offset >= dw_header_start
                    && ctx.cursor_offset <= dw_body_span.end.offset;
                if cursor_inside_dw {
                    prescan_loop_body_for_assignments(
                        std::iter::once(dw.statement),
                        dw_body_span.end.offset,
                        ctx,
                        results,
                    );
                }
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
    results: &mut Vec<ResolvedType>,
) {
    // ── Branch-aware mode ──
    // When `ctx.branch_aware` is true and the cursor is inside a
    // specific branch body, only that branch's assignments contribute
    // to the result.  This produces the single type visible at the
    // cursor position (what hover needs).  If the cursor is not inside
    // any branch body (i.e. it is after the if/else), fall through to
    // the normal union-all-branches logic.
    if ctx.branch_aware
        && let Some(()) = walk_if_branch_aware(if_stmt, enclosing_stmt, ctx, results)
    {
        return;
    }

    // ── Inline && narrowing inside the condition expression ──
    // When the cursor is inside the RHS of `&&` in the condition,
    // apply instanceof narrowing from the LHS so that e.g.
    // `if ($x instanceof Foo && $x->bar())` narrows `$x` to `Foo`
    // at the `$x->bar()` call site.
    ResolvedType::apply_narrowing(results, |classes| {
        narrowing::try_apply_inline_and_narrowing(if_stmt.condition, ctx, classes);
    });
    narrowing::try_apply_inline_and_null_narrowing(if_stmt.condition, ctx, results);

    // ── Assignment in condition ──
    // `if ($x = expr())` — register the assignment so `$x` is typed
    // inside the then-body and subsequent code.
    check_condition_for_assignment(if_stmt.condition, ctx, results);

    match &if_stmt.body {
        IfBody::Statement(body) => {
            // ── instanceof narrowing for then-body ──
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_instanceof_narrowing(
                    if_stmt.condition,
                    body.statement.span(),
                    ctx,
                    classes,
                );
            });
            // ── @phpstan-assert-if-true/false narrowing for then-body ──
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_assert_condition_narrowing(
                    if_stmt.condition,
                    body.statement.span(),
                    ctx,
                    classes,
                    false, // not inverted — this is the then-body
                );
            });
            // ── in_array strict-mode narrowing for then-body ──
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_in_array_narrowing(
                    if_stmt.condition,
                    body.statement.span(),
                    ctx,
                    classes,
                );
            });
            // ── null narrowing for then-body ──
            // Must run after check_condition_for_assignment so the
            // type is registered before we try to strip null from it.
            narrowing::try_apply_if_body_null_narrowing(
                if_stmt.condition,
                body.statement.span(),
                ctx,
                results,
            );
            // ── type-guard narrowing for then-body ──
            // `is_array($var)`, `is_string($var)`, etc.
            narrowing::try_apply_type_guard_narrowing(
                if_stmt.condition,
                body.statement.span(),
                ctx,
                results,
            );
            check_statement_for_assignments(body.statement, ctx, results, true);

            for else_if in body.else_if_clauses.iter() {
                // ── inline && narrowing for elseif condition ──
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_inline_and_narrowing(else_if.condition, ctx, classes);
                });
                narrowing::try_apply_inline_and_null_narrowing(else_if.condition, ctx, results);
                // ── Assignment in elseif condition ──
                check_condition_for_assignment(else_if.condition, ctx, results);
                // ── null narrowing for elseif-body ──
                narrowing::try_apply_if_body_null_narrowing(
                    else_if.condition,
                    else_if.statement.span(),
                    ctx,
                    results,
                );
                // ── instanceof narrowing for elseif-body ──
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_instanceof_narrowing(
                        else_if.condition,
                        else_if.statement.span(),
                        ctx,
                        classes,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_assert_condition_narrowing(
                        else_if.condition,
                        else_if.statement.span(),
                        ctx,
                        classes,
                        false,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_in_array_narrowing(
                        else_if.condition,
                        else_if.statement.span(),
                        ctx,
                        classes,
                    );
                });
                // ── type-guard narrowing for elseif-body ──
                narrowing::try_apply_type_guard_narrowing(
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
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        classes,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_assert_condition_narrowing(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        classes,
                        true, // inverted — this is the else-body
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_in_array_narrowing_inverse(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        classes,
                    );
                });
                // ── inverse null narrowing for else-body ──
                narrowing::try_apply_if_body_null_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                // ── inverse type-guard narrowing for else-body ──
                narrowing::try_apply_type_guard_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                // Also apply inverse narrowing for every elseif condition.
                // In the else branch, all preceding conditions were false,
                // so each elseif's condition is also inverted.
                for else_if in body.else_if_clauses.iter() {
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_instanceof_narrowing_inverse(
                            else_if.condition,
                            else_span,
                            ctx,
                            classes,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_assert_condition_narrowing(
                            else_if.condition,
                            else_span,
                            ctx,
                            classes,
                            true,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_in_array_narrowing_inverse(
                            else_if.condition,
                            else_span,
                            ctx,
                            classes,
                        );
                    });
                    narrowing::try_apply_type_guard_narrowing_inverse(
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
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_instanceof_narrowing(
                    if_stmt.condition,
                    then_span,
                    ctx,
                    classes,
                );
            });
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_assert_condition_narrowing(
                    if_stmt.condition,
                    then_span,
                    ctx,
                    classes,
                    false,
                );
            });
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_in_array_narrowing(if_stmt.condition, then_span, ctx, classes);
            });
            // ── null narrowing for then-body ──
            narrowing::try_apply_if_body_null_narrowing(if_stmt.condition, then_span, ctx, results);
            // ── type-guard narrowing for then-body ──
            narrowing::try_apply_type_guard_narrowing(if_stmt.condition, then_span, ctx, results);
            walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
            for else_if in body.else_if_clauses.iter() {
                // ── inline && narrowing for elseif condition ──
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_inline_and_narrowing(else_if.condition, ctx, classes);
                });
                narrowing::try_apply_inline_and_null_narrowing(else_if.condition, ctx, results);
                // ── Assignment in elseif condition ──
                check_condition_for_assignment(else_if.condition, ctx, results);
                // ── null narrowing for elseif (must be after condition assignment) ──
                // Note: we need the span for body narrowing, computed below,
                // but truthiness narrowing from the condition itself can use
                // the condition expression directly via try_extract_null_check
                // which doesn't need body_span (it's checked inside the fn).
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
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_instanceof_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        classes,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_assert_condition_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        classes,
                        false,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_in_array_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        classes,
                    );
                });
                narrowing::try_apply_if_body_null_narrowing(
                    else_if.condition,
                    ei_span,
                    ctx,
                    results,
                );
                // ── type-guard narrowing for elseif-body ──
                narrowing::try_apply_type_guard_narrowing(else_if.condition, ei_span, ctx, results);
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
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        classes,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_assert_condition_narrowing(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        classes,
                        true, // inverted — else-body
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_in_array_narrowing_inverse(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        classes,
                    );
                });
                // ── inverse null narrowing for else-body ──
                narrowing::try_apply_if_body_null_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                // ── inverse type-guard narrowing for else-body ──
                narrowing::try_apply_type_guard_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                // Also apply inverse narrowing for every elseif condition.
                for else_if in body.else_if_clauses.iter() {
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_instanceof_narrowing_inverse(
                            else_if.condition,
                            else_span,
                            ctx,
                            classes,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_assert_condition_narrowing(
                            else_if.condition,
                            else_span,
                            ctx,
                            classes,
                            true,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_in_array_narrowing_inverse(
                            else_if.condition,
                            else_span,
                            ctx,
                            classes,
                        );
                    });
                    narrowing::try_apply_type_guard_narrowing_inverse(
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
        ResolvedType::apply_narrowing(results, |classes| {
            narrowing::apply_guard_clause_narrowing(if_stmt, ctx, classes);
        });
        ResolvedType::apply_narrowing(results, |classes| {
            narrowing::apply_guard_clause_in_array_narrowing(if_stmt, ctx, classes);
        });
        // ── Null / falsy guard clause narrowing ──
        // `if (!$var) { return; }` or `if ($var === null) { continue; }`
        // → remove `null` from the resolved types after the guard.
        // This operates on `ResolvedType` directly because `null` is
        // not a class and would be missed by class-level narrowing.
        narrowing::apply_guard_clause_null_narrowing(if_stmt, ctx, results);
        // ── Type-guard guard clause narrowing ──
        // `if (is_array($x)) { return; }` → after if, $x is NOT array.
        narrowing::apply_guard_clause_type_guard_narrowing(if_stmt, ctx, results);
    }
}

/// Branch-aware if/else walking for hover.
///
/// When the cursor is inside a specific branch body, only that branch's
/// assignments are walked (with `conditional = false` since we know
/// we're in that branch).  The appropriate narrowing for the branch is
/// applied before walking.
///
/// Returns `Some(())` if the cursor was inside a branch body and the
/// branch was handled.  Returns `None` if the cursor is not inside any
/// branch (i.e. after the if/else), in which case the caller should
/// fall through to the normal union-all-branches logic.
fn walk_if_branch_aware<'b>(
    if_stmt: &'b If<'b>,
    _enclosing_stmt: &'b Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) -> Option<()> {
    match &if_stmt.body {
        IfBody::Statement(body) => {
            let then_span = body.statement.span();
            if ctx.cursor_offset >= then_span.start.offset
                && ctx.cursor_offset <= then_span.end.offset
            {
                // Cursor is inside the then-branch.
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_instanceof_narrowing(
                        if_stmt.condition,
                        then_span,
                        ctx,
                        classes,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_assert_condition_narrowing(
                        if_stmt.condition,
                        then_span,
                        ctx,
                        classes,
                        false,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_in_array_narrowing(
                        if_stmt.condition,
                        then_span,
                        ctx,
                        classes,
                    );
                });
                // ── Assignment in condition ──
                check_condition_for_assignment(if_stmt.condition, ctx, results);
                narrowing::try_apply_if_body_null_narrowing(
                    if_stmt.condition,
                    then_span,
                    ctx,
                    results,
                );
                // ── type-guard narrowing for then-body ──
                narrowing::try_apply_type_guard_narrowing(
                    if_stmt.condition,
                    then_span,
                    ctx,
                    results,
                );
                check_statement_for_assignments(body.statement, ctx, results, false);
                return Some(());
            }

            for else_if in body.else_if_clauses.iter() {
                let ei_span = else_if.statement.span();
                if ctx.cursor_offset >= ei_span.start.offset
                    && ctx.cursor_offset <= ei_span.end.offset
                {
                    // Cursor is inside this elseif-branch.
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_instanceof_narrowing(
                            else_if.condition,
                            ei_span,
                            ctx,
                            classes,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_assert_condition_narrowing(
                            else_if.condition,
                            ei_span,
                            ctx,
                            classes,
                            false,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_in_array_narrowing(
                            else_if.condition,
                            ei_span,
                            ctx,
                            classes,
                        );
                    });
                    narrowing::try_apply_if_body_null_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        results,
                    );
                    // ── type-guard narrowing for elseif-body ──
                    narrowing::try_apply_type_guard_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        results,
                    );
                    // ── Assignment in elseif condition ──
                    check_condition_for_assignment(else_if.condition, ctx, results);
                    narrowing::try_apply_if_body_null_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        results,
                    );
                    check_statement_for_assignments(else_if.statement, ctx, results, false);
                    return Some(());
                }
            }

            if let Some(else_clause) = &body.else_clause {
                let el_span = else_clause.statement.span();
                if ctx.cursor_offset >= el_span.start.offset
                    && ctx.cursor_offset <= el_span.end.offset
                {
                    // Cursor is inside the else-branch.
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_instanceof_narrowing_inverse(
                            if_stmt.condition,
                            el_span,
                            ctx,
                            classes,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_assert_condition_narrowing(
                            if_stmt.condition,
                            el_span,
                            ctx,
                            classes,
                            true,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_in_array_narrowing_inverse(
                            if_stmt.condition,
                            el_span,
                            ctx,
                            classes,
                        );
                    });
                    narrowing::try_apply_if_body_null_narrowing_inverse(
                        if_stmt.condition,
                        el_span,
                        ctx,
                        results,
                    );
                    // ── inverse type-guard narrowing for else-body ──
                    narrowing::try_apply_type_guard_narrowing_inverse(
                        if_stmt.condition,
                        el_span,
                        ctx,
                        results,
                    );
                    // Also apply inverse narrowing for every elseif condition.
                    for else_if in body.else_if_clauses.iter() {
                        ResolvedType::apply_narrowing(results, |classes| {
                            narrowing::try_apply_instanceof_narrowing_inverse(
                                else_if.condition,
                                el_span,
                                ctx,
                                classes,
                            );
                        });
                        ResolvedType::apply_narrowing(results, |classes| {
                            narrowing::try_apply_assert_condition_narrowing(
                                else_if.condition,
                                el_span,
                                ctx,
                                classes,
                                true,
                            );
                        });
                        ResolvedType::apply_narrowing(results, |classes| {
                            narrowing::try_apply_in_array_narrowing_inverse(
                                else_if.condition,
                                el_span,
                                ctx,
                                classes,
                            );
                        });
                        narrowing::try_apply_type_guard_narrowing_inverse(
                            else_if.condition,
                            el_span,
                            ctx,
                            results,
                        );
                    }
                    check_statement_for_assignments(else_clause.statement, ctx, results, false);
                    return Some(());
                }
            }
        }
        IfBody::ColonDelimited(body) => {
            // Approximate the span of each branch using colon/keyword
            // boundaries (same logic as the raw-type pipeline's
            // `accumulate_if_branch_at_cursor`).
            let then_start = body.colon.start.offset;
            let then_end = body
                .else_if_clauses
                .first()
                .map(|ei| ei.elseif.span().start.offset)
                .or_else(|| {
                    body.else_clause
                        .as_ref()
                        .map(|ec| ec.r#else.span().start.offset)
                })
                .unwrap_or(body.endif.span().start.offset);

            if ctx.cursor_offset >= then_start && ctx.cursor_offset < then_end {
                // Cursor is inside the then-branch.
                let then_span = mago_span::Span::new(
                    body.colon.file_id,
                    body.colon.start,
                    mago_span::Position::new(then_end),
                );
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_instanceof_narrowing(
                        if_stmt.condition,
                        then_span,
                        ctx,
                        classes,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_assert_condition_narrowing(
                        if_stmt.condition,
                        then_span,
                        ctx,
                        classes,
                        false,
                    );
                });
                ResolvedType::apply_narrowing(results, |classes| {
                    narrowing::try_apply_in_array_narrowing(
                        if_stmt.condition,
                        then_span,
                        ctx,
                        classes,
                    );
                });
                // ── Assignment in condition ──
                check_condition_for_assignment(if_stmt.condition, ctx, results);
                narrowing::try_apply_if_body_null_narrowing(
                    if_stmt.condition,
                    then_span,
                    ctx,
                    results,
                );
                // ── type-guard narrowing for then-body ──
                narrowing::try_apply_type_guard_narrowing(
                    if_stmt.condition,
                    then_span,
                    ctx,
                    results,
                );
                walk_statements_for_assignments(body.statements.iter(), ctx, results, false);
                return Some(());
            }

            for (i, else_if) in body.else_if_clauses.iter().enumerate() {
                let ei_start = else_if.colon.start.offset;
                let ei_end = body
                    .else_if_clauses
                    .get(i + 1)
                    .map(|next| next.elseif.span().start.offset)
                    .or_else(|| {
                        body.else_clause
                            .as_ref()
                            .map(|ec| ec.r#else.span().start.offset)
                    })
                    .unwrap_or(body.endif.span().start.offset);

                if ctx.cursor_offset >= ei_start && ctx.cursor_offset < ei_end {
                    // Cursor is inside this elseif-branch.
                    let ei_span = mago_span::Span::new(
                        else_if.colon.file_id,
                        else_if.colon.start,
                        mago_span::Position::new(ei_end),
                    );
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_instanceof_narrowing(
                            else_if.condition,
                            ei_span,
                            ctx,
                            classes,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_assert_condition_narrowing(
                            else_if.condition,
                            ei_span,
                            ctx,
                            classes,
                            false,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_in_array_narrowing(
                            else_if.condition,
                            ei_span,
                            ctx,
                            classes,
                        );
                    });
                    narrowing::try_apply_if_body_null_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        results,
                    );
                    // ── type-guard narrowing for elseif-body ──
                    narrowing::try_apply_type_guard_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        results,
                    );
                    // ── Assignment in elseif condition ──
                    check_condition_for_assignment(else_if.condition, ctx, results);
                    narrowing::try_apply_if_body_null_narrowing(
                        else_if.condition,
                        ei_span,
                        ctx,
                        results,
                    );
                    walk_statements_for_assignments(else_if.statements.iter(), ctx, results, false);
                    return Some(());
                }
            }

            if let Some(ref else_clause) = body.else_clause {
                let el_start = else_clause.colon.start.offset;
                let el_end = body.endif.span().start.offset;

                if ctx.cursor_offset >= el_start && ctx.cursor_offset < el_end {
                    // Cursor is inside the else-branch.
                    let else_span = mago_span::Span::new(
                        else_clause.colon.file_id,
                        else_clause.colon.start,
                        mago_span::Position::new(el_end),
                    );
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_instanceof_narrowing_inverse(
                            if_stmt.condition,
                            else_span,
                            ctx,
                            classes,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_assert_condition_narrowing(
                            if_stmt.condition,
                            else_span,
                            ctx,
                            classes,
                            true,
                        );
                    });
                    ResolvedType::apply_narrowing(results, |classes| {
                        narrowing::try_apply_in_array_narrowing_inverse(
                            if_stmt.condition,
                            else_span,
                            ctx,
                            classes,
                        );
                    });
                    narrowing::try_apply_if_body_null_narrowing_inverse(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        results,
                    );
                    // ── inverse type-guard narrowing for else-body ──
                    narrowing::try_apply_type_guard_narrowing_inverse(
                        if_stmt.condition,
                        else_span,
                        ctx,
                        results,
                    );
                    // Also apply inverse narrowing for every elseif condition.
                    for else_if in body.else_if_clauses.iter() {
                        ResolvedType::apply_narrowing(results, |classes| {
                            narrowing::try_apply_instanceof_narrowing_inverse(
                                else_if.condition,
                                else_span,
                                ctx,
                                classes,
                            );
                        });
                        ResolvedType::apply_narrowing(results, |classes| {
                            narrowing::try_apply_assert_condition_narrowing(
                                else_if.condition,
                                else_span,
                                ctx,
                                classes,
                                true,
                            );
                        });
                        ResolvedType::apply_narrowing(results, |classes| {
                            narrowing::try_apply_in_array_narrowing_inverse(
                                else_if.condition,
                                else_span,
                                ctx,
                                classes,
                            );
                        });
                        narrowing::try_apply_type_guard_narrowing_inverse(
                            else_if.condition,
                            else_span,
                            ctx,
                            results,
                        );
                    }
                    walk_statements_for_assignments(
                        else_clause.statements.iter(),
                        ctx,
                        results,
                        false,
                    );
                    return Some(());
                }
            }
        }
    }

    // Cursor is not inside any branch body — fall through to the
    // normal union-all-branches logic in `walk_if_statement`.
    None
}

/// Pre-scan a loop body for assignments to the target variable.
///
/// In a loop, an assignment that appears textually *after* the cursor
/// can still be live on subsequent iterations.  This helper walks the
/// loop body with `cursor_offset` set to `body_end` so that all
/// assignments are visible, and merges any discovered types into
/// `results` as conditional.  The caller should invoke this **before**
/// the normal positional walk so that narrowing (e.g. `!== null`)
/// applied during the normal walk can strip types contributed by the
/// pre-scan.
///
/// Only types that are not already present in `results` are added,
/// avoiding duplicates with assignments that the normal walk will
/// also discover.
fn prescan_loop_body_for_assignments<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    body_end: u32,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    // Only pre-scan when the cursor is actually inside the loop body.
    // The caller is responsible for checking this, but as a safety
    // measure we verify that the cursor is before the body end.
    if ctx.cursor_offset >= body_end {
        return;
    }

    let prescan_ctx = ctx.with_cursor_offset(body_end);
    let mut prescan_results: Vec<ResolvedType> = Vec::new();
    walk_statements_for_assignments(statements, &prescan_ctx, &mut prescan_results, true);

    // Merge pre-scanned types into results as conditional additions.
    ResolvedType::extend_unique(results, prescan_results);
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
    results: &mut Vec<ResolvedType>,
    conditional: bool,
) {
    let body_span = foreach.body.span();
    let header_start = foreach.foreach.span().start.offset;
    let cursor_inside =
        ctx.cursor_offset >= header_start && ctx.cursor_offset <= body_span.end.offset;

    if cursor_inside {
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
    }

    // ── Pre-scan for loop-carried assignments ──
    // When the cursor is inside the loop body, assignments that appear
    // textually after the cursor are live on subsequent iterations.
    // Pre-scan the body to pick them up as conditional types before the
    // normal positional walk (which skips statements after the cursor).
    if cursor_inside {
        match &foreach.body {
            ForeachBody::Statement(inner) => {
                prescan_loop_body_for_assignments(
                    std::iter::once(*inner),
                    body_span.end.offset,
                    ctx,
                    results,
                );
            }
            ForeachBody::ColonDelimited(body) => {
                prescan_loop_body_for_assignments(
                    body.statements.iter(),
                    body_span.end.offset,
                    ctx,
                    results,
                );
            }
        }
    }

    // Always walk the foreach body for variable assignments, even when
    // the cursor is after the foreach.  A foreach body may execute zero
    // or more times, so any assignment inside is conditional.
    //
    // Without this, `$x = null; foreach (...) { $x = new Foo(); }
    // $x->method();` would lose the `Foo` assignment because the body
    // was only walked when the cursor was inside the foreach.
    match &foreach.body {
        ForeachBody::Statement(inner) => {
            check_statement_for_assignments(inner, ctx, results, true);
        }
        ForeachBody::ColonDelimited(body) => {
            walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
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
    results: &mut Vec<ResolvedType>,
) {
    // ── Inline && narrowing inside the while condition ──
    ResolvedType::apply_narrowing(results, |classes| {
        narrowing::try_apply_inline_and_narrowing(while_stmt.condition, ctx, classes);
    });
    narrowing::try_apply_inline_and_null_narrowing(while_stmt.condition, ctx, results);

    // ── Assignment in condition ──
    // `while ($line = fgets($fp))` — register the assignment.
    check_condition_for_assignment(while_stmt.condition, ctx, results);

    // Determine whether the cursor is inside this while loop for the
    // pre-scan.  We check both body variants.
    let while_body_span = while_stmt.body.span();
    let while_header_start = while_stmt.r#while.span().start.offset;
    let cursor_inside_while =
        ctx.cursor_offset >= while_header_start && ctx.cursor_offset <= while_body_span.end.offset;

    // ── Pre-scan for loop-carried assignments ──
    if cursor_inside_while {
        match &while_stmt.body {
            WhileBody::Statement(inner) => {
                prescan_loop_body_for_assignments(
                    std::iter::once(*inner),
                    while_body_span.end.offset,
                    ctx,
                    results,
                );
            }
            WhileBody::ColonDelimited(body) => {
                prescan_loop_body_for_assignments(
                    body.statements.iter(),
                    while_body_span.end.offset,
                    ctx,
                    results,
                );
            }
        }
    }

    match &while_stmt.body {
        WhileBody::Statement(inner) => {
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_instanceof_narrowing(
                    while_stmt.condition,
                    inner.span(),
                    ctx,
                    classes,
                );
            });
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_assert_condition_narrowing(
                    while_stmt.condition,
                    inner.span(),
                    ctx,
                    classes,
                    false,
                );
            });
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_in_array_narrowing(
                    while_stmt.condition,
                    inner.span(),
                    ctx,
                    classes,
                );
            });
            // ── null narrowing for while-body ──
            // Runs after check_condition_for_assignment so `$row`
            // from `while ($row = nextRow())` is typed before we
            // strip null.
            narrowing::try_apply_if_body_null_narrowing(
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
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_instanceof_narrowing(
                    while_stmt.condition,
                    body_span,
                    ctx,
                    classes,
                );
            });
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_assert_condition_narrowing(
                    while_stmt.condition,
                    body_span,
                    ctx,
                    classes,
                    false,
                );
            });
            ResolvedType::apply_narrowing(results, |classes| {
                narrowing::try_apply_in_array_narrowing(
                    while_stmt.condition,
                    body_span,
                    ctx,
                    classes,
                );
            });
            // ── null narrowing for while-body ──
            // Must run after check_condition_for_assignment (called above
            // before the match) so the type is registered first.
            narrowing::try_apply_if_body_null_narrowing(
                while_stmt.condition,
                body_span,
                ctx,
                results,
            );
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
    results: &mut Vec<ResolvedType>,
) {
    // When the cursor is inside the try block body, assignments that
    // precede the cursor have executed sequentially — use
    // `conditional: false` so reassignments replace the previous type
    // instead of forming a union with it.  When the cursor is outside
    // (after the try statement), we don't know whether the try body
    // ran to completion, so keep `conditional: true`.
    let try_span = try_stmt.block.span();
    let cursor_in_try =
        ctx.cursor_offset >= try_span.start.offset && ctx.cursor_offset <= try_span.end.offset;
    walk_statements_for_assignments(
        try_stmt.block.statements.iter(),
        ctx,
        results,
        !cursor_in_try,
    );

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
            ResolvedType::extend_unique(results, ResolvedType::from_classes(resolved));
        }
        // Same logic: when the cursor is inside this catch block,
        // treat preceding assignments as unconditional.
        let catch_span = catch.block.span();
        let cursor_in_catch = ctx.cursor_offset >= catch_span.start.offset
            && ctx.cursor_offset <= catch_span.end.offset;
        walk_statements_for_assignments(
            catch.block.statements.iter(),
            ctx,
            results,
            !cursor_in_catch,
        );
    }
    if let Some(finally) = &try_stmt.finally_clause {
        let finally_span = finally.block.span();
        let cursor_in_finally = ctx.cursor_offset >= finally_span.start.offset
            && ctx.cursor_offset <= finally_span.end.offset;
        walk_statements_for_assignments(
            finally.block.statements.iter(),
            ctx,
            results,
            !cursor_in_finally,
        );
    }
}

/// Convenience wrapper that walks a single statement for assignments
/// to the target variable, delegating to `walk_statements_for_assignments`.
pub(in crate::completion) fn check_statement_for_assignments<'b>(
    stmt: &'b Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
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
    results: &mut Vec<ResolvedType>,
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

    // ── Skip when cursor is inside the RHS ─────────
    // When the cursor falls within the RHS of this assignment
    // (e.g. `/** @var array<string, mixed> */ $data = $data->toArray()`),
    // the `@var` cast should only apply *after* the assignment
    // completes.  The RHS `$data` still has its previous type.
    let rhs_start = assignment.rhs.span().start.offset;
    let assign_end = assignment.span().end.offset;
    if ctx.cursor_offset >= rhs_start && ctx.cursor_offset <= assign_end {
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

    let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
        &eff_type,
        &ctx.current_class.name,
        ctx.all_classes,
        ctx.class_loader,
    );

    if resolved.is_empty() {
        // When `type_hint_to_classes` can't resolve the type (e.g.
        // `list<User>`, `array{name: string}`, `int[]`), emit a
        // type-string-only entry so that downstream consumers like
        // foreach resolution can still extract element types via
        // `PhpType::extract_value_type`.  Skip non-informative types
        // (`array`, `mixed`, etc.) so normal resolution can provide
        // more precise information.
        if eff_type.is_informative() {
            let resolved_types = vec![ResolvedType::from_type_string(eff_type.clone())];
            if !conditional {
                results.clear();
            }
            ResolvedType::extend_unique(results, resolved_types);
            return true;
        }
        return false;
    }

    let resolved_types = ResolvedType::from_classes_with_hint(resolved, eff_type.clone());

    // Apply the resolved type(s) with the same conditional semantics
    // used by `check_expression_for_assignment`.
    if !conditional {
        results.clear();
    }
    ResolvedType::extend_unique(results, resolved_types);
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
                    ctx.function_loader()
                        .and_then(|fl| fl(&name))
                        .and_then(|fi| fi.return_type_str())
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
                                .and_then(|m| m.return_type_str())
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
                            .and_then(|m| m.return_type_str())
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

/// Extract and apply an assignment embedded in an `if` / `while`
/// condition expression.
///
/// Handles:
///   - `if ($x = expr())` — condition is a bare assignment
///   - `if (($x = expr()) !== null)` — assignment inside a comparison
///   - `if ($x = expr()) { … }` — truthiness check (truthy narrowing
///     is handled separately by the null-narrowing pass)
///
/// The assignment is always treated as conditional (the variable
/// *might* have this type), because the condition may be false.
/// The caller is responsible for applying truthiness narrowing
/// inside the then-body.
pub(in crate::completion) fn check_condition_for_assignment<'b>(
    condition: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    // Unwrap parentheses.
    let inner = match condition {
        Expression::Parenthesized(p) => p.expression,
        other => other,
    };

    // Direct assignment: `if ($x = expr())`
    if let Expression::Assignment(_) = inner {
        check_expression_for_assignment(inner, ctx, results, true);
        return;
    }

    // Assignment inside a comparison: `if (($x = expr()) !== null)`
    // or `if (null !== ($x = expr()))`.
    if let Expression::Binary(bin) = inner {
        if let Expression::Assignment(_) = bin.lhs {
            check_expression_for_assignment(bin.lhs, ctx, results, true);
            return;
        }
        if let Expression::Assignment(_) = bin.rhs {
            check_expression_for_assignment(bin.rhs, ctx, results, true);
            return;
        }
        // Unwrap one layer of parens on each side.
        if let Expression::Parenthesized(p) = bin.lhs
            && let Expression::Assignment(_) = p.expression
        {
            check_expression_for_assignment(p.expression, ctx, results, true);
            return;
        }
        if let Expression::Parenthesized(p) = bin.rhs
            && let Expression::Assignment(_) = p.expression
        {
            check_expression_for_assignment(p.expression, ctx, results, true);
        }
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
    results: &mut Vec<ResolvedType>,
    conditional: bool,
) {
    let var_name = ctx.var_name;

    /// Push one or more resolved types into `results`.
    ///
    /// * `conditional == false` → unconditional assignment: **clear**
    ///   previous candidates first, then add all new ones (handles
    ///   union return types like `A|B` from a single assignment).
    /// * `conditional == true` → conditional branch: **append**
    ///   without clearing (the variable *might* be these types).
    ///
    /// Duplicates (same type string) are always suppressed.
    fn push_results(results: &mut Vec<ResolvedType>, new: Vec<ResolvedType>, conditional: bool) {
        if new.is_empty() {
            return;
        }
        if !conditional {
            results.clear();
        }
        ResolvedType::extend_unique(results, new);
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

        // ── Incremental key assignment: `$var['key'] = expr;` ──
        // Track string-keyed assignments and merge them into the
        // base type's array shape.  This produces type strings like
        // `array{name: string, age: int}` which downstream consumers
        // use for shape-aware completion and hover.
        if let Expression::ArrayAccess(array_access) = assignment.lhs
            && let Expression::Variable(Variable::Direct(dv)) = array_access.array
            && dv.name == var_name
        {
            // ── Skip when cursor is inside the RHS ────────
            // Same guard as the base-assignment path below.
            // Without this, `$var['key'] = $var['key']->method()`
            // would infinitely recurse: resolving the RHS triggers
            // resolution of `$var`, which re-discovers the same
            // assignment and resolves its RHS again.
            let rhs_start = assignment.rhs.span().start.offset;
            let assign_end = assignment.span().end.offset;
            if ctx.cursor_offset >= rhs_start && ctx.cursor_offset <= assign_end {
                return;
            }

            let key = extract_array_key_for_shape(array_access.index);
            if let Some(key) = key {
                // ── String-literal key: merge into array shape ──
                let rhs_ctx = ctx.with_cursor_offset(assignment.span().start.offset);
                let resolved =
                    super::rhs_resolution::resolve_rhs_expression(assignment.rhs, &rhs_ctx);
                let value_type = if !resolved.is_empty() {
                    ResolvedType::types_joined(&resolved).to_string()
                } else {
                    "mixed".to_string()
                };
                // Read the current base type from results (if any)
                // and merge the new key into its shape.
                let base = results
                    .last()
                    .map(|rt| &rt.type_string)
                    .cloned()
                    .unwrap_or_else(|| PhpType::Named("array".to_string()));
                let value_php_type = PhpType::parse(&value_type);
                let merged = merge_shape_key(&base, &key, &value_php_type);
                // Replace results with the enriched shape type.
                // Use extend_unique so this works in conditional
                // branches (appends) as well as unconditional
                // (results cleared first by push_results via the
                // base assignment that preceded this).
                // We always push here without clearing — shape keys
                // accumulate on top of the existing base.
                let new_rt = ResolvedType::from_type_string(merged);
                results.clear();
                results.push(new_rt);
            } else {
                // ── Non-literal key (variable, numeric, expression) ──
                // Track the RHS type as the array's value type so that
                // subsequent `foreach` iteration and bracket access
                // resolve element members.  This handles patterns like
                // `$arr[$id] = $orderLine` inside a loop.
                let rhs_ctx = ctx.with_cursor_offset(assignment.span().start.offset);
                let resolved =
                    super::rhs_resolution::resolve_rhs_expression(assignment.rhs, &rhs_ctx);
                let value_type = if !resolved.is_empty() {
                    ResolvedType::types_joined(&resolved).to_string()
                } else {
                    "mixed".to_string()
                };
                let base_type = results
                    .last()
                    .map(|rt| &rt.type_string)
                    .cloned()
                    .unwrap_or_else(|| PhpType::Named("array".to_string()));
                // When the base already has a shape type from prior
                // string-keyed assignments, do not overwrite it with
                // a generic element type — shapes take precedence.
                if base_type.is_array_shape() {
                    return;
                }
                // Infer the key type from the index expression.
                let key_type_str = infer_array_key_type(array_access.index, &rhs_ctx);
                let key_php_type = PhpType::parse(&key_type_str);
                let value_php_type = PhpType::parse(&value_type);
                let merged = merge_keyed_type(&base_type, &key_php_type, &value_php_type);
                let new_rt = ResolvedType::from_type_string(merged);
                results.clear();
                results.push(new_rt);
            }
            return;
        }

        // ── Push assignment: `$var[] = expr;` ──
        // Track push-style assignments and merge them into the base
        // type's list element type.  This produces type strings like
        // `list<User>` or `list<User|Admin>`.
        if let Expression::ArrayAppend(array_append) = assignment.lhs
            && let Expression::Variable(Variable::Direct(dv)) = array_append.array
            && dv.name == var_name
        {
            // ── Skip when cursor is inside the RHS ────────
            let rhs_start = assignment.rhs.span().start.offset;
            let assign_end = assignment.span().end.offset;
            if ctx.cursor_offset >= rhs_start && ctx.cursor_offset <= assign_end {
                return;
            }

            let rhs_ctx = ctx.with_cursor_offset(assignment.span().start.offset);
            let resolved = super::rhs_resolution::resolve_rhs_expression(assignment.rhs, &rhs_ctx);
            let value_type = if !resolved.is_empty() {
                ResolvedType::types_joined(&resolved).to_string()
            } else {
                "mixed".to_string()
            };
            // Read the current base type from results (if any)
            // and merge the push element type into it.
            //
            // When the base is already an array shape (from prior
            // `$var['key'] = expr` assignments), skip the push merge.
            // String-keyed entries take precedence over positional
            // pushes, matching the old AssignmentAccumulator's
            // finalize() behaviour.
            let base_type = results
                .last()
                .map(|rt| &rt.type_string)
                .cloned()
                .unwrap_or_else(|| PhpType::Named("array".to_string()));
            if base_type.is_array_shape() {
                return;
            }
            let value_php_type = PhpType::parse(&value_type);
            let merged = merge_push_type(&base_type, &value_php_type);
            let new_rt = ResolvedType::from_type_string(merged);
            results.clear();
            results.push(new_rt);
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

        // ── Skip when cursor is inside the RHS ────────────
        // When the cursor falls within the RHS of this assignment
        // (e.g. `$request = new Bar(arg: $request->…)`), the
        // variable reference on the RHS still sees the *previous*
        // definition — PHP evaluates all RHS arguments before
        // performing the assignment.  Do not apply this assignment.
        let rhs_start = assignment.rhs.span().start.offset;
        let assign_end = assignment.span().end.offset;
        if ctx.cursor_offset >= rhs_start && ctx.cursor_offset <= assign_end {
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

// ── Shape mutation helpers ───────────────────────────────────────────

/// Extract a string key from an array access index expression.
///
/// Returns `Some(key)` for string-literal keys like `'name'` or `"age"`.
/// Returns `None` for numeric keys, variable indices, and other
/// non-string-literal expressions — these are not tracked as shape
/// entries.
fn extract_array_key_for_shape(index: &Expression<'_>) -> Option<String> {
    if let Expression::Literal(Literal::String(s)) = index {
        let key = s.value.map(|v| v.to_string()).unwrap_or_else(|| {
            crate::util::unquote_php_string(s.raw)
                .unwrap_or(s.raw)
                .to_string()
        });
        // Skip numeric-only keys — they are positional, not shape entries.
        if key.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(key)
    } else {
        None
    }
}

/// Merge a `(key, value_type)` pair into an existing `PhpType` to
/// produce an `ArrayShape`.
///
/// If `base` is already an `ArrayShape`, the key is added or updated.
/// Otherwise a new shape is created with just the given key.
///
/// Returns `PhpType::ArrayShape(entries)` with the merged entries.
fn merge_shape_key(base: &PhpType, key: &str, value_type: &PhpType) -> PhpType {
    let mut entries: Vec<ShapeEntry> = Vec::new();

    // Copy existing shape entries from the base type, skipping the
    // key we are about to upsert.
    if let Some(shape_entries) = base.shape_entries() {
        for entry in shape_entries {
            if entry.key.as_deref() != Some(key) {
                entries.push(entry.clone());
            }
        }
    }

    // Add/upsert the new key.
    entries.push(ShapeEntry {
        key: Some(key.to_string()),
        value_type: value_type.clone(),
        optional: false,
    });

    PhpType::ArrayShape(entries)
}

/// Merge a push element type into an existing `PhpType` to produce
/// a `Generic("list", …)` type.
///
/// If `base` already has a generic value type (e.g. `list<User>`),
/// the new type is unioned with it (e.g. `list<User|Admin>`).
/// Otherwise, produces `list<value_type>`.
///
/// Returns `PhpType::Generic("list", vec![elem_type])` or
/// `PhpType::Named("array")` when no element types are available.
fn merge_push_type(base: &PhpType, value_type: &PhpType) -> PhpType {
    let mut elem_types: Vec<PhpType> = Vec::new();

    // Extract existing element types from the base.
    if let Some(existing_elem) = base.extract_element_type() {
        for member in existing_elem.union_members() {
            if !matches!(member, PhpType::Named(n) if n.is_empty()) {
                elem_types.push(member.clone());
            }
        }
    }

    // Add new value type members (union-aware).
    for member in value_type.union_members() {
        if !matches!(member, PhpType::Named(n) if n.is_empty())
            && !elem_types.iter().any(|e| e.equivalent(member))
        {
            elem_types.push(member.clone());
        }
    }

    if elem_types.is_empty() {
        return PhpType::Named("array".to_string());
    }

    let elem_type = if elem_types.len() == 1 {
        elem_types.into_iter().next().unwrap()
    } else {
        PhpType::Union(elem_types)
    };

    PhpType::Generic("list".to_string(), vec![elem_type])
}

/// Merge a keyed element type into an existing `PhpType` to produce
/// a `Generic("array", …)` type.
///
/// Similar to [`merge_push_type`] but preserves the key type from the
/// index expression instead of assuming sequential integer keys.
///
/// When the base already has a generic value type (e.g.
/// `array<string, User>`), the new value type is unioned with it and
/// key types are unioned as well.
///
/// Returns `PhpType::Generic("array", vec![key, val])`,
/// `PhpType::Generic("array", vec![val])` when no key types are
/// available, or `PhpType::Named("array")` when no element types
/// are available.
fn merge_keyed_type(base: &PhpType, key_type: &PhpType, value_type: &PhpType) -> PhpType {
    // Collect existing key types from the base.
    let mut key_types: Vec<PhpType> = Vec::new();
    if let Some(existing_key) = base.extract_key_type(false)
        && !matches!(existing_key, PhpType::Named(n) if n.is_empty())
    {
        key_types.push(existing_key.clone());
    }
    // Add new key type members.
    for member in key_type.union_members() {
        if !matches!(member, PhpType::Named(n) if n.is_empty())
            && !key_types.iter().any(|e| e.equivalent(member))
        {
            key_types.push(member.clone());
        }
    }

    // Collect existing value types from the base.
    let mut elem_types: Vec<PhpType> = Vec::new();
    if let Some(existing_elem) = base.extract_element_type() {
        for member in existing_elem.union_members() {
            if !matches!(member, PhpType::Named(n) if n.is_empty()) {
                elem_types.push(member.clone());
            }
        }
    }
    // Add new value type members.
    for member in value_type.union_members() {
        if !matches!(member, PhpType::Named(n) if n.is_empty())
            && !elem_types.iter().any(|e| e.equivalent(member))
        {
            elem_types.push(member.clone());
        }
    }

    if elem_types.is_empty() {
        return PhpType::Named("array".to_string());
    }

    let val_type = if elem_types.len() == 1 {
        elem_types.into_iter().next().unwrap()
    } else {
        PhpType::Union(elem_types)
    };

    if key_types.is_empty() {
        // No key type information — use a single-param generic.
        PhpType::Generic("array".to_string(), vec![val_type])
    } else {
        let k_type = if key_types.len() == 1 {
            key_types.into_iter().next().unwrap()
        } else {
            PhpType::Union(key_types)
        };
        PhpType::Generic("array".to_string(), vec![k_type, val_type])
    }
}

/// Infer the key type of an array-access index expression.
///
/// Returns `"string"` for expressions that are known to produce
/// strings (string literals, method calls returning `string`, string
/// variables), `"int"` for integer expressions, and `"int|string"`
/// when the type cannot be determined.
fn infer_array_key_type(index: &Expression<'_>, ctx: &VarResolutionCtx<'_>) -> String {
    // Fast path: literal values.
    match index {
        Expression::Literal(Literal::Integer(_)) => return "int".to_string(),
        Expression::Literal(Literal::String(_)) => return "string".to_string(),
        _ => {}
    }

    // Resolve the expression type through the standard pipeline.
    let resolved = super::rhs_resolution::resolve_rhs_expression(index, ctx);
    if !resolved.is_empty() {
        let joined = ResolvedType::types_joined(&resolved);
        let key_str = joined.to_string();
        // Normalise the resolved type to a valid array key type.
        // PHP array keys are always int or string; bool and null are
        // coerced to int, float is truncated to int.
        if is_int_like_key(&key_str) {
            return "int".to_string();
        }
        if key_str == "string" || key_str == "non-empty-string" || key_str == "class-string" {
            return "string".to_string();
        }
        if key_str == "int|string" || key_str == "string|int" || key_str == "array-key" {
            return "int|string".to_string();
        }
        // If the resolved type is a known scalar that PHP coerces
        // to a key type, map it.  Otherwise fall through to the
        // default.
        if key_str == "mixed" {
            return "int|string".to_string();
        }
        // For anything else (e.g. a class-string<T>, or a union),
        // return as-is if it is composed entirely of int/string
        // subtypes; otherwise fall back.
        return key_str;
    }

    "int|string".to_string()
}

/// Returns `true` when the type string represents a PHP type that
/// is always coerced to `int` when used as an array key.
fn is_int_like_key(ty: &str) -> bool {
    matches!(
        ty,
        "int"
            | "float"
            | "bool"
            | "true"
            | "false"
            | "null"
            | "positive-int"
            | "negative-int"
            | "non-negative-int"
            | "non-positive-int"
            | "non-zero-int"
    )
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
/// `resolve_expression_type_string` for method calls, property access,
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
        let resolved = resolve_variable_types(
            &var_text,
            ctx.current_class,
            ctx.all_classes,
            ctx.content,
            offset as u32,
            ctx.class_loader,
            Loaders::with_function(ctx.function_loader()),
        );
        if !resolved.is_empty() {
            let joined = crate::types::ResolvedType::types_joined(&resolved);
            if joined.extract_value_type(true).is_some() {
                return Some(joined.to_string());
            }
        }
    }
    // Fall back to the unified pipeline (method calls, etc.)
    super::foreach_resolution::resolve_expression_type_string(arg_expr, ctx)
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
    results: &mut Vec<ResolvedType>,
    conditional: bool,
) {
    let (argument_list, parameters) = match expr {
        Expression::Call(Call::Function(func_call)) => {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => ident.value().to_string(),
                _ => return,
            };
            let fl = match ctx.function_loader() {
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
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                type_hint,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
            if !resolved.is_empty() {
                if !conditional {
                    results.clear();
                }
                ResolvedType::extend_unique(results, ResolvedType::from_classes(resolved));
            }
        }
    }
}

#[cfg(test)]
#[path = "resolution_tests.rs"]
mod tests;
