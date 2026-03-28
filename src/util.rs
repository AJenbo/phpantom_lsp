/// Utility functions for the PHPantom server.
///
/// This module contains helper methods for position/offset conversion,
/// class lookup by offset, logging, panic catching, and shared
/// text-processing helpers used by multiple modules.
///
/// Cross-file class/function resolution and name-resolution logic live
/// in the dedicated [`crate::resolution`] module.
///
/// Subject-extraction helpers (walking backwards through characters to
/// find variables, call expressions, balanced parentheses, `new`
/// expressions, etc.) live in [`crate::subject_extraction`].
use std::panic::{self, AssertUnwindSafe, UnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tower_lsp::lsp_types::*;

/// Run `f` inside [`panic::catch_unwind`], logging and swallowing any
/// panic.
///
/// Returns `Some(value)` on success and `None` on panic.  The error
/// message includes `label` (the operation name, e.g. `"hover"` or
/// `"goto_definition"`), `uri`, and the optional cursor `position`.
///
/// This centralises the boilerplate that every LSP handler uses to
/// guard against stack overflows and unexpected panics in the
/// resolution pipeline.
///
/// # Examples
///
/// ```ignore
/// let result = catch_panic("hover", uri, Some(position), || {
///     self.handle_hover(uri, content, position)
/// });
/// ```
pub(crate) fn catch_panic<T>(
    label: &str,
    uri: &str,
    position: Option<Position>,
    f: impl FnOnce() -> T + UnwindSafe,
) -> Option<T> {
    match panic::catch_unwind(f) {
        Ok(value) => Some(value),
        Err(_) => {
            if let Some(pos) = position {
                tracing::error!(
                    "PHPantom: panic during {} at {}:{}:{}",
                    label,
                    uri,
                    pos.line,
                    pos.character
                );
            } else {
                tracing::error!("PHPantom: panic during {} at {}", label, uri);
            }
            None
        }
    }
}

/// Convenience wrapper around [`catch_panic`] for closures that
/// capture `&self` or other non-[`UnwindSafe`] references.
///
/// Wraps `f` in [`AssertUnwindSafe`] before forwarding to
/// [`catch_panic`].  This is safe in our context because a panic
/// during LSP handling never leaves shared state in an inconsistent
/// state (the worst case is a stale cache entry).
pub(crate) fn catch_panic_unwind_safe<T>(
    label: &str,
    uri: &str,
    position: Option<Position>,
    f: impl FnOnce() -> T,
) -> Option<T> {
    catch_panic(label, uri, position, AssertUnwindSafe(f))
}

/// Convert a filesystem path to a properly percent-encoded `file://` URI string.
///
/// This **must** be used instead of `format!("file://{}", path.display())`
/// everywhere in the codebase.  The `format!` approach produces raw,
/// un-encoded paths (e.g. `file:///home/user/My Project/Foo.php`) while
/// LSP clients send URIs through the `Url` type which percent-encodes
/// special characters (e.g. `file:///home/user/My%20Project/Foo.php`).
/// When both forms end up as keys in `symbol_maps`, the same file is
/// indexed twice and every Find References result is duplicated.
///
/// Falls back to the raw `format!` form only when `Url::from_file_path`
/// fails (non-absolute paths on some platforms), which should never
/// happen in practice.
pub(crate) fn path_to_uri(path: &Path) -> String {
    Url::from_file_path(path)
        .map(|u| u.to_string())
        .unwrap_or_else(|()| format!("file://{}", path.display()))
}

/// Recursively collect all `.php` files under a directory, respecting
/// `.gitignore` rules and skipping hidden directories (`.git`,
/// `.idea`, etc.).
///
/// Uses the `ignore` crate's `WalkBuilder` for gitignore-aware
/// traversal.  This is consistent with the other workspace walkers
/// (`scan_workspace_fallback_full`, `collect_php_files_gitignore`).
///
/// Used by Go-to-implementation (Phase 5) which walks PSR-4 source
/// directories.
///
/// `vendor_dir_paths` contains absolute paths of all known vendor
/// directories (one per subproject in monorepo mode).  Any directory
/// whose absolute path matches one of these is skipped regardless of
/// `.gitignore` content.
///
/// Silently skips directories and files that cannot be read (e.g.
/// permission errors, broken symlinks).
pub(crate) fn collect_php_files(dir: &Path, vendor_dir_paths: &[PathBuf]) -> Vec<PathBuf> {
    use ignore::WalkBuilder;

    let mut result = Vec::new();
    let vendor_paths: Vec<PathBuf> = vendor_dir_paths.to_vec();

    let walker = WalkBuilder::new(dir)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .hidden(true)
        .parents(true)
        .ignore(true)
        .filter_entry(move |entry| {
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let path = entry.path();
                if vendor_paths.iter().any(|vp| vp == path) {
                    return false;
                }
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "php") {
            result.push(path.to_path_buf());
        }
    }

    result
}

/// Recursively collect all `.php` files under a workspace root,
/// respecting `.gitignore` rules (including nested and global
/// gitignore files).
///
/// Used by Find References which walks the entire workspace root.
/// Unlike [`collect_php_files`], this uses the `ignore` crate's
/// [`WalkBuilder`] so that generated/cached directories listed in
/// `.gitignore` (e.g. `storage/framework/views/`, `var/cache/`,
/// `node_modules/`) are automatically skipped.
///
/// All known vendor directories are always skipped regardless of
/// `.gitignore` content, since some projects commit their vendor
/// directory.  `vendor_dir_paths` contains absolute paths of all
/// known vendor directories (one per subproject in monorepo mode).
///
/// Hidden files and directories are skipped by default (handled by
/// the `ignore` crate).
pub(crate) fn collect_php_files_gitignore(
    root: &Path,
    vendor_dir_paths: &[PathBuf],
) -> Vec<PathBuf> {
    use ignore::WalkBuilder;

    let mut result = Vec::new();
    let vendor_paths_owned: Vec<PathBuf> = vendor_dir_paths.to_vec();

    let walker = WalkBuilder::new(root)
        // Respect .gitignore, .git/info/exclude, global gitignore
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        // Skip hidden files/dirs (.git, .idea, etc.)
        .hidden(true)
        // Read parent .gitignore files
        .parents(true)
        // Also respect .ignore files (ripgrep convention)
        .ignore(true)
        // Always skip vendor directories, even if not gitignored
        .filter_entry(move |entry| {
            if entry.file_type().is_some_and(|ft| ft.is_dir()) {
                let path = entry.path();
                if vendor_paths_owned.iter().any(|vp| vp == path) {
                    return false;
                }
            }
            true
        })
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "php") {
            result.push(path.to_path_buf());
        }
    }

    result
}

/// Convert a byte offset in `content` to an LSP `Position` (line, character).
///
/// This is the inverse of [`position_to_byte_offset`].  Characters are
/// counted as UTF-16 code units per the LSP specification.
/// If `offset` is past the end of `content`, the position at the end of
/// the file is returned.
pub(crate) fn offset_to_position(content: &str, offset: usize) -> Position {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in content.char_indices() {
        if i == offset {
            return Position {
                line,
                character: col,
            };
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16() as u32;
        }
    }
    // offset == content.len() (end of file)
    Position {
        line,
        character: col,
    }
}

/// Convert an LSP `Position` (line, character) to a byte offset in
/// `content`.
///
/// Characters are counted as UTF-16 code units per the LSP specification.
/// If the position is past the end of the file, the content length is
/// returned.
pub(crate) fn position_to_byte_offset(content: &str, position: Position) -> usize {
    let mut line = 0u32;
    let mut col = 0u32;
    for (i, ch) in content.char_indices() {
        if line == position.line && col == position.character {
            return i;
        }
        if ch == '\n' {
            if line == position.line {
                // Position is past the end of this line — clamp to newline.
                return i;
            }
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16() as u32;
        }
    }
    // Position at end of content.
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

/// Convert an LSP Position (line, character) to a byte offset in content.
///
/// Thin wrapper around [`position_to_byte_offset`] that returns `u32`
/// (matching the offset type used by `ClassInfo::start_offset` /
/// `end_offset` and `ResolutionCtx::cursor_offset`).
pub(crate) fn position_to_offset(content: &str, position: Position) -> u32 {
    position_to_byte_offset(content, position) as u32
}

/// Convert an LSP `Position` (line/character) to a character offset into
/// a pre-built char array.
///
/// Returns `None` when the position is beyond the end of `chars`.
/// Handles UTF-16 column widths, end-of-line clamping, and trailing
/// content without a newline.
pub fn position_to_char_offset(chars: &[char], position: Position) -> Option<usize> {
    let target_line = position.line as usize;
    let target_col = position.character as usize;
    let mut line = 0usize;
    let mut col = 0usize;

    for (i, &ch) in chars.iter().enumerate() {
        if line == target_line && col == target_col {
            return Some(i);
        }
        if ch == '\n' {
            // If we're at the target line and the target column is at or
            // past the end of the line, clamp to end-of-line.
            if line == target_line {
                return Some(i);
            }
            line += 1;
            col = 0;
        } else {
            col += ch.len_utf16();
        }
    }

    // Cursor at very end of content
    if line == target_line && col == target_col {
        return Some(chars.len());
    }
    // Target column past end of last line (no trailing newline)
    if line == target_line {
        return Some(chars.len());
    }

    None
}

/// Find which class the cursor (byte offset) is inside.
///
/// When multiple classes contain the offset (e.g. an anonymous class
/// nested inside a named class's method), the smallest (most specific)
/// class is returned.  This ensures that `$this` inside an anonymous
/// class body resolves to the anonymous class, not the outer class.
pub(crate) fn find_class_at_offset(classes: &[Arc<ClassInfo>], offset: u32) -> Option<&ClassInfo> {
    classes
        .iter()
        .map(|c| c.as_ref())
        .filter(|c| offset >= c.start_offset && offset <= c.end_offset)
        .min_by_key(|c| c.end_offset - c.start_offset)
}

/// Find a class in a slice by name, preferring namespace-aware matching
/// when the name is fully qualified.
///
/// When `name` contains backslashes (e.g. `Illuminate\Database\Eloquent\Builder`),
/// the lookup checks each candidate's `file_namespace` field so that the
/// correct class is returned even when multiple classes share the same short
/// name but live in different namespace blocks within the same file (e.g.
/// `Demo\Builder` vs `Illuminate\Database\Eloquent\Builder`).
///
/// When `name` is a bare short name (no backslashes), the first class with
/// a matching `name` field is returned (preserving existing behavior).
pub(crate) fn find_class_by_name<'a>(
    all_classes: &'a [Arc<ClassInfo>],
    name: &str,
) -> Option<&'a Arc<ClassInfo>> {
    let short = short_name(name);

    if name.contains('\\') {
        let expected_ns = name.rsplit_once('\\').map(|(ns, _)| ns);
        all_classes
            .iter()
            .find(|c| c.name == short && c.file_namespace.as_deref() == expected_ns)
    } else {
        all_classes.iter().find(|c| c.name == short)
    }
}

/// Collapse multi-line method chains around the cursor into a single line.
///
/// When the cursor line (after trimming leading whitespace) begins with
/// `->` or `?->`, this function walks backwards through preceding lines
/// that are also continuations, plus the base expression line, and joins
/// them into one flattened string.  The returned column is the cursor's
/// position within that flattened string.
///
/// If the cursor line is not a continuation, the original line and column
/// are returned unchanged.
///
/// # Returns
///
/// `(collapsed_line, adjusted_column)` — the flattened text and the
/// cursor's character offset within it.
pub(crate) fn collapse_continuation_lines(
    lines: &[&str],
    cursor_line: usize,
    cursor_col: usize,
) -> (String, usize) {
    let line = lines[cursor_line];
    let trimmed = line.trim_start();

    // Only collapse when the cursor line is a continuation (starts with
    // `->` or `?->` after optional whitespace).
    if !trimmed.starts_with("->") && !trimmed.starts_with("?->") {
        return (line.to_string(), cursor_col);
    }

    let cursor_leading_ws = line.len() - trimmed.len();

    // Walk backwards to find the first non-continuation line (the base).
    //
    // A continuation line is one that starts with `->` or `?->`.  However,
    // multi-line closure/function arguments can break the chain:
    //
    //   Brand::whereNested(function (Builder $q): void {
    //   })
    //   ->   // ← cursor
    //
    // Here line `})` is NOT a continuation but is part of the call
    // expression on the base line.  We detect this by tracking
    // brace/paren balance: when the accumulated lines (from the current
    // candidate upwards to the cursor) have unmatched closing delimiters,
    // we keep walking backwards until the delimiters balance out.
    let mut start = cursor_line;
    while start > 0 {
        let prev_trimmed = lines[start - 1].trim_start();

        // Skip blank (whitespace-only) lines — they don't terminate a
        // chain.  Without this, a blank line between chain segments
        // causes the backward walk to stop prematurely.
        if prev_trimmed.is_empty() {
            start -= 1;
            continue;
        }

        if prev_trimmed.starts_with("->") || prev_trimmed.starts_with("?->") {
            start -= 1;
        } else {
            // Check whether the accumulated text from this candidate
            // line through the line just before the cursor has
            // unbalanced closing delimiters.  If so, this line is in
            // the middle of a multi-line argument list and we must
            // keep walking backwards.
            start -= 1;

            // Count paren/brace balance from `start` up to (but not
            // including) the cursor line.
            let mut paren_depth: i32 = 0;
            let mut brace_depth: i32 = 0;
            for line in lines.iter().take(cursor_line).skip(start) {
                for ch in line.chars() {
                    match ch {
                        '(' => paren_depth += 1,
                        ')' => paren_depth -= 1,
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
            }

            // If balanced (or net-open), this is a proper base line.
            if paren_depth >= 0 && brace_depth >= 0 {
                break;
            }

            // Unbalanced — keep walking backwards until we close the
            // gap.  Each step re-checks the running balance.
            while start > 0 && (paren_depth < 0 || brace_depth < 0) {
                start -= 1;
                for ch in lines[start].chars() {
                    match ch {
                        '(' => paren_depth += 1,
                        ')' => paren_depth -= 1,
                        '{' => brace_depth += 1,
                        '}' => brace_depth -= 1,
                        _ => {}
                    }
                }
            }

            // After re-balancing we may have landed on a continuation
            // line (e.g. `->where(...\n...\n)->`) — keep walking if so.
            if start > 0 {
                let landed = lines[start].trim_start();
                if landed.starts_with("->") || landed.starts_with("?->") {
                    continue;
                }
            }
            break;
        }
    }

    // Build the collapsed string from the base line through the cursor line,
    // skipping blank lines so they don't leave gaps in the collapsed result.
    let mut prefix = String::new();
    for (i, line) in lines.iter().enumerate().take(cursor_line).skip(start) {
        let piece = if i == start {
            line.trim_end()
        } else {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            t
        };
        prefix.push_str(piece);
    }

    // The cursor position in the collapsed string is the length of the
    // prefix (everything before the cursor line) plus the cursor's offset
    // within the trimmed cursor line.
    let new_col = prefix.chars().count() + (cursor_col.saturating_sub(cursor_leading_ws));

    prefix.push_str(trimmed);

    (prefix, new_col)
}

impl Backend {
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
    /// Returns a shared `Arc<ClassInfo>` if found, or `None`.
    pub(crate) fn find_class_in_ast_map(&self, class_name: &str) -> Option<Arc<ClassInfo>> {
        // ── Fast path: O(1) lookup via fqn_index ──
        // For namespace-qualified names the FQN is the normalized name
        // itself.  For bare names (no backslash) the FQN equals the
        // short name, which is also stored in the index.
        if let Some(cls) = self.fqn_index.read().get(class_name) {
            return Some(Arc::clone(cls));
        }

        // ── Slow fallback: linear scan of ast_map ──
        // Covers edge cases where the fqn_index has not been populated
        // yet (e.g. anonymous classes, or race conditions during initial
        // indexing).
        let last_segment = short_name(class_name);
        let expected_ns: Option<&str> = if class_name.contains('\\') {
            Some(&class_name[..class_name.len() - last_segment.len() - 1])
        } else {
            None
        };

        let map = self.ast_map.read();

        for (_uri, classes) in map.iter() {
            // Iterate ALL classes with the matching short name, not just
            // the first.  A multi-namespace file can contain two classes
            // with the same short name in different namespace blocks
            // (e.g. `Illuminate\Database\Eloquent\Builder` and
            // `Illuminate\Database\Query\Builder`).
            for cls in classes.iter().filter(|c| c.name == last_segment) {
                if let Some(exp_ns) = expected_ns {
                    // Use the per-class namespace (set during parsing)
                    // rather than the file-level namespace.  This
                    // correctly handles files with multiple namespace
                    // blocks where different classes live under different
                    // namespaces.
                    let class_ns = cls.file_namespace.as_deref();
                    if class_ns != Some(exp_ns) {
                        continue;
                    }
                }
                return Some(Arc::clone(cls));
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
        if let Some(content) = self.open_files.read().get(uri) {
            return Some(String::clone(content));
        }

        // Embedded class stubs live under synthetic `phpantom-stub://`
        // URIs and have no on-disk file.  Retrieve the raw source from
        // the stub_index keyed by the class short name (the URI path).
        if let Some(class_name) = uri.strip_prefix("phpantom-stub://") {
            let stub_idx = self.stub_index.read();
            return stub_idx.get(class_name).map(|s| s.to_string());
        }

        // Embedded function stubs use `phpantom-stub-fn://` URIs.
        // The path component is the function name used as key in
        // stub_function_index.
        if let Some(func_name) = uri.strip_prefix("phpantom-stub-fn://") {
            let stub_fn_idx = self.stub_function_index.read();
            return stub_fn_idx.get(func_name).map(|s| s.to_string());
        }

        let path = Url::parse(uri).ok()?.to_file_path().ok()?;
        std::fs::read_to_string(path).ok()
    }

    /// Retrieve file content as a cheap `Arc<String>` reference when the
    /// file is in `open_files`.  Falls back to reading from disk (which
    /// wraps the result in a new `Arc`).
    ///
    /// Prefer this over [`get_file_content`] in hot paths where the
    /// content will be shared across tasks or stored for the duration
    /// of a request, since it avoids deep-cloning the file string.
    pub(crate) fn get_file_content_arc(&self, uri: &str) -> Option<Arc<String>> {
        if let Some(content) = self.open_files.read().get(uri) {
            return Some(Arc::clone(content));
        }

        // Embedded class stubs live under synthetic `phpantom-stub://`
        // URIs and have no on-disk file.
        if let Some(class_name) = uri.strip_prefix("phpantom-stub://") {
            let stub_idx = self.stub_index.read();
            return stub_idx.get(class_name).map(|s| Arc::new(s.to_string()));
        }

        // Embedded function stubs use `phpantom-stub-fn://` URIs.
        if let Some(func_name) = uri.strip_prefix("phpantom-stub-fn://") {
            let stub_fn_idx = self.stub_function_index.read();
            return stub_fn_idx.get(func_name).map(|s| Arc::new(s.to_string()));
        }

        let path = Url::parse(uri).ok()?.to_file_path().ok()?;
        std::fs::read_to_string(path).ok().map(Arc::new)
    }

    /// Public helper for tests: get the ast_map for a given URI.
    pub fn get_classes_for_uri(&self, uri: &str) -> Option<Vec<ClassInfo>> {
        self.ast_map
            .read()
            .get(uri)
            .map(|classes| classes.iter().map(|c| ClassInfo::clone(c)).collect())
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
        let classes = self.ast_map.read().get(uri).cloned().unwrap_or_default();

        // The legacy use_map (short name → FQN from `use` statements)
        // remains the canonical import table.  `resolved_names` is a
        // supplementary data source for consumers that can query by
        // byte offset — it must NOT replace the use_map because
        // `to_use_map()` only contains names that are actually
        // *referenced* in the code, not all *declared* imports.
        // The unused-imports diagnostic relies on seeing declared-but-
        // unreferenced imports.
        let use_map = self.use_map.read().get(uri).cloned().unwrap_or_default();

        let namespace = self.namespace_map.read().get(uri).cloned().flatten();

        let resolved_names = self.resolved_names.read().get(uri).cloned();

        FileContext {
            classes,
            use_map,
            namespace,
            resolved_names,
        }
    }

    /// Return the import table (short name → FQN) for a file.
    ///
    /// Returns the legacy `use_map` which contains all *declared*
    /// imports from `use` statements, regardless of whether they are
    /// actually referenced in the code.  This is the correct source
    /// for consumers that need the full import table (unused-import
    /// detection, import-class code actions, name resolution helpers).
    ///
    /// For consumers that can resolve names by byte offset, prefer
    /// querying `resolved_names` directly via [`file_context`] instead.
    pub(crate) fn file_use_map(&self, uri: &str) -> std::collections::HashMap<String, String> {
        self.use_map.read().get(uri).cloned().unwrap_or_default()
    }

    /// Remove a file's entries from `ast_map`, `use_map`, and `namespace_map`.
    ///
    /// This is the mirror of [`file_context`](Self::file_context): where that
    /// method *reads* the three maps, this method *clears* them for a given URI.
    /// Called from `did_close` to clean up state when a file is closed.
    pub(crate) fn clear_file_maps(&self, uri: &str) {
        // Collect FQNs for targeted class_index removal BEFORE clearing
        // ast_map — O(file_classes) instead of O(total_class_index).
        let old_fqns: Vec<String> = self
            .ast_map
            .read()
            .get(uri)
            .map(|classes| {
                classes
                    .iter()
                    .filter(|c| !c.name.starts_with("__anonymous@"))
                    .map(|c| match &c.file_namespace {
                        Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, c.name),
                        _ => c.name.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default();

        self.ast_map.write().remove(uri);
        self.symbol_maps.write().remove(uri);
        self.use_map.write().remove(uri);
        self.resolved_names.write().remove(uri);
        self.namespace_map.write().remove(uri);
        // Remove class_index entries that belonged to this file so
        // stale FQNs don't linger after the file is closed.
        if !old_fqns.is_empty() {
            let mut idx = self.class_index.write();
            for fqn in &old_fqns {
                idx.remove(fqn);
            }
        }
    }

    pub(crate) async fn log(&self, typ: MessageType, message: String) {
        if let Some(client) = &self.client {
            client.log_message(typ, message).await;
        }
    }

    // ── Work-done progress helpers ──────────────────────────────────

    /// Create a server-initiated work-done progress token and send the
    /// `window/workDoneProgress/create` request to the client.
    ///
    /// Returns `Some(token)` on success, `None` when there is no client
    /// or the client rejects the request.  The caller should pass the
    /// returned token to [`progress_begin`], [`progress_report`], and
    /// [`progress_end`].
    pub(crate) async fn progress_create(&self, token_name: &str) -> Option<NumberOrString> {
        use tower_lsp::lsp_types::request::WorkDoneProgressCreate;

        // Per the LSP spec, servers must only use
        // window/workDoneProgress/create when the client signals
        // support via the window.workDoneProgress capability.
        if !self
            .supports_work_done_progress
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return None;
        }

        let client = self.client.as_ref()?;
        let token = NumberOrString::String(token_name.to_string());
        let params = WorkDoneProgressCreateParams {
            token: token.clone(),
        };
        client
            .send_request::<WorkDoneProgressCreate>(params)
            .await
            .ok()?;
        Some(token)
    }

    /// Send a `WorkDoneProgressBegin` notification for the given token.
    ///
    /// `title` is the short label shown by the editor (e.g. "Indexing").
    /// `message` is an optional detail line (e.g. "Scanning subprojects").
    pub(crate) async fn progress_begin(
        &self,
        token: &NumberOrString,
        title: &str,
        message: Option<String>,
    ) {
        use tower_lsp::lsp_types::notification::Progress;

        let Some(client) = &self.client else { return };
        client
            .send_notification::<Progress>(ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Begin(
                    WorkDoneProgressBegin {
                        title: title.to_string(),
                        cancellable: Some(false),
                        message,
                        percentage: Some(0),
                    },
                )),
            })
            .await;
    }

    /// Send a `WorkDoneProgressReport` notification with a percentage
    /// and optional message.
    ///
    /// `percentage` should be in the range 0..=100.  `message` replaces
    /// the previous detail line when `Some`.
    pub(crate) async fn progress_report(
        &self,
        token: &NumberOrString,
        percentage: u32,
        message: Option<String>,
    ) {
        use tower_lsp::lsp_types::notification::Progress;

        let Some(client) = &self.client else { return };
        client
            .send_notification::<Progress>(ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::Report(
                    WorkDoneProgressReport {
                        cancellable: Some(false),
                        message,
                        percentage: Some(percentage),
                    },
                )),
            })
            .await;
    }

    /// Send a `WorkDoneProgressEnd` notification.
    ///
    /// After this call the editor removes the progress indicator.
    /// `message` is an optional final status line (e.g. "Indexed 5,678
    /// classes").
    pub(crate) async fn progress_end(&self, token: &NumberOrString, message: Option<String>) {
        use tower_lsp::lsp_types::notification::Progress;

        let Some(client) = &self.client else { return };
        client
            .send_notification::<Progress>(ProgressParams {
                token: token.clone(),
                value: ProgressParamsValue::WorkDone(WorkDoneProgress::End(WorkDoneProgressEnd {
                    message,
                })),
            })
            .await;
    }
}
