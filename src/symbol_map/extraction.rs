//! AST extraction for the symbol map.
//!
//! This module walks the `mago_syntax` AST and emits [`SymbolSpan`],
//! [`VarDefSite`], [`TemplateParamDef`], and [`CallSite`] entries for
//! every navigable symbol occurrence.  The entry point is
//! [`extract_symbol_map`].

use mago_span::HasSpan;
use mago_syntax::ast::sequence::TokenSeparatedSequence;
use mago_syntax::ast::*;

use super::docblock::{
    class_ref_span, class_ref_span_ctx, extract_docblock_symbols, extract_param_var_spans,
    get_docblock_text_with_offset, is_navigable_type,
};
use super::{
    CallSite, ClassRefContext, SelfStaticParentKind, SymbolKind, SymbolMap, SymbolSpan,
    TemplateParamDef, UntypedClosureSite, VarDefKind, VarDefSite,
};
use crate::util::strip_fqn_prefix;

// ─── Extraction context ─────────────────────────────────────────────────────

/// Bundles the mutable accumulators and read-only context threaded through
/// every `extract_from_*` function.
///
/// Before this struct existed, each extractor took 7–8 parameters (the five
/// `Vec`s plus `trivias`, `content`, and sometimes `scope_start`).  Grouping
/// them here eliminates the `#[allow(clippy::too_many_arguments)]` annotations
/// that were required on 19 functions and makes it trivial to add new
/// accumulated data in the future without touching every call site.
struct ExtractionCtx<'a> {
    /// Navigable symbol spans (class refs, member accesses, variables, …).
    spans: Vec<SymbolSpan>,
    /// Variable definition sites (assignments, parameters, foreach, …).
    var_defs: Vec<VarDefSite>,
    /// Scope ranges `(start, end)` for functions, methods, closures, etc.
    scopes: Vec<(u32, u32)>,
    /// Body boundaries `(body_start, body_end)` for closures and arrow fns.
    /// For closures the body start is the `{` offset; for arrow functions
    /// it is the `=>` token offset.  Used by signature help suppression.
    body_scopes: Vec<(u32, u32)>,
    /// Narrowing block boundaries `(start, end)` for if-body, elseif-body,
    /// else-body, match-arm, and switch-case blocks.  Used by the
    /// diagnostic subject cache to determine whether two variable accesses
    /// are in the same narrowing context.  Accesses in the same block get
    /// the same instanceof narrowing applied and can share a cache entry.
    narrowing_blocks: Vec<(u32, u32)>,
    /// Offsets of `assert($var instanceof ...)` expression statements.
    /// Used as sequential narrowing boundaries in the diagnostic cache.
    assert_narrowing_offsets: Vec<u32>,
    /// `@template` parameter definitions with their scoping ranges.
    template_defs: Vec<TemplateParamDef>,
    /// Call-site records for signature help and conditional return types.
    call_sites: Vec<CallSite>,
    /// Ranges where `break` is valid (loops and `switch`).
    breakable_scopes: Vec<(u32, u32)>,
    /// Ranges where `continue` is valid (loops only).
    loop_scopes: Vec<(u32, u32)>,
    /// Ranges of `switch` bodies (where `case/default` labels are valid).
    switch_scopes: Vec<(u32, u32)>,
    /// Trivia (comments, whitespace) from the parsed program.
    trivias: &'a [Trivia<'a>],
    /// The full source text of the file being extracted.
    content: &'a str,
    /// Closures and arrow functions passed as arguments to callable-typed
    /// parameters, used by inlay hints.
    untyped_closure_sites: Vec<UntypedClosureSite>,
}

// ─── AST extraction ─────────────────────────────────────────────────────────

/// Build a [`SymbolMap`] from a parsed PHP program.
///
/// Walks every statement recursively and emits [`SymbolSpan`] entries for
/// every navigable symbol occurrence.
pub(crate) fn extract_symbol_map(program: &Program<'_>, content: &str) -> SymbolMap {
    let mut ctx = ExtractionCtx {
        spans: Vec::new(),
        var_defs: Vec::new(),
        scopes: Vec::new(),
        body_scopes: Vec::new(),
        narrowing_blocks: Vec::new(),
        assert_narrowing_offsets: Vec::new(),
        template_defs: Vec::new(),
        call_sites: Vec::new(),
        breakable_scopes: Vec::new(),
        loop_scopes: Vec::new(),
        switch_scopes: Vec::new(),
        trivias: program.trivia.as_slice(),
        content,
        untyped_closure_sites: Vec::new(),
    };

    for stmt in program.statements.iter() {
        extract_from_statement(stmt, &mut ctx, 0);
    }

    // ── Sweep all docblock trivia for floating references ───────────
    // Docblocks attached to classes, functions, methods, properties, and
    // certain statements are already processed during the AST walk above.
    // However, docblocks in other positions (e.g. inline `/** @see ... */`
    // inside array literals or after expressions) are never visited.
    // Scan every docblock trivia entry and extract symbols; the dedup
    // step below removes any duplicates from already-processed docblocks.
    for t in program.trivia.iter() {
        if t.kind == TriviaKind::DocBlockComment {
            let _tpl = extract_docblock_symbols(t.value, t.span.start.offset, &mut ctx.spans);
        }
    }

    // Sort by start offset for binary search.
    ctx.spans.sort_by_key(|s| s.start);

    // Deduplicate overlapping spans (keep the first / most specific).
    ctx.spans
        .dedup_by(|b, a| a.start == b.start && a.end == b.end);

    // Sort var_defs by (scope_start, offset) for efficient lookup.
    ctx.var_defs.sort_by(|a, b| {
        a.scope_start
            .cmp(&b.scope_start)
            .then(a.offset.cmp(&b.offset))
    });

    // Sort scopes by start offset.
    ctx.scopes.sort_by_key(|s| s.0);

    // Sort narrowing blocks by start offset.
    ctx.narrowing_blocks.sort_by_key(|s| s.0);

    // Sort assert-narrowing offsets.
    ctx.assert_narrowing_offsets.sort();

    // Sort template_defs by name_offset for binary search / reverse scan.
    ctx.template_defs.sort_by_key(|d| d.name_offset);

    // Sort call_sites by args_start for reverse-scan lookup.
    ctx.call_sites.sort_by_key(|cs| cs.args_start);
    ctx.breakable_scopes.sort_by_key(|s| s.0);
    ctx.loop_scopes.sort_by_key(|s| s.0);
    ctx.switch_scopes.sort_by_key(|s| s.0);

    SymbolMap {
        spans: ctx.spans,
        var_defs: ctx.var_defs,
        scopes: ctx.scopes,
        body_scopes: ctx.body_scopes,
        narrowing_blocks: ctx.narrowing_blocks,
        assert_narrowing_offsets: ctx.assert_narrowing_offsets,
        template_defs: ctx.template_defs,
        call_sites: ctx.call_sites,
        breakable_scopes: ctx.breakable_scopes,
        loop_scopes: ctx.loop_scopes,
        switch_scopes: ctx.switch_scopes,
        untyped_closure_sites: ctx.untyped_closure_sites,
    }
}

// ─── Statement extractor ────────────────────────────────────────────────────

fn extract_from_statement<'a>(
    stmt: &'a Statement<'a>,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    match stmt {
        Statement::Namespace(ns) => {
            // Emit a span for the namespace name itself so rename can target it.
            if let Some(ref ident) = ns.name {
                let name = ident.value().to_string();
                if !name.is_empty() {
                    ctx.spans.push(SymbolSpan {
                        start: ident.span().start.offset,
                        end: ident.span().end.offset,
                        kind: SymbolKind::NamespaceDeclaration { name },
                    });
                }
            }
            for inner in ns.statements().iter() {
                extract_from_statement(inner, ctx, scope_start);
            }
        }
        Statement::Class(class) => {
            extract_from_class(class, ctx);
        }
        Statement::Interface(iface) => {
            extract_from_interface(iface, ctx);
        }
        Statement::Trait(trait_def) => {
            extract_from_trait(trait_def, ctx);
        }
        Statement::Enum(enum_def) => {
            extract_from_enum(enum_def, ctx);
        }
        Statement::Function(func) => {
            extract_from_function(func, ctx);
        }
        Statement::Use(use_stmt) => {
            extract_from_use_statement(use_stmt, &mut ctx.spans);
        }
        Statement::Expression(expr_stmt) => {
            extract_inline_docblock(expr_stmt, ctx);
            // Detect `assert($var instanceof ...)` and record its offset
            // as a sequential narrowing boundary for the diagnostic cache.
            if is_assert_instanceof(expr_stmt.expression) {
                ctx.assert_narrowing_offsets
                    .push(expr_stmt.expression.span().start.offset);
            }
            extract_from_expression(expr_stmt.expression, ctx, scope_start);
        }
        Statement::Return(ret) => {
            extract_inline_docblock(ret, ctx);
            if let Some(val) = ret.value {
                extract_from_expression(val, ctx, scope_start);
            }
        }
        Statement::Echo(echo) => {
            extract_inline_docblock(echo, ctx);
            for expr in echo.values.iter() {
                extract_from_expression(expr, ctx, scope_start);
            }
        }
        Statement::If(if_stmt) => {
            extract_from_expression(if_stmt.condition, ctx, scope_start);
            extract_from_if_body(&if_stmt.body, ctx, scope_start);
        }
        Statement::While(while_stmt) => {
            extract_from_expression(while_stmt.condition, ctx, scope_start);
            let body_span = while_stmt.body.span();
            record_breakable_scope(body_span.start.offset, body_span.end.offset, ctx);
            record_loop_scope(body_span.start.offset, body_span.end.offset, ctx);
            extract_from_while_body(&while_stmt.body, ctx, scope_start);
        }
        Statement::DoWhile(do_while) => {
            let body_span = do_while.statement.span();
            record_breakable_scope(body_span.start.offset, body_span.end.offset, ctx);
            record_loop_scope(body_span.start.offset, body_span.end.offset, ctx);
            extract_from_statement(do_while.statement, ctx, scope_start);
            extract_from_expression(do_while.condition, ctx, scope_start);
        }
        Statement::For(for_stmt) => {
            for expr in for_stmt.initializations.iter() {
                extract_from_expression(expr, ctx, scope_start);
            }
            for expr in for_stmt.conditions.iter() {
                extract_from_expression(expr, ctx, scope_start);
            }
            for expr in for_stmt.increments.iter() {
                extract_from_expression(expr, ctx, scope_start);
            }
            let body_span = for_stmt.body.span();
            record_breakable_scope(body_span.start.offset, body_span.end.offset, ctx);
            record_loop_scope(body_span.start.offset, body_span.end.offset, ctx);
            extract_from_for_body(&for_stmt.body, ctx, scope_start);
        }
        Statement::Foreach(foreach_stmt) => {
            extract_from_expression(foreach_stmt.expression, ctx, scope_start);
            // key and value are accessed via the target.
            if let Some(key_expr) = foreach_stmt.target.key() {
                extract_from_expression(key_expr, ctx, scope_start);
                // Emit VarDefSite for foreach key variable.
                if let Expression::Variable(Variable::Direct(dv)) = key_expr {
                    let name = dv.name.strip_prefix('$').unwrap_or(dv.name).to_string();
                    let offset = dv.span.start.offset;
                    ctx.var_defs.push(VarDefSite {
                        offset,
                        name,
                        kind: VarDefKind::Foreach,
                        scope_start,
                        effective_from: offset,
                    });
                }
            }
            let value_expr = foreach_stmt.target.value();
            extract_from_expression(value_expr, ctx, scope_start);
            // Emit VarDefSite for foreach value variable.
            if let Expression::Variable(Variable::Direct(dv)) = value_expr {
                let name = dv.name.strip_prefix('$').unwrap_or(dv.name).to_string();
                let offset = dv.span.start.offset;
                ctx.var_defs.push(VarDefSite {
                    offset,
                    name,
                    kind: VarDefKind::Foreach,
                    scope_start,
                    effective_from: offset,
                });
            }
            let body_span = foreach_stmt.body.span();
            record_breakable_scope(body_span.start.offset, body_span.end.offset, ctx);
            record_loop_scope(body_span.start.offset, body_span.end.offset, ctx);
            for inner in foreach_stmt.body.statements() {
                extract_from_statement(inner, ctx, scope_start);
            }
        }
        Statement::Switch(switch_stmt) => {
            extract_from_expression(switch_stmt.expression, ctx, scope_start);
            let switch_span = switch_stmt.body.span();
            record_breakable_scope(switch_span.start.offset, switch_span.end.offset, ctx);
            ctx.switch_scopes
                .push((switch_span.start.offset, switch_span.end.offset));
            extract_from_switch_body(&switch_stmt.body, ctx, scope_start);
        }
        Statement::Try(try_stmt) => {
            for s in try_stmt.block.statements.iter() {
                extract_from_statement(s, ctx, scope_start);
            }
            for catch in try_stmt.catch_clauses.iter() {
                // Catch type hint is a navigable class reference.
                extract_from_hint_ctx(&catch.hint, &mut ctx.spans, ClassRefContext::Catch);
                // The caught variable.
                if let Some(ref var) = catch.variable {
                    let var_name = var.name.strip_prefix('$').unwrap_or(var.name).to_string();
                    ctx.spans.push(SymbolSpan {
                        start: var.span.start.offset,
                        end: var.span.end.offset,
                        kind: SymbolKind::Variable {
                            name: var_name.clone(),
                        },
                    });
                    // Emit VarDefSite for catch variable.
                    let catch_var_offset = var.span.start.offset;
                    ctx.var_defs.push(VarDefSite {
                        offset: catch_var_offset,
                        name: var_name,
                        kind: VarDefKind::Catch,
                        scope_start,
                        effective_from: catch_var_offset,
                    });
                }
                for s in catch.block.statements.iter() {
                    extract_from_statement(s, ctx, scope_start);
                }
            }
            if let Some(ref finally) = try_stmt.finally_clause {
                for s in finally.block.statements.iter() {
                    extract_from_statement(s, ctx, scope_start);
                }
            }
        }
        Statement::Block(block) => {
            for s in block.statements.iter() {
                extract_from_statement(s, ctx, scope_start);
            }
        }
        Statement::Global(global) => {
            for var in global.variables.iter() {
                if let Variable::Direct(dv) = var {
                    let name = dv.name.strip_prefix('$').unwrap_or(dv.name).to_string();
                    ctx.spans.push(SymbolSpan {
                        start: dv.span.start.offset,
                        end: dv.span.end.offset,
                        kind: SymbolKind::Variable { name: name.clone() },
                    });
                    // Emit VarDefSite for global variable.
                    let global_offset = dv.span.start.offset;
                    ctx.var_defs.push(VarDefSite {
                        offset: global_offset,
                        name,
                        kind: VarDefKind::GlobalDecl,
                        scope_start,
                        effective_from: global_offset,
                    });
                }
            }
        }
        Statement::Static(static_stmt) => {
            for item in static_stmt.items.iter() {
                let dv = item.variable();
                let name = dv.name.strip_prefix('$').unwrap_or(dv.name).to_string();
                ctx.spans.push(SymbolSpan {
                    start: dv.span.start.offset,
                    end: dv.span.end.offset,
                    kind: SymbolKind::Variable { name: name.clone() },
                });
                // Emit VarDefSite for static variable.
                let static_offset = dv.span.start.offset;
                ctx.var_defs.push(VarDefSite {
                    offset: static_offset,
                    name,
                    kind: VarDefKind::StaticDecl,
                    scope_start,
                    effective_from: static_offset,
                });
            }
        }
        Statement::Unset(unset_stmt) => {
            for val in unset_stmt.values.iter() {
                extract_from_expression(val, ctx, scope_start);
            }
        }
        Statement::Constant(constant) => {
            // Top-level `const FOO = Expr;` — walk value expressions so
            // that class references like `Foo::class` produce spans.
            extract_from_attribute_lists(&constant.attribute_lists, ctx, scope_start);
            for item in constant.items.iter() {
                extract_from_expression(item.value, ctx, scope_start);
            }
        }
        Statement::Declare(declare) => {
            // `declare(strict_types=1) { ... }` — walk the body if present.
            match &declare.body {
                DeclareBody::Statement(inner) => {
                    extract_from_statement(inner, ctx, scope_start);
                }
                DeclareBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        extract_from_statement(s, ctx, scope_start);
                    }
                }
            }
        }
        Statement::EchoTag(echo_tag) => {
            // `<?= $expr ?>` — walk expressions inside short echo tags.
            for expr in echo_tag.values.iter() {
                extract_from_expression(expr, ctx, scope_start);
            }
        }
        _ => {}
    }
}

// ─── If / While / For / Switch body helpers ─────────────────────────────────

fn record_breakable_scope(start: u32, end: u32, ctx: &mut ExtractionCtx<'_>) {
    if start <= end {
        ctx.breakable_scopes.push((start, end));
    }
}

fn record_loop_scope(start: u32, end: u32, ctx: &mut ExtractionCtx<'_>) {
    if start <= end {
        ctx.loop_scopes.push((start, end));
    }
}

fn extract_from_if_body<'a>(body: &'a IfBody<'a>, ctx: &mut ExtractionCtx<'a>, scope_start: u32) {
    match body {
        IfBody::Statement(stmt_body) => {
            // Record then-body as a narrowing block.
            let then_span = stmt_body.statement.span();
            ctx.narrowing_blocks
                .push((then_span.start.offset, then_span.end.offset));
            extract_from_statement(stmt_body.statement, ctx, scope_start);
            for else_if in stmt_body.else_if_clauses.iter() {
                extract_from_expression(else_if.condition, ctx, scope_start);
                // Record elseif-body as a narrowing block.
                let ei_span = else_if.statement.span();
                ctx.narrowing_blocks
                    .push((ei_span.start.offset, ei_span.end.offset));
                extract_from_statement(else_if.statement, ctx, scope_start);
            }
            if let Some(ref else_clause) = stmt_body.else_clause {
                // Record else-body as a narrowing block.
                let el_span = else_clause.statement.span();
                ctx.narrowing_blocks
                    .push((el_span.start.offset, el_span.end.offset));
                extract_from_statement(else_clause.statement, ctx, scope_start);
            }
        }
        IfBody::ColonDelimited(colon_body) => {
            // Record the then-body span (first statement to last).
            if let (Some(first), Some(last)) =
                (colon_body.statements.first(), colon_body.statements.last())
            {
                ctx.narrowing_blocks
                    .push((first.span().start.offset, last.span().end.offset));
            }
            for inner in colon_body.statements.iter() {
                extract_from_statement(inner, ctx, scope_start);
            }
            for else_if in colon_body.else_if_clauses.iter() {
                extract_from_expression(else_if.condition, ctx, scope_start);
                if let (Some(first), Some(last)) =
                    (else_if.statements.first(), else_if.statements.last())
                {
                    ctx.narrowing_blocks
                        .push((first.span().start.offset, last.span().end.offset));
                }
                for inner in else_if.statements.iter() {
                    extract_from_statement(inner, ctx, scope_start);
                }
            }
            if let Some(ref else_clause) = colon_body.else_clause {
                if let (Some(first), Some(last)) = (
                    else_clause.statements.first(),
                    else_clause.statements.last(),
                ) {
                    ctx.narrowing_blocks
                        .push((first.span().start.offset, last.span().end.offset));
                }
                for inner in else_clause.statements.iter() {
                    extract_from_statement(inner, ctx, scope_start);
                }
            }
        }
    }
}

fn extract_from_while_body<'a>(
    body: &'a WhileBody<'a>,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    match body {
        WhileBody::Statement(inner) => {
            extract_from_statement(inner, ctx, scope_start);
        }
        WhileBody::ColonDelimited(colon_body) => {
            for inner in colon_body.statements.iter() {
                extract_from_statement(inner, ctx, scope_start);
            }
        }
    }
}

fn extract_from_for_body<'a>(body: &'a ForBody<'a>, ctx: &mut ExtractionCtx<'a>, scope_start: u32) {
    match body {
        ForBody::Statement(inner) => {
            extract_from_statement(inner, ctx, scope_start);
        }
        ForBody::ColonDelimited(colon_body) => {
            for inner in colon_body.statements.iter() {
                extract_from_statement(inner, ctx, scope_start);
            }
        }
    }
}

fn extract_from_switch_body<'a>(
    body: &'a SwitchBody<'a>,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    let cases = match body {
        SwitchBody::BraceDelimited(b) => &b.cases,
        SwitchBody::ColonDelimited(b) => &b.cases,
    };
    for case in cases.iter() {
        for inner in case.statements().iter() {
            extract_from_statement(inner, ctx, scope_start);
        }
    }
}

// ─── Class-like extractors ──────────────────────────────────────────────────

fn extract_from_class<'a>(class: &'a Class<'a>, ctx: &mut ExtractionCtx<'a>) {
    // Class name — declaration site, not a reference.
    let name = class.name.value.to_string();
    ctx.spans.push(SymbolSpan {
        start: class.name.span.start.offset,
        end: class.name.span.end.offset,
        kind: SymbolKind::ClassDeclaration { name },
    });

    // Attributes (PHP 8).
    extract_from_attribute_lists(&class.attribute_lists, ctx, 0);

    // Extends.
    if let Some(ref extends) = class.extends {
        for ident in extends.types.iter() {
            let raw = ident.value().to_string();
            ctx.spans.push(class_ref_span_ctx(
                ident.span().start.offset,
                ident.span().end.offset,
                &raw,
                ClassRefContext::ExtendsClass,
            ));
        }
    }

    // Implements.
    if let Some(ref implements) = class.implements {
        for ident in implements.types.iter() {
            let raw = ident.value().to_string();
            ctx.spans.push(class_ref_span_ctx(
                ident.span().start.offset,
                ident.span().end.offset,
                &raw,
                ClassRefContext::Implements,
            ));
        }
    }

    // Docblock.
    if let Some((doc_text, doc_offset)) =
        get_docblock_text_with_offset(ctx.trivias, ctx.content, class)
    {
        let tpl_params = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        let scope_end = class.right_brace.end.offset;
        for (name, name_offset, bound, variance) in tpl_params {
            ctx.template_defs.push(TemplateParamDef {
                name_offset,
                name,
                bound,
                variance,
                scope_start: doc_offset,
                scope_end,
            });
        }
    }

    // Members.
    for member in class.members.iter() {
        extract_from_class_member(member, ctx);
    }
}

fn extract_from_interface<'a>(iface: &'a Interface<'a>, ctx: &mut ExtractionCtx<'a>) {
    // Interface name — declaration site, not a reference.
    let name = iface.name.value.to_string();
    ctx.spans.push(SymbolSpan {
        start: iface.name.span.start.offset,
        end: iface.name.span.end.offset,
        kind: SymbolKind::ClassDeclaration { name },
    });

    // Attributes (PHP 8).
    extract_from_attribute_lists(&iface.attribute_lists, ctx, 0);

    if let Some(ref extends) = iface.extends {
        for ident in extends.types.iter() {
            let raw = ident.value().to_string();
            ctx.spans.push(class_ref_span_ctx(
                ident.span().start.offset,
                ident.span().end.offset,
                &raw,
                ClassRefContext::ExtendsInterface,
            ));
        }
    }

    if let Some((doc_text, doc_offset)) =
        get_docblock_text_with_offset(ctx.trivias, ctx.content, iface)
    {
        let tpl_params = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        let scope_end = iface.right_brace.end.offset;
        for (name, name_offset, bound, variance) in tpl_params {
            ctx.template_defs.push(TemplateParamDef {
                name_offset,
                name,
                bound,
                variance,
                scope_start: doc_offset,
                scope_end,
            });
        }
    }

    for member in iface.members.iter() {
        extract_from_class_member(member, ctx);
    }
}

fn extract_from_trait<'a>(trait_def: &'a Trait<'a>, ctx: &mut ExtractionCtx<'a>) {
    // Trait name — declaration site, not a reference.
    let name = trait_def.name.value.to_string();
    ctx.spans.push(SymbolSpan {
        start: trait_def.name.span.start.offset,
        end: trait_def.name.span.end.offset,
        kind: SymbolKind::ClassDeclaration { name },
    });

    // Attributes (PHP 8).
    extract_from_attribute_lists(&trait_def.attribute_lists, ctx, 0);

    if let Some((doc_text, doc_offset)) =
        get_docblock_text_with_offset(ctx.trivias, ctx.content, trait_def)
    {
        let tpl_params = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        let scope_end = trait_def.right_brace.end.offset;
        for (name, name_offset, bound, variance) in tpl_params {
            ctx.template_defs.push(TemplateParamDef {
                name_offset,
                name,
                bound,
                variance,
                scope_start: doc_offset,
                scope_end,
            });
        }
    }

    for member in trait_def.members.iter() {
        extract_from_class_member(member, ctx);
    }
}

fn extract_from_enum<'a>(enum_def: &'a Enum<'a>, ctx: &mut ExtractionCtx<'a>) {
    // Enum name — declaration site, not a reference.
    let name = enum_def.name.value.to_string();
    ctx.spans.push(SymbolSpan {
        start: enum_def.name.span.start.offset,
        end: enum_def.name.span.end.offset,
        kind: SymbolKind::ClassDeclaration { name },
    });

    // Attributes (PHP 8).
    extract_from_attribute_lists(&enum_def.attribute_lists, ctx, 0);

    if let Some(ref implements) = enum_def.implements {
        for ident in implements.types.iter() {
            let raw = ident.value().to_string();
            ctx.spans.push(class_ref_span_ctx(
                ident.span().start.offset,
                ident.span().end.offset,
                &raw,
                ClassRefContext::Implements,
            ));
        }
    }

    if let Some((doc_text, doc_offset)) =
        get_docblock_text_with_offset(ctx.trivias, ctx.content, enum_def)
    {
        let tpl_params = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        let scope_end = enum_def.right_brace.end.offset;
        for (name, name_offset, bound, variance) in tpl_params {
            ctx.template_defs.push(TemplateParamDef {
                name_offset,
                name,
                bound,
                variance,
                scope_start: doc_offset,
                scope_end,
            });
        }
    }

    for member in enum_def.members.iter() {
        extract_from_class_member(member, ctx);
    }
}

// ─── Class member extractors ────────────────────────────────────────────────

/// Extract symbols from PHP 8 attribute lists (`#[Attr(...)]`).
///
/// Emits a `ClassReference` for the attribute class name and recurses
/// into argument expressions.
fn extract_from_attribute_lists<'a>(
    attribute_lists: &mago_syntax::ast::sequence::Sequence<
        'a,
        mago_syntax::ast::attribute::AttributeList<'a>,
    >,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    for attr_list in attribute_lists.iter() {
        for attr in attr_list.attributes.iter() {
            // The attribute name (e.g. `\Illuminate\...\CollectedBy`).
            let raw = attr.name.value().to_string();
            ctx.spans.push(class_ref_span(
                attr.name.span().start.offset,
                attr.name.span().end.offset,
                &raw,
            ));

            // Attribute arguments — also emit a CallSite so that
            // signature help and named parameter completion work
            // inside `#[Attr(...)]` just like `new Attr(...)`.
            if let Some(ref arg_list) = attr.argument_list {
                extract_from_arguments(&arg_list.arguments, ctx, scope_start);
                let class_name = raw.trim_start_matches('\\');
                if !class_name.is_empty() {
                    emit_call_site(
                        format!("new {}", class_name),
                        arg_list,
                        &mut ctx.call_sites,
                        &mut ctx.untyped_closure_sites,
                    );
                }
            }
        }
    }
}

fn extract_from_class_member<'a>(member: &'a ClassLikeMember<'a>, ctx: &mut ExtractionCtx<'a>) {
    match member {
        ClassLikeMember::Method(method) => {
            extract_from_method(method, ctx);
        }
        ClassLikeMember::Property(property) => {
            extract_from_property(property, ctx);
        }
        ClassLikeMember::Constant(constant) => {
            extract_from_class_constant(constant, ctx);
        }
        ClassLikeMember::TraitUse(trait_use) => {
            // Process the docblock attached to the trait use statement
            // so that `@use Trait<TModel>` generic args get spans.
            if let Some((doc_text, doc_offset)) =
                get_docblock_text_with_offset(ctx.trivias, ctx.content, trait_use)
            {
                let _tpl = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
            }

            for ident in trait_use.trait_names.iter() {
                let raw = ident.value().to_string();
                ctx.spans.push(class_ref_span_ctx(
                    ident.span().start.offset,
                    ident.span().end.offset,
                    &raw,
                    ClassRefContext::TraitUse,
                ));
            }

            // Extract symbols from trait use adaptations (`{ ... }` block)
            // so that go-to-definition works on method names and trait
            // references inside `as` alias and `insteadof` declarations.
            if let TraitUseSpecification::Concrete(spec) = &trait_use.specification {
                // Collect trait names from the `use` list so we can use the
                // first one as a fallback subject for unqualified method
                // references (e.g. `method as alias` without `Trait::method`).
                let first_trait_name: Option<String> = trait_use
                    .trait_names
                    .iter()
                    .next()
                    .map(|id| id.value().to_string());

                for adaptation in spec.adaptations.iter() {
                    match adaptation {
                        TraitUseAdaptation::Alias(alias_adapt) => {
                            extract_from_trait_alias_adaptation(
                                alias_adapt,
                                first_trait_name.as_deref(),
                                ctx,
                            );
                        }
                        TraitUseAdaptation::Precedence(prec) => {
                            extract_from_trait_precedence_adaptation(prec, ctx);
                        }
                    }
                }
            }
        }
        ClassLikeMember::EnumCase(enum_case) => {
            // Attributes (PHP 8) on the enum case.
            extract_from_attribute_lists(&enum_case.attribute_lists, ctx, 0);

            // Enum case name — declaration site span for find-references,
            // rename, and document-highlights.  Enum cases are accessed
            // statically (`self::Issue`, `TaskType::Issue`).
            let case_name_ident = enum_case.item.name();
            ctx.spans.push(SymbolSpan {
                start: case_name_ident.span.start.offset,
                end: case_name_ident.span.end.offset,
                kind: SymbolKind::MemberDeclaration {
                    name: case_name_ident.value.to_string(),
                    is_static: true,
                },
            });

            // Enum case values (backed enums).
            if let EnumCaseItem::Backed(backed) = &enum_case.item {
                extract_from_expression(backed.value, ctx, 0);
            }
        }
    }
}

/// Extract symbol spans from a trait `as` alias adaptation.
///
/// For `TraitA::method as alias`:
///   - `TraitA` gets a `ClassReference` span
///   - `method` gets a `MemberAccess` span (subject = `TraitA`, static call)
///   - `alias` gets a `MemberAccess` span (subject = `self`) so that
///     `resolve_trait_alias` maps it back to the original method
///
/// For unqualified `method as alias`:
///   - `method` gets a `MemberAccess` span using the first trait in the
///     `use` list as the subject (or `self` as fallback)
///   - `alias` gets a `MemberAccess` span (subject = `self`)
fn extract_from_trait_alias_adaptation<'a>(
    alias_adapt: &'a TraitUseAliasAdaptation<'a>,
    first_trait_name: Option<&str>,
    ctx: &mut ExtractionCtx<'a>,
) {
    match &alias_adapt.method_reference {
        TraitUseMethodReference::Absolute(abs) => {
            // Emit ClassReference for the trait name.
            let trait_raw = abs.trait_name.value().to_string();
            ctx.spans.push(class_ref_span(
                abs.trait_name.span().start.offset,
                abs.trait_name.span().end.offset,
                &trait_raw,
            ));
            // Emit MemberAccess for the original method name.
            let method_name = abs.method_name.value.to_string();
            ctx.spans.push(SymbolSpan {
                start: abs.method_name.span.start.offset,
                end: abs.method_name.span.end.offset,
                kind: SymbolKind::MemberAccess {
                    subject_text: trait_raw,
                    member_name: method_name,
                    is_static: true,
                    is_method_call: true,
                    is_docblock_reference: false,
                },
            });
        }
        TraitUseMethodReference::Identifier(ident) => {
            // Unqualified reference: use the first trait name from the
            // `use` list, or fall back to `self`.
            let subject = first_trait_name.unwrap_or("self").to_string();
            let method_name = ident.value.to_string();
            ctx.spans.push(SymbolSpan {
                start: ident.span.start.offset,
                end: ident.span.end.offset,
                kind: SymbolKind::MemberAccess {
                    subject_text: subject,
                    member_name: method_name,
                    is_static: true,
                    is_method_call: true,
                    is_docblock_reference: false,
                },
            });
        }
    }

    // Emit MemberAccess for the alias name (the `as` target).
    // Using `self` as the subject so that `resolve_trait_alias` on
    // the owning class maps the alias back to the original method.
    if let Some(ref alias_ident) = alias_adapt.alias {
        let alias_name = alias_ident.value.to_string();
        ctx.spans.push(SymbolSpan {
            start: alias_ident.span.start.offset,
            end: alias_ident.span.end.offset,
            kind: SymbolKind::MemberAccess {
                subject_text: "self".to_string(),
                member_name: alias_name,
                is_static: true,
                is_method_call: true,
                is_docblock_reference: false,
            },
        });
    }
}

/// Extract symbol spans from a trait `insteadof` precedence adaptation.
///
/// For `TraitA::method insteadof TraitB, TraitC`:
///   - `TraitA` gets a `ClassReference` span
///   - `method` gets a `MemberAccess` span (subject = `TraitA`, static call)
///   - `TraitB` and `TraitC` each get a `ClassReference` span
fn extract_from_trait_precedence_adaptation<'a>(
    prec: &'a TraitUsePrecedenceAdaptation<'a>,
    ctx: &mut ExtractionCtx<'a>,
) {
    // Emit ClassReference for the trait name in the method reference.
    let trait_raw = prec.method_reference.trait_name.value().to_string();
    ctx.spans.push(class_ref_span(
        prec.method_reference.trait_name.span().start.offset,
        prec.method_reference.trait_name.span().end.offset,
        &trait_raw,
    ));

    // Emit MemberAccess for the method name.
    let method_name = prec.method_reference.method_name.value.to_string();
    ctx.spans.push(SymbolSpan {
        start: prec.method_reference.method_name.span.start.offset,
        end: prec.method_reference.method_name.span.end.offset,
        kind: SymbolKind::MemberAccess {
            subject_text: trait_raw,
            member_name: method_name,
            is_static: true,
            is_method_call: true,
            is_docblock_reference: false,
        },
    });

    // Emit ClassReference for each `insteadof` trait name.
    for ident in prec.trait_names.iter() {
        let raw = ident.value().to_string();
        ctx.spans.push(class_ref_span(
            ident.span().start.offset,
            ident.span().end.offset,
            &raw,
        ));
    }
}

fn extract_from_method<'a>(method: &'a Method<'a>, ctx: &mut ExtractionCtx<'a>) {
    // Method name — declaration site span for find-references and rename.
    let is_static = method.modifiers.iter().any(|m| m.is_static());
    ctx.spans.push(SymbolSpan {
        start: method.name.span.start.offset,
        end: method.name.span.end.offset,
        kind: SymbolKind::MemberDeclaration {
            name: method.name.value.to_string(),
            is_static,
        },
    });

    // Attributes (PHP 8) on the method.
    extract_from_attribute_lists(&method.attribute_lists, ctx, 0);

    // Docblock on the method.  We extract type spans and template params
    // now, but defer `@param $var` variable spans until after we know
    // `method_scope_start` (the body's opening-brace offset).
    let method_docblock = get_docblock_text_with_offset(ctx.trivias, ctx.content, method);
    if let Some((doc_text, doc_offset)) = method_docblock {
        let tpl_params = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        // Method-level template params: scope extends from the docblock to
        // the end of the method body (or the end of the docblock for
        // abstract methods without a body).
        let scope_end = if let MethodBody::Concrete(body) = &method.body {
            body.right_brace.end.offset
        } else {
            // Abstract / interface method — scope is just the docblock + signature.
            // Use the method span end as a reasonable bound.
            method.span().end.offset
        };
        for (name, name_offset, bound, variance) in tpl_params {
            ctx.template_defs.push(TemplateParamDef {
                name_offset,
                name,
                bound,
                variance,
                scope_start: doc_offset,
                scope_end,
            });
        }
    }

    // Determine scope_start for this method body.
    let method_scope_start = if let MethodBody::Concrete(body) = &method.body {
        let s = body.left_brace.start.offset;
        let e = body.right_brace.end.offset;
        ctx.scopes.push((s, e));
        s
    } else {
        0
    };

    // Emit Variable spans and VarDefSite markers for `@param $varName`
    // tokens in the docblock so that rename and find-references cover
    // them.  The VarDefSite with `DocblockParam` kind lets
    // `find_variable_scope` map the pre-body offset to the correct
    // function body scope.
    if let Some((doc_text, doc_offset)) = method_docblock {
        for (name, file_offset) in extract_param_var_spans(doc_text, doc_offset) {
            let end = file_offset + 1 + name.len() as u32;
            ctx.spans.push(SymbolSpan {
                start: file_offset,
                end,
                kind: SymbolKind::Variable { name: name.clone() },
            });
            ctx.var_defs.push(VarDefSite {
                offset: file_offset,
                name,
                kind: VarDefKind::DocblockParam,
                scope_start: method_scope_start,
                effective_from: file_offset,
            });
        }
    }

    // Parameter type hints, variable spans, and variable definition sites.
    for param in method.parameter_list.parameters.iter() {
        // Attributes (PHP 8) on the parameter.
        extract_from_attribute_lists(&param.attribute_lists, ctx, 0);
        if let Some(ref hint) = param.hint {
            extract_from_hint_ctx(hint, &mut ctx.spans, ClassRefContext::TypeHint);
        }
        // Docblock attached to the parameter itself (e.g. promoted
        // constructor properties with `/** @var list<Subscription> */`).
        if let Some((doc_text, doc_offset)) =
            get_docblock_text_with_offset(ctx.trivias, ctx.content, param)
        {
            let _tpl = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        }
        let name = param
            .variable
            .name
            .strip_prefix('$')
            .unwrap_or(param.variable.name)
            .to_string();
        let param_offset = param.variable.span.start.offset;
        // Emit a Variable span so the symbol map covers the parameter
        // token itself (needed for GTD-from-parameter-to-type-hint).
        ctx.spans.push(SymbolSpan {
            start: param_offset,
            end: param.variable.span.end.offset,
            kind: SymbolKind::Variable { name: name.clone() },
        });
        ctx.var_defs.push(VarDefSite {
            offset: param_offset,
            name,
            kind: VarDefKind::Parameter,
            scope_start: method_scope_start,
            effective_from: param_offset,
        });
        if let Some(ref default) = param.default_value {
            extract_from_expression(default.value, ctx, method_scope_start);
        }
    }

    // Return type hint.
    if let Some(ref return_type) = method.return_type_hint {
        extract_from_hint_ctx(&return_type.hint, &mut ctx.spans, ClassRefContext::TypeHint);
    }

    // Method body.
    if let MethodBody::Concrete(body) = &method.body {
        for stmt in body.statements.iter() {
            extract_from_statement(stmt, ctx, method_scope_start);
        }
    }
}

/// Extract docblock symbols from an inline `/** @var ... */` comment
/// attached to a body-level statement (expression, return, echo, etc.).
///
/// These comments are stored as trivia preceding the statement token.
/// Unlike class/method docblocks, inline `@var` annotations don't define
/// template parameters — we only care about the type spans they contain.
fn extract_inline_docblock(node: &impl HasSpan, ctx: &mut ExtractionCtx<'_>) {
    if let Some((doc_text, doc_offset)) =
        get_docblock_text_with_offset(ctx.trivias, ctx.content, node)
    {
        let _tpl = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
    }
}

fn extract_from_property<'a>(property: &Property<'a>, ctx: &mut ExtractionCtx<'a>) {
    // Attributes (PHP 8) on the property.
    match property {
        Property::Plain(plain) => extract_from_attribute_lists(&plain.attribute_lists, ctx, 0),
        Property::Hooked(hooked) => extract_from_attribute_lists(&hooked.attribute_lists, ctx, 0),
    }

    // Docblock.
    if let Some((doc_text, doc_offset)) =
        get_docblock_text_with_offset(ctx.trivias, ctx.content, property)
    {
        // Property docblocks don't define template params, but we still
        // need to consume the return value.
        let _tpl = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
    }

    // Property type hint.
    if let Some(hint) = property.hint() {
        extract_from_hint_ctx(hint, &mut ctx.spans, ClassRefContext::TypeHint);
    }

    // Property variable names and default value expressions.
    match property {
        Property::Plain(plain) => {
            for item in plain.items.iter() {
                let var = item.variable();
                let name = var.name.strip_prefix('$').unwrap_or(var.name).to_string();
                let var_offset = var.span.start.offset;
                ctx.spans.push(SymbolSpan {
                    start: var_offset,
                    end: var.span.end.offset,
                    kind: SymbolKind::Variable { name: name.clone() },
                });
                ctx.var_defs.push(VarDefSite {
                    offset: var_offset,
                    name,
                    kind: VarDefKind::Property,
                    scope_start: 0,
                    effective_from: var_offset,
                });
                // Walk the default value expression so that class
                // references like `Foo::class` in property defaults
                // produce navigable spans.
                if let PropertyItem::Concrete(concrete) = item {
                    extract_from_expression(concrete.value, ctx, 0);
                }
            }
        }
        Property::Hooked(hooked) => {
            let var = hooked.item.variable();
            let name = var.name.strip_prefix('$').unwrap_or(var.name).to_string();
            let var_offset = var.span.start.offset;
            ctx.spans.push(SymbolSpan {
                start: var_offset,
                end: var.span.end.offset,
                kind: SymbolKind::Variable { name: name.clone() },
            });
            ctx.var_defs.push(VarDefSite {
                offset: var_offset,
                name,
                kind: VarDefKind::Property,
                scope_start: 0,
                effective_from: var_offset,
            });
            if let PropertyItem::Concrete(concrete) = &hooked.item {
                extract_from_expression(concrete.value, ctx, 0);
            }
        }
    }
}

fn extract_from_class_constant<'a>(
    constant: &'a ClassLikeConstant<'a>,
    ctx: &mut ExtractionCtx<'a>,
) {
    // Attributes (PHP 8) on the constant.
    extract_from_attribute_lists(&constant.attribute_lists, ctx, 0);

    // Constant name(s) — declaration site spans for find-references and rename.
    // Class constants are always accessed statically (Foo::CONST).
    for item in constant.items.iter() {
        ctx.spans.push(SymbolSpan {
            start: item.name.span.start.offset,
            end: item.name.span.end.offset,
            kind: SymbolKind::MemberDeclaration {
                name: item.name.value.to_string(),
                is_static: true,
            },
        });
    }

    // Docblock.
    if let Some((doc_text, doc_offset)) =
        get_docblock_text_with_offset(ctx.trivias, ctx.content, constant)
    {
        let _tpl = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
    }

    // Type hint on constant (PHP 8.3+).
    if let Some(ref hint) = constant.hint {
        extract_from_hint_ctx(hint, &mut ctx.spans, ClassRefContext::TypeHint);
    }

    // Constant value expressions.
    for item in constant.items.iter() {
        extract_from_expression(item.value, ctx, 0);
    }
}

// ─── Function extractor ─────────────────────────────────────────────────────

fn extract_from_function<'a>(func: &'a Function<'a>, ctx: &mut ExtractionCtx<'a>) {
    // Attributes (PHP 8) on the function.
    extract_from_attribute_lists(&func.attribute_lists, ctx, 0);

    // Function name as a navigable reference.
    let name = func.name.value.to_string();
    ctx.spans.push(SymbolSpan {
        start: func.name.span.start.offset,
        end: func.name.span.end.offset,
        kind: SymbolKind::FunctionCall {
            name,
            is_definition: true,
        },
    });

    // Docblock.  We extract type spans and template params now, but
    // defer `@param $var` variable spans until after we know
    // `func_scope_start` (the body's opening-brace offset).
    let func_docblock = get_docblock_text_with_offset(ctx.trivias, ctx.content, func);
    if let Some((doc_text, doc_offset)) = func_docblock {
        let tpl_params = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        let scope_end = func.body.right_brace.end.offset;
        for (name, name_offset, bound, variance) in tpl_params {
            ctx.template_defs.push(TemplateParamDef {
                name_offset,
                name,
                bound,
                variance,
                scope_start: doc_offset,
                scope_end,
            });
        }
    }

    // Determine scope_start for this function body.
    let func_scope_start = func.body.left_brace.start.offset;
    let func_scope_end = func.body.right_brace.end.offset;
    ctx.scopes.push((func_scope_start, func_scope_end));

    // Emit Variable spans and VarDefSite markers for `@param $varName`
    // tokens in the docblock so that rename and find-references cover
    // them.  The VarDefSite with `DocblockParam` kind lets
    // `find_variable_scope` map the pre-body offset to the correct
    // function body scope.
    if let Some((doc_text, doc_offset)) = func_docblock {
        for (name, file_offset) in extract_param_var_spans(doc_text, doc_offset) {
            let end = file_offset + 1 + name.len() as u32;
            ctx.spans.push(SymbolSpan {
                start: file_offset,
                end,
                kind: SymbolKind::Variable { name: name.clone() },
            });
            ctx.var_defs.push(VarDefSite {
                offset: file_offset,
                name,
                kind: VarDefKind::DocblockParam,
                scope_start: func_scope_start,
                effective_from: file_offset,
            });
        }
    }

    // Parameter type hints, variable spans, and variable definition sites.
    for param in func.parameter_list.parameters.iter() {
        // Attributes (PHP 8) on the parameter.
        extract_from_attribute_lists(&param.attribute_lists, ctx, 0);
        if let Some(ref hint) = param.hint {
            extract_from_hint_ctx(hint, &mut ctx.spans, ClassRefContext::TypeHint);
        }
        // Docblock attached to the parameter itself (e.g. `/** @var list<Foo> */`).
        if let Some((doc_text, doc_offset)) =
            get_docblock_text_with_offset(ctx.trivias, ctx.content, param)
        {
            let _tpl = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
        }
        // Emit VarDefSite for each parameter.
        let pname = param
            .variable
            .name
            .strip_prefix('$')
            .unwrap_or(param.variable.name)
            .to_string();
        let param_offset = param.variable.span.start.offset;
        // Emit a Variable span so the symbol map covers the parameter
        // token itself (needed for GTD-from-parameter-to-type-hint).
        ctx.spans.push(SymbolSpan {
            start: param_offset,
            end: param.variable.span.end.offset,
            kind: SymbolKind::Variable {
                name: pname.clone(),
            },
        });
        ctx.var_defs.push(VarDefSite {
            offset: param_offset,
            name: pname,
            kind: VarDefKind::Parameter,
            scope_start: func_scope_start,
            effective_from: param_offset,
        });
        if let Some(ref default) = param.default_value {
            extract_from_expression(default.value, ctx, func_scope_start);
        }
    }

    // Return type hint.
    if let Some(ref return_type) = func.return_type_hint {
        extract_from_hint_ctx(&return_type.hint, &mut ctx.spans, ClassRefContext::TypeHint);
    }

    // Function body.
    for stmt in func.body.statements.iter() {
        extract_from_statement(stmt, ctx, func_scope_start);
    }
}

// ─── Use statement extractor ────────────────────────────────────────────────

fn extract_from_use_statement(use_stmt: &Use<'_>, spans: &mut Vec<SymbolSpan>) {
    fn register_use_item(item: &UseItem<'_>, spans: &mut Vec<SymbolSpan>) {
        let raw = item.name.value().to_string();
        // Use statement names are always fully qualified (even without a
        // leading `\`), so force `is_fqn = true`.  `class_ref_span`
        // derives the flag from a leading `\` which use statements omit.
        let name = strip_fqn_prefix(&raw).to_string();
        spans.push(SymbolSpan {
            start: item.name.span().start.offset,
            end: item.name.span().end.offset,
            kind: SymbolKind::ClassReference {
                name,
                is_fqn: true,
                context: ClassRefContext::Other,
            },
        });
    }

    match &use_stmt.items {
        UseItems::Sequence(seq) => {
            for use_item in seq.items.iter() {
                register_use_item(use_item, spans);
            }
        }
        UseItems::TypedSequence(typed_seq) => {
            // Only class imports (not function/const).
            if !typed_seq.r#type.is_function() && !typed_seq.r#type.is_const() {
                for use_item in typed_seq.items.iter() {
                    register_use_item(use_item, spans);
                }
            }
        }
        UseItems::TypedList(list) => {
            if !list.r#type.is_function() && !list.r#type.is_const() {
                for use_item in list.items.iter() {
                    register_use_item(use_item, spans);
                }
            }
        }
        UseItems::MixedList(list) => {
            for use_item in list.items.iter() {
                // MixedList items are MaybeTypedUseItem — skip function/const.
                if let Some(ref typ) = use_item.r#type
                    && (typ.is_function() || typ.is_const())
                {
                    continue;
                }
                register_use_item(&use_item.item, spans);
            }
        }
    }
}

// ─── Type hint extractor ────────────────────────────────────────────────────

/// Extract navigable symbols from a type hint, tagging emitted
/// `ClassReference` spans with the given [`ClassRefContext`].
fn extract_from_hint_ctx(hint: &Hint<'_>, spans: &mut Vec<SymbolSpan>, ref_ctx: ClassRefContext) {
    match hint {
        Hint::Identifier(ident) => {
            let raw = ident.value().to_string();
            let name_clean = strip_fqn_prefix(&raw).to_string();
            if is_navigable_type(&name_clean) {
                spans.push(class_ref_span_ctx(
                    ident.span().start.offset,
                    ident.span().end.offset,
                    &raw,
                    ref_ctx,
                ));
            }
        }
        Hint::Nullable(nullable) => {
            extract_from_hint_ctx(nullable.hint, spans, ref_ctx);
        }
        Hint::Union(union) => {
            extract_from_hint_ctx(union.left, spans, ref_ctx);
            extract_from_hint_ctx(union.right, spans, ref_ctx);
        }
        Hint::Intersection(intersection) => {
            extract_from_hint_ctx(intersection.left, spans, ref_ctx);
            extract_from_hint_ctx(intersection.right, spans, ref_ctx);
        }
        Hint::Parenthesized(paren) => {
            extract_from_hint_ctx(paren.hint, spans, ref_ctx);
        }
        Hint::Self_(kw) => {
            spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Self_),
            });
        }
        Hint::Static(kw) => {
            spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Static),
            });
        }
        Hint::Parent(kw) => {
            spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Parent),
            });
        }
        // Scalar / built-in type hints are not navigable.
        _ => {}
    }
}

// ─── Expression extractor ───────────────────────────────────────────────────

fn extract_from_expression<'a>(
    expr: &'a Expression<'a>,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    match expr {
        // ── Variables ──
        Expression::Variable(Variable::Direct(dv)) => {
            let raw = dv.name;
            if raw == "$this" {
                // `$this` is semantically equivalent to `static` for
                // go-to-definition — resolve it to the enclosing class.
                ctx.spans.push(SymbolSpan {
                    start: dv.span.start.offset,
                    end: dv.span.end.offset,
                    kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::This),
                });
            } else {
                let name = raw.strip_prefix('$').unwrap_or(raw).to_string();
                ctx.spans.push(SymbolSpan {
                    start: dv.span.start.offset,
                    end: dv.span.end.offset,
                    kind: SymbolKind::Variable { name },
                });
            }
        }

        // ── self / static / parent keywords ──
        Expression::Self_(kw) => {
            ctx.spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Self_),
            });
        }
        Expression::Static(kw) => {
            ctx.spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Static),
            });
        }
        Expression::Parent(kw) => {
            ctx.spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Parent),
            });
        }

        // ── Identifiers (standalone class/constant references) ──
        Expression::Identifier(ident) => {
            let name = ident.value().to_string();
            let name_clean = strip_fqn_prefix(&name).to_string();
            if is_navigable_type(&name_clean) {
                ctx.spans.push(class_ref_span(
                    ident.span().start.offset,
                    ident.span().end.offset,
                    &name,
                ));
            }
        }

        // ── Instantiation: `new Foo(...)` ──
        Expression::Instantiation(inst) => {
            match inst.class {
                Expression::Identifier(ident) => {
                    let raw = ident.value().to_string();
                    ctx.spans.push(class_ref_span_ctx(
                        ident.span().start.offset,
                        ident.span().end.offset,
                        &raw,
                        ClassRefContext::New,
                    ));
                }
                Expression::Self_(kw) => {
                    ctx.spans.push(SymbolSpan {
                        start: kw.span.start.offset,
                        end: kw.span.end.offset,
                        kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Self_),
                    });
                }
                Expression::Static(kw) => {
                    ctx.spans.push(SymbolSpan {
                        start: kw.span.start.offset,
                        end: kw.span.end.offset,
                        kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Static),
                    });
                }
                Expression::Parent(kw) => {
                    ctx.spans.push(SymbolSpan {
                        start: kw.span.start.offset,
                        end: kw.span.end.offset,
                        kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Parent),
                    });
                }
                _ => {
                    extract_from_expression(inst.class, ctx, scope_start);
                }
            }
            if let Some(ref args) = inst.argument_list {
                // Emit call site for constructor: `new ClassName(...)`
                let class_text = expr_to_subject_text(inst.class);
                if !class_text.is_empty() {
                    emit_call_site(
                        format!("new {}", class_text),
                        args,
                        &mut ctx.call_sites,
                        &mut ctx.untyped_closure_sites,
                    );
                }
                extract_from_arguments(&args.arguments, ctx, scope_start);
            }
        }

        // ── Function calls ──
        Expression::Call(call) => match call {
            Call::Function(func_call) => {
                match func_call.function {
                    Expression::Identifier(ident) => {
                        let name = ident.value().to_string();
                        let name_clean = strip_fqn_prefix(&name).to_string();
                        ctx.spans.push(SymbolSpan {
                            start: ident.span().start.offset,
                            end: ident.span().end.offset,
                            kind: SymbolKind::FunctionCall {
                                name: name_clean.clone(),
                                is_definition: false,
                            },
                        });
                        if name_clean.eq_ignore_ascii_case("config") {
                            try_emit_config_key_span(
                                &func_call.argument_list,
                                ctx.content,
                                &mut ctx.spans,
                            );
                        }
                    }
                    _ => {
                        extract_from_expression(func_call.function, ctx, scope_start);
                    }
                }
                // Emit call site for function call
                let func_text = expr_to_subject_text(func_call.function);
                if !func_text.is_empty() {
                    emit_call_site(
                        func_text,
                        &func_call.argument_list,
                        &mut ctx.call_sites,
                        &mut ctx.untyped_closure_sites,
                    );
                }
                extract_from_arguments(&func_call.argument_list.arguments, ctx, scope_start);
            }
            Call::Method(method_call) => {
                let subject_text = expr_to_subject_text(method_call.object);
                extract_from_expression(method_call.object, ctx, scope_start);

                if let ClassLikeMemberSelector::Identifier(ident) = &method_call.method {
                    let member_name = ident.value.to_string();
                    if is_laravel_config_repository_call(method_call.object, &member_name) {
                        try_emit_config_key_span(
                            &method_call.argument_list,
                            ctx.content,
                            &mut ctx.spans,
                        );
                    }
                    // Emit call site for method call: `$subject->method(...)`
                    emit_call_site(
                        format!("{}->{}", &subject_text, &member_name),
                        &method_call.argument_list,
                        &mut ctx.call_sites,
                        &mut ctx.untyped_closure_sites,
                    );
                    ctx.spans.push(SymbolSpan {
                        start: ident.span.start.offset,
                        end: ident.span.end.offset,
                        kind: SymbolKind::MemberAccess {
                            subject_text,
                            member_name,
                            is_static: false,
                            is_method_call: true,
                            is_docblock_reference: false,
                        },
                    });
                }
                extract_from_arguments(&method_call.argument_list.arguments, ctx, scope_start);
            }
            Call::NullSafeMethod(method_call) => {
                let subject_text = expr_to_subject_text(method_call.object);
                extract_from_expression(method_call.object, ctx, scope_start);

                if let ClassLikeMemberSelector::Identifier(ident) = &method_call.method {
                    let member_name = ident.value.to_string();
                    if is_laravel_config_repository_call(method_call.object, &member_name) {
                        try_emit_config_key_span(
                            &method_call.argument_list,
                            ctx.content,
                            &mut ctx.spans,
                        );
                    }
                    // Emit call site for null-safe method call.
                    // Use `->` so resolve_callable handles it the same
                    // as regular method calls.
                    emit_call_site(
                        format!("{}->{}", &subject_text, &member_name),
                        &method_call.argument_list,
                        &mut ctx.call_sites,
                        &mut ctx.untyped_closure_sites,
                    );
                    ctx.spans.push(SymbolSpan {
                        start: ident.span.start.offset,
                        end: ident.span.end.offset,
                        kind: SymbolKind::MemberAccess {
                            subject_text,
                            member_name,
                            is_static: false,
                            is_method_call: true,
                            is_docblock_reference: false,
                        },
                    });
                }
                extract_from_arguments(&method_call.argument_list.arguments, ctx, scope_start);
            }
            Call::StaticMethod(static_call) => {
                let subject_text = expr_to_subject_text(static_call.class);
                emit_class_expr_span(static_call.class, ctx, scope_start);

                if let ClassLikeMemberSelector::Identifier(ident) = &static_call.method {
                    let member_name = ident.value.to_string();
                    // Emit call site for static method call: `Class::method(...)`
                    emit_call_site(
                        format!("{}::{}", &subject_text, &member_name),
                        &static_call.argument_list,
                        &mut ctx.call_sites,
                        &mut ctx.untyped_closure_sites,
                    );
                    ctx.spans.push(SymbolSpan {
                        start: ident.span.start.offset,
                        end: ident.span.end.offset,
                        kind: SymbolKind::MemberAccess {
                            subject_text: subject_text.clone(),
                            member_name: member_name.clone(),
                            is_static: true,
                            is_method_call: true,
                            is_docblock_reference: false,
                        },
                    });
                    let clean_subject = strip_fqn_prefix(&subject_text);
                    if (clean_subject.eq_ignore_ascii_case("Config")
                        || clean_subject
                            .eq_ignore_ascii_case("Illuminate\\Support\\Facades\\Config"))
                        && is_config_repository_method(&member_name)
                    {
                        try_emit_config_key_span(
                            &static_call.argument_list,
                            ctx.content,
                            &mut ctx.spans,
                        );
                    }
                }
                extract_from_arguments(&static_call.argument_list.arguments, ctx, scope_start);
            }
        },

        // ── Property / constant access ──
        Expression::Access(access) => {
            match access {
                Access::Property(pa) => {
                    let subject_text = expr_to_subject_text(pa.object);
                    extract_from_expression(pa.object, ctx, scope_start);

                    if let ClassLikeMemberSelector::Identifier(ident) = &pa.property {
                        let member_name = ident.value.to_string();
                        ctx.spans.push(SymbolSpan {
                            start: ident.span.start.offset,
                            end: ident.span.end.offset,
                            kind: SymbolKind::MemberAccess {
                                subject_text,
                                member_name,
                                is_static: false,
                                is_method_call: false,
                                is_docblock_reference: false,
                            },
                        });
                    }
                }
                Access::NullSafeProperty(pa) => {
                    let subject_text = expr_to_subject_text(pa.object);
                    extract_from_expression(pa.object, ctx, scope_start);

                    if let ClassLikeMemberSelector::Identifier(ident) = &pa.property {
                        let member_name = ident.value.to_string();
                        ctx.spans.push(SymbolSpan {
                            start: ident.span.start.offset,
                            end: ident.span.end.offset,
                            kind: SymbolKind::MemberAccess {
                                subject_text,
                                member_name,
                                is_static: false,
                                is_method_call: false,
                                is_docblock_reference: false,
                            },
                        });
                    }
                }
                Access::StaticProperty(spa) => {
                    let subject_text = expr_to_subject_text(spa.class);
                    emit_class_expr_span(spa.class, ctx, scope_start);

                    if let Variable::Direct(dv) = &spa.property {
                        let prop_name = dv.name.strip_prefix('$').unwrap_or(dv.name).to_string();
                        ctx.spans.push(SymbolSpan {
                            start: dv.span.start.offset,
                            end: dv.span.end.offset,
                            kind: SymbolKind::MemberAccess {
                                subject_text,
                                member_name: prop_name,
                                is_static: true,
                                is_method_call: false,
                                is_docblock_reference: false,
                            },
                        });
                    }
                }
                Access::ClassConstant(cca) => {
                    let subject_text = expr_to_subject_text(cca.class);
                    emit_class_expr_span(cca.class, ctx, scope_start);

                    if let ClassLikeConstantSelector::Identifier(ident) = &cca.constant {
                        let const_name = ident.value.to_string();
                        if const_name == "class" {
                            // `Foo::class` — the navigable part is `Foo`.
                        } else {
                            ctx.spans.push(SymbolSpan {
                                start: ident.span.start.offset,
                                end: ident.span.end.offset,
                                kind: SymbolKind::MemberAccess {
                                    subject_text,
                                    member_name: const_name,
                                    is_static: true,
                                    is_method_call: false,
                                    is_docblock_reference: false,
                                },
                            });
                        }
                    }
                }
            }
        }

        // ── Assignment ──
        Expression::Assignment(assign) => {
            extract_from_expression(assign.lhs, ctx, scope_start);
            extract_from_expression(assign.rhs, ctx, scope_start);

            // The definition only becomes visible *after* the entire
            // assignment expression — the RHS still sees the previous
            // definition of the variable.
            let effective = assign.span().end.offset;

            // Emit VarDefSite for simple variable assignments: `$var = ...`
            match assign.lhs {
                Expression::Variable(Variable::Direct(dv)) => {
                    let name = dv.name.strip_prefix('$').unwrap_or(dv.name).to_string();
                    let kind = if assign.operator.is_assign() {
                        VarDefKind::Assignment
                    } else {
                        VarDefKind::CompoundAssignment
                    };
                    ctx.var_defs.push(VarDefSite {
                        offset: dv.span.start.offset,
                        name,
                        kind,
                        scope_start,
                        effective_from: effective,
                    });
                }
                // Array destructuring: `[$a, $b] = ...`
                Expression::Array(arr) => {
                    collect_destructuring_var_defs(
                        &arr.elements,
                        &mut ctx.var_defs,
                        scope_start,
                        VarDefKind::ArrayDestructuring,
                        effective,
                    );
                }
                // List destructuring: `list($a, $b) = ...`
                Expression::List(list) => {
                    collect_destructuring_var_defs(
                        &list.elements,
                        &mut ctx.var_defs,
                        scope_start,
                        VarDefKind::ListDestructuring,
                        effective,
                    );
                }
                _ => {}
            }
        }

        // ── Binary operations ──
        Expression::Binary(bin) => {
            extract_from_expression(bin.lhs, ctx, scope_start);
            // Tag the RHS of `instanceof` with the Instanceof context.
            if bin.operator.is_instanceof() {
                if let Expression::Identifier(ident) = bin.rhs {
                    let raw = ident.value().to_string();
                    ctx.spans.push(class_ref_span_ctx(
                        ident.span().start.offset,
                        ident.span().end.offset,
                        &raw,
                        ClassRefContext::Instanceof,
                    ));
                } else {
                    extract_from_expression(bin.rhs, ctx, scope_start);
                }
            } else {
                extract_from_expression(bin.rhs, ctx, scope_start);
            }
        }

        // ── Unary operations ──
        Expression::UnaryPrefix(un) => {
            extract_from_expression(un.operand, ctx, scope_start);
        }
        Expression::UnaryPostfix(un) => {
            extract_from_expression(un.operand, ctx, scope_start);
        }

        // ── Parenthesized ──
        Expression::Parenthesized(paren) => {
            extract_from_expression(paren.expression, ctx, scope_start);
        }

        // ── Ternary ──
        Expression::Conditional(ternary) => {
            extract_from_expression(ternary.condition, ctx, scope_start);
            if let Some(then_branch) = ternary.then {
                extract_from_expression(then_branch, ctx, scope_start);
            }
            extract_from_expression(ternary.r#else, ctx, scope_start);
        }

        // ── Array ──
        Expression::Array(array) => {
            extract_from_array_elements(&array.elements, ctx, scope_start);
        }
        Expression::LegacyArray(array) => {
            extract_from_array_elements(&array.elements, ctx, scope_start);
        }
        Expression::List(list) => {
            extract_from_array_elements(&list.elements, ctx, scope_start);
        }

        // ── Array access ──
        Expression::ArrayAccess(access) => {
            extract_from_expression(access.array, ctx, scope_start);
            extract_from_expression(access.index, ctx, scope_start);
        }

        // ── Closures / arrow functions ──
        Expression::Closure(closure) => {
            // Closure introduces a new scope.
            let closure_scope_start = closure.body.left_brace.start.offset;
            let closure_scope_end = closure.body.right_brace.end.offset;
            ctx.scopes.push((closure_scope_start, closure_scope_end));
            ctx.body_scopes
                .push((closure_scope_start, closure_scope_end));

            for param in closure.parameter_list.parameters.iter() {
                // Attributes (PHP 8) on the parameter.
                extract_from_attribute_lists(&param.attribute_lists, ctx, 0);
                if let Some(ref hint) = param.hint {
                    extract_from_hint_ctx(hint, &mut ctx.spans, ClassRefContext::TypeHint);
                }
                let name = param
                    .variable
                    .name
                    .strip_prefix('$')
                    .unwrap_or(param.variable.name)
                    .to_string();
                ctx.spans.push(SymbolSpan {
                    start: param.variable.span.start.offset,
                    end: param.variable.span.end.offset,
                    kind: SymbolKind::Variable { name: name.clone() },
                });
                // Emit VarDefSite for closure parameter.
                let cp_offset = param.variable.span.start.offset;
                ctx.var_defs.push(VarDefSite {
                    offset: cp_offset,
                    name,
                    kind: VarDefKind::Parameter,
                    scope_start: closure_scope_start,
                    effective_from: cp_offset,
                });
                if let Some(ref default) = param.default_value {
                    extract_from_expression(default.value, ctx, closure_scope_start);
                }
            }
            if let Some(ref use_clause) = closure.use_clause {
                for var in use_clause.variables.iter() {
                    let name = var
                        .variable
                        .name
                        .strip_prefix('$')
                        .unwrap_or(var.variable.name)
                        .to_string();
                    ctx.spans.push(SymbolSpan {
                        start: var.variable.span.start.offset,
                        end: var.variable.span.end.offset,
                        kind: SymbolKind::Variable { name: name.clone() },
                    });
                    // Emit VarDefSite so that GTD inside the closure body
                    // can find the captured variable.  The definition is
                    // scoped to the closure body and immediately visible.
                    let use_var_offset = var.variable.span.start.offset;
                    ctx.var_defs.push(VarDefSite {
                        offset: use_var_offset,
                        name,
                        kind: VarDefKind::ClosureCapture,
                        scope_start: closure_scope_start,
                        effective_from: use_var_offset,
                    });
                }
            }
            if let Some(ref return_type) = closure.return_type_hint {
                extract_from_hint_ctx(&return_type.hint, &mut ctx.spans, ClassRefContext::TypeHint);
            }
            for s in closure.body.statements.iter() {
                extract_from_statement(s, ctx, closure_scope_start);
            }
        }
        Expression::ArrowFunction(arrow) => {
            // Arrow functions introduce a new scope for their parameters.
            // They don't have braces, so use the span of the arrow function itself.
            let arrow_scope_start = arrow.span().start.offset;
            let arrow_scope_end = arrow.span().end.offset;
            ctx.scopes.push((arrow_scope_start, arrow_scope_end));
            // Body scope starts at `=>` for signature help suppression.
            ctx.body_scopes
                .push((arrow.arrow.start.offset, arrow_scope_end));

            for param in arrow.parameter_list.parameters.iter() {
                // Attributes (PHP 8) on the parameter.
                extract_from_attribute_lists(&param.attribute_lists, ctx, 0);
                if let Some(ref hint) = param.hint {
                    extract_from_hint_ctx(hint, &mut ctx.spans, ClassRefContext::TypeHint);
                }
                let name = param
                    .variable
                    .name
                    .strip_prefix('$')
                    .unwrap_or(param.variable.name)
                    .to_string();
                ctx.spans.push(SymbolSpan {
                    start: param.variable.span.start.offset,
                    end: param.variable.span.end.offset,
                    kind: SymbolKind::Variable { name: name.clone() },
                });
                // Emit VarDefSite for arrow function parameter.
                let ap_offset = param.variable.span.start.offset;
                ctx.var_defs.push(VarDefSite {
                    offset: ap_offset,
                    name,
                    kind: VarDefKind::Parameter,
                    scope_start: arrow_scope_start,
                    effective_from: ap_offset,
                });
                if let Some(ref default) = param.default_value {
                    extract_from_expression(default.value, ctx, arrow_scope_start);
                }
            }
            if let Some(ref return_type) = arrow.return_type_hint {
                extract_from_hint_ctx(&return_type.hint, &mut ctx.spans, ClassRefContext::TypeHint);
            }
            extract_from_expression(arrow.expression, ctx, arrow_scope_start);
        }

        // ── Match expression ──
        Expression::Match(match_expr) => {
            extract_from_expression(match_expr.expression, ctx, scope_start);
            for arm in match_expr.arms.iter() {
                match arm {
                    MatchArm::Expression(arm) => {
                        for cond in arm.conditions.iter() {
                            extract_from_expression(cond, ctx, scope_start);
                        }
                        extract_from_expression(arm.expression, ctx, scope_start);
                    }
                    MatchArm::Default(arm) => {
                        extract_from_expression(arm.expression, ctx, scope_start);
                    }
                }
            }
        }

        // ── Throw expression (PHP 8) ──
        Expression::Throw(throw_expr) => {
            extract_from_expression(throw_expr.exception, ctx, scope_start);
        }

        // ── Yield ──
        Expression::Yield(yield_expr) => match yield_expr {
            Yield::Value(yv) => {
                if let Some(value) = yv.value {
                    extract_from_expression(value, ctx, scope_start);
                }
            }
            Yield::Pair(yp) => {
                extract_from_expression(yp.key, ctx, scope_start);
                extract_from_expression(yp.value, ctx, scope_start);
            }
            Yield::From(yf) => {
                extract_from_expression(yf.iterator, ctx, scope_start);
            }
        },

        // ── Clone ──
        Expression::Clone(clone) => {
            extract_from_expression(clone.object, ctx, scope_start);
        }

        // ── Anonymous class ──
        // `new class(...) extends Foo implements Bar { ... }`
        Expression::AnonymousClass(anon) => {
            // Constructor arguments.
            if let Some(ref args) = anon.argument_list {
                extract_from_arguments(&args.arguments, ctx, scope_start);
            }

            // Extends.
            if let Some(ref extends) = anon.extends {
                for ident in extends.types.iter() {
                    let raw = ident.value().to_string();
                    ctx.spans.push(class_ref_span(
                        ident.span().start.offset,
                        ident.span().end.offset,
                        &raw,
                    ));
                }
            }

            // Implements.
            if let Some(ref implements) = anon.implements {
                for ident in implements.types.iter() {
                    let raw = ident.value().to_string();
                    ctx.spans.push(class_ref_span(
                        ident.span().start.offset,
                        ident.span().end.offset,
                        &raw,
                    ));
                }
            }

            // Attributes on the anonymous class.
            extract_from_attribute_lists(&anon.attribute_lists, ctx, scope_start);

            // Docblock.
            if let Some((doc_text, doc_offset)) =
                get_docblock_text_with_offset(ctx.trivias, ctx.content, anon)
            {
                let _tpl = extract_docblock_symbols(doc_text, doc_offset, &mut ctx.spans);
            }

            // Members.
            for member in anon.members.iter() {
                extract_from_class_member(member, ctx);
            }
        }

        // ── Language constructs ──
        // `isset($a, $b)`, `empty($x)`, `eval(...)`, `print(...)`,
        // `include ...`, `require ...`, `exit(...)`, `die(...)`
        Expression::Construct(construct) => match construct {
            Construct::Isset(isset) => {
                for val in isset.values.iter() {
                    extract_from_expression(val, ctx, scope_start);
                }
            }
            Construct::Empty(empty) => {
                extract_from_expression(empty.value, ctx, scope_start);
            }
            Construct::Eval(eval) => {
                extract_from_expression(eval.value, ctx, scope_start);
            }
            Construct::Include(inc) => {
                extract_from_expression(inc.value, ctx, scope_start);
            }
            Construct::IncludeOnce(inc) => {
                extract_from_expression(inc.value, ctx, scope_start);
            }
            Construct::Require(req) => {
                extract_from_expression(req.value, ctx, scope_start);
            }
            Construct::RequireOnce(req) => {
                extract_from_expression(req.value, ctx, scope_start);
            }
            Construct::Print(print) => {
                extract_from_expression(print.value, ctx, scope_start);
            }
            Construct::Exit(exit) => {
                if let Some(ref args) = exit.arguments {
                    extract_from_arguments(&args.arguments, ctx, scope_start);
                }
            }
            Construct::Die(die) => {
                if let Some(ref args) = die.arguments {
                    extract_from_arguments(&args.arguments, ctx, scope_start);
                }
            }
        },

        // ── Composite strings (interpolation) ──
        // `"Hello {$obj->method()}"`, heredocs, shell-exec backticks.
        Expression::CompositeString(composite) => {
            for part in composite.parts().iter() {
                match part {
                    StringPart::Expression(expr) => {
                        extract_from_expression(expr, ctx, scope_start);
                    }
                    StringPart::BracedExpression(braced) => {
                        extract_from_expression(braced.expression, ctx, scope_start);
                    }
                    StringPart::Literal(_) => {}
                }
            }
        }

        // ── Array append ──
        // `$arr[]` — the array expression is navigable.
        Expression::ArrayAppend(append) => {
            extract_from_expression(append.array, ctx, scope_start);
        }

        // ── Standalone constant access ──
        // `PHP_EOL`, `SORT_ASC`, `PHPStan\PHP_VERSION_ID`, etc.
        // The parser produces `ConstantAccess` for all standalone
        // constant references — including namespaced ones.  These are
        // never class names, so always emit `ConstantReference`.
        Expression::ConstantAccess(ca) => {
            let name = ca.name.value().to_string();
            let name_clean = strip_fqn_prefix(&name).to_string();
            ctx.spans.push(SymbolSpan {
                start: ca.name.span().start.offset,
                end: ca.name.span().end.offset,
                kind: SymbolKind::ConstantReference { name: name_clean },
            });
        }

        // ── Pipe operator (PHP 8.5) ──
        // `$value |> transform(...)`
        Expression::Pipe(pipe) => {
            extract_from_expression(pipe.input, ctx, scope_start);
            extract_from_expression(pipe.callable, ctx, scope_start);
        }

        // ── First-class callable / partial application ──
        // `strlen(...)`, `$obj->method(...)`, `Class::method(...)`
        Expression::PartialApplication(partial) => match partial {
            PartialApplication::Function(func_pa) => match func_pa.function {
                Expression::Identifier(ident) => {
                    let name = ident.value().to_string();
                    let name_clean = strip_fqn_prefix(&name).to_string();
                    ctx.spans.push(SymbolSpan {
                        start: ident.span().start.offset,
                        end: ident.span().end.offset,
                        kind: SymbolKind::FunctionCall {
                            name: name_clean,
                            is_definition: false,
                        },
                    });
                }
                _ => {
                    extract_from_expression(func_pa.function, ctx, scope_start);
                }
            },
            PartialApplication::Method(method_pa) => {
                let subject_text = expr_to_subject_text(method_pa.object);
                extract_from_expression(method_pa.object, ctx, scope_start);
                if let ClassLikeMemberSelector::Identifier(ident) = &method_pa.method {
                    let member_name = ident.value.to_string();
                    ctx.spans.push(SymbolSpan {
                        start: ident.span.start.offset,
                        end: ident.span.end.offset,
                        kind: SymbolKind::MemberAccess {
                            subject_text,
                            member_name,
                            is_static: false,
                            is_method_call: true,
                            is_docblock_reference: false,
                        },
                    });
                }
            }
            PartialApplication::StaticMethod(static_pa) => {
                let subject_text = expr_to_subject_text(static_pa.class);
                emit_class_expr_span(static_pa.class, ctx, scope_start);
                if let ClassLikeMemberSelector::Identifier(ident) = &static_pa.method {
                    let member_name = ident.value.to_string();
                    ctx.spans.push(SymbolSpan {
                        start: ident.span.start.offset,
                        end: ident.span.end.offset,
                        kind: SymbolKind::MemberAccess {
                            subject_text,
                            member_name,
                            is_static: true,
                            is_method_call: true,
                            is_docblock_reference: false,
                        },
                    });
                }
            }
        },

        // Non-navigable expressions (literals, etc.) are intentionally ignored.
        _ => {}
    }
}

/// Collect variable definition sites from a destructuring pattern
/// (`[$a, $b] = ...` or `list($a, $b) = ...`).
fn collect_destructuring_var_defs(
    elements: &TokenSeparatedSequence<'_, ArrayElement<'_>>,
    var_defs: &mut Vec<VarDefSite>,
    scope_start: u32,
    kind: VarDefKind,
    effective_from: u32,
) {
    for element in elements.iter() {
        let value_expr = match element {
            ArrayElement::KeyValue(kv) => kv.value,
            ArrayElement::Value(val) => val.value,
            _ => continue,
        };
        match value_expr {
            Expression::Variable(Variable::Direct(dv)) => {
                let name = dv.name.strip_prefix('$').unwrap_or(dv.name).to_string();
                var_defs.push(VarDefSite {
                    offset: dv.span.start.offset,
                    name,
                    kind: kind.clone(),
                    scope_start,
                    effective_from,
                });
            }
            // Nested destructuring: `[[$a, $b], $c] = ...`
            Expression::Array(arr) => {
                collect_destructuring_var_defs(
                    &arr.elements,
                    var_defs,
                    scope_start,
                    kind.clone(),
                    effective_from,
                );
            }
            Expression::List(list) => {
                collect_destructuring_var_defs(
                    &list.elements,
                    var_defs,
                    scope_start,
                    kind.clone(),
                    effective_from,
                );
            }
            _ => {}
        }
    }
}

// ─── Shared helpers ─────────────────────────────────────────────────────────

/// Walk an argument list and extract symbols from each argument expression.
fn extract_from_arguments<'a>(
    args: &TokenSeparatedSequence<'a, Argument<'a>>,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    for arg in args.iter() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        extract_from_expression(arg_expr, ctx, scope_start);
    }
}

/// Walk array elements and extract symbols from each element expression.
fn extract_from_array_elements<'a>(
    elements: &TokenSeparatedSequence<'a, ArrayElement<'a>>,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    for element in elements.iter() {
        match element {
            ArrayElement::KeyValue(kv) => {
                extract_from_expression(kv.key, ctx, scope_start);
                extract_from_expression(kv.value, ctx, scope_start);
            }
            ArrayElement::Value(val) => {
                extract_from_expression(val.value, ctx, scope_start);
            }
            ArrayElement::Variadic(variadic) => {
                extract_from_expression(variadic.value, ctx, scope_start);
            }
            _ => {}
        }
    }
}

/// For the class part of a static call/property/constant access, emit
/// the appropriate span (ClassReference, SelfStaticParent, or recurse).
fn emit_class_expr_span<'a>(
    expr: &'a Expression<'a>,
    ctx: &mut ExtractionCtx<'a>,
    scope_start: u32,
) {
    match expr {
        Expression::Identifier(ident) => {
            let raw = ident.value().to_string();
            ctx.spans.push(class_ref_span(
                ident.span().start.offset,
                ident.span().end.offset,
                &raw,
            ));
        }
        Expression::Self_(kw) => {
            ctx.spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Self_),
            });
        }
        Expression::Static(kw) => {
            ctx.spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Static),
            });
        }
        Expression::Parent(kw) => {
            ctx.spans.push(SymbolSpan {
                start: kw.span.start.offset,
                end: kw.span.end.offset,
                kind: SymbolKind::SelfStaticParent(SelfStaticParentKind::Parent),
            });
        }
        _ => {
            extract_from_expression(expr, ctx, scope_start);
        }
    }
}

// ─── Call site emission ─────────────────────────────────────────────────────

/// Build and push a [`CallSite`] from an argument list and its call expression string.
fn emit_call_site(
    call_expression: String,
    argument_list: &ArgumentList<'_>,
    call_sites: &mut Vec<CallSite>,
    untyped_closure_sites: &mut Vec<UntypedClosureSite>,
) {
    if call_expression.is_empty() {
        return;
    }
    let args_start = argument_list.left_parenthesis.end.offset;
    let args_end = argument_list.right_parenthesis.start.offset;
    let comma_offsets: Vec<u32> = argument_list
        .arguments
        .tokens
        .iter()
        .map(|t| t.start.offset)
        .collect();

    let arg_count = argument_list.arguments.len() as u32;

    // Collect the byte offset of each argument's start token and
    // track which arguments use named syntax (`name: value`).
    let mut arg_offsets = Vec::with_capacity(arg_count as usize);
    let mut named_arg_indices = Vec::new();
    let mut named_arg_names = Vec::new();
    let mut spread_arg_indices = Vec::new();
    for (i, arg) in argument_list.arguments.iter().enumerate() {
        match arg {
            Argument::Positional(pos) => {
                // If unpacking is used, the `...` token comes before the
                // value expression.  Use the ellipsis offset when present
                // so the hint appears before `...`.
                let offset = pos
                    .ellipsis
                    .as_ref()
                    .map(|e| e.start.offset)
                    .unwrap_or_else(|| pos.value.span().start.offset);
                arg_offsets.push(offset);
                if pos.ellipsis.is_some() {
                    spread_arg_indices.push(i as u32);
                }
            }
            Argument::Named(named) => {
                arg_offsets.push(named.name.span.start.offset);
                named_arg_indices.push(i as u32);
                named_arg_names.push(named.name.value.to_string());
            }
        }
    }

    // Detect argument unpacking (`...$args`).  Only positional
    // arguments can use the spread operator; the AST stores it as
    // `ellipsis: Some(Span)` on `PositionalArgument`.
    let has_unpacking = argument_list
        .arguments
        .iter()
        .any(|arg| matches!(arg, Argument::Positional(pos) if pos.ellipsis.is_some()));

    // Check arguments for closures/arrows with untyped parameters or
    // missing return types.
    for (arg_idx, arg) in argument_list.arguments.iter().enumerate() {
        let expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        collect_untyped_closure_site(expr, &call_expression, arg_idx, untyped_closure_sites);
    }

    call_sites.push(CallSite {
        args_start,
        args_end,
        call_expression,
        comma_offsets,
        arg_offsets,
        arg_count,
        has_unpacking,
        named_arg_indices,
        named_arg_names,
        spread_arg_indices,
    });
}

/// If `expr` is a closure or arrow function, collect an [`UntypedClosureSite`]
/// with its untyped parameters and (optionally) its close-paren offset for a
/// return type hint.
fn collect_untyped_closure_site(
    expr: &Expression<'_>,
    parent_call_expression: &str,
    arg_index: usize,
    out: &mut Vec<UntypedClosureSite>,
) {
    let (params, close_paren_offset, has_return_type) = match expr {
        Expression::Closure(c) => (
            &c.parameter_list.parameters,
            c.parameter_list.span().end.offset,
            c.return_type_hint.is_some(),
        ),
        Expression::ArrowFunction(a) => (
            &a.parameter_list.parameters,
            a.parameter_list.span().end.offset,
            a.return_type_hint.is_some(),
        ),
        _ => return,
    };

    let mut untyped_params = Vec::new();
    for (param_idx, param) in params.iter().enumerate() {
        if param.hint.is_none() {
            untyped_params.push((param_idx, param.variable.span.start.offset));
        }
    }

    // Only emit a site if there is something for inlay hints to show:
    // untyped parameters or a missing return type.
    if untyped_params.is_empty() && has_return_type {
        return;
    }

    out.push(UntypedClosureSite {
        parent_call_expression: parent_call_expression.to_string(),
        arg_index_in_parent: arg_index,
        close_paren_offset: if has_return_type {
            None
        } else {
            Some(close_paren_offset)
        },
        untyped_params,
    });
}

// ─── Expression to subject text ─────────────────────────────────────────────

/// Convert an AST expression to the subject text string that
/// `resolve_target_classes` expects.
fn expr_to_subject_text(expr: &Expression<'_>) -> String {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
        Expression::Self_(_) => "self".to_string(),
        Expression::Static(_) => "static".to_string(),
        Expression::Parent(_) => "parent".to_string(),
        Expression::Identifier(ident) => ident.value().to_string(),

        Expression::Access(Access::Property(pa)) => {
            let obj = expr_to_subject_text(pa.object);
            if let ClassLikeMemberSelector::Identifier(ident) = &pa.property {
                format!("{}->{}", obj, ident.value)
            } else {
                obj
            }
        }
        Expression::Access(Access::NullSafeProperty(pa)) => {
            let obj = expr_to_subject_text(pa.object);
            if let ClassLikeMemberSelector::Identifier(ident) = &pa.property {
                format!("{}?->{}", obj, ident.value)
            } else {
                obj
            }
        }
        Expression::Access(Access::StaticProperty(spa)) => {
            let class_text = expr_to_subject_text(spa.class);
            if let Variable::Direct(dv) = &spa.property {
                format!("{}::{}", class_text, dv.name)
            } else {
                class_text
            }
        }
        Expression::Access(Access::ClassConstant(cca)) => {
            let class_text = expr_to_subject_text(cca.class);
            match &cca.constant {
                ClassLikeConstantSelector::Identifier(ident) => {
                    format!("{}::{}", class_text, ident.value)
                }
                _ => class_text,
            }
        }

        Expression::Call(Call::Method(mc)) => {
            let obj = expr_to_subject_text(mc.object);
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let args_text = format_all_call_args(&mc.argument_list.arguments);
                format!("{}->{}({})", obj, ident.value, args_text)
            } else {
                format!("{}->?()", obj)
            }
        }
        Expression::Call(Call::NullSafeMethod(mc)) => {
            let obj = expr_to_subject_text(mc.object);
            if let ClassLikeMemberSelector::Identifier(ident) = &mc.method {
                let args_text = format_all_call_args(&mc.argument_list.arguments);
                format!("{}?->{}({})", obj, ident.value, args_text)
            } else {
                format!("{}?->?()", obj)
            }
        }
        Expression::Call(Call::StaticMethod(sc)) => {
            let class_text = expr_to_subject_text(sc.class);
            if let ClassLikeMemberSelector::Identifier(ident) = &sc.method {
                let args_text = format_all_call_args(&sc.argument_list.arguments);
                format!("{}::{}({})", class_text, ident.value, args_text)
            } else {
                format!("{}::?()", class_text)
            }
        }
        Expression::Call(Call::Function(fc)) => {
            let func_text = expr_to_subject_text(fc.function);
            let args_text = format_all_call_args(&fc.argument_list.arguments);
            // When the callee is a parenthesized expression (e.g.
            // `($this->formatter)()`), wrap the inner text back in
            // parens so that `SubjectExpr::parse` sees
            // `($this->formatter)()` rather than `$this->formatter()`
            // (which would be parsed as a method call).
            if matches!(fc.function, Expression::Parenthesized(_)) {
                format!("({})({})", func_text, args_text)
            } else {
                format!("{}({})", func_text, args_text)
            }
        }

        Expression::Instantiation(inst) => expr_to_subject_text(inst.class),

        Expression::Parenthesized(paren) => expr_to_subject_text(paren.expression),

        // `clone $expr` preserves the type of the operand.
        Expression::Clone(clone) => expr_to_subject_text(clone.object),

        // Array literals: `[Foo::class, 'bar']` → `[Foo::class, 'bar']`.
        // We format elements we can represent and elide the rest so that
        // callers (especially conditional return-type resolution) can see
        // that an argument was provided and is not null.
        Expression::Array(array) => {
            let mut parts = Vec::new();
            for element in array.elements.iter() {
                match element {
                    mago_syntax::ast::ArrayElement::KeyValue(kv) => {
                        let val = expr_to_subject_text(kv.value);
                        if !val.is_empty() {
                            let key = expr_to_subject_text(kv.key);
                            if key.is_empty() {
                                parts.push(val);
                            } else {
                                parts.push(format!("{} => {}", key, val));
                            }
                        } else {
                            parts.push("...".to_string());
                        }
                    }
                    mago_syntax::ast::ArrayElement::Value(v) => {
                        let val = expr_to_subject_text(v.value);
                        if val.is_empty() {
                            parts.push("...".to_string());
                        } else {
                            parts.push(val);
                        }
                    }
                    mago_syntax::ast::ArrayElement::Variadic(v) => {
                        let val = expr_to_subject_text(v.value);
                        if val.is_empty() {
                            parts.push("...".to_string());
                        } else {
                            parts.push(format!("...{}", val));
                        }
                    }
                    mago_syntax::ast::ArrayElement::Missing(_) => {
                        parts.push("...".to_string());
                    }
                }
            }
            format!("[{}]", parts.join(", "))
        }

        // Ternary `$a ? $b : $c` and short ternary `$a ?: $b`.
        // For short ternary (`then` is None), the condition is the
        // preferred branch; for full ternary, use the `then` branch.
        // Either way we pick one branch so the type engine has
        // something to resolve rather than an empty string.
        Expression::Conditional(cond) => {
            let preferred = cond.then.unwrap_or(cond.condition);
            let text = expr_to_subject_text(preferred);
            if !text.is_empty() {
                return text;
            }
            // Fall back to the else branch.
            expr_to_subject_text(cond.r#else)
        }

        // Null coalesce `$a ?? $b` — LHS is the preferred non-null value.
        Expression::Binary(binary) if binary.operator.is_null_coalesce() => {
            let text = expr_to_subject_text(binary.lhs);
            if !text.is_empty() {
                return text;
            }
            expr_to_subject_text(binary.rhs)
        }

        Expression::ArrayAccess(access) => {
            let base = expr_to_subject_text(access.array);
            if base.is_empty() {
                return String::new();
            }
            // Preserve string keys for array-shape resolution;
            // collapse everything else to `[]` (generic element access),
            // matching the convention used by `extract_arrow_subject`.
            let bracket = match access.index {
                Expression::Literal(Literal::String(s)) => {
                    // `s.raw` includes surrounding quotes (e.g. `'key'`).
                    // Strip them to get the bare key, then re-wrap in
                    // single quotes for the subject format.
                    let inner = crate::util::unquote_php_string(s.raw).unwrap_or(s.raw);
                    format!("['{}']", inner)
                }
                _ => "[]".to_string(),
            };
            format!("{}{}", base, bracket)
        }

        _ => String::new(),
    }
}

/// Format all arguments of a call expression as a comma-separated string.
///
/// Each argument is serialized to a text representation that preserves
/// enough information for downstream consumers:
/// - Conditional return-type resolution needs the first argument value
///   (`Foo::class`, string literals, `null`, etc.)
/// - Template parameter inference needs closure/arrow-function signatures
///   (parameter types and return type) and constructor calls (`new Foo()`)
///
/// When an argument cannot be represented, it is emitted as `...` so that
/// positional indices remain correct for template binding resolution.
fn format_all_call_args(args: &TokenSeparatedSequence<'_, Argument<'_>>) -> String {
    let mut parts = Vec::new();
    for arg in args.iter() {
        let arg_expr = match arg {
            Argument::Positional(pos) => pos.value,
            Argument::Named(named) => named.value,
        };
        let text = format_arg_expr(arg_expr);
        parts.push(text);
    }
    // Trim trailing `...` placeholders so that simple single-arg calls
    // (the common case) don't produce `method(Foo::class, ...)`.
    while parts.last().is_some_and(|p| p == "...") {
        parts.pop();
    }
    parts.join(", ")
}

/// Format a single argument expression to text.
///
/// Handles the same cases as the old `format_first_class_arg` plus
/// closure and arrow-function expressions.  For closures the full body
/// is replaced with a placeholder (`=> ...` or `{ ... }`) to keep the
/// subject text compact while preserving parameter types and return
/// type annotations that template inference depends on.
fn format_arg_expr(expr: &Expression<'_>) -> String {
    match expr {
        // Foo::class
        Expression::Access(Access::ClassConstant(cca)) => {
            if let ClassLikeConstantSelector::Identifier(ident) = &cca.constant
                && ident.value == "class"
            {
                let class_text = expr_to_subject_text(cca.class);
                return format!("{}::class", class_text);
            }
            "...".to_string()
        }
        // String literals: 'web', "guard"
        Expression::Literal(Literal::String(lit_str)) => lit_str.raw.to_string(),
        // Integer literals: 0, 42
        Expression::Literal(Literal::Integer(lit_int)) => lit_int.raw.to_string(),
        // Float literals: 3.14
        Expression::Literal(Literal::Float(lit_float)) => lit_float.raw.to_string(),
        // null
        Expression::Literal(Literal::Null(_)) => "null".to_string(),
        // true
        Expression::Literal(Literal::True(_)) => "true".to_string(),
        // false
        Expression::Literal(Literal::False(_)) => "false".to_string(),
        // $variable
        Expression::Variable(Variable::Direct(dv)) => dv.name.to_string(),
        // new ClassName(…) → "new ClassName()"
        Expression::Instantiation(inst) => {
            let class_text = expr_to_subject_text(inst.class);
            if class_text.is_empty() {
                "...".to_string()
            } else {
                format!("new {}()", class_text)
            }
        }
        // Arrow function: fn(Type $a, Type $b): ReturnType => …
        // Serialize the signature so template inference can extract
        // parameter types and the return type annotation.
        Expression::ArrowFunction(arrow) => {
            let params = format_callable_params(&arrow.parameter_list);
            let ret = arrow
                .return_type_hint
                .as_ref()
                .map(|rth| format!(": {}", crate::parser::extract_hint_type(&rth.hint)))
                .unwrap_or_default();
            format!("fn({}){} => ...", params, ret)
        }
        // Closure: function(Type $a, Type $b): ReturnType { … }
        Expression::Closure(closure) => {
            let params = format_callable_params(&closure.parameter_list);
            let ret = closure
                .return_type_hint
                .as_ref()
                .map(|rth| format!(": {}", crate::parser::extract_hint_type(&rth.hint)))
                .unwrap_or_default();
            format!("function({}){} {{ ... }}", params, ret)
        }
        // Any other expression — delegate to the general subject text
        // formatter.  Falls back to `...` when it can't be represented.
        _ => {
            let text = expr_to_subject_text(expr);
            if text.is_empty() {
                "...".to_string()
            } else {
                text
            }
        }
    }
}

/// Format a callable's parameter list as a comma-separated string of
/// `Type $name` pairs, preserving type annotations for template inference.
fn format_callable_params(params: &FunctionLikeParameterList<'_>) -> String {
    let mut parts = Vec::new();
    for param in params.parameters.iter() {
        let name = param.variable.name.to_string();
        let type_text = param
            .hint
            .as_ref()
            .map(|h| crate::parser::extract_hint_type(h).to_string());
        match type_text {
            Some(t) => parts.push(format!("{} {}", t, name)),
            None => parts.push(name),
        }
    }
    parts.join(", ")
}

/// Check whether `expr` is an `assert(… instanceof …)` call.
///
/// Returns `true` for patterns like:
/// - `assert($var instanceof Foo)`
/// - `assert($var instanceof Foo || $var instanceof Bar)`
///
/// This is intentionally loose — it does not check which variable is
/// being narrowed.  The diagnostic cache uses the result only to know
/// that *some* assert-instanceof boundary exists at this offset, which
/// is enough to split cache entries before vs after the assert.
fn is_assert_instanceof(expr: &Expression<'_>) -> bool {
    let expr = match expr {
        Expression::Parenthesized(inner) => inner.expression,
        other => other,
    };
    if let Expression::Call(Call::Function(func_call)) = expr {
        let func_name = match func_call.function {
            Expression::Identifier(ident) => ident.value(),
            _ => return false,
        };
        let func_name = func_name.strip_prefix('\\').unwrap_or(func_name);
        if !func_name.eq_ignore_ascii_case("assert") {
            return false;
        }
        if let Some(first_arg) = func_call.argument_list.arguments.iter().next() {
            let arg_expr = match first_arg {
                Argument::Positional(pos) => pos.value,
                Argument::Named(named) => named.value,
            };
            return arg_contains_instanceof(arg_expr);
        }
    }
    false
}

/// If the first argument of `argument_list` is a non-empty, non-interpolated
/// string literal, push a [`SymbolKind::LaravelStringKey`] span covering the
/// string content (inside the quotes) onto `spans`.
///
/// Called by the `config()` function-call extractor and the
/// `Config::get()` / `Config::set()` static-call extractor so that
/// find-references and go-to-definition for Laravel config keys can use
/// the pre-built symbol map instead of re-parsing every file on demand.
fn try_emit_config_key_span(
    argument_list: &ArgumentList<'_>,
    content: &str,
    spans: &mut Vec<SymbolSpan>,
) {
    let Some(first_arg) = argument_list.arguments.iter().next() else {
        return;
    };
    let Expression::Literal(literal::Literal::String(s)) = first_arg.value() else {
        return;
    };
    let inner_start = s.span.start.offset + 1;
    let inner_end = s.span.end.offset - 1;
    if inner_start >= inner_end || inner_end as usize > content.len() {
        return;
    }
    let key = &content[inner_start as usize..inner_end as usize];
    if key.is_empty() || !key.contains('.') {
        // Require at least one dot: bare keys like 'app' are not valid config paths.
        return;
    }
    spans.push(SymbolSpan {
        start: inner_start,
        end: inner_end,
        kind: SymbolKind::LaravelStringKey {
            kind: crate::symbol_map::LaravelStringKind::Config,
            key: key.to_string(),
        },
    });
}

fn is_config_repository_method(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "has"
            | "get"
            | "string"
            | "integer"
            | "float"
            | "boolean"
            | "array"
            | "collection"
            | "set"
            | "prepend"
            | "push"
    )
}

fn is_laravel_config_repository_call(object: &Expression<'_>, member_name: &str) -> bool {
    if !is_config_repository_method(member_name) {
        return false;
    }

    match object {
        Expression::Call(Call::Function(func_call)) => match func_call.function {
            Expression::Identifier(ident) => {
                strip_fqn_prefix(ident.value()).eq_ignore_ascii_case("config")
            }
            _ => false,
        },
        _ => false,
    }
}

/// Recursively check whether an expression contains an `instanceof` operator.
fn arg_contains_instanceof(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::Parenthesized(inner) => arg_contains_instanceof(inner.expression),
        Expression::UnaryPrefix(prefix) => arg_contains_instanceof(prefix.operand),
        Expression::Binary(bin) => {
            if bin.operator.is_instanceof() {
                return true;
            }
            arg_contains_instanceof(bin.lhs) || arg_contains_instanceof(bin.rhs)
        }
        _ => false,
    }
}
