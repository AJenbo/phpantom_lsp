/// Type resolution for completion subjects.
///
/// This module contains the core entry points for resolving a completion
/// subject (e.g. `$this`, `self`, `static`, `$var`, `$this->prop`,
/// `ClassName`) to a concrete `ClassInfo` so that the correct completion
/// items can be offered.
///
/// The resolution logic is split across several sibling modules:
///
/// - [`super::call_resolution`]: Call expression and callable target
///   resolution (method calls, static calls, function calls, constructor
///   calls, signature help, named-argument completion).
/// - [`super::type_resolution`]: Type-hint string to `ClassInfo` mapping
///   (unions, intersections, generics, type aliases, object shapes).
/// - [`super::source_helpers`]: Source-text scanning helpers (closure return
///   types, first-class callable resolution, `new` expression parsing,
///   array access segment walking).
/// - [`super::variable_resolution`]: Variable type resolution via
///   assignment scanning and parameter type hints.
/// - [`super::type_narrowing`]: instanceof / assert / custom type guard
///   narrowing.
/// - [`super::closure_resolution`]: Closure and arrow-function parameter
///   resolution.
/// - [`crate::inheritance`]: Class inheritance merging (traits, mixins,
///   parent chain).
/// - [`super::conditional_resolution`]: PHPStan conditional return type
///   resolution at call sites.
use std::sync::Arc;

use crate::Backend;
use crate::docblock;
use crate::types::*;
use crate::util::find_class_by_name;

/// Type alias for the optional function-loader closure passed through
/// the resolution chain.  Reduces clippy `type_complexity` warnings.
pub(crate) type FunctionLoaderFn<'a> = Option<&'a dyn Fn(&str) -> Option<FunctionInfo>>;

/// Bundles the context needed by [`resolve_target_classes`] and
/// the functions it delegates to.
///
/// Introduced to replace the 8-parameter signature of
/// `resolve_target_classes` with a cleaner `(subject, access_kind, ctx)`
/// triple.  Also used directly by `resolve_call_return_types_expr` and
/// `resolve_arg_text_to_type` (formerly `CallResolutionCtx`).
pub(crate) struct ResolutionCtx<'a> {
    /// The class the cursor is inside, if any.
    pub current_class: Option<&'a ClassInfo>,
    /// All classes known in the current file.
    pub all_classes: &'a [Arc<ClassInfo>],
    /// The full source text of the current file.
    pub content: &'a str,
    /// Byte offset of the cursor in `content`.
    pub cursor_offset: u32,
    /// Cross-file class resolution callback.
    pub class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    /// Shared cache of fully-resolved classes, keyed by FQN.
    ///
    /// When `Some`, [`resolve_class_fully_cached`](crate::virtual_members::resolve_class_fully_cached)
    /// is used instead of the uncached variant, eliminating redundant
    /// full-resolution work within a single request cycle.  `None` in
    /// contexts where no `Backend` (and therefore no cache) is available
    /// (e.g. standalone free-function callers, some test helpers).
    pub resolved_class_cache: Option<&'a crate::virtual_members::ResolvedClassCache>,
    /// Cross-file function resolution callback (optional).
    pub function_loader: FunctionLoaderFn<'a>,
}

/// Bundles the common parameters threaded through variable-type resolution.
///
/// Introducing this struct avoids passing 7–10 individual arguments to
/// every helper in the resolution chain, which keeps clippy happy and
/// makes call-sites much easier to read.
pub(super) struct VarResolutionCtx<'a> {
    pub var_name: &'a str,
    pub current_class: &'a ClassInfo,
    pub all_classes: &'a [Arc<ClassInfo>],
    pub content: &'a str,
    pub cursor_offset: u32,
    pub class_loader: &'a dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    pub function_loader: FunctionLoaderFn<'a>,
    /// Shared cache of fully-resolved classes, keyed by FQN.
    ///
    /// See [`ResolutionCtx::resolved_class_cache`] for details.
    pub resolved_class_cache: Option<&'a crate::virtual_members::ResolvedClassCache>,
    /// The `@return` type annotation of the enclosing function/method,
    /// if known.  Used inside generator bodies to reverse-infer variable
    /// types from `Generator<TKey, TValue, TSend, TReturn>`.
    pub enclosing_return_type: Option<String>,
}

impl<'a> VarResolutionCtx<'a> {
    /// Create a [`ResolutionCtx`] from this variable resolution context.
    ///
    /// The non-optional `current_class` is wrapped in `Some(…)`.
    pub(crate) fn as_resolution_ctx(&self) -> ResolutionCtx<'a> {
        ResolutionCtx {
            current_class: Some(self.current_class),
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset: self.cursor_offset,
            class_loader: self.class_loader,
            function_loader: self.function_loader,
            resolved_class_cache: self.resolved_class_cache,
        }
    }

    /// Clone this context with a different `enclosing_return_type`.
    ///
    /// All other fields are copied by reference.  This is useful when
    /// descending into a nested function/method body whose `@return`
    /// annotation differs from the outer scope.
    pub(super) fn with_enclosing_return_type(
        &self,
        enclosing_return_type: Option<String>,
    ) -> VarResolutionCtx<'a> {
        VarResolutionCtx {
            var_name: self.var_name,
            current_class: self.current_class,
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset: self.cursor_offset,
            class_loader: self.class_loader,
            function_loader: self.function_loader,
            resolved_class_cache: self.resolved_class_cache,
            enclosing_return_type,
        }
    }

    /// Clone this context with a different `cursor_offset`.
    ///
    /// All other fields (including `enclosing_return_type`) are preserved.
    /// This is useful when resolving a right-hand-side expression at a
    /// position earlier than the original cursor to avoid infinite
    /// recursion on self-referential assignments.
    pub(super) fn with_cursor_offset(&self, cursor_offset: u32) -> VarResolutionCtx<'a> {
        VarResolutionCtx {
            var_name: self.var_name,
            current_class: self.current_class,
            all_classes: self.all_classes,
            content: self.content,
            cursor_offset,
            class_loader: self.class_loader,
            function_loader: self.function_loader,
            resolved_class_cache: self.resolved_class_cache,
            enclosing_return_type: self.enclosing_return_type.clone(),
        }
    }
}

/// Resolve a completion subject to all candidate class types.
///
/// When a variable is assigned different types in conditional branches
/// (e.g. an `if` block reassigns `$thing`), this returns every possible
/// type so the caller can try each one when looking up members.
///
/// Internally parses the subject string into a [`SubjectExpr`] and
/// dispatches via `match` for exhaustive, type-safe routing.
pub(crate) fn resolve_target_classes(
    subject: &str,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let expr = SubjectExpr::parse(subject);
    resolve_target_classes_expr(&expr, access_kind, ctx)
}

/// Core dispatch for [`resolve_target_classes`], operating on a
/// pre-parsed [`SubjectExpr`].
pub(crate) fn resolve_target_classes_expr(
    expr: &SubjectExpr,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let current_class = ctx.current_class;
    let all_classes = ctx.all_classes;
    let class_loader = ctx.class_loader;

    match expr {
        // ── Keywords that always mean "current class" ────────────
        SubjectExpr::This => {
            // Check for `@param-closure-this` override: when the cursor
            // is inside a closure passed as an argument to a function
            // whose parameter carries `@param-closure-this`, resolve
            // `$this` to the declared type instead of the lexical class.
            if let Some(override_cls) =
                super::variable::closure_resolution::find_closure_this_override(ctx)
            {
                return vec![override_cls];
            }
            current_class.cloned().into_iter().collect()
        }
        SubjectExpr::SelfKw | SubjectExpr::StaticKw => current_class.cloned().into_iter().collect(),

        // ── `parent::` — resolve to the current class's parent ──
        SubjectExpr::Parent => {
            if let Some(cc) = current_class
                && let Some(ref parent_name) = cc.parent_class
            {
                if let Some(cls) = find_class_by_name(all_classes, parent_name) {
                    return vec![cls.clone()];
                }
                return class_loader(parent_name)
                    .map(Arc::unwrap_or_clone)
                    .into_iter()
                    .collect();
            }
            vec![]
        }

        // ── Inline array literal with index access ──────────────
        SubjectExpr::InlineArray { elements, .. } => {
            let mut element_classes = Vec::new();
            for elem_text in elements {
                let elem = elem_text.trim();
                if elem.is_empty() {
                    continue;
                }
                let elem_expr = SubjectExpr::parse(elem);
                let resolved = resolve_target_classes_expr(&elem_expr, AccessKind::Arrow, ctx);
                ClassInfo::extend_unique(&mut element_classes, resolved);
            }
            element_classes
        }

        // ── Enum case / static member access ────────────────────
        SubjectExpr::StaticAccess { class, .. } => {
            if let Some(cls) = find_class_by_name(all_classes, class) {
                return vec![cls.clone()];
            }
            class_loader(class)
                .map(Arc::unwrap_or_clone)
                .into_iter()
                .collect()
        }

        // ── Bare class name ─────────────────────────────────────
        SubjectExpr::ClassName(name) => {
            if let Some(cls) = find_class_by_name(all_classes, name) {
                return vec![cls.clone()];
            }
            class_loader(name)
                .map(Arc::unwrap_or_clone)
                .into_iter()
                .collect()
        }

        // ── `new ClassName` (without trailing call parens) ───────
        SubjectExpr::NewExpr { class_name } => {
            if let Some(cls) = find_class_by_name(all_classes, class_name) {
                return vec![cls.clone()];
            }
            class_loader(class_name)
                .map(Arc::unwrap_or_clone)
                .into_iter()
                .collect()
        }

        // ── Call expression ─────────────────────────────────────
        SubjectExpr::CallExpr { callee, args_text } => {
            Backend::resolve_call_return_types_expr(callee, args_text, ctx)
        }

        // ── Property chain ──────────────────────────────────────
        SubjectExpr::PropertyChain { base, property } => {
            let base_classes = resolve_target_classes_expr(base, access_kind, ctx);
            let mut results = Vec::new();
            for cls in &base_classes {
                let resolved = super::type_resolution::resolve_property_types(
                    property,
                    cls,
                    all_classes,
                    class_loader,
                );
                ClassInfo::extend_unique(&mut results, resolved);
            }

            // ── Property-level narrowing ────────────────────────
            // When the property chain resolves to a union (or a
            // broad interface type), an enclosing `instanceof`
            // check like `if ($this->prop instanceof Foo)` should
            // narrow the result set, just as it does for plain
            // variables.  Build the full access path (e.g.
            // `$this->timeline`) and run the narrowing walk.
            //
            // This also handles untyped properties: when the
            // property has no type hint, `results` is empty but
            // an `instanceof` check or `assert()` can still
            // provide a type via `apply_instanceof_inclusion`.
            if let Some(cc) = current_class {
                let full_path = format!("{}->{}", base.to_subject_text(), property);
                apply_property_narrowing(&full_path, cc, ctx, &mut results);
            }

            results
        }

        // ── Array access on variable ────────────────────────────
        SubjectExpr::ArrayAccess { base, segments } => {
            let base_var = base.to_subject_text();

            // Build candidate raw types from multiple strategies.
            // Each is tried as a complete pipeline (raw type →
            // segment walk → ClassInfo); the first that succeeds
            // through all segments wins.
            let docblock_type = docblock::find_iterable_raw_type_in_source(
                ctx.content,
                ctx.cursor_offset as usize,
                &base_var,
            );
            let ast_type = crate::completion::variable::raw_type_inference::resolve_variable_assignment_raw_type(
                &base_var,
                ctx.content,
                ctx.cursor_offset,
                current_class,
                all_classes,
                class_loader,
                ctx.function_loader,
            );

            let candidates = docblock_type.into_iter().chain(ast_type);

            if let Some(resolved) = super::source::helpers::try_chained_array_access_with_candidates(
                candidates,
                segments,
                current_class,
                all_classes,
                class_loader,
            ) {
                return resolved;
            }
            // Fall through to variable resolution if the base is a bare variable
            if let SubjectExpr::Variable(_) = **base {
                resolve_variable_fallback(&base_var, access_kind, ctx)
            } else {
                vec![]
            }
        }

        // ── Bare variable ───────────────────────────────────────
        SubjectExpr::Variable(var_name) => resolve_variable_fallback(var_name, access_kind, ctx),

        // ── Callee-only variants (MethodCall, StaticMethodCall,
        //    FunctionCall) should not appear as top-level subjects;
        //    they are wrapped in CallExpr.  If they do appear
        //    (e.g. from a partial parse), treat as class name. ────
        SubjectExpr::MethodCall { .. }
        | SubjectExpr::StaticMethodCall { .. }
        | SubjectExpr::FunctionCall(_) => {
            let text = expr.to_subject_text();
            if let Some(cls) = find_class_by_name(all_classes, &text) {
                return vec![cls.clone()];
            }
            class_loader(&text)
                .map(Arc::unwrap_or_clone)
                .into_iter()
                .collect()
        }
    }
}

/// Shared variable-resolution logic extracted from the former
/// bare-`$var` branch of `resolve_target_classes`.
fn resolve_variable_fallback(
    var_name: &str,
    access_kind: AccessKind,
    ctx: &ResolutionCtx<'_>,
) -> Vec<ClassInfo> {
    let current_class = ctx.current_class;
    let all_classes = ctx.all_classes;
    let class_loader = ctx.class_loader;
    let function_loader = ctx.function_loader;

    let dummy_class;
    let effective_class = match current_class {
        Some(cc) => cc,
        None => {
            dummy_class = ClassInfo::default();
            &dummy_class
        }
    };

    // ── `$var::` where `$var` holds a class-string ──
    if access_kind == AccessKind::DoubleColon {
        let class_string_targets =
            crate::completion::variable::class_string_resolution::resolve_class_string_targets(
                var_name,
                effective_class,
                all_classes,
                ctx.content,
                ctx.cursor_offset,
                class_loader,
            );
        if !class_string_targets.is_empty() {
            return class_string_targets;
        }
    }

    super::variable::resolution::resolve_variable_types(
        var_name,
        effective_class,
        all_classes,
        ctx.content,
        ctx.cursor_offset,
        class_loader,
        function_loader,
    )
}

// ── Static owner class resolution ───────────────────────────────────

/// Resolve a static class reference (`self`, `static`, `parent`, or a
/// class name) to its `ClassInfo`.
///
/// Handles the `self`/`static`/`parent` keywords and falls back to
/// `class_loader` then `resolve_target_classes` for named classes.
pub(in crate::completion) fn resolve_static_owner_class(
    class: &str,
    rctx: &ResolutionCtx<'_>,
) -> Option<ClassInfo> {
    if class == "self" || class == "static" {
        rctx.current_class.cloned()
    } else if class == "parent" {
        rctx.current_class
            .and_then(|cc| cc.parent_class.as_ref())
            .and_then(|p| (rctx.class_loader)(p).map(Arc::unwrap_or_clone))
    } else {
        find_class_by_name(rctx.all_classes, class)
            .cloned()
            .or_else(|| (rctx.class_loader)(class).map(Arc::unwrap_or_clone))
            .or_else(|| {
                resolve_target_classes(class, crate::AccessKind::DoubleColon, rctx)
                    .into_iter()
                    .next()
            })
    }
}

/// Apply instanceof / assert narrowing for a property-access path.
///
/// This is the property-level analog of the narrowing that
/// [`super::variable::resolution::walk_statements_for_assignments`]
/// performs for plain variables.  It re-parses the source, locates
/// the enclosing method body, and walks its statements with a
/// [`VarResolutionCtx`] whose `var_name` is the full property path
/// (e.g. `$this->timeline`).  The existing narrowing functions in
/// [`super::types::narrowing`] already support property paths via
/// [`super::types::narrowing::expr_to_subject_key`], so no changes
/// to those functions are required.
fn apply_property_narrowing(
    property_path: &str,
    current_class: &ClassInfo,
    rctx: &ResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use crate::parser::with_parsed_program;

    with_parsed_program(
        rctx.content,
        "apply_property_narrowing",
        |program, _content| {
            let ctx = VarResolutionCtx {
                var_name: property_path,
                current_class,
                all_classes: rctx.all_classes,
                content: rctx.content,
                cursor_offset: rctx.cursor_offset,
                class_loader: rctx.class_loader,
                function_loader: rctx.function_loader,
                resolved_class_cache: None,
                enclosing_return_type: None,
            };
            walk_property_narrowing_in_statements(program.statements.iter(), &ctx, results);
        },
    );
}

/// Walk top-level statements to find the class + method containing the
/// cursor, then apply narrowing to `results` for the given property path.
fn walk_property_narrowing_in_statements<'b>(
    statements: impl Iterator<Item = &'b mago_syntax::ast::Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_span::HasSpan;
    use mago_syntax::ast::*;

    for stmt in statements {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    walk_property_narrowing_in_members(class.members.iter(), ctx, results);
                    return;
                }
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if ctx.cursor_offset >= start && ctx.cursor_offset <= end {
                    walk_property_narrowing_in_members(trait_def.members.iter(), ctx, results);
                    return;
                }
            }
            Statement::Namespace(ns) => {
                let ns_span = ns.span();
                if ctx.cursor_offset >= ns_span.start.offset
                    && ctx.cursor_offset <= ns_span.end.offset
                {
                    walk_property_narrowing_in_statements(ns.statements().iter(), ctx, results);
                    return;
                }
            }
            _ => {}
        }
    }
}

/// Walk class members to find the method containing the cursor, then
/// apply instanceof / guard-clause narrowing for the property path.
fn walk_property_narrowing_in_members<'b>(
    members: impl Iterator<Item = &'b mago_syntax::ast::class_like::member::ClassLikeMember<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_syntax::ast::class_like::member::ClassLikeMember;
    use mago_syntax::ast::class_like::method::MethodBody;

    for member in members {
        if let ClassLikeMember::Method(method) = member {
            let body = match &method.body {
                MethodBody::Concrete(block) => block,
                _ => continue,
            };
            let body_start = body.left_brace.start.offset;
            let body_end = body.right_brace.end.offset;
            if ctx.cursor_offset >= body_start && ctx.cursor_offset <= body_end {
                walk_property_narrowing_stmts(body.statements.iter(), ctx, results);
                return;
            }
        }
    }
}

/// Walk statements applying only narrowing (no assignment scanning)
/// for a property path like `$this->prop`.
fn walk_property_narrowing_stmts<'b>(
    statements: impl Iterator<Item = &'b mago_syntax::ast::Statement<'b>>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_span::HasSpan;
    use mago_syntax::ast::*;

    use super::types::narrowing;

    for stmt in statements {
        let stmt_span = stmt.span();
        // Only consider statements whose start is before the cursor.
        if stmt_span.start.offset >= ctx.cursor_offset {
            continue;
        }

        match stmt {
            Statement::If(if_stmt) => {
                walk_property_narrowing_if(if_stmt, stmt, ctx, results);
            }
            Statement::Block(block) => {
                walk_property_narrowing_stmts(block.statements.iter(), ctx, results);
            }
            Statement::Expression(expr_stmt) => {
                // assert($this->prop instanceof Foo) — unconditional
                narrowing::try_apply_assert_instanceof_narrowing(
                    expr_stmt.expression,
                    ctx,
                    results,
                );
            }
            _ => {}
        }
    }
}

/// Apply property-level narrowing inside an if / elseif / else chain.
fn walk_property_narrowing_if<'b>(
    if_stmt: &'b mago_syntax::ast::If<'b>,
    enclosing_stmt: &'b mago_syntax::ast::Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    use mago_span::HasSpan;
    use mago_syntax::ast::*;

    use super::types::narrowing;

    match &if_stmt.body {
        IfBody::Statement(body) => {
            // ── then-body narrowing ──
            narrowing::try_apply_instanceof_narrowing(
                if_stmt.condition,
                body.statement.span(),
                ctx,
                results,
            );
            walk_property_narrowing_stmt(body.statement, ctx, results);

            // ── elseif narrowing ──
            for else_if in body.else_if_clauses.iter() {
                narrowing::try_apply_instanceof_narrowing(
                    else_if.condition,
                    else_if.statement.span(),
                    ctx,
                    results,
                );
                walk_property_narrowing_stmt(else_if.statement, ctx, results);
            }

            // ── else-body inverse narrowing ──
            if let Some(else_clause) = &body.else_clause {
                let else_span = else_clause.statement.span();
                narrowing::try_apply_instanceof_narrowing_inverse(
                    if_stmt.condition,
                    else_span,
                    ctx,
                    results,
                );
                for else_if in body.else_if_clauses.iter() {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                }
                walk_property_narrowing_stmt(else_clause.statement, ctx, results);
            }
        }
        IfBody::ColonDelimited(body) => {
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
            walk_property_narrowing_stmts(body.statements.iter(), ctx, results);

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
                narrowing::try_apply_instanceof_narrowing(else_if.condition, ei_span, ctx, results);
                walk_property_narrowing_stmts(else_if.statements.iter(), ctx, results);
            }

            if let Some(else_clause) = &body.else_clause {
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
                for else_if in body.else_if_clauses.iter() {
                    narrowing::try_apply_instanceof_narrowing_inverse(
                        else_if.condition,
                        else_span,
                        ctx,
                        results,
                    );
                }
                walk_property_narrowing_stmts(else_clause.statements.iter(), ctx, results);
            }
        }
    }

    // ── Guard clause narrowing ──
    // When the then-body unconditionally exits and there are no
    // elseif / else branches, apply inverse narrowing after the if.
    if enclosing_stmt.span().end.offset < ctx.cursor_offset {
        narrowing::apply_guard_clause_narrowing(if_stmt, ctx, results);
    }
}

/// Dispatch a single statement to `walk_property_narrowing_stmts`.
fn walk_property_narrowing_stmt<'b>(
    stmt: &'b mago_syntax::ast::Statement<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ClassInfo>,
) {
    walk_property_narrowing_stmts(std::iter::once(stmt), ctx, results);
}
