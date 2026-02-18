//! PHPDoc tag completion.
//!
//! When the user types `@` inside a `/** … */` docblock, this module
//! provides context-aware tag suggestions.  The tags offered depend on
//! what PHP symbol follows the docblock:
//!
//! - **Function / Method**: `@param`, `@return`, `@throws`, …
//! - **Class / Interface / Trait / Enum**: `@property`, `@method`, `@mixin`, `@template`, …
//! - **Property**: `@var`, `@deprecated`, …
//! - **Constant**: `@var`, `@deprecated`, …
//! - **Unknown / file-level**: all tags
//!
//! When the symbol following the docblock can be parsed, completions are
//! pre-filled with concrete type and parameter information extracted from
//! the declaration (e.g. `@param string $name` instead of bare `@param`).
//!
//! Smart `@throws` completion analyses the function/method body for
//! `throw new ExceptionType(…)` statements that are not caught by a
//! `try/catch` block enclosing them, and suggests documenting each
//! uncaught exception type.  When the exception class is not yet imported
//! via a `use` statement, an `additional_text_edits` entry is added to
//! insert the import automatically.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::completion::use_edit::{build_use_edit, find_use_insert_position};

// ─── Context Detection ─────────────────────────────────────────────────────

/// The kind of PHP symbol that follows the docblock containing the cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocblockContext {
    /// The docblock precedes a function or method declaration.
    FunctionOrMethod,
    /// The docblock precedes a class, interface, trait, or enum.
    ClassLike,
    /// The docblock precedes a property declaration.
    Property,
    /// The docblock precedes a constant declaration.
    Constant,
    /// Context could not be determined (file-level, or symbol not recognized).
    Unknown,
}

/// Information extracted from the PHP symbol following the docblock.
///
/// Used to pre-fill completion items with concrete types and names.
#[derive(Debug, Clone, Default)]
pub struct SymbolInfo {
    /// Parameters: `(optional_type_hint, $name)`.
    pub params: Vec<(Option<String>, String)>,
    /// Return type hint (e.g. `"string"`, `"void"`, `"?int"`).
    pub return_type: Option<String>,
    /// Property / constant type hint.
    pub type_hint: Option<String>,
}

/// Check whether the cursor at `position` is inside a `/** … */` docblock
/// comment, and if so, return the partial tag prefix the user is typing
/// (e.g. `"@par"`, `"@"`, `"@phpstan-a"`).
///
/// Returns `None` if the cursor is not inside a docblock or is not at a
/// tag position (i.e. no `@` on the current line before the cursor).
pub fn extract_phpdoc_prefix(content: &str, position: Position) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = position.line as usize;
    if line_idx >= lines.len() {
        return None;
    }

    let line = lines[line_idx];
    let chars: Vec<char> = line.chars().collect();
    let col = (position.character as usize).min(chars.len());

    // Walk backwards from cursor to find `@`
    let mut i = col;
    while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '-' || chars[i - 1] == '_') {
        i -= 1;
    }

    // Must be preceded by `@`
    if i == 0 || chars[i - 1] != '@' {
        return None;
    }
    // Include the `@`
    i -= 1;

    // The character before `@` (if any) must be whitespace or `*`
    // (typical docblock line prefix).  This prevents matching email
    // addresses or annotations in regular strings.
    if i > 0 {
        let prev = chars[i - 1];
        if !prev.is_whitespace() && prev != '*' {
            return None;
        }
    }

    let prefix: String = chars[i..col].iter().collect();

    // Now verify that we are actually inside a `/** … */` block.
    if !is_inside_docblock(content, position) {
        return None;
    }

    Some(prefix)
}

/// Returns `true` if the given position is inside a `/** … */` docblock.
///
/// Scans backwards from the cursor to find the nearest `/**` that has not
/// been closed by a matching `*/` before the cursor position.
pub fn is_inside_docblock(content: &str, position: Position) -> bool {
    // Convert position to byte offset for easier scanning
    let byte_offset = position_to_byte_offset(content, position);

    let before_cursor = &content[..byte_offset.min(content.len())];

    // Find the last `/**` before the cursor
    let last_open = before_cursor.rfind("/**");
    if last_open.is_none() {
        return false;
    }
    let open_pos = last_open.unwrap();

    // Check if there is a `*/` between the `/**` and the cursor
    // (which would mean the docblock is closed)
    let after_open = &before_cursor[open_pos + 3..];
    !after_open.contains("*/")
}

/// Determine what PHP symbol follows the docblock at the cursor position.
///
/// This looks at the content after the docblock's closing `*/` (or after
/// the current line if the docblock is still open) to identify the next
/// meaningful PHP token.
pub fn detect_context(content: &str, position: Position) -> DocblockContext {
    let remaining = get_text_after_docblock(content, position);
    classify_from_tokens(&remaining)
}

/// Extract information about the PHP symbol following the docblock.
///
/// Parses the declaration line(s) after `*/` to pull out parameter names,
/// type hints, return types, etc.
pub fn extract_symbol_info(content: &str, position: Position) -> SymbolInfo {
    let remaining = get_text_after_docblock(content, position);
    parse_symbol_info(&remaining)
}

/// Collect the names of parameters already documented with `@param` tags
/// in the current docblock above the cursor.
pub fn find_existing_param_tags(content: &str, position: Position) -> Vec<String> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    // Find the opening `/**`
    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let docblock_so_far = &before_cursor[open_pos..];

    let mut existing = Vec::new();
    for line in docblock_so_far.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim();
            // @param may have: Type $name desc  or just $name
            for word in rest.split_whitespace() {
                if word.starts_with('$') {
                    existing.push(word.to_string());
                    break;
                }
            }
        }
    }

    existing
}

/// Check whether `@return` is already documented in the current docblock.
fn has_existing_return_tag(content: &str, position: Position) -> bool {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return false,
    };

    let docblock_so_far = &before_cursor[open_pos..];
    docblock_so_far.lines().any(|line| {
        let trimmed = line.trim().trim_start_matches('*').trim();
        trimmed.starts_with("@return")
    })
}

/// Collect exception type names already documented with `@throws` tags
/// in the current docblock above the cursor.
///
/// Returns short type names as written in the docblock (e.g.
/// `"InvalidArgumentException"`, `"\\RuntimeException"`).
pub fn find_existing_throws_tags(content: &str, position: Position) -> Vec<String> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    // Also look at the docblock text AFTER the cursor (the user may have
    // already documented some throws below the cursor line).
    let close_pos = content[open_pos..].find("*/").map(|p| open_pos + p + 2);
    let docblock = if let Some(end) = close_pos {
        &content[open_pos..end]
    } else {
        &content[open_pos..byte_offset.min(content.len())]
    };

    let mut existing = Vec::new();
    for line in docblock.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("@throws") {
            let rest = rest.trim();
            if let Some(type_name) = rest.split_whitespace().next() {
                let clean = type_name.trim_start_matches('\\');
                if !clean.is_empty() {
                    existing.push(clean.to_string());
                }
            }
        }
    }

    existing
}

/// Extract the function/method body text that follows the docblock at
/// the cursor position.
///
/// Returns the text between the opening `{` and matching closing `}` of
/// the function/method declaration.  Returns `None` if the body cannot
/// be located (e.g. abstract method, or the docblock is not followed by
/// a function).
fn extract_function_body(content: &str, position: Position) -> Option<String> {
    let after_docblock = get_text_after_docblock(content, position);

    // Find the `function` keyword to confirm this is a function/method.
    let func_idx = {
        let lower = after_docblock.to_lowercase();
        let mut start = 0;
        let mut found = None;
        while let Some(pos) = lower[start..].find("function") {
            let abs = start + pos;
            let before_ok = abs == 0 || !after_docblock.as_bytes()[abs - 1].is_ascii_alphanumeric();
            let after_pos = abs + 8; // "function".len()
            let after_ok = after_pos >= after_docblock.len()
                || !after_docblock.as_bytes()[after_pos].is_ascii_alphanumeric();
            if before_ok && after_ok {
                found = Some(abs);
                break;
            }
            start = abs + 8;
        }
        found?
    };

    let after_func = &after_docblock[func_idx..];

    // Find the opening brace of the function body.
    let open_brace = after_func.find('{')?;
    let body_start = open_brace + 1;

    // Walk forward to find the matching closing brace.
    let mut depth = 1u32;
    let mut pos = body_start;
    let bytes = after_func.as_bytes();
    // Track whether we are inside a string literal to avoid counting
    // braces inside strings.
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    while pos < bytes.len() && depth > 0 {
        let b = bytes[pos];
        if in_single_quote {
            if b == b'\\' {
                pos += 1; // skip escaped char
            } else if b == b'\'' {
                in_single_quote = false;
            }
        } else if in_double_quote {
            if b == b'\\' {
                pos += 1; // skip escaped char
            } else if b == b'"' {
                in_double_quote = false;
            }
        } else {
            match b {
                b'\'' => in_single_quote = true,
                b'"' => in_double_quote = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(after_func[body_start..pos].to_string());
                    }
                }
                b'/' if pos + 1 < bytes.len() => {
                    // Skip line comments
                    if bytes[pos + 1] == b'/' {
                        while pos < bytes.len() && bytes[pos] != b'\n' {
                            pos += 1;
                        }
                        continue;
                    }
                    // Skip block comments
                    if bytes[pos + 1] == b'*' {
                        pos += 2;
                        while pos + 1 < bytes.len() {
                            if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                                pos += 1;
                                break;
                            }
                            pos += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        pos += 1;
    }

    None
}

/// Information about a `throw new Type(…)` statement in a function body.
#[derive(Debug)]
struct ThrowInfo {
    /// The exception type name as written in source (e.g. `"InvalidArgumentException"`,
    /// `"\\RuntimeException"`, `"Exceptions\\Custom"`).
    type_name: String,
    /// Byte offset of this throw statement relative to the function body start.
    offset: usize,
}

/// Information about a `catch (Type $var)` clause in a function body.
#[derive(Debug)]
struct CatchInfo {
    /// The caught exception type names (multi-catch produces multiple).
    type_names: Vec<String>,
    /// Byte offset of the start of the `try` block this catch belongs to.
    try_start: usize,
    /// Byte offset of the end of the `try` block (the matching `}`).
    try_end: usize,
}

/// Find all `throw new Type(…)` statements in the given function body text.
fn find_throw_statements(body: &str) -> Vec<ThrowInfo> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Skip string literals
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            let quote = bytes[pos];
            pos += 1;
            while pos < len {
                if bytes[pos] == b'\\' {
                    pos += 1; // skip escaped char
                } else if bytes[pos] == quote {
                    break;
                }
                pos += 1;
            }
            pos += 1;
            continue;
        }

        // Skip line comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            while pos < len && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // Skip block comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < len {
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }

        // Look for `throw`
        if pos + 5 <= len && &body[pos..pos + 5] == "throw" {
            // Make sure it's a whole word
            let before_ok = pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric();
            let after_ok =
                pos + 5 >= len || !bytes[pos + 5].is_ascii_alphanumeric() && bytes[pos + 5] != b'_';
            if before_ok && after_ok {
                // Look for `new` after `throw`
                let after_throw = &body[pos + 5..];
                let trimmed = after_throw.trim_start();
                if trimmed.starts_with("new ")
                    || trimmed.starts_with("new\t")
                    || trimmed.starts_with("new\n")
                {
                    let after_new = trimmed[3..].trim_start();
                    // Extract the class name (may include namespace separators)
                    let type_end = after_new
                        .find(|c: char| !c.is_alphanumeric() && c != '\\' && c != '_')
                        .unwrap_or(after_new.len());
                    let type_name = &after_new[..type_end];
                    if !type_name.is_empty() {
                        results.push(ThrowInfo {
                            type_name: type_name.to_string(),
                            offset: pos,
                        });
                    }
                }
            }
        }

        pos += 1;
    }

    results
}

/// Find all `try { … } catch (…)` blocks and their caught types.
fn find_catch_blocks(body: &str) -> Vec<CatchInfo> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Skip string literals
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            let quote = bytes[pos];
            pos += 1;
            while pos < len {
                if bytes[pos] == b'\\' {
                    pos += 1;
                } else if bytes[pos] == quote {
                    break;
                }
                pos += 1;
            }
            pos += 1;
            continue;
        }

        // Skip line comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            while pos < len && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }

        // Skip block comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < len {
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }

        // Look for `try`
        if pos + 3 <= len && &body[pos..pos + 3] == "try" {
            let before_ok = pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric();
            let after_ok = pos + 3 >= len
                || (!bytes[pos + 3].is_ascii_alphanumeric() && bytes[pos + 3] != b'_');
            if before_ok && after_ok {
                // Find the opening brace of the try block
                let after_try = &body[pos + 3..];
                if let Some(brace_offset) = after_try.find('{') {
                    let try_body_start = pos + 3 + brace_offset;
                    // Find the matching closing brace
                    if let Some(try_body_end) = find_matching_brace(body, try_body_start) {
                        // Now look for `catch` after the try block's `}`
                        let mut catch_search = try_body_end + 1;
                        while catch_search < len {
                            let remaining = body[catch_search..].trim_start();
                            let remaining_start = len - remaining.len();
                            if let Some(after_catch) = remaining.strip_prefix("catch") {
                                // Ensure `catch` is a whole word
                                if after_catch
                                    .bytes()
                                    .next()
                                    .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
                                {
                                    break;
                                }
                                let catch_keyword_len = "catch".len();
                                // Extract caught types from `catch (Type1 | Type2 $var)`
                                if let Some(open_p) = after_catch.find('(') {
                                    let paren_content_start = catch_keyword_len + open_p + 1;
                                    if let Some(close_p) =
                                        remaining[paren_content_start..].find(')')
                                    {
                                        let paren_content = &remaining
                                            [paren_content_start..paren_content_start + close_p];
                                        let type_names = parse_catch_types(paren_content);
                                        if !type_names.is_empty() {
                                            results.push(CatchInfo {
                                                type_names,
                                                try_start: try_body_start,
                                                try_end: try_body_end,
                                            });
                                        }

                                        // Skip past the catch block body
                                        let after_close_paren =
                                            remaining_start + paren_content_start + close_p + 1;
                                        if let Some(cb) = body[after_close_paren..].find('{') {
                                            let catch_body_start = after_close_paren + cb;
                                            if let Some(catch_body_end) =
                                                find_matching_brace(body, catch_body_start)
                                            {
                                                catch_search = catch_body_end + 1;
                                                continue;
                                            }
                                        }
                                    }
                                }
                                break;
                            } else if remaining.starts_with("finally") {
                                // Skip finally block, no more catches
                                break;
                            } else {
                                break;
                            }
                        }

                        // Continue scanning INSIDE the try body so that
                        // nested try-catch blocks are discovered.  We
                        // advance past the opening `{` to avoid
                        // re-matching the outer `try` keyword.
                        pos = try_body_start + 1;
                        continue;
                    }
                }
            }
        }

        pos += 1;
    }

    results
}

/// Find the position of the `}` that matches the `{` at `open_pos`.
fn find_matching_brace(text: &str, open_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if open_pos >= len || bytes[open_pos] != b'{' {
        return None;
    }
    let mut depth = 1u32;
    let mut pos = open_pos + 1;
    let mut in_single = false;
    let mut in_double = false;
    while pos < len && depth > 0 {
        let b = bytes[pos];
        if in_single {
            if b == b'\\' {
                pos += 1;
            } else if b == b'\'' {
                in_single = false;
            }
        } else if in_double {
            if b == b'\\' {
                pos += 1;
            } else if b == b'"' {
                in_double = false;
            }
        } else {
            match b {
                b'\'' => in_single = true,
                b'"' => in_double = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(pos);
                    }
                }
                b'/' if pos + 1 < len => {
                    if bytes[pos + 1] == b'/' {
                        while pos < len && bytes[pos] != b'\n' {
                            pos += 1;
                        }
                        continue;
                    }
                    if bytes[pos + 1] == b'*' {
                        pos += 2;
                        while pos + 1 < len {
                            if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                                pos += 1;
                                break;
                            }
                            pos += 1;
                        }
                    }
                }
                _ => {}
            }
        }
        pos += 1;
    }
    None
}

/// Parse the content inside `catch ( … )` into individual type names.
///
/// Handles multi-catch: `ExceptionA | ExceptionB $e` → `["ExceptionA", "ExceptionB"]`.
fn parse_catch_types(paren_content: &str) -> Vec<String> {
    let mut types = Vec::new();
    // Remove the variable name (starts with `$`)
    let without_var = if let Some(dollar) = paren_content.rfind('$') {
        &paren_content[..dollar]
    } else {
        paren_content
    };

    for part in without_var.split('|') {
        let t = part.trim().trim_start_matches('\\');
        if !t.is_empty() {
            // Take only the short name (last segment after `\`)
            let short = t.rsplit('\\').next().unwrap_or(t);
            types.push(short.to_string());
        }
    }

    types
}

/// Determine which exception types in a function body are **not** caught
/// by an enclosing `try/catch` block.
///
/// Detects three patterns:
/// 1. `throw new ExceptionType(…)` — direct instantiation
/// 2. `throw $this->method()` / `throw self::method()` / `throw static::method()`
///    — the method's return type is the thrown exception type
/// 3. `$this->method()` / `self::method()` calls where the called method's
///    docblock declares `@throws ExceptionType` — propagated throws
///
/// Returns a deduplicated list of short exception type names.
pub fn find_uncaught_throw_types(content: &str, position: Position) -> Vec<String> {
    let body = match extract_function_body(content, position) {
        Some(b) => b,
        None => return Vec::new(),
    };

    let throws = find_throw_statements(&body);
    let throw_expr_types = find_throw_expression_types(&body, content);
    let propagated = find_propagated_throws(&body, content);
    let catches = find_catch_blocks(&body);

    let mut uncaught: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Direct `throw new Type(…)` statements
    for throw in &throws {
        let short_name = throw
            .type_name
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(&throw.type_name);

        // Check if this throw is inside a try block whose catch handles
        // the same exception type.
        let is_caught = catches.iter().any(|c| {
            throw.offset > c.try_start
                && throw.offset < c.try_end
                && c.type_names.iter().any(|ct| {
                    let ct_short = ct.rsplit('\\').next().unwrap_or(ct);
                    ct_short.eq_ignore_ascii_case(short_name)
                        || ct_short == "Throwable"
                        || ct_short == "Exception"
                })
        });

        if !is_caught && seen.insert(short_name.to_string()) {
            uncaught.push(short_name.to_string());
        }
    }

    // 2. `throw $this->method()` — return type of method is the thrown type
    for te in &throw_expr_types {
        let short_name = te
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(te);
        if !short_name.is_empty() && seen.insert(short_name.to_string()) {
            uncaught.push(short_name.to_string());
        }
    }

    // 3. Propagated @throws from called methods
    for prop in &propagated {
        let short_name = prop
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(prop);
        if !short_name.is_empty() && seen.insert(short_name.to_string()) {
            uncaught.push(short_name.to_string());
        }
    }

    uncaught
}

/// Find `throw $this->method(…)` / `throw self::method(…)` /
/// `throw static::method(…)` patterns and resolve the called method's
/// return type from its declaration or docblock in the same file.
///
/// Returns a list of exception type names (the return types of the
/// called methods).
fn find_throw_expression_types(body: &str, file_content: &str) -> Vec<String> {
    let mut results = Vec::new();

    // Patterns: throw $this->method(  /  throw self::method(  /  throw static::method(
    let patterns: &[&str] = &["$this->", "self::", "static::"];

    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Skip strings
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            let quote = bytes[pos];
            pos += 1;
            while pos < len {
                if bytes[pos] == b'\\' {
                    pos += 1;
                } else if bytes[pos] == quote {
                    break;
                }
                pos += 1;
            }
            pos += 1;
            continue;
        }
        // Skip comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            while pos < len && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < len {
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }

        // Look for `throw` keyword
        if pos + 5 <= len && &body[pos..pos + 5] == "throw" {
            let before_ok = pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric();
            let after_ok = pos + 5 >= len
                || (!bytes[pos + 5].is_ascii_alphanumeric() && bytes[pos + 5] != b'_');
            if before_ok && after_ok {
                let after_throw = body[pos + 5..].trim_start();
                // Check if this is NOT `throw new` (handled separately)
                if !after_throw.starts_with("new ")
                    && !after_throw.starts_with("new\t")
                    && !after_throw.starts_with("new\n")
                {
                    // Check for $this->, self::, static::
                    for pat in patterns {
                        if let Some(rest) = after_throw.strip_prefix(pat) {
                            // Extract method name
                            let name_end = rest
                                .find(|c: char| !c.is_alphanumeric() && c != '_')
                                .unwrap_or(rest.len());
                            let method_name = &rest[..name_end];
                            if !method_name.is_empty() {
                                // Look up the method's return type in the file
                                if let Some(ret_type) =
                                    find_method_return_type(file_content, method_name)
                                {
                                    results.push(ret_type);
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        pos += 1;
    }

    results
}

/// Find all method calls (`$this->method(…)`, `self::method(…)`,
/// `static::method(…)`) in the function body and collect `@throws`
/// annotations from those methods' docblocks in the same file.
///
/// This propagates `@throws` declarations: if method A calls method B
/// and B declares `@throws SomeException`, then A should also document
/// (or is aware of) that exception.
fn find_propagated_throws(body: &str, file_content: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut seen_methods = std::collections::HashSet::new();

    let patterns: &[&str] = &["$this->", "self::", "static::"];

    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Skip strings
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            let quote = bytes[pos];
            pos += 1;
            while pos < len {
                if bytes[pos] == b'\\' {
                    pos += 1;
                } else if bytes[pos] == quote {
                    break;
                }
                pos += 1;
            }
            pos += 1;
            continue;
        }
        // Skip comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            while pos < len && bytes[pos] != b'\n' {
                pos += 1;
            }
            continue;
        }
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos += 2;
            while pos + 1 < len {
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    pos += 2;
                    break;
                }
                pos += 1;
            }
            continue;
        }

        // Look for $this->, self::, static::
        for pat in patterns {
            if pos + pat.len() <= len && &body[pos..pos + pat.len()] == *pat {
                // Verify word boundary before the pattern
                let before_ok = if *pat == "$this->" {
                    // $ is its own boundary
                    true
                } else {
                    pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric() && bytes[pos - 1] != b'_'
                };
                if !before_ok {
                    break;
                }

                let after_pat = &body[pos + pat.len()..];
                let name_end = after_pat
                    .find(|c: char| !c.is_alphanumeric() && c != '_')
                    .unwrap_or(after_pat.len());
                let method_name = &after_pat[..name_end];

                // Must be followed by `(` to be a method call
                let after_name = after_pat[name_end..].trim_start();
                if !method_name.is_empty()
                    && after_name.starts_with('(')
                    && seen_methods.insert(method_name.to_string())
                {
                    // Look up @throws in the method's docblock
                    let throws = find_method_throws_tags(file_content, method_name);
                    results.extend(throws);
                }
                break;
            }
        }

        pos += 1;
    }

    results
}

/// Find the return type of a method by scanning the file for its
/// declaration and docblock.
///
/// Looks for `function methodName(…): ReturnType` in the file content
/// and also checks `@return Type` in the preceding docblock.
fn find_method_return_type(file_content: &str, method_name: &str) -> Option<String> {
    // Find `function methodName(` in the file
    let search = format!("function {}", method_name);
    let mut search_start = 0;
    while let Some(func_pos) = file_content[search_start..].find(&search) {
        let abs_pos = search_start + func_pos;
        let after = abs_pos + search.len();

        // Ensure it's a whole word and followed by `(`
        let before_ok =
            abs_pos == 0 || !file_content.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_trimmed = file_content[after..].trim_start();
        if before_ok && after_trimmed.starts_with('(') {
            // Try native return type: find `)` then `: Type`
            if let Some(paren_close) = after_trimmed.find(')') {
                let after_paren = after_trimmed[paren_close + 1..].trim_start();
                if let Some(rest) = after_paren.strip_prefix(':') {
                    let rest = rest.trim_start();
                    let type_end = rest
                        .find(|c: char| !c.is_alphanumeric() && c != '\\' && c != '_' && c != '?')
                        .unwrap_or(rest.len());
                    let ret_type = rest[..type_end].trim();
                    if !ret_type.is_empty() && ret_type != "void" && ret_type != "mixed" {
                        let clean = ret_type.trim_start_matches('?').trim_start_matches('\\');
                        let short = clean.rsplit('\\').next().unwrap_or(clean);
                        return Some(short.to_string());
                    }
                }
            }

            // Try docblock @return
            let before_func = &file_content[..abs_pos];
            if let Some(doc_end) = before_func.rfind("*/") {
                let doc_region = &before_func[..doc_end + 2];
                if let Some(doc_start) = doc_region.rfind("/**") {
                    let between = file_content[doc_end + 2..abs_pos].trim();
                    // Ensure the docblock is immediately before the function
                    // (only visibility/static/abstract modifiers between)
                    let is_adjacent =
                        between.is_empty() || between.split_whitespace().all(is_modifier_keyword);
                    if is_adjacent {
                        let docblock = &doc_region[doc_start..];
                        for line in docblock.lines() {
                            let trimmed = line.trim().trim_start_matches('*').trim();
                            if let Some(rest) = trimmed.strip_prefix("@return") {
                                let rest = rest.trim();
                                if let Some(type_str) = rest.split_whitespace().next() {
                                    let clean =
                                        type_str.trim_start_matches('?').trim_start_matches('\\');
                                    let short = clean.rsplit('\\').next().unwrap_or(clean);
                                    if !short.is_empty()
                                        && short != "void"
                                        && short != "mixed"
                                        && short != "self"
                                        && short != "static"
                                    {
                                        return Some(short.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }

            return None;
        }
        search_start = abs_pos + search.len();
    }
    None
}

/// Find `@throws` tags in a method's docblock by scanning the file.
///
/// Returns the list of exception type short names declared via `@throws`
/// in the docblock preceding `function methodName(…)`.
fn find_method_throws_tags(file_content: &str, method_name: &str) -> Vec<String> {
    let search = format!("function {}", method_name);
    let mut search_start = 0;
    while let Some(func_pos) = file_content[search_start..].find(&search) {
        let abs_pos = search_start + func_pos;
        let after = abs_pos + search.len();

        let before_ok =
            abs_pos == 0 || !file_content.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_trimmed = file_content[after..].trim_start();
        if before_ok && after_trimmed.starts_with('(') {
            // Find the preceding docblock
            let before_func = &file_content[..abs_pos];
            if let Some(doc_end) = before_func.rfind("*/") {
                let doc_region = &before_func[..doc_end + 2];
                if let Some(doc_start) = doc_region.rfind("/**") {
                    let between = file_content[doc_end + 2..abs_pos].trim();
                    let is_adjacent =
                        between.is_empty() || between.split_whitespace().all(is_modifier_keyword);
                    if is_adjacent {
                        let docblock = &doc_region[doc_start..];
                        let mut throws = Vec::new();
                        for line in docblock.lines() {
                            let trimmed = line.trim().trim_start_matches('*').trim();
                            if let Some(rest) = trimmed.strip_prefix("@throws") {
                                let rest = rest.trim();
                                if let Some(type_str) = rest.split_whitespace().next() {
                                    let clean = type_str.trim_start_matches('\\');
                                    let short = clean.rsplit('\\').next().unwrap_or(clean);
                                    if !short.is_empty() {
                                        throws.push(short.to_string());
                                    }
                                }
                            }
                        }
                        return throws;
                    }
                }
            }
            return Vec::new();
        }
        search_start = abs_pos + search.len();
    }
    Vec::new()
}

/// Check whether a token is a PHP visibility / modifier keyword that can
/// appear between a docblock and a function declaration.
fn is_modifier_keyword(word: &str) -> bool {
    matches!(
        word.to_lowercase().as_str(),
        "public" | "protected" | "private" | "static" | "abstract" | "final"
    )
}

/// Resolve a short exception type name to its fully-qualified name using
/// the file's `use` map and namespace.
///
/// Returns the FQN (without leading `\`) if found, or `None` if the type
/// is already unqualified and in the global namespace.
fn resolve_exception_fqn(
    short_name: &str,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
) -> Option<String> {
    // Check the use map first
    if let Some(fqn) = use_map.get(short_name) {
        return Some(fqn.clone());
    }

    // If there's a namespace, the type might be in the current namespace
    if let Some(ns) = file_namespace {
        return Some(format!("{}\\{}", ns, short_name));
    }

    // Global namespace, no FQN to resolve to
    None
}

/// Check whether a `use` statement for the given FQN already exists in
/// the file content.
fn has_use_import(content: &str, fqn: &str) -> bool {
    let target = format!("use {};", fqn);
    let target_with_alias = format!("use {} as", fqn); // alias import
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == target || trimmed.starts_with(&target_with_alias) {
            return true;
        }
        // Handle group imports: `use Foo\{Bar, Baz};`
        // Check if the FQN's namespace prefix is used in a group import
        // that includes the short name.
        if let Some(ns_sep) = fqn.rfind('\\') {
            let ns_prefix = &fqn[..ns_sep];
            let short = &fqn[ns_sep + 1..];
            let group_prefix = format!("use {}\\{{", ns_prefix);
            if trimmed.starts_with(&group_prefix) {
                // Check if short name is in the brace list
                if let Some(brace_start) = trimmed.find('{')
                    && let Some(brace_end) = trimmed.find('}')
                {
                    let names = &trimmed[brace_start + 1..brace_end];
                    if names.split(',').any(|n| n.trim() == short) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Convert a `Position` (line, character) to a byte offset in `content`.
pub fn position_to_byte_offset(content: &str, position: Position) -> usize {
    let mut offset = 0usize;
    for (line_idx, line) in content.lines().enumerate() {
        if line_idx == position.line as usize {
            offset += (position.character as usize).min(line.len());
            return offset;
        }
        offset += line.len() + 1; // +1 for newline
    }
    offset
}

/// Get the text after the current docblock's closing `*/`.
///
/// If the docblock isn't closed yet, returns the text after the cursor
/// position (skipping lines that look like docblock continuation).
fn get_text_after_docblock(content: &str, position: Position) -> String {
    let byte_offset = position_to_byte_offset(content, position);
    let after_cursor = &content[byte_offset.min(content.len())..];

    if let Some(close_pos) = after_cursor.find("*/") {
        after_cursor[close_pos + 2..].to_string()
    } else {
        // Docblock not closed — return whatever follows
        after_cursor.to_string()
    }
}

/// Classify the PHP symbol from the first meaningful tokens.
fn classify_from_tokens(text: &str) -> DocblockContext {
    let mut tokens = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') || trimmed.starts_with("/**") {
            continue;
        }
        for word in trimmed.split_whitespace() {
            tokens.push(word.to_lowercase());
            if tokens.len() >= 6 {
                break;
            }
        }
        if tokens.len() >= 6 {
            break;
        }
    }

    if tokens.is_empty() {
        return DocblockContext::Unknown;
    }

    for token in &tokens {
        let t = token.as_str();
        match t {
            "function" => return DocblockContext::FunctionOrMethod,
            "class" | "interface" | "trait" | "enum" => return DocblockContext::ClassLike,
            "const" => return DocblockContext::Constant,
            "public" | "protected" | "private" | "static" | "readonly" | "abstract" | "final" => {
                continue;
            }
            _ => {
                if t.starts_with('$') {
                    return DocblockContext::Property;
                }
                if t.starts_with('?')
                    || t.starts_with('\\')
                    || t.chars().next().is_some_and(|c| c.is_uppercase())
                    || is_type_keyword(t)
                {
                    continue;
                }
                return DocblockContext::Unknown;
            }
        }
    }

    DocblockContext::Unknown
}

/// Check if a token is a PHP type keyword (used in property declarations).
fn is_type_keyword(token: &str) -> bool {
    matches!(
        token,
        "int"
            | "float"
            | "string"
            | "bool"
            | "array"
            | "object"
            | "callable"
            | "iterable"
            | "mixed"
            | "void"
            | "never"
            | "null"
            | "false"
            | "true"
            | "self"
            | "parent"
    )
}

/// Parse symbol info (params, return type, property type) from the
/// declaration text following the docblock.
fn parse_symbol_info(text: &str) -> SymbolInfo {
    let mut info = SymbolInfo::default();

    // Collect the declaration — may span multiple lines until `{` or `;`
    let mut decl = String::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('*') || trimmed.starts_with("/**") {
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

    // Check if it's a function/method
    if let Some(func_pos) = find_keyword_pos(decl, "function") {
        let after_func = &decl[func_pos + 8..]; // "function" is 8 chars

        // Find the parameter list between ( and )
        if let Some(open_paren) = after_func.find('(') {
            let after_open = &after_func[open_paren + 1..];
            if let Some(close_paren) = find_matching_paren(after_open) {
                let params_str = &after_open[..close_paren];
                info.params = parse_params(params_str);

                // Extract return type: look for `: Type` after the closing paren
                let after_close = &after_open[close_paren + 1..];
                info.return_type = extract_return_type_from_decl(after_close);
            }
        }
    } else {
        // Property or constant — extract type hint
        info.type_hint = extract_property_type(decl);
    }

    info
}

/// Find the position of a keyword in the declaration, making sure it's
/// a whole word (not part of another identifier).
fn find_keyword_pos(decl: &str, keyword: &str) -> Option<usize> {
    let lower = decl.to_lowercase();
    let mut start = 0;
    while let Some(pos) = lower[start..].find(keyword) {
        let abs_pos = start + pos;
        let before_ok = abs_pos == 0 || !decl.as_bytes()[abs_pos - 1].is_ascii_alphanumeric();
        let after_pos = abs_pos + keyword.len();
        let after_ok =
            after_pos >= decl.len() || !decl.as_bytes()[after_pos].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return Some(abs_pos);
        }
        start = abs_pos + keyword.len();
    }
    None
}

/// Find the position of the matching `)` for the first `(`, handling nesting.
fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

/// Parse a comma-separated parameter list into `(type_hint, $name)` pairs.
fn parse_params(params_str: &str) -> Vec<(Option<String>, String)> {
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();

    // Split on commas, respecting nested parens/angle brackets
    for param in split_params_str(params_str) {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }

        // Each param looks like: [Type] [$name] [= default]
        // or: [Type] &$name, [Type] ...$name
        let tokens: Vec<&str> = param.split_whitespace().collect();

        let mut type_hint: Option<String> = None;
        let mut name: Option<String> = None;

        for token in &tokens {
            let t = *token;
            // Skip default value part
            if t == "=" {
                break;
            }
            if t.starts_with('$') || t.starts_with("&$") || t.starts_with("...$") {
                // This is the variable name
                let clean = t.trim_start_matches("...").trim_start_matches('&');
                name = Some(clean.to_string());
                break;
            }
            // Otherwise it's (part of) the type hint
            if type_hint.is_some() {
                // Union/intersection types with spaces shouldn't happen,
                // but handle it gracefully
                let existing = type_hint.unwrap();
                type_hint = Some(format!("{}{}", existing, t));
            } else {
                type_hint = Some(t.to_string());
            }
        }

        if let Some(n) = name {
            result.push((type_hint, n));
        }
    }

    result
}

/// Split a parameter string on commas, respecting nested parens and angle brackets.
fn split_params_str(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for c in s.chars() {
        match c {
            '(' | '<' | '[' => {
                depth += 1;
                current.push(c);
            }
            ')' | '>' | ']' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }

    parts
}

/// Extract the return type from the portion after `)` in a function declaration.
///
/// Looks for `: Type` pattern.
fn extract_return_type_from_decl(after_close_paren: &str) -> Option<String> {
    let trimmed = after_close_paren.trim();
    let rest = trimmed.strip_prefix(':')?;
    let rest = rest.trim();

    // The return type is everything up to `{`, `;`, or end
    let end = rest.find(['{', ';']).unwrap_or(rest.len());

    let ret_type = rest[..end].trim();
    if ret_type.is_empty() {
        None
    } else {
        Some(ret_type.to_string())
    }
}

/// Extract the type hint from a property declaration.
///
/// Handles: `public string $name`, `protected ?int $count = 0`,
/// `private static array $cache`, `readonly Foo $bar`, etc.
fn extract_property_type(decl: &str) -> Option<String> {
    let tokens: Vec<&str> = decl.split_whitespace().collect();

    // Walk tokens: skip modifiers, the token before `$var` is the type
    let mut last_non_modifier: Option<String> = None;
    for token in &tokens {
        let t = *token;
        let lower = t.to_lowercase();

        if t.starts_with('$') {
            // The previous non-modifier token is the type
            return last_non_modifier;
        }

        // Skip `=` and everything after it
        if t == "=" || t == ";" {
            break;
        }

        match lower.as_str() {
            "public" | "protected" | "private" | "static" | "readonly" | "const" => {
                continue;
            }
            _ => {
                last_non_modifier = Some(t.to_string());
            }
        }
    }

    None
}

// ─── Tag Definitions ────────────────────────────────────────────────────────

/// A PHPDoc / PHPStan tag definition with metadata for completion.
struct TagDef {
    /// The tag text including `@` (e.g. `"@param"`).
    tag: &'static str,
    /// Brief one-line description shown in the completion detail.
    detail: &'static str,
    /// Display label showing usage format (e.g. `"@param Type $name"`).
    /// `None` means use `tag` as the label.
    label: Option<&'static str>,
}

/// Strip the leading `@` from a tag string.
///
/// The user has already typed `@` (or `@par…`) in the buffer and the LSP
/// client only replaces the *word* portion after `@`.  If the insert text
/// still contains `@`, the result is a doubled `@@tag`.
fn strip_at(s: &str) -> &str {
    s.strip_prefix('@').unwrap_or(s)
}

// ── Function / Method tags ──────────────────────────────────────────────────

const FUNCTION_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@param",
        detail: "Document a function parameter",
        label: Some("@param Type $name"),
    },
    TagDef {
        tag: "@return",
        detail: "Document the return type",
        label: Some("@return Type"),
    },
    TagDef {
        tag: "@throws",
        detail: "Document a thrown exception",
        label: Some("@throws ExceptionType"),
    },
    TagDef {
        tag: "@inheritdoc",
        detail: "Inherit documentation from parent",
        label: None,
    },
];

// ── Class-like tags ─────────────────────────────────────────────────────────

const CLASS_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@property",
        detail: "Declare a magic property",
        label: Some("@property Type $name"),
    },
    TagDef {
        tag: "@method",
        detail: "Declare a magic method",
        label: Some("@method ReturnType name()"),
    },
    TagDef {
        tag: "@mixin",
        detail: "Declare a mixin class",
        label: Some("@mixin ClassName"),
    },
    TagDef {
        tag: "@template",
        detail: "Declare a generic type parameter",
        label: Some("@template T"),
    },
    TagDef {
        tag: "@extends",
        detail: "Specify generic parent class type",
        label: Some("@extends ClassName<Type>"),
    },
    TagDef {
        tag: "@implements",
        detail: "Specify generic interface type",
        label: Some("@implements InterfaceName<Type>"),
    },
    TagDef {
        tag: "@use",
        detail: "Specify generic trait type",
        label: Some("@use TraitName<Type>"),
    },
];

// ── Property tags ───────────────────────────────────────────────────────────

const PROPERTY_TAGS: &[TagDef] = &[TagDef {
    tag: "@var",
    detail: "Document the property type",
    label: Some("@var Type"),
}];

// ── Constant tags ───────────────────────────────────────────────────────────

const CONSTANT_TAGS: &[TagDef] = &[TagDef {
    tag: "@var",
    detail: "Document the constant type",
    label: Some("@var Type"),
}];

// ── General tags (available everywhere) ─────────────────────────────────────

const GENERAL_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@deprecated",
        detail: "Mark as deprecated",
        label: None,
    },
    TagDef {
        tag: "@see",
        detail: "Reference to related element",
        label: Some("@see ClassName::method()"),
    },
    TagDef {
        tag: "@since",
        detail: "Version when this was introduced",
        label: Some("@since 1.0.0"),
    },
    TagDef {
        tag: "@example",
        detail: "Reference to an example file",
        label: None,
    },
    TagDef {
        tag: "@link",
        detail: "URL to external documentation",
        label: Some("@link https://"),
    },
    TagDef {
        tag: "@internal",
        detail: "Mark as internal / not part of the public API",
        label: None,
    },
    TagDef {
        tag: "@todo",
        detail: "Document a to-do item",
        label: None,
    },
];

// ── PHPStan tags ────────────────────────────────────────────────────────────

const PHPSTAN_FUNCTION_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@phpstan-assert",
        detail: "PHPStan: assert parameter type after call",
        label: Some("@phpstan-assert Type $var"),
    },
    TagDef {
        tag: "@phpstan-assert-if-true",
        detail: "PHPStan: assert type when method returns true",
        label: Some("@phpstan-assert-if-true Type $var"),
    },
    TagDef {
        tag: "@phpstan-assert-if-false",
        detail: "PHPStan: assert type when method returns false",
        label: Some("@phpstan-assert-if-false Type $var"),
    },
    TagDef {
        tag: "@phpstan-self-out",
        detail: "PHPStan: narrow the type of $this after call",
        label: Some("@phpstan-self-out Type"),
    },
    TagDef {
        tag: "@phpstan-this-out",
        detail: "PHPStan: narrow the type of $this after call",
        label: Some("@phpstan-this-out Type"),
    },
    TagDef {
        tag: "@phpstan-ignore-next-line",
        detail: "PHPStan: suppress errors on the next line",
        label: None,
    },
    TagDef {
        tag: "@phpstan-type",
        detail: "PHPStan: define a local type alias",
        label: Some("@phpstan-type TypeName = Type"),
    },
    TagDef {
        tag: "@phpstan-import-type",
        detail: "PHPStan: import a type alias from another class",
        label: Some("@phpstan-import-type TypeName from ClassName"),
    },
];

const PHPSTAN_CLASS_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@phpstan-type",
        detail: "PHPStan: define a local type alias",
        label: Some("@phpstan-type TypeName = Type"),
    },
    TagDef {
        tag: "@phpstan-import-type",
        detail: "PHPStan: import a type alias from another class",
        label: Some("@phpstan-import-type TypeName from ClassName"),
    },
    TagDef {
        tag: "@phpstan-require-extends",
        detail: "PHPStan: require extending a specific class",
        label: Some("@phpstan-require-extends ClassName"),
    },
    TagDef {
        tag: "@phpstan-require-implements",
        detail: "PHPStan: require implementing a specific interface",
        label: Some("@phpstan-require-implements InterfaceName"),
    },
];

const PHPSTAN_PROPERTY_TAGS: &[TagDef] = &[];

// ─── Completion Builder ─────────────────────────────────────────────────────

/// Build completion items for PHPDoc tags based on context.
///
/// `content` is the full file text (used to extract symbol info and
/// detect already-documented parameters).
/// `prefix` is the partial tag the user has typed (e.g. `"@par"`, `"@"`).
/// `context` indicates what PHP symbol follows the docblock.
/// `position` is the cursor position (used to scan the docblock and the
/// following declaration).
/// `use_map` maps short class names to FQNs from `use` statements.
/// `file_namespace` is the file's declared namespace (if any).
///
/// Returns the list of matching `CompletionItem`s.
pub fn build_phpdoc_completions(
    content: &str,
    prefix: &str,
    context: DocblockContext,
    position: Position,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    // Extract symbol info for smart pre-filling
    let sym = extract_symbol_info(content, position);

    // Collect all applicable tag lists based on context
    let tag_lists: Vec<&[TagDef]> = match context {
        DocblockContext::FunctionOrMethod => {
            vec![FUNCTION_TAGS, GENERAL_TAGS, PHPSTAN_FUNCTION_TAGS]
        }
        DocblockContext::ClassLike => vec![CLASS_TAGS, GENERAL_TAGS, PHPSTAN_CLASS_TAGS],
        DocblockContext::Property => vec![PROPERTY_TAGS, GENERAL_TAGS, PHPSTAN_PROPERTY_TAGS],
        DocblockContext::Constant => vec![CONSTANT_TAGS, GENERAL_TAGS],
        DocblockContext::Unknown => vec![
            FUNCTION_TAGS,
            CLASS_TAGS,
            PROPERTY_TAGS,
            GENERAL_TAGS,
            PHPSTAN_FUNCTION_TAGS,
            PHPSTAN_CLASS_TAGS,
            PHPSTAN_PROPERTY_TAGS,
        ],
    };

    for tags in tag_lists {
        for def in tags {
            if !def.tag.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            if !seen.insert(def.tag) {
                continue;
            }

            // ── Smart items for @throws ─────────────────────────────
            if def.tag == "@throws"
                && matches!(
                    context,
                    DocblockContext::FunctionOrMethod | DocblockContext::Unknown
                )
            {
                let uncaught = find_uncaught_throw_types(content, position);
                let existing_throws = find_existing_throws_tags(content, position);

                // Filter out already-documented throws
                let missing: Vec<&String> = uncaught
                    .iter()
                    .filter(|t| {
                        !existing_throws
                            .iter()
                            .any(|e| e.eq_ignore_ascii_case(t.as_str()))
                    })
                    .collect();

                if !missing.is_empty() {
                    let use_insert_pos = find_use_insert_position(content);

                    for (idx, exc_type) in missing.iter().enumerate() {
                        let insert = format!("throws {}", exc_type);
                        let label = format!("@throws {}", exc_type);

                        // Build an auto-import edit if the exception type
                        // isn't already imported.
                        let additional_edits =
                            resolve_exception_fqn(exc_type, use_map, file_namespace)
                                .filter(|fqn| !has_use_import(content, fqn))
                                .and_then(|fqn| {
                                    build_use_edit(&fqn, use_insert_pos, file_namespace)
                                });

                        items.push(CompletionItem {
                            label,
                            kind: Some(CompletionItemKind::KEYWORD),
                            detail: Some(def.detail.to_string()),
                            insert_text: Some(insert),
                            filter_text: Some(def.tag.to_string()),
                            sort_text: Some(format!("0a_{}_{:03}", def.tag.to_lowercase(), idx)),
                            additional_text_edits: additional_edits,
                            ..CompletionItem::default()
                        });
                    }
                }
                // Always skip the generic fallback — either we emitted
                // smart items above, or all thrown exceptions are already
                // documented / there are none.
                continue;
            }

            // ── Smart items for @param ──────────────────────────────
            if def.tag == "@param" {
                // If the function has parameters, offer smart pre-filled
                // items for each undocumented one.  When ALL params are
                // already documented (or the function has none), skip
                // entirely — the generic fallback is not useful.
                if !sym.params.is_empty() {
                    let existing = find_existing_param_tags(content, position);
                    let mut param_idx = 0u32;

                    for (type_hint, name) in &sym.params {
                        // Skip params already documented
                        if existing.iter().any(|e| e == name) {
                            continue;
                        }

                        let (insert, label) = if let Some(th) = type_hint {
                            (
                                format!("param {} {}", th, name),
                                format!("@param {} {}", th, name),
                            )
                        } else {
                            (format!("param {}", name), format!("@param {}", name))
                        };

                        items.push(CompletionItem {
                            label,
                            kind: Some(CompletionItemKind::KEYWORD),
                            detail: Some(def.detail.to_string()),
                            insert_text: Some(insert),
                            filter_text: Some(def.tag.to_string()),
                            sort_text: Some(format!(
                                "0a_{}_{:03}",
                                def.tag.to_lowercase(),
                                param_idx
                            )),
                            ..CompletionItem::default()
                        });
                        param_idx += 1;
                    }
                }
                // Always skip the generic fallback for @param — either
                // we emitted smart items above, or all params are
                // documented / there are none.
                continue;
            }

            // ── Smart item for @return ──────────────────────────────
            if def.tag == "@return" {
                if has_existing_return_tag(content, position) {
                    continue;
                }
                if let Some(ref ret) = sym.return_type
                    && ret != "void"
                {
                    items.push(CompletionItem {
                        label: format!("@return {}", ret),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some(def.detail.to_string()),
                        insert_text: Some(format!("return {}", ret)),
                        filter_text: Some(def.tag.to_string()),
                        sort_text: Some(format!("0a_{}", def.tag.to_lowercase())),
                        ..CompletionItem::default()
                    });
                }
                // Always skip the generic fallback — either we emitted
                // a smart item above, or the return type is void / not
                // detectable, or @return is already documented.
                continue;
            }

            // ── Smart item for @var on properties / constants ───────
            if def.tag == "@var"
                && matches!(
                    context,
                    DocblockContext::Property | DocblockContext::Constant
                )
                && let Some(ref th) = sym.type_hint
            {
                items.push(CompletionItem {
                    label: format!("@var {}", th),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(def.detail.to_string()),
                    insert_text: Some(format!("var {}", th)),
                    filter_text: Some(def.tag.to_string()),
                    sort_text: Some(format!("0a_{}", def.tag.to_lowercase())),
                    ..CompletionItem::default()
                });
                continue;
            }

            // ── Generic fallback ────────────────────────────────────
            let display_label = def.label.unwrap_or(def.tag);

            // PHPStan tags sort after standard tags.
            let sort_prefix = if def.tag.starts_with("@phpstan") {
                "2"
            } else {
                "1"
            };

            items.push(CompletionItem {
                label: display_label.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(def.detail.to_string()),
                insert_text: Some(strip_at(def.tag).to_string()),
                filter_text: Some(def.tag.to_string()),
                sort_text: Some(format!("{}_{}", sort_prefix, def.tag.to_lowercase())),
                ..CompletionItem::default()
            });
        }
    }

    items
}
