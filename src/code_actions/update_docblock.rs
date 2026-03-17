//! Update Docblock to Match Signature code action.
//!
//! When a function or method signature changes (parameters added, removed,
//! reordered, or type hints updated), the docblock often falls out of sync.
//! This code action patches the `@param` and `@return` tags to match the
//! current signature while preserving descriptions and other tags.
//!
//! **Trigger:** Cursor is on a function/method that has an existing
//! docblock whose `@param` tags don't match the signature's parameters
//! (by name, count, or order), or whose `@return` tag contradicts the
//! return type hint.
//!
//! **Code action kind:** `quickfix`.

use std::collections::HashMap;
use std::sync::Arc;

use bumpalo::Bump;
use mago_span::HasSpan;
use mago_syntax::ast::class_like::member::ClassLikeMember;
use mago_syntax::ast::*;
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::phpdoc::generation::enrichment_plain;
use crate::completion::source::throws_analysis;
use crate::docblock::type_strings::split_type_token;
use crate::types::ClassInfo;
use crate::util::offset_to_position;

// ── Data types ──────────────────────────────────────────────────────────────

/// A parameter extracted from the function/method signature.
#[derive(Debug, Clone)]
struct SigParam {
    /// Parameter name including `$` prefix.
    name: String,
    /// Native type hint (e.g. `string`, `?int`, `Foo|Bar`), if present.
    type_hint: Option<String>,
    /// Whether the parameter is variadic (`...$args`).
    is_variadic: bool,
}

/// A `@param` tag parsed from an existing docblock.
#[derive(Debug, Clone)]
struct DocParam {
    /// The full type string from the tag.
    type_str: String,
    /// Parameter name including `$` prefix (and optional `...` prefix for variadic).
    name: String,
    /// Description text after the `$name`.
    description: String,
}

/// A `@return` tag parsed from an existing docblock.
#[derive(Debug, Clone)]
struct DocReturn {
    /// The type string from the tag.
    type_str: String,
    /// Description text after the type.
    description: String,
}

/// Information about the function/method under the cursor, including its
/// docblock position and parsed tags.
struct FunctionWithDocblock {
    /// Byte range of the docblock comment (from `/**` to `*/` inclusive).
    docblock_start: usize,
    docblock_end: usize,
    /// The raw docblock text.
    docblock_text: String,
    /// Parameters from the signature.
    sig_params: Vec<SigParam>,
    /// Return type from the signature (if any).
    sig_return: Option<String>,
    /// `@param` tags from the docblock.
    doc_params: Vec<DocParam>,
    /// `@return` tag from the docblock (if any).
    doc_return: Option<DocReturn>,
    /// `@throws` exception type names from the docblock.
    doc_throws: Vec<String>,
    /// Indentation of the docblock lines (whitespace before ` * `).
    indent: String,
    /// LSP position of the docblock start (for throws analysis).
    docblock_position: Position,
}

impl Backend {
    /// Collect "Update docblock" code actions for the function/method
    /// under the cursor.
    pub(crate) fn collect_update_docblock_actions(
        &self,
        uri: &str,
        content: &str,
        params: &CodeActionParams,
        out: &mut Vec<CodeActionOrCommand>,
    ) {
        let doc_uri: Url = match uri.parse() {
            Ok(u) => u,
            Err(_) => return,
        };

        let cursor_offset = position_to_offset_ud(content, params.range.start);

        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = mago_syntax::parser::parse_file_content(&arena, file_id, content);

        let info = match find_function_with_docblock(
            &program.statements,
            program.trivia.as_slice(),
            content,
            cursor_offset,
        ) {
            Some(info) => info,
            None => return,
        };

        // Build a class loader for type enrichment.
        let ctx = self.file_context(uri);
        let class_loader = self.class_loader(&ctx);

        // Determine if anything needs updating.
        let needs_update = check_needs_update(&info, content, &class_loader);
        if !needs_update {
            return;
        }

        // Build the replacement docblock.
        let new_docblock = build_updated_docblock(&info, content, &class_loader);
        if new_docblock == info.docblock_text {
            return;
        }

        let start_pos = offset_to_position(content, info.docblock_start);
        let end_pos = offset_to_position(content, info.docblock_end);

        let mut changes = HashMap::new();
        changes.insert(
            doc_uri,
            vec![TextEdit {
                range: Range {
                    start: start_pos,
                    end: end_pos,
                },
                new_text: new_docblock,
            }],
        );

        out.push(CodeActionOrCommand::CodeAction(CodeAction {
            title: "Update docblock to match signature".to_string(),
            kind: Some(CodeActionKind::QUICKFIX),
            diagnostics: None,
            edit: Some(WorkspaceEdit {
                changes: Some(changes),
                document_changes: None,
                change_annotations: None,
            }),
            command: None,
            is_preferred: Some(true),
            disabled: None,
            data: None,
        }));
    }
}

// ── AST walk ────────────────────────────────────────────────────────────────

/// Walk the AST to find a function/method at the cursor position that has
/// an existing docblock.
fn find_function_with_docblock<'a>(
    statements: &Sequence<'a, Statement<'a>>,
    trivia: &[Trivia<'a>],
    content: &str,
    cursor: u32,
) -> Option<FunctionWithDocblock> {
    for stmt in statements.iter() {
        if let Some(info) = find_in_statement_ud(stmt, trivia, content, cursor) {
            return Some(info);
        }
    }
    None
}

/// Check whether the cursor is inside the AST node span or inside the
/// docblock trivia that immediately precedes it.
fn cursor_in_node_or_docblock(
    cursor: u32,
    node_start: u32,
    node_end: u32,
    trivia: &[Trivia<'_>],
    content: &str,
) -> bool {
    if cursor >= node_start && cursor <= node_end {
        return true;
    }
    // Check if the cursor is inside the docblock that belongs to this node.
    if let Some(db_start) = find_docblock_start_for_node(node_start, trivia, content)
        && cursor >= db_start
        && cursor < node_start
    {
        return true;
    }
    false
}

/// Find the start offset of the docblock trivia immediately before a node,
/// if one exists.
fn find_docblock_start_for_node(
    node_start: u32,
    trivia: &[Trivia<'_>],
    content: &str,
) -> Option<u32> {
    let candidate_idx = trivia.partition_point(|t| t.span.start.offset < node_start);
    if candidate_idx == 0 {
        return None;
    }

    let content_bytes = content.as_bytes();
    let mut covered_from = node_start;

    for i in (0..candidate_idx).rev() {
        let t = &trivia[i];
        let t_end = t.span.end.offset;

        let gap = content_bytes
            .get(t_end as usize..covered_from as usize)
            .unwrap_or(&[]);
        if !gap.iter().all(u8::is_ascii_whitespace) {
            break;
        }

        match t.kind {
            TriviaKind::DocBlockComment => {
                return Some(t.span.start.offset);
            }
            TriviaKind::WhiteSpace
            | TriviaKind::SingleLineComment
            | TriviaKind::MultiLineComment
            | TriviaKind::HashComment => {
                covered_from = t.span.start.offset;
            }
        }
    }

    None
}

fn find_in_statement_ud<'a>(
    stmt: &Statement<'a>,
    trivia: &[Trivia<'a>],
    content: &str,
    cursor: u32,
) -> Option<FunctionWithDocblock> {
    match stmt {
        Statement::Namespace(ns) => {
            for s in ns.statements().iter() {
                if let Some(info) = find_in_statement_ud(s, trivia, content, cursor) {
                    return Some(info);
                }
            }
        }
        Statement::Function(func) => {
            let span = func.span();
            if cursor_in_node_or_docblock(
                cursor,
                span.start.offset,
                span.end.offset,
                trivia,
                content,
            ) {
                return build_info_for_function_like(
                    span.start.offset,
                    &func.parameter_list,
                    func.return_type_hint.as_ref(),
                    trivia,
                    content,
                );
            }
        }
        Statement::Class(class) => {
            let span = class.span();
            if cursor >= span.start.offset && cursor <= span.end.offset {
                return find_in_class_members(class.members.iter(), trivia, content, cursor);
            }
        }
        Statement::Interface(iface) => {
            let span = iface.span();
            if cursor >= span.start.offset && cursor <= span.end.offset {
                return find_in_class_members(iface.members.iter(), trivia, content, cursor);
            }
        }
        Statement::Trait(tr) => {
            let span = tr.span();
            if cursor >= span.start.offset && cursor <= span.end.offset {
                return find_in_class_members(tr.members.iter(), trivia, content, cursor);
            }
        }
        Statement::Enum(en) => {
            let span = en.span();
            if cursor >= span.start.offset && cursor <= span.end.offset {
                return find_in_class_members(en.members.iter(), trivia, content, cursor);
            }
        }
        _ => {}
    }
    None
}

fn find_in_class_members<'a>(
    members: impl Iterator<Item = &'a ClassLikeMember<'a>>,
    trivia: &[Trivia<'a>],
    content: &str,
    cursor: u32,
) -> Option<FunctionWithDocblock> {
    for member in members {
        if let ClassLikeMember::Method(method) = member {
            let span = method.span();
            if cursor_in_node_or_docblock(
                cursor,
                span.start.offset,
                span.end.offset,
                trivia,
                content,
            ) {
                return build_info_for_function_like(
                    span.start.offset,
                    &method.parameter_list,
                    method.return_type_hint.as_ref(),
                    trivia,
                    content,
                );
            }
        }
    }
    None
}

/// Extract the hint string from a type hint node.
fn extract_hint_string_local(hint: &Hint<'_>, content: &str) -> String {
    let span = hint.span();
    let start = span.start.offset as usize;
    let end = span.end.offset as usize;
    content.get(start..end).unwrap_or("").to_string()
}

/// Build a `FunctionWithDocblock` from a function-like AST node.
fn build_info_for_function_like<'a>(
    node_start: u32,
    param_list: &function_like::parameter::FunctionLikeParameterList<'a>,
    return_type_hint: Option<&function_like::r#return::FunctionLikeReturnTypeHint<'a>>,
    trivia: &[Trivia<'a>],
    content: &str,
) -> Option<FunctionWithDocblock> {
    // Find the docblock trivia immediately before this node.
    let candidate_idx = trivia.partition_point(|t| t.span.start.offset < node_start);
    if candidate_idx == 0 {
        return None;
    }

    let content_bytes = content.as_bytes();
    let mut covered_from = node_start;

    let mut docblock_trivia = None;
    for i in (0..candidate_idx).rev() {
        let t = &trivia[i];
        let t_end = t.span.end.offset;

        let gap = content_bytes
            .get(t_end as usize..covered_from as usize)
            .unwrap_or(&[]);
        if !gap.iter().all(u8::is_ascii_whitespace) {
            break;
        }

        match t.kind {
            TriviaKind::DocBlockComment => {
                docblock_trivia = Some(t);
                break;
            }
            TriviaKind::WhiteSpace
            | TriviaKind::SingleLineComment
            | TriviaKind::MultiLineComment
            | TriviaKind::HashComment => {
                covered_from = t.span.start.offset;
            }
        }
    }

    let trivia_node = docblock_trivia?;
    let docblock_start = trivia_node.span.start.offset as usize;
    let docblock_end = trivia_node.span.end.offset as usize;
    let docblock_text = content.get(docblock_start..docblock_end)?.to_string();

    // Extract signature parameters.
    let sig_params: Vec<SigParam> = param_list
        .parameters
        .iter()
        .map(|p| {
            let name = p.variable.name.to_string();
            let type_hint = p
                .hint
                .as_ref()
                .map(|h| extract_hint_string_local(h, content));
            let is_variadic = p.ellipsis.is_some();
            SigParam {
                name,
                type_hint,
                is_variadic,
            }
        })
        .collect();

    // Extract return type.
    let sig_return = return_type_hint.map(|rth| extract_hint_string_local(&rth.hint, content));

    // Parse existing docblock tags.
    let doc_params = parse_doc_params(&docblock_text, docblock_start);
    let doc_return = parse_doc_return(&docblock_text, docblock_start);
    let doc_throws = parse_doc_throws(&docblock_text);

    // Detect indentation.
    let indent = detect_indent(content, docblock_start);

    // Compute LSP position for throws analysis.
    let docblock_position = offset_to_position(content, docblock_start);

    Some(FunctionWithDocblock {
        docblock_start,
        docblock_end,
        docblock_text,
        sig_params,
        sig_return,
        doc_params,
        doc_return,
        doc_throws,
        indent,
        docblock_position,
    })
}

// ── Docblock parsing ────────────────────────────────────────────────────────

/// Parse all `@param` tags from a docblock.
fn parse_doc_params(docblock: &str, _base_offset: usize) -> Vec<DocParam> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();
    let lines: Vec<&str> = inner.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim_start();

            if rest.is_empty() {
                i += 1;
                continue;
            }

            // When the first token starts with `$` (or `...$` for variadic),
            // there is no type — the token is the parameter name directly.
            let first_token = rest.split_whitespace().next().unwrap_or("");
            let is_name_first = first_token.starts_with('$') || first_token.starts_with("...$");

            let (type_str, name_token, after_params) = if is_name_first {
                ("", first_token, &rest[first_token.len()..])
            } else {
                // Extract type token.
                let (type_str, remainder) = split_type_token(rest);
                let remainder = remainder.trim_start();

                // Extract parameter name.
                let name_token = remainder.split_whitespace().next().unwrap_or("");
                let after_params = remainder.get(name_token.len()..).unwrap_or("");
                (type_str, name_token, after_params)
            };

            if name_token.is_empty() || (!name_token.contains('$')) {
                i += 1;
                continue;
            }

            let name = name_token.to_string();

            // Extract description (rest of line after name).
            let after_name = after_params.trim_start();
            let mut description = after_name.to_string();

            // Collect continuation lines.
            let mut j = i + 1;
            while j < lines.len() {
                let cont = lines[j].trim().trim_start_matches('*').trim();
                if cont.is_empty() || cont.starts_with('@') {
                    break;
                }
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str(cont);
                j += 1;
            }

            results.push(DocParam {
                type_str: type_str.to_string(),
                name,
                description,
            });

            i = j;
        } else {
            i += 1;
        }
    }

    results
}

/// Parse the `@return` tag from a docblock.
fn parse_doc_return(docblock: &str, _base_offset: usize) -> Option<DocReturn> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let lines: Vec<&str> = inner.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@return") {
            let rest = rest.trim_start();

            if rest.is_empty() {
                i += 1;
                continue;
            }

            // Skip conditional return types.
            if rest.starts_with('(') {
                i += 1;
                continue;
            }

            let (type_str, remainder) = split_type_token(rest);
            let description = remainder.trim().to_string();

            return Some(DocReturn {
                type_str: type_str.to_string(),
                description,
            });
        }
        i += 1;
    }

    None
}

/// Parse `@throws` tags from a docblock, returning the exception type names.
fn parse_doc_throws(docblock: &str) -> Vec<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();
    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("@throws") {
            let rest = rest.trim_start();
            if let Some(type_name) = rest.split_whitespace().next()
                && !type_name.is_empty()
            {
                results.push(type_name.to_string());
            }
        }
    }
    results
}

/// Detect the indentation prefix from the source at the docblock position.
fn detect_indent(content: &str, docblock_start: usize) -> String {
    // Walk backward from docblock_start to find the line start.
    let before = &content[..docblock_start];
    let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let prefix = &content[line_start..docblock_start];
    // The indent is just whitespace.
    prefix.chars().take_while(|c| c.is_whitespace()).collect()
}

// ── Diff and update logic ───────────────────────────────────────────────────

/// Check whether the docblock needs updating.
fn check_needs_update(
    info: &FunctionWithDocblock,
    content: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    // Build a map of existing doc param names.
    let doc_param_names: Vec<&str> = info
        .doc_params
        .iter()
        .map(|p| {
            let n = p.name.as_str();
            n.strip_prefix("...").unwrap_or(n)
        })
        .collect();

    let sig_param_names: Vec<String> = info.sig_params.iter().map(|p| p.name.clone()).collect();

    // Check for missing, extra, or reordered params.
    if doc_param_names.len() != sig_param_names.len() {
        return true;
    }

    for (doc_name, sig_name) in doc_param_names.iter().zip(sig_param_names.iter()) {
        if *doc_name != sig_name.as_str() {
            return true;
        }
    }

    // Check for type contradictions in @param tags.
    for sig_param in &info.sig_params {
        if let Some(native_type) = &sig_param.type_hint
            && let Some(doc_param) = info.doc_params.iter().find(|dp| {
                let n = dp.name.as_str();
                let n = n.strip_prefix("...").unwrap_or(n);
                n == sig_param.name
            })
            && is_type_contradiction(&doc_param.type_str, native_type)
        {
            return true;
        }
    }

    // Check whether any existing @param type needs enrichment (e.g. a bare
    // `Closure` that should become `(Closure(): mixed)`, or a class with templates).
    // Skip when the doc type is already more specific (contains `<` or `(`).
    for sig_param in &info.sig_params {
        if let Some(doc_param) = info.doc_params.iter().find(|dp| {
            let n = dp.name.as_str();
            let n = n.strip_prefix("...").unwrap_or(n);
            n == sig_param.name
        }) {
            // If the doc type already carries generic params or a callable
            // signature, it is already enriched — no update needed.
            if doc_param.type_str.contains('<')
                || doc_param.type_str.contains('(')
                || doc_param.type_str.contains('{')
            {
                continue;
            }
            if let Some(enriched) = enrichment_plain(&sig_param.type_hint, class_loader)
                && enriched != doc_param.type_str
            {
                return true;
            }
        }
    }

    // Check @return tag.
    if let Some(sig_ret) = &info.sig_return
        && let Some(doc_ret) = &info.doc_return
    {
        // Remove `@return void` if the signature also has `: void`.
        let sig_lower = sig_ret.to_lowercase();
        let doc_lower = doc_ret.type_str.to_lowercase();
        if sig_lower == "void" && doc_lower == "void" {
            return true;
        }
        if is_type_contradiction(&doc_ret.type_str, sig_ret) {
            return true;
        }
    }

    // Check for missing @throws tags.
    let uncaught = throws_analysis::find_uncaught_throw_types(content, info.docblock_position);
    let existing_lower: Vec<String> = info
        .doc_throws
        .iter()
        .map(|t| {
            t.trim_start_matches('\\')
                .rsplit('\\')
                .next()
                .unwrap_or(t)
                .to_lowercase()
        })
        .collect();
    for exc in &uncaught {
        let short = exc
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc);
        if !existing_lower.contains(&short.to_lowercase()) {
            return true;
        }
    }

    false
}

/// Check if a docblock type contradicts a native type hint.
///
/// A contradiction means the docblock type is NOT a refinement of the native
/// type. For example, docblock says `string` but native says `int` is a
/// contradiction. But docblock says `non-empty-string` while native says
/// `string` is a refinement (not a contradiction).
fn is_type_contradiction(doc_type: &str, native_type: &str) -> bool {
    let doc_clean = normalize_type_for_comparison(doc_type);
    let native_clean = normalize_type_for_comparison(native_type);

    if doc_clean == native_clean {
        return false;
    }

    // If the docblock type carries generics or callable signatures, it's
    // likely a refinement.
    if doc_type.contains('<') || doc_type.contains('{') || doc_type.contains('(') {
        // Check that the base type is compatible.
        let doc_base = base_type(doc_type).to_lowercase();
        let native_base = base_type(native_type).to_lowercase();
        if doc_base == native_base {
            return false;
        }
        // `list<X>` refines `array`.
        if native_base == "array" && (doc_base == "list" || doc_base == "array") {
            return false;
        }
    }

    // PHPStan-style type refinements like `non-empty-string`, `positive-int`,
    // `class-string`, `literal-string`, etc. are refinements, not contradictions.
    let doc_lower = doc_type.to_lowercase();
    let native_lower = native_type.to_lowercase();
    if is_refinement_of(&doc_lower, &native_lower) {
        return false;
    }

    // If the doc type contains `|` and the native type doesn't, it might be
    // a broader docblock type — that's also a contradiction.
    // If the native type contains `|`, compare the union components.

    // Simple heuristic: normalize both and compare base types.
    let doc_bases = split_union_types(&doc_clean);
    let native_bases = split_union_types(&native_clean);

    // If every native base appears in doc bases (or a refinement thereof),
    // it's not a contradiction.
    // For simplicity, if the base types are completely different, it's a
    // contradiction.
    if doc_bases.len() == 1 && native_bases.len() == 1 {
        let db = &doc_bases[0];
        let nb = &native_bases[0];
        if db != nb && !is_refinement_of(db, nb) {
            return true;
        }
    }

    false
}

/// Normalize a type string for comparison: strip leading `\`, lowercase,
/// normalize nullable.
fn normalize_type_for_comparison(t: &str) -> String {
    let t = t.strip_prefix('\\').unwrap_or(t);
    let t = t.strip_prefix('?').map_or_else(
        || t.to_lowercase(),
        |rest| format!("{}|null", rest.to_lowercase()),
    );
    // Sort union components.
    let mut parts: Vec<&str> = t.split('|').map(|s| s.trim()).collect();
    parts.sort();
    parts.join("|")
}

/// Get the base type (before `<` or `{`).
fn base_type(t: &str) -> &str {
    let t = t.strip_prefix('\\').unwrap_or(t);
    let t = t.strip_prefix('?').unwrap_or(t);
    if let Some(pos) = t.find('<') {
        &t[..pos]
    } else if let Some(pos) = t.find('{') {
        &t[..pos]
    } else {
        t
    }
}

/// Check if `doc_type` is a known refinement of `native_type`.
fn is_refinement_of(doc_type: &str, native_type: &str) -> bool {
    let refinements: &[(&str, &str)] = &[
        ("non-empty-string", "string"),
        ("non-falsy-string", "string"),
        ("numeric-string", "string"),
        ("literal-string", "string"),
        ("class-string", "string"),
        ("callable-string", "string"),
        ("truthy-string", "string"),
        ("positive-int", "int"),
        ("negative-int", "int"),
        ("non-negative-int", "int"),
        ("non-positive-int", "int"),
        ("int-mask", "int"),
        ("non-empty-array", "array"),
        ("non-empty-list", "array"),
        ("list", "array"),
    ];
    for &(refined, base) in refinements {
        // Handle both `doc_type` directly matching and `doc_type` having
        // generic params (e.g. `class-string<foo>`).
        let doc_base = if let Some(pos) = doc_type.find('<') {
            &doc_type[..pos]
        } else {
            doc_type
        };
        if doc_base == refined && native_type == base {
            return true;
        }
    }
    false
}

/// Split a union type string into its components.
fn split_union_types(t: &str) -> Vec<String> {
    t.split('|')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build the updated docblock text.
fn build_updated_docblock(
    info: &FunctionWithDocblock,
    content: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let indent = &info.indent;

    // Parse the existing docblock into lines, categorizing each line.
    let mut lines = parse_docblock_lines(&info.docblock_text);

    // Remove existing @param lines.
    lines.retain(|l| !matches!(l, DocLine::Param(_)));

    // Clean up orphaned empty lines left after removing @param lines.
    // Remove Empty lines that directly follow Open (no summary text).
    while lines.len() >= 2
        && matches!(lines[0], DocLine::Open)
        && matches!(lines[1], DocLine::Empty)
        && lines.get(2).is_some_and(|l| !matches!(l, DocLine::Text(_)))
    {
        lines.remove(1);
    }

    // Remove @return if it's redundant (void) or contradicted.
    let should_remove_return = should_remove_return(info);
    let should_update_return = should_update_return(info);
    if should_remove_return {
        lines.retain(|l| !matches!(l, DocLine::Return(_)));
    }

    // Find where to insert new @param lines.
    // Prefer inserting before the first @return or @throws, or at the end
    // before the closing `*/`.
    let insert_pos = find_param_insert_position(&lines);

    // Build new @param entries: (type_str, name_with_prefix, description).
    let param_entries: Vec<(String, String, String)> = info
        .sig_params
        .iter()
        .map(|sig| {
            // Try to preserve the existing description for this param.
            let existing = info.doc_params.iter().find(|dp| {
                let n = dp.name.as_str();
                let n = n.strip_prefix("...").unwrap_or(n);
                n == sig.name
            });

            let type_str = if let Some(existing) = existing {
                // If the existing type is a refinement, keep it.
                if let Some(native) = &sig.type_hint {
                    if is_type_contradiction(&existing.type_str, native) {
                        // Type is contradicted — try enrichment first, fall
                        // back to the raw native hint.
                        enrichment_plain(&sig.type_hint, class_loader)
                            .unwrap_or_else(|| native.clone())
                    } else if existing.type_str.contains('<')
                        || existing.type_str.contains('(')
                        || existing.type_str.contains('{')
                    {
                        // Doc already has generics / callable / shape — keep it.
                        existing.type_str.clone()
                    } else {
                        // Check if enrichment would upgrade the type (e.g.
                        // bare `Closure` → `(Closure(): mixed)`).
                        if let Some(enriched) = enrichment_plain(&sig.type_hint, class_loader) {
                            if enriched != existing.type_str {
                                enriched
                            } else {
                                existing.type_str.clone()
                            }
                        } else {
                            existing.type_str.clone()
                        }
                    }
                } else {
                    existing.type_str.clone()
                }
            } else {
                // New param — use enrichment or fall back to raw hint / mixed.
                enrichment_plain(&sig.type_hint, class_loader)
                    .unwrap_or_else(|| sig.type_hint.clone().unwrap_or_else(|| "mixed".to_string()))
            };

            let description = existing.map(|e| e.description.clone()).unwrap_or_default();

            let name_prefix = if sig.is_variadic { "..." } else { "" };
            let full_name = format!("{}{}", name_prefix, sig.name);

            (type_str, full_name, description)
        })
        .collect();

    // Compute max type width for column alignment.
    let max_type_len = param_entries
        .iter()
        .map(|(t, _, _)| t.len())
        .max()
        .unwrap_or(0);

    // Build aligned @param DocLines.
    let new_params: Vec<DocLine> = param_entries
        .iter()
        .map(|(type_str, name, description)| {
            let padding = " ".repeat(max_type_len - type_str.len());
            let line_text = if description.is_empty() {
                format!("@param {}{} {}", type_str, padding, name)
            } else {
                format!("@param {}{} {} {}", type_str, padding, name, description)
            };
            DocLine::Param(line_text)
        })
        .collect();

    // Insert new param lines.
    for (i, param_line) in new_params.into_iter().enumerate() {
        lines.insert(insert_pos + i, param_line);
    }

    // Add missing @throws tags.
    let uncaught = throws_analysis::find_uncaught_throw_types(content, info.docblock_position);
    let existing_throws_lower: Vec<String> = info
        .doc_throws
        .iter()
        .map(|t| {
            t.trim_start_matches('\\')
                .rsplit('\\')
                .next()
                .unwrap_or(t)
                .to_lowercase()
        })
        .collect();

    let mut new_throws: Vec<String> = Vec::new();
    for exc in &uncaught {
        let short = exc
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc);
        if !existing_throws_lower.contains(&short.to_lowercase()) {
            new_throws.push(short.to_string());
        }
    }

    if !new_throws.is_empty() {
        // Find the position to insert @throws — after the last existing
        // @throws tag, or after @param block, or before @return.
        let throws_insert_pos = find_throws_insert_position(&lines);
        for (i, exc) in new_throws.iter().enumerate() {
            lines.insert(
                throws_insert_pos + i,
                DocLine::OtherTag(format!("@throws {}", exc)),
            );
        }
    }

    // Update @return type if needed.
    if should_update_return
        && let Some(sig_ret) = &info.sig_return
        && let Some(doc_ret) = &info.doc_return
    {
        // Find and update the return line.
        for line in &mut lines {
            if let DocLine::Return(text) = line {
                let description = &doc_ret.description;
                if description.is_empty() {
                    *text = format!("@return {}", sig_ret);
                } else {
                    *text = format!("@return {} {}", sig_ret, description);
                }
                break;
            }
        }
    }

    // Rebuild the docblock text.
    rebuild_docblock(&lines, indent)
}

/// Categorized docblock line.
#[derive(Debug, Clone)]
enum DocLine {
    /// Opening `/**`.
    Open,
    /// Closing `*/`.
    Close,
    /// A summary or description line (not a tag).
    Text(String),
    /// A `@param` tag line.
    Param(String),
    /// A `@return` tag line.
    Return(String),
    /// Any other tag line (`@throws`, `@template`, `@deprecated`, etc.).
    OtherTag(String),
    /// An empty line (just ` * `).
    Empty,
}

/// Parse a docblock into categorized lines.
fn parse_docblock_lines(docblock: &str) -> Vec<DocLine> {
    let mut result = Vec::new();
    let lines: Vec<&str> = docblock.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if i == 0 && trimmed.starts_with("/**") {
            // Single-line docblock: `/** @return void */`
            if trimmed.ends_with("*/") && trimmed.len() > 5 {
                let inner = trimmed
                    .strip_prefix("/**")
                    .unwrap_or("")
                    .strip_suffix("*/")
                    .unwrap_or("")
                    .trim();
                result.push(DocLine::Open);
                if !inner.is_empty() {
                    categorize_tag_line(inner, &mut result);
                }
                result.push(DocLine::Close);
                continue;
            }
            result.push(DocLine::Open);
            // Check if there's content after `/**` on the same line.
            let after_open = trimmed.strip_prefix("/**").unwrap_or("").trim();
            if !after_open.is_empty() {
                categorize_tag_line(after_open, &mut result);
            }
            continue;
        }

        if trimmed == "*/" || trimmed.ends_with("*/") {
            // Check if there's content before `*/`.
            let before_close = trimmed.strip_suffix("*/").unwrap_or("").trim();
            let before_close = before_close
                .strip_prefix('*')
                .unwrap_or(before_close)
                .trim();
            if !before_close.is_empty() {
                categorize_tag_line(before_close, &mut result);
            }
            result.push(DocLine::Close);
            continue;
        }

        // Regular docblock line: ` * content`
        let content = trimmed.strip_prefix('*').unwrap_or(trimmed).trim();

        // Check if this is a continuation line (no `@` prefix, preceded by
        // a tag line). If so, merge it into the previous tag line.
        if !content.is_empty()
            && !content.starts_with('@')
            && !result.is_empty()
            && matches!(
                result.last(),
                Some(DocLine::Param(_)) | Some(DocLine::Return(_)) | Some(DocLine::OtherTag(_))
            )
        {
            match result.last_mut() {
                Some(DocLine::Param(text))
                | Some(DocLine::Return(text))
                | Some(DocLine::OtherTag(text)) => {
                    text.push(' ');
                    text.push_str(content);
                }
                _ => {}
            }
            continue;
        }

        if content.is_empty() {
            result.push(DocLine::Empty);
        } else {
            categorize_tag_line(content, &mut result);
        }
    }

    result
}

/// Categorize a single content line (without the `*` prefix).
fn categorize_tag_line(content: &str, result: &mut Vec<DocLine>) {
    if content.starts_with("@param") {
        result.push(DocLine::Param(content.to_string()));
    } else if content.starts_with("@return") {
        result.push(DocLine::Return(content.to_string()));
    } else if content.starts_with('@') {
        result.push(DocLine::OtherTag(content.to_string()));
    } else {
        result.push(DocLine::Text(content.to_string()));
    }
}

/// Find the position to insert new `@param` lines.
fn find_param_insert_position(lines: &[DocLine]) -> usize {
    // Insert before the first @return, @throws, or other tag that comes
    // after any text/summary.
    let mut last_text_or_empty = None;
    let mut first_return_or_throws = None;

    for (i, line) in lines.iter().enumerate() {
        match line {
            DocLine::Text(_) | DocLine::Empty => {
                last_text_or_empty = Some(i);
            }
            DocLine::Return(_) => {
                if first_return_or_throws.is_none() {
                    first_return_or_throws = Some(i);
                }
            }
            DocLine::OtherTag(text) => {
                if (text.starts_with("@throws") || text.starts_with("@return"))
                    && first_return_or_throws.is_none()
                {
                    first_return_or_throws = Some(i);
                }
            }
            _ => {}
        }
    }

    // Prefer inserting before @return/@throws.
    if let Some(pos) = first_return_or_throws {
        return pos;
    }

    // Otherwise insert after the last text/empty line.
    if let Some(pos) = last_text_or_empty {
        return pos + 1;
    }

    // Fallback: insert before Close.
    for (i, line) in lines.iter().enumerate() {
        if matches!(line, DocLine::Close) {
            return i;
        }
    }

    lines.len()
}

/// Find the position to insert new `@throws` lines.
fn find_throws_insert_position(lines: &[DocLine]) -> usize {
    // Insert after the last existing @throws tag.
    let mut last_throws = None;
    let mut first_return = None;

    for (i, line) in lines.iter().enumerate() {
        match line {
            DocLine::OtherTag(text) if text.starts_with("@throws") => {
                last_throws = Some(i);
            }
            DocLine::Return(_) => {
                if first_return.is_none() {
                    first_return = Some(i);
                }
            }
            _ => {}
        }
    }

    // After the last existing @throws.
    if let Some(pos) = last_throws {
        return pos + 1;
    }

    // Before @return (but after any blank separator preceding it).
    if let Some(pos) = first_return {
        // If the line before @return is Empty, insert before that too.
        if pos > 0 && matches!(lines.get(pos - 1), Some(DocLine::Empty)) {
            return pos - 1;
        }
        return pos;
    }

    // After the last @param.
    let mut last_param = None;
    for (i, line) in lines.iter().enumerate() {
        if matches!(line, DocLine::Param(_)) {
            last_param = Some(i);
        }
    }
    if let Some(pos) = last_param {
        return pos + 1;
    }

    // Fallback: before Close.
    for (i, line) in lines.iter().enumerate() {
        if matches!(line, DocLine::Close) {
            return i;
        }
    }

    lines.len()
}

/// Check if the `@return` tag should be removed.
fn should_remove_return(info: &FunctionWithDocblock) -> bool {
    if let Some(sig_ret) = &info.sig_return
        && let Some(doc_ret) = &info.doc_return
        && sig_ret.to_lowercase() == "void"
        && doc_ret.type_str.to_lowercase() == "void"
        && doc_ret.description.is_empty()
    {
        return true;
    }
    false
}

/// Check if the `@return` tag needs its type updated.
fn should_update_return(info: &FunctionWithDocblock) -> bool {
    if let Some(sig_ret) = &info.sig_return
        && let Some(doc_ret) = &info.doc_return
        && is_type_contradiction(&doc_ret.type_str, sig_ret)
    {
        return true;
    }
    false
}

/// Rebuild a docblock string from categorized lines.
fn rebuild_docblock(lines: &[DocLine], indent: &str) -> String {
    let mut result = String::new();
    let mut prev_was_param = false;
    let mut prev_was_text_or_empty = false;

    for (i, line) in lines.iter().enumerate() {
        match line {
            DocLine::Open => {
                result.push_str("/**");
                result.push('\n');
                prev_was_param = false;
                prev_was_text_or_empty = false;
            }
            DocLine::Close => {
                result.push_str(indent);
                result.push_str(" */");
                prev_was_param = false;
                prev_was_text_or_empty = false;
            }
            DocLine::Text(text) => {
                // Add blank separator before text if preceded by tags.
                if prev_was_param {
                    result.push_str(indent);
                    result.push_str(" *\n");
                }
                result.push_str(indent);
                result.push_str(" * ");
                result.push_str(text);
                result.push('\n');
                prev_was_param = false;
                prev_was_text_or_empty = true;
            }
            DocLine::Empty => {
                result.push_str(indent);
                result.push_str(" *\n");
                prev_was_param = false;
                prev_was_text_or_empty = true;
            }
            DocLine::Param(text) => {
                // Add blank separator before first @param if preceded by text.
                if !prev_was_param && prev_was_text_or_empty {
                    // Check if the previous line was already empty.
                    let prev_empty = i > 0 && matches!(lines.get(i - 1), Some(DocLine::Empty));
                    if !prev_empty {
                        result.push_str(indent);
                        result.push_str(" *\n");
                    }
                }
                result.push_str(indent);
                result.push_str(" * ");
                result.push_str(text);
                result.push('\n');
                prev_was_param = true;
                prev_was_text_or_empty = false;
            }
            DocLine::Return(text) => {
                // Add blank separator before @return if preceded by @param.
                if prev_was_param {
                    result.push_str(indent);
                    result.push_str(" *\n");
                }
                // Add blank separator if preceded by text without a blank line.
                if prev_was_text_or_empty && !prev_was_param {
                    let prev_empty = i > 0 && matches!(lines.get(i - 1), Some(DocLine::Empty));
                    if !prev_empty {
                        result.push_str(indent);
                        result.push_str(" *\n");
                    }
                }
                result.push_str(indent);
                result.push_str(" * ");
                result.push_str(text);
                result.push('\n');
                prev_was_param = false;
                prev_was_text_or_empty = false;
            }
            DocLine::OtherTag(text) => {
                if prev_was_param {
                    result.push_str(indent);
                    result.push_str(" *\n");
                }
                result.push_str(indent);
                result.push_str(" * ");
                result.push_str(text);
                result.push('\n');
                prev_was_param = false;
                prev_was_text_or_empty = false;
            }
        }
    }

    result
}

/// Convert an LSP position to a byte offset.
fn position_to_offset_ud(content: &str, position: Position) -> u32 {
    let mut offset = 0usize;
    for (line_idx, line) in content.split('\n').enumerate() {
        if line_idx == position.line as usize {
            let char_offset = position.character as usize;
            let byte_col = line
                .char_indices()
                .nth(char_offset)
                .map(|(i, _)| i)
                .unwrap_or(line.len());
            return (offset + byte_col) as u32;
        }
        offset += line.len() + 1;
    }
    offset as u32
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: parse PHP and check if an update is needed at the given offset.
    fn find_info(php: &str, offset: u32) -> Option<FunctionWithDocblock> {
        let arena = Bump::new();
        let file_id = mago_database::file::FileId::new("input.php");
        let program = mago_syntax::parser::parse_file_content(&arena, file_id, php);
        find_function_with_docblock(&program.statements, program.trivia.as_slice(), php, offset)
    }

    /// Stub class loader that never resolves anything (for unit tests).
    fn no_class_loader() -> impl Fn(&str) -> Option<Arc<ClassInfo>> {
        |_| None
    }

    #[test]
    fn detects_missing_param() {
        let php = r#"<?php
class Foo {
    /**
     * Does something.
     *
     * @param string $a The first param
     */
    public function bar(string $a, int $b): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn detects_extra_param() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     * @param int $b
     */
    public function bar(string $a): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn detects_reordered_params() {
        let php = r#"<?php
class Foo {
    /**
     * @param int $b
     * @param string $a
     */
    public function bar(string $a, int $b): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn no_update_when_params_match() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     * @param int $b
     */
    public function bar(string $a, int $b): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(!check_needs_update(&info, php, &cl));
    }

    #[test]
    fn detects_type_contradiction_in_param() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     */
    public function bar(int $a): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn preserves_refinement_type() {
        let php = r#"<?php
class Foo {
    /**
     * @param non-empty-string $a
     */
    public function bar(string $a): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(!check_needs_update(&info, php, &cl));
    }

    #[test]
    fn detects_void_return_redundancy() {
        let php = r#"<?php
class Foo {
    /**
     * @return void
     */
    public function bar(): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn detects_return_type_contradiction() {
        let php = r#"<?php
class Foo {
    /**
     * @return string
     */
    public function bar(): int {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn no_action_without_docblock() {
        let php = r#"<?php
class Foo {
    public function bar(string $a): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos);
        assert!(info.is_none());
    }

    #[test]
    fn works_with_standalone_function() {
        let php = r#"<?php
/**
 * @param string $a
 * @param int $b
 */
function bar(string $a, int $b, bool $c): void {}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn preserves_descriptions() {
        let php = r#"<?php
class Foo {
    /**
     * Summary line.
     *
     * @param string $a The first param
     */
    public function bar(string $a, int $b): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        assert!(
            updated.contains("The first param"),
            "Should preserve description: {}",
            updated
        );
        assert!(
            updated.contains("$b"),
            "Should add missing param: {}",
            updated
        );
        assert!(
            updated.contains("Summary line"),
            "Should preserve summary: {}",
            updated
        );
    }

    #[test]
    fn removes_extra_param_and_adds_missing() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $old
     * @param int $b
     */
    public function bar(int $b, bool $c): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        assert!(
            !updated.contains("$old"),
            "Should remove old param: {}",
            updated
        );
        assert!(updated.contains("$b"), "Should keep $b: {}", updated);
        assert!(updated.contains("$c"), "Should add $c: {}", updated);
    }

    #[test]
    fn updates_contradicted_return_type() {
        let php = r#"<?php
class Foo {
    /**
     * @return string Some description
     */
    public function bar(): int {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        assert!(
            updated.contains("@return int Some description"),
            "Should update return type: {}",
            updated
        );
    }

    #[test]
    fn removes_void_return() {
        let php = r#"<?php
class Foo {
    /**
     * Does something.
     *
     * @return void
     */
    public function bar(): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        assert!(
            !updated.contains("@return"),
            "Should remove @return void: {}",
            updated
        );
    }

    #[test]
    fn handles_variadic_param() {
        let php = r#"<?php
class Foo {
    /**
     * @param string ...$args
     */
    public function bar(string ...$args): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        // Variadic params should match — no update needed.
        assert!(!check_needs_update(&info, php, &cl));
    }

    #[test]
    fn preserves_generic_refinement() {
        let php = r#"<?php
class Foo {
    /**
     * @param array<int, string> $items
     */
    public function bar(array $items): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        // array<int, string> refines array — no contradiction.
        assert!(!check_needs_update(&info, php, &cl));
    }

    #[test]
    fn preserves_other_tags() {
        let php = r#"<?php
class Foo {
    /**
     * Summary.
     *
     * @template T
     * @param string $a
     * @throws \RuntimeException
     */
    public function bar(string $a, int $b): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        assert!(
            updated.contains("@template T"),
            "Should preserve @template: {}",
            updated
        );
        assert!(
            updated.contains("@throws"),
            "Should preserve @throws: {}",
            updated
        );
        assert!(
            updated.contains("$b"),
            "Should add missing param: {}",
            updated
        );
    }

    #[test]
    fn is_contradiction_basic() {
        assert!(is_type_contradiction("string", "int"));
        assert!(!is_type_contradiction("string", "string"));
        assert!(!is_type_contradiction("non-empty-string", "string"));
        assert!(!is_type_contradiction("array<int, string>", "array"));
    }

    #[test]
    fn is_contradiction_nullable() {
        // ?string and string|null are equivalent.
        assert!(!is_type_contradiction("?string", "?string"));
        assert!(!is_type_contradiction("string|null", "?string"));
    }

    #[test]
    fn works_in_namespace() {
        let php = r#"<?php
namespace App;
class Foo {
    /**
     * @param string $a
     */
    public function bar(int $a): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(check_needs_update(&info, php, &cl));
    }

    #[test]
    fn aligns_param_columns() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     */
    public function bar(string $a, int $b, array $items): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        // All $names should be aligned at the same column.
        assert!(
            updated.contains("@param string $a"),
            "Should have string padded: {}",
            updated
        );
        assert!(
            updated.contains("@param int    $b"),
            "Should have int padded: {}",
            updated
        );
        assert!(
            updated.contains("@param array  $items"),
            "Should have array padded: {}",
            updated
        );
    }

    #[test]
    fn no_spurious_blank_line_after_open() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     * @param int $b
     *
     * @return string
     */
    public function bar(string $a, int $b, bool $c): string {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        // Should NOT have a blank line between /** and the first @param.
        let lines: Vec<&str> = updated.lines().collect();
        assert_eq!(
            lines[0].trim(),
            "/**",
            "First line should be opening: {}",
            updated
        );
        assert!(
            lines[1].trim().starts_with("* @param"),
            "Second line should be @param, not blank: {}",
            updated
        );
    }

    #[test]
    fn enriches_callable_types() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     */
    public function bar(string $a, Closure $handler, callable $fallback): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        assert!(
            updated.contains("(Closure(): mixed)"),
            "Should enrich Closure: {}",
            updated
        );
        assert!(
            updated.contains("(callable(): mixed)"),
            "Should enrich callable: {}",
            updated
        );
    }

    #[test]
    fn adds_missing_throws() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     *
     * @return string
     */
    public function bar(string $a): string {
        throw new \RuntimeException('oops');
    }
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        let updated = build_updated_docblock(&info, php, &cl);
        assert!(
            updated.contains("@throws RuntimeException"),
            "Should add missing @throws: {}",
            updated
        );
    }

    #[test]
    fn does_not_duplicate_existing_throws() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     *
     * @throws RuntimeException
     *
     * @return string
     */
    public function bar(string $a): string {
        throw new \RuntimeException('oops');
    }
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(
            !check_needs_update(&info, php, &cl),
            "Should not need update when throws already documented"
        );
    }

    #[test]
    fn triggers_when_cursor_inside_docblock() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     */
    public function bar(string $a, int $b): void {}
}
"#;
        // Place the cursor on the @param line inside the docblock.
        let pos = php.find("@param string").unwrap() as u32;
        let info = find_info(php, pos);
        assert!(
            info.is_some(),
            "Should find function info when cursor is inside the docblock"
        );
        let cl = no_class_loader();
        assert!(check_needs_update(&info.unwrap(), php, &cl));
    }

    #[test]
    fn triggers_when_cursor_on_docblock_summary() {
        let php = r#"<?php
class Foo {
    /**
     * Does something.
     *
     * @param string $a
     */
    public function bar(string $a, int $b): void {}
}
"#;
        // Place the cursor on the summary line.
        let pos = php.find("Does something").unwrap() as u32;
        let info = find_info(php, pos);
        assert!(
            info.is_some(),
            "Should find function info when cursor is on docblock summary"
        );
    }

    #[test]
    fn triggers_when_cursor_on_opening_docblock() {
        let php = r#"<?php
class Foo {
    /**
     * @param string $a
     */
    public function bar(string $a, int $b): void {}
}
"#;
        // Place the cursor on the /** line.
        let pos = php.find("/**").unwrap() as u32;
        let info = find_info(php, pos);
        assert!(
            info.is_some(),
            "Should find function info when cursor is on opening /**"
        );
    }

    // ── @param with no type ─────────────────────────────────────────

    #[test]
    fn parse_param_no_type_recognised() {
        let docblock = r#"/**
     * @param $name The user name
     */"#;
        let params = parse_doc_params(docblock, 0);
        assert_eq!(params.len(), 1, "should parse one param: {:?}", params);
        assert_eq!(params[0].name, "$name");
        assert_eq!(params[0].type_str, "");
        assert_eq!(params[0].description, "The user name");
    }

    #[test]
    fn parse_param_no_type_variadic() {
        let docblock = r#"/**
     * @param ...$args The arguments
     */"#;
        let params = parse_doc_params(docblock, 0);
        assert_eq!(params.len(), 1, "should parse one param: {:?}", params);
        assert_eq!(params[0].name, "...$args");
        assert_eq!(params[0].type_str, "");
        assert_eq!(params[0].description, "The arguments");
    }

    #[test]
    fn parse_param_no_type_no_description() {
        let docblock = r#"/**
     * @param $name
     */"#;
        let params = parse_doc_params(docblock, 0);
        assert_eq!(params.len(), 1, "should parse one param: {:?}", params);
        assert_eq!(params[0].name, "$name");
        assert_eq!(params[0].type_str, "");
    }

    #[test]
    fn parse_param_no_type_mixed_with_typed() {
        let docblock = r#"/**
     * @param string $a First
     * @param $b Second
     * @param int $c Third
     */"#;
        let params = parse_doc_params(docblock, 0);
        assert_eq!(params.len(), 3, "should parse three params: {:?}", params);
        assert_eq!(params[0].name, "$a");
        assert_eq!(params[0].type_str, "string");
        assert_eq!(params[1].name, "$b");
        assert_eq!(params[1].type_str, "");
        assert_eq!(params[1].description, "Second");
        assert_eq!(params[2].name, "$c");
        assert_eq!(params[2].type_str, "int");
    }

    #[test]
    fn update_needed_when_untyped_param_matches_untyped_sig() {
        // Even when both the docblock and signature omit the type, the
        // update action should fire to add `mixed` as the explicit type.
        let php = r#"<?php
class Foo {
    /**
     * @param $name The user name
     */
    public function bar($name): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(
            check_needs_update(&info, php, &cl),
            "should need update to add `mixed` type to @param $name"
        );
        // The param must still be recognised (not duplicated).
        assert_eq!(info.doc_params.len(), 1);
        assert_eq!(info.doc_params[0].name, "$name");
        assert_eq!(info.doc_params[0].type_str, "");
        assert_eq!(info.doc_params[0].description, "The user name");
    }

    #[test]
    fn detects_missing_param_when_existing_has_no_type() {
        let php = r#"<?php
class Foo {
    /**
     * @param $a First param
     */
    public function bar(string $a, int $b): void {}
}
"#;
        let pos = php.find("function bar").unwrap() as u32;
        let info = find_info(php, pos).unwrap();
        let cl = no_class_loader();
        assert!(
            check_needs_update(&info, php, &cl),
            "should need update because $b is missing"
        );
        assert_eq!(info.doc_params.len(), 1);
        assert_eq!(info.doc_params[0].name, "$a");
        assert_eq!(info.doc_params[0].description, "First param");
    }
}
