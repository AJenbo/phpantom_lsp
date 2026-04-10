/// Closure and arrow-function variable resolution.
///
/// When the cursor is inside a **closure** (`function (Type $p) { … }`),
/// variables are resolved from the closure's own parameter list rather
/// than the enclosing scope (closures have isolated scope in PHP).
///
/// **Arrow functions** (`fn(Type $p) => …`) automatically capture the
/// enclosing scope.  If the target variable matches an arrow function
/// parameter, it is resolved from that parameter list.  Otherwise the
/// walker returns `false` so the enclosing scope continues to resolve
/// the variable from prior assignments, just as PHP semantics require.
///
/// This module contains the recursive AST walkers that detect whether
/// the cursor falls inside such a construct and handle resolution
/// accordingly.
///
/// ## Callable parameter inference
///
/// When a closure or arrow function is passed as an argument to a method
/// or function call, and its parameters have no explicit type hints, the
/// resolver attempts to infer the parameter types from the called
/// method/function's signature.  For example, in
/// `$users->map(fn($u) => $u->name)`, the resolver looks up the `map`
/// method on the resolved type of `$users`, finds that its parameter is
/// typed as `callable(TValue): mixed` (with `TValue` already substituted
/// through generic resolution), and infers `$u` as the concrete element
/// type.
use std::cell::Cell;
use std::sync::Arc;

use mago_span::HasSpan;
use mago_syntax::ast::sequence::TokenSeparatedSequence;
use mago_syntax::ast::*;

use crate::completion::resolver::ResolutionCtx;
use crate::php_type::PhpType;
use crate::virtual_members::laravel::{
    ELOQUENT_BUILDER_FQN, RELATION_QUERY_METHODS, extends_eloquent_model, resolve_relation_chain,
};

/// Maximum recursion depth for callable parameter inference.
///
/// When a closure parameter is untyped, the resolver infers its type
/// from the enclosing method's signature by resolving the receiver
/// object.  If that receiver text itself contains the same variable
/// (e.g. nested closures reusing `$q`), the resolution re-enters this
/// path, creating an infinite cycle.  This cap breaks the cycle.
const MAX_CLOSURE_INFER_DEPTH: u32 = 4;

thread_local! {
    /// Tracks the current recursion depth of callable parameter
    /// inference to prevent stack overflow from nested closures.
    static CLOSURE_INFER_DEPTH: Cell<u32> = const { Cell::new(0) };

    /// Re-entrancy guard for [`find_closure_this_override`].
    ///
    /// The override check re-parses the program and resolves the
    /// receiver of the enclosing call expression.  If the receiver
    /// is `$this`, that triggers `resolve_target_classes_expr` →
    /// `SubjectExpr::This` → `find_closure_this_override` again,
    /// creating an infinite cycle.  This flag breaks the cycle by
    /// returning `None` on re-entry.
    static IN_CLOSURE_THIS_OVERRIDE: Cell<bool> = const { Cell::new(false) };
}

use crate::parser::extract_hint_type;
use crate::parser::with_parsed_program;
use crate::types::{AccessKind, ClassInfo, FunctionInfo, MethodInfo, ResolvedType};

use crate::completion::resolver::VarResolutionCtx;

// ─── @param-closure-this resolution ─────────────────────────────────────────

/// Check whether the cursor is inside a closure that is passed as an
/// argument to a function/method whose parameter carries a
/// `@param-closure-this` annotation.  If so, resolve the declared type
/// and return it as a `ClassInfo`.
///
/// This is the static-analysis equivalent of `Closure::bindTo()`:
/// frameworks like Laravel rebind closures so that `$this` inside the
/// closure body refers to a different object.  The
/// `@param-closure-this` PHPDoc tag declares what `$this` should
/// resolve to.
pub(crate) fn find_closure_this_override(ctx: &ResolutionCtx<'_>) -> Option<ClassInfo> {
    // Re-entrancy guard: when resolving the receiver of the enclosing
    // call (e.g. `$this->group(…)`), `resolve_target_classes` will hit
    // `SubjectExpr::This` and call us again.  Return `None` on the
    // second entry so the normal `current_class` fallback is used for
    // the receiver, avoiding infinite recursion.
    let already_inside = IN_CLOSURE_THIS_OVERRIDE.with(|f| f.get());
    if already_inside {
        return None;
    }
    IN_CLOSURE_THIS_OVERRIDE.with(|f| f.set(true));

    let result = with_parsed_program(ctx.content, "find_closure_this_override", |program, _| {
        for stmt in program.statements.iter() {
            if let Some(result) = walk_stmt_for_closure_this(stmt, ctx) {
                return Some(result);
            }
        }
        None
    });

    IN_CLOSURE_THIS_OVERRIDE.with(|f| f.set(false));
    result
}

/// Recursively walk a statement looking for a closure argument that
/// contains the cursor and whose receiving parameter has
/// `closure_this_type`.
fn walk_stmt_for_closure_this(stmt: &Statement<'_>, ctx: &ResolutionCtx<'_>) -> Option<ClassInfo> {
    let sp = stmt.span();
    if ctx.cursor_offset < sp.start.offset || ctx.cursor_offset > sp.end.offset {
        return None;
    }

    match stmt {
        Statement::Class(class) => {
            let start = class.left_brace.start.offset;
            let end = class.right_brace.end.offset;
            if ctx.cursor_offset < start || ctx.cursor_offset > end {
                return None;
            }
            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member
                    && let MethodBody::Concrete(body) = &method.body
                {
                    let bsp = body.span();
                    if ctx.cursor_offset >= bsp.start.offset && ctx.cursor_offset <= bsp.end.offset
                    {
                        for inner in body.statements.iter() {
                            if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                                return Some(r);
                            }
                        }
                    }
                }
            }
            None
        }
        Statement::Expression(expr_stmt) => walk_expr_for_closure_this(expr_stmt.expression, ctx),
        Statement::Return(ret) => ret
            .value
            .as_ref()
            .and_then(|v| walk_expr_for_closure_this(v, ctx)),
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                    return Some(r);
                }
            }
            None
        }
        Statement::If(if_stmt) => match &if_stmt.body {
            IfBody::Statement(body) => walk_stmt_for_closure_this(body.statement, ctx),
            IfBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                        return Some(r);
                    }
                }
                None
            }
        },
        Statement::Foreach(foreach) => match &foreach.body {
            ForeachBody::Statement(inner) => walk_stmt_for_closure_this(inner, ctx),
            ForeachBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                        return Some(r);
                    }
                }
                None
            }
        },
        Statement::While(while_stmt) => match &while_stmt.body {
            WhileBody::Statement(inner) => walk_stmt_for_closure_this(inner, ctx),
            WhileBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                        return Some(r);
                    }
                }
                None
            }
        },
        Statement::For(for_stmt) => match &for_stmt.body {
            ForBody::Statement(inner) => walk_stmt_for_closure_this(inner, ctx),
            ForBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                        return Some(r);
                    }
                }
                None
            }
        },
        Statement::DoWhile(dw) => walk_stmt_for_closure_this(dw.statement, ctx),
        Statement::Namespace(ns) => {
            for inner in ns.statements().iter() {
                if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                    return Some(r);
                }
            }
            None
        }
        Statement::Try(try_stmt) => {
            for inner in try_stmt.block.statements.iter() {
                if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                    return Some(r);
                }
            }
            for catch in try_stmt.catch_clauses.iter() {
                for inner in catch.block.statements.iter() {
                    if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                        return Some(r);
                    }
                }
            }
            if let Some(finally) = &try_stmt.finally_clause {
                for inner in finally.block.statements.iter() {
                    if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                        return Some(r);
                    }
                }
            }
            None
        }
        Statement::Function(func) => {
            let bsp = func.body.span();
            if ctx.cursor_offset >= bsp.start.offset && ctx.cursor_offset <= bsp.end.offset {
                for inner in func.body.statements.iter() {
                    if let Some(r) = walk_stmt_for_closure_this(inner, ctx) {
                        return Some(r);
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Walk an expression looking for a call whose closure argument
/// contains the cursor and whose parameter has `closure_this_type`.
fn walk_expr_for_closure_this(expr: &Expression<'_>, ctx: &ResolutionCtx<'_>) -> Option<ClassInfo> {
    let sp = expr.span();
    if ctx.cursor_offset < sp.start.offset || ctx.cursor_offset > sp.end.offset {
        return None;
    }

    match expr {
        Expression::Call(call) => walk_call_for_closure_this(call, ctx),
        Expression::Parenthesized(p) => walk_expr_for_closure_this(p.expression, ctx),
        Expression::Assignment(a) => walk_expr_for_closure_this(a.lhs, ctx)
            .or_else(|| walk_expr_for_closure_this(a.rhs, ctx)),
        Expression::Binary(bin) => walk_expr_for_closure_this(bin.lhs, ctx)
            .or_else(|| walk_expr_for_closure_this(bin.rhs, ctx)),
        Expression::Conditional(cond) => walk_expr_for_closure_this(cond.condition, ctx)
            .or_else(|| cond.then.and_then(|e| walk_expr_for_closure_this(e, ctx)))
            .or_else(|| walk_expr_for_closure_this(cond.r#else, ctx)),
        Expression::Array(arr) => {
            for elem in arr.elements.iter() {
                let found = match elem {
                    ArrayElement::KeyValue(kv) => walk_expr_for_closure_this(kv.key, ctx)
                        .or_else(|| walk_expr_for_closure_this(kv.value, ctx)),
                    ArrayElement::Value(v) => walk_expr_for_closure_this(v.value, ctx),
                    ArrayElement::Variadic(v) => walk_expr_for_closure_this(v.value, ctx),
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
                    ArrayElement::KeyValue(kv) => walk_expr_for_closure_this(kv.key, ctx)
                        .or_else(|| walk_expr_for_closure_this(kv.value, ctx)),
                    ArrayElement::Value(v) => walk_expr_for_closure_this(v.value, ctx),
                    ArrayElement::Variadic(v) => walk_expr_for_closure_this(v.value, ctx),
                    _ => None,
                };
                if found.is_some() {
                    return found;
                }
            }
            None
        }
        Expression::Match(m) => {
            if let Some(r) = walk_expr_for_closure_this(m.expression, ctx) {
                return Some(r);
            }
            for arm in m.arms.iter() {
                if let Some(r) = walk_expr_for_closure_this(arm.expression(), ctx) {
                    return Some(r);
                }
            }
            None
        }
        Expression::Access(access) => match access {
            Access::Property(pa) => walk_expr_for_closure_this(pa.object, ctx),
            Access::NullSafeProperty(pa) => walk_expr_for_closure_this(pa.object, ctx),
            Access::StaticProperty(pa) => walk_expr_for_closure_this(pa.class, ctx),
            Access::ClassConstant(pa) => walk_expr_for_closure_this(pa.class, ctx),
        },
        Expression::Instantiation(inst) => {
            if let Some(ref args) = inst.argument_list {
                walk_args_for_closure_this(&args.arguments, ctx, &|_| None)
            } else {
                None
            }
        }
        Expression::UnaryPrefix(u) => walk_expr_for_closure_this(u.operand, ctx),
        Expression::UnaryPostfix(u) => walk_expr_for_closure_this(u.operand, ctx),
        Expression::Yield(y) => match y {
            Yield::Value(yv) => yv
                .value
                .as_ref()
                .and_then(|v| walk_expr_for_closure_this(v, ctx)),
            Yield::Pair(yp) => walk_expr_for_closure_this(yp.key, ctx)
                .or_else(|| walk_expr_for_closure_this(yp.value, ctx)),
            Yield::From(yf) => walk_expr_for_closure_this(yf.iterator, ctx),
        },
        Expression::Throw(t) => walk_expr_for_closure_this(t.exception, ctx),
        Expression::Clone(c) => walk_expr_for_closure_this(c.object, ctx),
        Expression::Pipe(p) => walk_expr_for_closure_this(p.input, ctx)
            .or_else(|| walk_expr_for_closure_this(p.callable, ctx)),
        // Closures/arrow-functions that are NOT inside a call argument
        // are handled by the caller; we don't descend into their bodies
        // here because there is no call context to check.
        _ => None,
    }
}

/// Walk a call expression, checking each closure/arrow-function argument
/// to see if the cursor is inside it and the target parameter has
/// `closure_this_type`.
fn walk_call_for_closure_this(call: &Call<'_>, ctx: &ResolutionCtx<'_>) -> Option<ClassInfo> {
    match call {
        Call::Function(fc) => {
            let func_name = match fc.function {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                _ => None,
            };
            let result = walk_args_for_closure_this(&fc.argument_list.arguments, ctx, &|arg_idx| {
                let name = func_name.as_deref()?;
                let fi = ctx.function_loader.and_then(|fl| fl(name))?;
                closure_this_from_function_params(&fi, arg_idx, ctx)
            });
            if result.is_some() {
                return result;
            }
            // Recurse into arguments that are not closures (e.g. nested calls).
            for arg in fc.argument_list.arguments.iter() {
                let arg_expr = arg.value();
                if !is_closure_like(arg_expr)
                    && let Some(r) = walk_expr_for_closure_this(arg_expr, ctx)
                {
                    return Some(r);
                }
            }
            None
        }
        Call::Method(mc) => {
            if let Some(r) = walk_expr_for_closure_this(mc.object, ctx) {
                return Some(r);
            }
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let method_name = ident.value.to_string();
                let obj_span = mc.object.span();
                let result =
                    walk_args_for_closure_this(&mc.argument_list.arguments, ctx, &|arg_idx| {
                        closure_this_from_receiver(
                            obj_span.start.offset,
                            obj_span.end.offset,
                            &method_name,
                            arg_idx,
                            ctx,
                        )
                    });
                if result.is_some() {
                    return result;
                }
            }
            for arg in mc.argument_list.arguments.iter() {
                let arg_expr = arg.value();
                if !is_closure_like(arg_expr)
                    && let Some(r) = walk_expr_for_closure_this(arg_expr, ctx)
                {
                    return Some(r);
                }
            }
            None
        }
        Call::NullSafeMethod(mc) => {
            if let Some(r) = walk_expr_for_closure_this(mc.object, ctx) {
                return Some(r);
            }
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let method_name = ident.value.to_string();
                let obj_span = mc.object.span();
                let result =
                    walk_args_for_closure_this(&mc.argument_list.arguments, ctx, &|arg_idx| {
                        closure_this_from_receiver(
                            obj_span.start.offset,
                            obj_span.end.offset,
                            &method_name,
                            arg_idx,
                            ctx,
                        )
                    });
                if result.is_some() {
                    return result;
                }
            }
            for arg in mc.argument_list.arguments.iter() {
                let arg_expr = arg.value();
                if !is_closure_like(arg_expr)
                    && let Some(r) = walk_expr_for_closure_this(arg_expr, ctx)
                {
                    return Some(r);
                }
            }
            None
        }
        Call::StaticMethod(sc) => {
            if let Some(r) = walk_expr_for_closure_this(sc.class, ctx) {
                return Some(r);
            }
            if let ClassLikeMemberSelector::Identifier(ident) = &sc.method {
                let method_name = ident.value.to_string();
                let result =
                    walk_args_for_closure_this(&sc.argument_list.arguments, ctx, &|arg_idx| {
                        closure_this_from_static_receiver(sc.class, &method_name, arg_idx, ctx)
                    });
                if result.is_some() {
                    return result;
                }
            }
            for arg in sc.argument_list.arguments.iter() {
                let arg_expr = arg.value();
                if !is_closure_like(arg_expr)
                    && let Some(r) = walk_expr_for_closure_this(arg_expr, ctx)
                {
                    return Some(r);
                }
            }
            None
        }
    }
}

/// Check whether an expression is a closure or arrow function.
fn is_closure_like(expr: &Expression<'_>) -> bool {
    matches!(expr, Expression::Closure(_) | Expression::ArrowFunction(_))
}

/// Walk call arguments.  For each closure/arrow-function argument whose
/// body contains the cursor, call `lookup_fn(arg_idx)` to check whether
/// the target parameter has `closure_this_type`.
fn walk_args_for_closure_this<F>(
    arguments: &TokenSeparatedSequence<'_, Argument<'_>>,
    ctx: &ResolutionCtx<'_>,
    lookup_fn: &F,
) -> Option<ClassInfo>
where
    F: Fn(usize) -> Option<ClassInfo>,
{
    for (arg_idx, arg) in arguments.iter().enumerate() {
        let arg_expr = arg.value();
        let arg_span = arg_expr.span();
        if ctx.cursor_offset < arg_span.start.offset || ctx.cursor_offset > arg_span.end.offset {
            continue;
        }

        let cursor_inside_body = match arg_expr {
            Expression::Closure(closure) => {
                let body_start = closure.body.left_brace.start.offset;
                let body_end = closure.body.right_brace.end.offset;
                ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end
            }
            Expression::ArrowFunction(arrow) => {
                let arrow_body_span = arrow.expression.span();
                ctx.cursor_offset >= arrow.arrow.start.offset
                    && ctx.cursor_offset <= arrow_body_span.end.offset
            }
            _ => false,
        };

        if cursor_inside_body {
            return lookup_fn(arg_idx);
        }
    }
    None
}

/// Look up `closure_this_type` on a standalone function's parameter at
/// `arg_idx`.
fn closure_this_from_function_params(
    fi: &FunctionInfo,
    arg_idx: usize,
    ctx: &ResolutionCtx<'_>,
) -> Option<ClassInfo> {
    let param = fi.parameters.get(arg_idx)?;
    let php_type = param.closure_this_type.as_ref()?;
    resolve_closure_this_type(php_type, None, ctx)
}

/// Look up `closure_this_type` on an instance method's parameter at
/// `arg_idx`, resolving the receiver from the source span.
fn closure_this_from_receiver(
    obj_start: u32,
    obj_end: u32,
    method_name: &str,
    arg_idx: usize,
    ctx: &ResolutionCtx<'_>,
) -> Option<ClassInfo> {
    let start = obj_start as usize;
    let end = obj_end as usize;
    if end > ctx.content.len() {
        return None;
    }
    let obj_text = ctx.content[start..end].trim();
    let receiver_classes = ResolvedType::into_arced_classes(
        crate::completion::resolver::resolve_target_classes(obj_text, AccessKind::Arrow, ctx),
    );
    for cls in &receiver_classes {
        let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
            cls,
            ctx.class_loader,
            ctx.resolved_class_cache,
        );
        if let Some(method) = resolved.methods.iter().find(|m| m.name == method_name)
            && let Some(result) =
                closure_this_from_method_params(method, arg_idx, Some(&resolved), ctx)
        {
            return Some(result);
        }
    }
    None
}

/// Look up `closure_this_type` on a static method's parameter at
/// `arg_idx`.
fn closure_this_from_static_receiver(
    class_expr: &Expression<'_>,
    method_name: &str,
    arg_idx: usize,
    ctx: &ResolutionCtx<'_>,
) -> Option<ClassInfo> {
    let class_name = match class_expr {
        Expression::Self_(_) | Expression::Static(_) => ctx.current_class.map(|cc| cc.name.clone()),
        Expression::Identifier(ident) => Some(ident.value().to_string()),
        Expression::Parent(_) => ctx.current_class.and_then(|cc| cc.parent_class.clone()),
        _ => None,
    }?;

    let owner = ctx
        .all_classes
        .iter()
        .find(|c| c.name == class_name)
        .map(|c| ClassInfo::clone(c))
        .or_else(|| (ctx.class_loader)(&class_name).map(Arc::unwrap_or_clone))?;

    let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
        &owner,
        ctx.class_loader,
        ctx.resolved_class_cache,
    );
    let method = resolved.methods.iter().find(|m| m.name == method_name)?;
    closure_this_from_method_params(method, arg_idx, Some(&resolved), ctx)
}

/// Extract `closure_this_type` from a method's parameter at `arg_idx`
/// and resolve it to a `ClassInfo`.
fn closure_this_from_method_params(
    method: &MethodInfo,
    arg_idx: usize,
    owner: Option<&ClassInfo>,
    ctx: &ResolutionCtx<'_>,
) -> Option<ClassInfo> {
    let param = method.parameters.get(arg_idx)?;
    let php_type = param.closure_this_type.as_ref()?;
    resolve_closure_this_type(php_type, owner, ctx)
}

/// Resolve a raw `@param-closure-this` type string to a `ClassInfo`.
///
/// Handles `$this`, `static`, and `self` by mapping them to the
/// declaring class (owner), and resolves fully-qualified class names
/// through the class loader.
fn resolve_closure_this_type(
    php_type: &PhpType,
    owner: Option<&ClassInfo>,
    ctx: &ResolutionCtx<'_>,
) -> Option<ClassInfo> {
    // `$this`, `static`, and `self` all refer to the declaring class.
    if php_type.is_self_like() {
        return owner.cloned().or_else(|| ctx.current_class.cloned());
    }

    // Extract the base class name without stringifying.
    let type_str = php_type.base_name()?;

    // Try local classes first, then the cross-file loader.
    if let Some(cls) = ctx.all_classes.iter().find(|c| c.name == type_str) {
        return Some(ClassInfo::clone(cls));
    }

    let resolved = (ctx.class_loader)(type_str)?;
    Some(Arc::unwrap_or_clone(
        crate::virtual_members::resolve_class_fully_maybe_cached(
            &resolved,
            ctx.class_loader,
            ctx.resolved_class_cache,
        ),
    ))
}

/// Check whether `stmt` contains a closure or arrow function whose
/// body encloses the cursor.  If so, resolve the variable from the
/// closure's parameter list and walk its body, then return `true`.
pub(in crate::completion) fn try_resolve_in_closure_stmt<'b>(
    stmt: &'b Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) -> bool {
    match stmt {
        Statement::Expression(expr_stmt) => {
            try_resolve_in_closure_expr(expr_stmt.expression, ctx, results)
        }
        Statement::Return(ret) => {
            if let Some(val) = &ret.value {
                try_resolve_in_closure_expr(val, ctx, results)
            } else {
                false
            }
        }
        Statement::Block(block) => {
            for inner in block.statements.iter() {
                let s = inner.span();
                if ctx.cursor_offset >= s.start.offset
                    && ctx.cursor_offset <= s.end.offset
                    && try_resolve_in_closure_stmt(inner, ctx, results)
                {
                    return true;
                }
            }
            false
        }
        Statement::If(if_stmt) => {
            // Check the condition expression first — the closure may
            // be inside `if (array_any(..., fn(Foo $x) => $x->...))`.
            if try_resolve_in_closure_expr(if_stmt.condition, ctx, results) {
                return true;
            }
            // Then check elseif conditions.
            if let IfBody::Statement(body) = &if_stmt.body {
                for elseif in body.else_if_clauses.iter() {
                    if try_resolve_in_closure_expr(elseif.condition, ctx, results) {
                        return true;
                    }
                }
            }
            match &if_stmt.body {
                IfBody::Statement(body) => {
                    if try_resolve_in_closure_stmt(body.statement, ctx, results) {
                        return true;
                    }
                    for elseif in body.else_if_clauses.iter() {
                        if try_resolve_in_closure_stmt(elseif.statement, ctx, results) {
                            return true;
                        }
                    }
                    if let Some(else_clause) = &body.else_clause
                        && try_resolve_in_closure_stmt(else_clause.statement, ctx, results)
                    {
                        return true;
                    }
                    false
                }
                IfBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            }
        }
        Statement::Foreach(foreach) => match &foreach.body {
            ForeachBody::Statement(inner) => try_resolve_in_closure_stmt(inner, ctx, results),
            ForeachBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
                false
            }
        },
        Statement::While(while_stmt) => match &while_stmt.body {
            WhileBody::Statement(inner) => try_resolve_in_closure_stmt(inner, ctx, results),
            WhileBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
                false
            }
        },
        Statement::For(for_stmt) => match &for_stmt.body {
            ForBody::Statement(inner) => try_resolve_in_closure_stmt(inner, ctx, results),
            ForBody::ColonDelimited(body) => {
                for inner in body.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
                false
            }
        },
        Statement::DoWhile(dw) => try_resolve_in_closure_stmt(dw.statement, ctx, results),
        Statement::Namespace(ns) => {
            for inner in ns.statements().iter() {
                let s = inner.span();
                if ctx.cursor_offset >= s.start.offset
                    && ctx.cursor_offset <= s.end.offset
                    && try_resolve_in_closure_stmt(inner, ctx, results)
                {
                    return true;
                }
            }
            false
        }
        Statement::Try(try_stmt) => {
            for inner in try_stmt.block.statements.iter() {
                let s = inner.span();
                if ctx.cursor_offset >= s.start.offset
                    && ctx.cursor_offset <= s.end.offset
                    && try_resolve_in_closure_stmt(inner, ctx, results)
                {
                    return true;
                }
            }
            for catch in try_stmt.catch_clauses.iter() {
                for inner in catch.block.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
            }
            if let Some(finally) = &try_stmt.finally_clause {
                for inner in finally.block.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
            }
            false
        }
        Statement::Switch(switch) => {
            for case in switch.body.cases().iter() {
                for inner in case.statements().iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Recursively walk an expression looking for a `Closure` or
/// `ArrowFunction` whose body contains the cursor.  When found,
/// resolve the target variable from the closure's parameters and
/// walk its body statements, returning `true`.
pub(in crate::completion) fn try_resolve_in_closure_expr<'b>(
    expr: &'b Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) -> bool {
    // Quick span-based prune: if the cursor is not within this
    // expression at all, skip the entire sub-tree.
    let sp = expr.span();
    if ctx.cursor_offset < sp.start.offset || ctx.cursor_offset > sp.end.offset {
        return false;
    }

    match expr {
        // ── Closure: `function (Type $param) { … }` ──
        Expression::Closure(closure) => {
            let body_start = closure.body.left_brace.start.offset;
            let body_end = closure.body.right_brace.end.offset;
            if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                resolve_closure_params(&closure.parameter_list, ctx, results);
                super::resolution::walk_statements_for_assignments(
                    closure.body.statements.iter(),
                    ctx,
                    results,
                    false,
                );
                // When parameter resolution and assignment walking
                // yielded no type (common for untyped closure params),
                // fall back to a backward `@var` docblock scan.
                // This handles the pattern:
                //   function ($app, $params) {
                //       /** @var App $app */
                //       $app->make(...);
                //   }
                if results.is_empty() {
                    try_standalone_var_docblock(ctx, results);
                }
                return true;
            }
            false
        }
        // ── Arrow function: `fn(Type $param) => expr` ──
        // Arrow functions capture the enclosing scope automatically
        // (unlike closures which require `use`).  Only claim the
        // variable if it matches one of the arrow function's own
        // parameters; otherwise return `false` so the outer scope
        // walk continues and can resolve variables like `$feature`
        // that were assigned before the arrow function.
        Expression::ArrowFunction(arrow) => {
            let arrow_body_span = arrow.expression.span();
            if ctx.cursor_offset >= arrow.arrow.start.offset
                && ctx.cursor_offset <= arrow_body_span.end.offset
            {
                let is_arrow_param = arrow
                    .parameter_list
                    .parameters
                    .iter()
                    .any(|p| *p.variable.name == *ctx.var_name);
                if is_arrow_param {
                    resolve_closure_params(&arrow.parameter_list, ctx, results);
                    return true;
                }
                // Variable is not a parameter of this arrow function.
                // Before falling back to the enclosing scope, recurse
                // into the body expression — the cursor may be inside
                // a closure nested within the arrow function's body
                // (e.g. `fn($r) => $r->whereHas('x', function (Foo $q) { $q-> })`).
                // If we find and resolve inside a nested closure, we're
                // done; otherwise return false so the outer walk continues.
                return try_resolve_in_closure_expr(arrow.expression, ctx, results);
            }
            false
        }
        Expression::Parenthesized(p) => try_resolve_in_closure_expr(p.expression, ctx, results),
        Expression::Assignment(a) => {
            try_resolve_in_closure_expr(a.lhs, ctx, results)
                || try_resolve_in_closure_expr(a.rhs, ctx, results)
        }
        Expression::Binary(bin) => {
            try_resolve_in_closure_expr(bin.lhs, ctx, results)
                || try_resolve_in_closure_expr(bin.rhs, ctx, results)
        }
        Expression::Conditional(cond) => {
            try_resolve_in_closure_expr(cond.condition, ctx, results)
                || cond
                    .then
                    .is_some_and(|e| try_resolve_in_closure_expr(e, ctx, results))
                || try_resolve_in_closure_expr(cond.r#else, ctx, results)
        }
        Expression::Call(call) => try_resolve_in_closure_call(call, ctx, results),
        Expression::Array(arr) => {
            for elem in arr.elements.iter() {
                let found = match elem {
                    ArrayElement::KeyValue(kv) => {
                        try_resolve_in_closure_expr(kv.key, ctx, results)
                            || try_resolve_in_closure_expr(kv.value, ctx, results)
                    }
                    ArrayElement::Value(v) => try_resolve_in_closure_expr(v.value, ctx, results),
                    ArrayElement::Variadic(v) => try_resolve_in_closure_expr(v.value, ctx, results),
                    _ => false,
                };
                if found {
                    return true;
                }
            }
            false
        }
        Expression::LegacyArray(arr) => {
            for elem in arr.elements.iter() {
                let found = match elem {
                    ArrayElement::KeyValue(kv) => {
                        try_resolve_in_closure_expr(kv.key, ctx, results)
                            || try_resolve_in_closure_expr(kv.value, ctx, results)
                    }
                    ArrayElement::Value(v) => try_resolve_in_closure_expr(v.value, ctx, results),
                    ArrayElement::Variadic(v) => try_resolve_in_closure_expr(v.value, ctx, results),
                    _ => false,
                };
                if found {
                    return true;
                }
            }
            false
        }
        Expression::Match(m) => {
            if try_resolve_in_closure_expr(m.expression, ctx, results) {
                return true;
            }
            for arm in m.arms.iter() {
                if try_resolve_in_closure_expr(arm.expression(), ctx, results) {
                    return true;
                }
            }
            false
        }
        Expression::Access(access) => match access {
            Access::Property(pa) => try_resolve_in_closure_expr(pa.object, ctx, results),
            Access::NullSafeProperty(pa) => try_resolve_in_closure_expr(pa.object, ctx, results),
            Access::StaticProperty(pa) => try_resolve_in_closure_expr(pa.class, ctx, results),
            Access::ClassConstant(pa) => try_resolve_in_closure_expr(pa.class, ctx, results),
        },
        Expression::Instantiation(inst) => {
            if let Some(ref args) = inst.argument_list {
                try_resolve_in_closure_args(&args.arguments, ctx, results)
            } else {
                false
            }
        }
        Expression::UnaryPrefix(u) => try_resolve_in_closure_expr(u.operand, ctx, results),
        Expression::UnaryPostfix(u) => try_resolve_in_closure_expr(u.operand, ctx, results),
        Expression::Yield(y) => match y {
            Yield::Value(yv) => {
                if let Some(val) = &yv.value {
                    try_resolve_in_closure_expr(val, ctx, results)
                } else {
                    false
                }
            }
            Yield::Pair(yp) => {
                try_resolve_in_closure_expr(yp.key, ctx, results)
                    || try_resolve_in_closure_expr(yp.value, ctx, results)
            }
            Yield::From(yf) => try_resolve_in_closure_expr(yf.iterator, ctx, results),
        },
        Expression::Throw(t) => try_resolve_in_closure_expr(t.exception, ctx, results),
        Expression::Clone(c) => try_resolve_in_closure_expr(c.object, ctx, results),
        Expression::Pipe(p) => {
            try_resolve_in_closure_expr(p.input, ctx, results)
                || try_resolve_in_closure_expr(p.callable, ctx, results)
        }
        _ => false,
    }
}

/// Dispatch a `Call` expression: recurse into function, method, and
/// static method calls, checking their argument lists for closures.
fn try_resolve_in_closure_call<'b>(
    call: &'b Call<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) -> bool {
    match call {
        Call::Function(fc) => {
            // Try with callable parameter inference from the function signature.
            if let Some(func_name) = extract_function_name_from_call(fc)
                && try_resolve_closure_in_call_args(
                    &fc.argument_list.arguments,
                    ctx,
                    results,
                    |arg_idx| {
                        infer_callable_params_from_function(
                            &func_name,
                            arg_idx,
                            &fc.argument_list.arguments,
                            ctx,
                        )
                    },
                )
            {
                return true;
            }
            try_resolve_in_closure_args(&fc.argument_list.arguments, ctx, results)
        }
        Call::Method(mc) => {
            if try_resolve_in_closure_expr(mc.object, ctx, results) {
                return true;
            }
            // Try with callable parameter inference from the method signature.
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let method_name = ident.value.to_string();
                let obj_span = mc.object.span();
                let first_arg = extract_first_arg_string(&mc.argument_list.arguments, ctx.content);
                if try_resolve_closure_in_call_args(
                    &mc.argument_list.arguments,
                    ctx,
                    results,
                    |arg_idx| {
                        infer_callable_params_from_receiver(
                            obj_span.start.offset,
                            obj_span.end.offset,
                            &method_name,
                            arg_idx,
                            first_arg.as_deref(),
                            ctx,
                        )
                    },
                ) {
                    return true;
                }
            }
            try_resolve_in_closure_args(&mc.argument_list.arguments, ctx, results)
        }
        Call::NullSafeMethod(mc) => {
            if try_resolve_in_closure_expr(mc.object, ctx, results) {
                return true;
            }
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let method_name = ident.value.to_string();
                let obj_span = mc.object.span();
                let first_arg = extract_first_arg_string(&mc.argument_list.arguments, ctx.content);
                if try_resolve_closure_in_call_args(
                    &mc.argument_list.arguments,
                    ctx,
                    results,
                    |arg_idx| {
                        infer_callable_params_from_receiver(
                            obj_span.start.offset,
                            obj_span.end.offset,
                            &method_name,
                            arg_idx,
                            first_arg.as_deref(),
                            ctx,
                        )
                    },
                ) {
                    return true;
                }
            }
            try_resolve_in_closure_args(&mc.argument_list.arguments, ctx, results)
        }
        Call::StaticMethod(sc) => {
            if try_resolve_in_closure_expr(sc.class, ctx, results) {
                return true;
            }
            if let ClassLikeMemberSelector::Identifier(ident) = &sc.method {
                let method_name = ident.value.to_string();
                let first_arg = extract_first_arg_string(&sc.argument_list.arguments, ctx.content);
                if try_resolve_closure_in_call_args(
                    &sc.argument_list.arguments,
                    ctx,
                    results,
                    |arg_idx| {
                        infer_callable_params_from_static_receiver(
                            sc.class,
                            &method_name,
                            arg_idx,
                            first_arg.as_deref(),
                            ctx,
                        )
                    },
                ) {
                    return true;
                }
            }
            try_resolve_in_closure_args(&sc.argument_list.arguments, ctx, results)
        }
    }
}

/// Walk a flat list of call arguments, recursing into each expression.
fn try_resolve_in_closure_args<'b>(
    arguments: &'b TokenSeparatedSequence<'b, Argument<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) -> bool {
    for arg in arguments.iter() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        if try_resolve_in_closure_expr(arg_expr, ctx, results) {
            return true;
        }
    }
    false
}

/// Walk call arguments looking for a closure/arrow-function that
/// contains the cursor.  When found, resolve its parameters using
/// both explicit type hints and inferred types from the enclosing
/// call's signature (provided by `infer_fn`).
///
/// Returns `true` if the cursor was inside a closure argument and
/// resolution was performed.
fn try_resolve_closure_in_call_args<'b, F>(
    arguments: &'b TokenSeparatedSequence<'b, Argument<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
    infer_fn: F,
) -> bool
where
    F: Fn(usize) -> Vec<PhpType>,
{
    for (arg_idx, arg) in arguments.iter().enumerate() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        let arg_span = arg_expr.span();
        if ctx.cursor_offset < arg_span.start.offset || ctx.cursor_offset > arg_span.end.offset {
            continue;
        }

        match arg_expr {
            Expression::Closure(closure) => {
                let body_start = closure.body.left_brace.start.offset;
                let body_end = closure.body.right_brace.end.offset;
                if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                    // Only run inference when the target variable is
                    // actually one of the closure's own parameters.
                    // If the target variable is NOT a closure param,
                    // the inference result would be unused anyway, so
                    // skipping it avoids an infinite recursion cycle.
                    let is_closure_param = closure
                        .parameter_list
                        .parameters
                        .iter()
                        .any(|p| *p.variable.name == *ctx.var_name);
                    if is_closure_param {
                        let inferred = infer_fn(arg_idx);
                        resolve_closure_params_with_inferred(
                            &closure.parameter_list,
                            ctx,
                            results,
                            &inferred,
                        );
                    } else {
                        resolve_closure_params(&closure.parameter_list, ctx, results);
                    }
                    super::resolution::walk_statements_for_assignments(
                        closure.body.statements.iter(),
                        ctx,
                        results,
                        false,
                    );
                    return true;
                }
            }
            Expression::ArrowFunction(arrow) => {
                let arrow_body_span = arrow.expression.span();
                if ctx.cursor_offset >= arrow.arrow.start.offset
                    && ctx.cursor_offset <= arrow_body_span.end.offset
                {
                    let is_closure_param = arrow
                        .parameter_list
                        .parameters
                        .iter()
                        .any(|p| *p.variable.name == *ctx.var_name);
                    if is_closure_param {
                        let inferred = infer_fn(arg_idx);
                        resolve_closure_params_with_inferred(
                            &arrow.parameter_list,
                            ctx,
                            results,
                            &inferred,
                        );
                        return true;
                    }
                    // Variable is not a parameter of this arrow
                    // function — it comes from the enclosing scope.
                    // Return false so the outer walk resolves it.
                    return false;
                }
            }
            _ => {}
        }
        // Not a direct closure — fall through so the normal recursive
        // walker handles nested closures (without inference).
        return false;
    }
    false
}

/// Resolve a variable's type from a closure / arrow-function
/// parameter list.  If the variable matches a typed parameter,
/// the resolved classes replace whatever is currently in `results`.
/// Fall back to a backward `@var Type $varName` docblock scan when
/// normal parameter resolution and assignment walking produced no type.
///
/// This covers the common pattern where closure parameters lack type
/// hints and a standalone multi-variable `@var` block declares their
/// types without a following assignment:
///
/// ```php
/// app()->bind(Foo::class, function ($app, $params) {
///     /**
///      * @var App                      $app
///      * @var array{indexName: string} $params
///      */
///     $app->make(Client::class);
/// });
/// ```
pub(in crate::completion) fn try_standalone_var_docblock(
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    if let Some(raw_type) = crate::docblock::find_var_raw_type_in_source(
        ctx.content,
        ctx.cursor_offset as usize,
        ctx.var_name,
    ) {
        let parsed = raw_type;
        let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
            &parsed,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );
        if !resolved.is_empty() {
            *results = ResolvedType::from_classes_with_hint(resolved, parsed);
        } else if parsed.is_informative() {
            // Non-class types like `array{key: string}` or `int[]`.
            *results = vec![ResolvedType::from_type_string(parsed)];
        }
    }
}

pub(in crate::completion) fn resolve_closure_params(
    parameter_list: &FunctionLikeParameterList<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
) {
    resolve_closure_params_with_inferred(parameter_list, ctx, results, &[]);
}

/// Like [`resolve_closure_params`] but accepts a list of inferred
/// parameter types from the enclosing callable signature.  When a
/// closure parameter has no explicit type hint, the corresponding
/// entry in `inferred_types` (matched by positional index) is used
/// as a fallback.
fn resolve_closure_params_with_inferred(
    parameter_list: &FunctionLikeParameterList<'_>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
    inferred_types: &[PhpType],
) {
    let mut matched_param_is_variadic = false;
    for (idx, param) in parameter_list.parameters.iter().enumerate() {
        let pname = param.variable.name.to_string();
        if pname == ctx.var_name {
            matched_param_is_variadic = param.ellipsis.is_some();
            // Clone the inferred type (if any) so that all branches
            // below can reuse it without reborrowing the slice.
            let parsed_inferred = inferred_types.get(idx).cloned();
            // 1. Try the explicit type hint first.
            if let Some(hint) = &param.hint {
                let hint_type = extract_hint_type(hint);

                // When the explicit hint is a bare class name (no
                // generic args) and the inferred type from the callable
                // signature is the same class WITH generic args, prefer
                // the inferred type.  For example, the user writes
                // `function (Collection $customers)` but the callable
                // signature says `callable(Collection<int, Customer>, int): mixed`.
                // Using the inferred `Collection<int, Customer>` preserves
                // template substitution so that foreach iteration resolves
                // the element type.
                if let Some(inferred) = inferred_types.get(idx)
                    && inferred_type_is_more_specific(&hint_type, inferred)
                {
                    let pi = parsed_inferred.as_ref().unwrap();
                    let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                        pi,
                        &ctx.current_class.name,
                        ctx.all_classes,
                        ctx.class_loader,
                    );
                    if !resolved.is_empty() {
                        *results = ResolvedType::from_classes_with_hint(
                            resolved,
                            parsed_inferred.unwrap(),
                        );
                        break;
                    }
                }

                let resolved_classes =
                    crate::completion::type_resolution::type_hint_to_classes_typed(
                        &hint_type,
                        &ctx.current_class.name,
                        ctx.all_classes,
                        ctx.class_loader,
                    );
                if !resolved_classes.is_empty() {
                    // When the inferred type from the callable signature
                    // is a subclass of the explicit type hint, prefer
                    // the inferred type.  For example, the user writes
                    // `function (Model $item)` but the callable signature
                    // says `callable(BrandTranslation): void` where
                    // `BrandTranslation extends Model`.  The narrower
                    // inferred type gives better completion results.
                    if let Some(ref pi) = parsed_inferred {
                        let inferred_resolved =
                            crate::completion::type_resolution::type_hint_to_classes_typed(
                                pi,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            );
                        if !inferred_resolved.is_empty()
                            && inferred_resolved.iter().all(|inferred_cls| {
                                resolved_classes.iter().any(|explicit_cls| {
                                    crate::util::is_subtype_of_names(
                                        &inferred_cls.fqn(),
                                        &explicit_cls.fqn(),
                                        ctx.class_loader,
                                    )
                                })
                            })
                        {
                            *results = ResolvedType::from_classes_with_hint(
                                inferred_resolved,
                                parsed_inferred.unwrap(),
                            );
                            break;
                        }
                    }
                    *results =
                        ResolvedType::from_classes_with_hint(resolved_classes, hint_type.clone());
                    break;
                }

                // The explicit hint didn't resolve to any class (e.g.
                // `int $value`, `string $name`).  Check the `@param`
                // docblock annotation which may carry a more specific
                // type (e.g. `class-string<BackedEnum>` when the native
                // hint is just `string`).
                let param_start = parameter_list.left_parenthesis.start.offset as usize;
                let docblock_type = crate::docblock::find_iterable_raw_type_in_source(
                    ctx.content,
                    param_start,
                    ctx.var_name,
                );
                if let Some(ref parsed_dt) = docblock_type {
                    let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                        parsed_dt,
                        &ctx.current_class.name,
                        ctx.all_classes,
                        ctx.class_loader,
                    );
                    if !resolved.is_empty() {
                        *results =
                            ResolvedType::from_classes_with_hint(resolved, parsed_dt.clone());
                        break;
                    }
                }

                // Emit a type-string-only entry so that consumers like
                // hover and diagnostics can see the parameter's type
                // even when it's a scalar.  Prefer the docblock type
                // over the native type when available.
                let best_type = docblock_type.unwrap_or_else(|| hint_type.clone());
                *results = vec![ResolvedType::from_type_string(best_type)];
                break;
            }
            // 2. Fall back to the inferred type from the callable
            //    signature of the enclosing method/function call.
            if let Some(ref pi) = parsed_inferred {
                let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                    pi,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
                if !resolved.is_empty() {
                    *results =
                        ResolvedType::from_classes_with_hint(resolved, parsed_inferred.unwrap());
                }
            }
            break;
        }
    }

    // ── Variadic parameter wrapping ─────────────────────────────
    // When the matched parameter is variadic (e.g.
    // `HtmlString|int|string ...$placeholders`), the native type
    // hint describes the *element* type, but the variable itself
    // holds `list<ElementType>`.  Wrap the resolved types so that
    // foreach iteration can extract the element type via
    // `PhpType::extract_value_type`.
    if matched_param_is_variadic && !results.is_empty() {
        for rt in results.iter_mut() {
            rt.type_string = PhpType::list(rt.type_string.clone());
            // The variable is now an array, not a class instance,
            // so clear the class_info.
            rt.class_info = None;
        }
    }
}

/// Check whether the inferred callable-signature type is a more specific
/// version of the explicit type hint.
///
/// Returns `true` when the explicit hint is a bare class name (e.g.
/// `Collection`) and the inferred type is the same class with generic
/// arguments (e.g. `Collection<int, Customer>`).  Namespace-qualified
/// names are compared by their last segment so that `Collection` matches
/// `Illuminate\Support\Collection<int, Customer>`.
fn inferred_type_is_more_specific(explicit_hint: &PhpType, inferred: &PhpType) -> bool {
    // The explicit hint must be a bare class name (no generic args).
    let explicit_base = match explicit_hint {
        PhpType::Named(name) => name.as_str(),
        _ => return false,
    };

    // The inferred type must be a generic type (carries generic args).
    let inferred_base = match inferred {
        PhpType::Generic(name, _) => name.as_str(),
        _ => return false,
    };

    // Compare by short name so that `Collection` matches
    // `Illuminate\Support\Collection<…>`.
    let explicit_short = crate::util::short_name(explicit_base);
    let inferred_short = crate::util::short_name(inferred_base);

    explicit_short.eq_ignore_ascii_case(inferred_short)
}

// ── Callable parameter inference helpers ────────────────────────────

/// Extract the function name from a `FunctionCall` AST node.
fn extract_function_name_from_call(fc: &FunctionCall<'_>) -> Option<String> {
    match fc.function {
        Expression::Identifier(ident) => Some(ident.value().to_string()),
        _ => None,
    }
}

/// Infer callable parameter types for a closure passed at position
/// `arg_idx` to a standalone function call.
///
/// When the function has `@template` parameters (e.g.
/// `array_any(array<TKey, TValue> $array, callable(TValue, TKey): bool $cb)`),
/// the template substitution map is built from the other call-site
/// arguments and applied to the callable's parameter types.  This turns
/// raw template names like `TValue` into concrete types like
/// `PurchaseFileProduct`.
fn infer_callable_params_from_function(
    func_name: &str,
    arg_idx: usize,
    arguments: &TokenSeparatedSequence<'_, argument::Argument<'_>>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<PhpType> {
    let rctx = ctx.as_resolution_ctx();
    let func_info = if let Some(fl) = rctx.function_loader {
        fl(func_name)
    } else {
        None
    };
    if let Some(fi) = func_info {
        let mut params = extract_callable_params_at(&fi.parameters, arg_idx, ctx);

        // When the function has template parameters, build a
        // substitution map from the concrete call-site arguments and
        // apply it to the extracted callable param types.  Without
        // this, functions like `array_any($this->items, fn($item) => …)`
        // would pass raw template names (`TValue`) instead of the
        // concrete element type (`PurchaseFileProduct`).
        if !params.is_empty() && !fi.template_params.is_empty() && !fi.template_bindings.is_empty()
        {
            let arg_texts = extract_argument_texts(arguments, ctx.content);
            let subs = super::rhs_resolution::build_function_template_subs(&fi, &arg_texts, &rctx);
            if !subs.is_empty() {
                params = params.into_iter().map(|p| p.substitute(&subs)).collect();
            }
        }

        params
    } else {
        vec![]
    }
}

/// Extract the source text of each positional argument in a call's
/// argument list.
fn extract_argument_texts(
    arguments: &TokenSeparatedSequence<'_, argument::Argument<'_>>,
    content: &str,
) -> Vec<String> {
    arguments
        .iter()
        .map(|arg| {
            let span = match arg {
                argument::Argument::Positional(pos) => pos.value.span(),
                argument::Argument::Named(named) => named.value.span(),
            };
            let start = span.start.offset as usize;
            let end = span.end.offset as usize;
            if end <= content.len() {
                content[start..end].to_string()
            } else {
                String::new()
            }
        })
        .collect()
}

/// Infer callable parameter types for a closure passed at position
/// `arg_idx` to an instance method call whose receiver expression
/// spans `[obj_start, obj_end)` in the source text.
fn infer_callable_params_from_receiver(
    obj_start: u32,
    obj_end: u32,
    method_name: &str,
    arg_idx: usize,
    first_arg_text: Option<&str>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<PhpType> {
    // Guard against infinite recursion when nested closures reuse the
    // same variable name (e.g. `$q` in both an outer and inner closure).
    // The cycle is: infer_callable_params_from_receiver →
    // resolve_target_classes → resolve_variable_types →
    // walk_statements_for_assignments → try_resolve_closure_in_call_args
    // → infer_callable_params_from_receiver → ∞
    let depth = CLOSURE_INFER_DEPTH.with(|d| d.get());
    if depth >= MAX_CLOSURE_INFER_DEPTH {
        return vec![];
    }
    CLOSURE_INFER_DEPTH.with(|d| d.set(depth + 1));

    let start = obj_start as usize;
    let end = obj_end as usize;
    if end > ctx.content.len() {
        CLOSURE_INFER_DEPTH.with(|d| d.set(depth));
        return vec![];
    }
    let obj_text = ctx.content[start..end].trim();
    let rctx = ctx.as_resolution_ctx();
    let receiver_classes = ResolvedType::into_arced_classes(
        crate::completion::resolver::resolve_target_classes(obj_text, AccessKind::Arrow, &rctx),
    );

    // For relation-query methods (whereHas, etc.), override the closure
    // parameter type with Builder<RelatedModel> resolved from the
    // relation name string.
    if let Some(override_params) = try_relation_query_override(
        &receiver_classes,
        method_name,
        first_arg_text,
        ctx.class_loader,
    ) {
        CLOSURE_INFER_DEPTH.with(|d| d.set(depth));
        return override_params;
    }

    let params = find_callable_params_on_classes(&receiver_classes, method_name, arg_idx, ctx);

    // Replace `$this` / `static` tokens with the receiver's full type
    // so that `resolve_closure_params_with_inferred` resolves them
    // against the declaring class rather than the user's current class.
    //
    // When the receiver is a generic class whose template params have
    // already been substituted (e.g. `Builder<Product>`), we need to
    // reconstruct the full generic type so that the inferred callable
    // param type carries the generic args.  Without this, `$this` on
    // `Builder<Product>` would degrade to bare `Builder`, losing the
    // model type and preventing scope method resolution.
    let result = if let Some(receiver) = receiver_classes.first() {
        let receiver_type = build_receiver_self_type(receiver, ctx.class_loader);
        params
            .into_iter()
            .map(|p| p.replace_self_with_type(&receiver_type))
            .collect()
    } else {
        params
    };

    CLOSURE_INFER_DEPTH.with(|d| d.set(depth));
    result
}

/// Infer callable parameter types for a closure passed at position
/// `arg_idx` to a static method call.
fn infer_callable_params_from_static_receiver(
    class_expr: &Expression<'_>,
    method_name: &str,
    arg_idx: usize,
    first_arg_text: Option<&str>,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<PhpType> {
    let class_name = match class_expr {
        Expression::Self_(_) => Some(ctx.current_class.name.clone()),
        Expression::Static(_) => Some(ctx.current_class.name.clone()),
        Expression::Identifier(ident) => Some(ident.value().to_string()),
        Expression::Parent(_) => ctx.current_class.parent_class.clone(),
        _ => None,
    };
    let owner = class_name.and_then(|name| {
        ctx.all_classes
            .iter()
            .find(|c| c.name == name)
            .map(|c| ClassInfo::clone(c))
            .or_else(|| (ctx.class_loader)(&name).map(Arc::unwrap_or_clone))
    });
    if let Some(ref cls) = owner {
        // For relation-query methods (whereHas, etc.), override the
        // closure parameter type with Builder<RelatedModel> resolved
        // from the relation name string.
        if let Some(override_params) = try_relation_query_override(
            &[Arc::new(cls.clone())],
            method_name,
            first_arg_text,
            ctx.class_loader,
        ) {
            return override_params;
        }

        let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
            cls,
            ctx.class_loader,
            ctx.resolved_class_cache,
        );
        let params = find_callable_params_on_method(&resolved, method_name, arg_idx, ctx);
        let owner_fqn = cls.fqn();
        params
            .into_iter()
            .map(|p| p.replace_self(&owner_fqn))
            .collect()
    } else {
        vec![]
    }
}

/// Search for the method `method_name` on each of `classes` and
/// extract callable parameter types at `arg_idx`.
fn find_callable_params_on_classes(
    classes: &[Arc<ClassInfo>],
    method_name: &str,
    arg_idx: usize,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<PhpType> {
    for cls in classes {
        let resolved = crate::virtual_members::resolve_class_fully_maybe_cached(
            cls,
            ctx.class_loader,
            ctx.resolved_class_cache,
        );
        let result = find_callable_params_on_method(&resolved, method_name, arg_idx, ctx);
        if !result.is_empty() {
            return result;
        }
    }
    vec![]
}

/// Look up method `method_name` on `class` and extract callable
/// parameter types from the parameter at position `arg_idx`.
fn find_callable_params_on_method(
    class: &ClassInfo,
    method_name: &str,
    arg_idx: usize,
    ctx: &VarResolutionCtx<'_>,
) -> Vec<PhpType> {
    let method = class.methods.iter().find(|m| m.name == method_name);
    if let Some(m) = method {
        extract_callable_params_at(&m.parameters, arg_idx, ctx)
    } else {
        vec![]
    }
}

/// Given a list of method/function parameters, look at the parameter
/// at `arg_idx`.  If its type hint is a `callable(…)` or
/// `Closure(…)`, extract and return the callable's parameter types.
fn extract_callable_params_at(
    params: &[crate::types::ParameterInfo],
    arg_idx: usize,
    _ctx: &VarResolutionCtx<'_>,
) -> Vec<PhpType> {
    let param = params.get(arg_idx);
    if let Some(p) = param
        && let Some(ref hint) = p.type_hint
        && let Some(callable_params) = hint.callable_param_types()
    {
        return callable_params
            .iter()
            .map(|cp| cp.type_hint.clone())
            .collect();
    }
    vec![]
}

/// Extract the text of the first positional argument from a call's
/// argument list, stripping surrounding quotes from string literals.
fn extract_first_arg_string(
    arguments: &TokenSeparatedSequence<'_, argument::Argument<'_>>,
    content: &str,
) -> Option<String> {
    let first = arguments.iter().next()?;
    let expr = match first {
        argument::Argument::Positional(pos) => pos.value,
        argument::Argument::Named(named) => named.value,
    };
    let span = expr.span();
    let start = span.start.offset as usize;
    let end = span.end.offset as usize;
    let raw = content.get(start..end)?.trim();

    // Strip surrounding quotes (single or double).
    if raw.len() >= 2
        && ((raw.starts_with('\'') && raw.ends_with('\''))
            || (raw.starts_with('"') && raw.ends_with('"')))
    {
        Some(raw[1..raw.len() - 1].to_string())
    } else {
        None
    }
}

/// Check whether `method_name` is a relation-query method (e.g.
/// `whereHas`, `orWhereHas`, `whereDoesntHave`, etc.) and the receiver
/// is an Eloquent model or `Builder<Model>`.  If so, resolve the
/// relation chain from the first argument string and return
/// `Builder<FinalRelatedModel>` as the closure parameter type.
///
/// Returns `None` when the override does not apply (not a relation-query
/// method, receiver is not a model, relation chain cannot be resolved),
/// in which case the caller falls through to normal callable param
/// inference.
fn try_relation_query_override(
    receiver_classes: &[Arc<ClassInfo>],
    method_name: &str,
    first_arg_text: Option<&str>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<Vec<PhpType>> {
    // Only applies to the known relation-query methods.
    if !RELATION_QUERY_METHODS.contains(&method_name) {
        return None;
    }

    let relation_name = first_arg_text?;
    if relation_name.is_empty() {
        return None;
    }

    // Determine the base model from the receiver.  The receiver may be
    // the model itself (static call: `Brand::whereHas(...)`) or a
    // `Builder<Model>` instance.
    let model = find_model_from_receivers(receiver_classes, class_loader)?;

    // Walk the dot-separated relation chain to find the final related model.
    let related_fqn = resolve_relation_chain(&model, relation_name, class_loader)?;

    // Return `Builder<RelatedModel>` as the closure parameter type.
    let builder_type = PhpType::Generic(
        ELOQUENT_BUILDER_FQN.to_string(),
        vec![PhpType::Named(related_fqn)],
    );

    Some(vec![builder_type])
}

/// Given a list of receiver classes, find the underlying Eloquent model.
///
/// If the receiver is a model class directly, return it.  If it's
/// `Builder<Model>`, extract the model from the Builder's method return
/// types (which contain the substituted generic arg, e.g.
/// `Builder<Brand>` → `Brand`).
/// Build a `PhpType` representing the receiver class for `$this`/`static`
/// replacement in callable parameter inference.
///
/// For most classes this returns `PhpType::Named(fqn)`.  For classes
/// whose template parameters have been concretely substituted (detected
/// by scanning method return types for generic signatures), the full
/// generic type is reconstructed.  For example, an Eloquent
/// `Builder<Product>` receiver produces
/// `PhpType::Generic("Illuminate\\Database\\Eloquent\\Builder", [Named("App\\Product")])`
/// instead of a bare `PhpType::Named("Illuminate\\Database\\Eloquent\\Builder")`.
///
/// This preserves generic args through callable param inference so that
/// `callable($this)` on `Builder<Product>` infers `Builder<Product>`,
/// not bare `Builder`.
fn build_receiver_self_type(
    receiver: &ClassInfo,
    _class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> PhpType {
    let fqn = receiver.fqn();

    // Only attempt reconstruction when the class declares template
    // params — otherwise there are no generic args to recover.
    if receiver.template_params.is_empty() {
        return PhpType::Named(fqn);
    }

    // For Eloquent Builder, extract the model name from method return
    // types where generic substitution has already been applied.
    if (receiver.name == "Builder" || fqn == ELOQUENT_BUILDER_FQN)
        && let Some(model_type) = extract_model_from_builder(receiver)
    {
        return PhpType::Generic(ELOQUENT_BUILDER_FQN.to_string(), vec![model_type]);
    }

    // General case: try to recover concrete generic args from method
    // return types that reference the class itself with generic params.
    // For example, if a `Collection<int, User>` has a method returning
    // `Collection<int, User>`, we can extract `[int, User]` as the
    // concrete args.
    if let Some(args) = extract_generic_args_from_methods(receiver, &fqn) {
        return PhpType::Generic(fqn, args);
    }

    // Fallback: if we have a parent class with @extends generics and
    // only one template param, try to extract from the parent chain.
    // This covers cases like Relation<TRelatedModel> subclasses.
    if !receiver.extends_generics.is_empty() && receiver.template_params.len() == 1 {
        for (_, args) in &receiver.extends_generics {
            if let Some(first_arg) = args.first() {
                // Skip raw template param names that weren't substituted.
                let is_unsubstituted = if let PhpType::Named(name) = first_arg {
                    receiver.template_params.contains(name)
                } else {
                    false
                };
                if !is_unsubstituted {
                    return PhpType::Generic(fqn, vec![first_arg.clone()]);
                }
            }
        }
    }

    PhpType::Named(fqn)
}

/// Try to extract concrete generic args from a class's own methods.
///
/// Scans method return types for `ClassName<Arg1, Arg2, ...>` patterns
/// where the base name matches the class, and the args are concrete
/// (not raw template param names).
fn extract_generic_args_from_methods(class: &ClassInfo, class_fqn: &str) -> Option<Vec<PhpType>> {
    let class_short = crate::util::short_name(class_fqn);
    for method in &class.methods {
        if let Some(PhpType::Generic(base, args)) = &method.return_type {
            let base_short = crate::util::short_name(base);
            if (base == class_fqn || base_short.eq_ignore_ascii_case(class_short))
                && !args.is_empty()
                && args.iter().all(|a| {
                    if let PhpType::Named(n) = a {
                        !class.template_params.contains(n)
                    } else {
                        true
                    }
                })
            {
                return Some(args.clone());
            }
        }
    }
    None
}

fn find_model_from_receivers(
    receiver_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<Arc<ClassInfo>> {
    for cls in receiver_classes {
        // Direct model class.
        if extends_eloquent_model(cls, class_loader) {
            return Some(Arc::clone(cls));
        }

        // Builder<Model> — extract the model name from the Builder's
        // method return types.  After generic substitution, methods like
        // `where()` return `Builder<Brand>`, so we can extract `Brand`
        // from any method's return type that is `Builder<X>`.
        let cls_fqn = cls.fqn();
        if (cls.name == "Builder" || cls_fqn == ELOQUENT_BUILDER_FQN)
            && let Some(model_type) = extract_model_from_builder(cls)
            && let Some(model_cls) = model_type.base_name().and_then(class_loader)
            && extends_eloquent_model(&model_cls, class_loader)
        {
            return Some(model_cls);
        }
    }
    None
}

/// Extract the model type from a resolved `Builder<Model>` class by
/// scanning its method return types for `Builder<X>` and returning `X`.
fn extract_model_from_builder(builder: &ClassInfo) -> Option<PhpType> {
    for method in &builder.methods {
        if let Some(ref ret) = method.return_type
            && let PhpType::Generic(base, args) = ret
            && !args.is_empty()
            && (base == ELOQUENT_BUILDER_FQN || base == "Builder")
        {
            // Skip unsubstituted template params like "TModel".
            if !args[0].is_empty() && !args[0].is_named("TModel") {
                return Some(args[0].clone());
            }
        }
    }
    None
}
