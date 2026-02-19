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
/// to the [`super::type_narrowing`] module.  Closure/arrow-function scope
/// handling is delegated to [`super::closure_resolution`].
use std::panic;

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
        // Wrap in catch_unwind so a mago-syntax parser panic doesn't
        // crash the LSP server (producing a zombie process).
        let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
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
        }));

        match result {
            Ok(classes) => classes,
            Err(_) => {
                log::error!(
                    "PHPantomLSP: parser panicked during variable resolution for '{}'",
                    var_name
                );
                vec![]
            }
        }
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
                    if pname == ctx.var_name {
                        // Try the native AST type hint first.
                        let native_type_str =
                            param.hint.as_ref().map(|h| Self::extract_hint_string(h));

                        let resolved_from_native = native_type_str
                            .as_deref()
                            .map(|ts| {
                                Self::type_hint_to_classes(
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
                            let resolved = Self::type_hint_to_classes(
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

                    // ── ternary instanceof narrowing ──
                    // `$var instanceof Foo ? $var->method() : …`
                    // When the cursor is inside a ternary whose condition
                    // checks instanceof, narrow accordingly.
                    Self::try_apply_ternary_instanceof_narrowing(
                        expr_stmt.expression,
                        ctx,
                        results,
                    );
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
                Statement::If(if_stmt) => {
                    match &if_stmt.body {
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
                            Self::check_statement_for_assignments(
                                body.statement,
                                ctx,
                                results,
                                true,
                            );

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
                    if stmt.span().end.offset < ctx.cursor_offset {
                        Self::apply_guard_clause_narrowing(if_stmt, ctx, results);
                    }
                }
                Statement::Foreach(foreach) => {
                    // Only resolve the foreach value variable and recurse
                    // into the body when the cursor is actually inside it.
                    // Outside the loop the iteration variables are out of
                    // scope.
                    let body_span = foreach.body.span();
                    if ctx.cursor_offset >= body_span.start.offset
                        && ctx.cursor_offset <= body_span.end.offset
                    {
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
                        Self::try_resolve_foreach_value_type(foreach, ctx, results, conditional);
                        Self::try_resolve_foreach_key_type(foreach, ctx, results, conditional);

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
                        // Seed the catch variable's type from the catch
                        // clause's type hint(s) before recursing into the
                        // block.  Handles single types like
                        // `catch (ValidationException $e)` and multi-catch
                        // like `catch (TypeA | TypeB $e)`.
                        if let Some(ref var) = catch.variable
                            && var.name == ctx.var_name
                        {
                            let hint_str = Self::extract_hint_string(&catch.hint);
                            let resolved = Self::type_hint_to_classes(
                                &hint_str,
                                &ctx.current_class.name,
                                ctx.all_classes,
                                ctx.class_loader,
                            );
                            ClassInfo::extend_unique(results, resolved);
                        }
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

        // Try to extract the raw iterable type from the foreach expression.
        // `extract_rhs_iterable_raw_type` handles method calls, static
        // calls, property access, function calls, and simple variables.
        let raw_type = Self::extract_rhs_iterable_raw_type(foreach.expression, ctx).or_else(|| {
            // Fallback: for simple `$variable` expressions, search backward
            // from the foreach for @var or @param annotations.
            let expr_span = foreach.expression.span();
            let expr_start = expr_span.start.offset as usize;
            let expr_end = expr_span.end.offset as usize;
            let expr_text = ctx.content.get(expr_start..expr_end)?.trim();

            if !expr_text.starts_with('$') || expr_text.contains("->") || expr_text.contains("::") {
                return None;
            }

            let foreach_offset = foreach.foreach.span().start.offset as usize;
            docblock::find_iterable_raw_type_in_source(ctx.content, foreach_offset, expr_text)
        });

        // Extract the generic element type (e.g. `list<User>` → `User`).
        if let Some(ref rt) = raw_type
            && let Some(element_type) = docblock::types::extract_generic_value_type(rt)
        {
            Self::push_foreach_resolved_types(&element_type, ctx, results, conditional);
            return;
        }

        // ── Fallback: resolve the iterated expression to ClassInfo and
        //    extract the value type from its generic annotations ─────────
        //
        // This handles cases where the iterated expression resolves to a
        // concrete collection class (e.g. `$items = new UserCollection()`)
        // whose `@extends` or `@implements` annotations carry the generic
        // type parameters, but no inline `@var` annotation is present.
        //
        // Also handles the case where a method/property returns a class
        // name like `PaymentOptionLocaleCollection` without generic syntax
        // in the return type string.
        let iterable_classes = if let Some(ref rt) = raw_type {
            // raw_type is a class name like "PaymentOptionLocaleCollection"
            // (extract_generic_value_type returned None above).
            Self::type_hint_to_classes(
                rt,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            )
        } else {
            // No raw type at all — resolve the foreach expression as a
            // subject string via variable / assignment scanning.
            Self::resolve_foreach_expression_to_classes(foreach.expression, ctx)
        };

        for cls in &iterable_classes {
            let merged = Self::resolve_class_with_inheritance(cls, ctx.class_loader);
            if let Some(value_type) = Self::extract_iterable_element_type_from_class(&merged) {
                Self::push_foreach_resolved_types(&value_type, ctx, results, conditional);
                return;
            }
        }
    }

    /// Try to resolve the foreach **key** variable's type from a generic
    /// iterable annotation on the iterated expression.
    ///
    /// When the variable being resolved (`ctx.var_name`) matches the
    /// foreach key variable and the iterated expression is a simple
    /// `$variable` whose type is annotated as a two-parameter generic
    /// iterable (via `@var array<Request, Response> $var` or similar),
    /// this method extracts the key type and pushes the resolved
    /// `ClassInfo` into `results`.
    ///
    /// For common scalar key types (`int`, `string`), no `ClassInfo` is
    /// produced — which is correct because scalars have no members to
    /// complete on.
    fn try_resolve_foreach_key_type<'b>(
        foreach: &'b Foreach<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        // Check if the foreach has a key variable and if it matches what
        // we're resolving.
        let key_expr = match foreach.target.key() {
            Some(expr) => expr,
            None => return,
        };
        let key_var_name = match key_expr {
            Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
            _ => return,
        };
        if key_var_name != ctx.var_name {
            return;
        }

        // Try to extract the raw iterable type from the foreach expression.
        // `extract_rhs_iterable_raw_type` handles method calls, static
        // calls, property access, function calls, and simple variables.
        let raw_type = Self::extract_rhs_iterable_raw_type(foreach.expression, ctx).or_else(|| {
            // Fallback: for simple `$variable` expressions, search backward
            // from the foreach for @var or @param annotations.
            let expr_span = foreach.expression.span();
            let expr_start = expr_span.start.offset as usize;
            let expr_end = expr_span.end.offset as usize;
            let expr_text = ctx.content.get(expr_start..expr_end)?.trim();

            if !expr_text.starts_with('$') || expr_text.contains("->") || expr_text.contains("::") {
                return None;
            }

            let foreach_offset = foreach.foreach.span().start.offset as usize;
            docblock::find_iterable_raw_type_in_source(ctx.content, foreach_offset, expr_text)
        });

        // Extract the generic key type (e.g. `array<Request, Response>` → `Request`).
        if let Some(ref rt) = raw_type
            && let Some(key_type) = docblock::types::extract_generic_key_type(rt)
        {
            Self::push_foreach_resolved_types(&key_type, ctx, results, conditional);
            return;
        }

        // ── Fallback: resolve the iterated expression to ClassInfo and
        //    extract the key type from its generic annotations ───────────
        let iterable_classes = if let Some(ref rt) = raw_type {
            Self::type_hint_to_classes(
                rt,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            )
        } else {
            Self::resolve_foreach_expression_to_classes(foreach.expression, ctx)
        };

        for cls in &iterable_classes {
            let merged = Self::resolve_class_with_inheritance(cls, ctx.class_loader);
            if let Some(key_type) = Self::extract_iterable_key_type_from_class(&merged) {
                Self::push_foreach_resolved_types(&key_type, ctx, results, conditional);
                return;
            }
        }
    }

    /// Push resolved foreach element types into the results list.
    ///
    /// Shared by both value and key foreach resolution paths: resolves a
    /// type string to `ClassInfo`(s) and merges them into `results`.
    fn push_foreach_resolved_types(
        type_str: &str,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        let resolved = Self::type_hint_to_classes(
            type_str,
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

    /// Resolve the foreach iterated expression to `ClassInfo`(s).
    ///
    /// Extracts the source text of the expression and resolves it using
    /// `resolve_target_classes`, which handles `$variable`, `$this->prop`,
    /// method calls, etc.
    fn resolve_foreach_expression_to_classes<'b>(
        expression: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        let expr_span = expression.span();
        let expr_start = expr_span.start.offset as usize;
        let expr_end = expr_span.end.offset as usize;
        let expr_text = match ctx.content.get(expr_start..expr_end) {
            Some(t) => t.trim(),
            None => return vec![],
        };

        if expr_text.is_empty() {
            return vec![];
        }

        Self::resolve_target_classes(
            expr_text,
            crate::types::AccessKind::Arrow,
            Some(ctx.current_class),
            ctx.all_classes,
            ctx.content,
            ctx.cursor_offset,
            ctx.class_loader,
            ctx.function_loader,
        )
    }

    /// Known interface/class names whose generic parameters describe
    /// iteration types in PHP's `foreach`.
    const ITERABLE_IFACE_NAMES: &'static [&'static str] = &[
        "Iterator",
        "IteratorAggregate",
        "Traversable",
        "ArrayAccess",
        "Enumerable",
    ];

    /// Extract the iterable **value** (element) type from a class's generic
    /// annotations.
    ///
    /// When a collection class like `PaymentOptionLocaleCollection` has
    /// `@extends Collection<int, PaymentOptionLocale>` or
    /// `@implements IteratorAggregate<int, PaymentOptionLocale>`, this
    /// function returns `Some("PaymentOptionLocale")`.
    ///
    /// Checks (in order of priority):
    /// 1. `implements_generics` for known iterable interfaces
    /// 2. `extends_generics` for any parent with generic type args
    ///
    /// Returns `None` when no generic iterable annotation is found or
    /// when the element type is a scalar (scalars have no completable
    /// members).
    fn extract_iterable_element_type_from_class(class: &ClassInfo) -> Option<String> {
        // 1. Check implements_generics for known iterable interfaces.
        for (name, args) in &class.implements_generics {
            let short = name.rsplit('\\').next().unwrap_or(name);
            if Self::ITERABLE_IFACE_NAMES.contains(&short) && !args.is_empty() {
                let value = args.last().unwrap();
                if !docblock::types::is_scalar(value) {
                    return Some(value.clone());
                }
            }
        }

        // 2. Check extends_generics — common for collection subclasses
        //    like `@extends Collection<int, User>`.
        for (_, args) in &class.extends_generics {
            if !args.is_empty() {
                let value = args.last().unwrap();
                if !docblock::types::is_scalar(value) {
                    return Some(value.clone());
                }
            }
        }

        None
    }

    /// Extract the iterable **key** type from a class's generic annotations.
    ///
    /// For two-parameter generics (e.g. `@implements ArrayAccess<int, User>`),
    /// returns the first parameter (`"int"`).
    ///
    /// Returns `None` when no suitable annotation is found or when only a
    /// single type parameter is present (single-param generics have an
    /// implicit `int` key which is scalar).
    fn extract_iterable_key_type_from_class(class: &ClassInfo) -> Option<String> {
        // 1. Check implements_generics for known iterable interfaces.
        for (name, args) in &class.implements_generics {
            let short = name.rsplit('\\').next().unwrap_or(name);
            if Self::ITERABLE_IFACE_NAMES.contains(&short) && args.len() >= 2 {
                let key = &args[0];
                if !docblock::types::is_scalar(key) {
                    return Some(key.clone());
                }
            }
        }

        // 2. Check extends_generics.
        for (_, args) in &class.extends_generics {
            if args.len() >= 2 {
                let key = &args[0];
                if !docblock::types::is_scalar(key) {
                    return Some(key.clone());
                }
            }
        }

        None
    }

    pub(super) fn check_statement_for_assignments<'b>(
        stmt: &'b Statement<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        Self::walk_statements_for_assignments(std::iter::once(stmt), ctx, results, conditional);
    }

    /// Check whether the target variable appears inside an array/list
    /// destructuring LHS and, if so, resolve its type from the RHS's
    /// generic element type or array shape entry.
    ///
    /// Supported patterns:
    ///   - `[$a, $b] = getUsers()`           — function call RHS (generic)
    ///   - `list($a, $b) = $users`           — variable RHS with `@var`/`@param`
    ///   - `[$a, $b] = $this->m()`           — method/static-method call RHS
    ///   - `['user' => $p] = $data`          — named key from array shape
    ///   - `[0 => $first, 1 => $second] = $data` — numeric key from array shape
    ///
    /// When the RHS type is an array shape (`array{key: Type, …}`), the
    /// destructured variable's key is matched against the shape entries.
    /// For positional (value-only) elements, the 0-based index is used as
    /// the key.  Falls back to `extract_generic_value_type` for generic
    /// iterable types (`list<User>`, `array<int, User>`, `User[]`).
    fn try_resolve_destructured_type<'b>(
        assignment: &'b Assignment<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        // ── 1. Collect the elements from the LHS ────────────────────────
        let elements = match assignment.lhs {
            Expression::Array(arr) => &arr.elements,
            Expression::List(list) => &list.elements,
            _ => return,
        };

        // ── 2. Find our target variable and extract its destructuring key
        //
        // For `KeyValue` elements like `'user' => $person`, extract the
        // string/integer key.  For positional `Value` elements, track
        // the 0-based index so we can look up positional shape entries.
        let var_name = ctx.var_name;
        let mut shape_key: Option<String> = None;
        let mut found = false;
        let mut positional_index: usize = 0;

        for elem in elements.iter() {
            match elem {
                ArrayElement::KeyValue(kv) => {
                    if let Expression::Variable(Variable::Direct(dv)) = kv.value
                        && dv.name == var_name
                    {
                        found = true;
                        // Extract the key from the LHS expression.
                        shape_key = Self::extract_destructuring_key(kv.key);
                        break;
                    }
                }
                ArrayElement::Value(val) => {
                    if let Expression::Variable(Variable::Direct(dv)) = val.value
                        && dv.name == var_name
                    {
                        found = true;
                        // Use the positional index as the shape key.
                        shape_key = Some(positional_index.to_string());
                        break;
                    }
                    positional_index += 1;
                }
                _ => {}
            }
        }
        if !found {
            return;
        }

        let current_class_name: &str = &ctx.current_class.name;
        let all_classes = ctx.all_classes;
        let content = ctx.content;
        let class_loader = ctx.class_loader;

        // ── 3. Try inline `/** @var … */` annotation ────────────────────
        // Handles both:
        //   `/** @var list<User> */`             (no variable name)
        //   `/** @var array{user: User} $data */` (with variable name)
        let stmt_offset = assignment.span().start.offset as usize;
        if let Some((var_type, _var_name_opt)) =
            docblock::find_inline_var_docblock(content, stmt_offset)
        {
            if let Some(ref key) = shape_key
                && let Some(entry_type) =
                    docblock::types::extract_array_shape_value_type(&var_type, key)
            {
                let resolved = Self::type_hint_to_classes(
                    &entry_type,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    if !conditional {
                        results.clear();
                    }
                    for cls in resolved {
                        if !results.iter().any(|c| c.name == cls.name) {
                            results.push(cls);
                        }
                    }
                    return;
                }
            }

            if let Some(element_type) = docblock::types::extract_generic_value_type(&var_type) {
                let resolved = Self::type_hint_to_classes(
                    &element_type,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    if !conditional {
                        results.clear();
                    }
                    for cls in resolved {
                        if !results.iter().any(|c| c.name == cls.name) {
                            results.push(cls);
                        }
                    }
                    return;
                }
            }
        }

        // ── 4. Try to extract the raw iterable type from the RHS ────────
        let raw_type: Option<String> = Self::extract_rhs_iterable_raw_type(assignment.rhs, ctx);

        if let Some(ref raw) = raw_type {
            // First try array shape lookup with the destructured key.
            if let Some(ref key) = shape_key
                && let Some(entry_type) = docblock::types::extract_array_shape_value_type(raw, key)
            {
                let resolved = Self::type_hint_to_classes(
                    &entry_type,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    if !conditional {
                        results.clear();
                    }
                    for cls in resolved {
                        if !results.iter().any(|c| c.name == cls.name) {
                            results.push(cls);
                        }
                    }
                    return;
                }
            }

            // Fall back to generic element type extraction.
            if let Some(element_type) = docblock::types::extract_generic_value_type(raw) {
                let resolved = Self::type_hint_to_classes(
                    &element_type,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
                if !resolved.is_empty() {
                    if !conditional {
                        results.clear();
                    }
                    for cls in resolved {
                        if !results.iter().any(|c| c.name == cls.name) {
                            results.push(cls);
                        }
                    }
                }
            }
        }
    }

    /// Extract a string key from a destructuring key expression.
    ///
    /// Handles string literals (`'user'`, `"user"`) and integer literals
    /// (`0`, `1`).  Returns `None` for dynamic or unsupported key
    /// expressions.
    fn extract_destructuring_key(key_expr: &Expression<'_>) -> Option<String> {
        match key_expr {
            Expression::Literal(Literal::String(lit_str)) => {
                // `value` strips the quotes; fall back to `raw` trimmed.
                lit_str.value.map(|v| v.to_string()).or_else(|| {
                    let raw = lit_str.raw;
                    // Strip surrounding quotes from the raw representation.
                    raw.strip_prefix('\'')
                        .and_then(|s| s.strip_suffix('\''))
                        .or_else(|| raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')))
                        .map(|s| s.to_string())
                })
            }
            Expression::Literal(Literal::Integer(lit_int)) => Some(lit_int.raw.to_string()),
            _ => None,
        }
    }

    /// Extract the raw iterable type string from an RHS expression.
    ///
    /// Returns the type annotation string (e.g. `"array<int, User>"`,
    /// `"list<User>"`) without resolving it to `ClassInfo`.  The caller
    /// can then use `extract_generic_value_type` to get the element type.
    fn extract_rhs_iterable_raw_type<'b>(
        rhs: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Option<String> {
        let current_class_name: &str = &ctx.current_class.name;
        let all_classes = ctx.all_classes;
        let content = ctx.content;
        let class_loader = ctx.class_loader;
        let function_loader = ctx.function_loader;

        // ── Variable RHS: `[$a, $b] = $users` ──────────────────────────
        if let Expression::Variable(Variable::Direct(dv)) = rhs {
            let var_text = dv.name.to_string();
            let offset = rhs.span().start.offset as usize;
            return docblock::find_iterable_raw_type_in_source(content, offset, &var_text);
        }

        // ── Function call RHS: `[$a, $b] = getUsers()` ─────────────────
        if let Expression::Call(Call::Function(func_call)) = rhs {
            let func_name = match func_call.function {
                Expression::Identifier(ident) => Some(ident.value().to_string()),
                _ => None,
            };
            if let Some(name) = func_name
                && let Some(fl) = function_loader
                && let Some(func_info) = fl(&name)
                && let Some(ref ret) = func_info.return_type
            {
                return Some(ret.clone());
            }
        }

        // ── Method call RHS: `[$a, $b] = $this->getUsers()` ────────────
        if let Expression::Call(Call::Method(method_call)) = rhs {
            if let Expression::Variable(Variable::Direct(dv)) = method_call.object
                && dv.name == "$this"
                && let ClassLikeMemberSelector::Identifier(ident) = &method_call.method
            {
                let method_name = ident.value.to_string();
                if let Some(owner) = all_classes.iter().find(|c| c.name == current_class_name) {
                    let merged = Self::resolve_class_with_inheritance(owner, class_loader);
                    if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
                        return method.return_type.clone();
                    }
                }
            } else {
                // General case: resolve the object, then look up the method.
                let rhs_span = rhs.span();
                let start = rhs_span.start.offset as usize;
                let end = rhs_span.end.offset as usize;
                if end <= content.len() {
                    let rhs_text = content[start..end].trim();
                    if rhs_text.ends_with(')')
                        && let Some((call_body, _args_text)) = split_call_subject(rhs_text)
                    {
                        // Split at the last `->` to get the object and method name.
                        if let Some(arrow_pos) = call_body.rfind("->") {
                            let obj_text = &call_body[..arrow_pos];
                            let method_name = &call_body[arrow_pos + 2..];
                            let current_class =
                                all_classes.iter().find(|c| c.name == current_class_name);
                            let obj_classes = Self::resolve_target_classes(
                                obj_text,
                                crate::types::AccessKind::Arrow,
                                current_class,
                                all_classes,
                                content,
                                ctx.cursor_offset,
                                class_loader,
                                function_loader,
                            );
                            for cls in &obj_classes {
                                let merged =
                                    Self::resolve_class_with_inheritance(cls, class_loader);
                                if let Some(method) =
                                    merged.methods.iter().find(|m| m.name == method_name)
                                {
                                    return method.return_type.clone();
                                }
                            }
                        }
                    }
                }
            }
        }

        // ── Static method call RHS: `[$a, $b] = MyClass::getUsers()` ───
        if let Expression::Call(Call::StaticMethod(static_call)) = rhs {
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
                    let merged = Self::resolve_class_with_inheritance(owner, class_loader);
                    if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
                        return method.return_type.clone();
                    }
                }
            }
        }

        // ── Property access RHS: `[$a, $b] = $this->items` ─────────────
        if let Expression::Access(access) = rhs {
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
                    let owner_classes: Vec<ClassInfo> =
                        if let Expression::Variable(Variable::Direct(dv)) = obj
                            && dv.name == "$this"
                        {
                            all_classes
                                .iter()
                                .find(|c| c.name == current_class_name)
                                .cloned()
                                .into_iter()
                                .collect()
                        } else if let Expression::Variable(Variable::Direct(dv)) = obj {
                            let var = dv.name.to_string();
                            Self::resolve_target_classes(
                                &var,
                                crate::types::AccessKind::Arrow,
                                Some(ctx.current_class),
                                ctx.all_classes,
                                ctx.content,
                                ctx.cursor_offset,
                                ctx.class_loader,
                                ctx.function_loader,
                            )
                        } else {
                            vec![]
                        };
                    for owner in &owner_classes {
                        let merged = Self::resolve_class_with_inheritance(owner, class_loader);
                        if let Some(prop) = merged.properties.iter().find(|p| p.name == prop_name)
                            && let Some(ref hint) = prop.type_hint
                        {
                            return Some(hint.clone());
                        }
                    }
                }
            }
        }

        None
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
                Self::try_resolve_destructured_type(assignment, ctx, results, conditional);
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
            let rhs_ctx = VarResolutionCtx {
                var_name: ctx.var_name,
                current_class: ctx.current_class,
                all_classes: ctx.all_classes,
                content: ctx.content,
                cursor_offset: assignment.span().start.offset,
                class_loader: ctx.class_loader,
                function_loader: ctx.function_loader,
            };
            let resolved = Self::resolve_rhs_expression(assignment.rhs, &rhs_ctx);
            push_results(results, resolved, conditional);
        }
    }

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
    fn resolve_rhs_expression<'b>(
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        let current_class_name: &str = &ctx.current_class.name;
        let all_classes = ctx.all_classes;
        let content = ctx.content;
        let class_loader = ctx.class_loader;
        let function_loader = ctx.function_loader;

        // ── Instantiation: `new ClassName(…)` ──
        if let Expression::Instantiation(inst) = expr {
            let class_name = match inst.class {
                Expression::Self_(_) => Some("self"),
                Expression::Static(_) => Some("static"),
                Expression::Identifier(ident) => Some(ident.value()),
                _ => None,
            };
            if let Some(name) = class_name {
                return Self::type_hint_to_classes(
                    name,
                    current_class_name,
                    all_classes,
                    class_loader,
                );
            }
            return vec![];
        }

        // ── Array access: `$arr[0]` or `$arr[$key]` ──
        // Resolve the base array's generic/iterable type and extract
        // the element type.
        if let Expression::ArrayAccess(array_access) = expr {
            if let Expression::Variable(Variable::Direct(base_dv)) = array_access.array {
                let base_var = base_dv.name.to_string();
                let access_offset = expr.span().start.offset as usize;
                if let Some(raw_type) =
                    docblock::find_iterable_raw_type_in_source(content, access_offset, &base_var)
                    && let Some(element_type) =
                        docblock::types::extract_generic_value_type(&raw_type)
                {
                    return Self::type_hint_to_classes(
                        &element_type,
                        current_class_name,
                        all_classes,
                        class_loader,
                    );
                }
            }
            return vec![];
        }

        // ── Function / method / static calls ──
        if let Expression::Call(call) = expr {
            match call {
                Call::Function(func_call) => {
                    let func_name = match func_call.function {
                        Expression::Identifier(ident) => Some(ident.value().to_string()),
                        _ => None,
                    };
                    if let Some(name) = func_name
                        && let Some(fl) = function_loader
                        && let Some(func_info) = fl(&name)
                    {
                        // Try conditional return type first
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
                                    return resolved;
                                }
                            }
                        }
                        if let Some(ref ret) = func_info.return_type {
                            return Self::type_hint_to_classes(
                                ret,
                                current_class_name,
                                all_classes,
                                class_loader,
                            );
                        }
                    }
                }
                Call::Method(method_call) => {
                    if let Expression::Variable(Variable::Direct(dv)) = method_call.object
                        && dv.name == "$this"
                        && let ClassLikeMemberSelector::Identifier(ident) = &method_call.method
                    {
                        let method_name = ident.value.to_string();
                        if let Some(owner) =
                            all_classes.iter().find(|c| c.name == current_class_name)
                        {
                            return Self::resolve_method_return_types(
                                owner,
                                &method_name,
                                all_classes,
                                class_loader,
                            );
                        }
                    } else {
                        // General case: extract the call expression text and
                        // delegate to text-based resolution.
                        let rhs_span = expr.span();
                        let start = rhs_span.start.offset as usize;
                        let end = rhs_span.end.offset as usize;
                        if end <= content.len() {
                            let rhs_text = content[start..end].trim();
                            if rhs_text.ends_with(')')
                                && let Some((call_body, args_text)) = split_call_subject(rhs_text)
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
                                return Self::resolve_call_return_types(
                                    call_body, args_text, &call_ctx,
                                );
                            }
                        }
                    }
                }
                Call::StaticMethod(static_call) => {
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
                            return Self::resolve_method_return_types(
                                owner,
                                &method_name,
                                all_classes,
                                class_loader,
                            );
                        }
                    }
                }
                _ => {}
            }
            return vec![];
        }

        // ── Property access: `$this->prop` or `$obj->prop` ──
        if let Expression::Access(access) = expr {
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
                    let owner_classes: Vec<ClassInfo> =
                        if let Expression::Variable(Variable::Direct(dv)) = obj
                            && dv.name == "$this"
                        {
                            all_classes
                                .iter()
                                .find(|c| c.name == current_class_name)
                                .cloned()
                                .into_iter()
                                .collect()
                        } else if let Expression::Variable(Variable::Direct(dv)) = obj {
                            let var = dv.name.to_string();
                            Self::resolve_target_classes(
                                &var,
                                crate::types::AccessKind::Arrow,
                                Some(ctx.current_class),
                                ctx.all_classes,
                                ctx.content,
                                ctx.cursor_offset,
                                ctx.class_loader,
                                ctx.function_loader,
                            )
                        } else {
                            vec![]
                        };

                    for owner in &owner_classes {
                        let resolved = Self::resolve_property_types(
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
        }

        // ── Match expression ──
        if let Expression::Match(match_expr) = expr {
            let mut combined = Vec::new();
            for arm in match_expr.arms.iter() {
                let arm_results = Self::resolve_rhs_expression(arm.expression(), ctx);
                ClassInfo::extend_unique(&mut combined, arm_results);
            }
            return combined;
        }

        // ── Ternary expression ──
        if let Expression::Conditional(cond_expr) = expr {
            let mut combined = Vec::new();
            let then_expr = cond_expr.then.unwrap_or(cond_expr.condition);
            ClassInfo::extend_unique(&mut combined, Self::resolve_rhs_expression(then_expr, ctx));
            ClassInfo::extend_unique(
                &mut combined,
                Self::resolve_rhs_expression(cond_expr.r#else, ctx),
            );
            return combined;
        }

        // ── Null-coalescing expression ──
        if let Expression::Binary(binary) = expr
            && binary.operator.is_null_coalesce()
        {
            let mut combined = Vec::new();
            ClassInfo::extend_unique(&mut combined, Self::resolve_rhs_expression(binary.lhs, ctx));
            ClassInfo::extend_unique(&mut combined, Self::resolve_rhs_expression(binary.rhs, ctx));
            return combined;
        }

        // ── Clone expression: `clone $expr` preserves the type ──
        // First try resolving the inner expression structurally (handles
        // `clone new Foo()`, `clone $this->getConfig()`, ternary, etc.).
        // If that yields nothing and the inner expression is a variable,
        // fall back to text-based resolution by extracting the source
        // text of the cloned expression and resolving it as a subject
        // string via `resolve_target_classes`.
        if let Expression::Clone(clone_expr) = expr {
            let structural = Self::resolve_rhs_expression(clone_expr.object, ctx);
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
            if end <= content.len() {
                let obj_text = content[start..end].trim();
                if !obj_text.is_empty() {
                    let current_class = all_classes.iter().find(|c| c.name == current_class_name);
                    return Self::resolve_target_classes(
                        obj_text,
                        crate::types::AccessKind::Arrow,
                        current_class,
                        all_classes,
                        content,
                        ctx.cursor_offset,
                        class_loader,
                        ctx.function_loader,
                    );
                }
            }
            return vec![];
        }

        vec![]
    }
}
