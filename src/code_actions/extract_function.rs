//! **Extract Function / Method** code action (`refactor.extract`).
//!
//! When the user selects one or more complete statements inside a
//! function or method body, this action extracts them into a new
//! function (or method, if `$this`/`self::`/`static::` is used).
//!
//! The implementation uses the `ScopeCollector` infrastructure (A11) to
//! classify variables as parameters, return values, or locals relative
//! to the selected range.  Type annotations are inferred via the hover
//! variable-type resolution pipeline.

use bumpalo::Bump;
use mago_span::HasSpan;
use mago_syntax::ast::*;
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::code_actions::cursor_context::{CursorContext, MemberContext, find_cursor_context};
use crate::completion::phpdoc::generation::enrichment_plain;
use crate::completion::resolver::Loaders;
use crate::scope_collector::{
    FrameKind, ScopeMap, collect_function_scope, collect_function_scope_with_kind, collect_scope,
};
use crate::types::ClassInfo;
use crate::util::{find_class_at_offset, offset_to_position, position_to_byte_offset};

// ─── Statement boundary validation ─────────────────────────────────────────

/// Check whether the selected byte range `[start, end)` covers one or
/// more complete statements.
///
/// We parse the file and walk the AST to verify that every statement
/// whose span overlaps the selection is *fully* contained within it.
/// If any statement is only partially selected, the selection is
/// invalid for extraction.
fn selection_covers_complete_statements(content: &str, start: usize, end: usize) -> bool {
    let arena = Bump::new();
    let file_id = mago_database::file::FileId::new("extract_fn_validate");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content);

    // Find the enclosing function/method body statements.
    let body_stmts = find_enclosing_body_statements(&program.statements, start as u32);
    if body_stmts.is_empty() {
        return false;
    }

    let mut found_any = false;
    for stmt in &body_stmts {
        let span = stmt.span();
        let stmt_start = span.start.offset as usize;
        let stmt_end = span.end.offset as usize;

        // Statement fully outside the selection — fine, skip it.
        if stmt_end <= start || stmt_start >= end {
            continue;
        }

        // Statement overlaps the selection — it must be fully contained.
        if stmt_start < start || stmt_end > end {
            return false;
        }

        found_any = true;
    }

    found_any
}

/// Collect references to top-level statements in the enclosing
/// function/method body that contains `offset`.
///
/// Returns byte ranges `(start, end)` for each direct child statement.
fn find_enclosing_body_statements<'a>(
    statements: &'a Sequence<'a, Statement<'a>>,
    offset: u32,
) -> Vec<&'a Statement<'a>> {
    for stmt in statements.iter() {
        match stmt {
            Statement::Function(func) => {
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if offset >= body_start && offset <= body_end {
                    return func.body.statements.iter().collect();
                }
            }
            Statement::Class(class) => {
                for member in class.members.iter() {
                    if let ClassLikeMember::Method(method) = member
                        && let MethodBody::Concrete(block) = &method.body
                    {
                        let body_start = block.left_brace.start.offset;
                        let body_end = block.right_brace.end.offset;
                        if offset >= body_start && offset <= body_end {
                            return block.statements.iter().collect();
                        }
                    }
                }
            }
            Statement::Trait(tr) => {
                for member in tr.members.iter() {
                    if let ClassLikeMember::Method(method) = member
                        && let MethodBody::Concrete(block) = &method.body
                    {
                        let body_start = block.left_brace.start.offset;
                        let body_end = block.right_brace.end.offset;
                        if offset >= body_start && offset <= body_end {
                            return block.statements.iter().collect();
                        }
                    }
                }
            }
            Statement::Enum(en) => {
                for member in en.members.iter() {
                    if let ClassLikeMember::Method(method) = member
                        && let MethodBody::Concrete(block) = &method.body
                    {
                        let body_start = block.left_brace.start.offset;
                        let body_end = block.right_brace.end.offset;
                        if offset >= body_start && offset <= body_end {
                            return block.statements.iter().collect();
                        }
                    }
                }
            }
            Statement::Namespace(ns) => {
                let result = find_enclosing_body_statements(ns.statements(), offset);
                if !result.is_empty() {
                    return result;
                }
            }
            _ => {}
        }
    }
    Vec::new()
}

// ─── Context detection ──────────────────────────────────────────────────────

/// Whether the extracted code should become a method or a standalone function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExtractionTarget {
    /// Extract as a private method on the enclosing class.
    Method,
    /// Extract as a standalone function after the enclosing function.
    Function,
}

/// Information about the enclosing function/method for insertion purposes.
#[derive(Debug, Clone)]
struct EnclosingContext {
    /// Whether to extract as a method or function.
    target: ExtractionTarget,
    /// Byte offset of the closing `}` of the enclosing class (for method
    /// insertion) or the enclosing function (for function insertion).
    insert_offset: usize,
    /// The body's opening `{` offset — used to determine indentation.
    body_start: usize,
    /// Whether the enclosing method is static.
    is_static: bool,
    /// The name of the enclosing function/method (e.g. `"run"`, `"process"`).
    /// Used by name generation to produce contextual names like `runGuard`.
    enclosing_name: String,
    /// Method names that already exist in the enclosing class (for
    /// deduplication when extracting a method).  Empty when extracting
    /// a standalone function.
    sibling_method_names: Vec<String>,
}

/// Determine the extraction target and insertion point by walking the AST.
fn find_enclosing_context(content: &str, offset: u32, uses_this: bool) -> Option<EnclosingContext> {
    let arena = Bump::new();
    let file_id = mago_database::file::FileId::new("extract_fn_ctx");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content);

    let ctx = find_cursor_context(&program.statements, offset);

    match ctx {
        CursorContext::InClassLike {
            member,
            all_members,
        } => {
            if let MemberContext::Method(method, true) = member {
                let is_static = method.modifiers.iter().any(|m| m.is_static());
                let enclosing_name = method.name.value.to_string();

                // Collect sibling method names for scoped deduplication.
                let sibling_method_names: Vec<String> = all_members
                    .iter()
                    .filter_map(|m| {
                        if let ClassLikeMember::Method(m) = m {
                            Some(m.name.value.to_string())
                        } else {
                            None
                        }
                    })
                    .collect();

                // For method extraction, insert before the closing `}` of the class.
                // Find the class closing brace by walking up from the method.
                let class_end = find_class_end_offset(&program.statements, offset);

                if let MethodBody::Concrete(block) = &method.body {
                    let body_start = block.left_brace.start.offset as usize;

                    if uses_this && is_static {
                        // $this in a static method — can't extract as method.
                        // Fall back to extracting as a function.
                        let func_end = block.right_brace.end.offset as usize;
                        return Some(EnclosingContext {
                            target: ExtractionTarget::Function,
                            insert_offset: find_after_class_end(&program.statements, offset)
                                .unwrap_or(func_end),
                            body_start,
                            is_static,
                            enclosing_name,
                            sibling_method_names: Vec::new(),
                        });
                    }

                    return Some(EnclosingContext {
                        target: ExtractionTarget::Method,
                        insert_offset: class_end.unwrap_or(block.right_brace.end.offset as usize),
                        body_start,
                        is_static,
                        enclosing_name,
                        sibling_method_names,
                    });
                }
            }
            None
        }
        CursorContext::InFunction(func, true) => {
            let body_start = func.body.left_brace.start.offset as usize;
            let func_end = func.body.right_brace.end.offset as usize;
            let enclosing_name = func.name.value.to_string();

            // For function extraction, insert after the enclosing function.
            // Find the end of the line containing the closing `}`.
            let insert_offset = find_line_end(content, func_end);

            Some(EnclosingContext {
                target: ExtractionTarget::Function,
                insert_offset,
                body_start,
                is_static: false,
                enclosing_name,
                sibling_method_names: Vec::new(),
            })
        }
        _ => None,
    }
}

/// Find the byte offset of the closing `}` of the class containing `offset`.
fn find_class_end_offset(statements: &Sequence<'_, Statement<'_>>, offset: u32) -> Option<usize> {
    for stmt in statements.iter() {
        match stmt {
            Statement::Class(class) => {
                let span = class.span();
                if offset >= span.start.offset && offset <= span.end.offset {
                    return Some(class.right_brace.start.offset as usize);
                }
            }
            Statement::Trait(tr) => {
                let span = tr.span();
                if offset >= span.start.offset && offset <= span.end.offset {
                    return Some(tr.right_brace.start.offset as usize);
                }
            }
            Statement::Enum(en) => {
                let span = en.span();
                if offset >= span.start.offset && offset <= span.end.offset {
                    return Some(en.right_brace.start.offset as usize);
                }
            }
            Statement::Namespace(ns) => {
                if let Some(offset) = find_class_end_offset(ns.statements(), offset) {
                    return Some(offset);
                }
            }
            _ => {}
        }
    }
    None
}

/// Find the byte offset after the closing `}` of the class containing `offset`.
fn find_after_class_end(statements: &Sequence<'_, Statement<'_>>, offset: u32) -> Option<usize> {
    for stmt in statements.iter() {
        match stmt {
            Statement::Class(class) => {
                let span = class.span();
                if offset >= span.start.offset && offset <= span.end.offset {
                    return Some(span.end.offset as usize);
                }
            }
            Statement::Trait(tr) => {
                let span = tr.span();
                if offset >= span.start.offset && offset <= span.end.offset {
                    return Some(span.end.offset as usize);
                }
            }
            Statement::Enum(en) => {
                let span = en.span();
                if offset >= span.start.offset && offset <= span.end.offset {
                    return Some(span.end.offset as usize);
                }
            }
            Statement::Namespace(ns) => {
                if let Some(end) = find_after_class_end(ns.statements(), offset) {
                    return Some(end);
                }
            }
            _ => {}
        }
    }
    None
}

// ─── Scope map building ─────────────────────────────────────────────────────

/// Build a `ScopeMap` for the enclosing function/method at `offset`.
fn build_scope_map(content: &str, offset: u32) -> ScopeMap {
    let arena = Bump::new();
    let file_id = mago_database::file::FileId::new("extract_fn_scope");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content);

    for stmt in program.statements.iter() {
        if let Some(map) = try_build_scope_from_statement(stmt, offset) {
            return map;
        }
    }

    // Fallback: top-level scope.
    let body_end = content.len() as u32;
    collect_scope(program.statements.as_slice(), 0, body_end)
}

/// Recursively try to build a scope map from a statement.
fn try_build_scope_from_statement(stmt: &Statement<'_>, offset: u32) -> Option<ScopeMap> {
    match stmt {
        Statement::Function(func) => {
            let body_start = func.body.left_brace.start.offset;
            let body_end = func.body.right_brace.end.offset;
            if offset >= body_start && offset <= body_end {
                return Some(collect_function_scope(
                    &func.parameter_list,
                    func.body.statements.as_slice(),
                    body_start,
                    body_end,
                ));
            }
        }
        Statement::Class(class) => {
            for member in class.members.iter() {
                if let ClassLikeMember::Method(method) = member
                    && let MethodBody::Concrete(block) = &method.body
                {
                    let body_start = block.left_brace.start.offset;
                    let body_end = block.right_brace.end.offset;
                    if offset >= body_start && offset <= body_end {
                        return Some(collect_function_scope_with_kind(
                            &method.parameter_list,
                            block.statements.as_slice(),
                            body_start,
                            body_end,
                            FrameKind::Method,
                        ));
                    }
                }
            }
        }
        Statement::Trait(tr) => {
            for member in tr.members.iter() {
                if let ClassLikeMember::Method(method) = member
                    && let MethodBody::Concrete(block) = &method.body
                {
                    let body_start = block.left_brace.start.offset;
                    let body_end = block.right_brace.end.offset;
                    if offset >= body_start && offset <= body_end {
                        return Some(collect_function_scope_with_kind(
                            &method.parameter_list,
                            block.statements.as_slice(),
                            body_start,
                            body_end,
                            FrameKind::Method,
                        ));
                    }
                }
            }
        }
        Statement::Enum(en) => {
            for member in en.members.iter() {
                if let ClassLikeMember::Method(method) = member
                    && let MethodBody::Concrete(block) = &method.body
                {
                    let body_start = block.left_brace.start.offset;
                    let body_end = block.right_brace.end.offset;
                    if offset >= body_start && offset <= body_end {
                        return Some(collect_function_scope_with_kind(
                            &method.parameter_list,
                            block.statements.as_slice(),
                            body_start,
                            body_end,
                            FrameKind::Method,
                        ));
                    }
                }
            }
        }
        Statement::Namespace(ns) => {
            for inner in ns.statements().iter() {
                if let Some(map) = try_build_scope_from_statement(inner, offset) {
                    return Some(map);
                }
            }
        }
        _ => {}
    }
    None
}

// ─── Type resolution ────────────────────────────────────────────────────────

/// Resolve the type of a variable at a given offset using the hover
/// pipeline.
fn resolve_var_type(
    backend: &Backend,
    var_name: &str,
    content: &str,
    cursor_offset: u32,
    uri: &str,
) -> Option<String> {
    let ctx = backend.file_context(uri);
    let class_loader = backend.class_loader(&ctx);
    let function_loader = backend.function_loader(&ctx);
    let constant_loader = backend.constant_loader();
    let loaders = Loaders {
        function_loader: Some(
            &function_loader as &dyn Fn(&str) -> Option<crate::types::FunctionInfo>,
        ),
        constant_loader: Some(&constant_loader),
    };

    let current_class = find_class_at_offset(&ctx.classes, cursor_offset);

    crate::hover::variable_type::resolve_variable_type_string(
        var_name,
        content,
        cursor_offset,
        current_class,
        &ctx.classes,
        &class_loader,
        loaders,
    )
}

// ─── Name generation ────────────────────────────────────────────────────────

/// Generate a unique function/method name that doesn't conflict with
/// existing members or functions.
/// Context passed to [`generate_function_name`] to produce meaningful names.
struct NamingContext<'a> {
    /// The enclosing function/method name (e.g. `"run"`, `"process"`).
    enclosing_name: &'a str,
    /// The return strategy chosen for the extraction.
    return_strategy: &'a ReturnStrategy,
    /// The selected body text (trimmed source of the extracted statements).
    body_text: &'a str,
    /// Names of return-value variables (written inside, read after).
    return_var_names: &'a [String],
    /// The trailing return type hint (e.g. `"Collection"`, `"User"`).
    trailing_return_type: &'a str,
}

/// Generate a contextual name for the extracted function/method.
///
/// The naming follows these heuristics (first match wins):
///
/// 1. **Guard strategies** (`VoidGuards`, `UniformGuards`,
///    `NullGuardWithValue`): `{enclosing}Guard` — the user extracted
///    validation / precondition logic.
/// 2. **`SentinelNull`**: `try{Enclosing}` — a "try" pattern where
///    `null` signals failure.
/// 3. **`TrailingReturn` with `new ClassName`** in the body:
///    `create{ClassName}` — a factory pattern.
/// 4. **`TrailingReturn`** (other): `get{Enclosing}Result`.
/// 5. **Body is pure output** (every statement is `echo`/`print`/
///    `printf`/`var_dump`): `render{Enclosing}`.
/// 6. **Single return variable**: `compute{VarName}` — the user
///    extracted a calculation into its own function.
/// 7. **Fallback**: `"extracted"`.
///
/// After choosing a base name, the function deduplicates against
/// existing names in the appropriate scope (class members for methods,
/// file-level `function` declarations for standalone functions).
fn generate_function_name(content: &str, enclosing_ctx: &EnclosingContext, naming: &NamingContext) -> String {
    let base = derive_base_name(naming);

    // Deduplicate against the right scope.
    deduplicate_name(&base, content, enclosing_ctx)
}

/// Pick a base name from the naming context (before deduplication).
fn derive_base_name(ctx: &NamingContext) -> String {
    let enc = ctx.enclosing_name;

    // 1. Guard strategies → {enclosing}Guard
    match ctx.return_strategy {
        ReturnStrategy::VoidGuards
        | ReturnStrategy::UniformGuards(_)
        | ReturnStrategy::NullGuardWithValue(_) => {
            if !enc.is_empty() {
                return format!("{}Guard", enc);
            }
            return "guard".to_string();
        }

        // 2. SentinelNull → try{Enclosing}
        ReturnStrategy::SentinelNull => {
            if !enc.is_empty() {
                return format!("try{}", capitalise(enc));
            }
            return "tryExtract".to_string();
        }

        // 3–4. TrailingReturn
        ReturnStrategy::TrailingReturn => {
            // 3. Factory: body contains `new ClassName` → create{ClassName}
            if let Some(class_name) = detect_factory_pattern(ctx.body_text) {
                return format!("create{}", class_name);
            }
            // 4. Generic trailing return
            if !enc.is_empty() {
                // If there's a return type, use it for a more descriptive name
                if !ctx.trailing_return_type.is_empty() {
                    let clean = strip_generics_for_hint(ctx.trailing_return_type);
                    // Only use the return type if it's a class name (starts uppercase)
                    if clean.starts_with(|c: char| c.is_ascii_uppercase()) {
                        return format!("get{}", clean);
                    }
                }
                return format!("get{}Result", capitalise(enc));
            }
        }

        ReturnStrategy::None | ReturnStrategy::Unsafe => {}
    }

    // 5. Pure output → render{Enclosing}
    if is_pure_output(ctx.body_text) && !enc.is_empty() {
        return format!("render{}", capitalise(enc));
    }

    // 6. Single return variable → compute{VarName}
    if ctx.return_var_names.len() == 1 {
        let var = ctx.return_var_names[0].trim_start_matches('$');
        if !var.is_empty() {
            return format!("compute{}", capitalise(var));
        }
    }

    // 7. Fallback
    "extracted".to_string()
}

/// Capitalise the first character of a string (ASCII).
fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => {
            let upper: String = c.to_uppercase().collect();
            format!("{}{}", upper, chars.as_str())
        }
        None => String::new(),
    }
}

/// Detect if the body text is a factory pattern: it contains `new ClassName`
/// and the last non-empty statement is a `return`.
///
/// Returns the class name if a factory pattern is detected.
fn detect_factory_pattern(body: &str) -> Option<String> {
    // Look for `new ClassName` patterns.
    let mut class_name = None;
    for (i, _) in body.match_indices("new ") {
        let after = &body[i + 4..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '\\')
            .collect();
        if !name.is_empty() && name.starts_with(|c: char| c.is_ascii_uppercase()) {
            class_name = Some(name);
        }
    }

    // Only use the factory name if the body actually returns something
    // (the TrailingReturn strategy already guarantees this, but let's
    // verify the `new` is the thing being constructed and returned).
    if let Some(name) = class_name {
        // Strip namespace prefix, keep only the short class name.
        let short = name.rsplit('\\').next().unwrap_or(&name);
        return Some(short.to_string());
    }

    None
}

/// Check whether every statement in the body is a pure output statement
/// (`echo`, `print`, `printf`, `var_dump`, `print_r`, `var_export`).
fn is_pure_output(body: &str) -> bool {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return false;
    }

    let output_prefixes = [
        "echo ", "echo(", "print ", "print(",
        "printf(", "var_dump(", "print_r(", "var_export(",
    ];

    for line in trimmed.lines() {
        let line = line.trim().trim_end_matches(';').trim();
        if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
            continue;
        }
        let is_output = output_prefixes.iter().any(|p| line.starts_with(p));
        // Also match string interpolation in echo: echo "..."
        let is_echo_string = line.starts_with("echo \"") || line.starts_with("echo '");
        if !is_output && !is_echo_string {
            return false;
        }
    }

    true
}

/// Deduplicate a base name against existing names in the appropriate scope.
///
/// For methods, checks against sibling method names in the class.
/// For functions, checks against `function <name>` patterns in the file.
fn deduplicate_name(base: &str, content: &str, ctx: &EnclosingContext) -> String {
    let mut name = base.to_string();
    let mut counter = 1u32;

    match ctx.target {
        ExtractionTarget::Method => {
            // Check against sibling method names in the class.
            loop {
                if !ctx.sibling_method_names.contains(&name) {
                    break;
                }
                counter += 1;
                name = format!("{}{}", base, counter);
            }
        }
        ExtractionTarget::Function => {
            // Check against function declarations in the file.
            loop {
                let pattern_fn = format!("function {}", name);
                if !content.contains(&pattern_fn) {
                    break;
                }
                counter += 1;
                name = format!("{}{}", base, counter);
            }
        }
    }

    name
}

// ─── Selection trimming ────────────────────────────────────────────────────

/// Trim the selection to exclude leading/trailing whitespace and ensure
/// it starts/ends on statement boundaries.
///
/// Returns `(trimmed_start, trimmed_end)` or `None` if the trimmed
/// selection is empty.
fn trim_selection(content: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    if start >= end || end > content.len() {
        return None;
    }

    let selected = &content[start..end];
    let trimmed = selected.trim();
    if trimmed.is_empty() {
        return None;
    }

    let trim_start = start + (selected.len() - selected.trim_start().len());
    let trim_end = end - (selected.len() - selected.trim_end().len());

    if trim_start >= trim_end {
        return None;
    }

    Some((trim_start, trim_end))
}

// ─── Indentation helpers ────────────────────────────────────────────────────

/// Detect the indentation of the line containing the given offset.
///
/// Returns only the leading whitespace of that line, without adding
/// an extra indent level.
fn detect_line_indent(content: &str, offset: usize) -> String {
    let before = &content[..offset];
    let line_start = before.rfind('\n').map_or(0, |p| p + 1);
    let line = &content[line_start..offset];
    line.chars().take_while(|c| c.is_whitespace()).collect()
}

/// Detect whether the file uses tabs or spaces (and how many spaces).
fn detect_indent_unit(content: &str) -> &str {
    for line in content.lines() {
        if line.starts_with('\t') {
            return "\t";
        }
        let spaces: usize = line.chars().take_while(|c| *c == ' ').count();
        if spaces >= 2 {
            if spaces.is_multiple_of(4) {
                return "    ";
            }
            return "  ";
        }
    }
    "    "
}

/// Find the end of the line containing `offset` (after the `\n`).
fn find_line_end(content: &str, offset: usize) -> usize {
    match content[offset..].find('\n') {
        Some(pos) => offset + pos + 1,
        None => content.len(),
    }
}

/// Find the start of the line containing `offset`.
fn find_line_start(content: &str, offset: usize) -> usize {
    content[..offset].rfind('\n').map_or(0, |p| p + 1)
}

/// Extract the indentation (leading whitespace) of the line at `offset`.
fn indent_at(content: &str, offset: usize) -> String {
    let line_start = find_line_start(content, offset);
    let rest = &content[line_start..];
    rest.chars().take_while(|c| c.is_whitespace()).collect()
}

// ─── Code generation ────────────────────────────────────────────────────────

/// Information gathered for code generation.
struct ExtractionInfo {
    /// The name of the new function/method.
    name: String,
    /// Parameters: `(var_name_with_dollar, type_hint_or_empty)`.
    params: Vec<(String, String)>,
    /// Return values: `(var_name_with_dollar, type_hint_or_empty)`.
    returns: Vec<(String, String)>,
    /// The selected statements as source text.
    body: String,
    /// Whether to extract as method or function.
    target: ExtractionTarget,
    /// Whether the enclosing method is static.
    is_static: bool,
    /// Indentation of the member level (for methods) or top level (for functions).
    member_indent: String,
    /// Indentation of the body inside the new function/method.
    body_indent: String,
    /// How return statements in the selection are handled.
    return_strategy: ReturnStrategy,
    /// Return type hint for the trailing return (resolved from the
    /// enclosing function's return type or the return expression).
    trailing_return_type: String,
    /// Pre-computed PHPDoc block (including `/**` … `*/\n`) to prepend
    /// before the function definition, or empty if no enrichment needed.
    docblock: String,
}

/// Build a PHPDoc block for the extracted function when types need enrichment.
///
/// Each parameter is a triple `(var_name, cleaned_type, raw_type)` where
/// `cleaned_type` is the native PHP hint (generics stripped) and
/// `raw_type` is the full resolved type string (e.g. `Collection<User>`).
///
/// When `raw_type` already contains concrete generic arguments (`<`),
/// it is used verbatim as the docblock type.  Otherwise we fall back to
/// `enrichment_plain` which reconstructs template parameters from the
/// class definition (yielding placeholder names like `T`).
///
/// A `@return` tag follows the same logic: if `raw_return_type` carries
/// concrete generics, use it; otherwise try enrichment.
///
/// Returns an empty string when no enrichment is needed.
fn build_docblock_for_extraction(
    params: &[(String, String, String)],
    return_type_hint: &str,
    raw_return_type: &str,
    member_indent: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let mut tags: Vec<String> = Vec::new();

    // Collect @param tags that need enrichment.
    for (name, type_hint, raw) in params {
        if type_hint.is_empty() && raw.is_empty() {
            continue;
        }
        // Prefer the raw resolved type when it carries concrete generics.
        if raw.contains('<') {
            tags.push(format!("@param {} {}", raw, name));
            continue;
        }
        let hint = if type_hint.is_empty() { raw } else { type_hint };
        let opt = Some(hint.clone());
        if let Some(enriched) = enrichment_plain(&opt, class_loader) {
            tags.push(format!("@param {} {}", enriched, name));
        }
    }

    // Collect @return tag if the return type needs enrichment.
    if !return_type_hint.is_empty() || !raw_return_type.is_empty() {
        if raw_return_type.contains('<') {
            tags.push(format!("@return {}", raw_return_type));
        } else {
            let hint = if return_type_hint.is_empty() {
                raw_return_type
            } else {
                return_type_hint
            };
            let opt = Some(hint.to_string());
            if let Some(enriched) = enrichment_plain(&opt, class_loader) {
                tags.push(format!("@return {}", enriched));
            }
        }
    }

    if tags.is_empty() {
        return String::new();
    }

    // Align @param tag types for readability.
    // Find the max type width among @param tags.
    let param_tags: Vec<(&str, &str)> = tags
        .iter()
        .filter_map(|t| {
            let rest = t.strip_prefix("@param ")?;
            // Split on `$` — PHP param names always start with `$`,
            // and the type string may contain spaces (e.g. `(Closure(): mixed)`).
            let dollar_pos = rest.find('$')?;
            let type_str = rest[..dollar_pos].trim_end();
            let name_str = &rest[dollar_pos..];
            Some((type_str, name_str))
        })
        .collect();

    let max_type_len = param_tags.iter().map(|(t, _)| t.len()).max().unwrap_or(0);

    let mut out = String::new();
    out.push_str(member_indent);
    out.push_str("/**\n");

    for tag in &tags {
        out.push_str(member_indent);
        out.push_str(" * ");
        if let Some(rest) = tag.strip_prefix("@param ") {
            if let Some(dollar_pos) = rest.find('$') {
                let type_str = rest[..dollar_pos].trim_end();
                let name_str = &rest[dollar_pos..];
                out.push_str("@param ");
                out.push_str(type_str);
                // Pad to align parameter names.
                for _ in 0..(max_type_len.saturating_sub(type_str.len())) {
                    out.push(' ');
                }
                out.push(' ');
                out.push_str(name_str);
            } else {
                out.push_str(tag);
            }
        } else {
            out.push_str(tag);
        }
        out.push('\n');
    }

    out.push_str(member_indent);
    out.push_str(" */\n");

    out
}

/// Build the definition text of the extracted function or method.
fn build_extracted_definition(info: &ExtractionInfo) -> String {
    let mut out = String::new();

    // Blank line before the new definition.
    out.push('\n');

    // Prepend PHPDoc block if types need enrichment.
    if !info.docblock.is_empty() {
        out.push_str(&info.docblock);
    }

    let param_list = build_param_list(&info.params);
    let return_type = build_return_type(info);

    match info.target {
        ExtractionTarget::Method => {
            out.push_str(&info.member_indent);
            out.push_str("private ");
            if info.is_static {
                out.push_str("static ");
            }
            out.push_str("function ");
            out.push_str(&info.name);
            out.push('(');
            out.push_str(&param_list);
            out.push(')');
            if !return_type.is_empty() {
                out.push_str(": ");
                out.push_str(&return_type);
            }
            out.push('\n');
            out.push_str(&info.member_indent);
            out.push_str("{\n");
        }
        ExtractionTarget::Function => {
            out.push_str(&info.member_indent);
            out.push_str("function ");
            out.push_str(&info.name);
            out.push('(');
            out.push_str(&param_list);
            out.push(')');
            if !return_type.is_empty() {
                out.push_str(": ");
                out.push_str(&return_type);
            }
            out.push('\n');
            out.push_str(&info.member_indent);
            out.push_str("{\n");
        }
    }

    // Rewrite guard returns in the body if needed.
    let body_text = match &info.return_strategy {
        ReturnStrategy::VoidGuards => {
            // Bare `return;` → `return false;` (false = early exit).
            rewrite_guard_returns(&info.body, None)
        }
        ReturnStrategy::UniformGuards(value) => {
            let lower = value.to_lowercase();
            if lower == "false" || lower == "true" {
                // Already boolean — the body's returns are correct as-is.
                info.body.clone()
            } else {
                // Non-boolean uniform value (e.g. `null`, `0`, `'error'`):
                // rewrite `return <value>;` → `return false;`.
                rewrite_guard_returns(&info.body, Some(value))
            }
        }
        ReturnStrategy::NullGuardWithValue(void_guards) if *void_guards => {
            // Bare `return;` → `return null;` so the extracted
            // function returns null on guard-fire.
            rewrite_void_returns_to_null(&info.body)
        }
        _ => info.body.clone(),
    };

    // Re-indent the body to match the new function's body indentation.
    let body_lines = body_text.lines().collect::<Vec<_>>();
    let min_indent = body_lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    for line in &body_lines {
        if line.trim().is_empty() {
            out.push('\n');
        } else {
            out.push_str(&info.body_indent);
            if line.len() > min_indent {
                out.push_str(&line[min_indent..]);
            }
            out.push('\n');
        }
    }

    // Add return/sentinel after the body based on the strategy.
    match &info.return_strategy {
        ReturnStrategy::TrailingReturn => {
            // Body already ends with `return` — nothing to add.
        }
        ReturnStrategy::VoidGuards => {
            // All guards are bare `return;`.  Add `return true;` as the
            // fall-through (meaning "no early exit, keep going").
            out.push_str(&info.body_indent);
            out.push_str("return true;\n");
        }
        ReturnStrategy::UniformGuards(value) => {
            // All guards return the same value.  The extracted function
            // uses bool: guards become `return false;` (exit), and
            // fall-through is `return true;` (continue).
            // But the body already has the original returns — we need
            // to add the sentinel.  The body's returns stay as-is and
            // get rewritten below by `rewrite_guard_returns_to_bool`.
            // Here we just add the fall-through sentinel.
            let lower = value.to_lowercase();
            let sentinel = if lower == "false" {
                "true"
            } else if lower == "true" {
                "false"
            } else {
                // Non-boolean uniform value: use `true` = continue.
                "true"
            };
            out.push_str(&info.body_indent);
            out.push_str("return ");
            out.push_str(sentinel);
            out.push_str(";\n");
        }
        ReturnStrategy::SentinelNull => {
            // Different non-null values — null = "no early exit".
            out.push_str(&info.body_indent);
            out.push_str("return null;\n");
        }
        ReturnStrategy::NullGuardWithValue(_) => {
            // Guards return null (or were rewritten from bare return;),
            // and we also compute a value.  The fall-through returns
            // the computed variable.
            if info.returns.len() == 1 {
                out.push_str(&info.body_indent);
                out.push_str("return ");
                out.push_str(&info.returns[0].0);
                out.push_str(";\n");
            }
        }
        ReturnStrategy::None | ReturnStrategy::Unsafe => {
            // Normal extraction: add return for captured variables.
            if info.returns.len() == 1 {
                out.push_str(&info.body_indent);
                out.push_str("return ");
                out.push_str(&info.returns[0].0);
                out.push_str(";\n");
            } else if info.returns.len() > 1 {
                out.push_str(&info.body_indent);
                out.push_str("return [");
                let names: Vec<&str> = info.returns.iter().map(|(n, _)| n.as_str()).collect();
                out.push_str(&names.join(", "));
                out.push_str("];\n");
            }
        }
    }

    out.push_str(&info.member_indent);
    out.push_str("}\n");

    out
}

/// Rewrite guard-clause return statements in the body text.
///
/// For `VoidGuards` (`uniform_value` is `None`): bare `return;` becomes
/// `return false;`.
///
/// For `UniformGuards` with a non-boolean value (`uniform_value` is
/// `Some`): `return <value>;` becomes `return false;`.
///
/// This operates on source text rather than AST to keep things simple.
/// It matches `return` followed by optional whitespace and either `;`
/// (void) or the uniform value and `;`.
///
/// See also [`rewrite_void_returns_to_null`] for the
/// `NullGuardWithValue(true)` case.
fn rewrite_guard_returns(body: &str, uniform_value: Option<&str>) -> String {
    match uniform_value {
        None => {
            // VoidGuards: rewrite bare `return;` to `return false;`.
            // We need to be careful not to match `return $x;` etc.
            // Strategy: find `return` followed by optional whitespace
            // then `;`, with no expression in between.
            let mut result = String::with_capacity(body.len());
            let mut remaining = body;
            while let Some(pos) = remaining.find("return") {
                // Check that this is a keyword boundary (not part of
                // `$returnValue` etc.).
                let before_ok = pos == 0
                    || !remaining.as_bytes()[pos - 1].is_ascii_alphanumeric()
                        && remaining.as_bytes()[pos - 1] != b'_'
                        && remaining.as_bytes()[pos - 1] != b'$';
                if !before_ok {
                    result.push_str(&remaining[..pos + 6]);
                    remaining = &remaining[pos + 6..];
                    continue;
                }
                let after = &remaining[pos + 6..];
                let trimmed = after.trim_start();
                if trimmed.starts_with(';') {
                    // Bare `return;` → `return false;`
                    result.push_str(&remaining[..pos]);
                    result.push_str("return false");
                    // Skip past `return` + whitespace, keep the `;`.
                    let ws_len = after.len() - trimmed.len();
                    remaining = &remaining[pos + 6 + ws_len..];
                } else {
                    result.push_str(&remaining[..pos + 6]);
                    remaining = &remaining[pos + 6..];
                }
            }
            result.push_str(remaining);
            result
        }
        Some(value) => {
            // UniformGuards with non-boolean value: rewrite
            // `return <value>;` to `return false;`.
            let mut result = String::with_capacity(body.len());
            let mut remaining = body;
            while let Some(pos) = remaining.find("return") {
                let before_ok = pos == 0
                    || !remaining.as_bytes()[pos - 1].is_ascii_alphanumeric()
                        && remaining.as_bytes()[pos - 1] != b'_'
                        && remaining.as_bytes()[pos - 1] != b'$';
                if !before_ok {
                    result.push_str(&remaining[..pos + 6]);
                    remaining = &remaining[pos + 6..];
                    continue;
                }
                let after = &remaining[pos + 6..];
                let trimmed = after.trim_start();
                // Check if the return expression matches the uniform
                // value (case-insensitive for keywords like `null`).
                let value_trimmed = value.trim();
                if trimmed.len() >= value_trimmed.len() {
                    let candidate = &trimmed[..value_trimmed.len()];
                    let after_value = trimmed[value_trimmed.len()..].trim_start();
                    if candidate.eq_ignore_ascii_case(value_trimmed) && after_value.starts_with(';')
                    {
                        // `return <value>;` → `return false;`
                        result.push_str(&remaining[..pos]);
                        result.push_str("return false");
                        // Skip past `return <ws> <value> <ws>`, keep `;`.
                        let consumed = (trimmed.as_ptr() as usize - after.as_ptr() as usize)
                            + value_trimmed.len()
                            + (after_value.as_ptr() as usize
                                - trimmed[value_trimmed.len()..].as_ptr() as usize);
                        remaining = &remaining[pos + 6 + consumed..];
                        continue;
                    }
                }
                result.push_str(&remaining[..pos + 6]);
                remaining = &remaining[pos + 6..];
            }
            result.push_str(remaining);
            result
        }
    }
}

/// Rewrite bare `return;` to `return null;` in the body text.
///
/// Used by `NullGuardWithValue(true)` — void guard clauses that are
/// extracted alongside a computed value.  The extracted function must
/// return `null` (not void) to signal "guard fired" to the caller.
fn rewrite_void_returns_to_null(body: &str) -> String {
    let mut result = String::with_capacity(body.len());
    let mut remaining = body;
    while let Some(pos) = remaining.find("return") {
        let before_ok = pos == 0
            || !remaining.as_bytes()[pos - 1].is_ascii_alphanumeric()
                && remaining.as_bytes()[pos - 1] != b'_'
                && remaining.as_bytes()[pos - 1] != b'$';
        if !before_ok {
            result.push_str(&remaining[..pos + 6]);
            remaining = &remaining[pos + 6..];
            continue;
        }
        let after = &remaining[pos + 6..];
        let trimmed = after.trim_start();
        if trimmed.starts_with(';') {
            // Bare `return;` → `return null;`
            result.push_str(&remaining[..pos]);
            result.push_str("return null");
            let ws_len = after.len() - trimmed.len();
            remaining = &remaining[pos + 6 + ws_len..];
        } else {
            result.push_str(&remaining[..pos + 6]);
            remaining = &remaining[pos + 6..];
        }
    }
    result.push_str(remaining);
    result
}

/// Build the parameter list string for the function signature.
fn build_param_list(params: &[(String, String)]) -> String {
    params
        .iter()
        .map(|(name, type_hint)| {
            if type_hint.is_empty() {
                name.clone()
            } else {
                format!("{} {}", type_hint, name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Build the return type annotation string.
fn build_return_type(info: &ExtractionInfo) -> String {
    match &info.return_strategy {
        ReturnStrategy::TrailingReturn => {
            // Use the enclosing function's return type.
            let t = clean_type_for_signature(&info.trailing_return_type);
            if !t.is_empty() {
                return t;
            }
            String::new()
        }
        ReturnStrategy::VoidGuards | ReturnStrategy::UniformGuards(_) => {
            // Guard strategies use bool: true = continue, false = exit.
            "bool".to_string()
        }
        ReturnStrategy::SentinelNull => {
            // Sentinel-null: the return type is nullable.  Try to
            // derive it from the trailing_return_type if available,
            // otherwise leave untyped.
            let t = clean_type_for_signature(&info.trailing_return_type);
            if !t.is_empty() && !t.starts_with('?') && t != "null" && t != "mixed" {
                return format!("?{}", t);
            }
            // Can't determine a useful nullable type.
            String::new()
        }
        ReturnStrategy::NullGuardWithValue(_) => {
            // The return type is the computed value's type made nullable.
            if info.returns.len() == 1 {
                let type_hint = &info.returns[0].1;
                if !type_hint.is_empty() {
                    let t = clean_type_for_signature(type_hint);
                    if !t.is_empty() && !t.starts_with('?') && t != "null" && t != "mixed" {
                        return format!("?{}", t);
                    }
                    // Already nullable or mixed — use as-is.
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
            String::new()
        }
        ReturnStrategy::None | ReturnStrategy::Unsafe => {
            // Normal extraction — derive from return variables.
            if info.returns.is_empty() {
                return "void".to_string();
            }
            if info.returns.len() == 1 {
                let type_hint = &info.returns[0].1;
                if type_hint.is_empty() {
                    return String::new();
                }
                return type_hint.clone();
            }
            // Multiple return values → return as array.
            "array".to_string()
        }
    }
}

/// Build the call-site text that replaces the selected statements.
fn build_call_site(info: &ExtractionInfo, call_indent: &str) -> String {
    let mut out = String::new();

    let args: Vec<&str> = info.params.iter().map(|(n, _)| n.as_str()).collect();
    let arg_list = args.join(", ");

    // Build the function/method call expression.
    let call_expr = match info.target {
        ExtractionTarget::Method => {
            if info.is_static {
                format!("self::{}({})", info.name, arg_list)
            } else {
                format!("$this->{}({})", info.name, arg_list)
            }
        }
        ExtractionTarget::Function => {
            format!("{}({})", info.name, arg_list)
        }
    };

    match &info.return_strategy {
        ReturnStrategy::TrailingReturn => {
            // The body ends with `return expr;` — the call site passes
            // the return value through.
            out.push_str(call_indent);
            out.push_str("return ");
            out.push_str(&call_expr);
            out.push_str(";\n");
        }
        ReturnStrategy::VoidGuards => {
            // Extracted function returns bool (true = continue).
            // Call site: `if (!extracted(…)) return;`
            out.push_str(call_indent);
            out.push_str("if (!");
            out.push_str(&call_expr);
            out.push_str(") return;\n");
        }
        ReturnStrategy::UniformGuards(value) => {
            // Extracted function returns bool (true = continue).
            // Call site: `if (!extracted(…)) return <value>;`
            out.push_str(call_indent);
            out.push_str("if (!");
            out.push_str(&call_expr);
            out.push_str(") return ");
            out.push_str(value);
            out.push_str(";\n");
        }
        ReturnStrategy::SentinelNull => {
            // Extracted function returns null on fall-through, or the
            // actual value on early exit.
            // Call site:
            //   $__early = extracted(…);
            //   if ($__early !== null) return $__early;
            out.push_str(call_indent);
            out.push_str("$__early = ");
            out.push_str(&call_expr);
            out.push_str(";\n");
            out.push_str(call_indent);
            out.push_str("if ($__early !== null) return $__early;\n");
        }
        ReturnStrategy::NullGuardWithValue(void_guards) => {
            // Guards return null (or were void), the function also
            // computes a value.
            // Call site:
            //   $var = extracted(…);
            //   if ($var === null) return null;  // or `return;`
            if info.returns.len() == 1 {
                out.push_str(call_indent);
                out.push_str(&info.returns[0].0);
                out.push_str(" = ");
                out.push_str(&call_expr);
                out.push_str(";\n");
                out.push_str(call_indent);
                out.push_str("if (");
                out.push_str(&info.returns[0].0);
                if *void_guards {
                    out.push_str(" === null) return;\n");
                } else {
                    out.push_str(" === null) return null;\n");
                }
            }
        }
        ReturnStrategy::None | ReturnStrategy::Unsafe => {
            // Normal extraction.
            if info.returns.is_empty() {
                // No return values — just call the function.
                out.push_str(call_indent);
                out.push_str(&call_expr);
                out.push_str(";\n");
            } else if info.returns.len() == 1 {
                // Single return value — assign it.
                out.push_str(call_indent);
                out.push_str(&info.returns[0].0);
                out.push_str(" = ");
                out.push_str(&call_expr);
                out.push_str(";\n");
            } else {
                // Multiple return values — destructure from array.
                let vars: Vec<&str> = info.returns.iter().map(|(n, _)| n.as_str()).collect();
                out.push_str(call_indent);
                out.push('[');
                out.push_str(&vars.join(", "));
                out.push_str("] = ");
                out.push_str(&call_expr);
                out.push_str(";\n");
            }
        }
    }

    out
}

// ─── Return statement analysis ──────────────────────────────────────────────

/// Analyse `return` statements within the selected range and determine
/// the extraction strategy.
///
/// The returned `ReturnStrategy` tells the code generator how to handle
/// early returns in the extracted code:
/// - `None` — no returns in the selection.
/// - `TrailingReturn` — last statement is `return`, call site uses
///   `return extracted(…)`.
/// - `VoidGuards` / `UniformGuards` / `SentinelNull` — guard-clause
///   patterns that can be safely extracted with special call sites.
/// - `Unsafe` — cannot safely extract.
///
/// `return_value_count` is the number of variables modified inside the
/// selection that are read after it (the scope classifier's
/// `return_values.len()`).  Most guard strategies are rejected when
/// this is non-zero, except `NullGuardWithValue` which handles exactly
/// one return value with all-null guards.
fn analyse_returns(
    content: &str,
    start: usize,
    end: usize,
    return_value_count: usize,
) -> ReturnStrategy {
    let arena = Bump::new();
    let file_id = mago_database::file::FileId::new("extract_fn_ret");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content);

    let body_stmts = find_enclosing_body_statements(&program.statements, start as u32);

    // Collect the statements that fall inside the selection.
    let selected: Vec<&Statement<'_>> = body_stmts
        .iter()
        .filter(|stmt| {
            let span = stmt.span();
            let s = span.start.offset as usize;
            let e = span.end.offset as usize;
            s >= start && e <= end
        })
        .copied()
        .collect();

    if selected.is_empty() {
        return ReturnStrategy::None;
    }

    // Check whether the last selected statement is a `return`.
    let has_trailing_return = matches!(selected.last(), Some(Statement::Return(_)));

    // Check whether any statement in the selection contains a return
    // (at any nesting level).
    let any_return = selected.iter().any(|s| selection_stmt_contains_return(s));

    if !any_return {
        return ReturnStrategy::None;
    }

    // When the selection ends with `return`, the call site is
    // `return extracted(…)`, so every return path inside the
    // extracted function propagates correctly.
    if has_trailing_return {
        return ReturnStrategy::TrailingReturn;
    }

    // The selection contains returns but does NOT end with one.
    // Try to find a guard-clause strategy.
    classify_guard_returns(content, &selected, return_value_count)
}

/// Check whether a statement is or contains a `return` at any depth.
fn selection_stmt_contains_return(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::Return(_) => true,
        Statement::If(if_stmt) => match &if_stmt.body {
            IfBody::Statement(body) => {
                selection_stmt_contains_return(body.statement)
                    || body
                        .else_if_clauses
                        .iter()
                        .any(|c| selection_stmt_contains_return(c.statement))
                    || body
                        .else_clause
                        .as_ref()
                        .is_some_and(|c| selection_stmt_contains_return(c.statement))
            }
            IfBody::ColonDelimited(body) => {
                body.statements
                    .iter()
                    .any(|s| selection_stmt_contains_return(s))
                    || body.else_if_clauses.iter().any(|c| {
                        c.statements
                            .iter()
                            .any(|s| selection_stmt_contains_return(s))
                    })
                    || body.else_clause.as_ref().is_some_and(|c| {
                        c.statements
                            .iter()
                            .any(|s| selection_stmt_contains_return(s))
                    })
            }
        },
        Statement::Foreach(f) => match &f.body {
            ForeachBody::Statement(s) => selection_stmt_contains_return(s),
            ForeachBody::ColonDelimited(b) => b
                .statements
                .iter()
                .any(|s| selection_stmt_contains_return(s)),
        },
        Statement::While(w) => match &w.body {
            WhileBody::Statement(s) => selection_stmt_contains_return(s),
            WhileBody::ColonDelimited(b) => b
                .statements
                .iter()
                .any(|s| selection_stmt_contains_return(s)),
        },
        Statement::DoWhile(dw) => selection_stmt_contains_return(dw.statement),
        Statement::For(f) => match &f.body {
            ForBody::Statement(s) => selection_stmt_contains_return(s),
            ForBody::ColonDelimited(b) => b
                .statements
                .iter()
                .any(|s| selection_stmt_contains_return(s)),
        },
        Statement::Switch(sw) => sw.body.cases().iter().any(|c| match c {
            SwitchCase::Expression(e) => e
                .statements
                .iter()
                .any(|s| selection_stmt_contains_return(s)),
            SwitchCase::Default(d) => d
                .statements
                .iter()
                .any(|s| selection_stmt_contains_return(s)),
        }),
        Statement::Try(t) => {
            t.block
                .statements
                .iter()
                .any(|s| selection_stmt_contains_return(s))
                || t.catch_clauses.iter().any(|c| {
                    c.block
                        .statements
                        .iter()
                        .any(|s| selection_stmt_contains_return(s))
                })
                || t.finally_clause.as_ref().is_some_and(|f| {
                    f.block
                        .statements
                        .iter()
                        .any(|s| selection_stmt_contains_return(s))
                })
        }
        Statement::Block(b) => b
            .statements
            .iter()
            .any(|s| selection_stmt_contains_return(s)),
        _ => false,
    }
}

// ─── Return strategy ────────────────────────────────────────────────────────

/// How to handle return statements in the extracted code.
///
/// When the selection contains `return` statements that are NOT the last
/// statement, naive extraction would break control flow.  This enum
/// describes the strategy for preserving the caller's early-exit
/// semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReturnStrategy {
    /// No return statements in the selection.
    None,
    /// The last selected statement is a `return` — the call site becomes
    /// `return extracted(…)` and every return path propagates correctly.
    TrailingReturn,
    /// All returns are bare `return;` (void guards).  The extracted
    /// function returns `bool` (true = continue, false = exit early)
    /// and the call site is `if (!extracted(…)) return;`.
    VoidGuards,
    /// All returns return the same non-null literal value.  The
    /// extracted function returns `bool` and the call site is
    /// `if (!extracted(…)) return <value>;`.
    ///
    /// The string is the source text of the common return value.
    UniformGuards(String),
    /// Returns have different non-null values — use `null` as a
    /// sentinel for "no early exit."  The extracted function returns
    /// `?<type>` and the call site is:
    /// ```php
    /// $__early = extracted(…);
    /// if ($__early !== null) return $__early;
    /// ```
    SentinelNull,
    /// All guard returns are `null` (or bare `return;`) and the
    /// selection also computes exactly one return value.  The extracted
    /// function returns the computed value on success or `null` when a
    /// guard fires.  The call site assigns the result and checks for
    /// null:
    /// ```php
    /// $var = extracted(…);
    /// if ($var === null) return null;  // or `return;` for void guards
    /// ```
    ///
    /// The `bool` flag is `true` when the original guards were bare
    /// `return;` (void).  In that case the body's `return;` statements
    /// are rewritten to `return null;`, and the call site uses bare
    /// `return;` instead of `return null;`.
    NullGuardWithValue(bool),
    /// Cannot safely extract (e.g. returns null, or modified variables
    /// are used after the selection).
    Unsafe,
}

/// Collect the source text of every `return` expression in the selected
/// statements.
///
/// Bare `return;` is represented as `None`.  `return expr;` yields
/// `Some("expr")` with the expression's source text.
fn collect_return_expressions<'a>(
    content: &'a str,
    stmts: &[&Statement<'_>],
) -> Vec<Option<&'a str>> {
    let mut out = Vec::new();
    for stmt in stmts {
        collect_returns_from_stmt(content, stmt, &mut out);
    }
    out
}

/// Recursively collect return expressions from a single statement.
fn collect_returns_from_stmt<'a>(
    content: &'a str,
    stmt: &Statement<'_>,
    out: &mut Vec<Option<&'a str>>,
) {
    match stmt {
        Statement::Return(ret) => {
            let expr_text = ret.value.as_ref().map(|expr| {
                let s = expr.span().start.offset as usize;
                let e = expr.span().end.offset as usize;
                content[s..e].trim()
            });
            out.push(expr_text);
        }
        Statement::If(if_stmt) => match &if_stmt.body {
            IfBody::Statement(body) => {
                collect_returns_from_stmt(content, body.statement, out);
                for c in &body.else_if_clauses {
                    collect_returns_from_stmt(content, c.statement, out);
                }
                if let Some(c) = &body.else_clause {
                    collect_returns_from_stmt(content, c.statement, out);
                }
            }
            IfBody::ColonDelimited(body) => {
                for s in &body.statements {
                    collect_returns_from_stmt(content, s, out);
                }
                for c in &body.else_if_clauses {
                    for s in &c.statements {
                        collect_returns_from_stmt(content, s, out);
                    }
                }
                if let Some(c) = &body.else_clause {
                    for s in &c.statements {
                        collect_returns_from_stmt(content, s, out);
                    }
                }
            }
        },
        Statement::Foreach(f) => match &f.body {
            ForeachBody::Statement(s) => collect_returns_from_stmt(content, s, out),
            ForeachBody::ColonDelimited(b) => {
                for s in &b.statements {
                    collect_returns_from_stmt(content, s, out);
                }
            }
        },
        Statement::While(w) => match &w.body {
            WhileBody::Statement(s) => collect_returns_from_stmt(content, s, out),
            WhileBody::ColonDelimited(b) => {
                for s in &b.statements {
                    collect_returns_from_stmt(content, s, out);
                }
            }
        },
        Statement::DoWhile(dw) => collect_returns_from_stmt(content, dw.statement, out),
        Statement::For(f) => match &f.body {
            ForBody::Statement(s) => collect_returns_from_stmt(content, s, out),
            ForBody::ColonDelimited(b) => {
                for s in &b.statements {
                    collect_returns_from_stmt(content, s, out);
                }
            }
        },
        Statement::Switch(sw) => {
            for c in sw.body.cases().iter() {
                let stmts = match c {
                    SwitchCase::Expression(e) => &e.statements,
                    SwitchCase::Default(d) => &d.statements,
                };
                for s in stmts.iter() {
                    collect_returns_from_stmt(content, s, out);
                }
            }
        }
        Statement::Try(t) => {
            for s in &t.block.statements {
                collect_returns_from_stmt(content, s, out);
            }
            for c in &t.catch_clauses {
                for s in &c.block.statements {
                    collect_returns_from_stmt(content, s, out);
                }
            }
            if let Some(f) = &t.finally_clause {
                for s in &f.block.statements {
                    collect_returns_from_stmt(content, s, out);
                }
            }
        }
        Statement::Block(b) => {
            for s in &b.statements {
                collect_returns_from_stmt(content, s, out);
            }
        }
        _ => {}
    }
}

/// Classify the return strategy for a selection that contains return
/// statements but does NOT end with one.
///
/// This is called only when `has_unsafe_return` would have been `true`
/// under the old logic.  It inspects the actual return expressions to
/// decide whether a safe extraction pattern exists.
fn classify_guard_returns(
    content: &str,
    stmts: &[&Statement<'_>],
    return_value_count: usize,
) -> ReturnStrategy {
    let return_exprs = collect_return_expressions(content, stmts);
    if return_exprs.is_empty() {
        return ReturnStrategy::Unsafe;
    }

    // When the selection modifies variables that are used after it,
    // most guard strategies can't work — we'd need to return both
    // the sentinel and the modified variables.  The exception is
    // NullGuardWithValue: all guards return null (or bare return;),
    // exactly one return value, and the extracted function returns
    // the value or null.
    if return_value_count > 0 {
        if return_value_count != 1 {
            return ReturnStrategy::Unsafe;
        }
        // All bare `return;` → NullGuardWithValue(true) (void guards).
        if return_exprs.iter().all(|e| e.is_none()) {
            return ReturnStrategy::NullGuardWithValue(true);
        }
        // All `return null;` → NullGuardWithValue(false).
        if return_exprs.iter().any(|e| e.is_none()) {
            // Mix of bare and valued returns — can't handle.
            return ReturnStrategy::Unsafe;
        }
        let all_null = return_exprs
            .iter()
            .all(|e| e.unwrap().trim().eq_ignore_ascii_case("null"));
        if all_null {
            return ReturnStrategy::NullGuardWithValue(false);
        }
        return ReturnStrategy::Unsafe;
    }

    // Case 1: All returns are bare `return;` (void guards).
    if return_exprs.iter().all(|e| e.is_none()) {
        return ReturnStrategy::VoidGuards;
    }

    // If any return is bare but others aren't, we have a mix of void
    // and valued returns — can't handle this.
    if return_exprs.iter().any(|e| e.is_none()) {
        return ReturnStrategy::Unsafe;
    }

    // All returns have values.  Check if any returns null.
    let values: Vec<&str> = return_exprs.iter().map(|e| e.unwrap()).collect();
    let any_returns_null = values.iter().any(|v| {
        let lower = v.trim().to_lowercase();
        lower == "null"
    });

    // Case 2: All return the same value.
    let all_same = values.windows(2).all(|w| w[0].trim() == w[1].trim());
    if all_same {
        let value = values[0].trim().to_string();
        // If the uniform value is `true` or `false`, we can use the
        // inverse as the sentinel — the cleanest possible output.
        let lower = value.to_lowercase();
        if lower == "false" || lower == "true" {
            return ReturnStrategy::UniformGuards(value);
        }
        // If the uniform value is `null`, we can't use null as sentinel,
        // but we can still use bool: the extracted function returns bool,
        // and the call site does `if (!extracted()) return null;`.
        if lower == "null" {
            return ReturnStrategy::UniformGuards(value);
        }
        // For other uniform values, if it's not null, bool flag works.
        return ReturnStrategy::UniformGuards(value);
    }

    // Case 3: Different values, none are null — use null sentinel.
    if !any_returns_null {
        return ReturnStrategy::SentinelNull;
    }

    // Different values including null — can't use null as sentinel
    // and can't use bool flag either.
    ReturnStrategy::Unsafe
}

/// Resolve the return type of the enclosing function/method at `offset`.
///
/// Extracts the native return type hint from the function signature.
/// Extract the parameter names of the enclosing function/method in
/// declaration order.  Used to sort extracted-function parameters so
/// they mirror the original signature.
fn resolve_enclosing_param_order(content: &str, offset: u32) -> Vec<String> {
    let arena = Bump::new();
    let file_id = mago_database::file::FileId::new("extract_fn_pord");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content);

    let ctx = find_cursor_context(&program.statements, offset);

    let param_list = match ctx {
        CursorContext::InClassLike { member, .. } => {
            if let MemberContext::Method(method, true) = member {
                Some(&method.parameter_list)
            } else {
                None
            }
        }
        CursorContext::InFunction(func, true) => Some(&func.parameter_list),
        _ => None,
    };

    match param_list {
        Some(pl) => pl
            .parameters
            .iter()
            .map(|p| p.variable.name.to_string())
            .collect(),
        None => Vec::new(),
    }
}

/// Sort extracted-function parameters so that variables matching the
/// enclosing function's signature come first (in their original order),
/// followed by any other variables in classification order.
fn sort_params_by_enclosing_order(
    mut params: Vec<(String, String, String)>,
    enclosing_order: &[String],
) -> Vec<(String, String, String)> {
    if enclosing_order.is_empty() {
        return params;
    }
    params.sort_by(|a, b| {
        let idx_a = enclosing_order.iter().position(|n| *n == a.0);
        let idx_b = enclosing_order.iter().position(|n| *n == b.0);
        match (idx_a, idx_b) {
            // Both are signature params → preserve signature order.
            (Some(ia), Some(ib)) => ia.cmp(&ib),
            // Signature params come before non-signature variables.
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            // Neither is a signature param → preserve classification order.
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    params
}

fn resolve_enclosing_return_type(content: &str, offset: u32) -> String {
    let arena = Bump::new();
    let file_id = mago_database::file::FileId::new("extract_fn_rtype");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content);

    let ctx = find_cursor_context(&program.statements, offset);

    match ctx {
        CursorContext::InClassLike { member, .. } => {
            if let MemberContext::Method(method, true) = member {
                return method
                    .return_type_hint
                    .as_ref()
                    .map(|h| {
                        let s = h.span().start.offset as usize;
                        let e = h.span().end.offset as usize;
                        strip_return_type_colon(content[s..e].trim())
                    })
                    .unwrap_or_default();
            }
            String::new()
        }
        CursorContext::InFunction(func, true) => func
            .return_type_hint
            .as_ref()
            .map(|h| {
                let s = h.span().start.offset as usize;
                let e = h.span().end.offset as usize;
                strip_return_type_colon(content[s..e].trim())
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// Strip the leading `: ` from a return type hint span.
///
/// The mago AST's `return_type_hint` span includes the colon and
/// whitespace prefix (e.g. `": string"`).  This helper strips that
/// prefix to yield just the type name.
fn strip_return_type_colon(raw: &str) -> String {
    let stripped = raw.strip_prefix(':').unwrap_or(raw).trim_start();
    stripped.to_string()
}

// ─── Main code action collector ─────────────────────────────────────────────

impl Backend {
    /// Collect "Extract Function" / "Extract Method" code actions.
    ///
    /// This action is offered when the user has a non-empty selection
    /// that covers one or more complete statements inside a function or
    /// method body.
    pub(crate) fn collect_extract_function_actions(
        &self,
        uri: &str,
        content: &str,
        params: &CodeActionParams,
        out: &mut Vec<CodeActionOrCommand>,
    ) {
        // Only activate when the selection is non-empty.
        if params.range.start == params.range.end {
            return;
        }

        let start_offset = position_to_byte_offset(content, params.range.start);
        let end_offset = position_to_byte_offset(content, params.range.end);

        // Trim the selection to exclude leading/trailing whitespace.
        let (start, end) = match trim_selection(content, start_offset, end_offset) {
            Some(range) => range,
            None => return,
        };

        // Validate that the selection covers complete statements.
        if !selection_covers_complete_statements(content, start, end) {
            return;
        }

        // ── From here the user plausibly intended an extraction ──────
        // Any rejection below emits a *disabled* code action with a
        // reason string instead of silently returning.

        // Build scope map and classify the selected range.
        let scope_map = build_scope_map(content, start as u32);
        let classification = scope_map.classify_range(start as u32, end as u32);

        // Analyse return statements in the selection.  We pass
        // whether the selection has return values (variables modified
        // inside and read after) so guard strategies can be rejected
        // when we'd need to return both a sentinel and modified vars.
        let return_value_count = classification.return_values.len();
        let return_strategy = analyse_returns(content, start, end, return_value_count);

        // Reject when the return strategy is Unsafe.
        if return_strategy == ReturnStrategy::Unsafe {
            Self::push_disabled_extract(
                out,
                "Selection contains return statements that cannot be safely extracted",
            );
            return;
        }

        // For NullGuardWithValue the return values are incorporated into
        // the extracted function's return (not as separate out-variables),
        // so we must also resolve the enclosing return type to derive a
        // nullable hint.

        // Use the scope-level flag as a quick pre-check before the more
        // granular range classification.  If the *entire* scope has no
        // $this/self/static/parent usage, the range certainly doesn't
        // either — skip the detailed per-access scan in that case.
        let uses_this = if scope_map.has_this_or_self {
            classification.uses_this
        } else {
            false
        };

        // When the scope uses by-reference parameters, extraction could
        // change semantics (writes through `&$var` would no longer
        // propagate to the caller).  Reject for safety.
        if scope_map.uses_reference_params() && !classification.reference_writes.is_empty() {
            Self::push_disabled_extract(out, "Selection writes to by-reference parameters");
            return;
        }

        // Reject if there are too many return values (more than can be
        // cleanly handled with list() / array destructuring).
        if classification.return_values.len() > 4 {
            Self::push_disabled_extract(out, "Too many return values for clean extraction");
            return;
        }

        // Determine enclosing context (method vs function) and insertion point.
        let enclosing = match find_enclosing_context(content, start as u32, uses_this) {
            Some(ctx) => ctx,
            None => return,
        };

        // Generate the function/method name.  We need the return
        // strategy and body text for contextual naming, so this
        // happens after strategy analysis.
        let body_line_start_for_naming = find_line_start(content, start);
        let body_text_for_naming = &content[body_line_start_for_naming..end];
        let naming_ctx = NamingContext {
            enclosing_name: &enclosing.enclosing_name,
            return_strategy: &return_strategy,
            body_text: body_text_for_naming,
            return_var_names: &classification.return_values,
            trailing_return_type: "",
        };
        // We don't have the trailing return type yet (it's resolved
        // below), but the naming heuristic only uses it for
        // TrailingReturn, which we can approximate here.
        let pre_trailing_return_type = if matches!(return_strategy, ReturnStrategy::TrailingReturn) {
            resolve_enclosing_return_type(content, start as u32)
        } else {
            String::new()
        };
        let naming_ctx = NamingContext {
            trailing_return_type: &pre_trailing_return_type,
            ..naming_ctx
        };
        let fn_name = generate_function_name(content, &enclosing, &naming_ctx);

        // Resolve types for parameters and return values.
        let typed_params =
            self.resolve_param_types(uri, content, start as u32, &classification.parameters);
        // Sort parameters: enclosing function's signature params first
        // (in their original order), then any other variables in the
        // order they were classified.
        let enclosing_param_order = resolve_enclosing_param_order(content, start as u32);
        let typed_params = sort_params_by_enclosing_order(typed_params, &enclosing_param_order);
        let typed_returns =
            self.resolve_param_types(uri, content, start as u32, &classification.return_values);

        // Determine indentation.
        let call_indent = indent_at(content, start);
        let (member_indent, body_indent) = match enclosing.target {
            ExtractionTarget::Method => {
                // The member indent is the same level as the enclosing
                // method's signature line (sibling methods share this
                // indentation).  `body_start` points at the `{` which
                // sits on the same line as the method signature.
                let member = detect_line_indent(content, enclosing.body_start);
                let unit = detect_indent_unit(content);
                let body = format!("{}{}", member, unit);
                (member, body)
            }
            ExtractionTarget::Function => {
                let member = String::new();
                let unit = detect_indent_unit(content);
                (member, unit.to_string())
            }
        };

        // Extract the selected text (the body of the new function).
        // Start from the beginning of the first line so that the
        // leading whitespace is included — this ensures all lines
        // have consistent indentation for the re-indent pass.
        let body_line_start = find_line_start(content, start);
        let body_text = content[body_line_start..end].to_string();

        // When the selection ends with `return`, uses sentinel-null,
        // or uses null-guard-with-value, resolve the enclosing
        // function's return type.
        let trailing_return_type = if matches!(
            return_strategy,
            ReturnStrategy::TrailingReturn
                | ReturnStrategy::SentinelNull
                | ReturnStrategy::NullGuardWithValue(_)
        ) {
            resolve_enclosing_return_type(content, start as u32)
        } else {
            String::new()
        };

        // Also resolve the docblock @return type of the enclosing
        // function — it may carry concrete generic arguments (e.g.
        // `Collection<User>`) that the native hint lacks.
        let enclosing_docblock_return = if matches!(
            return_strategy,
            ReturnStrategy::TrailingReturn | ReturnStrategy::SentinelNull
        ) {
            crate::docblock::find_enclosing_return_type(content, start).unwrap_or_default()
        } else {
            String::new()
        };

        // ── PHPDoc generation ───────────────────────────────────────
        // Build a docblock when parameter or return types benefit from
        // enrichment (generics, array<K,V>, callable signatures, etc.).
        let return_type_for_docblock = build_return_type_hint_for_docblock(
            &return_strategy,
            &trailing_return_type,
            &typed_returns,
        );
        let raw_return_type_for_docblock = build_raw_return_type_for_docblock(
            &return_strategy,
            &trailing_return_type,
            &enclosing_docblock_return,
            &typed_returns,
        );
        let ctx = self.file_context(uri);
        let class_loader = self.class_loader(&ctx);
        let docblock = build_docblock_for_extraction(
            &typed_params,
            &return_type_for_docblock,
            &raw_return_type_for_docblock,
            &member_indent,
            &class_loader,
        );

        // Strip raw types for ExtractionInfo (only cleaned hints
        // are needed for code generation).
        let params_for_info: Vec<(String, String)> = typed_params
            .iter()
            .map(|(name, cleaned, _)| (name.clone(), cleaned.clone()))
            .collect();
        let returns_for_info: Vec<(String, String)> = typed_returns
            .iter()
            .map(|(name, cleaned, _)| (name.clone(), cleaned.clone()))
            .collect();

        let info = ExtractionInfo {
            name: fn_name.clone(),
            params: params_for_info,
            returns: returns_for_info,
            body: body_text,
            target: enclosing.target,
            is_static: enclosing.is_static,
            member_indent,
            body_indent,
            return_strategy,
            trailing_return_type,
            docblock,
        };

        // Build the definition text.
        let definition = build_extracted_definition(&info);

        // Build the call-site text.
        let call_site = build_call_site(&info, &call_indent);

        // Build workspace edits.
        let doc_uri: Url = match uri.parse() {
            Ok(u) => u,
            Err(_) => return,
        };

        // Expand the replacement range to cover the full lines of the selection.
        let replace_start = find_line_start(content, start);
        let replace_end = find_line_end(content, end.saturating_sub(1).max(start));

        let replace_start_pos = offset_to_position(content, replace_start);
        let replace_end_pos = offset_to_position(content, replace_end);

        // Insertion point for the new definition.
        let insert_pos = offset_to_position(content, enclosing.insert_offset);

        let edits = vec![
            // Edit 1: Replace the selected statements with the call site.
            TextEdit {
                range: Range {
                    start: replace_start_pos,
                    end: replace_end_pos,
                },
                new_text: call_site,
            },
            // Edit 2: Insert the new function/method definition.
            TextEdit {
                range: Range {
                    start: insert_pos,
                    end: insert_pos,
                },
                new_text: definition,
            },
        ];

        let mut changes = HashMap::new();
        changes.insert(doc_uri, edits);

        let title = match enclosing.target {
            ExtractionTarget::Method => format!("Extract method '{}'", fn_name),
            ExtractionTarget::Function => format!("Extract function '{}'", fn_name),
        };

        out.push(CodeActionOrCommand::CodeAction(CodeAction {
            title,
            kind: Some(CodeActionKind::REFACTOR_EXTRACT),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: Some(false),
            disabled: None,
            data: None,
        }));
    }

    /// Push a disabled "Extract function/method" code action with a
    /// human-readable reason string.  Editors show this greyed-out in
    /// the refactor menu so the user knows *why* extraction is
    /// unavailable for their selection.
    fn push_disabled_extract(out: &mut Vec<CodeActionOrCommand>, reason: &str) {
        out.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: "Extract function/method".to_string(),
            kind: Some(CodeActionKind::REFACTOR_EXTRACT),
            diagnostics: None,
            edit: None,
            command: None,
            is_preferred: Some(false),
            disabled: Some(CodeActionDisabled {
                reason: reason.to_string(),
            }),
            data: None,
        }));
    }

    /// Resolve types for a list of variable names at a given offset.
    ///
    /// Returns `(dollar_name, cleaned_hint, raw_hint)` triples.
    /// `cleaned_hint` has generics stripped for use in native PHP
    /// signatures.  `raw_hint` preserves the full resolved type
    /// (e.g. `Collection<User>`) for PHPDoc generation.
    fn resolve_param_types(
        &self,
        uri: &str,
        content: &str,
        offset: u32,
        var_names: &[String],
    ) -> Vec<(String, String, String)> {
        var_names
            .iter()
            .map(|name| {
                let dollar_name = if name.starts_with('$') {
                    name.clone()
                } else {
                    format!("${}", name)
                };
                let type_hint =
                    resolve_var_type(self, &dollar_name, content, offset, uri).unwrap_or_default();
                // Clean up the type hint for use in a signature.
                let cleaned = clean_type_for_signature(&type_hint);
                (dollar_name, cleaned, type_hint)
            })
            .collect()
    }
}

/// Clean a resolved type string for use in a function signature.
///
/// Removes generic parameters (PHP doesn't support them in signatures),
/// and simplifies union types that are too complex for type hints.
/// Compute the raw (un-cleaned) return type hint string for PHPDoc
/// enrichment purposes.  Unlike `build_return_type` (which strips
/// generics for native hints), this preserves the full type so that
/// `enrichment_plain` can detect whether a docblock `@return` tag is
/// warranted.
fn build_return_type_hint_for_docblock(
    strategy: &ReturnStrategy,
    trailing_return_type: &str,
    returns: &[(String, String, String)],
) -> String {
    match strategy {
        ReturnStrategy::TrailingReturn => trailing_return_type.to_string(),
        ReturnStrategy::VoidGuards | ReturnStrategy::UniformGuards(_) => "bool".to_string(),
        ReturnStrategy::SentinelNull => {
            if !trailing_return_type.is_empty() {
                trailing_return_type.to_string()
            } else {
                String::new()
            }
        }
        ReturnStrategy::NullGuardWithValue(_) => {
            if returns.len() == 1 && !returns[0].1.is_empty() {
                returns[0].1.clone()
            } else {
                String::new()
            }
        }
        ReturnStrategy::None | ReturnStrategy::Unsafe => {
            if returns.is_empty() {
                "void".to_string()
            } else if returns.len() == 1 {
                returns[0].1.clone()
            } else {
                "array".to_string()
            }
        }
    }
}

/// Like `build_return_type_hint_for_docblock` but returns the raw
/// (un-cleaned) type string that preserves concrete generic arguments.
fn build_raw_return_type_for_docblock(
    strategy: &ReturnStrategy,
    trailing_return_type: &str,
    enclosing_docblock_return: &str,
    returns: &[(String, String, String)],
) -> String {
    match strategy {
        ReturnStrategy::TrailingReturn => {
            // Prefer the docblock @return type when it carries concrete
            // generics (e.g. `Collection<User>`) over the native hint
            // (e.g. `Collection`).
            if !enclosing_docblock_return.is_empty() && enclosing_docblock_return.contains('<') {
                enclosing_docblock_return.to_string()
            } else {
                trailing_return_type.to_string()
            }
        }
        ReturnStrategy::VoidGuards | ReturnStrategy::UniformGuards(_) => "bool".to_string(),
        ReturnStrategy::SentinelNull => {
            if !enclosing_docblock_return.is_empty() && enclosing_docblock_return.contains('<') {
                enclosing_docblock_return.to_string()
            } else if !trailing_return_type.is_empty() {
                trailing_return_type.to_string()
            } else {
                String::new()
            }
        }
        ReturnStrategy::NullGuardWithValue(_) => {
            // Use raw type (index 2) which preserves generics.
            if returns.len() == 1 && !returns[0].2.is_empty() {
                returns[0].2.clone()
            } else {
                String::new()
            }
        }
        ReturnStrategy::None | ReturnStrategy::Unsafe => {
            if returns.is_empty() {
                "void".to_string()
            } else if returns.len() == 1 {
                // Use raw type (index 2) which preserves generics.
                returns[0].2.clone()
            } else {
                "array".to_string()
            }
        }
    }
}

fn clean_type_for_signature(type_str: &str) -> String {
    if type_str.is_empty() {
        return String::new();
    }

    // Parenthesized callable signatures like `(Closure(int): string)`
    // or `(callable(Foo): Bar)` — extract the base callable name for
    // the native PHP hint.
    let trimmed = type_str.trim();
    if let Some(inner) = trimmed.strip_prefix('(') {
        for callable in &["Closure", "callable"] {
            if inner.starts_with(callable) {
                return callable.to_string();
            }
        }
    }

    // If it contains generic params like `array<string, int>`, strip them
    // for the native type hint.
    let cleaned = strip_generics_for_hint(type_str);

    // If it's a simple scalar or class name, use it directly.
    if is_valid_php_type_hint(&cleaned) {
        return cleaned;
    }

    // For complex types (unions with more than PHP supports), return empty.
    String::new()
}

/// Strip generic parameters from a type string.
///
/// `Collection<int, string>` → `Collection`
/// `array<string>` → `array`
fn strip_generics_for_hint(type_str: &str) -> String {
    if let Some(pos) = type_str.find('<') {
        type_str[..pos].to_string()
    } else {
        type_str.to_string()
    }
}

/// Check whether a type string is valid as a PHP native type hint.
fn is_valid_php_type_hint(type_str: &str) -> bool {
    if type_str.is_empty() {
        return false;
    }

    // Handle nullable types.
    let inner = type_str.strip_prefix('?').unwrap_or(type_str);

    // Simple scalar types.
    let scalars = [
        "int", "float", "string", "bool", "array", "callable", "void", "null", "false", "true",
        "never", "object", "mixed", "iterable", "self", "static", "parent",
    ];
    if scalars.contains(&inner) {
        return true;
    }

    // Union types: `Type1|Type2`
    if inner.contains('|') {
        return inner.split('|').all(|part| {
            let p = part.trim();
            !p.is_empty() && is_valid_php_type_hint(p)
        });
    }

    // Intersection types: `Type1&Type2`
    if inner.contains('&') {
        return inner.split('&').all(|part| {
            let p = part.trim();
            !p.is_empty() && is_valid_php_type_hint(p)
        });
    }

    // Class names: starts with uppercase letter or backslash.
    if inner.starts_with('\\') || inner.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        // Must be a valid identifier sequence.
        return inner.split('\\').filter(|s| !s.is_empty()).all(|s| {
            s.chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        });
    }

    false
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Enclosing return type resolution ────────────────────────────

    #[test]
    fn resolve_return_type_standalone_function() {
        let php = "<?php\nfunction classify(int $code): string\n{\n    if ($code < 0) return 'negative';\n    return 'ok';\n}\n";
        let offset = php.find("if ($code").unwrap() as u32;
        let result = resolve_enclosing_return_type(php, offset);
        assert_eq!(
            result, "string",
            "should resolve enclosing function return type"
        );
    }

    #[test]
    fn resolve_return_type_method() {
        let php = "<?php\nclass Foo {\n    public function bar(): int\n    {\n        return 42;\n    }\n}\n";
        let offset = php.find("return 42").unwrap() as u32;
        let result = resolve_enclosing_return_type(php, offset);
        assert_eq!(result, "int", "should resolve enclosing method return type");
    }

    // ── Statement boundary validation ───────────────────────────────

    #[test]
    fn complete_statements_single() {
        let php = "<?php\nfunction foo() {\n    $x = 1;\n    $y = 2;\n}\n";
        // Select `$x = 1;`
        let start = php.find("$x = 1;").unwrap();
        let end = start + "$x = 1;".len();
        assert!(selection_covers_complete_statements(php, start, end));
    }

    #[test]
    fn complete_statements_multiple() {
        let php = "<?php\nfunction foo() {\n    $x = 1;\n    $y = 2;\n    $z = 3;\n}\n";
        let start = php.find("$x = 1;").unwrap();
        let end = php.find("$y = 2;").unwrap() + "$y = 2;".len();
        assert!(selection_covers_complete_statements(php, start, end));
    }

    #[test]
    fn incomplete_statement_rejected() {
        let php = "<?php\nfunction foo() {\n    $x = 1;\n}\n";
        // Select just `$x = ` (incomplete).
        let start = php.find("$x = 1;").unwrap();
        let end = start + "$x =".len();
        assert!(!selection_covers_complete_statements(php, start, end));
    }

    #[test]
    fn partial_if_rejected() {
        let php = "<?php\nfunction foo() {\n    if ($x) {\n        $y = 1;\n    }\n}\n";
        // Select just the body of the if without the if itself.
        let start = php.find("$y = 1;").unwrap();
        let end = start + "$y = 1;".len();
        // This is inside the if body — those ARE complete statements
        // within the if block, but they're not top-level statements in
        // the function body.  The validator checks against the function
        // body's direct children, so this should fail.
        assert!(!selection_covers_complete_statements(php, start, end));
    }

    #[test]
    fn complete_if_accepted() {
        let php =
            "<?php\nfunction foo() {\n    if ($x) {\n        $y = 1;\n    }\n    $z = 2;\n}\n";
        // Select the entire if statement.
        let start = php.find("if ($x)").unwrap();
        let end = php.find("    }\n    $z").unwrap() + "    }".len();
        assert!(selection_covers_complete_statements(php, start, end));
    }

    // ── Selection trimming ──────────────────────────────────────────

    #[test]
    fn trim_whitespace() {
        let content = "  hello world  ";
        let result = trim_selection(content, 0, content.len());
        assert_eq!(result, Some((2, 13)));
    }

    #[test]
    fn trim_empty_rejected() {
        let content = "   ";
        assert_eq!(trim_selection(content, 0, content.len()), None);
    }

    // ── Return detection ────────────────────────────────────────────

    #[test]
    fn detects_trailing_return() {
        let php = "<?php\nfunction foo() {\n    $x = 1;\n    return $x;\n}\n";
        let start = php.find("$x = 1;").unwrap();
        let end = php.find("return $x;").unwrap() + "return $x;".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::TrailingReturn);
    }

    #[test]
    fn detects_unsafe_return_without_trailing() {
        // `return 1;` followed by `$x = 2;` — the return doesn't end
        // the selection, and the values are mixed (not guard clauses),
        // so this can use sentinel-null (1 is not null).
        let php = "<?php\nfunction foo() {\n    return 1;\n    $x = 2;\n}\n";
        let start = php.find("return 1;").unwrap();
        let end = php.find("$x = 2;").unwrap() + "$x = 2;".len();
        let strategy = analyse_returns(php, start, end, 0);
        // `$x = 2;` is NOT a return, but there IS a return in the
        // selection that doesn't end it.  The only return value is `1`
        // → uniform guards with value "1".
        assert_eq!(
            strategy,
            ReturnStrategy::UniformGuards("1".to_string()),
            "single non-null return value should use uniform guards"
        );
    }

    #[test]
    fn no_false_positive_on_return_in_identifier() {
        let php = "<?php\nfunction foo() {\n    $returnValue = 1;\n}\n";
        let start = php.find("$returnValue").unwrap();
        let end = start + "$returnValue = 1;".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::None);
    }

    #[test]
    fn nested_return_safe_when_trailing_return_present() {
        // Guard clause pattern: `if (!$x) return 0;` followed by
        // a trailing `return $result;`.  Since the selection ends
        // with return, ALL returns are safe (call site will be
        // `return extracted(…)`).
        let php = "<?php\nfunction foo($x) {\n    if (!$x) return 0;\n    $r = $x * 2;\n    return $r;\n}\n";
        let start = php.find("if (!$x)").unwrap();
        let end = php.find("return $r;").unwrap() + "return $r;".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::TrailingReturn);
    }

    #[test]
    fn nested_return_unsafe_without_trailing_return() {
        // Return inside an if, but the selection does NOT end with return.
        // The return value is `1` (not null) → uses sentinel-null since
        // there are no modified variables.
        let php = "<?php\nfunction foo($x) {\n    if ($x) {\n        return 1;\n    }\n    echo 'done';\n}\n";
        let start = php.find("if ($x)").unwrap();
        let end = php.find("echo 'done';").unwrap() + "echo 'done';".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(
            strategy,
            ReturnStrategy::UniformGuards("1".to_string()),
            "single non-null return should use uniform guards"
        );
    }

    // ── Guard return strategies ─────────────────────────────────────

    #[test]
    fn void_guards_strategy() {
        // All returns are bare `return;` → VoidGuards.
        let php = "<?php\nfunction foo($x, $y) {\n    if (!$x) return;\n    if (!$y) return;\n    echo 'ok';\n}\n";
        let start = php.find("if (!$x)").unwrap();
        let end = php.find("echo 'ok';").unwrap() + "echo 'ok';".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::VoidGuards);
    }

    #[test]
    fn uniform_false_guards_strategy() {
        // All returns are `return false;` → UniformGuards("false").
        let php = "<?php\nfunction foo($x, $y) {\n    if (!$x) return false;\n    if (!$y) return false;\n    echo 'ok';\n}\n";
        let start = php.find("if (!$x)").unwrap();
        let end = php.find("echo 'ok';").unwrap() + "echo 'ok';".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::UniformGuards("false".to_string()));
    }

    #[test]
    fn uniform_null_guards_strategy() {
        // All returns are `return null;` → UniformGuards("null").
        // This works because the bool-flag approach doesn't need null
        // as a sentinel.
        let php = "<?php\nfunction foo($id) {\n    if ($id <= 0) return null;\n    if (!$this->exists($id)) return null;\n    echo 'ok';\n}\n";
        let start = php.find("if ($id").unwrap();
        let end = php.find("echo 'ok';").unwrap() + "echo 'ok';".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::UniformGuards("null".to_string()));
    }

    #[test]
    fn sentinel_null_strategy() {
        // Different non-null return values → SentinelNull.
        let php = "<?php\nfunction foo($x) {\n    if ($x < 0) return 'negative';\n    if ($x > 100) return 'overflow';\n    echo 'ok';\n}\n";
        let start = php.find("if ($x < 0)").unwrap();
        let end = php.find("echo 'ok';").unwrap() + "echo 'ok';".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::SentinelNull);
    }

    #[test]
    fn mixed_null_and_other_values_is_unsafe() {
        // Returns include null AND other values → Unsafe (can't use
        // null as sentinel when null is also a valid return).
        let php = "<?php\nfunction foo($x) {\n    if ($x < 0) return null;\n    if ($x > 100) return 'overflow';\n    echo 'ok';\n}\n";
        let start = php.find("if ($x < 0)").unwrap();
        let end = php.find("echo 'ok';").unwrap() + "echo 'ok';".len();
        let strategy = analyse_returns(php, start, end, 0);
        assert_eq!(strategy, ReturnStrategy::Unsafe);
    }

    #[test]
    fn guard_with_return_values_is_unsafe() {
        // Selection has return values (modified variables read after
        // the selection) — can't use guard strategies unless all
        // guards return null and there's exactly 1 return value.
        let php = "<?php\nfunction foo($x) {\n    if (!$x) return false;\n    echo 'ok';\n}\n";
        let start = php.find("if (!$x)").unwrap();
        let end = php.find("echo 'ok';").unwrap() + "echo 'ok';".len();
        let strategy = analyse_returns(php, start, end, 1);
        assert_eq!(strategy, ReturnStrategy::Unsafe);
    }

    #[test]
    fn guard_with_multiple_return_values_is_unsafe() {
        // More than 1 return value — even null guards can't help.
        let php =
            "<?php\nfunction foo($x) {\n    if (!$x) return null;\n    $a = 1;\n    $b = 2;\n}\n";
        let start = php.find("if (!$x)").unwrap();
        let end = php.find("$b = 2;").unwrap() + "$b = 2;".len();
        let strategy = analyse_returns(php, start, end, 2);
        assert_eq!(strategy, ReturnStrategy::Unsafe);
    }

    #[test]
    fn null_guard_with_single_return_value() {
        // All guards return null, exactly 1 return value →
        // NullGuardWithValue(false).
        let php = "<?php\nfunction foo($obj) {\n    if (!$obj) return null;\n    $val = $obj->compute();\n}\n";
        let start = php.find("if (!$obj)").unwrap();
        let end = php.find("$val = $obj->compute();").unwrap() + "$val = $obj->compute();".len();
        let strategy = analyse_returns(php, start, end, 1);
        assert_eq!(strategy, ReturnStrategy::NullGuardWithValue(false));
    }

    #[test]
    fn void_guard_with_single_return_value() {
        // All guards are bare `return;`, exactly 1 return value →
        // NullGuardWithValue(true).
        let php =
            "<?php\nfunction foo($obj) {\n    if (!$obj) return;\n    $val = $obj->compute();\n}\n";
        let start = php.find("if (!$obj)").unwrap();
        let end = php.find("$val = $obj->compute();").unwrap() + "$val = $obj->compute();".len();
        let strategy = analyse_returns(php, start, end, 1);
        assert_eq!(strategy, ReturnStrategy::NullGuardWithValue(true));
    }

    #[test]
    fn non_null_guard_with_return_value_is_unsafe() {
        // Guards return false (not null) with a return value — can't
        // use NullGuardWithValue, and other strategies can't handle
        // return values.
        let php = "<?php\nfunction foo($obj) {\n    if (!$obj) return false;\n    $val = $obj->compute();\n}\n";
        let start = php.find("if (!$obj)").unwrap();
        let end = php.find("$val = $obj->compute();").unwrap() + "$val = $obj->compute();".len();
        let strategy = analyse_returns(php, start, end, 1);
        assert_eq!(strategy, ReturnStrategy::Unsafe);
    }

    // ── Type hint validation ────────────────────────────────────────

    #[test]
    fn valid_scalar_hints() {
        assert!(is_valid_php_type_hint("int"));
        assert!(is_valid_php_type_hint("string"));
        assert!(is_valid_php_type_hint("bool"));
        assert!(is_valid_php_type_hint("float"));
        assert!(is_valid_php_type_hint("array"));
        assert!(is_valid_php_type_hint("void"));
        assert!(is_valid_php_type_hint("mixed"));
    }

    #[test]
    fn valid_nullable_hints() {
        assert!(is_valid_php_type_hint("?int"));
        assert!(is_valid_php_type_hint("?string"));
    }

    #[test]
    fn valid_class_hints() {
        assert!(is_valid_php_type_hint("Foo"));
        assert!(is_valid_php_type_hint("\\App\\Models\\User"));
    }

    #[test]
    fn valid_union_hints() {
        assert!(is_valid_php_type_hint("int|string"));
        assert!(is_valid_php_type_hint("Foo|null"));
    }

    #[test]
    fn invalid_hints() {
        assert!(!is_valid_php_type_hint(""));
        assert!(!is_valid_php_type_hint("array<int>"));
        assert!(!is_valid_php_type_hint("123"));
    }

    #[test]
    fn strip_generics() {
        assert_eq!(strip_generics_for_hint("array<string>"), "array");
        assert_eq!(
            strip_generics_for_hint("Collection<int, string>"),
            "Collection"
        );
        assert_eq!(strip_generics_for_hint("int"), "int");
    }

    // ── Build param list ────────────────────────────────────────────

    #[test]
    fn param_list_empty() {
        assert_eq!(build_param_list(&[]), "");
    }

    #[test]
    fn param_list_untyped() {
        let params = vec![("$x".to_string(), String::new())];
        assert_eq!(build_param_list(&params), "$x");
    }

    #[test]
    fn param_list_typed() {
        let params = vec![
            ("$x".to_string(), "int".to_string()),
            ("$y".to_string(), "string".to_string()),
        ];
        assert_eq!(build_param_list(&params), "int $x, string $y");
    }

    // ── Return type ─────────────────────────────────────────────────

    #[test]
    fn return_type_void() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "void");
    }

    #[test]
    fn return_type_single() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![("$x".to_string(), "int".to_string())],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "int");
    }

    #[test]
    fn return_type_multiple() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![
                ("$x".to_string(), "int".to_string()),
                ("$y".to_string(), "string".to_string()),
            ],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "array");
    }

    #[test]
    fn return_type_trailing_return() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::TrailingReturn,
            trailing_return_type: "string".to_string(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "string");
    }

    #[test]
    fn return_type_void_guards() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::VoidGuards,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "bool");
    }

    #[test]
    fn return_type_uniform_guards() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::UniformGuards("false".to_string()),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "bool");
    }

    #[test]
    fn return_type_sentinel_null_with_type() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::SentinelNull,
            trailing_return_type: "string".to_string(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "?string");
    }

    #[test]
    fn return_type_null_guard_with_value() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![("$sound".to_string(), "string".to_string())],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::NullGuardWithValue(false),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "?string");
    }

    #[test]
    fn return_type_null_guard_with_value_already_nullable() {
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![("$val".to_string(), "?int".to_string())],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::NullGuardWithValue(false),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "?int");
    }

    #[test]
    fn return_type_void_guard_with_value() {
        // Void guards with a computed value — return type is still
        // nullable (the extracted function returns null on guard-fire).
        let info = ExtractionInfo {
            name: String::new(),
            params: vec![],
            returns: vec![("$sound".to_string(), "string".to_string())],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: String::new(),
            return_strategy: ReturnStrategy::NullGuardWithValue(true),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        assert_eq!(build_return_type(&info), "?string");
    }

    // ── Name generation ─────────────────────────────────────────────

    #[test]
    fn generates_unique_name() {
        let content = "<?php\nfunction extracted() {}\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Function,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: String::new(),
            sibling_method_names: Vec::new(),
        };
        let naming = NamingContext {
            enclosing_name: "",
            return_strategy: &ReturnStrategy::None,
            body_text: "echo 'hello';",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "extracted2");
    }

    #[test]
    fn generates_base_name_when_no_conflict() {
        let content = "<?php\nfunction foo() {}\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Function,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: String::new(),
            sibling_method_names: Vec::new(),
        };
        let naming = NamingContext {
            enclosing_name: "",
            return_strategy: &ReturnStrategy::None,
            body_text: "$x = 1;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "extracted");
    }

    #[test]
    fn name_guard_from_void_guards() {
        let content = "<?php\nclass Foo { function run() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "run".to_string(),
            sibling_method_names: vec!["run".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "run",
            return_strategy: &ReturnStrategy::VoidGuards,
            body_text: "if (!$x) return;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "runGuard");
    }

    #[test]
    fn name_guard_dedup_against_class() {
        let content = "<?php\nclass Foo { function run() {} function runGuard() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "run".to_string(),
            sibling_method_names: vec!["run".to_string(), "runGuard".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "run",
            return_strategy: &ReturnStrategy::VoidGuards,
            body_text: "if (!$x) return;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "runGuard2");
    }

    #[test]
    fn name_try_from_sentinel_null() {
        let content = "<?php\nclass Foo { function fetch() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "fetch".to_string(),
            sibling_method_names: vec!["fetch".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "fetch",
            return_strategy: &ReturnStrategy::SentinelNull,
            body_text: "return $result;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "tryFetch");
    }

    #[test]
    fn name_factory_from_trailing_return() {
        let content = "<?php\nclass Foo { function build() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "build".to_string(),
            sibling_method_names: vec!["build".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "build",
            return_strategy: &ReturnStrategy::TrailingReturn,
            body_text: "$u = new User('Alice');\nreturn $u;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "createUser");
    }

    #[test]
    fn name_render_from_pure_output() {
        let content = "<?php\nclass Foo { function show() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "show".to_string(),
            sibling_method_names: vec!["show".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "show",
            return_strategy: &ReturnStrategy::None,
            body_text: "echo $name;\necho $age;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "renderShow");
    }

    #[test]
    fn name_compute_from_single_return_var() {
        let content = "<?php\nfunction calc() {}\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Function,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "calc".to_string(),
            sibling_method_names: Vec::new(),
        };
        let naming = NamingContext {
            enclosing_name: "calc",
            return_strategy: &ReturnStrategy::None,
            body_text: "$total = $a + $b;",
            return_var_names: &["$total".to_string()],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "computeTotal");
    }

    #[test]
    fn name_method_dedup_scoped_to_class() {
        // "extracted" exists as a function elsewhere in the file, but
        // the class has no method called "extracted" → no dedup needed.
        let content = "<?php\nfunction extracted() {}\nclass Foo { function run() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 50,
            is_static: false,
            enclosing_name: String::new(),
            sibling_method_names: vec!["run".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "",
            return_strategy: &ReturnStrategy::None,
            body_text: "$x = 1;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "extracted");
    }

    #[test]
    fn name_trailing_return_with_return_type() {
        let content = "<?php\nclass Foo { function getUsers() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "getUsers".to_string(),
            sibling_method_names: vec!["getUsers".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "getUsers",
            return_strategy: &ReturnStrategy::TrailingReturn,
            body_text: "$users = query();\nreturn $users;",
            return_var_names: &[],
            trailing_return_type: "Collection",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "getCollection");
    }

    #[test]
    fn name_uniform_guards() {
        let content = "<?php\nclass Foo { function validate() {} }\n";
        let ctx = EnclosingContext {
            target: ExtractionTarget::Method,
            insert_offset: content.len(),
            body_start: 20,
            is_static: false,
            enclosing_name: "validate".to_string(),
            sibling_method_names: vec!["validate".to_string()],
        };
        let naming = NamingContext {
            enclosing_name: "validate",
            return_strategy: &ReturnStrategy::UniformGuards("false".to_string()),
            body_text: "if (!$x) return false;",
            return_var_names: &[],
            trailing_return_type: "",
        };
        let name = generate_function_name(content, &ctx, &naming);
        assert_eq!(name, "validateGuard");
    }

    // ── Call site generation ────────────────────────────────────────

    #[test]
    fn call_site_no_returns() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), "int".to_string())],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: "    ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "    ");
        assert_eq!(result, "    extracted($x);\n");
    }

    #[test]
    fn call_site_single_return() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), "int".to_string())],
            returns: vec![("$result".to_string(), "int".to_string())],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: "    ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "    ");
        assert_eq!(result, "    $result = extracted($x);\n");
    }

    #[test]
    fn call_site_multiple_returns() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![],
            returns: vec![
                ("$a".to_string(), "int".to_string()),
                ("$b".to_string(), "string".to_string()),
            ],
            body: String::new(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: "    ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "    ");
        assert_eq!(result, "    [$a, $b] = extracted();\n");
    }

    #[test]
    fn call_site_method() {
        let info = ExtractionInfo {
            name: "runGuard".to_string(),
            params: vec![("$x".to_string(), "int".to_string())],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(result, "        $this->runGuard($x);\n");
    }

    #[test]
    fn call_site_static_method() {
        let info = ExtractionInfo {
            name: "computeTotal".to_string(),
            params: vec![],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Method,
            is_static: true,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(result, "        self::computeTotal();\n");
    }

    #[test]
    fn call_site_trailing_return() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), "int".to_string())],
            returns: vec![],
            body: "return $x * 2;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::TrailingReturn,
            trailing_return_type: "int".to_string(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(result, "        return $this->extracted($x);\n");
    }

    #[test]
    fn call_site_void_guards() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::VoidGuards,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(result, "        if (!$this->extracted($x)) return;\n");
    }

    #[test]
    fn call_site_uniform_false_guards() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::UniformGuards("false".to_string()),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(result, "        if (!$this->extracted($x)) return false;\n");
    }

    #[test]
    fn call_site_sentinel_null() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: String::new(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::SentinelNull,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(
            result,
            "        $__early = $this->extracted($x);\n        if ($__early !== null) return $__early;\n"
        );
    }

    #[test]
    fn call_site_null_guard_with_value() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$obj".to_string(), String::new())],
            returns: vec![("$sound".to_string(), "string".to_string())],
            body: String::new(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::NullGuardWithValue(false),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(
            result,
            "        $sound = $this->extracted($obj);\n        if ($sound === null) return null;\n"
        );
    }

    #[test]
    fn call_site_void_guard_with_value() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$obj".to_string(), String::new())],
            returns: vec![("$sound".to_string(), "string".to_string())],
            body: String::new(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::NullGuardWithValue(true),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_call_site(&info, "        ");
        assert_eq!(
            result,
            "        $sound = $this->extracted($obj);\n        if ($sound === null) return;\n"
        );
    }

    // ── Definition generation ───────────────────────────────────────

    #[test]
    fn definition_method_no_params_void() {
        let info = ExtractionInfo {
            name: "doWork".to_string(),
            params: vec![],
            returns: vec![],
            body: "$x = 1;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains("private function doWork(): void"),
            "got: {result}"
        );
        assert!(result.contains("        $x = 1;"), "got: {result}");
    }

    #[test]
    fn definition_function_with_params_and_return() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), "int".to_string())],
            returns: vec![("$result".to_string(), "string".to_string())],
            body: "$result = strval($x);".to_string(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: "    ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains("function extracted(int $x): string"),
            "got: {result}"
        );
        assert!(result.contains("return $result;"), "got: {result}");
    }

    #[test]
    fn definition_static_method() {
        let info = ExtractionInfo {
            name: "compute".to_string(),
            params: vec![("$n".to_string(), "int".to_string())],
            returns: vec![],
            body: "echo $n;".to_string(),
            target: ExtractionTarget::Method,
            is_static: true,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains("private static function compute(int $n): void"),
            "got: {result}"
        );
    }

    #[test]
    fn definition_with_trailing_return() {
        let info = ExtractionInfo {
            name: "extracted".to_string(),
            params: vec![("$x".to_string(), "int".to_string())],
            returns: vec![],
            body: "return $x * 2;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::TrailingReturn,
            trailing_return_type: "int".to_string(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains("private function extracted(int $x): int"),
            "should carry enclosing return type: {result}"
        );
        // Body already contains the return — no extra return appended.
        assert!(
            result.contains("return $x * 2;"),
            "body should keep the return statement: {result}"
        );
        // Should not have a duplicate return.
        assert_eq!(
            result.matches("return").count(),
            1,
            "should have exactly one return: {result}"
        );
    }

    #[test]
    fn definition_void_guards_appends_return_true() {
        let info = ExtractionInfo {
            name: "validate".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: "if (!$x) return;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::VoidGuards,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains(": bool"),
            "should have bool return type: {result}"
        );
        assert!(
            result.contains("return true;"),
            "should append return true as fall-through: {result}"
        );
    }

    #[test]
    fn definition_uniform_false_guards_appends_return_true() {
        let info = ExtractionInfo {
            name: "validate".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: "if (!$x) return false;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::UniformGuards("false".to_string()),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains(": bool"),
            "should have bool return type: {result}"
        );
        assert!(
            result.contains("return true;"),
            "should append return true (inverse of false) as sentinel: {result}"
        );
    }

    #[test]
    fn definition_uniform_true_guards_appends_return_false() {
        let info = ExtractionInfo {
            name: "validate".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: "if (!$x) return true;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::UniformGuards("true".to_string()),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains("return false;"),
            "should append return false (inverse of true) as sentinel: {result}"
        );
    }

    #[test]
    fn definition_sentinel_null_appends_return_null() {
        let info = ExtractionInfo {
            name: "classify".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: "if ($x < 0) return 'negative';".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::SentinelNull,
            trailing_return_type: "string".to_string(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains(": ?string"),
            "should have nullable return type: {result}"
        );
        assert!(
            result.contains("return null;"),
            "should append return null as sentinel: {result}"
        );
    }

    #[test]
    fn definition_null_guard_with_value_appends_return_variable() {
        let info = ExtractionInfo {
            name: "getSound".to_string(),
            params: vec![],
            returns: vec![("$sound".to_string(), "string".to_string())],
            body: "if (!$this->frog) return null;\n$sound = $this->frog->speak();".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::NullGuardWithValue(false),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains(": ?string"),
            "should have nullable return type: {result}"
        );
        assert!(
            result.contains("return $sound;"),
            "should append return $sound as fall-through: {result}"
        );
        assert!(
            result.contains("return null;"),
            "should keep the guard's return null: {result}"
        );
    }

    #[test]
    fn definition_void_guard_with_value_rewrites_returns() {
        let info = ExtractionInfo {
            name: "getSound".to_string(),
            params: vec![],
            returns: vec![("$sound".to_string(), "string".to_string())],
            body: "if (!$this->frog) return;\n$sound = $this->frog->speak();".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::NullGuardWithValue(true),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains(": ?string"),
            "should have nullable return type: {result}"
        );
        assert!(
            result.contains("return $sound;"),
            "should append return $sound as fall-through: {result}"
        );
        // Bare `return;` should be rewritten to `return null;`.
        assert!(
            result.contains("return null;"),
            "void guard should be rewritten to return null: {result}"
        );
        // Should NOT contain bare `return;`.
        assert_eq!(
            result.matches("return;").count(),
            0,
            "should not contain bare return: {result}"
        );
    }

    // ── Void return rewriting ───────────────────────────────────────

    #[test]
    fn rewrite_void_returns_to_null_basic() {
        let body = "if (!$x) return;\nif (!$y) return;";
        let result = rewrite_void_returns_to_null(body);
        assert_eq!(result, "if (!$x) return null;\nif (!$y) return null;");
    }

    #[test]
    fn rewrite_void_returns_to_null_preserves_valued_returns() {
        let body = "if (!$x) return;\nreturn $result;";
        let result = rewrite_void_returns_to_null(body);
        assert_eq!(result, "if (!$x) return null;\nreturn $result;");
    }

    #[test]
    fn rewrite_void_returns_to_null_ignores_identifiers() {
        let body = "$returnValue = 1;\nif (!$x) return;";
        let result = rewrite_void_returns_to_null(body);
        assert_eq!(result, "$returnValue = 1;\nif (!$x) return null;");
    }

    // ── Guard return rewriting ──────────────────────────────────────

    #[test]
    fn rewrite_void_guards_to_false() {
        let body = "if (!$x) return;\nif (!$y) return;";
        let result = rewrite_guard_returns(body, None);
        assert_eq!(result, "if (!$x) return false;\nif (!$y) return false;");
    }

    #[test]
    fn rewrite_void_guards_preserves_non_bare_returns() {
        let body = "if (!$x) return;\nreturn $result;";
        let result = rewrite_guard_returns(body, None);
        assert_eq!(
            result, "if (!$x) return false;\nreturn $result;",
            "should only rewrite bare returns"
        );
    }

    #[test]
    fn rewrite_void_guards_ignores_return_in_identifiers() {
        let body = "$returnValue = 1;\nif (!$x) return;";
        let result = rewrite_guard_returns(body, None);
        assert_eq!(result, "$returnValue = 1;\nif (!$x) return false;");
    }

    #[test]
    fn rewrite_uniform_null_to_false() {
        let body = "if ($id <= 0) return null;\nif (!$org) return null;";
        let result = rewrite_guard_returns(body, Some("null"));
        assert_eq!(
            result,
            "if ($id <= 0) return false;\nif (!$org) return false;"
        );
    }

    #[test]
    fn rewrite_uniform_value_preserves_other_returns() {
        let body = "if ($id <= 0) return null;\nreturn $result;";
        let result = rewrite_guard_returns(body, Some("null"));
        assert_eq!(
            result, "if ($id <= 0) return false;\nreturn $result;",
            "should only rewrite matching return values"
        );
    }

    #[test]
    fn rewrite_uniform_numeric_to_false() {
        let body = "if ($x < 0) return 0;\nif ($x > 100) return 0;";
        let result = rewrite_guard_returns(body, Some("0"));
        assert_eq!(
            result,
            "if ($x < 0) return false;\nif ($x > 100) return false;"
        );
    }

    #[test]
    fn void_guards_definition_rewrites_body() {
        // End-to-end: the definition should contain `return false;`
        // for the guards and `return true;` for the fall-through.
        let info = ExtractionInfo {
            name: "validate".to_string(),
            params: vec![("$x".to_string(), String::new())],
            returns: vec![],
            body: "if (!$x) return;\nif (!$y) return;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::VoidGuards,
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains("return false;"),
            "guards should be rewritten to return false: {result}"
        );
        assert!(
            result.contains("return true;"),
            "fall-through should be return true: {result}"
        );
        // Should NOT contain bare `return;` (the original void return).
        let bare_return_count = result.matches("return;").count();
        assert_eq!(
            bare_return_count, 0,
            "should not contain bare return: {result}"
        );
    }

    #[test]
    fn uniform_null_definition_rewrites_body() {
        // `return null;` guards should become `return false;` in the
        // extracted function since the return type is bool.
        let info = ExtractionInfo {
            name: "validate".to_string(),
            params: vec![("$id".to_string(), String::new())],
            returns: vec![],
            body: "if ($id <= 0) return null;\nif (!$this->exists($id)) return null;".to_string(),
            target: ExtractionTarget::Method,
            is_static: false,
            member_indent: "    ".to_string(),
            body_indent: "        ".to_string(),
            return_strategy: ReturnStrategy::UniformGuards("null".to_string()),
            trailing_return_type: String::new(),
            docblock: String::new(),
        };
        let result = build_extracted_definition(&info);
        assert!(
            result.contains("return false;"),
            "null guards should be rewritten to return false: {result}"
        );
        assert!(
            result.contains("return true;"),
            "fall-through should be return true: {result}"
        );
        // Should NOT contain `return null;`.
        let null_return_count = result.matches("return null;").count();
        assert_eq!(
            null_return_count, 0,
            "should not contain return null: {result}"
        );
    }

    // ── Integration: code action on Backend ─────────────────────────

    #[test]
    fn extract_function_action_offered_for_complete_statements() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo() {
    $x = 1;
    $y = $x + 2;
    echo $y;
}
";
        // Select `$x = 1;\n    $y = $x + 2;`
        let start_line = 2; // `    $x = 1;`
        let end_line = 3; // `    $y = $x + 2;`

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(start_line, 4),
                end: Position::new(end_line, 16),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let extract_action = actions
            .iter()
            .find(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.starts_with("Extract function")));
        assert!(
            extract_action.is_some(),
            "should offer extract function action, got: {:?}",
            actions
                .iter()
                .map(|a| match a {
                    CodeActionOrCommand::CodeAction(ca) => ca.title.clone(),
                    CodeActionOrCommand::Command(cmd) => cmd.title.clone(),
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn extract_function_not_offered_for_empty_selection() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo() {
    $x = 1;
}
";
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 4),
                end: Position::new(2, 4), // empty selection
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let extract_actions: Vec<_> = actions
            .iter()
            .filter(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.starts_with("Extract function") || ca.title.starts_with("Extract method")))
            .collect();
        assert!(
            extract_actions.is_empty(),
            "should not offer extract for empty selection"
        );
    }

    #[test]
    fn extract_function_not_offered_for_partial_statement() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo() {
    $x = 1 + 2;
}
";
        // Select just `1 + 2` — not a complete statement.
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 9),
                end: Position::new(2, 14),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let extract_actions: Vec<_> = actions
            .iter()
            .filter(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.starts_with("Extract function") || ca.title.starts_with("Extract method")))
            .collect();
        assert!(
            extract_actions.is_empty(),
            "should not offer extract for partial statement"
        );
    }

    #[test]
    fn extract_method_offered_when_using_this() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
class Foo {
    private int $value = 0;

    public function bar() {
        $x = $this->value;
        echo $x;
    }
}
";
        // Select `$x = $this->value;\n        echo $x;`
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(5, 8),
                end: Position::new(6, 16),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let extract_method = actions
            .iter()
            .find(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.starts_with("Extract method")));
        assert!(
            extract_method.is_some(),
            "should offer extract method when $this is used, got: {:?}",
            actions
                .iter()
                .map(|a| match a {
                    CodeActionOrCommand::CodeAction(ca) => ca.title.clone(),
                    CodeActionOrCommand::Command(cmd) => cmd.title.clone(),
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn extract_function_offered_for_trailing_return() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo() {
    $x = 1;
    return $x;
}
";
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 4),
                end: Position::new(3, 14),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let extract_action = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.starts_with("Extract function") || ca.title.starts_with("Extract method"))
        });
        assert!(
            extract_action.is_some(),
            "should offer extract when return is the last selected statement"
        );
    }

    #[test]
    fn extract_function_offered_for_guard_clause_return() {
        // Non-trailing returns that form guard clauses should now be
        // offered with the appropriate guard strategy.
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo($x) {
    if ($x) {
        return 1;
    }
    echo 'done';
}
";
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 4),
                end: Position::new(5, 17),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let extract_action = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.title.starts_with("Extract function") || ca.title.starts_with("Extract method"))
        });
        assert!(
            extract_action.is_some(),
            "should offer extract for guard clause return pattern, got: {:?}",
            actions
                .iter()
                .map(|a| match a {
                    CodeActionOrCommand::CodeAction(ca) => ca.title.clone(),
                    CodeActionOrCommand::Command(cmd) => cmd.title.clone(),
                })
                .collect::<Vec<_>>()
        );
    }

    // ── Indent detection ────────────────────────────────────────────

    #[test]
    fn detect_indent_unit_spaces() {
        let content = "<?php\n    function foo() {\n        $x = 1;\n    }\n";
        assert_eq!(detect_indent_unit(content), "    ");
    }

    #[test]
    fn detect_indent_unit_tabs() {
        let content = "<?php\n\tfunction foo() {\n\t\t$x = 1;\n\t}\n";
        assert_eq!(detect_indent_unit(content), "\t");
    }

    #[test]
    fn indent_at_line() {
        let content = "<?php\n    $x = 1;\n";
        let offset = content.find("$x").unwrap();
        assert_eq!(indent_at(content, offset), "    ");
    }

    #[test]
    fn detect_line_indent_method() {
        let content =
            "<?php\nclass Foo {\n    public function bar() {\n        $x = 1;\n    }\n}\n";
        // body_start is the `{` after `bar()`
        let offset = content.find("{\n        $x").unwrap();
        assert_eq!(detect_line_indent(content, offset), "    ");
    }

    // ── Extraction context ──────────────────────────────────────────

    #[test]
    fn detects_function_context() {
        let content = "<?php\nfunction foo() {\n    $x = 1;\n}\n";
        let offset = content.find("$x").unwrap() as u32;
        let ctx = find_enclosing_context(content, offset, false);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.target, ExtractionTarget::Function);
    }

    #[test]
    fn detects_method_context() {
        let content =
            "<?php\nclass Foo {\n    public function bar() {\n        $x = 1;\n    }\n}\n";
        let offset = content.find("$x").unwrap() as u32;
        let ctx = find_enclosing_context(content, offset, false);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.target, ExtractionTarget::Method);
    }

    #[test]
    fn detects_method_context_with_this() {
        let content =
            "<?php\nclass Foo {\n    public function bar() {\n        $this->baz();\n    }\n}\n";
        let offset = content.find("$this").unwrap() as u32;
        let ctx = find_enclosing_context(content, offset, true);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.target, ExtractionTarget::Method);
    }

    // ── PHPDoc generation on extracted method ───────────────────────

    fn no_classes(_name: &str) -> Option<Arc<ClassInfo>> {
        None
    }

    #[test]
    fn docblock_not_generated_for_scalar_types() {
        let params = vec![
            ("$x".to_string(), "int".to_string(), "int".to_string()),
            ("$y".to_string(), "string".to_string(), "string".to_string()),
        ];
        let result = build_docblock_for_extraction(&params, "void", "void", "    ", &no_classes);
        assert!(
            result.is_empty(),
            "scalar types should not trigger docblock, got: {result}"
        );
    }

    #[test]
    fn docblock_generated_for_array_param() {
        let params = vec![(
            "$items".to_string(),
            "array".to_string(),
            "array".to_string(),
        )];
        let result = build_docblock_for_extraction(&params, "void", "void", "    ", &no_classes);
        assert!(
            result.contains("@param"),
            "array param should trigger @param enrichment, got: {result}"
        );
        assert!(result.contains("$items"));
        assert!(result.starts_with("    /**"));
        assert!(result.contains("     */"));
    }

    #[test]
    fn docblock_generated_for_callable_param() {
        let params = vec![(
            "$fn".to_string(),
            "Closure".to_string(),
            "Closure".to_string(),
        )];
        let result = build_docblock_for_extraction(&params, "void", "void", "    ", &no_classes);
        assert!(
            result.contains("@param"),
            "Closure param should trigger @param enrichment, got: {result}"
        );
        assert!(result.contains("$fn"));
    }

    #[test]
    fn docblock_not_generated_for_empty_types() {
        let params = vec![("$x".to_string(), String::new(), String::new())];
        let result = build_docblock_for_extraction(&params, "", "", "", &no_classes);
        assert!(
            result.is_empty(),
            "empty types should not trigger docblock, got: {result}"
        );
    }

    #[test]
    fn docblock_aligns_param_names() {
        let params = vec![
            (
                "$items".to_string(),
                "array".to_string(),
                "array".to_string(),
            ),
            (
                "$cb".to_string(),
                "Closure".to_string(),
                "Closure".to_string(),
            ),
        ];
        let result = build_docblock_for_extraction(&params, "void", "void", "", &no_classes);
        // Both @param tags should be present.
        let param_lines: Vec<&str> = result.lines().filter(|l| l.contains("@param")).collect();
        assert_eq!(
            param_lines.len(),
            2,
            "expected 2 @param lines, got: {result}"
        );
        // The $-names should be aligned (both start at the same column).
        let dollar_positions: Vec<usize> =
            param_lines.iter().map(|l| l.find('$').unwrap()).collect();
        assert_eq!(
            dollar_positions[0], dollar_positions[1],
            "param names should be aligned, got: {result}"
        );
    }

    #[test]
    fn docblock_return_type_hint_for_docblock_trailing() {
        let result =
            build_return_type_hint_for_docblock(&ReturnStrategy::TrailingReturn, "string", &[]);
        assert_eq!(result, "string");
    }

    #[test]
    fn docblock_return_type_hint_for_docblock_void_guards() {
        let result = build_return_type_hint_for_docblock(&ReturnStrategy::VoidGuards, "", &[]);
        assert_eq!(result, "bool");
    }

    #[test]
    fn docblock_return_type_hint_for_docblock_none_void() {
        let result = build_return_type_hint_for_docblock(&ReturnStrategy::None, "", &[]);
        assert_eq!(result, "void");
    }

    #[test]
    fn docblock_return_type_hint_for_docblock_single_return() {
        let returns = vec![("$x".to_string(), "array".to_string(), "array".to_string())];
        let result = build_return_type_hint_for_docblock(&ReturnStrategy::None, "", &returns);
        assert_eq!(result, "array");
    }

    #[test]
    fn definition_includes_docblock_for_array_param() {
        let info = ExtractionInfo {
            name: "process".to_string(),
            params: vec![("$items".to_string(), "array".to_string())],
            returns: vec![],
            body: "foreach ($items as $item) {}".to_string(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: "    ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: build_docblock_for_extraction(
                &[(
                    "$items".to_string(),
                    "array".to_string(),
                    "array".to_string(),
                )],
                "void",
                "void",
                "",
                &no_classes,
            ),
        };
        let def = build_extracted_definition(&info);
        assert!(
            def.contains("/**"),
            "definition should include docblock for array param, got:\n{def}"
        );
        assert!(
            def.contains("@param"),
            "definition should include @param tag, got:\n{def}"
        );
        // Docblock should appear before the function keyword.
        let doc_pos = def.find("/**").unwrap();
        let fn_pos = def.find("function").unwrap();
        assert!(doc_pos < fn_pos, "docblock should precede function keyword");
    }

    #[test]
    fn definition_no_docblock_for_scalar_params() {
        let info = ExtractionInfo {
            name: "add".to_string(),
            params: vec![
                ("$a".to_string(), "int".to_string()),
                ("$b".to_string(), "int".to_string()),
            ],
            returns: vec![("$sum".to_string(), "int".to_string())],
            body: "$sum = $a + $b;".to_string(),
            target: ExtractionTarget::Function,
            is_static: false,
            member_indent: String::new(),
            body_indent: "    ".to_string(),
            return_strategy: ReturnStrategy::None,
            trailing_return_type: String::new(),
            docblock: build_docblock_for_extraction(
                &[
                    ("$a".to_string(), "int".to_string(), "int".to_string()),
                    ("$b".to_string(), "int".to_string(), "int".to_string()),
                ],
                "int",
                "int",
                "",
                &no_classes,
            ),
        };
        let def = build_extracted_definition(&info);
        assert!(
            !def.contains("/**"),
            "definition should NOT include docblock for scalar types, got:\n{def}"
        );
    }

    // ── Disabled code action with rejection reason ──────────────────

    #[test]
    fn disabled_action_for_unsafe_returns() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo() {
    if ($a) return 1;
    if ($b) return null;
    echo 'done';
}
";
        // Select the three statements (mixed return values including
        // null → Unsafe strategy).
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 4),
                end: Position::new(4, 17),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let disabled = actions
            .iter()
            .find(|a| matches!(a, CodeActionOrCommand::CodeAction(ca) if ca.disabled.is_some()));
        assert!(
            disabled.is_some(),
            "should emit a disabled code action for unsafe returns, got: {:?}",
            actions
                .iter()
                .map(|a| match a {
                    CodeActionOrCommand::CodeAction(ca) => format!(
                        "{} (disabled: {:?})",
                        ca.title,
                        ca.disabled.as_ref().map(|d| &d.reason)
                    ),
                    CodeActionOrCommand::Command(cmd) => cmd.title.clone(),
                })
                .collect::<Vec<_>>()
        );
        if let Some(CodeActionOrCommand::CodeAction(ca)) = disabled {
            assert!(
                ca.disabled.as_ref().unwrap().reason.contains("return"),
                "reason should mention returns, got: {}",
                ca.disabled.as_ref().unwrap().reason
            );
            assert!(ca.edit.is_none(), "disabled action should have no edit");
        }
    }

    #[test]
    fn no_disabled_action_for_empty_selection() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo() {
    $x = 1;
}
";
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 4),
                end: Position::new(2, 4), // empty selection
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let disabled_extract = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca)
                if ca.disabled.is_some()
                    && ca.kind == Some(CodeActionKind::REFACTOR_EXTRACT)
                    && ca.title.contains("Extract"))
        });
        assert!(
            disabled_extract.is_none(),
            "should NOT emit a disabled extract action for empty selection"
        );
    }

    #[test]
    fn no_disabled_action_for_partial_statement() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "\
<?php
function foo() {
    $x = some_function($a, $b);
}
";
        // Select partial statement (just the function call, not the assignment).
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(2, 9),
                end: Position::new(2, 30),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
            partial_result_params: PartialResultParams {
                partial_result_token: None,
            },
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let disabled_extract = actions.iter().find(|a| {
            matches!(a, CodeActionOrCommand::CodeAction(ca)
                if ca.disabled.is_some()
                    && ca.kind == Some(CodeActionKind::REFACTOR_EXTRACT)
                    && ca.title.contains("Extract"))
        });
        assert!(
            disabled_extract.is_none(),
            "should NOT emit a disabled extract action for partial statement"
        );
    }
}
