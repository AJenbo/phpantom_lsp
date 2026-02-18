//! Smart catch clause exception completion.
//!
//! When the cursor is inside `catch (|)` or `catch (Partial|)`, this
//! module analyses the corresponding `try` block to find all exception
//! types that can be thrown, and suggests them as completions.
//!
//! Sources of thrown exception types (in priority order):
//!   1. `throw new ExceptionType(…)` statements in the try block
//!   2. Inline `/** @throws ExceptionType */` annotations in the try block
//!   3. Propagated `@throws` from methods called in the try block
//!   4. `throw $this->method()` / `throw self::method()` return types
//!
//! The inline `/** @throws */` annotation is an escape hatch that lets
//! developers document exceptions from dependencies that don't have
//! `@throws` tags themselves.

use tower_lsp::lsp_types::*;

use super::phpdoc::{find_throw_statements, position_to_byte_offset};

/// Information about the catch clause context at the cursor position.
#[derive(Debug)]
pub struct CatchContext {
    /// The partial class name the user has typed so far (may be empty).
    pub partial: String,
    /// Exception type names found in the corresponding try block.
    pub suggested_types: Vec<String>,
    /// Types already listed in this catch clause (for multi-catch `|`).
    #[allow(dead_code)]
    pub already_listed: Vec<String>,
    /// Whether specific thrown types were discovered in the try block.
    /// When `false`, the caller should fall back to generic class
    /// completion instead of showing only `Throwable`.
    pub has_specific_types: bool,
}

/// Detect whether the cursor is inside a `catch (…)` clause's type
/// position, and if so, return a [`CatchContext`] with the try block's
/// thrown exception types.
///
/// Returns `None` if the cursor is not in a catch clause type position.
pub fn detect_catch_context(content: &str, position: Position) -> Option<CatchContext> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    // Walk backward from cursor to find the opening `(` of the catch clause,
    // collecting what's been typed so far.
    let (catch_paren_offset, partial, already_listed) = find_catch_paren(before_cursor)?;

    // From the `(` position, walk backward to find the `catch` keyword.
    let before_paren = &content[..catch_paren_offset];
    let trimmed = before_paren.trim_end();
    if !trimmed.ends_with("catch") {
        return None;
    }

    // Verify `catch` is a whole word
    let catch_end = trimmed.len();
    let catch_start = catch_end - 5;
    if catch_start > 0 {
        let prev_byte = trimmed.as_bytes()[catch_start - 1];
        if prev_byte.is_ascii_alphanumeric() || prev_byte == b'_' {
            return None;
        }
    }

    // Now find the matching try block by scanning backward from `catch`.
    let before_catch = trimmed[..catch_start].trim_end();

    // The text just before `catch` should be `}` (closing the try block
    // or a previous catch block).
    if !before_catch.ends_with('}') {
        return None;
    }

    // Find the try block: walk back through possible catch/finally blocks
    // to find the original `try {`.
    let try_body = find_try_block_body(content, before_catch)?;

    // Analyse the try block for thrown exception types.
    let mut suggested_types = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Direct `throw new ExceptionType(…)` statements
    let throws = find_throw_statements(&try_body);
    for throw in &throws {
        let short_name = throw
            .type_name
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(&throw.type_name);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 2. Inline `/** @throws ExceptionType */` annotations
    let inline_throws = find_inline_throws_annotations(&try_body);
    for exc_type in &inline_throws {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 3. Propagated @throws from called methods
    let propagated = find_propagated_throws_in_block(&try_body, content);
    for exc_type in &propagated {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 4. `throw $this->method()` / `throw self::method()` return types
    let throw_expr_types = find_throw_expression_types_in_block(&try_body, content);
    for exc_type in &throw_expr_types {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // Track whether we found any specific thrown types before adding
    // the universal Throwable fallback.
    let has_specific_types = !suggested_types.is_empty();

    // Always offer \Throwable as a catch-all safety net
    if seen.insert("throwable".to_string()) {
        suggested_types.push("\\Throwable".to_string());
    }

    // Filter out types already listed in this catch clause
    let already_lower: Vec<String> = already_listed.iter().map(|s| s.to_lowercase()).collect();
    suggested_types.retain(|t| !already_lower.contains(&t.to_lowercase()));

    Some(CatchContext {
        partial,
        suggested_types,
        already_listed,
        has_specific_types,
    })
}

/// Build LSP completion items from a [`CatchContext`].
///
/// Smart exception suggestions sort before any fallback items.
/// `\Throwable` is always offered but sorted last among the suggestions.
pub fn build_catch_completions(ctx: &CatchContext) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let partial_lower = ctx.partial.to_lowercase();

    for (idx, exc_type) in ctx.suggested_types.iter().enumerate() {
        let label = exc_type.trim_start_matches('\\');

        // Filter by the partial text the user has typed
        if !partial_lower.is_empty() && !label.to_lowercase().starts_with(&partial_lower) {
            continue;
        }

        // Sort \Throwable after specific exception types
        let sort_text = if exc_type.starts_with('\\') {
            format!("1_{:03}_{}", idx, label)
        } else {
            format!("0_{:03}_{}", idx, label)
        };

        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Exception thrown in try block".to_string()),
            sort_text: Some(sort_text),
            filter_text: Some(label.to_string()),
            ..CompletionItem::default()
        });
    }

    items
}

// ─── Internal helpers ───────────────────────────────────────────────────────

/// Walk backward from the cursor position to find the `(` of a catch clause.
///
/// Returns `(paren_byte_offset, partial_typed, already_listed_types)` or
/// `None` if no suitable `(` is found.
fn find_catch_paren(before_cursor: &str) -> Option<(usize, String, Vec<String>)> {
    let bytes = before_cursor.as_bytes();
    let mut pos = bytes.len();
    let mut depth = 0i32;

    // Walk backward collecting the text inside the parentheses
    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    // Found our opening paren
                    let inside = &before_cursor[pos + 1..];
                    let (partial, already_listed) = parse_catch_paren_content(inside);
                    return Some((pos, partial, already_listed));
                }
                depth -= 1;
            }
            // Stop at semicolons, opening braces — we've gone too far
            b';' | b'{' => return None,
            _ => {}
        }
    }

    None
}

/// Parse the content inside `catch (` up to the cursor.
///
/// For `catch (IOException | ` the partial is `""` and already_listed is
/// `["IOException"]`.
///
/// For `catch (IOEx` the partial is `"IOEx"` and already_listed is `[]`.
///
/// For `catch (IOException | Time` the partial is `"Time"` and
/// already_listed is `["IOException"]`.
fn parse_catch_paren_content(inside: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = inside.split('|').collect();
    let mut already_listed = Vec::new();

    if parts.len() <= 1 {
        // No `|` separator — everything is the partial
        let partial = inside.trim().trim_start_matches('\\').to_string();
        return (partial, already_listed);
    }

    // Everything except the last segment is an already-listed type
    for part in &parts[..parts.len() - 1] {
        let t = part.trim().trim_start_matches('\\');
        if !t.is_empty() {
            // Strip the variable name if present (shouldn't be before `|`, but be safe)
            let type_name = t.split_whitespace().next().unwrap_or(t);
            if !type_name.starts_with('$') {
                already_listed.push(type_name.to_string());
            }
        }
    }

    // The last segment is the partial
    let last = parts.last().unwrap_or(&"");
    let partial = last.trim().trim_start_matches('\\').to_string();

    (partial, already_listed)
}

/// Find the try block body by walking backward from the `}` that precedes
/// the `catch` keyword.
///
/// Handles chains like `try { … } catch (A $a) { … } catch (|)` by
/// walking back through previous catch (and finally) blocks to find the
/// original `try {`.
fn find_try_block_body(_content: &str, before_catch: &str) -> Option<String> {
    // `before_catch` ends with `}`. Find the matching `{`.
    let close_brace_offset = before_catch.len() - 1;

    // We need the absolute position in `content`. `before_catch` is a
    // prefix of `content` (after trimming), so we can use its length.
    // But actually, `before_catch` was derived by slicing `content`, so
    // we need to find where this `}` is in the full content.
    //
    // Walk backward to find the matching `{`.
    let block_open = find_matching_brace_reverse(before_catch, close_brace_offset)?;

    // Now check what keyword precedes this block.
    let before_block = before_catch[..block_open].trim_end();

    // Check for `)` — this block was a catch block
    if before_block.ends_with(')') {
        // Skip the catch clause parentheses
        let close_paren = before_block.len() - 1;
        let open_paren = find_matching_paren_reverse(before_block, close_paren)?;
        let before_paren = before_block[..open_paren].trim_end();

        // Should be `catch`
        if before_paren.ends_with("catch") {
            let kw_start = before_paren.len() - 5;
            // Verify whole word
            if kw_start == 0
                || (!before_paren.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                    && before_paren.as_bytes()[kw_start - 1] != b'_')
            {
                // Must be preceded by `}` of the previous block
                let before_kw = before_paren[..kw_start].trim_end();
                if before_kw.ends_with('}') {
                    // Recurse to find the try block
                    return find_try_block_body(_content, before_kw);
                }
            }
        }
        return None;
    }

    // Check for `finally`
    if before_block.ends_with("finally") {
        let kw_start = before_block.len() - 7;
        if kw_start == 0
            || (!before_block.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                && before_block.as_bytes()[kw_start - 1] != b'_')
        {
            let before_kw = before_block[..kw_start].trim_end();
            if before_kw.ends_with('}') {
                return find_try_block_body(_content, before_kw);
            }
        }
        return None;
    }

    // Check for `try`
    if before_block.ends_with("try") {
        let kw_start = before_block.len() - 3;
        if kw_start == 0
            || (!before_block.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                && before_block.as_bytes()[kw_start - 1] != b'_')
        {
            // Found it! Extract the try block body.
            let body = &before_catch[block_open + 1..close_brace_offset];
            return Some(body.to_string());
        }
    }

    None
}

/// Find the matching opening `{` for a closing `}` by walking backward.
fn find_matching_brace_reverse(text: &str, close_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if close_pos >= bytes.len() || bytes[close_pos] != b'}' {
        return None;
    }

    let mut depth = 1i32;
    let mut pos = close_pos;

    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b'}' => depth += 1,
            b'{' => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            // Skip string literals (walking backward is tricky, but we
            // do a simple quote-matching heuristic)
            b'\'' | b'"' => {
                let quote = bytes[pos];
                if pos > 0 {
                    pos -= 1;
                    while pos > 0 {
                        if bytes[pos] == quote {
                            // Check for escape — count preceding backslashes
                            let mut bs = 0;
                            let mut check = pos;
                            while check > 0 && bytes[check - 1] == b'\\' {
                                bs += 1;
                                check -= 1;
                            }
                            if bs % 2 == 0 {
                                break; // unescaped quote — string start
                            }
                        }
                        pos -= 1;
                    }
                }
            }
            _ => {}
        }
    }

    None
}

/// Find the matching opening `(` for a closing `)` by walking backward.
fn find_matching_paren_reverse(text: &str, close_pos: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if close_pos >= bytes.len() || bytes[close_pos] != b')' {
        return None;
    }

    let mut depth = 1i32;
    let mut pos = close_pos;

    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b')' => depth += 1,
            b'(' => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            _ => {}
        }
    }

    None
}

/// Find inline `/** @throws ExceptionType */` annotations in a block of code.
///
/// These are single-line docblock comments that developers can place
/// inside a try block to hint at exceptions thrown by code that doesn't
/// have `@throws` annotations itself.
fn find_inline_throws_annotations(body: &str) -> Vec<String> {
    let mut results = Vec::new();

    // Scan for `/** ... @throws ... */` patterns
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
                // Look for @throws in this docblock
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

/// Find `@throws` annotations propagated from method calls in a code block.
///
/// Looks for `$this->method(…)`, `self::method(…)`, `static::method(…)`
/// calls and reads their docblocks from the file content.
fn find_propagated_throws_in_block(body: &str, file_content: &str) -> Vec<String> {
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
                    let throws = find_method_throws_tags_local(file_content, method_name);
                    results.extend(throws);
                }
                break;
            }
        }

        pos += 1;
    }

    results
}

/// Find `throw $this->method()` / `throw self::method()` return types
/// in a code block.
fn find_throw_expression_types_in_block(body: &str, file_content: &str) -> Vec<String> {
    let mut results = Vec::new();

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
                                    find_method_return_type_local(file_content, method_name)
                            {
                                results.push(ret_type);
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

/// Skip PHP modifier keywords (visibility, static, abstract, final, readonly)
/// when walking backward from the `function` keyword to find a docblock.
///
/// Given text like `/** @throws X */\n    private static`, this strips
/// `static` and `private` to yield `/** @throws X */` so the caller can
/// check for a trailing `*/`.
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
                // Verify word boundary before the modifier
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

/// Find `@throws` tags in a method's docblock by scanning the file.
///
/// This is a local copy of `phpdoc::find_method_throws_tags` to avoid
/// making that function public.
fn find_method_throws_tags_local(file_content: &str, method_name: &str) -> Vec<String> {
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
        // like `private`, `public`, `static`, etc.
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
                        let clean = type_str.trim_start_matches('\\');
                        let short = clean.rsplit('\\').next().unwrap_or(clean);
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

/// Find the return type of a method by scanning the file content.
///
/// Local copy of `phpdoc::find_method_return_type`.
fn find_method_return_type_local(file_content: &str, method_name: &str) -> Option<String> {
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

        // Check the native return type
        let after = &file_content[after_pos..];
        if let Some(paren_start) = after.find('(')
            && let Some(close_offset) = find_matching_brace_forward(after, paren_start, b'(', b')')
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
                        let short = clean.rsplit('\\').next().unwrap_or(clean);
                        if !short.is_empty() {
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

/// Simple forward parenthesis/brace matching.
fn find_matching_brace_forward(text: &str, open_pos: usize, open: u8, close: u8) -> Option<usize> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_method_throws_tags_local_with_private() {
        let content = concat!(
            "<?php\n",
            "class Foo {\n",
            "    /** @throws ValidationException */\n",
            "    private function riskyOperation(): void {}\n",
            "}\n",
        );
        let result = find_method_throws_tags_local(content, "riskyOperation");
        assert_eq!(
            result,
            vec!["ValidationException"],
            "Should find @throws through 'private' modifier"
        );
    }

    #[test]
    fn test_find_method_throws_tags_local_with_protected_static() {
        let content = concat!(
            "<?php\n",
            "class Foo {\n",
            "    /** @throws RuntimeException */\n",
            "    protected static function dangerousCall(): void {}\n",
            "}\n",
        );
        let result = find_method_throws_tags_local(content, "dangerousCall");
        assert_eq!(
            result,
            vec!["RuntimeException"],
            "Should find @throws through 'protected static' modifiers"
        );
    }

    #[test]
    fn test_find_method_throws_tags_local_without_modifier() {
        let content = concat!(
            "<?php\n",
            "/** @throws LogicException */\n",
            "function standalone(): void {}\n",
        );
        let result = find_method_throws_tags_local(content, "standalone");
        assert_eq!(
            result,
            vec!["LogicException"],
            "Should find @throws on a standalone function (no modifier)"
        );
    }

    #[test]
    fn test_skip_modifiers_backward() {
        assert_eq!(
            skip_modifiers_backward("/** @throws X */\n    private"),
            "/** @throws X */"
        );
        assert_eq!(
            skip_modifiers_backward("/** @throws X */\n    private static"),
            "/** @throws X */"
        );
        assert_eq!(
            skip_modifiers_backward("/** @return Foo */\n    public final"),
            "/** @return Foo */"
        );
        assert_eq!(skip_modifiers_backward("/** docs */"), "/** docs */");
        // Should not strip partial word matches
        assert_eq!(skip_modifiers_backward("myprivate"), "myprivate");
    }

    #[test]
    fn test_propagated_throws_with_visibility() {
        // Full file content — cursor will be inside catch()
        //                                                    v cursor (line 5, char 17)
        // Line 0: <?php
        // Line 1: class Foo {
        // Line 2:     public function doStuff(): void {
        // Line 3:         try {
        // Line 4:             $this->riskyOperation();
        // Line 5:         } catch () {}
        // Line 6:     }
        // Line 7:
        // Line 8:     /** @throws ValidationException */
        // Line 9:     private function riskyOperation(): void {}
        // Line 10: }
        let full_content = concat!(
            "<?php\n",
            "class Foo {\n",
            "    public function doStuff(): void {\n",
            "        try {\n",
            "            $this->riskyOperation();\n",
            "        } catch () {}\n",
            "    }\n",
            "\n",
            "    /** @throws ValidationException */\n",
            "    private function riskyOperation(): void {}\n",
            "}\n",
        );

        // Character 17 is between `(` (char 16) and `)` (char 17) on line 5
        let pos = Position {
            line: 5,
            character: 17,
        };
        let ctx = detect_catch_context(full_content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"ValidationException".to_string()),
            "Should suggest ValidationException from propagated @throws on private method, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_propagated_throws_with_protected_static() {
        let full_content = concat!(
            "<?php\n",
            "class Bar {\n",
            "    public function handle(): void {\n",
            "        try {\n",
            "            $this->dangerousCall();\n",
            "        } catch () {}\n",
            "    }\n",
            "\n",
            "    /** @throws RuntimeException */\n",
            "    protected static function dangerousCall(): void {}\n",
            "}\n",
        );

        let pos = Position {
            line: 5,
            character: 17,
        };
        let ctx = detect_catch_context(full_content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string()),
            "Should suggest RuntimeException through protected static modifier, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_find_inline_throws_annotations() {
        let body = r#"
            /** @throws ModelNotFoundException */
            $model = SomeService::find($id);
            /** @throws \App\Exceptions\AuthException */
            $auth = doSomething();
        "#;
        let result = find_inline_throws_annotations(body);
        // Raw names are returned; short-name extraction happens in detect_catch_context
        assert_eq!(
            result,
            vec!["ModelNotFoundException", "App\\Exceptions\\AuthException"]
        );
    }

    #[test]
    fn test_find_inline_throws_multiline_docblock() {
        let body = r#"
            /**
             * @throws RuntimeException
             */
            doStuff();
        "#;
        let result = find_inline_throws_annotations(body);
        assert_eq!(result, vec!["RuntimeException"]);
    }

    #[test]
    fn test_parse_catch_paren_content_empty() {
        let (partial, already) = parse_catch_paren_content("");
        assert_eq!(partial, "");
        assert!(already.is_empty());
    }

    #[test]
    fn test_parse_catch_paren_content_partial() {
        let (partial, already) = parse_catch_paren_content("IOEx");
        assert_eq!(partial, "IOEx");
        assert!(already.is_empty());
    }

    #[test]
    fn test_parse_catch_paren_content_multi_catch() {
        let (partial, already) = parse_catch_paren_content("IOException | ");
        assert_eq!(partial, "");
        assert_eq!(already, vec!["IOException"]);
    }

    #[test]
    fn test_parse_catch_paren_content_multi_catch_with_partial() {
        let (partial, already) = parse_catch_paren_content("IOException | Time");
        assert_eq!(partial, "Time");
        assert_eq!(already, vec!["IOException"]);
    }

    #[test]
    fn test_parse_catch_paren_content_three_types() {
        let (partial, already) = parse_catch_paren_content("IOException | TimeoutException | ");
        assert_eq!(partial, "");
        assert_eq!(already, vec!["IOException", "TimeoutException"]);
    }

    #[test]
    fn test_detect_catch_context_always_includes_throwable() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new RuntimeException('error');\n",
            "} catch (",
        );
        let pos = Position {
            line: 3,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos).unwrap();
        assert!(
            ctx.suggested_types.contains(&"\\Throwable".to_string()),
            "Should always include \\Throwable, got: {:?}",
            ctx.suggested_types
        );
        assert!(ctx.has_specific_types);
    }

    #[test]
    fn test_detect_catch_context_no_specific_types_sets_flag() {
        let content = concat!("<?php\n", "try {\n", "    doSomething();\n", "} catch (",);
        let pos = Position {
            line: 3,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos).unwrap();
        assert!(
            !ctx.has_specific_types,
            "Should have no specific types when try block has no throws"
        );
        // Throwable is still offered
        assert!(ctx.suggested_types.contains(&"\\Throwable".to_string()));
    }

    #[test]
    fn test_detect_catch_context_simple() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new RuntimeException('error');\n",
            "} catch (",
        );
        let pos = Position {
            line: 3,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string()),
            "Should suggest RuntimeException, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_detect_catch_context_with_inline_throws() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    /** @throws ModelNotFoundException */\n",
            "    $model = SomeService::find($id);\n",
            "} catch (",
        );
        let pos = Position {
            line: 4,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some(), "Should detect catch context");
        let ctx = ctx.unwrap();
        assert!(
            ctx.suggested_types
                .contains(&"ModelNotFoundException".to_string()),
            "Should suggest ModelNotFoundException from inline @throws, got: {:?}",
            ctx.suggested_types
        );
    }

    #[test]
    fn test_detect_catch_context_multi_throw() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new IOException('io');\n",
            "    throw new TimeoutException('timeout');\n",
            "} catch (",
        );
        let pos = Position {
            line: 4,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert!(ctx.suggested_types.contains(&"IOException".to_string()));
        assert!(
            ctx.suggested_types
                .contains(&"TimeoutException".to_string())
        );
    }

    #[test]
    fn test_detect_catch_context_second_catch() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new IOException('io');\n",
            "    throw new TimeoutException('timeout');\n",
            "} catch (IOException $e) {\n",
            "    // handled\n",
            "} catch (",
        );
        let pos = Position {
            line: 6,
            character: 10,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some(), "Should detect second catch context");
        let ctx = ctx.unwrap();
        // Both types are in the try block
        assert!(ctx.suggested_types.contains(&"IOException".to_string()));
        assert!(
            ctx.suggested_types
                .contains(&"TimeoutException".to_string())
        );
    }

    #[test]
    fn test_detect_catch_context_partial_typed() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new RuntimeException('error');\n",
            "    throw new InvalidArgumentException('bad');\n",
            "} catch (Run",
        );
        let pos = Position {
            line: 4,
            character: 13,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.partial, "Run");
        // Both are suggested (filtering happens in build_catch_completions)
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string())
        );
        assert!(
            ctx.suggested_types
                .contains(&"InvalidArgumentException".to_string())
        );
    }

    #[test]
    fn test_detect_catch_context_not_catch() {
        let content = concat!("<?php\n", "function foo(",);
        let pos = Position {
            line: 1,
            character: 14,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_none(), "Should not detect catch context in function");
    }

    #[test]
    fn test_build_catch_completions_filters_by_partial() {
        let ctx = CatchContext {
            partial: "Run".to_string(),
            suggested_types: vec![
                "RuntimeException".to_string(),
                "InvalidArgumentException".to_string(),
            ],
            already_listed: vec![],
            has_specific_types: true,
        };
        let items = build_catch_completions(&ctx);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "RuntimeException");
    }

    #[test]
    fn test_build_catch_completions_empty_partial_shows_all() {
        let ctx = CatchContext {
            partial: String::new(),
            suggested_types: vec![
                "RuntimeException".to_string(),
                "InvalidArgumentException".to_string(),
            ],
            already_listed: vec![],
            has_specific_types: true,
        };
        let items = build_catch_completions(&ctx);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_detect_catch_context_multi_catch_pipe() {
        let content = concat!(
            "<?php\n",
            "try {\n",
            "    throw new IOException('io');\n",
            "    throw new TimeoutException('timeout');\n",
            "    throw new RuntimeException('rt');\n",
            "} catch (IOException | ",
        );
        let pos = Position {
            line: 5,
            character: 23,
        };
        let ctx = detect_catch_context(content, pos);
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert_eq!(ctx.already_listed, vec!["IOException"]);
        // IOException should be filtered out since it's already listed
        assert!(
            !ctx.suggested_types.contains(&"IOException".to_string()),
            "IOException should be filtered out"
        );
        assert!(
            ctx.suggested_types
                .contains(&"TimeoutException".to_string())
        );
        assert!(
            ctx.suggested_types
                .contains(&"RuntimeException".to_string())
        );
    }
}
