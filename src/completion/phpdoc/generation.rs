//! PHPDoc block generation on `/**`.
//!
//! Two entry points:
//!
//! 1. **Completion** (`try_generate_docblock`) — fires when `/` is a
//!    trigger character and the cursor is right after `/**`.  Returns a
//!    snippet-format `CompletionItem` with tab stops.  Works in editors
//!    that do *not* auto-close `/**`.
//!
//! 2. **On-type formatting** (`try_generate_docblock_on_enter`) — fires
//!    on Enter (`\n`) via `textDocument/onTypeFormatting`.  Detects a
//!    freshly auto-generated empty `/** … */` block (the kind VS Code
//!    and Zed produce when you type `/**`), replaces it with a filled
//!    docblock, and positions the cursor on the summary line.  Works in
//!    editors that *do* auto-close `/**`.
//!
//! Both paths share the same declaration analysis and snippet/text
//! building helpers defined below.
//!
//! **Design choices:**
//!
//! - A docblock is always generated (at minimum a summary skeleton).
//! - `@param` / `@return` tags are only emitted when the native type
//!   hint cannot fully express the type: missing type, bare `array`,
//!   `Closure` / `callable`, union containing any of those, or a
//!   class that has `@template` parameters.
//! - `Closure` and `callable` get a callable-signature placeholder
//!   wrapped in parentheses: `(Closure(): mixed)`, `(callable(): mixed)`.
//! - Union types containing `array`, `Closure`, or `callable` echo
//!   the raw type string so the user can refine the relevant part.
//! - `@throws` tags are always added for uncaught exception types.
//! - No special treatment for overrides — the same rules apply.
//! - Class-like declarations get `@extends` / `@implements` tags when
//!   the parent or interface has `@template` parameters.
//! - Properties and constants always get `@var Type`.
//! - Tags are ordered `@param`, `@throws`, `@return` with a blank
//!   `*` separator line between different groups (not within a group,
//!   and not before the first group).  No summary line is emitted
//!   when tags are present.
//! - When there are no tags, a summary-only skeleton is generated.
//! - Parameter names within the `@param` block are space-aligned.

use std::collections::HashMap;
use std::sync::Arc;

use tower_lsp::lsp_types::*;

use super::context::{DocblockContext, SymbolInfo};
use super::helpers::{find_keyword_pos, find_matching_paren, split_params};
use crate::completion::resolver::FunctionLoaderFn;
use crate::completion::source::comment_position::position_to_byte_offset;
use crate::completion::source::throws_analysis::{self, ThrowsContext};
use crate::completion::use_edit::{analyze_use_block, build_use_edit};
use crate::php_type::PhpType;
use crate::types::{ClassInfo, FunctionLoader};
use crate::util::{byte_offset_to_utf16_col, utf16_col_to_byte_offset};

/// Detect whether the cursor is immediately after a `/**` trigger and,
/// if so, generate a full docblock completion item.
///
/// Returns `None` when the cursor is not at a `/**` trigger position or
/// when the declaration below cannot be identified.
pub fn try_generate_docblock(
    content: &str,
    position: Position,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoader<'_>,
) -> Option<CompletionResponse> {
    let (trigger_range, indent) = detect_docblock_trigger(content, position)?;

    // Find the declaration below and classify it.
    let remaining = get_text_after_trigger(content, position);
    let context = classify_declaration(&remaining);

    // Inside a function body (Inline / Unknown) we don't generate a
    // full docblock — the `@` tag completion is more appropriate there
    // because the user might want @var, @throws, @todo, etc.
    if matches!(context, DocblockContext::Inline | DocblockContext::Unknown) {
        return None;
    }

    let mut sym = parse_declaration_info(&remaining);

    // For untyped properties, try to fill in the type from the parsed
    // class data (e.g. constructor-inferred `$this->prop = new Foo()`).
    if matches!(context, DocblockContext::Property) && sym.type_hint.is_none() {
        enrich_property_type_from_class(&mut sym, content, position, local_classes);
    }

    let snippet = build_docblock_snippet(
        &context,
        &sym,
        &indent,
        content,
        position,
        use_map,
        file_namespace,
        local_classes,
        class_loader,
        function_loader,
    );

    if snippet.is_empty() {
        return None;
    }

    // Collect additional text edits (e.g. use imports for @throws).
    let additional_edits = build_throws_import_edits(
        content,
        position,
        use_map,
        file_namespace,
        &context,
        class_loader,
        function_loader,
    );

    let item = CompletionItem {
        label: "/** PHPDoc Block */".to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Generate PHPDoc block".to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: trigger_range,
            new_text: snippet,
        })),
        filter_text: Some("/**".to_string()),
        sort_text: Some("0".to_string()),
        additional_text_edits: if additional_edits.is_empty() {
            None
        } else {
            Some(additional_edits)
        },
        // Pre-select so the user can just press Enter.
        preselect: Some(true),
        ..CompletionItem::default()
    };

    Some(CompletionResponse::Array(vec![item]))
}

// ─── On-Type Formatting Entry Point ─────────────────────────────────────────

/// Handle `textDocument/onTypeFormatting` after Enter inside a freshly
/// auto-generated `/** */` or `/**\n * \n */` block.
///
/// Most editors (VS Code, Zed, Neovim with auto-pairs) expand `/**`
/// into a closed block before the LSP sees anything.  The user then
/// presses Enter, and `onTypeFormatting` fires with `ch = "\n"`.
///
/// This function detects that pattern, finds the declaration below the
/// docblock, and returns `TextEdit`s that replace the empty block with
/// a filled one.  Returns `None` when the cursor is not inside a fresh
/// empty docblock.
pub fn try_generate_docblock_on_enter(
    content: &str,
    position: Position,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoader<'_>,
) -> Option<Vec<TextEdit>> {
    // Detect the empty docblock range and indentation.
    let (block_range, _block_indent, after_block) = detect_empty_docblock(content, position)?;

    // Use the declaration's indentation rather than the `/**` line's.
    // Some editors (e.g. Zed) place the auto-closed `/** */` at the
    // wrong indent level inside constructor parameter lists.  The
    // declaration line is always at the correct level.
    let indent = declaration_indent(&after_block);

    // Classify and parse the declaration after the block.
    let context = classify_declaration(&after_block);

    // Inside a function body (Inline / Unknown) we don't generate a
    // full docblock — the `@` tag completion is more appropriate there.
    if matches!(context, DocblockContext::Inline | DocblockContext::Unknown) {
        return None;
    }

    let mut sym = parse_declaration_info(&after_block);

    // For untyped properties, try to fill in the type from the parsed
    // class data (e.g. constructor-inferred `$this->prop = new Foo()`).
    if matches!(context, DocblockContext::Property) && sym.type_hint.is_none() {
        enrich_property_type_from_class(&mut sym, content, position, local_classes);
    }

    // Build the docblock as plain text (no snippet tab stops).
    let plain = build_docblock_plain(
        &context,
        &sym,
        &indent,
        content,
        position,
        use_map,
        file_namespace,
        local_classes,
        class_loader,
        function_loader,
    );

    if plain.is_empty() {
        return None;
    }

    let mut edits = vec![TextEdit {
        range: block_range,
        new_text: plain,
    }];

    // Auto-import edits for @throws.
    edits.extend(build_throws_import_edits(
        content,
        position,
        use_map,
        file_namespace,
        &context,
        class_loader,
        function_loader,
    ));

    Some(edits)
}

/// Detect whether the cursor is inside a freshly auto-generated empty
/// docblock.  Returns `(range_of_entire_block, indent, text_after_block)`.
///
/// Recognised patterns (after the editor auto-closes `/**`):
///
/// ```text
/// /** */          ← single-line empty
/// /**             ← multi-line empty
///  *              (cursor is here after Enter)
///  */
/// /**             ← multi-line with blank star line
///  * |
///  */
/// ```
fn detect_empty_docblock(content: &str, position: Position) -> Option<(Range, String, String)> {
    let lines: Vec<&str> = content.lines().collect();
    let cur_line = position.line as usize;

    // ── Find the `/**` opening ──────────────────────────────────────
    // Walk backwards from the cursor line to find a line containing `/**`.
    let mut open_line = None;
    for i in (0..=cur_line).rev() {
        if i >= lines.len() {
            continue;
        }
        let trimmed = lines[i].trim();
        if trimmed.contains("/**") {
            open_line = Some(i);
            break;
        }
        // Stop if we hit a non-docblock, non-empty line (e.g. code).
        if !trimmed.is_empty() && !trimmed.starts_with('*') && !trimmed.starts_with("*/") {
            return None;
        }
    }
    let open_idx = open_line?;

    // ── Check this is a fresh empty docblock ────────────────────────
    // The opening line must be just `/**` (with optional whitespace and
    // optional `*/` on the same line).
    let open_text = lines[open_idx];
    let trimmed_open = open_text.trim();
    if !trimmed_open.starts_with("/**") {
        return None;
    }

    // Extract indentation from the opening line.
    let indent: String = open_text
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .collect();

    // ── Find the `*/` closing ───────────────────────────────────────
    let mut close_line = None;

    // Single-line case: `/** */` on one line.
    if trimmed_open.ends_with("*/") && trimmed_open.len() <= "/** */".len() + 2 {
        close_line = Some(open_idx);
    } else {
        // Multi-line: scan forward from the opening line.
        for (i, line) in lines.iter().enumerate().skip(open_idx + 1) {
            let trimmed = line.trim();
            if trimmed == "*/" || trimmed.ends_with("*/") {
                close_line = Some(i);
                break;
            }
            // A line with real content (not just `*` or whitespace)
            // means this is an existing docblock with documentation.
            if let Some(after_star) = trimmed
                .strip_prefix("* ")
                .or_else(|| trimmed.strip_prefix("*\t"))
            {
                let after_star = after_star.trim();
                if !after_star.is_empty() {
                    // There's actual text — this is not a fresh block.
                    return None;
                }
            }
        }
    }
    let close_idx = close_line?;

    // Verify the docblock is "empty" — the only content between `/**`
    // and `*/` should be blank `*` lines.
    for line in lines.iter().take(close_idx).skip(open_idx + 1) {
        let trimmed = line.trim();
        // Allow: empty, bare `*`, `* ` (trailing space), or cursor line.
        if !trimmed.is_empty()
            && trimmed != "*"
            && !trimmed.chars().all(|c| c == '*' || c == ' ' || c == '\t')
        {
            return None;
        }
    }

    // ── Build the range covering the entire block ───────────────────
    let start = Position {
        line: open_idx as u32,
        character: 0,
    };
    // End covers through the closing `*/` line (including its newline
    // if there is a next line).
    let close_line_len = lines.get(close_idx).map(|l| l.len()).unwrap_or(0);
    let end = if close_idx + 1 < lines.len() {
        // Include the trailing newline.
        Position {
            line: (close_idx + 1) as u32,
            character: 0,
        }
    } else {
        Position {
            line: close_idx as u32,
            character: close_line_len as u32,
        }
    };
    let block_range = Range { start, end };

    // ── Collect text after the block ────────────────────────────────
    let after_start = if close_idx + 1 < lines.len() {
        close_idx + 1
    } else {
        lines.len()
    };
    let after_block: String = lines[after_start..].to_vec().join("\n");

    Some((block_range, indent, after_block))
}

/// Extract the indentation of the first declaration line in `text`,
/// skipping empty lines and attribute blocks.
fn declaration_indent(text: &str) -> String {
    let mut attr_depth = 0i32;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if attr_depth > 0 || trimmed.starts_with("#[") {
            for ch in trimmed.chars() {
                match ch {
                    '[' => attr_depth += 1,
                    ']' => attr_depth -= 1,
                    _ => {}
                }
            }
            continue;
        }
        // First non-empty, non-attribute line — return its indent.
        return line
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
    }
    String::new()
}

// ─── Trigger Detection ──────────────────────────────────────────────────────

/// Check if the cursor is immediately after `/**` with only whitespace
/// before it on the line, and that there is no existing docblock (i.e.
/// the `/**` is not already closed with `*/`).
///
/// Returns the range covering the `/**` text (to be replaced by the
/// snippet) and the leading indentation string.
fn detect_docblock_trigger(content: &str, position: Position) -> Option<(Range, String)> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];

    // Convert the UTF-16 column offset to a byte offset within the line.
    // LSP positions use UTF-16 code units, which diverge from byte offsets
    // when the line contains multibyte characters (e.g. "ń" is 2 bytes in
    // UTF-8 but 1 UTF-16 code unit).
    let col = utf16_col_to_byte_offset(line, position.character);

    // The cursor column must be at least 3 (for `/**`).
    if col < 3 {
        return None;
    }

    // Get the text up to the cursor on this line.
    let before_cursor = if col <= line.len() {
        &line[..col]
    } else {
        line
    };

    // Must end with `/**`.
    if !before_cursor.ends_with("/**") {
        return None;
    }

    // Everything before `/**` must be whitespace.
    let prefix = &before_cursor[..before_cursor.len() - 3];
    if !prefix.chars().all(|c| c == ' ' || c == '\t') {
        return None;
    }

    // Check what follows the `/**` on this line.
    let after_trigger = if col <= line.len() { &line[col..] } else { "" };

    // Editors like VS Code auto-close `/**` into `/** */` on the same
    // line.  We allow this when the only thing after `/**` is optional
    // whitespace and `*/` (i.e. an empty auto-closed block).
    let after_trimmed = after_trigger.trim();
    let auto_closed = after_trimmed == "*/" || after_trimmed.is_empty();

    // If there is a `*/` with real content between `/**` and `*/`
    // (e.g. `/** @var int */`), this is an existing single-line
    // docblock — don't trigger.
    if !auto_closed && after_trigger.contains("*/") {
        return None;
    }

    // Also check that the next few lines don't form an existing
    // docblock (i.e. don't generate a new block inside an existing one).
    // A simple heuristic: if the next non-empty line starts with `*` or
    // contains `*/`, there's already a docblock.
    if !after_trigger.contains("*/") {
        for next_line in lines.iter().skip(line_idx + 1) {
            let trimmed = next_line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.starts_with('*') || trimmed.starts_with("*/") {
                return None;
            }
            // First non-empty, non-docblock-continuation line found — OK.
            break;
        }
    }

    let indent = prefix.to_string();

    // Convert byte offsets back to UTF-16 columns for the LSP Range.
    let start_col = byte_offset_to_utf16_col(line, col - 3);
    let end_col = if after_trigger.contains("*/") {
        byte_offset_to_utf16_col(line, line.len())
    } else {
        byte_offset_to_utf16_col(line, col)
    };

    let range = Range {
        start: Position {
            line: position.line,
            character: start_col,
        },
        end: Position {
            line: position.line,
            character: end_col,
        },
    };

    Some((range, indent))
}

// ─── Declaration Analysis ───────────────────────────────────────────────────

/// Get the text after the `/**` trigger position, skipping the rest of
/// the trigger line.
fn get_text_after_trigger(content: &str, position: Position) -> String {
    let byte_offset = position_to_byte_offset(content, position);
    let after = &content[byte_offset.min(content.len())..];

    // Skip to the next line (the trigger line has `/**` and possibly
    // nothing else useful).
    if let Some(nl) = after.find('\n') {
        after[nl + 1..].to_string()
    } else {
        String::new()
    }
}

/// Classify the PHP symbol from the first meaningful tokens after the
/// trigger.
fn classify_declaration(text: &str) -> DocblockContext {
    let mut tokens = Vec::new();
    let mut attr_depth = 0i32;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Skip lines that look like docblock continuation (shouldn't
        // happen after our trigger, but be safe).
        if trimmed.starts_with('*') || trimmed.starts_with("/**") {
            continue;
        }
        // Skip PHP 8 attribute lines (#[...]).  Track bracket nesting
        // depth so that array literals inside attributes (e.g.
        // `#[Route(methods: ['GET'])]`) don't prematurely end tracking.
        if attr_depth > 0 || trimmed.starts_with("#[") {
            for ch in trimmed.chars() {
                match ch {
                    '[' => attr_depth += 1,
                    ']' => attr_depth -= 1,
                    _ => {}
                }
            }
            continue;
        }
        for word in trimmed.split_whitespace() {
            tokens.push(word.to_lowercase());
            if tokens.len() >= 8 {
                break;
            }
        }
        if tokens.len() >= 8 {
            break;
        }
    }

    if tokens.is_empty() {
        return DocblockContext::Unknown;
    }

    let mut saw_modifier = false;
    for token in &tokens {
        let t = token.as_str();
        match t {
            "function" => return DocblockContext::FunctionOrMethod,
            "class" | "interface" | "trait" | "enum" | "abstract" | "final" | "readonly" => {
                // "abstract" and "final" could precede either a class or
                // a method.  Keep scanning.
                if matches!(t, "class" | "interface" | "trait" | "enum") {
                    return DocblockContext::ClassLike;
                }
                saw_modifier = true;
            }
            "public" | "protected" | "private" | "static" | "var" => {
                saw_modifier = true;
            }
            "const" => return DocblockContext::Constant,
            _ => {
                if saw_modifier {
                    // After a visibility/static keyword, if the next
                    // token is `function`, it's a method.  Otherwise
                    // it's likely a property (e.g. `public int $x`).
                    if t == "function" {
                        return DocblockContext::FunctionOrMethod;
                    }
                    if t.starts_with('$') {
                        return DocblockContext::Property;
                    }
                    // Could be a type hint before a property.
                    continue;
                }
                // Bare `$var` without modifiers — a local variable
                // assignment (e.g. `$var = [''];`).
                if t.starts_with('$') {
                    return DocblockContext::Inline;
                }
                break;
            }
        }
    }

    if saw_modifier {
        // Saw modifiers but no clear keyword — likely a typed property.
        return DocblockContext::Property;
    }

    DocblockContext::Unknown
}

/// Parse the declaration after the trigger to extract parameter names,
/// type hints, return types, etc.
fn parse_declaration_info(text: &str) -> SymbolInfo {
    // Reuse the existing parser from the context module, but we need
    // to work from the raw text directly.
    let mut info = SymbolInfo::default();

    // Collect the declaration — may span multiple lines until `{` or `;`.
    let mut decl = String::new();
    let mut attr_depth = 0i32;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('*') || trimmed.starts_with("/**") {
            continue;
        }
        // Skip PHP 8 attribute lines (#[...]).  Track bracket nesting
        // depth so that array literals inside attributes don't
        // prematurely end tracking.
        if attr_depth > 0 || trimmed.starts_with("#[") {
            for ch in trimmed.chars() {
                match ch {
                    '[' => attr_depth += 1,
                    ']' => attr_depth -= 1,
                    _ => {}
                }
            }
            continue;
        }
        decl.push(' ');
        decl.push_str(trimmed);
        if trimmed.contains('{') || trimmed.contains(';') {
            break;
        }
    }

    let decl = decl.trim();
    if decl.is_empty() {
        return info;
    }

    // Check if it's a function/method.
    if let Some(func_pos) = find_keyword_pos(decl, "function") {
        let after_func = &decl[func_pos + 8..].trim_start();

        // Extract the method/function name (skip leading `&` for references).
        let name_src = after_func
            .strip_prefix('&')
            .unwrap_or(after_func)
            .trim_start();
        let name: String = name_src
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            info.method_name = Some(name);
        }

        if let Some(open_paren) = after_func.find('(') {
            let after_open = &after_func[open_paren + 1..];
            if let Some(close_paren) = find_matching_paren(after_open) {
                let params_str = &after_open[..close_paren];
                info.params = parse_params(params_str);

                let after_close = &after_open[close_paren + 1..];
                info.return_type = extract_return_type_from_decl(after_close);
            }
        }
    } else if is_class_like_keyword(decl) {
        // Class-like — extract extends/implements names.
        let (extends, implements) = extract_class_supertypes(decl);
        info.extends_names = extends;
        info.implements_names = implements;
    } else {
        // Property or constant — extract type hint.
        info.type_hint = extract_property_type(decl);
        // For inline variable assignments, extract the variable name.
        if let Some(dollar) = decl.find('$') {
            let name: String = decl[dollar..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$')
                .collect();
            if !name.is_empty() {
                info.variable_name = Some(name);
            }
        }
    }

    info
}

/// Check whether a declaration string starts with a class-like keyword
/// (class, interface, trait, enum), possibly preceded by modifiers.
fn is_class_like_keyword(decl: &str) -> bool {
    let class_keywords = ["class", "interface", "trait", "enum"];
    let modifier_keywords = ["abstract", "final", "readonly"];
    let lower = decl.to_lowercase();
    let mut rest = lower.as_str().trim();
    loop {
        let mut found = false;
        for kw in &class_keywords {
            if let Some(after) = rest.strip_prefix(*kw)
                && (after.is_empty() || after.starts_with(|c: char| c.is_whitespace()))
            {
                return true;
            }
        }
        for kw in &modifier_keywords {
            if let Some(after) = rest.strip_prefix(*kw)
                && (after.is_empty() || after.starts_with(|c: char| c.is_whitespace()))
            {
                rest = after.trim_start();
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
    }
    false
}

/// Extract parent class names and interface names from a class-like
/// declaration header (e.g. `class Foo extends Bar implements Baz`).
fn extract_class_supertypes(decl: &str) -> (Vec<String>, Vec<String>) {
    let normalised: String = decl.split_whitespace().collect::<Vec<_>>().join(" ");
    // Truncate at `{` so brace-delimited bodies don't pollute names.
    let truncated = if let Some(brace) = normalised.find('{') {
        &normalised[..brace]
    } else {
        &normalised
    };
    let lower = truncated.to_lowercase();

    let mut parents = Vec::new();
    let mut interfaces = Vec::new();

    if let Some(ext_pos) = lower.find(" extends ") {
        let after = &truncated[ext_pos + 9..];
        let end = after
            .to_lowercase()
            .find(" implements ")
            .unwrap_or(after.len());
        let segment = after[..end].trim();
        for name in segment.split(',') {
            let name = name.trim();
            if !name.is_empty() {
                parents.push(name.to_string());
            }
        }
    }

    if let Some(impl_pos) = lower.find(" implements ") {
        let after = &truncated[impl_pos + 12..];
        let segment = after.trim();
        for name in segment.split(',') {
            let name = name.trim();
            if !name.is_empty() {
                interfaces.push(name.to_string());
            }
        }
    }

    (parents, interfaces)
}

/// Parse a comma-separated parameter list into `(type_hint, $name)` pairs.
fn parse_params(params_str: &str) -> Vec<(Option<PhpType>, String)> {
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();

    for param in split_params(params_str) {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }

        // Each param looks like: [Type] [$name] [= default]
        // or: [Type] &$name, [Type] ...$name
        let tokens: Vec<&str> = param.split_whitespace().collect();

        // Find the variable name token (starts with $, &$, or ...$).
        let mut var_name = None;
        let mut type_parts = Vec::new();

        for tok in &tokens {
            if tok.starts_with('$') || tok.starts_with("&$") || tok.starts_with("...$") {
                let name = tok.trim_start_matches('&').trim_start_matches("...");
                // Strip default value.
                let name = if let Some(eq) = name.find('=') {
                    name[..eq].trim()
                } else {
                    name
                };
                var_name = Some(name.to_string());
                break;
            }
            // Skip `=` and default values.
            if *tok == "=" {
                break;
            }
            // Skip constructor promotion modifiers.
            match tok.to_lowercase().as_str() {
                "public" | "protected" | "private" | "static" | "readonly" => continue,
                _ => {}
            }
            type_parts.push(*tok);
        }

        if let Some(name) = var_name {
            let type_hint = if type_parts.is_empty() {
                None
            } else {
                Some(PhpType::parse(&type_parts.join(" ")))
            };
            result.push((type_hint, name));
        }
    }

    result
}

/// Extract the return type from the text after the closing `)`.
fn extract_return_type_from_decl(after_close: &str) -> Option<PhpType> {
    // Look for `: Type` pattern.
    let trimmed = after_close.trim_start();
    if !trimmed.starts_with(':') {
        return None;
    }

    let after_colon = trimmed[1..].trim_start();

    // Collect everything up to `{`, `;`, or end of string.
    let mut end = after_colon.len();
    let mut depth = 0i32;
    for (i, c) in after_colon.char_indices() {
        match c {
            '(' | '<' => depth += 1,
            ')' | '>' => depth -= 1,
            '{' | ';' if depth == 0 => {
                end = i;
                break;
            }
            _ => {}
        }
    }

    let ret_type = after_colon[..end].trim();
    if ret_type.is_empty() {
        None
    } else {
        Some(PhpType::parse(ret_type))
    }
}

/// Extract the type hint from a property or constant declaration.
/// Enrich an untyped property's [`SymbolInfo::type_hint`] by looking up
/// the property in the file's parsed class data.
///
/// When a property has no native type hint or docblock, the constructor-
/// inference pass in `extract_class_like_members` may have filled in a
/// type from `$this->prop = new ClassName()` or a promoted parameter
/// default.  This function finds that inferred type and copies it into
/// `sym` so that the generated `@var` tag uses the concrete class name
/// instead of `mixed`.
///
/// The type is shortened (leading namespace segments stripped) for
/// readability in the generated docblock.
fn enrich_property_type_from_class(
    sym: &mut SymbolInfo,
    content: &str,
    position: Position,
    local_classes: &[Arc<ClassInfo>],
) {
    // Extract the bare property name (strip the `$` prefix).
    let prop_name = sym
        .variable_name
        .as_ref()
        .and_then(|v| v.strip_prefix('$'))
        .unwrap_or("");
    if prop_name.is_empty() {
        return;
    }

    // Find the enclosing class by byte offset.
    let cursor_offset = position_to_byte_offset(content, position) as u32;
    let enclosing = local_classes
        .iter()
        .find(|cls| cls.start_offset <= cursor_offset && cursor_offset <= cls.end_offset);
    let Some(cls) = enclosing else {
        return;
    };

    // Look up the property.  Only use the type when it was inferred
    // (the native_type_hint is None — if it were set, the source-text
    // parser would already have extracted it).
    if let Some(prop) = cls.properties.iter().find(|p| p.name == prop_name)
        && prop.native_type_hint.is_none()
        && let Some(ref inferred) = prop.type_hint
    {
        sym.type_hint = Some(inferred.shorten());
    }
}

fn extract_property_type(decl: &str) -> Option<PhpType> {
    // Strip modifiers.
    let modifiers = [
        "public",
        "protected",
        "private",
        "static",
        "readonly",
        "var",
        "const",
        "final",
    ];
    let mut rest = decl;
    loop {
        rest = rest.trim_start();
        let mut found = false;
        for m in &modifiers {
            if rest.to_lowercase().starts_with(m) {
                let after = &rest[m.len()..];
                if after.is_empty() || after.starts_with(|c: char| c.is_whitespace()) {
                    rest = after;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            break;
        }
    }

    let rest = rest.trim_start();

    // If the next token starts with `$`, there's no type hint.
    if rest.starts_with('$') || rest.starts_with('=') {
        return None;
    }

    // For properties the name starts with `$`, so collect until `$`.
    // For constants the name is an identifier without `$`, so the type
    // is the first whitespace-delimited token (type hints never contain
    // spaces: `int`, `?string`, `int|string`, `A&B`, `\Foo`).
    let type_str: &str = if rest.contains('$') {
        // Property: collect everything before `$`, `=`, `;`, or `{`.
        let mut end = rest.len();
        for (i, c) in rest.char_indices() {
            if c == '$' || c == '=' || c == ';' || c == '{' {
                end = i;
                break;
            }
        }
        rest[..end].trim()
    } else {
        // Constant: the type (if present) is the first token, and the
        // constant name is the second.  When the first token is
        // immediately followed by `=` (i.e. there is no second token
        // before `=`), the constant is untyped and the first token is
        // actually the name.
        let mut tokens = rest.split_whitespace();
        let first = tokens.next().unwrap_or("");
        let second = tokens.next().unwrap_or("");
        if second.is_empty() || second.starts_with('=') {
            // Untyped constant: `const NAME = ...`
            ""
        } else {
            // Typed constant: `const int NAME = ...`
            first.trim()
        }
    };
    if type_str.is_empty() {
        None
    } else {
        Some(PhpType::parse(type_str))
    }
}

// ─── Type Enrichment Helpers ────────────────────────────────────────────────

/// Check whether a `PhpType` is a bare callable/Closure keyword (no signature).
fn is_callable_keyword(pt: &PhpType) -> bool {
    pt.is_callable()
}

/// Check whether a `PhpType` is a bare `array` keyword (no generic params).
fn is_bare_array(pt: &PhpType) -> bool {
    pt.is_bare_array()
}

/// Extract the callable display name from a `PhpType` that satisfies
/// `is_callable_keyword`.
fn callable_display_name(pt: &PhpType) -> &str {
    match pt {
        PhpType::Named(s) => s.as_str(),
        _ => "callable",
    }
}

/// Determine whether a native type hint "needs enrichment" via a PHPDoc
/// tag, and if so return the tag type string to use.
///
/// Returns `None` when the native type is fully expressed (scalars,
/// union types, intersection types, non-generic classes).
///
/// Returns `Some(tag_text)` when a PHPDoc tag should be emitted:
/// - Missing type → `"${N:mixed}"` (snippet) or `"mixed"` (plain)
/// - `array` → `"${N:array}"` (snippet) or `"array"` (plain)
/// - Class with templates → `"ClassName<${N:T1}, ${N+1:T2}>"` or plain equivalent
pub(crate) fn enrichment_snippet(
    type_hint: Option<&PhpType>,
    tab_stop: &mut u32,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    let pt = match type_hint {
        None => {
            let s = format!("${{{}:mixed}}", *tab_stop);
            *tab_stop += 1;
            return Some(s);
        }
        Some(t) => t,
    };

    // `void` is never enriched for return types (caller handles skip).
    // `array` always needs enrichment.
    if is_bare_array(pt) {
        let s = format!("array<${{{}:mixed}}>", *tab_stop);
        *tab_stop += 1;
        return Some(s);
    }

    // `Closure` / `callable` need a callable-signature placeholder.
    if is_callable_keyword(pt) {
        let name = callable_display_name(pt);
        let s = format!("({}(): ${{{}:mixed}})", name, *tab_stop);
        *tab_stop += 1;
        return Some(s);
    }

    // Union types — enrich individual callable / array parts.
    // Use union_members to correctly handle generic nesting
    // (e.g. `Collection<int|string, User>|null` must not be split on the inner `|`).
    let members = pt.union_members();
    if members.len() > 1 {
        let needs = members
            .iter()
            .any(|member| is_bare_array(member) || is_callable_keyword(member));
        if needs {
            let enriched_parts: Vec<String> = members
                .iter()
                .map(|member| {
                    if is_callable_keyword(member) {
                        let name = callable_display_name(member);
                        format!("({}(): ${{{}:mixed}})", name, {
                            let t = *tab_stop;
                            *tab_stop += 1;
                            t
                        })
                    } else if is_bare_array(member) {
                        let s = format!("array<${{{}:mixed}}>", *tab_stop);
                        *tab_stop += 1;
                        s
                    } else {
                        member.to_string()
                    }
                })
                .collect();
            return Some(enriched_parts.join("|"));
        }
        return None;
    }

    // Intersection types (&), nullable (?Type) — skip.
    if matches!(pt, PhpType::Intersection(_) | PhpType::Nullable(_)) {
        return None;
    }

    // Scalar / built-in types never have template parameters.
    if pt.is_scalar() {
        return None;
    }

    // Try to load the class and check for templates.
    if let Some(name) = pt.base_name()
        && let Some(cls) = class_loader(name)
        && !cls.template_params.is_empty()
    {
        let mut parts = Vec::new();
        for tp in &cls.template_params {
            parts.push(format!("${{{}:{}}}", *tab_stop, tp));
            *tab_stop += 1;
        }
        return Some(format!("{}<{}>", name, parts.join(", ")));
    }

    None
}

/// Structured version of [`enrichment_plain`] returning a [`PhpType`]
/// instead of a display string.
///
/// Use this when the enriched type needs to be compared structurally
/// (via [`PhpType::equivalent`]) rather than by string equality. The
/// plain-text callers that only need a display string should keep using
/// [`enrichment_plain`].
pub(crate) fn enrichment_plain_typed(
    type_hint: Option<&PhpType>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    let pt = match type_hint {
        None => return Some(PhpType::mixed()),
        Some(t) => t,
    };

    if is_bare_array(pt) {
        return Some(PhpType::generic_array_val(PhpType::mixed()));
    }

    if is_callable_keyword(pt) {
        let kind = callable_display_name(pt).to_string();
        return Some(PhpType::Callable {
            kind,
            params: vec![],
            return_type: Some(Box::new(PhpType::mixed())),
        });
    }

    // Union types — enrich individual callable / array parts.
    let members = pt.union_members();
    if members.len() > 1 {
        let needs = members
            .iter()
            .any(|member| is_bare_array(member) || is_callable_keyword(member));
        if needs {
            let enriched: Vec<PhpType> = members
                .iter()
                .map(|member| {
                    if is_callable_keyword(member) {
                        let kind = callable_display_name(member).to_string();
                        PhpType::Callable {
                            kind,
                            params: vec![],
                            return_type: Some(Box::new(PhpType::mixed())),
                        }
                    } else if is_bare_array(member) {
                        PhpType::generic_array_val(PhpType::mixed())
                    } else {
                        (*member).clone()
                    }
                })
                .collect();
            return Some(PhpType::Union(enriched));
        }
        return None;
    }

    if matches!(pt, PhpType::Intersection(_) | PhpType::Nullable(_)) {
        return None;
    }

    // Scalar / built-in types never have template parameters.
    if pt.is_scalar() {
        return None;
    }

    // Try to load the class and check for templates.
    if let Some(name) = pt.base_name()
        && let Some(cls) = class_loader(name)
        && !cls.template_params.is_empty()
    {
        let args: Vec<PhpType> = cls
            .template_params
            .iter()
            .map(|s| PhpType::Named(s.to_string()))
            .collect();
        return Some(PhpType::Generic(name.to_string(), args));
    }

    None
}

/// Plain-text version of `enrichment_snippet` (no tab stops).
///
/// Also used by tag completion (`build_phpdoc_completions`) to enrich
/// `@param`, `@return`, and `@var` type hints with template parameters.
///
/// Callable types are wrapped in parentheses for PHPDoc notation:
/// `(Closure(): mixed)`, `(callable(): mixed)`.
pub(crate) fn enrichment_plain(
    type_hint: Option<&PhpType>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    let typed = enrichment_plain_typed(type_hint, class_loader)?;

    // Callable types need parentheses in PHPDoc notation, which
    // PhpType::Display does not add.
    Some(enrichment_type_to_plain(&typed))
}

/// Format an enriched `PhpType` as a plain-text PHPDoc type string.
///
/// Callable types are wrapped in parentheses (`(Closure(): mixed)`)
/// to match PHPDoc inline callable notation. Union members are
/// formatted individually and joined with `|`.
fn enrichment_type_to_plain(ty: &PhpType) -> String {
    match ty {
        PhpType::Callable { .. } => format!("({})", ty),
        PhpType::Union(members) => members
            .iter()
            .map(enrichment_type_to_plain)
            .collect::<Vec<_>>()
            .join("|"),
        _ => ty.to_string(),
    }
}

// ─── Snippet / Plain Builder ────────────────────────────────────────────────

/// Build the full docblock as plain text (no tab stops).
///
/// Used by the `onTypeFormatting` path where snippets are not supported.
///
/// Only called for declaration-level contexts (`FunctionOrMethod`,
/// `ClassLike`, `Property`, `Constant`).  `Inline` and `Unknown` are
/// filtered out by the caller before we get here.
#[allow(clippy::too_many_arguments)]
fn build_docblock_plain(
    context: &DocblockContext,
    sym: &SymbolInfo,
    indent: &str,
    content: &str,
    position: Position,
    _use_map: &HashMap<String, String>,
    _file_namespace: &Option<String>,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoader<'_>,
) -> String {
    match context {
        DocblockContext::FunctionOrMethod => build_function_plain(
            sym,
            indent,
            content,
            position,
            _use_map,
            _file_namespace,
            local_classes,
            class_loader,
            function_loader,
        ),
        DocblockContext::ClassLike => build_class_plain(sym, indent, class_loader),
        DocblockContext::Property => build_property_plain(sym, indent, class_loader),
        DocblockContext::Constant => build_constant_plain(sym, indent, class_loader),
        // Inline and Unknown are early-returned by the caller.
        DocblockContext::Inline | DocblockContext::Unknown => String::new(),
    }
}

/// Build the full docblock snippet text.
///
/// The snippet uses VSCode-style tab stops (`$1`, `$2`, etc.) so the
/// user can tab through the placeholders.
///
/// Only called for declaration-level contexts (`FunctionOrMethod`,
/// `ClassLike`, `Property`, `Constant`).  `Inline` and `Unknown` are
/// filtered out by the caller before we get here.
#[allow(clippy::too_many_arguments)]
fn build_docblock_snippet(
    context: &DocblockContext,
    sym: &SymbolInfo,
    indent: &str,
    content: &str,
    position: Position,
    _use_map: &HashMap<String, String>,
    _file_namespace: &Option<String>,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoader<'_>,
) -> String {
    match context {
        DocblockContext::FunctionOrMethod => build_function_snippet(
            sym,
            indent,
            content,
            position,
            _use_map,
            _file_namespace,
            local_classes,
            class_loader,
            function_loader,
        ),
        DocblockContext::ClassLike => build_class_snippet(sym, indent, class_loader),
        DocblockContext::Property => build_property_snippet(sym, indent, class_loader),
        DocblockContext::Constant => build_constant_snippet(sym, indent, class_loader),
        // Inline and Unknown are early-returned by the caller.
        DocblockContext::Inline | DocblockContext::Unknown => String::new(),
    }
}

/// Build a docblock snippet for a function or method.
///
/// Only emits `@param` / `@return` tags when the native type needs
/// enrichment (missing, `array`, or class with `@template` params).
/// `@throws` tags are always emitted for uncaught exceptions.
/// Tags are grouped with blank `*` lines between groups.
/// Parameter names within the `@param` block are space-aligned.
#[allow(clippy::too_many_arguments)]
fn build_function_snippet(
    sym: &SymbolInfo,
    _indent: &str,
    content: &str,
    position: Position,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoader<'_>,
) -> String {
    let throws_ctx = ThrowsContext {
        class_loader,
        function_loader,
        use_map,
        file_namespace,
    };
    let uncaught = throws_analysis::find_uncaught_throw_types_with_context(
        content,
        position,
        Some(&throws_ctx),
    );

    let mut tab_stop = 1u32;

    // Collect @param tags that need enrichment.
    // Each entry is (snippet_type, display_len, escaped_name).
    let mut param_tags: Vec<(String, usize, String)> = Vec::new();
    for (type_hint, name) in &sym.params {
        if let Some(enriched) = enrichment_snippet(type_hint.as_ref(), &mut tab_stop, class_loader)
        {
            // Use the plain-text version to measure the rendered width for
            // alignment.  The snippet version contains `${N:...}` markers
            // that inflate its length.
            let display_len = enrichment_plain(type_hint.as_ref(), class_loader)
                .map(|p| p.len())
                .unwrap_or(enriched.len());
            // Escape `$` in PHP parameter names so the snippet parser
            // does not treat them as snippet variables.
            param_tags.push((enriched, display_len, name.replace('$', "\\$")));
        }
    }

    // Determine @return enrichment.
    // Constructors never get @return (they implicitly return the class).
    let is_constructor = sym
        .method_name
        .as_ref()
        .is_some_and(|n| n.eq_ignore_ascii_case("__construct"));
    let is_void = sym.return_type.as_ref().is_some_and(|r| r.is_void());
    let return_tag = if is_void || is_constructor {
        None
    } else {
        // Try body-based inference first (produces richer types like
        // `list<string>` instead of `array<mixed>`).
        let body_inferred = crate::code_actions::phpstan::fix_return_type::enrichment_return_type(
            content,
            position,
            local_classes,
            class_loader,
            function_loader,
        );
        let inferred = body_inferred.filter(|t| {
            !t.is_void()
                && !t.is_mixed()
                && sym.return_type.as_ref().is_none_or(|s| !t.equivalent(s))
        });
        // Fall back to signature-based enrichment when body inference
        // doesn't produce anything useful.
        if let Some(t) = inferred {
            Some(t.to_string())
        } else {
            enrichment_snippet(sym.return_type.as_ref(), &mut tab_stop, class_loader)
        }
    };

    let has_throws = !uncaught.is_empty();

    let has_any_tag = !param_tags.is_empty() || has_throws || return_tag.is_some();

    let mut lines = Vec::new();
    lines.push("/**".to_string());

    if !has_any_tag {
        // No tags — emit a summary-only skeleton.
        lines.push(" * ${1}".to_string());
    }

    // @param block with space-aligned names.
    if !param_tags.is_empty() {
        let max_display_len = param_tags.iter().map(|(_, dl, _)| *dl).max().unwrap_or(0);
        for (type_str, display_len, name) in &param_tags {
            let padding = " ".repeat(max_display_len - display_len);
            lines.push(format!(" * @param {}{} {}", type_str, padding, name));
        }
    }

    // @throws block (blank separator from preceding group).
    if has_throws {
        if !param_tags.is_empty() {
            lines.push(" *".to_string());
        }
        for exc in &uncaught {
            let exc_str = exc.to_string();
            let display = crate::util::short_name(&exc_str);
            lines.push(format!(" * @throws {}", display));
        }
    }

    // @return tag (blank separator from preceding group).
    if let Some(ret) = return_tag {
        if !param_tags.is_empty() || has_throws {
            lines.push(" *".to_string());
        }
        lines.push(format!(" * @return {}", ret));
    }

    lines.push(" */".to_string());
    lines.join("\n")
}

/// Build a plain-text docblock for a function or method (no tab stops).
///
/// Same enrichment logic as the snippet builder, but without tab stops.
#[allow(clippy::too_many_arguments)]
fn build_function_plain(
    sym: &SymbolInfo,
    indent: &str,
    content: &str,
    position: Position,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
    local_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoader<'_>,
) -> String {
    let throws_ctx = ThrowsContext {
        class_loader,
        function_loader,
        use_map,
        file_namespace,
    };
    let uncaught = throws_analysis::find_uncaught_throw_types_with_context(
        content,
        position,
        Some(&throws_ctx),
    );

    // Collect @param tags that need enrichment.
    let mut param_tags: Vec<(String, String)> = Vec::new();
    for (type_hint, name) in &sym.params {
        if let Some(enriched) = enrichment_plain(type_hint.as_ref(), class_loader) {
            param_tags.push((enriched, name.clone()));
        }
    }

    // Constructors never get @return.
    let is_constructor = sym
        .method_name
        .as_ref()
        .is_some_and(|n| n.eq_ignore_ascii_case("__construct"));
    let is_void = sym.return_type.as_ref().is_some_and(|r| r.is_void());
    let return_tag = if is_void || is_constructor {
        None
    } else {
        // Try body-based inference first (produces richer types like
        // `list<string>` instead of `array<mixed>`).
        let body_inferred = crate::code_actions::phpstan::fix_return_type::enrichment_return_type(
            content,
            position,
            local_classes,
            class_loader,
            function_loader,
        );
        // Filter out types that don't need a @return tag (void, scalars
        // that match the native hint exactly).
        let inferred = body_inferred.filter(|t| {
            !t.is_void()
                && !t.is_mixed()
                && sym.return_type.as_ref().is_none_or(|s| !t.equivalent(s))
        });
        // Fall back to signature-based enrichment when body inference
        // doesn't produce anything useful.
        inferred
            .map(|t| t.to_string())
            .or_else(|| enrichment_plain(sym.return_type.as_ref(), class_loader))
    };

    let has_throws = !uncaught.is_empty();

    let has_any_tag = !param_tags.is_empty() || has_throws || return_tag.is_some();

    let mut lines = Vec::new();
    lines.push(format!("{}/**", indent));

    if !has_any_tag {
        lines.push(format!("{} * ", indent));
    }

    if !param_tags.is_empty() {
        let max_type_len = param_tags.iter().map(|(t, _)| t.len()).max().unwrap_or(0);
        for (type_str, name) in &param_tags {
            let padding = " ".repeat(max_type_len - type_str.len());
            lines.push(format!(
                "{} * @param {}{} {}",
                indent, type_str, padding, name
            ));
        }
    }

    if has_throws {
        if !param_tags.is_empty() {
            lines.push(format!("{} *", indent));
        }
        for exc in &uncaught {
            let exc_str = exc.to_string();
            let display = crate::util::short_name(&exc_str).to_string();
            lines.push(format!("{} * @throws {}", indent, display));
        }
    }

    if let Some(ret) = return_tag {
        if !param_tags.is_empty() || has_throws {
            lines.push(format!("{} *", indent));
        }
        lines.push(format!("{} * @return {}", indent, ret));
    }

    lines.push(format!("{} */", indent));
    lines.join("\n") + "\n"
}

/// Build a plain-text docblock for a class (no tab stops).
///
/// Generates `@extends` / `@implements` tags when the parent or
/// interface has `@template` parameters.
fn build_class_plain(
    sym: &SymbolInfo,
    indent: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let mut tag_lines = Vec::new();

    for parent in &sym.extends_names {
        if let Some(cls) = class_loader(parent)
            && !cls.template_params.is_empty()
        {
            let parts: Vec<&str> = cls.template_params.iter().map(|s| s.as_str()).collect();
            tag_lines.push(format!(
                "{} * @extends {}<{}>",
                indent,
                parent,
                parts.join(", ")
            ));
        }
    }

    for iface in &sym.implements_names {
        if let Some(cls) = class_loader(iface)
            && !cls.template_params.is_empty()
        {
            let parts: Vec<&str> = cls.template_params.iter().map(|s| s.as_str()).collect();
            tag_lines.push(format!(
                "{} * @implements {}<{}>",
                indent,
                iface,
                parts.join(", ")
            ));
        }
    }

    if tag_lines.is_empty() {
        format!("{indent}/**\n{indent} * \n{indent} */\n")
    } else {
        let mut lines = Vec::new();
        lines.push(format!("{}/**", indent));
        lines.extend(tag_lines);
        lines.push(format!("{} */", indent));
        lines.join("\n") + "\n"
    }
}

/// Build a plain-text docblock for a property (no tab stops).
///
/// Emits a single-line `/** @var Type */` format.
fn build_property_plain(
    sym: &SymbolInfo,
    indent: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let var_type = property_var_type_plain(sym.type_hint.as_ref(), class_loader);
    format!("{indent}/** @var {var_type} */\n")
}

/// Build a plain-text docblock for a constant (no tab stops).
///
/// Emits a single-line `/** @var Type */` format.
fn build_constant_plain(
    sym: &SymbolInfo,
    indent: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let var_type = property_var_type_plain(sym.type_hint.as_ref(), class_loader);
    format!("{indent}/** @var {var_type} */\n")
}

/// Build a docblock snippet for a class, interface, trait, or enum.
///
/// Generates `@extends` / `@implements` tags with tab-stop placeholders
/// when the parent or interface has `@template` parameters.
fn build_class_snippet(
    sym: &SymbolInfo,
    _indent: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let mut tag_lines = Vec::new();
    let mut tab_stop = 1u32;

    for parent in &sym.extends_names {
        if let Some(cls) = class_loader(parent)
            && !cls.template_params.is_empty()
        {
            let mut parts = Vec::new();
            for tp in &cls.template_params {
                parts.push(format!("${{{}:{}}}", tab_stop, tp));
                tab_stop += 1;
            }
            tag_lines.push(format!(" * @extends {}<{}>", parent, parts.join(", ")));
        }
    }

    for iface in &sym.implements_names {
        if let Some(cls) = class_loader(iface)
            && !cls.template_params.is_empty()
        {
            let mut parts = Vec::new();
            for tp in &cls.template_params {
                parts.push(format!("${{{}:{}}}", tab_stop, tp));
                tab_stop += 1;
            }
            tag_lines.push(format!(" * @implements {}<{}>", iface, parts.join(", ")));
        }
    }

    let mut lines = Vec::new();
    lines.push("/**".to_string());

    if tag_lines.is_empty() {
        // No template tags — emit a summary-only skeleton.
        lines.push(" * ${1}".to_string());
    } else {
        lines.extend(tag_lines);
    }

    lines.push(" */".to_string());
    lines.join("\n")
}

/// Build a docblock snippet for a property.
///
/// Emits a single-line `/** @var Type */` format.
/// For missing types, the type is a tab-stop placeholder.
/// For classes with templates, template names are tab-stop placeholders.
fn build_property_snippet(
    sym: &SymbolInfo,
    _indent: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let mut tab_stop = 1u32;
    let var_type = property_var_type_snippet(sym.type_hint.as_ref(), &mut tab_stop, class_loader);
    format!("/** @var {} */", var_type)
}

/// Build a docblock snippet for a constant.
///
/// Emits a single-line `/** @var Type */` format.
fn build_constant_snippet(
    sym: &SymbolInfo,
    _indent: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    let mut tab_stop = 1u32;
    let var_type = property_var_type_snippet(sym.type_hint.as_ref(), &mut tab_stop, class_loader);
    format!("/** @var {} */", var_type)
}

/// Attempt to infer the type of an inline variable assignment using the
/// hover type-resolution pipeline.
///
/// Given `$var = ['']`, this resolves to `list<string>` by delegating
/// to the same `resolve_variable_type` that powers hover.
pub(crate) fn infer_inline_variable_type(
    sym: &SymbolInfo,
    content: &str,
    position: Position,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoaderFn<'_>,
) -> Option<PhpType> {
    let var_name = sym.variable_name.as_deref()?;

    // The cursor is at the `/**` trigger, which is above the variable
    // assignment.  We need an offset that falls within the assignment
    // line so that the resolution pipeline can find the assignment.
    let trigger_offset = position_to_byte_offset(content, position);

    // The `/**` trigger may be unclosed (completion path) or already
    // closed as `/** */` (on-enter path).  An unclosed `/**` causes
    // the PHP parser to swallow the assignment line into a comment,
    // making it invisible to the AST.  Fix this by replacing the
    // docblock trigger text with spaces so the parser sees the
    // assignment.
    let patched = patch_docblock_trigger(content, trigger_offset);
    let effective_content = patched.as_deref().unwrap_or(content);

    // Place the cursor after the assignment's semicolon so the
    // resolution pipeline (which scans backwards) can find it.
    let cursor_offset = effective_content[trigger_offset..]
        .find(';')
        .map(|off| trigger_offset + off + 1)
        .unwrap_or(trigger_offset + 1) as u32;

    let current_class = crate::util::find_class_at_offset(all_classes, cursor_offset);

    crate::completion::variable::resolution::resolve_variable_php_type(
        var_name,
        effective_content,
        cursor_offset,
        current_class,
        all_classes,
        class_loader,
        crate::completion::resolver::Loaders::with_function(function_loader),
    )
}

/// Replace the `/**` (or `/** */`) block around `trigger_offset` with
/// spaces so the PHP parser does not swallow the next line into a
/// docblock comment.
///
/// Returns `None` when no patching is needed (no `/**` found).
fn patch_docblock_trigger(content: &str, trigger_offset: usize) -> Option<String> {
    // Walk backwards from the trigger to find the start of `/**`.
    let before = &content[..trigger_offset];
    let line_start = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let trigger_line = &content[line_start..];

    // Find the `/**` on this line.
    let doc_start_in_line = trigger_line.find("/**")?;
    let abs_doc_start = line_start + doc_start_in_line;

    // Find the end of the docblock: either `*/` on the same or next
    // lines, or end-of-line if unclosed.
    let after_open = abs_doc_start + 3; // skip `/**`
    let abs_doc_end = if let Some(close) = content[after_open..].find("*/") {
        after_open + close + 2
    } else {
        // Unclosed — blank out to end of the line containing `/**`.
        content[abs_doc_start..]
            .find('\n')
            .map(|i| abs_doc_start + i)
            .unwrap_or(content.len())
    };

    let mut patched = content.to_string();
    // Replace the docblock region with spaces (preserving byte offsets).
    patched.replace_range(
        abs_doc_start..abs_doc_end,
        &" ".repeat(abs_doc_end - abs_doc_start),
    );
    Some(patched)
}

/// Compute the `@var` type string for a property/constant snippet.
///
/// - Missing type → `${N:mixed}` tab stop
/// - `array` → `${N:array}` tab stop
/// - Class with templates → `ClassName<${N:T1}, ...>` tab stops
/// - Other → literal type string
fn property_var_type_snippet(
    type_hint: Option<&PhpType>,
    tab_stop: &mut u32,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    match type_hint {
        None => {
            let s = format!("${{{}:mixed}}", *tab_stop);
            *tab_stop += 1;
            s
        }
        Some(th) if th.is_bare_array() => {
            let s = format!("${{{}:array}}", *tab_stop);
            *tab_stop += 1;
            s
        }
        Some(th) => {
            let shortened = th.shorten();
            // Callable types get a signature placeholder.
            if th.is_callable() {
                let s = format!("(${{{}:{}()}})", *tab_stop, shortened);
                *tab_stop += 1;
                return s;
            }
            if let Some(name) = shortened.base_name()
                && let Some(cls) = class_loader(name)
                && !cls.template_params.is_empty()
            {
                let mut parts = Vec::new();
                for tp in &cls.template_params {
                    parts.push(format!("${{{}:{}}}", *tab_stop, tp));
                    *tab_stop += 1;
                }
                return format!("{}<{}>", name, parts.join(", "));
            }
            shortened.to_string()
        }
    }
}

/// Compute the `@var` type string for a property/constant in plain text.
fn property_var_type_plain(
    type_hint: Option<&PhpType>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> String {
    match type_hint {
        None => PhpType::mixed().to_string(),
        Some(th) if th.is_bare_array() => "array".to_string(),
        Some(th) => {
            let shortened = th.shorten();
            if th.is_callable() {
                return format!("({}())", shortened);
            }
            if let Some(name) = shortened.base_name()
                && let Some(cls) = class_loader(name)
                && !cls.template_params.is_empty()
            {
                let parts: Vec<&str> = cls.template_params.iter().map(|s| s.as_str()).collect();
                return format!("{}<{}>", name, parts.join(", "));
            }
            shortened.to_string()
        }
    }
}

// ─── Import Edits ───────────────────────────────────────────────────────────

/// Build additional text edits for auto-importing exception types
/// referenced in `@throws` tags.
fn build_throws_import_edits(
    content: &str,
    position: Position,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
    context: &DocblockContext,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    function_loader: FunctionLoader<'_>,
) -> Vec<TextEdit> {
    if !matches!(context, DocblockContext::FunctionOrMethod) {
        return Vec::new();
    }

    let throws_ctx = ThrowsContext {
        class_loader,
        function_loader,
        use_map,
        file_namespace,
    };
    let uncaught = throws_analysis::find_uncaught_throw_types_with_context(
        content,
        position,
        Some(&throws_ctx),
    );
    if uncaught.is_empty() {
        return Vec::new();
    }

    let use_block = analyze_use_block(content);
    let mut edits = Vec::new();

    for exc in &uncaught {
        // Exception types are already resolved to FQNs by
        // `find_uncaught_throw_types_with_context` — do not re-resolve.
        let fqn = exc.to_string();
        if !throws_analysis::has_use_import(content, &fqn)
            && let Some(edit) = build_use_edit(&fqn, &use_block, file_namespace)
        {
            edits.extend(edit);
        }
    }

    edits
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atom::atom;

    /// Dummy class loader that returns `None` for all lookups.
    fn no_classes(_name: &str) -> Option<Arc<ClassInfo>> {
        None
    }

    // ── Trigger detection ───────────────────────────────────────────────

    #[test]
    fn detects_trigger_at_line_start() {
        let content = "<?php\n/**";
        let pos = Position {
            line: 1,
            character: 3,
        };
        let result = detect_docblock_trigger(content, pos);
        assert!(result.is_some(), "Should detect /** trigger");
        let (range, indent) = result.unwrap();
        assert_eq!(indent, "");
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.character, 3);
    }

    #[test]
    fn detects_trigger_with_indentation() {
        let content = "<?php\nclass Foo {\n    /**";
        let pos = Position {
            line: 2,
            character: 7,
        };
        let result = detect_docblock_trigger(content, pos);
        assert!(result.is_some(), "Should detect indented /** trigger");
        let (_, indent) = result.unwrap();
        assert_eq!(indent, "    ");
    }

    #[test]
    fn rejects_trigger_inside_existing_docblock() {
        let content = "<?php\n/**\n * @param\n */\nfunction test() {}";
        let pos = Position {
            line: 1,
            character: 3,
        };
        let result = detect_docblock_trigger(content, pos);
        assert!(
            result.is_none(),
            "Should not trigger inside existing docblock"
        );
    }

    #[test]
    fn rejects_trigger_with_closing_on_same_line() {
        let content = "<?php\n/** @var int */";
        let pos = Position {
            line: 1,
            character: 3,
        };
        let result = detect_docblock_trigger(content, pos);
        assert!(
            result.is_none(),
            "Should not trigger when */ is on the same line"
        );
    }

    #[test]
    fn rejects_trigger_with_code_before() {
        let content = "<?php\n$x = /**";
        let pos = Position {
            line: 1,
            character: 8,
        };
        let result = detect_docblock_trigger(content, pos);
        assert!(
            result.is_none(),
            "Should not trigger when code precedes /**"
        );
    }

    #[test]
    fn no_panic_on_multibyte_characters() {
        // "ń" is 2 bytes in UTF-8 but 1 UTF-16 code unit.
        // The cursor is after the closing paren, UTF-16 column 32.
        // Using that as a byte offset would land inside "ń" and panic.
        let content = "<?php\n                $table->string(ń);";
        let pos = Position {
            line: 1,
            character: 32,
        };
        // Must not panic — should simply return None.
        let result = detect_docblock_trigger(content, pos);
        assert!(result.is_none());
    }

    // ── Declaration classification ──────────────────────────────────────

    #[test]
    fn classifies_function() {
        let decl = "function test(string $name): void {}";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::FunctionOrMethod
        ));
    }

    #[test]
    fn classifies_method() {
        let decl = "    public function test(): int {}";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::FunctionOrMethod
        ));
    }

    #[test]
    fn classifies_abstract_method() {
        let decl = "    abstract public function test(): int;";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::FunctionOrMethod
        ));
    }

    #[test]
    fn classifies_class() {
        let decl = "class Foo extends Bar {}";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::ClassLike
        ));
    }

    #[test]
    fn classifies_abstract_class() {
        let decl = "abstract class Foo {}";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::ClassLike
        ));
    }

    #[test]
    fn classifies_interface() {
        let decl = "interface Foo {}";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::ClassLike
        ));
    }

    #[test]
    fn classifies_trait() {
        let decl = "trait Foo {}";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::ClassLike
        ));
    }

    #[test]
    fn classifies_enum() {
        let decl = "enum Status: string {}";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::ClassLike
        ));
    }

    #[test]
    fn classifies_property() {
        let decl = "    public string $name;";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::Property
        ));
    }

    #[test]
    fn classifies_untyped_property() {
        let decl = "    public $name;";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::Property
        ));
    }

    #[test]
    fn classifies_constant() {
        let decl = "    const FOO = 'bar';";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::Constant
        ));
    }

    #[test]
    fn classifies_public_constant() {
        let decl = "    public const int MAX = 100;";
        assert!(matches!(
            classify_declaration(decl),
            DocblockContext::Constant
        ));
    }

    // ── Parameter parsing ───────────────────────────────────────────────

    #[test]
    fn parses_params_with_types() {
        let text = "function test(string $name, int $age): void {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params.len(), 2);
        assert_eq!(
            info.params[0],
            (Some(PhpType::parse("string")), "$name".to_string())
        );
        assert_eq!(
            info.params[1],
            (Some(PhpType::parse("int")), "$age".to_string())
        );
        assert_eq!(info.return_type, Some(PhpType::parse("void")));
    }

    #[test]
    fn parses_params_without_types() {
        let text = "function test($name, $age) {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params.len(), 2);
        assert_eq!(info.params[0], (None, "$name".to_string()));
        assert_eq!(info.params[1], (None, "$age".to_string()));
    }

    #[test]
    fn parses_nullable_type() {
        let text = "function test(?string $name): ?int {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params[0].0, Some(PhpType::parse("?string")));
        assert_eq!(info.return_type, Some(PhpType::parse("?int")));
    }

    #[test]
    fn parses_union_type() {
        let text = "function test(string|int $value): string|false {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params[0].0, Some(PhpType::parse("string|int")));
        assert_eq!(info.return_type, Some(PhpType::parse("string|false")));
    }

    #[test]
    fn parses_variadic_param() {
        let text = "function test(string ...$names): void {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params.len(), 1);
        assert_eq!(
            info.params[0],
            (Some(PhpType::parse("string")), "$names".to_string())
        );
    }

    #[test]
    fn parses_reference_param() {
        let text = "function test(array &$data): void {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params.len(), 1);
        assert_eq!(
            info.params[0],
            (Some(PhpType::parse("array")), "$data".to_string())
        );
    }

    #[test]
    fn parses_param_with_default() {
        let text = "function test(string $name = 'world'): void {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params.len(), 1);
        assert_eq!(
            info.params[0],
            (Some(PhpType::parse("string")), "$name".to_string())
        );
    }

    #[test]
    fn parses_no_params() {
        let text = "function test(): void {}";
        let info = parse_declaration_info(text);
        assert!(info.params.is_empty());
        assert_eq!(info.return_type, Some(PhpType::parse("void")));
    }

    #[test]
    fn parses_property_type() {
        let text = "    public string $name;";
        let info = parse_declaration_info(text);
        assert_eq!(info.type_hint, Some(PhpType::parse("string")));
    }

    #[test]
    fn parses_readonly_property_type() {
        let text = "    public readonly string $name;";
        let info = parse_declaration_info(text);
        assert_eq!(info.type_hint, Some(PhpType::parse("string")));
    }

    #[test]
    fn parses_typed_constant_extracts_only_type() {
        let text = "    const int COW = 0;";
        let info = parse_declaration_info(text);
        assert_eq!(info.type_hint, Some(PhpType::parse("int")));
    }

    #[test]
    fn parses_public_typed_constant_extracts_only_type() {
        let text = "    public const string NAME = 'foo';";
        let info = parse_declaration_info(text);
        assert_eq!(info.type_hint, Some(PhpType::parse("string")));
    }

    #[test]
    fn parses_untyped_constant_has_no_type() {
        let text = "    const MAX = 100;";
        let info = parse_declaration_info(text);
        assert_eq!(info.type_hint, None);
    }

    #[test]
    fn parses_promoted_param_type() {
        let text = "function __construct(public readonly bool $selected) {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.params.len(), 1);
        assert_eq!(
            info.params[0],
            (Some(PhpType::parse("bool")), "$selected".to_string())
        );
    }

    #[test]
    fn parses_class_extends() {
        let text = "class Child extends Base {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.extends_names, vec!["Base"]);
        assert!(info.implements_names.is_empty());
    }

    #[test]
    fn parses_class_implements() {
        let text = "class Foo implements Bar, Baz {}";
        let info = parse_declaration_info(text);
        assert!(info.extends_names.is_empty());
        assert_eq!(info.implements_names, vec!["Bar", "Baz"]);
    }

    #[test]
    fn parses_class_extends_and_implements() {
        let text = "class Child extends Base implements Iface {}";
        let info = parse_declaration_info(text);
        assert_eq!(info.extends_names, vec!["Base"]);
        assert_eq!(info.implements_names, vec!["Iface"]);
    }

    // ── Type enrichment ─────────────────────────────────────────────────

    #[test]
    fn enrichment_missing_type_produces_mixed() {
        let mut ts = 1;
        let result = enrichment_snippet(None, &mut ts, &no_classes);
        assert_eq!(result, Some("${1:mixed}".to_string()));
        assert_eq!(ts, 2);
    }

    #[test]
    fn enrichment_array_produces_array_tabstop() {
        let mut ts = 1;
        let hint = PhpType::parse("array");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert_eq!(result, Some("array<${1:mixed}>".to_string()));
        assert_eq!(ts, 2);
    }

    #[test]
    fn enrichment_scalar_returns_none() {
        let mut ts = 1;
        let hint = PhpType::parse("string");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert!(result.is_none());
        assert_eq!(ts, 1, "tab stop should not advance for skipped types");
    }

    #[test]
    fn enrichment_union_without_array_returns_none() {
        let mut ts = 1;
        let hint = PhpType::parse("string|int");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert!(result.is_none());
    }

    #[test]
    fn enrichment_union_with_array_enriches_parts() {
        let mut ts = 1;
        let hint = PhpType::parse("array|string");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert_eq!(result, Some("array<${1:mixed}>|string".to_string()));
    }

    #[test]
    fn enrichment_union_with_closure_enriches_parts() {
        let mut ts = 1;
        let hint = PhpType::parse("Closure|null");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert_eq!(result, Some("(Closure(): ${1:mixed})|null".to_string()));
    }

    #[test]
    fn enrichment_nullable_returns_none() {
        let mut ts = 1;
        let hint = PhpType::parse("?string");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert!(result.is_none());
    }

    #[test]
    fn enrichment_void_returns_none() {
        let mut ts = 1;
        let hint = PhpType::parse("void");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert!(result.is_none());
    }

    #[test]
    fn enrichment_closure_produces_callable_placeholder() {
        let mut ts = 1;
        let hint = PhpType::parse("Closure");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert_eq!(result, Some("(Closure(): ${1:mixed})".to_string()));
        assert_eq!(ts, 2);
    }

    #[test]
    fn enrichment_callable_produces_callable_placeholder() {
        let mut ts = 1;
        let hint = PhpType::parse("callable");
        let result = enrichment_snippet(Some(&hint), &mut ts, &no_classes);
        assert_eq!(result, Some("(callable(): ${1:mixed})".to_string()));
        assert_eq!(ts, 2);
    }

    #[test]
    fn enrichment_class_without_templates_returns_none() {
        let mut ts = 1;
        // Class exists but has no template params.
        let loader = |name: &str| -> Option<Arc<ClassInfo>> {
            if name == "User" {
                Some(Arc::new(ClassInfo {
                    name: crate::atom::atom("User"),
                    ..Default::default()
                }))
            } else {
                None
            }
        };
        let hint = PhpType::parse("User");
        let result = enrichment_snippet(Some(&hint), &mut ts, &loader);
        assert!(result.is_none());
    }

    #[test]
    fn enrichment_class_with_templates_produces_generic() {
        let mut ts = 1;
        let loader = |name: &str| -> Option<Arc<ClassInfo>> {
            if name == "Collection" {
                Some(Arc::new(ClassInfo {
                    name: crate::atom::atom("Collection"),
                    template_params: vec![atom("TKey"), atom("TValue")],
                    ..Default::default()
                }))
            } else {
                None
            }
        };
        let hint = PhpType::parse("Collection");
        let result = enrichment_snippet(Some(&hint), &mut ts, &loader);
        assert_eq!(
            result,
            Some("Collection<${1:TKey}, ${2:TValue}>".to_string())
        );
        assert_eq!(ts, 3);
    }

    // ── Snippet generation ──────────────────────────────────────────────

    #[test]
    fn generates_function_snippet_no_indent_in_continuation() {
        // Snippet continuation lines must NOT include the base indent.
        // The editor auto-indents multi-line completion snippets to
        // match the text-edit range's start column.
        let sym = SymbolInfo {
            params: vec![(None, "$data".to_string())],
            return_type: Some(PhpType::parse("void")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            "    ",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        // No line should start with the base indent "    ".
        for (i, line) in snippet.lines().enumerate() {
            if i == 0 {
                continue; // first line is just "/**"
            }
            assert!(
                !line.starts_with("    "),
                "Snippet line {} should not have base indent, got: {:?}",
                i,
                line
            );
        }
    }

    #[test]
    fn snippet_escapes_dollar_in_param_names() {
        let sym = SymbolInfo {
            params: vec![(None, "$data".to_string())],
            return_type: Some(PhpType::parse("void")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            "",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        // The `$` in `$data` must be escaped as `\$` so the snippet
        // parser does not treat it as a snippet variable.
        assert!(
            snippet.contains("\\$data"),
            "$ in param name should be escaped, got:\n{}",
            snippet
        );
        assert!(
            !snippet.contains(" $data"),
            "Unescaped $data should not appear, got:\n{}",
            snippet
        );
    }

    #[test]
    fn generates_function_snippet_skips_fully_typed_params() {
        // string and int are fully typed — no @param needed.
        // User is a class without templates — no @param needed.
        // Only @return for User should be skipped (no templates).
        let sym = SymbolInfo {
            params: vec![
                (Some(PhpType::parse("string")), "$name".to_string()),
                (Some(PhpType::parse("int")), "$age".to_string()),
            ],
            return_type: Some(PhpType::parse("User")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let loader = |name: &str| -> Option<Arc<ClassInfo>> {
            if name == "User" {
                Some(Arc::new(ClassInfo {
                    name: crate::atom::atom("User"),
                    ..Default::default()
                }))
            } else {
                None
            }
        };
        let snippet = build_function_snippet(
            &sym,
            "",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &loader,
            None,
        );
        // All params are fully typed, return type is non-template class.
        // Should be a summary-only skeleton with no tags.
        assert!(
            !snippet.contains("@param"),
            "Fully-typed params should not get @param, got:\n{}",
            snippet
        );
        assert!(
            !snippet.contains("@return"),
            "Non-template class return should not get @return, got:\n{}",
            snippet
        );
        // Summary-only skeleton: /**\n * ${1}\n */
        assert!(snippet.contains("${1}"), "Should have summary tab stop");
    }

    #[test]
    fn generates_function_snippet_for_untyped_params() {
        let sym = SymbolInfo {
            params: vec![
                (None, "$data".to_string()),
                (Some(PhpType::parse("string")), "$name".to_string()),
            ],
            return_type: Some(PhpType::parse("void")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            "    ",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        // Only $data (untyped) should get @param, not $name (string).
        assert!(
            snippet.contains("@param ${"),
            "Untyped param should get @param with mixed placeholder, got:\n{}",
            snippet
        );
        assert!(
            snippet.contains("mixed"),
            "Untyped param should have mixed placeholder, got:\n{}",
            snippet
        );
        assert!(
            snippet.contains("$data"),
            "Should contain $data, got:\n{}",
            snippet
        );
        assert!(
            !snippet.contains("$name"),
            "Fully-typed $name should not appear in @param, got:\n{}",
            snippet
        );
        assert!(!snippet.contains("@return"), "void should not have @return");
    }

    #[test]
    fn generates_function_snippet_for_array_param_and_return() {
        let sym = SymbolInfo {
            params: vec![(Some(PhpType::parse("array")), "$items".to_string())],
            return_type: Some(PhpType::parse("array")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            "    ",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        assert!(snippet.contains("@param"), "array param should get @param");
        assert!(snippet.contains("$items"), "Should reference $items");
        assert!(
            snippet.contains("@return"),
            "array return should get @return"
        );
    }

    #[test]
    fn generates_void_function_snippet_without_return() {
        let sym = SymbolInfo {
            params: vec![(None, "$name".to_string())],
            return_type: Some(PhpType::parse("void")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            "    ",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        assert!(snippet.contains("@param"));
        assert!(
            !snippet.contains("@return"),
            "void functions should not have @return"
        );
    }

    #[test]
    fn paramless_void_generates_summary_skeleton() {
        let sym = SymbolInfo {
            params: vec![],
            return_type: Some(PhpType::parse("void")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            "    ",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        assert!(
            !snippet.is_empty(),
            "Paramless void function should produce a summary skeleton"
        );
        assert!(snippet.starts_with("/**"));
        assert!(
            snippet.contains("${1}"),
            "Should have summary tab stop when no tags"
        );
        assert!(!snippet.contains("@param"));
        assert!(!snippet.contains("@return"));
        // Should be exactly 3 lines: /**, * ${1}, */
        let line_count = snippet.lines().count();
        assert_eq!(
            line_count, 3,
            "Summary skeleton should be 3 lines, got:\n{}",
            snippet
        );
    }

    #[test]
    fn generates_class_snippet_without_templates() {
        let sym = SymbolInfo::default();
        let snippet = build_class_snippet(&sym, "    ", &no_classes);
        assert!(snippet.starts_with("/**"));
        assert!(
            snippet.contains("${1}"),
            "No-template class should have summary tab stop"
        );
        assert!(snippet.ends_with(" */"));
        assert!(!snippet.contains("@extends"));
        assert!(!snippet.contains("@implements"));
        // Should be exactly 3 lines: /**, * ${1}, */
        let line_count = snippet.lines().count();
        assert_eq!(
            line_count, 3,
            "Summary skeleton should be 3 lines, got:\n{}",
            snippet
        );
    }

    #[test]
    fn generates_class_snippet_with_templated_parent() {
        let sym = SymbolInfo {
            extends_names: vec!["Factory".to_string()],
            ..Default::default()
        };
        let loader = |name: &str| -> Option<Arc<ClassInfo>> {
            if name == "Factory" {
                Some(Arc::new(ClassInfo {
                    name: crate::atom::atom("Factory"),
                    template_params: vec![atom("TModel")],
                    ..Default::default()
                }))
            } else {
                None
            }
        };
        let snippet = build_class_snippet(&sym, " ", &loader);
        assert!(
            snippet.contains("@extends Factory<${1:TModel}>"),
            "Should contain @extends with template tab stop, got:\n{}",
            snippet
        );
        // No summary line when tags are present.
        assert!(
            !snippet.contains("* ${"),
            "Should not have a summary placeholder when tags exist, got:\n{}",
            snippet
        );
        // No blank * separator lines.
        assert!(
            !snippet.lines().any(|l| l.trim() == "*"),
            "Should not have blank separator lines, got:\n{}",
            snippet
        );
    }

    #[test]
    fn generates_class_snippet_with_templated_interface() {
        let sym = SymbolInfo {
            implements_names: vec!["Comparable".to_string()],
            ..Default::default()
        };
        let loader = |name: &str| -> Option<Arc<ClassInfo>> {
            if name == "Comparable" {
                Some(Arc::new(ClassInfo {
                    name: crate::atom::atom("Comparable"),
                    template_params: vec![atom("T")],
                    ..Default::default()
                }))
            } else {
                None
            }
        };
        let snippet = build_class_snippet(&sym, " ", &loader);
        assert!(
            snippet.contains("@implements Comparable<${1:T}>"),
            "Should contain @implements with template tab stop, got:\n{}",
            snippet
        );
    }

    #[test]
    fn generates_property_snippet_always_has_var() {
        let sym = SymbolInfo {
            type_hint: Some(PhpType::parse("string")),
            ..Default::default()
        };
        let snippet = build_property_snippet(&sym, "    ", &no_classes);
        assert!(
            snippet.contains("@var string"),
            "Typed property should have @var string, got:\n{}",
            snippet
        );
        // No summary line for properties — just /** @var Type */
        assert!(
            !snippet.lines().any(|l| l.contains("* ${")),
            "Property snippet should not have summary placeholder, got:\n{}",
            snippet
        );
    }

    #[test]
    fn generates_property_snippet_untyped_has_mixed() {
        let sym = SymbolInfo::default();
        let snippet = build_property_snippet(&sym, "    ", &no_classes);
        assert!(
            snippet.contains("@var ${1:mixed}"),
            "Untyped property should have @var with mixed placeholder, got:\n{}",
            snippet
        );
    }

    #[test]
    fn generates_constant_snippet_with_type() {
        let sym = SymbolInfo {
            type_hint: Some(PhpType::parse("int")),
            ..Default::default()
        };
        let snippet = build_constant_snippet(&sym, "    ", &no_classes);
        assert!(snippet.contains("@var int"));
    }

    #[test]
    fn generates_constant_snippet_without_type() {
        let sym = SymbolInfo::default();
        let snippet = build_constant_snippet(&sym, "    ", &no_classes);
        assert!(snippet.contains("@var ${1:mixed}"));
    }

    #[test]
    fn param_names_are_space_aligned() {
        let sym = SymbolInfo {
            params: vec![
                (None, "$activeAlerts".to_string()),
                (None, "$x".to_string()),
            ],
            return_type: Some(PhpType::parse("void")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            " ",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        // Both params are untyped → both get mixed placeholders.
        // The `$` names should start at the same column.
        let param_lines: Vec<&str> = snippet.lines().filter(|l| l.contains("@param")).collect();
        assert_eq!(param_lines.len(), 2, "Should have 2 @param lines");
        let col1 = param_lines[0].find('$').unwrap();
        let col2 = param_lines[1].find('$').unwrap();
        assert_eq!(col1, col2, "$ names should be aligned, got:\n{}", snippet);
    }

    #[test]
    fn param_names_aligned_with_mixed_enrichment_widths() {
        // Simulate: one param with a generic class type (wide snippet) and
        // one untyped param (short snippet).  The visible `$` columns must
        // still line up even though the raw snippet lengths differ.
        use std::sync::Arc;
        let cls = Arc::new(ClassInfo {
            template_params: vec![atom("TKey"), atom("TValue")],
            ..Default::default()
        });
        let loader = |name: &str| -> Option<Arc<ClassInfo>> {
            if name == "TypedCollection" {
                Some(Arc::clone(&cls))
            } else {
                None
            }
        };

        let sym = SymbolInfo {
            params: vec![
                (None, "$data".to_string()),
                (
                    Some(PhpType::parse("TypedCollection")),
                    "$primary".to_string(),
                ),
            ],
            return_type: Some(PhpType::parse("void")),
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        let snippet = build_function_snippet(
            &sym,
            " ",
            "<?php\n",
            Position {
                line: 0,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &loader,
            None,
        );
        let param_lines: Vec<&str> = snippet.lines().filter(|l| l.contains("@param")).collect();
        assert_eq!(param_lines.len(), 2, "Should have 2 @param lines");

        // The plain-text renderings are "mixed" (5) and
        // "TypedCollection<TKey, TValue>" (29).  The snippet for the
        // shorter one must contain enough padding so the escaped `\$`
        // param names start at the same visible column.
        //
        // To verify, strip snippet markers and compare the column of
        // the first `\$` (escaped dollar) in each line.
        fn strip_snippets(s: &str) -> String {
            let mut out = String::new();
            let bytes = s.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if i + 1 < bytes.len() && bytes[i] == b'$' && bytes[i + 1] == b'{' {
                    i += 2;
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i < bytes.len() && bytes[i] == b':' {
                        i += 1;
                    }
                    let mut depth = 1u32;
                    while i < bytes.len() && depth > 0 {
                        if bytes[i] == b'}' {
                            depth -= 1;
                            i += 1;
                        } else {
                            out.push(bytes[i] as char);
                            i += 1;
                        }
                    }
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
            out
        }
        let plain1 = strip_snippets(param_lines[0]);
        let plain2 = strip_snippets(param_lines[1]);
        let col1 = plain1.find('$').expect("should contain $");
        let col2 = plain2.find('$').expect("should contain $");
        assert_eq!(
            col1, col2,
            "$ names should be visually aligned, got:\n  {}\n  {}",
            plain1, plain2
        );
    }

    #[test]
    fn blank_separator_between_tag_groups() {
        let sym = SymbolInfo {
            params: vec![(None, "$x".to_string())],
            return_type: None,
            ..Default::default()
        };
        let use_map = HashMap::new();
        let file_ns = None;
        // Use content with a throw so we get @throws.
        let content = "<?php\nfunction test($x) { throw new \\RuntimeException(); }";
        let snippet = build_function_snippet(
            &sym,
            "",
            content,
            Position {
                line: 1,
                character: 0,
            },
            &use_map,
            &file_ns,
            &[],
            &no_classes,
            None,
        );
        // @param, @throws and @return should all be present.
        assert!(
            snippet.contains("@param"),
            "Should have @param, got:\n{}",
            snippet
        );
        assert!(
            snippet.contains("@throws"),
            "Should have @throws, got:\n{}",
            snippet
        );
        assert!(
            snippet.contains("@return"),
            "Should have @return, got:\n{}",
            snippet
        );
        // There should be a blank `*` line between @param and @throws,
        // and between @throws and @return.
        let lines: Vec<&str> = snippet.lines().collect();
        let param_idx = lines.iter().position(|l| l.contains("@param")).unwrap();
        let throws_idx = lines.iter().position(|l| l.contains("@throws")).unwrap();
        let return_idx = lines.iter().position(|l| l.contains("@return")).unwrap();
        assert_eq!(
            lines[param_idx + 1].trim(),
            "*",
            "Blank separator between @param and @throws, got:\n{}",
            snippet
        );
        assert_eq!(
            lines[throws_idx + 1].trim(),
            "*",
            "Blank separator between @throws and @return, got:\n{}",
            snippet
        );
        // But no blank line before @param (first group).
        assert_ne!(
            lines[param_idx - 1].trim(),
            "*",
            "No blank separator before @param, got:\n{}",
            snippet
        );
        assert!(
            throws_idx == param_idx + 2,
            "@throws should be right after blank line, got:\n{}",
            snippet
        );
        assert!(
            return_idx == throws_idx + 2,
            "@return should be right after blank line, got:\n{}",
            snippet
        );
    }

    // ── is_class_like_keyword ───────────────────────────────────────────

    #[test]
    fn is_class_like_plain_class() {
        assert!(is_class_like_keyword("class Foo {}"));
    }

    #[test]
    fn is_class_like_abstract_class() {
        assert!(is_class_like_keyword("abstract class Foo {}"));
    }

    #[test]
    fn is_class_like_interface() {
        assert!(is_class_like_keyword("interface Foo {}"));
    }

    #[test]
    fn is_class_like_not_function() {
        assert!(!is_class_like_keyword("function foo() {}"));
    }

    #[test]
    fn is_class_like_not_property() {
        assert!(!is_class_like_keyword("public string $foo;"));
    }

    // ── extract_class_supertypes ────────────────────────────────────────

    #[test]
    fn extracts_extends_from_decl() {
        let (parents, ifaces) = extract_class_supertypes("class Child extends Base {}");
        assert_eq!(parents, vec!["Base"]);
        assert!(ifaces.is_empty());
    }

    #[test]
    fn extracts_implements_from_decl() {
        let (parents, ifaces) = extract_class_supertypes("class Foo implements Bar, Baz {}");
        assert!(parents.is_empty());
        assert_eq!(ifaces, vec!["Bar", "Baz"]);
    }

    #[test]
    fn extracts_both_from_decl() {
        let (parents, ifaces) =
            extract_class_supertypes("class Child extends Base implements Iface {}");
        assert_eq!(parents, vec!["Base"]);
        assert_eq!(ifaces, vec!["Iface"]);
    }

    // ── classify_declaration: variable assignments ──────────────────────

    #[test]
    fn classifies_variable_assignment() {
        let ctx = classify_declaration("    $items = [''];\n");
        assert!(matches!(ctx, DocblockContext::Inline));
    }

    #[test]
    fn classifies_variable_assignment_no_value() {
        let ctx = classify_declaration("    $x = null;\n");
        assert!(matches!(ctx, DocblockContext::Inline));
    }

    #[test]
    fn classifies_variable_not_confused_with_property() {
        // Properties have modifiers; bare `$var` does not.
        let ctx = classify_declaration("    public string $name;\n");
        assert!(matches!(ctx, DocblockContext::Property));
    }

    #[test]
    fn extracts_variable_name_from_inline_assignment() {
        let info = parse_declaration_info("    $items = [''];\n");
        assert_eq!(info.variable_name.as_deref(), Some("$items"));
    }

    #[test]
    fn extracts_variable_name_from_simple_assignment() {
        let info = parse_declaration_info("    $count = 42;\n");
        assert_eq!(info.variable_name.as_deref(), Some("$count"));
    }
}
