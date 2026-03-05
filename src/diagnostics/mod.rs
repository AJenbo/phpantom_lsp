//! Diagnostics — publish LSP diagnostics for PHP files.
//!
//! This module collects diagnostics from multiple providers and publishes
//! them via `textDocument/publishDiagnostics`.  Currently implemented:
//!
//! - **`@deprecated` usage diagnostics** — report references to symbols
//!   marked `@deprecated` with `DiagnosticTag::Deprecated` (renders as
//!   strikethrough in most editors).
//! - **Unused `use` dimming** — dim `use` declarations that are not
//!   referenced anywhere in the file with `DiagnosticTag::Unnecessary`.
//! - **Unknown class diagnostics** — report `ClassReference` spans that
//!   cannot be resolved through any resolution phase (use-map, local
//!   classes, same-namespace, class_index, classmap, PSR-4, stubs).

mod deprecated;
pub(crate) mod unknown_classes;
mod unused_imports;

use tower_lsp::lsp_types::*;

use crate::Backend;

impl Backend {
    /// Collect all diagnostics for a single file and publish them.
    ///
    /// Called from `did_open` and `did_change` after `update_ast` has
    /// refreshed the AST, symbol map, use map, and namespace map.
    ///
    /// `uri_str` is the file URI string (e.g. `"file:///path/to/file.php"`).
    /// `content` is the full text of the file.
    pub(crate) async fn publish_diagnostics_for_file(&self, uri_str: &str, content: &str) {
        let client = match &self.client {
            Some(c) => c,
            None => return,
        };

        // Skip diagnostics for stub files — they are internal.
        if uri_str.starts_with("phpantom-stub://") || uri_str.starts_with("phpantom-stub-fn://") {
            return;
        }

        // Skip diagnostics for vendor files — they are third-party code
        // and should not produce warnings in the user's editor.  The
        // vendor URI prefix is built during `initialized` from the
        // workspace root and `composer.json`'s `config.vendor-dir`.
        if let Ok(prefix) = self.vendor_uri_prefix.lock()
            && !prefix.is_empty()
            && uri_str.starts_with(prefix.as_str())
        {
            return;
        }

        let uri = match uri_str.parse::<Url>() {
            Ok(u) => u,
            Err(_) => return,
        };

        let mut diagnostics = Vec::new();

        // ── @deprecated usage diagnostics ───────────────────────────────
        self.collect_deprecated_diagnostics(uri_str, content, &mut diagnostics);

        // ── Unused `use` dimming ────────────────────────────────────────
        self.collect_unused_import_diagnostics(uri_str, content, &mut diagnostics);

        // ── Unknown class references ────────────────────────────────────
        self.collect_unknown_class_diagnostics(uri_str, content, &mut diagnostics);

        client.publish_diagnostics(uri, diagnostics, None).await;
    }

    /// Clear diagnostics for a file (e.g. on `did_close`).
    pub(crate) async fn clear_diagnostics_for_file(&self, uri_str: &str) {
        let client = match &self.client {
            Some(c) => c,
            None => return,
        };

        let uri = match uri_str.parse::<Url>() {
            Ok(u) => u,
            Err(_) => return,
        };

        client.publish_diagnostics(uri, Vec::new(), None).await;
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Convert a byte offset in `content` to an LSP `Position` (0-based line
/// and character).
///
/// Returns `None` if the offset is beyond the content length.
pub(crate) fn offset_to_position(content: &str, offset: usize) -> Option<Position> {
    if offset > content.len() {
        return None;
    }

    let mut line: u32 = 0;
    let mut col: u32 = 0;
    for (i, ch) in content.char_indices() {
        if i == offset {
            return Some(Position::new(line, col));
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }

    // offset == content.len() — position at the very end
    if offset == content.len() {
        Some(Position::new(line, col))
    } else {
        None
    }
}

/// Convert a byte offset range to an LSP `Range`.
pub(crate) fn offset_range_to_lsp_range(content: &str, start: usize, end: usize) -> Option<Range> {
    let start_pos = offset_to_position(content, start)?;
    let end_pos = offset_to_position(content, end)?;
    Some(Range::new(start_pos, end_pos))
}
