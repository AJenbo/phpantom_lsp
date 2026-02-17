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
///   - Foreach value variables: when iterating over a variable annotated
///     with a generic iterable type (e.g. `@var list<User>`, `@param
///     list<User>`, `User[]`), the foreach value variable is resolved to
///     the element type.
///
/// Type narrowing (instanceof, assert, custom type guards) is delegated
/// to the [`super::type_narrowing`] module.  Closure/arrow-function scope
/// handling is delegated to [`super::closure_resolution`].
use bumpalo::Bump;
use mago_span::HasSpan;
use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;

use crate::Backend;
use crate::docblock;
use crate::types::ClassInfo;

use super::conditional_resolution::{resolve_conditional_with_args, split_call_subject};
use super::resolver::{CallResolutionCtx, FunctionLoaderFn, VarResolutionCtx};

impl Backend {
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
    pub(super) fn resolve_variable_types(
        var_name: &str,
        current_class: &ClassInfo,
        all_classes: &[ClassInfo],
        content: &str,
        cursor_offset: u32,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        function_loader: FunctionLoaderFn<'_>,
    ) -> Vec<ClassInfo> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = parse_file_content(&arena, file_id, content);

        let ctx = VarResolutionCtx {
            var_name,
            current_class,
            all_classes,
            content,
            cursor_offset,
            class_loader,
            function_loader,
        };

        // Walk top-level (and namespace-nested) statements to find the
        // class + method containing the cursor.
        Self::resolve_variable_in_statements(program.statements.iter(), &ctx)
    }

    pub(super) fn resolve_variable_in_statements<'b>(
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
                    let results = Self::resolve_variable_in_members(class.members.iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Interface(iface) => {
                    let start = iface.left_brace.start.offset;
                    let end = iface.right_brace.end.offset;
                    if ctx.cursor_offset < start || ctx.cursor_offset > end {
                        continue;
                    }
                    let results = Self::resolve_variable_in_members(iface.members.iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Enum(enum_def) => {
                    let start = enum_def.left_brace.start.offset;
                    let end = enum_def.right_brace.end.offset;
                    if ctx.cursor_offset < start || ctx.cursor_offset > end {
                        continue;
                    }
                    let results = Self::resolve_variable_in_members(enum_def.members.iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                Statement::Namespace(ns) => {
                    let results = Self::resolve_variable_in_statements(ns.statements().iter(), ctx);
                    if !results.is_empty() {
                        return results;
                    }
                }
                // ── Top-level function declarations ──
                // If the cursor is inside a `function foo(Type $p) { … }`
                // at the top level, resolve the variable from its params
                // and walk its body.
                Statement::Function(func) => {
                    let body_start = func.body.left_brace.start.offset;
                    let body_end = func.body.right_brace.end.offset;
                    if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                        let mut results: Vec<ClassInfo> = Vec::new();
                        Self::resolve_closure_params(&func.parameter_list, ctx, &mut results);
                        Self::walk_statements_for_assignments(
                            func.body.statements.iter(),
                            ctx,
                            &mut results,
                            false,
                        );
                        if !results.is_empty() {
                            return results;
                        }
                    }
                }
                _ => {}
            }
        }

        // The cursor is not inside any class/interface/enum body — it must
        // be in top-level code.  Walk all top-level statements to find
        // variable assignments (e.g. `$user = new User(…);`).
        let mut results: Vec<ClassInfo> = Vec::new();
        Self::walk_statements_for_assignments(stmts.into_iter(), ctx, &mut results, false);
        results
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
                    if pname == ctx.var_name
                        && let Some(hint) = &param.hint
                    {
                        let type_str = Self::extract_hint_string(hint);
                        let resolved = Self::type_hint_to_classes(
                            &type_str,
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

                if let MethodBody::Concrete(block) = &method.body {
                    let blk_start = block.left_brace.start.offset;
                    let blk_end = block.right_brace.end.offset;
                    if ctx.cursor_offset >= blk_start && ctx.cursor_offset <= blk_end {
                        // Seed the result set with the parameter type hint
                        // (if any) so that instanceof narrowing and
                        // unconditional reassignments can refine it.
                        let mut results = param_results.clone();
                        Self::walk_statements_for_assignments(
                            block.statements.iter(),
                            ctx,
                            &mut results,
                            false,
                        );
                        if !results.is_empty() {
                            return results;
                        }
                    }
                }

                // Cursor is outside the method body — return the
                // parameter type hint as-is (no body to narrow in).
                if !param_results.is_empty() {
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
    pub(super) fn walk_statements_for_assignments<'b>(
        statements: impl Iterator<Item = &'b Statement<'b>>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        for stmt in statements {
            // ── Closure / arrow-function scope ──
            // If the cursor falls *inside* this statement, check whether
            // it is (or contains) a closure / arrow function whose body
            // encloses the cursor.  Closures introduce a new variable
            // scope, so we resolve entirely within that scope and stop.
            let stmt_span = stmt.span();
            if ctx.cursor_offset >= stmt_span.start.offset
                && ctx.cursor_offset <= stmt_span.end.offset
                && Self::try_resolve_in_closure_stmt(stmt, ctx, results)
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
                    if !Self::try_inline_var_override(
                        expr_stmt.expression,
                        stmt.span().start.offset as usize,
                        ctx,
                        results,
                        conditional,
                    ) {
                        Self::check_expression_for_assignment(
                            expr_stmt.expression,
                            ctx,
                            results,
                            conditional,
                        );
                    }

                    // ── assert($var instanceof ClassName) narrowing ──
                    // When `assert($var instanceof Foo)` appears before
                    // the cursor, narrow the variable to `Foo` for the
                    // remainder of the current scope.
                    Self::try_apply_assert_instanceof_narrowing(expr_stmt.expression, ctx, results);

                    // ── @phpstan-assert / @psalm-assert narrowing ──
                    // When a function with `@phpstan-assert Type $param`
                    // is called as a standalone statement, narrow the
                    // corresponding argument variable unconditionally.
                    Self::try_apply_custom_assert_narrowing(expr_stmt.expression, ctx, results);

                    // ── match(true) { $var instanceof Foo => … } narrowing ──
                    Self::try_apply_match_true_narrowing(expr_stmt.expression, ctx, results);
                }
                // Recurse into blocks — these are just `{ … }` groupings,
                // not conditional, so preserve the current `conditional` flag.
                Statement::Block(block) => {
                    Self::walk_statements_for_assignments(
                        block.statements.iter(),
                        ctx,
                        results,
                        conditional,
                    );
                }
                // ── Conditional blocks: recurse with conditional = true ──
                //
                // When the condition is `$var instanceof ClassName` and the
                // cursor falls inside the corresponding branch body, we
                // *narrow* the variable to only that class — replacing all
                // previous candidates.
                Statement::If(if_stmt) => match &if_stmt.body {
                    IfBody::Statement(body) => {
                        // ── instanceof narrowing for then-body ──
                        Self::try_apply_instanceof_narrowing(
                            if_stmt.condition,
                            body.statement.span(),
                            ctx,
                            results,
                        );
                        // ── @phpstan-assert-if-true/false narrowing for then-body ──
                        Self::try_apply_assert_condition_narrowing(
                            if_stmt.condition,
                            body.statement.span(),
                            ctx,
                            results,
                            false, // not inverted — this is the then-body
                        );
                        Self::check_statement_for_assignments(body.statement, ctx, results, true);

                        for else_if in body.else_if_clauses.iter() {
                            // ── instanceof narrowing for elseif-body ──
                            Self::try_apply_instanceof_narrowing(
                                else_if.condition,
                                else_if.statement.span(),
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                else_if.condition,
                                else_if.statement.span(),
                                ctx,
                                results,
                                false,
                            );
                            Self::check_statement_for_assignments(
                                else_if.statement,
                                ctx,
                                results,
                                true,
                            );
                        }
                        if let Some(else_clause) = &body.else_clause {
                            // ── inverse instanceof narrowing for else-body ──
                            // `if ($v instanceof Foo) { … } else { ← here }`
                            // means $v is NOT Foo in the else branch.
                            Self::try_apply_instanceof_narrowing_inverse(
                                if_stmt.condition,
                                else_clause.statement.span(),
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                if_stmt.condition,
                                else_clause.statement.span(),
                                ctx,
                                results,
                                true, // inverted — this is the else-body
                            );
                            Self::check_statement_for_assignments(
                                else_clause.statement,
                                ctx,
                                results,
                                true,
                            );
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
                        Self::try_apply_instanceof_narrowing(
                            if_stmt.condition,
                            then_span,
                            ctx,
                            results,
                        );
                        Self::try_apply_assert_condition_narrowing(
                            if_stmt.condition,
                            then_span,
                            ctx,
                            results,
                            false,
                        );
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                        for else_if in body.else_if_clauses.iter() {
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
                            Self::try_apply_instanceof_narrowing(
                                else_if.condition,
                                ei_span,
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                else_if.condition,
                                ei_span,
                                ctx,
                                results,
                                false,
                            );
                            Self::walk_statements_for_assignments(
                                else_if.statements.iter(),
                                ctx,
                                results,
                                true,
                            );
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
                            Self::try_apply_instanceof_narrowing_inverse(
                                if_stmt.condition,
                                else_span,
                                ctx,
                                results,
                            );
                            Self::try_apply_assert_condition_narrowing(
                                if_stmt.condition,
                                else_span,
                                ctx,
                                results,
                                true, // inverted — else-body
                            );
                            Self::walk_statements_for_assignments(
                                else_clause.statements.iter(),
                                ctx,
                                results,
                                true,
                            );
                        }
                    }
                },
                Statement::Foreach(foreach) => {
                    // Only resolve the foreach value variable and recurse
                    // into the body when the cursor is actually inside it.
                    // Outside the loop the iteration variables are out of
                    // scope.
                    let body_span = foreach.body.span();
                    if ctx.cursor_offset >= body_span.start.offset
                        && ctx.cursor_offset <= body_span.end.offset
                    {
                        // ── Foreach value type from generic iterables ──
                        // When the variable we're resolving is the foreach
                        // *value* variable, try to infer its type from the
                        // iterated expression's generic type annotation.
                        //
                        // Example:
                        //   /** @var list<User> $users */
                        //   foreach ($users as $user) { $user-> }
                        //
                        // Here `$user` is resolved to `User`.
                        Self::try_resolve_foreach_value_type(foreach, ctx, results, conditional);

                        match &foreach.body {
                            ForeachBody::Statement(inner) => {
                                Self::check_statement_for_assignments(inner, ctx, results, true);
                            }
                            ForeachBody::ColonDelimited(body) => {
                                Self::walk_statements_for_assignments(
                                    body.statements.iter(),
                                    ctx,
                                    results,
                                    true,
                                );
                            }
                        }
                    }
                }
                Statement::While(while_stmt) => match &while_stmt.body {
                    WhileBody::Statement(inner) => {
                        // ── instanceof narrowing for while-body ──
                        Self::try_apply_instanceof_narrowing(
                            while_stmt.condition,
                            inner.span(),
                            ctx,
                            results,
                        );
                        Self::try_apply_assert_condition_narrowing(
                            while_stmt.condition,
                            inner.span(),
                            ctx,
                            results,
                            false,
                        );
                        Self::check_statement_for_assignments(inner, ctx, results, true);
                    }
                    WhileBody::ColonDelimited(body) => {
                        let body_span = mago_span::Span::new(
                            body.colon.file_id,
                            body.colon.start,
                            mago_span::Position::new(body.end_while.span().start.offset),
                        );
                        Self::try_apply_instanceof_narrowing(
                            while_stmt.condition,
                            body_span,
                            ctx,
                            results,
                        );
                        Self::try_apply_assert_condition_narrowing(
                            while_stmt.condition,
                            body_span,
                            ctx,
                            results,
                            false,
                        );
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                },
                Statement::For(for_stmt) => match &for_stmt.body {
                    ForBody::Statement(inner) => {
                        Self::check_statement_for_assignments(inner, ctx, results, true);
                    }
                    ForBody::ColonDelimited(body) => {
                        Self::walk_statements_for_assignments(
                            body.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                },
                Statement::DoWhile(dw) => {
                    Self::check_statement_for_assignments(dw.statement, ctx, results, true);
                }
                Statement::Try(try_stmt) => {
                    Self::walk_statements_for_assignments(
                        try_stmt.block.statements.iter(),
                        ctx,
                        results,
                        true,
                    );
                    for catch in try_stmt.catch_clauses.iter() {
                        Self::walk_statements_for_assignments(
                            catch.block.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                    if let Some(finally) = &try_stmt.finally_clause {
                        Self::walk_statements_for_assignments(
                            finally.block.statements.iter(),
                            ctx,
                            results,
                            true,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Helper: treat a single statement as an iterator of one and recurse.
    /// Try to resolve the foreach value variable's type from a generic
    /// iterable annotation on the iterated expression.
    ///
    /// When the variable being resolved (`ctx.var_name`) matches the
    /// foreach value variable and the iterated expression is a simple
    /// `$variable` whose type is annotated as a generic iterable (via
    /// `@var list<User> $var` or `@param list<User> $var`), this method
    /// extracts the element type and pushes the resolved `ClassInfo` into
    /// `results`.
    fn try_resolve_foreach_value_type<'b>(
        foreach: &'b Foreach<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        // Check if the foreach value variable is the one we're resolving.
        let value_expr = foreach.target.value();
        let value_var_name = match value_expr {
            Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
            _ => return,
        };
        if value_var_name != ctx.var_name {
            return;
        }

        // Extract the iterated expression's source text.
        let expr_span = foreach.expression.span();
        let expr_start = expr_span.start.offset as usize;
        let expr_end = expr_span.end.offset as usize;
        let expr_text = match ctx.content.get(expr_start..expr_end) {
            Some(t) => t.trim(),
            None => return,
        };

        // Currently we handle simple `$variable` expressions.
        if !expr_text.starts_with('$') || expr_text.contains("->") || expr_text.contains("::") {
            return;
        }

        // Search backward from the foreach for @var or @param annotations
        // on the iterated variable that include a generic type.
        let foreach_offset = foreach.foreach.span().start.offset as usize;
        let raw_type = match docblock::find_iterable_raw_type_in_source(
            ctx.content,
            foreach_offset,
            expr_text,
        ) {
            Some(t) => t,
            None => return,
        };

        // Extract the generic element type (e.g. `list<User>` → `User`).
        let element_type = match docblock::types::extract_generic_value_type(&raw_type) {
            Some(t) => t,
            None => return,
        };

        // Resolve the element type to ClassInfo.
        let resolved = Self::type_hint_to_classes(
            &element_type,
            &ctx.current_class.name,
            ctx.all_classes,
            ctx.class_loader,
        );

        if resolved.is_empty() {
            return;
        }

        if !conditional {
            results.clear();
        }
        for cls in resolved {
            if !results.iter().any(|c| c.name == cls.name) {
                results.push(cls);
            }
        }
    }

    pub(super) fn check_statement_for_assignments<'b>(
        stmt: &'b Statement<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        Self::walk_statements_for_assignments(std::iter::once(stmt), ctx, results, conditional);
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
    pub(super) fn try_inline_var_override<'b>(
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
        let (var_type, var_name) = match docblock::find_inline_var_docblock(ctx.content, stmt_start)
        {
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
        let native_type = Self::extract_native_type_from_rhs(assignment.rhs, ctx);
        let effective = docblock::resolve_effective_type(native_type.as_deref(), Some(&var_type));

        let eff_type = match effective {
            Some(t) => t,
            None => return false,
        };

        let resolved = Self::type_hint_to_classes(
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
                            .cloned()
                            .or_else(|| (ctx.class_loader)(&cls_name));
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
    pub(super) fn check_expression_for_assignment<'b>(
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        let var_name = ctx.var_name;
        let current_class_name: &str = &ctx.current_class.name;
        let all_classes = ctx.all_classes;
        let content = ctx.content;
        let class_loader = ctx.class_loader;
        let function_loader = ctx.function_loader;
        /// Push one or more resolved classes into `results`.
        ///
        /// * `conditional == false` → unconditional assignment: **clear**
        ///   previous candidates first, then add all new ones (handles
        ///   union return types like `A|B` from a single assignment).
        /// * `conditional == true` → conditional branch: **append**
        ///   without clearing (the variable *might* be these types).
        ///
        /// Duplicates (same class name) are always suppressed.
        fn push_results(
            results: &mut Vec<ClassInfo>,
            new_classes: Vec<ClassInfo>,
            conditional: bool,
        ) {
            if new_classes.is_empty() {
                return;
            }
            if !conditional {
                results.clear();
            }
            for cls in new_classes {
                if !results.iter().any(|c| c.name == cls.name) {
                    results.push(cls);
                }
            }
        }

        if let Expression::Assignment(assignment) = expr {
            if !assignment.operator.is_assign() {
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
            // Check RHS is a `new …`
            if let Expression::Instantiation(inst) = assignment.rhs {
                let class_name = match inst.class {
                    Expression::Self_(_) => Some("self"),
                    Expression::Static(_) => Some("static"),
                    Expression::Identifier(ident) => Some(ident.value()),
                    _ => None,
                };
                if let Some(name) = class_name {
                    let resolved = Self::type_hint_to_classes(
                        name,
                        current_class_name,
                        all_classes,
                        class_loader,
                    );
                    push_results(results, resolved, conditional);
                }
                return;
            }
            // Check RHS is an array access: `$var = $arr[0]` or `$var = $arr[$key]`
            // Resolve the base array's generic/iterable type and extract
            // the element type.
            if let Expression::ArrayAccess(array_access) = assignment.rhs {
                // The base expression must be a simple variable (e.g. `$admins`).
                if let Expression::Variable(Variable::Direct(base_dv)) = array_access.array {
                    let base_var = base_dv.name.to_string();
                    // Search backward from this assignment for a @var/@param
                    // annotation on the base variable with a generic type.
                    let assign_offset = assignment.span().start.offset as usize;
                    if let Some(raw_type) = docblock::find_iterable_raw_type_in_source(
                        content,
                        assign_offset,
                        &base_var,
                    ) && let Some(element_type) =
                        docblock::types::extract_generic_value_type(&raw_type)
                    {
                        let resolved = Self::type_hint_to_classes(
                            &element_type,
                            current_class_name,
                            all_classes,
                            class_loader,
                        );
                        push_results(results, resolved, conditional);
                    }
                }
                return;
            }
            // Check RHS is a function call: `$var = someFunction(…)`
            // Look up the function's return type and resolve to a class.
            if let Expression::Call(call) = assignment.rhs {
                match call {
                    Call::Function(func_call) => {
                        // Extract the function name from the call target
                        let func_name = match func_call.function {
                            Expression::Identifier(ident) => Some(ident.value().to_string()),
                            _ => None,
                        };
                        if let Some(name) = func_name
                            && let Some(fl) = function_loader
                            && let Some(func_info) = fl(&name)
                        {
                            // Try conditional return type first (PHPStan syntax)
                            let mut handled = false;
                            if let Some(ref cond) = func_info.conditional_return {
                                let resolved_type = resolve_conditional_with_args(
                                    cond,
                                    &func_info.parameters,
                                    &func_call.argument_list,
                                );
                                if let Some(ref ty) = resolved_type {
                                    let resolved = Self::type_hint_to_classes(
                                        ty,
                                        current_class_name,
                                        all_classes,
                                        class_loader,
                                    );
                                    if !resolved.is_empty() {
                                        push_results(results, resolved, conditional);
                                        handled = true;
                                    }
                                }
                            }
                            if !handled && let Some(ref ret) = func_info.return_type {
                                let resolved = Self::type_hint_to_classes(
                                    ret,
                                    current_class_name,
                                    all_classes,
                                    class_loader,
                                );
                                push_results(results, resolved, conditional);
                            }
                        }
                    }
                    Call::Method(method_call) => {
                        // `$var = $obj->method()` — resolve the object, find the
                        // method, and use its return type.
                        //
                        // We handle the common `$this->method()` case via the
                        // AST, and for everything else (chained calls like
                        // `app()->make(...)`, `$factory->create()`, etc.) we
                        // fall back to text-based resolution which already
                        // handles arbitrary nesting.
                        if let Expression::Variable(Variable::Direct(dv)) = method_call.object
                            && dv.name == "$this"
                            && let ClassLikeMemberSelector::Identifier(ident) = &method_call.method
                        {
                            let method_name = ident.value.to_string();
                            if let Some(owner) =
                                all_classes.iter().find(|c| c.name == current_class_name)
                            {
                                let resolved = Self::resolve_method_return_types(
                                    owner,
                                    &method_name,
                                    all_classes,
                                    class_loader,
                                );
                                push_results(results, resolved, conditional);
                            }
                        } else {
                            // General case: extract the RHS call expression text
                            // from the source and delegate to the text-based
                            // resolver that already handles chains like
                            // `app()->make(Foo::class)`, `$obj->method()`, etc.
                            let rhs_span = assignment.rhs.span();
                            let start = rhs_span.start.offset as usize;
                            let end = rhs_span.end.offset as usize;
                            if end <= content.len() {
                                let rhs_text = content[start..end].trim();
                                if rhs_text.ends_with(')')
                                    && let Some((call_body, args_text)) =
                                        split_call_subject(rhs_text)
                                {
                                    let current_class =
                                        all_classes.iter().find(|c| c.name == current_class_name);
                                    let call_ctx = CallResolutionCtx {
                                        current_class,
                                        all_classes,
                                        content,
                                        cursor_offset: ctx.cursor_offset,
                                        class_loader,
                                        function_loader,
                                    };
                                    let resolved = Self::resolve_call_return_types(
                                        call_body, args_text, &call_ctx,
                                    );
                                    push_results(results, resolved, conditional);
                                }
                            }
                        }
                    }
                    Call::StaticMethod(static_call) => {
                        // `$var = ClassName::method()` — resolve the class, find the
                        // method, and use its return type.
                        let class_name = match static_call.class {
                            Expression::Self_(_) => Some(current_class_name.to_string()),
                            Expression::Static(_) => Some(current_class_name.to_string()),
                            Expression::Identifier(ident) => Some(ident.value().to_string()),
                            _ => None,
                        };
                        if let Some(cls_name) = class_name
                            && let ClassLikeMemberSelector::Identifier(ident) = &static_call.method
                        {
                            let method_name = ident.value.to_string();
                            let owner = all_classes
                                .iter()
                                .find(|c| c.name == cls_name)
                                .cloned()
                                .or_else(|| class_loader(&cls_name));
                            if let Some(ref owner) = owner {
                                let resolved = Self::resolve_method_return_types(
                                    owner,
                                    &method_name,
                                    all_classes,
                                    class_loader,
                                );
                                push_results(results, resolved, conditional);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
