/// Variable name completion.
///
/// This module handles building completion items for variable names (`$`
/// prefix) by walking the AST to collect variables visible at the cursor
/// position, respecting PHP scoping rules (function, method, closure, and
/// top-level scope).
use std::collections::HashSet;

use mago_span::HasSpan;
use mago_syntax::ast::*;
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::parser::with_parsed_program;
use crate::util::position_to_byte_offset;

impl Backend {
    /// PHP superglobal variable names (always available in any scope).
    const SUPERGLOBALS: &'static [&'static str] = &[
        "$_GET",
        "$_POST",
        "$_REQUEST",
        "$_SESSION",
        "$_COOKIE",
        "$_SERVER",
        "$_FILES",
        "$_ENV",
        "$GLOBALS",
        "$argc",
        "$argv",
    ];

    /// Maximum number of variable completions to return.
    const MAX_VARIABLE_COMPLETIONS: usize = 100;

    /// Extract the partial variable name (including `$`) that the user
    /// is currently typing at the given cursor position.
    ///
    /// Walks backward from the cursor through alphanumeric characters and
    /// underscores, then checks for a preceding `$`.  Returns `None` if
    /// no `$` is found or the result is just `"$"` with no identifier
    /// characters.
    ///
    /// Examples:
    ///   - `$us|`  → `Some("$us")`
    ///   - `$_SE|` → `Some("$_SE")`
    ///   - `$|`    → `Some("$")`  (bare dollar — show all variables)
    ///   - `foo|`  → `None`
    pub fn extract_partial_variable_name(content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        if lines.is_empty() {
            return None;
        }

        // When the cursor is past the last line (editor can send this for
        // a trailing blank line after the final newline), treat it as the
        // end of the last line so variables defined earlier are still found.
        let (line, col) = if position.line as usize >= lines.len() {
            let last = lines[lines.len() - 1];
            (last, last.chars().count())
        } else {
            let l = lines[position.line as usize];
            (l, (position.character as usize).min(l.chars().count()))
        };

        let chars: Vec<char> = line.chars().collect();

        // Walk backwards through identifier characters
        let mut i = col;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        // Must be preceded by `$`
        if i == 0 || chars[i - 1] != '$' {
            return None;
        }
        // Include the `$`
        i -= 1;

        // If preceded by another `$` (e.g. `$$var` — variable variable),
        // skip for now.
        if i > 0 && chars[i - 1] == '$' {
            return None;
        }

        // If preceded by `->` or `::`, member completion handles this
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            return None;
        }
        if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            return None;
        }

        let partial: String = chars[i..col].iter().collect();
        // Must be at least `$`
        if partial.is_empty() {
            return None;
        }

        Some(partial)
    }

    /// Build completion items for variable names visible at the cursor.
    ///
    /// Uses the mago parser to walk the AST and collect variables from
    /// the correct scope (method body, function body, closure, or
    /// top-level code).  This ensures:
    ///   - Properties (`$this->name`) are NOT listed as variables.
    ///   - Method/function parameters only appear inside their body.
    ///   - `$this` only appears inside non-static methods.
    ///   - Variables from unrelated classes/methods are excluded.
    ///
    /// Additionally, PHP superglobals (`$_GET`, `$_POST`, …) are always
    /// offered.
    ///
    /// The prefix must include the `$` (e.g. `"$us"`).
    /// Returns `(items, is_incomplete)`.
    pub(crate) fn build_variable_completions(
        content: &str,
        prefix: &str,
        position: Position,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        let cursor_offset = position_to_byte_offset(content, position) as u32;

        // Compute the replacement range: from the start of the `$` prefix
        // to the cursor position.  Using `text_edit` with an explicit range
        // prevents the double-dollar problem in editors (Helix, Neovim)
        // that don't consider `$` part of a word boundary.
        let prefix_char_len = prefix.chars().count() as u32;
        let replace_range = Range {
            start: Position {
                line: position.line,
                character: position.character.saturating_sub(prefix_char_len),
            },
            end: position,
        };

        // ── 1. AST-based scope-aware variable collection ────────────
        let scope_vars = collect_variables_in_scope(content, cursor_offset);

        for var_name in &scope_vars {
            if !var_name.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            if !seen.insert(var_name.clone()) {
                continue;
            }
            items.push(CompletionItem {
                label: var_name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("variable".to_string()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: var_name.clone(),
                })),
                filter_text: Some(var_name.clone()),
                sort_text: Some(format!("0_{}", var_name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        // ── 2. PHP superglobals ─────────────────────────────────────
        for &name in Self::SUPERGLOBALS {
            if !name.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("PHP superglobal".to_string()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: replace_range,
                    new_text: name.to_string(),
                })),
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("z_{}", name.to_lowercase())),
                deprecated: Some(true),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_VARIABLE_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_VARIABLE_COMPLETIONS);
        }

        (items, is_incomplete)
    }
}

// ─── Scope-aware variable collector ─────────────────────────────────────────

/// Collect all variable names visible at `cursor_offset` by parsing the
/// file and walking the AST to find the enclosing scope.
///
/// The returned set contains variable names including the `$` prefix
/// (e.g. `"$user"`, `"$this"`).
fn collect_variables_in_scope(content: &str, cursor_offset: u32) -> HashSet<String> {
    with_parsed_program(content, "collect_variables_in_scope", |program, content| {
        let mut vars = HashSet::new();
        find_scope_and_collect(content, program.statements.iter(), cursor_offset, &mut vars);
        vars
    })
}

/// Walk top-level statements to find the scope enclosing the cursor,
/// then collect variables from that scope.
fn find_scope_and_collect<'b>(
    content: &str,
    statements: impl Iterator<Item = &'b Statement<'b>>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    let stmts: Vec<&Statement> = statements.collect();

    // First pass: check if cursor is inside a class, function, namespace,
    // or a closure/arrow-function (which introduces a new variable scope).
    for &stmt in &stmts {
        if try_collect_from_enclosing_closure(content, stmt, cursor_offset, vars) {
            return;
        }
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(content, class.members.iter(), cursor_offset, vars);
                    return;
                }
            }
            Statement::Interface(iface) => {
                let start = iface.left_brace.start.offset;
                let end = iface.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(content, iface.members.iter(), cursor_offset, vars);
                    return;
                }
            }
            Statement::Enum(enum_def) => {
                let start = enum_def.left_brace.start.offset;
                let end = enum_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(
                        content,
                        enum_def.members.iter(),
                        cursor_offset,
                        vars,
                    );
                    return;
                }
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(
                        content,
                        trait_def.members.iter(),
                        cursor_offset,
                        vars,
                    );
                    return;
                }
            }
            Statement::Function(func) => {
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if cursor_offset >= body_start && cursor_offset <= body_end {
                    // Collect parameters
                    collect_from_params(&func.parameter_list, vars);
                    // Collect from body statements
                    collect_from_statements(
                        content,
                        func.body.statements.iter(),
                        cursor_offset,
                        vars,
                    );
                    return;
                }
            }
            Statement::Namespace(ns) => {
                let ns_span = ns.span();
                if cursor_offset >= ns_span.start.offset && cursor_offset <= ns_span.end.offset {
                    find_scope_and_collect(content, ns.statements().iter(), cursor_offset, vars);
                    return;
                }
            }
            _ => {}
        }
    }

    // If the cursor is past the end of the last statement and that
    // statement is a namespace, the user is typing at EOF inside an
    // unbraced namespace (`namespace Foo;`).  The parser's span for the
    // namespace may not extend to cover newly-typed content (e.g. a bare
    // `$`), so the range check above misses it.  Recurse into the last
    // namespace so variables declared inside it are still visible.
    if let Some(&Statement::Namespace(ns)) = stmts.last() {
        let ns_span = ns.span();
        if cursor_offset > ns_span.end.offset {
            find_scope_and_collect(content, ns.statements().iter(), cursor_offset, vars);
            return;
        }
    }

    // Cursor is in top-level code — collect from all top-level statements.
    collect_from_statements(content, stmts.into_iter(), cursor_offset, vars);
}

/// Recursively search a statement for a closure or arrow function whose body
/// contains the cursor.  If found, collect only the variables from that
/// closure's scope (parameters + `use()` clause + body) and return `true`.
///
/// PHP closures have strict scope isolation: only variables captured via
/// `use($var)`, the closure's own parameters, variables defined in the body,
/// and superglobals are visible inside.  This prevents outer variables from
/// leaking into the closure's completion list.
fn try_collect_from_enclosing_closure<'b>(
    content: &str,
    stmt: &'b Statement<'b>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) -> bool {
    let stmt_span = stmt.span();
    if cursor_offset < stmt_span.start.offset || cursor_offset > stmt_span.end.offset {
        return false;
    }
    // Scan all expressions in the statement for closures/arrow functions.
    match stmt {
        Statement::Expression(expr_stmt) => {
            expr_contains_enclosing_closure(content, expr_stmt.expression, cursor_offset, vars)
        }
        Statement::Return(ret) => {
            if let Some(expr) = ret.value {
                expr_contains_enclosing_closure(content, expr, cursor_offset, vars)
            } else {
                false
            }
        }
        Statement::Echo(echo) => echo
            .values
            .iter()
            .any(|expr| expr_contains_enclosing_closure(content, expr, cursor_offset, vars)),
        Statement::If(if_stmt) => {
            if expr_contains_enclosing_closure(content, if_stmt.condition, cursor_offset, vars) {
                return true;
            }
            match &if_stmt.body {
                IfBody::Statement(body) => {
                    if try_collect_from_enclosing_closure(
                        content,
                        body.statement,
                        cursor_offset,
                        vars,
                    ) {
                        return true;
                    }
                    for else_if in body.else_if_clauses.iter() {
                        if expr_contains_enclosing_closure(
                            content,
                            else_if.condition,
                            cursor_offset,
                            vars,
                        ) {
                            return true;
                        }
                        if try_collect_from_enclosing_closure(
                            content,
                            else_if.statement,
                            cursor_offset,
                            vars,
                        ) {
                            return true;
                        }
                    }
                    if let Some(else_clause) = &body.else_clause
                        && try_collect_from_enclosing_closure(
                            content,
                            else_clause.statement,
                            cursor_offset,
                            vars,
                        )
                    {
                        return true;
                    }
                }
                IfBody::ColonDelimited(body) => {
                    for s in body.statements.iter() {
                        if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                            return true;
                        }
                    }
                    for else_if in body.else_if_clauses.iter() {
                        if expr_contains_enclosing_closure(
                            content,
                            else_if.condition,
                            cursor_offset,
                            vars,
                        ) {
                            return true;
                        }
                        for s in else_if.statements.iter() {
                            if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                                return true;
                            }
                        }
                    }
                    if let Some(else_clause) = &body.else_clause {
                        for s in else_clause.statements.iter() {
                            if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                                return true;
                            }
                        }
                    }
                }
            }
            false
        }
        Statement::Block(block) => block
            .statements
            .iter()
            .any(|s| try_collect_from_enclosing_closure(content, s, cursor_offset, vars)),
        Statement::Foreach(foreach) => {
            for s in foreach.body.statements() {
                if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                    return true;
                }
            }
            false
        }
        Statement::For(for_stmt) => {
            for init in for_stmt.initializations.iter() {
                if expr_contains_enclosing_closure(content, init, cursor_offset, vars) {
                    return true;
                }
            }
            match &for_stmt.body {
                ForBody::Statement(s) => {
                    try_collect_from_enclosing_closure(content, s, cursor_offset, vars)
                }
                ForBody::ColonDelimited(body) => body
                    .statements
                    .iter()
                    .any(|s| try_collect_from_enclosing_closure(content, s, cursor_offset, vars)),
            }
        }
        Statement::While(while_stmt) => match &while_stmt.body {
            WhileBody::Statement(s) => {
                try_collect_from_enclosing_closure(content, s, cursor_offset, vars)
            }
            WhileBody::ColonDelimited(body) => body
                .statements
                .iter()
                .any(|s| try_collect_from_enclosing_closure(content, s, cursor_offset, vars)),
        },
        Statement::DoWhile(dw) => {
            try_collect_from_enclosing_closure(content, dw.statement, cursor_offset, vars)
        }
        Statement::Try(try_stmt) => {
            for s in try_stmt.block.statements.iter() {
                if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                    return true;
                }
            }
            for catch in try_stmt.catch_clauses.iter() {
                for s in catch.block.statements.iter() {
                    if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                        return true;
                    }
                }
            }
            if let Some(finally) = &try_stmt.finally_clause {
                for s in finally.block.statements.iter() {
                    if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                        return true;
                    }
                }
            }
            false
        }
        Statement::Switch(switch) => {
            match &switch.body {
                SwitchBody::BraceDelimited(body) => {
                    for case in body.cases.iter() {
                        for s in case.statements().iter() {
                            if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                                return true;
                            }
                        }
                    }
                }
                SwitchBody::ColonDelimited(body) => {
                    for case in body.cases.iter() {
                        for s in case.statements().iter() {
                            if try_collect_from_enclosing_closure(content, s, cursor_offset, vars) {
                                return true;
                            }
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// Recursively check if an expression contains a closure or arrow function
/// whose body encloses the cursor.  If found, collect from that closure's
/// scope and return `true`.
fn expr_contains_enclosing_closure<'b>(
    content: &str,
    expr: &'b Expression<'b>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) -> bool {
    let expr_span = expr.span();
    if cursor_offset < expr_span.start.offset || cursor_offset > expr_span.end.offset {
        return false;
    }
    match expr {
        Expression::Closure(closure) => {
            let body_start = closure.body.left_brace.start.offset;
            let body_end = closure.body.right_brace.end.offset;
            if cursor_offset >= body_start && cursor_offset <= body_end {
                collect_from_params(&closure.parameter_list, vars);
                if let Some(ref use_clause) = closure.use_clause {
                    for use_var in use_clause.variables.iter() {
                        vars.insert(use_var.variable.name.to_string());
                    }
                }
                collect_from_statements(
                    content,
                    closure.body.statements.iter(),
                    cursor_offset,
                    vars,
                );
                return true;
            }
            false
        }
        Expression::ArrowFunction(arrow) => {
            let span = arrow.span();
            if cursor_offset >= span.start.offset && cursor_offset <= span.end.offset {
                // Arrow functions inherit the outer scope (like JS), but
                // also have their own parameters.  However, for the purpose
                // of variable *completion*, the arrow function body is
                // inside the enclosing scope — we do NOT isolate it.
                // Let the caller handle it.
            }
            false
        }
        Expression::Assignment(assignment) => {
            // Check the RHS — closures commonly appear as `$f = function() { ... }`
            expr_contains_enclosing_closure(content, assignment.rhs, cursor_offset, vars)
        }
        Expression::Call(call) => {
            let arg_list = call.get_argument_list();
            for arg in arg_list.arguments.iter() {
                let arg_expr = match arg {
                    Argument::Positional(a) => a.value,
                    Argument::Named(a) => a.value,
                };
                if expr_contains_enclosing_closure(content, arg_expr, cursor_offset, vars) {
                    return true;
                }
            }
            // Also check the object/class expression for method calls.
            match call {
                Call::Method(mc) => {
                    if expr_contains_enclosing_closure(content, mc.object, cursor_offset, vars) {
                        return true;
                    }
                }
                Call::NullSafeMethod(nmc) => {
                    if expr_contains_enclosing_closure(content, nmc.object, cursor_offset, vars) {
                        return true;
                    }
                }
                Call::StaticMethod(sc) => {
                    if expr_contains_enclosing_closure(content, sc.class, cursor_offset, vars) {
                        return true;
                    }
                }
                _ => {}
            }
            false
        }

        Expression::Parenthesized(p) => {
            expr_contains_enclosing_closure(content, p.expression, cursor_offset, vars)
        }
        Expression::Array(arr) => {
            for elem in arr.elements.iter() {
                match elem {
                    ArrayElement::KeyValue(kv) => {
                        if expr_contains_enclosing_closure(content, kv.value, cursor_offset, vars) {
                            return true;
                        }
                    }
                    ArrayElement::Value(v) => {
                        if expr_contains_enclosing_closure(content, v.value, cursor_offset, vars) {
                            return true;
                        }
                    }
                    ArrayElement::Variadic(v) => {
                        if expr_contains_enclosing_closure(content, v.value, cursor_offset, vars) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        Expression::LegacyArray(arr) => {
            for elem in arr.elements.iter() {
                match elem {
                    ArrayElement::KeyValue(kv) => {
                        if expr_contains_enclosing_closure(content, kv.value, cursor_offset, vars) {
                            return true;
                        }
                    }
                    ArrayElement::Value(v) => {
                        if expr_contains_enclosing_closure(content, v.value, cursor_offset, vars) {
                            return true;
                        }
                    }
                    ArrayElement::Variadic(v) => {
                        if expr_contains_enclosing_closure(content, v.value, cursor_offset, vars) {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            false
        }
        Expression::Conditional(cond) => {
            if expr_contains_enclosing_closure(content, cond.condition, cursor_offset, vars) {
                return true;
            }
            if let Some(then_expr) = cond.then
                && expr_contains_enclosing_closure(content, then_expr, cursor_offset, vars)
            {
                return true;
            }
            expr_contains_enclosing_closure(content, cond.r#else, cursor_offset, vars)
        }
        Expression::Binary(binary) => {
            if expr_contains_enclosing_closure(content, binary.lhs, cursor_offset, vars) {
                return true;
            }
            expr_contains_enclosing_closure(content, binary.rhs, cursor_offset, vars)
        }
        _ => false,
    }
}

/// Scan class-like members to find the method containing the cursor
/// and collect variables from that method's scope.
fn collect_from_class_members<'b>(
    content: &str,
    members: impl Iterator<Item = &'b ClassLikeMember<'b>>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    for member in members {
        if let ClassLikeMember::Method(method) = member
            && let MethodBody::Concrete(block) = &method.body
        {
            let blk_start = block.left_brace.start.offset;
            let blk_end = block.right_brace.end.offset;
            if cursor_offset >= blk_start && cursor_offset <= blk_end {
                // Add $this only if the method is NOT static
                let is_static = method
                    .modifiers
                    .iter()
                    .any(|m| matches!(m, Modifier::Static(_)));
                if !is_static {
                    vars.insert("$this".to_string());
                }
                // Collect parameters (skip promoted properties —
                // they act as both params and properties, but as
                // variables they are still accessible in the body)
                collect_from_params(&method.parameter_list, vars);
                // Collect from body
                collect_from_statements(content, block.statements.iter(), cursor_offset, vars);
                return;
            }
        }
    }
    // Cursor is inside the class body but not inside any method body
    // (e.g. in a property declaration) — no variables are in scope.
}

/// Collect parameter names from a function/method/closure parameter list.
fn collect_from_params(params: &FunctionLikeParameterList, vars: &mut HashSet<String>) {
    for param in params.parameters.iter() {
        let name = param.variable.name.to_string();
        vars.insert(name);
    }
}

/// Walk statements within a scope collecting variable names.
///
/// This handles assignments, foreach, for, try/catch, closures,
/// global, static, and all control-flow structures.
///
/// Only variables defined **before** the cursor position are collected.
/// This prevents suggesting variables that haven't been defined yet
/// (e.g. a variable assigned on line 535 shouldn't appear when typing
/// on line 15).
fn collect_from_statements<'b>(
    content: &str,
    statements: impl Iterator<Item = &'b Statement<'b>>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    for stmt in statements {
        // Skip statements that start after the cursor — variables
        // defined there haven't been introduced yet.
        let stmt_span = stmt.span();
        if stmt_span.start.offset > cursor_offset {
            continue;
        }

        // ── @var docblock variable declarations ──
        // A `/** @var Type $varName */` right before a statement declares
        // the variable for type-narrowing purposes.  Include the named
        // variable in suggestions so that typing `$` after such a
        // docblock offers the annotated name.
        if let Some((_type_str, Some(var_name))) =
            crate::docblock::find_inline_var_docblock(content, stmt_span.start.offset as usize)
        {
            vars.insert(var_name);
        }

        match stmt {
            Statement::Expression(expr_stmt) => {
                collect_vars_from_expression(content, expr_stmt.expression, cursor_offset, vars);
            }
            Statement::Block(block) => {
                collect_from_statements(content, block.statements.iter(), cursor_offset, vars);
            }
            Statement::If(if_stmt) => match &if_stmt.body {
                IfBody::Statement(body) => {
                    // Collect from the condition (assignments in conditions)
                    collect_vars_from_expression(content, if_stmt.condition, cursor_offset, vars);
                    collect_from_statement(content, body.statement, cursor_offset, vars);
                    for else_if in body.else_if_clauses.iter() {
                        collect_vars_from_expression(
                            content,
                            else_if.condition,
                            cursor_offset,
                            vars,
                        );
                        collect_from_statement(content, else_if.statement, cursor_offset, vars);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        collect_from_statement(content, else_clause.statement, cursor_offset, vars);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    collect_vars_from_expression(content, if_stmt.condition, cursor_offset, vars);
                    collect_from_statements(content, body.statements.iter(), cursor_offset, vars);
                    for else_if in body.else_if_clauses.iter() {
                        collect_vars_from_expression(
                            content,
                            else_if.condition,
                            cursor_offset,
                            vars,
                        );
                        collect_from_statements(
                            content,
                            else_if.statements.iter(),
                            cursor_offset,
                            vars,
                        );
                    }
                    if let Some(else_clause) = &body.else_clause {
                        collect_from_statements(
                            content,
                            else_clause.statements.iter(),
                            cursor_offset,
                            vars,
                        );
                    }
                }
            },
            Statement::Foreach(foreach) => {
                // Only collect the key/value variables when the cursor is
                // inside the foreach body.  Outside the loop these
                // iteration variables should not be in scope.
                let body_span = foreach.body.span();
                if cursor_offset >= body_span.start.offset && cursor_offset <= body_span.end.offset
                {
                    if let Some(key_expr) = foreach.target.key() {
                        collect_var_name_from_expression(key_expr, vars);
                    }
                    collect_var_name_from_expression(foreach.target.value(), vars);
                    // Recurse into body
                    for inner in foreach.body.statements() {
                        collect_from_statement(content, inner, cursor_offset, vars);
                    }
                }
            }
            Statement::For(for_stmt) => {
                // Collect variables from initializations (e.g. `$i = 0`)
                for init_expr in for_stmt.initializations.iter() {
                    collect_vars_from_expression(content, init_expr, cursor_offset, vars);
                }
                match &for_stmt.body {
                    ForBody::Statement(inner) => {
                        collect_from_statement(content, inner, cursor_offset, vars);
                    }
                    ForBody::ColonDelimited(body) => {
                        collect_from_statements(
                            content,
                            body.statements.iter(),
                            cursor_offset,
                            vars,
                        );
                    }
                }
            }
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(inner) => {
                    collect_from_statement(content, inner, cursor_offset, vars);
                }
                WhileBody::ColonDelimited(body) => {
                    collect_from_statements(content, body.statements.iter(), cursor_offset, vars);
                }
            },
            Statement::DoWhile(dw) => {
                collect_from_statement(content, dw.statement, cursor_offset, vars);
            }
            Statement::Try(try_stmt) => {
                collect_from_statements(
                    content,
                    try_stmt.block.statements.iter(),
                    cursor_offset,
                    vars,
                );
                for catch in try_stmt.catch_clauses.iter() {
                    // Only collect the catch variable if its clause starts
                    // before the cursor (i.e. the cursor is inside or after
                    // the catch block).
                    let catch_span = catch.span();
                    if catch_span.start.offset > cursor_offset {
                        continue;
                    }
                    if let Some(ref var) = catch.variable {
                        vars.insert(var.name.to_string());
                    }
                    collect_from_statements(
                        content,
                        catch.block.statements.iter(),
                        cursor_offset,
                        vars,
                    );
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    let finally_span = finally.span();
                    if finally_span.start.offset <= cursor_offset {
                        collect_from_statements(
                            content,
                            finally.block.statements.iter(),
                            cursor_offset,
                            vars,
                        );
                    }
                }
            }
            Statement::Global(global) => {
                // The span check above already ensures this statement is
                // before the cursor.
                for var in global.variables.iter() {
                    if let Variable::Direct(dv) = var {
                        vars.insert(dv.name.to_string());
                    }
                }
            }
            Statement::Static(static_stmt) => {
                for item in static_stmt.items.iter() {
                    vars.insert(item.variable().name.to_string());
                }
            }
            Statement::Return(ret) => {
                if let Some(expr) = ret.value {
                    collect_vars_from_expression(content, expr, cursor_offset, vars);
                }
            }
            Statement::Echo(echo) => {
                for expr in echo.values.iter() {
                    collect_vars_from_expression(content, expr, cursor_offset, vars);
                }
            }
            Statement::Switch(switch) => {
                collect_vars_from_expression(content, switch.expression, cursor_offset, vars);
                match &switch.body {
                    SwitchBody::BraceDelimited(body) => {
                        for case in body.cases.iter() {
                            collect_from_statements(
                                content,
                                case.statements().iter(),
                                cursor_offset,
                                vars,
                            );
                        }
                    }
                    SwitchBody::ColonDelimited(body) => {
                        for case in body.cases.iter() {
                            collect_from_statements(
                                content,
                                case.statements().iter(),
                                cursor_offset,
                                vars,
                            );
                        }
                    }
                }
            }
            // ── unset($var) ──
            // When `unset($var)` appears before the cursor, the variable
            // is no longer in scope and should not be suggested.
            Statement::Unset(unset_stmt) => {
                for val in unset_stmt.values.iter() {
                    if let Expression::Variable(Variable::Direct(dv)) = val {
                        vars.remove(&dv.name.to_string());
                    }
                }
            }
            // Skip class/function/namespace declarations (they have their
            // own scopes handled by find_scope_and_collect).
            Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Trait(_)
            | Statement::Enum(_)
            | Statement::Function(_)
            | Statement::Namespace(_) => {}
            _ => {}
        }
    }
}

/// Helper: dispatch a single statement to `collect_from_statements`.
fn collect_from_statement<'b>(
    content: &str,
    stmt: &'b Statement<'b>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    collect_from_statements(content, std::iter::once(stmt), cursor_offset, vars);
}

/// Extract variable names from an expression.
///
/// Handles assignments, closures (which introduce a new scope),
/// and arrow functions.
fn collect_vars_from_expression<'b>(
    content: &str,
    expr: &'b Expression<'b>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    match expr {
        Expression::Assignment(assignment) => {
            // Collect the LHS variable name
            collect_var_name_from_expression(assignment.lhs, vars);
            // Also scan the RHS for nested assignments
            collect_vars_from_expression(content, assignment.rhs, cursor_offset, vars);
        }
        // If the cursor is inside a closure body, collect from that
        // closure's scope instead (closures have their own variable scope).
        Expression::Closure(closure) => {
            let body_start = closure.body.left_brace.start.offset;
            let body_end = closure.body.right_brace.end.offset;
            if cursor_offset >= body_start && cursor_offset <= body_end {
                // Closure introduces a new scope: parameters + use clause
                collect_from_params(&closure.parameter_list, vars);
                if let Some(ref use_clause) = closure.use_clause {
                    for use_var in use_clause.variables.iter() {
                        vars.insert(use_var.variable.name.to_string());
                    }
                }
                collect_from_statements(
                    content,
                    closure.body.statements.iter(),
                    cursor_offset,
                    vars,
                );
            }
            // If cursor is outside this closure, don't collect its internals.
        }
        Expression::ArrowFunction(arrow) => {
            let span = arrow.span();
            if cursor_offset >= span.start.offset && cursor_offset <= span.end.offset {
                collect_from_params(&arrow.parameter_list, vars);
                collect_vars_from_expression(content, arrow.expression, cursor_offset, vars);
            }
        }
        // Don't recurse into sub-expressions that aren't scoping constructs
        // — we only care about assignment LHS variables, not every variable
        // reference (those are handled by the statement walker).
        _ => {}
    }
}

/// Extract a direct variable name from an expression (for assignment LHS,
/// foreach targets, etc.).  Only extracts `$name` from direct variables;
/// ignores property accesses, array accesses, etc.
fn collect_var_name_from_expression(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => {
            vars.insert(dv.name.to_string());
        }
        // `list($a, $b) = ...` or `[$a, $b] = ...`
        Expression::List(list) => {
            for element in list.elements.iter() {
                if let ArrayElement::KeyValue(kv) = element {
                    collect_var_name_from_expression(kv.value, vars);
                } else if let ArrayElement::Value(val) = element {
                    collect_var_name_from_expression(val.value, vars);
                }
            }
        }
        Expression::Array(arr) => {
            for element in arr.elements.iter() {
                if let ArrayElement::KeyValue(kv) = element {
                    collect_var_name_from_expression(kv.value, vars);
                } else if let ArrayElement::Value(val) = element {
                    collect_var_name_from_expression(val.value, vars);
                }
            }
        }
        _ => {}
    }
}
