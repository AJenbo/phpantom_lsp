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
use std::collections::HashMap;

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::Backend;
use crate::docblock;
use crate::parser::with_parsed_program;
use crate::types::ClassInfo;
use crate::util::{
    ARRAY_ELEMENT_FUNCS, ARRAY_PRESERVING_FUNCS, find_semicolon_balanced, short_name,
};

use super::conditional_resolution::{resolve_conditional_with_args, split_call_subject};
use super::resolver::{FunctionLoaderFn, VarResolutionCtx};

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
        with_parsed_program(content, "resolve_variable_types", |program, _content| {
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
        })
    }

    /// Walk a sequence of top-level statements to find the class or
    /// function body that contains the cursor, then resolve the target
    /// variable's type within that scope.
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
                    // The cursor is inside this class body.  PHP method
                    // scopes are isolated — they cannot access variables
                    // from enclosing or top-level code.  Return whatever
                    // the member scan found (even if empty, e.g. after
                    // `unset($var)`), and never fall through to the
                    // top-level walk.
                    return Self::resolve_variable_in_members(class.members.iter(), ctx);
                }
                Statement::Interface(iface) => {
                    let start = iface.left_brace.start.offset;
                    let end = iface.right_brace.end.offset;
                    if ctx.cursor_offset < start || ctx.cursor_offset > end {
                        continue;
                    }
                    return Self::resolve_variable_in_members(iface.members.iter(), ctx);
                }
                Statement::Enum(enum_def) => {
                    let start = enum_def.left_brace.start.offset;
                    let end = enum_def.right_brace.end.offset;
                    if ctx.cursor_offset < start || ctx.cursor_offset > end {
                        continue;
                    }
                    return Self::resolve_variable_in_members(enum_def.members.iter(), ctx);
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
                        // The cursor is inside this function body.  PHP
                        // function scopes are isolated, so return the
                        // result directly (even if empty after `unset`).
                        let mut results: Vec<ClassInfo> = Vec::new();
                        Self::resolve_closure_params(&func.parameter_list, ctx, &mut results);
                        Self::walk_statements_for_assignments(
                            func.body.statements.iter(),
                            ctx,
                            &mut results,
                            false,
                        );
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
                Statement::If(if_stmt) => {
                    Self::walk_if_statement(if_stmt, stmt, ctx, results);
                }
                Statement::Foreach(foreach) => {
                    Self::walk_foreach_statement(foreach, ctx, results, conditional);
                }
                Statement::While(while_stmt) => {
                    Self::walk_while_statement(while_stmt, ctx, results);
                }
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
                    Self::walk_try_statement(try_stmt, ctx, results);
                }
                Statement::Switch(switch) => {
                    for case in switch.body.cases().iter() {
                        Self::walk_statements_for_assignments(
                            case.statements().iter(),
                            ctx,
                            results,
                            true,
                        );
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
                    Self::check_statement_for_assignments(else_if.statement, ctx, results, true);
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
                Self::try_apply_instanceof_narrowing(if_stmt.condition, then_span, ctx, results);
                Self::try_apply_assert_condition_narrowing(
                    if_stmt.condition,
                    then_span,
                    ctx,
                    results,
                    false,
                );
                Self::walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
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
                    Self::try_apply_instanceof_narrowing(else_if.condition, ei_span, ctx, results);
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
        if enclosing_stmt.span().end.offset < ctx.cursor_offset {
            Self::apply_guard_clause_narrowing(if_stmt, ctx, results);
        }
    }

    /// Handle `foreach` statements during variable assignment walking.
    ///
    /// Only resolves the foreach value/key variable and recurses into the
    /// body when the cursor is actually inside the loop body (iteration
    /// variables are out of scope outside the loop).
    fn walk_foreach_statement<'b>(
        foreach: &'b Foreach<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
        conditional: bool,
    ) {
        let body_span = foreach.body.span();
        if ctx.cursor_offset >= body_span.start.offset && ctx.cursor_offset <= body_span.end.offset
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

    /// Handle `while` statements during variable assignment walking.
    ///
    /// Applies instanceof and `@phpstan-assert` narrowing based on the
    /// while condition and recurses into the loop body.
    fn walk_while_statement<'b>(
        while_stmt: &'b While<'b>,
        ctx: &VarResolutionCtx<'_>,
        results: &mut Vec<ClassInfo>,
    ) {
        match &while_stmt.body {
            WhileBody::Statement(inner) => {
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
                Self::try_apply_instanceof_narrowing(while_stmt.condition, body_span, ctx, results);
                Self::try_apply_assert_condition_narrowing(
                    while_stmt.condition,
                    body_span,
                    ctx,
                    results,
                    false,
                );
                Self::walk_statements_for_assignments(body.statements.iter(), ctx, results, true);
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
        Self::walk_statements_for_assignments(try_stmt.block.statements.iter(), ctx, results, true);
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

    /// Convenience wrapper that walks a single statement for assignments
    /// to the target variable, delegating to `walk_statements_for_assignments`.
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
        match expr {
            Expression::Instantiation(inst) => Self::resolve_rhs_instantiation(inst, ctx),
            Expression::ArrayAccess(array_access) => {
                Self::resolve_rhs_array_access(array_access, expr, ctx)
            }
            Expression::Call(call) => Self::resolve_rhs_call(call, expr, ctx),
            Expression::Access(access) => Self::resolve_rhs_property_access(access, ctx),
            Expression::Match(match_expr) => {
                let mut combined = Vec::new();
                for arm in match_expr.arms.iter() {
                    let arm_results = Self::resolve_rhs_expression(arm.expression(), ctx);
                    ClassInfo::extend_unique(&mut combined, arm_results);
                }
                combined
            }
            Expression::Conditional(cond_expr) => {
                let mut combined = Vec::new();
                let then_expr = cond_expr.then.unwrap_or(cond_expr.condition);
                ClassInfo::extend_unique(
                    &mut combined,
                    Self::resolve_rhs_expression(then_expr, ctx),
                );
                ClassInfo::extend_unique(
                    &mut combined,
                    Self::resolve_rhs_expression(cond_expr.r#else, ctx),
                );
                combined
            }
            Expression::Binary(binary) if binary.operator.is_null_coalesce() => {
                let mut combined = Vec::new();
                ClassInfo::extend_unique(
                    &mut combined,
                    Self::resolve_rhs_expression(binary.lhs, ctx),
                );
                ClassInfo::extend_unique(
                    &mut combined,
                    Self::resolve_rhs_expression(binary.rhs, ctx),
                );
                combined
            }
            Expression::Clone(clone_expr) => Self::resolve_rhs_clone(clone_expr, ctx),
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
            return Self::type_hint_to_classes(
                name,
                &ctx.current_class.name,
                ctx.all_classes,
                ctx.class_loader,
            );
        }
        vec![]
    }

    /// Resolve `$arr[0]` / `$arr[$key]` by extracting the generic element
    /// type from the base array's annotation.
    fn resolve_rhs_array_access<'b>(
        array_access: &ArrayAccess<'b>,
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        if let Expression::Variable(Variable::Direct(base_dv)) = array_access.array {
            let base_var = base_dv.name.to_string();
            let access_offset = expr.span().start.offset as usize;
            if let Some(raw_type) =
                docblock::find_iterable_raw_type_in_source(ctx.content, access_offset, &base_var)
                && let Some(element_type) = docblock::types::extract_generic_value_type(&raw_type)
            {
                return Self::type_hint_to_classes(
                    &element_type,
                    &ctx.current_class.name,
                    ctx.all_classes,
                    ctx.class_loader,
                );
            }
        }
        vec![]
    }

    /// Resolve function, method, and static method calls to their return
    /// types.
    fn resolve_rhs_call<'b>(
        call: &'b Call<'b>,
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        match call {
            Call::Function(func_call) => Self::resolve_rhs_function_call(func_call, expr, ctx),
            Call::Method(method_call) => Self::resolve_rhs_method_call(method_call, expr, ctx),
            Call::StaticMethod(static_call) => Self::resolve_rhs_static_call(static_call, ctx),
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
            && let Some(element_type) =
                Self::resolve_array_func_element_type(name, &func_call.argument_list, ctx)
        {
            let resolved = Self::type_hint_to_classes(
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
                let resolved =
                    Self::type_hint_to_classes(&ret, current_class_name, all_classes, class_loader);
                if !resolved.is_empty() {
                    return resolved;
                }
            }

            // 2. Scan for closure literal assignment and
            //    extract native return type hint.
            if let Some(ret) = Self::extract_closure_return_type_from_assignment(
                &var_name,
                content,
                ctx.cursor_offset,
            ) {
                let resolved =
                    Self::type_hint_to_classes(&ret, current_class_name, all_classes, class_loader);
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }

        vec![]
    }

    /// Resolve an instance method call: `$this->method()` (fast path) or
    /// general `$obj->method()` (text-based fallback).
    fn resolve_rhs_method_call<'b>(
        method_call: &'b MethodCall<'b>,
        expr: &'b Expression<'b>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        if let Expression::Variable(Variable::Direct(dv)) = method_call.object
            && dv.name == "$this"
            && let ClassLikeMemberSelector::Identifier(ident) = &method_call.method
        {
            let method_name = ident.value.to_string();
            if let Some(owner) = ctx
                .all_classes
                .iter()
                .find(|c| c.name == ctx.current_class.name)
            {
                let text_args =
                    Self::extract_argument_text(&method_call.argument_list, ctx.content);
                let rctx = ctx.as_resolution_ctx();
                let template_subs = if !text_args.is_empty() {
                    Self::build_method_template_subs(
                        owner,
                        &method_name,
                        &text_args,
                        &rctx,
                        ctx.class_loader,
                    )
                } else {
                    HashMap::new()
                };
                return Self::resolve_method_return_types_with_args(
                    owner,
                    &method_name,
                    &text_args,
                    ctx.all_classes,
                    ctx.class_loader,
                    &template_subs,
                );
            }
        } else {
            // General case: extract the call expression text and
            // delegate to text-based resolution.
            let rhs_span = expr.span();
            let start = rhs_span.start.offset as usize;
            let end = rhs_span.end.offset as usize;
            if end <= ctx.content.len() {
                let rhs_text = ctx.content[start..end].trim();
                if rhs_text.ends_with(')')
                    && let Some((call_body, args_text)) = split_call_subject(rhs_text)
                {
                    let rctx = ctx.as_resolution_ctx();
                    return Self::resolve_call_return_types(call_body, args_text, &rctx);
                }
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
            if let Some(ref owner) = owner {
                let text_args =
                    Self::extract_argument_text(&static_call.argument_list, ctx.content);
                let rctx = ctx.as_resolution_ctx();
                let template_subs = if !text_args.is_empty() {
                    Self::build_method_template_subs(
                        owner,
                        &method_name,
                        &text_args,
                        &rctx,
                        ctx.class_loader,
                    )
                } else {
                    HashMap::new()
                };
                return Self::resolve_method_return_types_with_args(
                    owner,
                    &method_name,
                    &text_args,
                    ctx.all_classes,
                    ctx.class_loader,
                    &template_subs,
                );
            }
        }
        vec![]
    }

    /// Resolve property access: `$this->prop`, `$obj->prop`, `$obj?->prop`.
    fn resolve_rhs_property_access(
        access: &Access<'_>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Vec<ClassInfo> {
        let current_class_name: &str = &ctx.current_class.name;
        let all_classes = ctx.all_classes;
        let class_loader = ctx.class_loader;

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
                            &ctx.as_resolution_ctx(),
                        )
                    } else {
                        vec![]
                    };

                for owner in &owner_classes {
                    let resolved =
                        Self::resolve_property_types(&prop_name, owner, all_classes, class_loader);
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
        if end <= ctx.content.len() {
            let obj_text = ctx.content[start..end].trim();
            if !obj_text.is_empty() {
                let rctx = ctx.as_resolution_ctx();
                return Self::resolve_target_classes(
                    obj_text,
                    crate::types::AccessKind::Arrow,
                    &rctx,
                );
            }
        }
        vec![]
    }

    // ── Array function type preservation helpers ─────────────────────────

    /// Extract the first positional argument expression from an
    /// argument list.
    fn first_arg_expr<'b>(args: &'b ArgumentList<'b>) -> Option<&'b Expression<'b>> {
        args.arguments.first().map(|arg| match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        })
    }

    /// Extract the nth positional argument expression (0-based).
    fn nth_arg_expr<'b>(args: &'b ArgumentList<'b>, n: usize) -> Option<&'b Expression<'b>> {
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
    fn resolve_arg_raw_type<'b>(
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

            // No docblock — chase the variable's assignment to extract
            // the raw iterable type.  This handles cases like
            // `$users = $this->getUsers(); array_pop($users)` where
            // `$users` has no `@var` annotation but was assigned from a
            // method returning `list<User>`.
            let current_class = ctx
                .all_classes
                .iter()
                .find(|c| c.name == ctx.current_class.name);
            if let Some(raw) = Self::chase_var_assignment_raw_type(
                &var_text,
                ctx.content,
                offset,
                current_class,
                ctx.all_classes,
                ctx.class_loader,
            ) && docblock::types::extract_generic_value_type(&raw).is_some()
            {
                return Some(raw);
            }
        }
        // Fall back to structural extraction (method calls, etc.)
        Self::extract_rhs_iterable_raw_type(arg_expr, ctx)
    }

    /// Text-based fallback for resolving a variable's raw iterable type
    /// by scanning backward for its assignment and extracting the RHS
    /// return type.
    ///
    /// Used by `resolve_arg_raw_type` when a variable argument to an
    /// array function has no `@var` / `@param` docblock but was assigned
    /// from a method call (e.g. `$users = $this->getUsers()`).
    fn chase_var_assignment_raw_type(
        var_name: &str,
        content: &str,
        before_offset: usize,
        current_class: Option<&ClassInfo>,
        all_classes: &[ClassInfo],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> Option<String> {
        let search_area = content.get(..before_offset)?;

        // Find the most recent assignment to this variable.
        let assign_pattern = format!("{} = ", var_name);
        let assign_pos = search_area.rfind(&assign_pattern)?;
        let rhs_start = assign_pos + assign_pattern.len();

        // Extract the RHS up to the next `;`
        let remaining = &content[rhs_start..];
        let semi_pos = find_semicolon_balanced(remaining)?;
        let rhs_text = remaining[..semi_pos].trim();

        // Only handle call expressions — that's the common case for
        // `$users = $this->getUsers()` or `$users = getUsers()`.
        if !rhs_text.ends_with(')') {
            return None;
        }

        let (callee, _args_text) = split_call_subject(rhs_text)?;

        // Method call: `$this->methodName(…)`
        if let Some(method_name) = callee
            .strip_prefix("$this->")
            .or_else(|| callee.strip_prefix("$this?->"))
        {
            let owner = current_class?;
            return Self::resolve_method_return_type(owner, method_name, class_loader);
        }

        // Static call: `ClassName::methodName(…)`
        if let Some((class_part, method_part)) = callee.rsplit_once("::") {
            let resolved_class = if class_part == "self" || class_part == "static" {
                current_class.cloned()
            } else {
                let lookup = short_name(class_part);
                all_classes
                    .iter()
                    .find(|c| c.name == lookup)
                    .cloned()
                    .or_else(|| class_loader(class_part))
            };
            if let Some(cls) = resolved_class {
                return Self::resolve_method_return_type(&cls, method_part, class_loader);
            }
        }

        None
    }

    /// For known array functions, resolve the **raw output type** string
    /// (e.g. `"list<User>"`) from the input arguments.
    ///
    /// Used by `extract_rhs_iterable_raw_type` so that foreach and
    /// destructuring over `array_filter(...)` etc. preserve element types.
    pub(super) fn resolve_array_func_raw_type(
        func_name: &str,
        args: &ArgumentList<'_>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Option<String> {
        // Type-preserving functions: output array has same element type.
        if ARRAY_PRESERVING_FUNCS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(func_name))
        {
            let arr_expr = Self::first_arg_expr(args)?;
            let raw = Self::resolve_arg_raw_type(arr_expr, ctx)?;
            // If the raw type already has generic params, return it as-is
            // so downstream `extract_generic_value_type` can extract the
            // element type.  Otherwise it's a plain class name and we
            // can't infer element type.
            if docblock::types::extract_generic_value_type(&raw).is_some() {
                return Some(raw);
            }
        }

        // array_map: callback is first arg, array is second.
        // The callback's return type determines the output element type.
        if func_name.eq_ignore_ascii_case("array_map")
            && let Some(element_type) = Self::extract_array_map_element_type(args, ctx)
        {
            return Some(format!("list<{}>", element_type));
        }

        // Element-extracting functions: wrap element type in list<> so
        // it can be used as an iterable raw type.
        if ARRAY_ELEMENT_FUNCS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(func_name))
        {
            let arr_expr = Self::first_arg_expr(args)?;
            let raw = Self::resolve_arg_raw_type(arr_expr, ctx)?;
            if docblock::types::extract_generic_value_type(&raw).is_some() {
                return Some(raw);
            }
        }

        None
    }

    /// For known array functions, resolve the **element type** string
    /// (e.g. `"User"`) for the output.
    ///
    /// Used by `resolve_rhs_expression` so that `$item = array_pop($users)`
    /// resolves `$item` to `User`.  This handles both element-extracting
    /// functions (array_pop, current, etc.) and `array_map` (via callback
    /// return type).
    fn resolve_array_func_element_type(
        func_name: &str,
        args: &ArgumentList<'_>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Option<String> {
        // Element-extracting functions: return the element type directly.
        if ARRAY_ELEMENT_FUNCS
            .iter()
            .any(|f| f.eq_ignore_ascii_case(func_name))
        {
            let arr_expr = Self::first_arg_expr(args)?;
            let raw = Self::resolve_arg_raw_type(arr_expr, ctx)?;
            return docblock::types::extract_generic_value_type(&raw);
        }

        // array_map: callback return type is the element type.
        if func_name.eq_ignore_ascii_case("array_map") {
            return Self::extract_array_map_element_type(args, ctx);
        }

        None
    }

    /// Extract the raw text of a function/method argument list from source.
    ///
    /// Returns the text between the parentheses (exclusive), trimmed.
    /// For example, an argument list `($user, $role)` returns `"$user, $role"`.
    fn extract_argument_text(
        argument_list: &mago_syntax::ast::ArgumentList<'_>,
        content: &str,
    ) -> String {
        let left = argument_list.left_parenthesis.span().end.offset as usize;
        let right = argument_list.right_parenthesis.span().start.offset as usize;
        if right > left && right <= content.len() {
            content[left..right].trim().to_string()
        } else {
            String::new()
        }
    }

    /// Extract the output element type for `array_map($callback, $array)`.
    ///
    /// Strategy:
    /// 1. If the callback (first arg) is a closure/arrow function with a
    ///    return type hint, use that.
    /// 2. Otherwise, fall back to the **input array's** element type
    ///    (assumes the callback preserves type, which is a reasonable
    ///    default when no return type is declared).
    fn extract_array_map_element_type(
        args: &ArgumentList<'_>,
        ctx: &VarResolutionCtx<'_>,
    ) -> Option<String> {
        let callback_expr = Self::first_arg_expr(args)?;

        // Try to get the callback's return type hint.
        let return_hint = match callback_expr {
            Expression::Closure(closure) => closure
                .return_type_hint
                .as_ref()
                .map(|rth| Self::extract_hint_string(&rth.hint)),
            Expression::ArrowFunction(arrow) => arrow
                .return_type_hint
                .as_ref()
                .map(|rth| Self::extract_hint_string(&rth.hint)),
            _ => None,
        };

        if let Some(hint) = return_hint {
            let cleaned = docblock::clean_type(&hint);
            if !cleaned.is_empty() && !docblock::types::is_scalar(&cleaned) {
                return Some(cleaned);
            }
        }

        // Fallback: use the input array's element type.
        let arr_expr = Self::nth_arg_expr(args, 1)?;
        let raw = Self::resolve_arg_raw_type(arr_expr, ctx)?;
        docblock::types::extract_generic_value_type(&raw)
    }
}
