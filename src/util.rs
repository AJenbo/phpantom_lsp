/// Utility functions for the PHPantom server.
///
/// This module contains helper methods for position/offset conversion,
/// class lookup by offset, logging, and shared text-processing helpers
/// used by multiple modules.
///
/// Cross-file class/function resolution and name-resolution logic live
/// in the dedicated [`crate::resolution`] module.
///
/// Subject-extraction helpers (walking backwards through characters to
/// find variables, call expressions, balanced parentheses, `new`
/// expressions, etc.) live in [`crate::subject_extraction`].
use tower_lsp::lsp_types::*;

/// Convert an LSP `Position` (line, character) to a byte offset in
/// `content`.
///
/// Characters are treated as single-byte (sufficient for the vast
/// majority of PHP source).  If the position is past the end of the
/// file, the content length is returned.
pub(crate) fn position_to_byte_offset(content: &str, position: Position) -> usize {
    let mut offset = 0usize;
    for (line_idx, line) in content.lines().enumerate() {
        if line_idx == position.line as usize {
            let char_offset = position.character as usize;
            // Convert character offset (UTF-16 code units in LSP) to byte offset.
            // For simplicity, treat characters as single-byte (ASCII).
            // This is sufficient for most PHP code.
            let byte_col = line
                .char_indices()
                .nth(char_offset)
                .map(|(idx, _)| idx)
                .unwrap_or(line.len());
            return offset + byte_col;
        }
        offset += line.len() + 1; // +1 for newline
    }
    // If the position is past the last line, return end of content
    content.len()
}

/// Extract the short (unqualified) class name from a potentially
/// fully-qualified name.
///
/// For example, `"Illuminate\\Support\\Collection"` → `"Collection"`,
/// and `"Collection"` → `"Collection"`.
pub(crate) fn short_name(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
}

/// Find the first `;` in `s` that is not nested inside `()`, `[]`,
/// `{}`, or string literals.
///
/// Returns the byte offset of the semicolon, or `None` if no
/// top-level semicolon exists.  Used by multiple completion modules
/// to delimit the right-hand side of assignment statements.
pub(crate) fn find_semicolon_balanced(s: &str) -> Option<usize> {
    let mut depth_paren = 0i32;
    let mut depth_bracket = 0i32;
    let mut depth_brace = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';

    for (i, ch) in s.char_indices() {
        if in_single_quote {
            if ch == '\'' && prev_char != '\\' {
                in_single_quote = false;
            }
            prev_char = ch;
            continue;
        }
        if in_double_quote {
            if ch == '"' && prev_char != '\\' {
                in_double_quote = false;
            }
            prev_char = ch;
            continue;
        }
        match ch {
            '\'' => in_single_quote = true,
            '"' => in_double_quote = true,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '[' => depth_bracket += 1,
            ']' => depth_bracket -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ';' if depth_paren == 0 && depth_bracket == 0 && depth_brace == 0 => {
                return Some(i);
            }
            _ => {}
        }
        prev_char = ch;
    }
    None
}

/// Known array functions whose output preserves the input array's
/// element type (the first positional argument).
pub(crate) const ARRAY_PRESERVING_FUNCS: &[&str] = &[
    "array_filter",
    "array_values",
    "array_unique",
    "array_reverse",
    "array_slice",
    "array_splice",
    "array_chunk",
    "array_diff",
    "array_intersect",
    "array_merge",
];

/// Known array functions that extract a single element from the input
/// array (the element type is the output type, not wrapped in an array).
pub(crate) const ARRAY_ELEMENT_FUNCS: &[&str] = &[
    "array_pop",
    "array_shift",
    "current",
    "end",
    "reset",
    "next",
    "prev",
];

/// Find the position of the closing delimiter that matches the opening
/// delimiter at `open_pos`, scanning forward.
///
/// `open` and `close` are the opening and closing byte values (e.g.
/// `b'{'` / `b'}'` or `b'('` / `b')'`).  The scan is aware of string
/// literals (`'…'` and `"…"` with backslash escaping) and both styles
/// of PHP comment (`// …` and `/* … */`), so delimiters inside strings
/// or comments are not counted.
pub(crate) fn find_matching_forward(
    text: &str,
    open_pos: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if open_pos >= len || bytes[open_pos] != open {
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
                b if b == open => depth += 1,
                b if b == close => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(pos);
                    }
                }
                b'/' if pos + 1 < len => {
                    if bytes[pos + 1] == b'/' {
                        // Line comment — skip to end of line
                        while pos < len && bytes[pos] != b'\n' {
                            pos += 1;
                        }
                        continue;
                    }
                    if bytes[pos + 1] == b'*' {
                        // Block comment — skip to `*/`
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

/// Find the position of the opening delimiter that matches the closing
/// delimiter at `close_pos`, scanning backward.
///
/// `open` and `close` are the opening and closing byte values (e.g.
/// `b'{'` / `b'}'` or `b'('` / `b')'`).  The scan skips over string
/// literals (`'…'` and `"…"`) by counting preceding backslashes to
/// distinguish escaped from unescaped quotes.
pub(crate) fn find_matching_backward(
    text: &str,
    close_pos: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    let bytes = text.as_bytes();
    if close_pos >= bytes.len() || bytes[close_pos] != close {
        return None;
    }

    let mut depth = 1i32;
    let mut pos = close_pos;

    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b if b == close => depth += 1,
            b if b == open => {
                depth -= 1;
                if depth == 0 {
                    return Some(pos);
                }
            }
            // Skip string literals by walking backward to the opening quote.
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

use crate::Backend;
use crate::types::{ClassInfo, FileContext};

impl Backend {
    /// Convert an LSP Position (line, character) to a byte offset in content.
    ///
    /// Thin wrapper around [`position_to_byte_offset`] that returns `u32`
    /// (matching the offset type used by `ClassInfo::start_offset` /
    /// `end_offset` and `ResolutionCtx::cursor_offset`).
    pub(crate) fn position_to_offset(content: &str, position: Position) -> u32 {
        position_to_byte_offset(content, position) as u32
    }

    /// Find which class the cursor (byte offset) is inside.
    ///
    /// When multiple classes contain the offset (e.g. an anonymous class
    /// nested inside a named class's method), the smallest (most specific)
    /// class is returned.  This ensures that `$this` inside an anonymous
    /// class body resolves to the anonymous class, not the outer class.
    pub(crate) fn find_class_at_offset(classes: &[ClassInfo], offset: u32) -> Option<&ClassInfo> {
        classes
            .iter()
            .filter(|c| offset >= c.start_offset && offset <= c.end_offset)
            .min_by_key(|c| c.end_offset - c.start_offset)
    }

    /// Look up a class by its (possibly namespace-qualified) name in the
    /// in-memory `ast_map`, without triggering any disk I/O.
    ///
    /// The `class_name` can be:
    ///   - A simple name like `"Customer"`
    ///   - A namespace-qualified name like `"Klarna\\Customer"`
    ///   - A fully-qualified name like `"\\Klarna\\Customer"` (leading `\` is stripped)
    ///
    /// When a namespace prefix is present, the file's namespace (from
    /// `namespace_map`) must match for the class to be returned.  This
    /// prevents `"Demo\\PDO"` from matching the global `PDO` stub.
    ///
    /// Returns a cloned `ClassInfo` if found, or `None`.
    pub(crate) fn find_class_in_ast_map(&self, class_name: &str) -> Option<ClassInfo> {
        let normalized = class_name.strip_prefix('\\').unwrap_or(class_name);
        let last_segment = short_name(normalized);
        let expected_ns: Option<&str> = if normalized.contains('\\') {
            Some(&normalized[..normalized.len() - last_segment.len() - 1])
        } else {
            None
        };

        let map = self.ast_map.lock().ok()?;
        let nmap = self.namespace_map.lock().ok();
        for (uri, classes) in map.iter() {
            if let Some(cls) = classes.iter().find(|c| c.name == last_segment) {
                if let Some(exp_ns) = expected_ns {
                    let file_ns = nmap
                        .as_ref()
                        .and_then(|nm| nm.get(uri))
                        .and_then(|opt| opt.as_deref());
                    if file_ns != Some(exp_ns) {
                        continue;
                    }
                }
                return Some(cls.clone());
            }
        }
        None
    }

    /// Get the content of a file by URI, trying open files first then disk.
    ///
    /// This replaces the repeated pattern of locking `open_files`, looking
    /// up the URI, and falling back to reading from disk via
    /// `Url::to_file_path` + `std::fs::read_to_string`.  Three call sites
    /// in the definition modules used this exact sequence.
    pub(crate) fn get_file_content(&self, uri: &str) -> Option<String> {
        if let Some(content) = self
            .open_files
            .lock()
            .ok()
            .and_then(|files| files.get(uri).cloned())
        {
            return Some(content);
        }
        let path = Url::parse(uri).ok()?.to_file_path().ok()?;
        std::fs::read_to_string(path).ok()
    }

    /// Public helper for tests: get the ast_map for a given URI.
    pub fn get_classes_for_uri(&self, uri: &str) -> Option<Vec<ClassInfo>> {
        if let Ok(map) = self.ast_map.lock() {
            map.get(uri).cloned()
        } else {
            None
        }
    }

    /// Gather the per-file context (classes, use-map, namespace) in one call.
    ///
    /// This replaces the repeated lock-and-unwrap boilerplate that was
    /// duplicated across the completion handler, definition resolver,
    /// member definition, implementation resolver, and variable definition
    /// modules.  Each of those sites used to have three nearly-identical
    /// blocks acquiring `ast_map`, `use_map`, and `namespace_map` locks
    /// and extracting the entry for a given URI.
    pub(crate) fn file_context(&self, uri: &str) -> FileContext {
        let classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let use_map = self
            .use_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        let namespace = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        FileContext {
            classes,
            use_map,
            namespace,
        }
    }

    /// Remove a file's entries from `ast_map`, `use_map`, and `namespace_map`.
    ///
    /// This is the mirror of [`file_context`](Self::file_context): where that
    /// method *reads* the three maps, this method *clears* them for a given URI.
    /// Called from `did_close` to clean up state when a file is closed.
    pub(crate) fn clear_file_maps(&self, uri: &str) {
        if let Ok(mut map) = self.ast_map.lock() {
            map.remove(uri);
        }
        if let Ok(mut map) = self.use_map.lock() {
            map.remove(uri);
        }
        if let Ok(mut map) = self.namespace_map.lock() {
            map.remove(uri);
        }
    }

    pub(crate) async fn log(&self, typ: MessageType, message: String) {
        if let Some(client) = &self.client {
            client.log_message(typ, message).await;
        }
    }
}
