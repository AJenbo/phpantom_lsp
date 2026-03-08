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
//! - **Unknown member diagnostics** — report `MemberAccess` spans where
//!   the member does not exist on the resolved class after full
//!   resolution (inheritance + virtual member providers).  Suppressed
//!   when the class has `__call` / `__callStatic` / `__get` magic methods.
//! - **Unresolved member access diagnostics** (opt-in) — report
//!   `MemberAccess` spans where the **subject type** cannot be resolved
//!   at all.  Off by default; enable via `[diagnostics]
//!   unresolved-member-access = true` in `.phpantom.toml`.  Uses
//!   `Severity::HINT` to surface type-coverage gaps without drowning
//!   the editor in warnings.
//!
//! Diagnostics are published **asynchronously** via [`Backend::schedule_diagnostics`].
//! On every `did_change` event a version counter is bumped and the
//! diagnostic worker is notified.  The worker debounces rapid edits
//! (waits [`DIAGNOSTIC_DEBOUNCE_MS`] after the last notification) and
//! then runs a single diagnostic pass.  At most one pass runs at a time;
//! if new edits arrive while a pass is in flight, a single follow-up
//! pass is scheduled once the current one finishes.  This two-slot
//! design (one running, one pending) ensures diagnostics never block
//! completion, hover, or other latency-sensitive requests.

mod deprecated;
pub(crate) mod unknown_classes;
pub(crate) mod unknown_members;
pub(crate) mod unresolved_member_access;
mod unused_imports;

use std::sync::atomic::Ordering;

use tower_lsp::lsp_types::*;

use crate::Backend;

/// How long to wait after the last keystroke before publishing diagnostics.
const DIAGNOSTIC_DEBOUNCE_MS: u64 = 500;

impl Backend {
    /// Collect all diagnostics for a single file and publish them.
    ///
    /// Called from the diagnostic worker task spawned by
    /// [`schedule_diagnostics`](Self::schedule_diagnostics).  Both
    /// `did_open` and `did_change` schedule diagnostics asynchronously
    /// so that lazy stub parsing (which can trigger hundreds of
    /// cache-miss parses on first open) never blocks the LSP response.
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
        {
            let prefix = self.vendor_uri_prefix.lock();
            if !prefix.is_empty() && uri_str.starts_with(prefix.as_str()) {
                return;
            }
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

        // ── Unknown member access ───────────────────────────────────────
        self.collect_unknown_member_diagnostics(uri_str, content, &mut diagnostics);

        // ── Unresolved member access (opt-in) ───────────────────────────
        self.collect_unresolved_member_access_diagnostics(uri_str, content, &mut diagnostics);

        client.publish_diagnostics(uri, diagnostics, None).await;
    }

    /// Notify the diagnostic worker that new work is available.
    ///
    /// Bumps the diagnostic version counter and wakes the worker.
    /// The worker will debounce rapid calls (waiting
    /// [`DIAGNOSTIC_DEBOUNCE_MS`] after the *last* notification) and
    /// then run a single diagnostic pass.
    ///
    /// This returns immediately — all diagnostic computation happens
    /// in the background so that completion, hover, and signature help
    /// are never blocked.
    pub(crate) fn schedule_diagnostics(&self, uri: String) {
        // Store the URI that needs diagnostics.
        *self.diag_pending_uri.lock() = Some(uri);
        // Bump version so the worker knows there is fresh work.
        self.diag_version.fetch_add(1, Ordering::Release);
        // Wake the worker (no-op if it is already awake).
        self.diag_notify.notify_one();
    }

    /// Long-lived background task that processes diagnostic requests.
    ///
    /// Spawned once during `initialized`.  Loops forever, waiting for
    /// [`schedule_diagnostics`](Self::schedule_diagnostics) to signal
    /// new work.  On each iteration:
    ///
    /// 1. Wait for a notification (new edit arrived).
    /// 2. Debounce: sleep [`DIAGNOSTIC_DEBOUNCE_MS`], then check
    ///    whether the version counter moved (more edits).  If so,
    ///    loop back to step 2.
    /// 3. Snapshot the pending URI and current file content.
    /// 4. Run the diagnostic collectors and publish results.
    /// 5. Loop back to step 1.
    ///
    /// Because there is exactly one instance of this task, at most one
    /// diagnostic pass runs at a time.  If edits arrive during step 4
    /// the version counter will have moved, and step 1 picks up
    /// immediately after step 4 finishes — giving the two-slot
    /// (one running + one pending) behaviour.
    pub(crate) async fn diagnostic_worker(&self) {
        loop {
            // ── Step 1: wait for work ───────────────────────────────
            self.diag_notify.notified().await;

            // ── Step 2: debounce ────────────────────────────────────
            loop {
                let version_before = self.diag_version.load(Ordering::Acquire);
                tokio::time::sleep(std::time::Duration::from_millis(DIAGNOSTIC_DEBOUNCE_MS)).await;
                let version_after = self.diag_version.load(Ordering::Acquire);
                if version_before == version_after {
                    // No new edits during the sleep — proceed.
                    break;
                }
                // More edits arrived — loop and debounce again.
            }

            // ── Step 3: snapshot ────────────────────────────────────
            let uri = match self.diag_pending_uri.lock().take() {
                Some(u) => u,
                None => continue,
            };
            let content = {
                let files = self.open_files.read();
                match files.get(&uri) {
                    Some(c) => c.clone(),
                    None => continue,
                }
            };

            // ── Step 4: collect and publish ─────────────────────────
            self.publish_diagnostics_for_file(&uri, &content).await;
        }
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

/// Build a diagnostic range from byte offsets, returning `None` if the
/// conversion fails (e.g. invalid offset or multi-byte boundary).
pub(crate) fn offset_range_to_lsp_range(
    content: &str,
    start_byte: usize,
    end_byte: usize,
) -> Option<Range> {
    let start_pos = byte_offset_to_position(content, start_byte)?;
    let end_pos = byte_offset_to_position(content, end_byte)?;
    Some(Range {
        start: start_pos,
        end: end_pos,
    })
}

/// Convert a byte offset to an LSP `Position` (0-based line and character).
fn byte_offset_to_position(content: &str, byte_offset: usize) -> Option<Position> {
    if byte_offset > content.len() {
        return None;
    }
    let before = &content[..byte_offset];
    let line = before.matches('\n').count() as u32;
    let last_newline = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let character = before[last_newline..].encode_utf16().count() as u32;
    Some(Position { line, character })
}
