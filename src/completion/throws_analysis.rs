//! Throws analysis: scanning, catch-block filtering, and uncaught detection.
//!
//! This module provides a complete throws-analysis pipeline used by both
//! `phpdoc.rs` (for `@throws` tag completion) and `catch_completion.rs`
//! (for catch-clause exception suggestions).
//!
//! **Low-level scanning** (used by both callers):
//!   - Find `throw new Type(…)` statements in a block of PHP code
//!   - Find `throw $this->method(…)` / `throw self::method(…)` patterns
//!     and resolve the called method's return type
//!   - Find method calls and collect their `@throws` docblock annotations
//!   - Look up a method's return type from its declaration or docblock
//!   - Look up a method's `@throws` tags from its docblock
//!
//! **High-level analysis** (used by phpdoc completion):
//!   - Extract the function body following a docblock
//!   - Find `try/catch` blocks and their caught exception types
//!   - Determine which thrown exceptions are **not** caught
//!   - Resolve exception short names to FQNs via use-map / namespace
//!   - Check whether a `use` import already exists in the file
//!
//! Callers that only need type names can map `ThrowInfo::type_name`;
//! callers that need offset information (e.g. for catch-block filtering)
//! use the full `ThrowInfo` struct.

use std::collections::HashMap;

use tower_lsp::lsp_types::Position;

use super::comment_position::position_to_byte_offset;
use crate::util::short_name;

/// Information about a `throw` statement (or throw-expression) found in
/// a block of PHP source code.
#[derive(Debug)]
pub struct ThrowInfo {
    /// The exception type name as written in source (e.g.
    /// `"InvalidArgumentException"`, `"\\RuntimeException"`,
    /// `"Exceptions\\Custom"`).
    pub type_name: String,
    /// Byte offset of this throw statement relative to the start of the
    /// scanned block.
    pub offset: usize,
}

// ─── Core Scanning Primitives ───────────────────────────────────────────────

/// Find all `throw new Type(…)` statements in the given PHP source text.
///
/// Returns a [`ThrowInfo`] for each statement with the type name and the
/// byte offset of the `throw` keyword within `body`.
pub fn find_throw_statements(body: &str) -> Vec<ThrowInfo> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        // Skip string literals
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            pos = skip_string_forward(bytes, pos);
            continue;
        }

        // Skip line comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            pos = skip_line_comment(bytes, pos);
            continue;
        }

        // Skip block comments
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos = skip_block_comment(bytes, pos);
            continue;
        }

        // Look for `throw` keyword
        if pos + 5 <= len && &body[pos..pos + 5] == "throw" {
            let before_ok =
                pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric() && bytes[pos - 1] != b'_';
            let after_ok = pos + 5 >= len
                || (!bytes[pos + 5].is_ascii_alphanumeric() && bytes[pos + 5] != b'_');
            if before_ok && after_ok {
                let after_throw = body[pos + 5..].trim_start();
                if after_throw.starts_with("new ")
                    || after_throw.starts_with("new\t")
                    || after_throw.starts_with("new\n")
                {
                    let after_new = after_throw[3..].trim_start();
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

/// Find `throw $this->method(…)` / `throw self::method(…)` /
/// `throw static::method(…)` patterns and resolve the called method's
/// return type from its declaration or docblock in the same file.
///
/// Returns a [`ThrowInfo`] for each resolved throw-expression.
pub fn find_throw_expression_types(body: &str, file_content: &str) -> Vec<ThrowInfo> {
    let mut results = Vec::new();
    let patterns: &[&str] = &["$this->", "self::", "static::"];

    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            pos = skip_string_forward(bytes, pos);
            continue;
        }
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            pos = skip_line_comment(bytes, pos);
            continue;
        }
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos = skip_block_comment(bytes, pos);
            continue;
        }

        // Look for `throw` keyword
        if pos + 5 <= len && &body[pos..pos + 5] == "throw" {
            let before_ok =
                pos == 0 || !bytes[pos - 1].is_ascii_alphanumeric() && bytes[pos - 1] != b'_';
            let after_ok = pos + 5 >= len
                || (!bytes[pos + 5].is_ascii_alphanumeric() && bytes[pos + 5] != b'_');
            if before_ok && after_ok {
                let after_throw = body[pos + 5..].trim_start();
                // Skip `throw new` (handled by find_throw_statements)
                if !after_throw.starts_with("new ")
                    && !after_throw.starts_with("new\t")
                    && !after_throw.starts_with("new\n")
                {
                    for pat in patterns {
                        if let Some(rest) = after_throw.strip_prefix(pat) {
                            let name_end = rest
                                .find(|c: char| !c.is_alphanumeric() && c != '_')
                                .unwrap_or(rest.len());
                            let method_name = &rest[..name_end];
                            if !method_name.is_empty()
                                && let Some(ret_type) =
                                    find_method_return_type(file_content, method_name)
                            {
                                results.push(ThrowInfo {
                                    type_name: ret_type,
                                    offset: pos,
                                });
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
/// `static::method(…)`) in the given PHP source text and collect
/// `@throws` annotations from those methods' docblocks in the same file.
///
/// This propagates `@throws` declarations: if method A calls method B
/// and B declares `@throws SomeException`, then A should also be aware
/// of that exception.
///
/// Returns a [`ThrowInfo`] for each propagated throw, with the byte
/// offset set to the call site so that catch-block filtering works.
pub fn find_propagated_throws(body: &str, file_content: &str) -> Vec<ThrowInfo> {
    let mut results = Vec::new();
    let mut seen_methods = std::collections::HashSet::new();
    let patterns: &[&str] = &["$this->", "self::", "static::"];

    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        if bytes[pos] == b'\'' || bytes[pos] == b'"' {
            pos = skip_string_forward(bytes, pos);
            continue;
        }
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'/' {
            pos = skip_line_comment(bytes, pos);
            continue;
        }
        if pos + 1 < len && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            pos = skip_block_comment(bytes, pos);
            continue;
        }

        for pat in patterns {
            if pos + pat.len() <= len && &body[pos..pos + pat.len()] == *pat {
                let before_ok = if *pat == "$this->" {
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

                let after_name = after_pat[name_end..].trim_start();
                if !method_name.is_empty()
                    && after_name.starts_with('(')
                    && seen_methods.insert(method_name.to_string())
                {
                    let throws = find_method_throws_tags(file_content, method_name);
                    for t in throws {
                        results.push(ThrowInfo {
                            type_name: t,
                            offset: pos,
                        });
                    }
                }
                break;
            }
        }

        pos += 1;
    }

    results
}

/// Find inline `/** @throws ExceptionType */` annotations in a block of
/// PHP code.
///
/// These are single-line docblock comments that developers place inside
/// code (often in a try block) to hint at exceptions thrown by code that
/// doesn't have `@throws` annotations itself.
///
/// Returns the short type names found.
pub fn find_inline_throws_annotations(body: &str) -> Vec<String> {
    let mut results = Vec::new();
    let bytes = body.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos + 6 < len {
        // Look for `/**`
        if bytes[pos] == b'/' && pos + 2 < len && bytes[pos + 1] == b'*' && bytes[pos + 2] == b'*' {
            let doc_start = pos;
            pos += 3;

            // Find the closing `*/`
            let mut doc_end = None;
            while pos + 1 < len {
                if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
                    doc_end = Some(pos + 2);
                    break;
                }
                pos += 1;
            }

            if let Some(end) = doc_end {
                let docblock = &body[doc_start..end];
                for line in docblock.lines() {
                    let trimmed = line
                        .trim()
                        .trim_start_matches('/')
                        .trim_start_matches('*')
                        .trim();
                    if let Some(rest) = trimmed.strip_prefix("@throws") {
                        let rest = rest.trim();
                        if let Some(type_name) = rest.split_whitespace().next() {
                            let clean = type_name
                                .trim_start_matches('\\')
                                .trim_end_matches('*')
                                .trim_end_matches('/');
                            if !clean.is_empty() && !clean.starts_with('$') {
                                results.push(clean.to_string());
                            }
                        }
                    }
                }
                pos = end;
                continue;
            }
        }

        pos += 1;
    }

    results
}

// ─── Method Lookup Helpers ──────────────────────────────────────────────────

/// Find the return type of a method by scanning the file content for its
/// declaration.
///
/// Checks the native return type hint first, then falls back to the
/// `@return` tag in the method's docblock.  Skips visibility and
/// modifier keywords between the docblock and the `function` keyword.
///
/// Returns the short type name (last segment after `\`), or `None` if
/// the method is not found or has no resolvable return type.
pub fn find_method_return_type(file_content: &str, method_name: &str) -> Option<String> {
    let search = format!("function {}", method_name);

    let mut search_start = 0;
    while let Some(func_pos) = file_content[search_start..].find(&search) {
        let abs_pos = search_start + func_pos;
        search_start = abs_pos + search.len();

        let after_pos = abs_pos + search.len();
        if after_pos < file_content.len() {
            let next_byte = file_content.as_bytes()[after_pos];
            if next_byte.is_ascii_alphanumeric() || next_byte == b'_' {
                continue;
            }
        }

        // Check the native return type: find matching `)` then `: Type`
        let after = &file_content[after_pos..];
        if let Some(paren_start) = after.find('(')
            && let Some(close_offset) =
                find_matching_delimiter_forward(after, paren_start, b'(', b')')
        {
            let after_close = after[close_offset + 1..].trim_start();
            if let Some(rest) = after_close.strip_prefix(':') {
                let rest = rest.trim_start();
                let type_end = rest.find(['{', ';']).unwrap_or(rest.len());
                let type_str = rest[..type_end].trim().trim_start_matches('?');
                if !type_str.is_empty() {
                    let short = type_str
                        .trim_start_matches('\\')
                        .rsplit('\\')
                        .next()
                        .unwrap_or(type_str);
                    return Some(short.to_string());
                }
            }
        }

        // Check docblock @return, skipping visibility/modifier keywords
        let before = skip_modifiers_backward(&file_content[..abs_pos]);
        if before.ends_with("*/")
            && let Some(doc_start) = before.rfind("/**")
        {
            let docblock = &before[doc_start..];
            for line in docblock.lines() {
                let trimmed = line
                    .trim()
                    .trim_start_matches('/')
                    .trim_start_matches('*')
                    .trim();
                if let Some(rest) = trimmed.strip_prefix("@return") {
                    let rest = rest.trim();
                    if let Some(type_str) = rest.split_whitespace().next() {
                        let clean = type_str.trim_start_matches('\\').trim_start_matches('?');
                        let short = short_name(clean);
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
        break;
    }

    None
}

/// Find `@throws` tags in a method's docblock by scanning the file
/// content for the method declaration.
///
/// Skips visibility and modifier keywords between the docblock and the
/// `function` keyword.
///
/// Returns the short type names declared in `@throws` tags.
pub fn find_method_throws_tags(file_content: &str, method_name: &str) -> Vec<String> {
    let mut throws = Vec::new();
    let search = format!("function {}", method_name);

    let mut search_start = 0;
    while let Some(func_pos) = file_content[search_start..].find(&search) {
        let abs_pos = search_start + func_pos;
        search_start = abs_pos + search.len();

        // Verify word boundary after
        let after_pos = abs_pos + search.len();
        if after_pos < file_content.len() {
            let next_byte = file_content.as_bytes()[after_pos];
            if next_byte.is_ascii_alphanumeric() || next_byte == b'_' {
                continue;
            }
        }

        // Look backward for a docblock, skipping visibility/modifier keywords
        let before = skip_modifiers_backward(&file_content[..abs_pos]);
        if before.ends_with("*/")
            && let Some(doc_start) = before.rfind("/**")
        {
            let docblock = &before[doc_start..];
            for line in docblock.lines() {
                let trimmed = line
                    .trim()
                    .trim_start_matches('/')
                    .trim_start_matches('*')
                    .trim();
                if let Some(rest) = trimmed.strip_prefix("@throws") {
                    let rest = rest.trim();
                    if let Some(type_str) = rest.split_whitespace().next() {
                        let clean = type_str
                            .trim_end_matches('/')
                            .trim_end_matches('*')
                            .trim_start_matches('\\');
                        let short = short_name(clean);
                        if !short.is_empty() {
                            throws.push(short.to_string());
                        }
                    }
                }
            }
        }
        break;
    }

    throws
}

// ─── Internal Helpers ───────────────────────────────────────────────────────

/// Skip backward past PHP visibility and modifier keywords
/// (`public`, `protected`, `private`, `static`, `abstract`, `final`,
/// `readonly`) to locate the docblock that precedes a method
/// declaration.
///
/// Returns the trimmed prefix of `text` with modifiers stripped.
fn skip_modifiers_backward(text: &str) -> &str {
    const MODIFIERS: &[&str] = &[
        "private",
        "protected",
        "public",
        "static",
        "abstract",
        "final",
        "readonly",
    ];

    let mut s = text.trim_end();
    loop {
        let mut found = false;
        for modifier in MODIFIERS {
            if s.ends_with(modifier) {
                let start = s.len() - modifier.len();
                if start == 0
                    || (!s.as_bytes()[start - 1].is_ascii_alphanumeric()
                        && s.as_bytes()[start - 1] != b'_')
                {
                    s = s[..start].trim_end();
                    found = true;
                    break;
                }
            }
        }
        if !found {
            break;
        }
    }
    s
}

/// Find the matching closing delimiter for an opening delimiter at
/// `open_pos`, respecting string literal nesting.
///
/// `open` and `close` are the delimiter bytes (e.g. `b'('` / `b')'`
/// or `b'{'` / `b'}'`).
fn find_matching_delimiter_forward(
    text: &str,
    open_pos: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    let bytes = text.as_bytes();
    if open_pos >= bytes.len() || bytes[open_pos] != open {
        return None;
    }

    let mut depth = 1i32;
    let mut pos = open_pos + 1;

    while pos < bytes.len() && depth > 0 {
        match bytes[pos] {
            b if b == open => depth += 1,
            b if b == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            b'\'' | b'"' => {
                let quote = bytes[pos];
                pos += 1;
                while pos < bytes.len() {
                    if bytes[pos] == b'\\' {
                        pos += 1;
                    } else if bytes[pos] == quote {
                        break;
                    }
                    pos += 1;
                }
            }
            _ => {}
        }
        pos += 1;
    }

    None
}

/// Skip past a string literal starting at `pos` (which must point to
/// the opening quote).  Returns the position after the closing quote.
fn skip_string_forward(bytes: &[u8], pos: usize) -> usize {
    let quote = bytes[pos];
    let mut i = pos + 1;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 1; // skip escaped char
        } else if bytes[i] == quote {
            return i + 1;
        }
        i += 1;
    }
    i
}

/// Skip past a line comment (`//…`) starting at `pos`.  Returns the
/// position of the newline (or end of input).
fn skip_line_comment(bytes: &[u8], pos: usize) -> usize {
    let mut i = pos;
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    i
}

/// Skip past a block comment (`/* … */`) starting at `pos`.  Returns
/// the position after the closing `*/` (or end of input).
fn skip_block_comment(bytes: &[u8], pos: usize) -> usize {
    let mut i = pos + 2;
    while i + 1 < bytes.len() {
        if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            return i + 2;
        }
        i += 1;
    }
    i
}

// ─── High-Level Uncaught Throws Analysis ────────────────────────────────────

/// Extract the function/method body text that follows the docblock at
/// the cursor position.
///
/// Returns the text between the opening `{` and matching closing `}` of
/// the function/method declaration.  Returns `None` if the body cannot
/// be located (e.g. abstract method, or the docblock is not followed by
/// a function).
pub(crate) fn extract_function_body(content: &str, position: Position) -> Option<String> {
    let after_docblock = {
        let byte_offset = position_to_byte_offset(content, position);
        let after_cursor = &content[byte_offset.min(content.len())..];

        if let Some(close_pos) = after_cursor.find("*/") {
            after_cursor[close_pos + 2..].to_string()
        } else {
            after_cursor.to_string()
        }
    };

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
                    if let Some(try_body_end) =
                        crate::util::find_matching_forward(body, try_body_start, b'{', b'}')
                    {
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
                                                crate::util::find_matching_forward(
                                                    body,
                                                    catch_body_start,
                                                    b'{',
                                                    b'}',
                                                )
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
            let short = short_name(t);
            types.push(short.to_string());
        }
    }

    types
}

/// Determine which exception types in a function body are **not** caught
/// by an enclosing `try/catch` block.
///
/// Detects three patterns:
/// 1. `throw new ExceptionType(…)` -- direct instantiation
/// 2. `throw $this->method()` / `throw self::method()` / `throw static::method()`
///    -- the method's return type is the thrown exception type
/// 3. `$this->method()` / `self::method()` calls where the called method's
///    docblock declares `@throws ExceptionType` -- propagated throws
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

    /// Check whether a throw at `offset` in the function body is caught
    /// by one of the `catches`, given the short exception type name.
    fn is_caught_by(catches: &[CatchInfo], offset: usize, exc_name: &str) -> bool {
        catches.iter().any(|c| {
            offset > c.try_start
                && offset < c.try_end
                && c.type_names.iter().any(|ct| {
                    let ct_short = short_name(ct);
                    ct_short.eq_ignore_ascii_case(exc_name)
                        || ct_short == "Throwable"
                        || ct_short == "Exception"
                })
        })
    }

    // 1. Direct `throw new Type(…)` statements
    for throw in &throws {
        let short_name = throw
            .type_name
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(&throw.type_name);

        if !is_caught_by(&catches, throw.offset, short_name) && seen.insert(short_name.to_string())
        {
            uncaught.push(short_name.to_string());
        }
    }

    // 2. `throw $this->method()` -- return type of method is the thrown type
    for te in &throw_expr_types {
        let sn = short_name(te.type_name.trim_start_matches('\\'));
        if !sn.is_empty() && !is_caught_by(&catches, te.offset, sn) && seen.insert(sn.to_string()) {
            uncaught.push(sn.to_string());
        }
    }

    // 3. Propagated @throws from called methods
    for prop in &propagated {
        let sn = short_name(prop.type_name.trim_start_matches('\\'));
        if !sn.is_empty() && !is_caught_by(&catches, prop.offset, sn) && seen.insert(sn.to_string())
        {
            uncaught.push(sn.to_string());
        }
    }

    uncaught
}

// ─── Import Helpers ─────────────────────────────────────────────────────────

/// Resolve a short exception type name to its fully-qualified name using
/// the file's `use` map and namespace.
///
/// Returns the FQN (without leading `\`) if found, or `None` if the type
/// is already unqualified and in the global namespace.
pub(super) fn resolve_exception_fqn(
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
pub(super) fn has_use_import(content: &str, fqn: &str) -> bool {
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Low-level scanning tests ────────────────────────────────────────

    #[test]
    fn test_find_throw_statements_basic() {
        let body = r#"
            throw new InvalidArgumentException("bad");
            throw new \RuntimeException("oops");
        "#;
        let throws = find_throw_statements(body);
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].type_name, "InvalidArgumentException");
        assert_eq!(throws[1].type_name, "\\RuntimeException");
    }

    #[test]
    fn test_find_throw_statements_skips_strings() {
        let body = r#"
            $msg = "throw new FakeException()";
            throw new RealException("msg");
        "#;
        let throws = find_throw_statements(body);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].type_name, "RealException");
    }

    #[test]
    fn test_find_throw_statements_skips_comments() {
        let body = r#"
            // throw new CommentException();
            /* throw new BlockException(); */
            throw new RealException("msg");
        "#;
        let throws = find_throw_statements(body);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].type_name, "RealException");
    }

    #[test]
    fn test_find_method_throws_tags_basic() {
        let content = r#"
    /**
     * @throws InvalidArgumentException
     * @throws \RuntimeException
     */
    public function doSomething(): void {
    }
        "#;
        let tags = find_method_throws_tags(content, "doSomething");
        assert_eq!(tags, vec!["InvalidArgumentException", "RuntimeException"]);
    }

    #[test]
    fn test_find_method_throws_tags_with_modifiers() {
        let content = r#"
    /**
     * @throws InvalidArgumentException
     */
    private static function doSomething(): void {
    }
        "#;
        let tags = find_method_throws_tags(content, "doSomething");
        assert_eq!(tags, vec!["InvalidArgumentException"]);
    }

    #[test]
    fn test_find_method_return_type_native() {
        let content = r#"
    public function createException(): RuntimeException {
    }
        "#;
        let ret = find_method_return_type(content, "createException");
        assert_eq!(ret, Some("RuntimeException".to_string()));
    }

    #[test]
    fn test_find_method_return_type_docblock() {
        let content = r#"
    /**
     * @return RuntimeException
     */
    public function createException() {
    }
        "#;
        let ret = find_method_return_type(content, "createException");
        assert_eq!(ret, Some("RuntimeException".to_string()));
    }

    #[test]
    fn test_find_method_return_type_skips_void() {
        let content = r#"
    /**
     * @return void
     */
    public function doNothing() {
    }
        "#;
        let ret = find_method_return_type(content, "doNothing");
        assert_eq!(ret, None);
    }

    #[test]
    fn test_find_inline_throws_annotations() {
        let body = r#"
            /** @throws InvalidArgumentException */
            $client->request();
            /** @throws RuntimeException when things go wrong */
            $db->query();
        "#;
        let annotations = find_inline_throws_annotations(body);
        assert_eq!(
            annotations,
            vec!["InvalidArgumentException", "RuntimeException"]
        );
    }

    #[test]
    fn test_find_propagated_throws() {
        let file_content = r#"
    /**
     * @throws IOException
     * @throws NetworkException
     */
    public function riskyMethod(): void {
        // ...
    }

    public function caller(): void {
        $this->riskyMethod();
    }
        "#;
        // Scan the body of `caller`
        let body = "$this->riskyMethod();";
        let propagated = find_propagated_throws(body, file_content);
        let names: Vec<&str> = propagated.iter().map(|t| t.type_name.as_str()).collect();
        assert_eq!(names, vec!["IOException", "NetworkException"]);
    }

    #[test]
    fn test_find_throw_expression_types() {
        let file_content = r#"
    public function createException(): RuntimeException {
        return new RuntimeException("oops");
    }

    public function caller(): void {
        throw $this->createException();
    }
        "#;
        let body = "throw $this->createException();";
        let types = find_throw_expression_types(body, file_content);
        assert_eq!(types.len(), 1);
        assert_eq!(types[0].type_name, "RuntimeException");
    }

    #[test]
    fn test_skip_modifiers_backward() {
        assert_eq!(skip_modifiers_backward("    public static "), "");
        assert_eq!(
            skip_modifiers_backward("/** @return void */ private "),
            "/** @return void */"
        );
        assert_eq!(
            skip_modifiers_backward("no modifiers here"),
            "no modifiers here"
        );
    }

    #[test]
    fn test_find_method_return_type_with_nested_parens() {
        let content = r#"
    public function createException(array $opts = array()): RuntimeException {
    }
        "#;
        let ret = find_method_return_type(content, "createException");
        assert_eq!(ret, Some("RuntimeException".to_string()));
    }

    // ── High-level analysis tests ───────────────────────────────────────

    #[test]
    fn test_extract_function_body_basic() {
        let content =
            "<?php\n/** @return void */\nfunction foo(): void {\n    echo \"hello\";\n}\n";
        let pos = Position {
            line: 1,
            character: 5,
        };
        let body = extract_function_body(content, pos);
        assert!(body.is_some());
        assert!(body.unwrap().contains("echo"));
    }

    #[test]
    fn test_extract_function_body_abstract() {
        let content = "<?php\n/** @return void */\nabstract function foo(): void;\n";
        let pos = Position {
            line: 1,
            character: 5,
        };
        let body = extract_function_body(content, pos);
        assert!(body.is_none());
    }

    #[test]
    fn test_extract_function_body_with_nested_braces() {
        let content = concat!(
            "<?php\n",
            "/** @return void */\n",
            "function foo(): void {\n",
            "    if (true) {\n",
            "        echo 'inner';\n",
            "    }\n",
            "}\n",
        );
        let pos = Position {
            line: 1,
            character: 5,
        };
        let body = extract_function_body(content, pos).unwrap();
        assert!(body.contains("if (true)"));
        assert!(body.contains("echo 'inner'"));
    }

    #[test]
    fn test_find_catch_blocks_basic() {
        let body = r#"
            try {
                throw new InvalidArgumentException("bad");
            } catch (InvalidArgumentException $e) {
                // handled
            }
            throw new RuntimeException("oops");
        "#;
        let catches = find_catch_blocks(body);
        assert_eq!(catches.len(), 1);
        assert_eq!(catches[0].type_names, vec!["InvalidArgumentException"]);
    }

    #[test]
    fn test_find_catch_blocks_multi_catch() {
        let body = r#"
            try {
                doSomething();
            } catch (InvalidArgumentException | RuntimeException $e) {
                // handled
            }
        "#;
        let catches = find_catch_blocks(body);
        assert_eq!(catches.len(), 1);
        assert_eq!(
            catches[0].type_names,
            vec!["InvalidArgumentException", "RuntimeException"]
        );
    }

    #[test]
    fn test_parse_catch_types_basic() {
        assert_eq!(
            parse_catch_types("InvalidArgumentException $e"),
            vec!["InvalidArgumentException"]
        );
    }

    #[test]
    fn test_parse_catch_types_multi() {
        assert_eq!(
            parse_catch_types("\\InvalidArgumentException | \\RuntimeException $e"),
            vec!["InvalidArgumentException", "RuntimeException"]
        );
    }

    #[test]
    fn test_find_uncaught_throw_types_all_caught() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public function bar(): void {\n",
            "        try {\n",
            "            throw new InvalidArgumentException(\"bad\");\n",
            "        } catch (InvalidArgumentException $e) {\n",
            "            // handled\n",
            "        }\n",
            "    }\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 5,
        };
        let uncaught = find_uncaught_throw_types(content, pos);
        assert!(uncaught.is_empty());
    }

    #[test]
    fn test_find_uncaught_throw_types_uncaught() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public function bar(): void {\n",
            "        throw new RuntimeException(\"oops\");\n",
            "    }\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 5,
        };
        let uncaught = find_uncaught_throw_types(content, pos);
        assert_eq!(uncaught, vec!["RuntimeException"]);
    }

    #[test]
    fn test_find_uncaught_throw_types_mixed() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @\n",
            "     */\n",
            "    public function bar(): void {\n",
            "        try {\n",
            "            throw new InvalidArgumentException(\"bad\");\n",
            "        } catch (InvalidArgumentException $e) {\n",
            "            // handled\n",
            "        }\n",
            "        throw new RuntimeException(\"oops\");\n",
            "    }\n",
            "}\n",
        );
        let pos = Position {
            line: 3,
            character: 5,
        };
        let uncaught = find_uncaught_throw_types(content, pos);
        assert_eq!(uncaught, vec!["RuntimeException"]);
    }

    // ── Import helper tests ─────────────────────────────────────────────

    #[test]
    fn test_resolve_exception_fqn_from_use_map() {
        let mut use_map = HashMap::new();
        use_map.insert(
            "RuntimeException".to_string(),
            "App\\Exceptions\\RuntimeException".to_string(),
        );
        let result = resolve_exception_fqn("RuntimeException", &use_map, &None);
        assert_eq!(
            result,
            Some("App\\Exceptions\\RuntimeException".to_string())
        );
    }

    #[test]
    fn test_resolve_exception_fqn_from_namespace() {
        let use_map = HashMap::new();
        let ns = Some("App\\Services".to_string());
        let result = resolve_exception_fqn("CustomException", &use_map, &ns);
        assert_eq!(result, Some("App\\Services\\CustomException".to_string()));
    }

    #[test]
    fn test_resolve_exception_fqn_global() {
        let use_map = HashMap::new();
        let result = resolve_exception_fqn("RuntimeException", &use_map, &None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_has_use_import_direct() {
        let content = "<?php\nuse App\\Exceptions\\RuntimeException;\n";
        assert!(has_use_import(content, "App\\Exceptions\\RuntimeException"));
        assert!(!has_use_import(content, "App\\Exceptions\\LogicException"));
    }

    #[test]
    fn test_has_use_import_group() {
        let content = "<?php\nuse App\\Exceptions\\{RuntimeException, LogicException};\n";
        assert!(has_use_import(content, "App\\Exceptions\\RuntimeException"));
        assert!(has_use_import(content, "App\\Exceptions\\LogicException"));
        assert!(!has_use_import(content, "App\\Exceptions\\CustomException"));
    }

    #[test]
    fn test_has_use_import_alias() {
        let content = "<?php\nuse App\\Exceptions\\RuntimeException as RE;\n";
        assert!(has_use_import(content, "App\\Exceptions\\RuntimeException"));
    }
}
