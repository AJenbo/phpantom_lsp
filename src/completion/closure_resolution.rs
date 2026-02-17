/// Closure and arrow-function variable resolution.
///
/// When the cursor is inside a closure (`function (Type $p) { … }`) or
/// arrow function (`fn(Type $p) => …`), variables are resolved from the
/// closure's own parameter list rather than the enclosing scope.  This
/// module contains the recursive AST walkers that detect whether the
/// cursor falls inside such a construct and, if so, resolve the target
/// variable from its typed parameters.
use mago_span::HasSpan;
use mago_syntax::ast::sequence::TokenSeparatedSequence;
use mago_syntax::ast::*;

use crate::Backend;
use crate::types::ClassInfo;

use super::resolver::VarResolutionCtx;

impl Backend {
    /// Check whether `stmt` contains a closure or arrow function whose
    /// body encloses the cursor.  If so, resolve the variable from the
    /// closure's parameter list and walk its body, then return `true`.
    pub(super) fn try_resolve_in_closure_stmt<'b>(
        stmt: &'b Statement<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) -> bool {
        match stmt {
            Statement::Expression(expr_stmt) => {
                Self::try_resolve_in_closure_expr(expr_stmt.expression, ctx, results)
            }
            Statement::Return(ret) => {
                if let Some(val) = &ret.value {
                    Self::try_resolve_in_closure_expr(val, ctx, results)
                } else {
                    false
                }
            }
            Statement::Block(block) => {
                for inner in block.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
                false
            }
            Statement::If(if_stmt) => match &if_stmt.body {
                IfBody::Statement(body) => {
                    Self::try_resolve_in_closure_stmt(body.statement, ctx, results)
                }
                IfBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::Foreach(foreach) => match &foreach.body {
                ForeachBody::Statement(inner) => {
                    Self::try_resolve_in_closure_stmt(inner, ctx, results)
                }
                ForeachBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(inner) => {
                    Self::try_resolve_in_closure_stmt(inner, ctx, results)
                }
                WhileBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::For(for_stmt) => match &for_stmt.body {
                ForBody::Statement(inner) => Self::try_resolve_in_closure_stmt(inner, ctx, results),
                ForBody::ColonDelimited(body) => {
                    for inner in body.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                        {
                            return true;
                        }
                    }
                    false
                }
            },
            Statement::DoWhile(dw) => Self::try_resolve_in_closure_stmt(dw.statement, ctx, results),
            Statement::Try(try_stmt) => {
                for inner in try_stmt.block.statements.iter() {
                    let s = inner.span();
                    if ctx.cursor_offset >= s.start.offset
                        && ctx.cursor_offset <= s.end.offset
                        && Self::try_resolve_in_closure_stmt(inner, ctx, results)
                    {
                        return true;
                    }
                }
                for catch in try_stmt.catch_clauses.iter() {
                    for inner in catch.block.statements.iter() {
                        let s = inner.span();
                        if ctx.cursor_offset >= s.start.offset
                            && ctx.cursor_offset <= s.end.offset
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
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
                            && Self::try_resolve_in_closure_stmt(inner, ctx, results)
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

    /// Recursively search an expression tree for a `Closure` or
    /// `ArrowFunction` whose body contains the cursor.  When found,
    /// resolve the target variable from the closure's parameters and
    /// walk its body statements, returning `true`.
    pub(super) fn try_resolve_in_closure_expr<'b>(
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
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
                    Self::resolve_closure_params(&closure.parameter_list, ctx, results);
                    Self::walk_statements_for_assignments(
                        closure.body.statements.iter(),
                        ctx,
                        results,
                        false,
                    );
                    return true;
                }
                false
            }
            // ── Arrow function: `fn(Type $param) => expr` ──
            Expression::ArrowFunction(arrow) => {
                let arrow_body_span = arrow.expression.span();
                if ctx.cursor_offset >= arrow.arrow.start.offset
                    && ctx.cursor_offset <= arrow_body_span.end.offset
                {
                    Self::resolve_closure_params(&arrow.parameter_list, ctx, results);
                    // Arrow functions have a single expression body — no
                    // statements to walk, but the params are resolved.
                    return true;
                }
                false
            }
            // ── Recurse into sub-expressions that might contain closures ──
            Expression::Parenthesized(p) => {
                Self::try_resolve_in_closure_expr(p.expression, ctx, results)
            }
            Expression::Assignment(a) => {
                Self::try_resolve_in_closure_expr(a.lhs, ctx, results)
                    || Self::try_resolve_in_closure_expr(a.rhs, ctx, results)
            }
            Expression::Binary(bin) => {
                Self::try_resolve_in_closure_expr(bin.lhs, ctx, results)
                    || Self::try_resolve_in_closure_expr(bin.rhs, ctx, results)
            }
            Expression::Conditional(cond) => {
                Self::try_resolve_in_closure_expr(cond.condition, ctx, results)
                    || cond
                        .then
                        .is_some_and(|e| Self::try_resolve_in_closure_expr(e, ctx, results))
                    || Self::try_resolve_in_closure_expr(cond.r#else, ctx, results)
            }
            Expression::Call(call) => Self::try_resolve_in_closure_call(call, ctx, results),
            Expression::Array(arr) => {
                for elem in arr.elements.iter() {
                    let found = match elem {
                        ArrayElement::KeyValue(kv) => {
                            Self::try_resolve_in_closure_expr(kv.key, ctx, results)
                                || Self::try_resolve_in_closure_expr(kv.value, ctx, results)
                        }
                        ArrayElement::Value(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Variadic(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Missing(_) => false,
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
                            Self::try_resolve_in_closure_expr(kv.key, ctx, results)
                                || Self::try_resolve_in_closure_expr(kv.value, ctx, results)
                        }
                        ArrayElement::Value(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Variadic(v) => {
                            Self::try_resolve_in_closure_expr(v.value, ctx, results)
                        }
                        ArrayElement::Missing(_) => false,
                    };
                    if found {
                        return true;
                    }
                }
                false
            }
            Expression::Match(m) => {
                if Self::try_resolve_in_closure_expr(m.expression, ctx, results) {
                    return true;
                }
                for arm in m.arms.iter() {
                    if Self::try_resolve_in_closure_expr(arm.expression(), ctx, results) {
                        return true;
                    }
                }
                false
            }
            Expression::Access(access) => match access {
                Access::Property(pa) => Self::try_resolve_in_closure_expr(pa.object, ctx, results),
                Access::NullSafeProperty(pa) => {
                    Self::try_resolve_in_closure_expr(pa.object, ctx, results)
                }
                Access::StaticProperty(pa) => {
                    Self::try_resolve_in_closure_expr(pa.class, ctx, results)
                }
                Access::ClassConstant(pa) => {
                    Self::try_resolve_in_closure_expr(pa.class, ctx, results)
                }
            },
            Expression::Instantiation(inst) => {
                if let Some(ref args) = inst.argument_list {
                    Self::try_resolve_in_closure_args(&args.arguments, ctx, results)
                } else {
                    false
                }
            }
            Expression::UnaryPrefix(u) => {
                Self::try_resolve_in_closure_expr(u.operand, ctx, results)
            }
            Expression::UnaryPostfix(u) => {
                Self::try_resolve_in_closure_expr(u.operand, ctx, results)
            }
            Expression::Yield(y) => match y {
                Yield::Value(yv) => {
                    if let Some(val) = &yv.value {
                        Self::try_resolve_in_closure_expr(val, ctx, results)
                    } else {
                        false
                    }
                }
                Yield::Pair(yp) => {
                    Self::try_resolve_in_closure_expr(yp.key, ctx, results)
                        || Self::try_resolve_in_closure_expr(yp.value, ctx, results)
                }
                Yield::From(yf) => Self::try_resolve_in_closure_expr(yf.iterator, ctx, results),
            },
            Expression::Throw(t) => Self::try_resolve_in_closure_expr(t.exception, ctx, results),
            Expression::Clone(c) => Self::try_resolve_in_closure_expr(c.object, ctx, results),
            Expression::Pipe(p) => {
                Self::try_resolve_in_closure_expr(p.input, ctx, results)
                    || Self::try_resolve_in_closure_expr(p.callable, ctx, results)
            }
            _ => false,
        }
    }

    /// Check call-expression arguments for closures containing the cursor.
    fn try_resolve_in_closure_call<'b>(
        call: &'b Call<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) -> bool {
        match call {
            Call::Function(fc) => {
                Self::try_resolve_in_closure_args(&fc.argument_list.arguments, ctx, results)
            }
            Call::Method(mc) => {
                Self::try_resolve_in_closure_expr(mc.object, ctx, results)
                    || Self::try_resolve_in_closure_args(&mc.argument_list.arguments, ctx, results)
            }
            Call::NullSafeMethod(mc) => {
                Self::try_resolve_in_closure_expr(mc.object, ctx, results)
                    || Self::try_resolve_in_closure_args(&mc.argument_list.arguments, ctx, results)
            }
            Call::StaticMethod(sc) => {
                Self::try_resolve_in_closure_expr(sc.class, ctx, results)
                    || Self::try_resolve_in_closure_args(&sc.argument_list.arguments, ctx, results)
            }
        }
    }

    /// Check a list of arguments for closures containing the cursor.
    fn try_resolve_in_closure_args<'b>(
        arguments: &'b TokenSeparatedSequence<'b, Argument<'b>>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) -> bool {
        for arg in arguments.iter() {
            let arg_expr = match arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            if Self::try_resolve_in_closure_expr(arg_expr, ctx, results) {
                return true;
            }
        }
        false
    }

    /// Resolve a variable's type from a closure / arrow-function
    /// parameter list.  If the variable matches a typed parameter,
    /// the resolved classes replace whatever is currently in `results`.
    pub(super) fn resolve_closure_params(
        parameter_list: &FunctionLikeParameterList<'_>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        for param in parameter_list.parameters.iter() {
            let pname = param.variable.name.to_string();
            if pname == ctx.var_name {
                if let Some(hint) = &param.hint {
                    let type_str = Self::extract_hint_string(hint);
                    let resolved = Self::type_hint_to_classes(
                        &type_str,
                        &ctx.current_class.name,
                        ctx.all_classes,
                        ctx.class_loader,
                    );
                    if !resolved.is_empty() {
                        *results = resolved;
                    }
                }
                break;
            }
        }
    }
}
