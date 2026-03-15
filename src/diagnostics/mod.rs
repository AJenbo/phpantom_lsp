//! Diagnostics — publish LSP diagnostics for PHP files.
//!
//! This module collects diagnostics from multiple providers and publishes
//! them via `textDocument/publishDiagnostics`.  Providers are grouped
//! into three phases so that cheap results appear immediately and
//! expensive external tools never block native feedback:
//!
//! ## Phase 1 — fast (no type resolution)
//!
//! - **`@deprecated` usage diagnostics** — report references to symbols
//!   marked `@deprecated` with `DiagnosticTag::Deprecated` (renders as
//!   strikethrough in most editors).
//! - **Unused `use` dimming** — dim `use` declarations that are not
//!   referenced anywhere in the file with `DiagnosticTag::Unnecessary`.
//!
//! ## Phase 2 — slow (require type resolution)
//!
//! - **Unknown class diagnostics** — report `ClassReference` spans that
//!   cannot be resolved through any resolution phase (use-map, local
//!   classes, same-namespace, class_index, classmap, PSR-4, stubs).
//! - **Unknown member diagnostics** — report `MemberAccess` spans where
//!   the member does not exist on the resolved class after full
//!   resolution (inheritance + virtual member providers).  Suppressed
//!   when the class has `__call` / `__callStatic` / `__get` magic methods.
//! - **Unknown function diagnostics** — report function calls that
//!   cannot be resolved to any known function definition.
//! - **Unresolved member access diagnostics** (opt-in) — report
//!   `MemberAccess` spans where the **subject type** cannot be resolved
//!   at all.  Off by default; enable via `[diagnostics]
//!   unresolved-member-access = true` in `.phpantom.toml`.  Uses
//!   `Severity::HINT` to surface type-coverage gaps without drowning
//!   the editor in warnings.
//! - **Argument count diagnostics** — report calls where the number of
//!   arguments does not match the function/method signature.
//!
//! ## Phase 3 — heavy (external process, dedicated worker)
//!
//! - **PHPStan proxy diagnostics** — run PHPStan in editor mode
//!   (`--tmp-file` / `--instead-of`) and surface its errors as LSP
//!   diagnostics.  Auto-detected via `vendor/bin/phpstan` or `$PATH`;
//!   configurable in `.phpantom.toml` under `[phpstan]`.
//!
//!   PHPStan runs in a **dedicated worker task**, separate from the
//!   main diagnostic worker, because it is extremely slow and
//!   resource-intensive.  At most one PHPStan process runs at a time.
//!   If edits arrive while PHPStan is running, the pending URI is
//!   updated and the worker picks it up after the current run finishes.
//!   Native diagnostics (phases 1 and 2) are never blocked.
//!
//! ## Publishing strategy
//!
//! Each phase publishes immediately, merging its fresh results with
//! **cached** results from the other phases:
//!
//! - Phase 1 publishes: fresh fast + cached slow + cached PHPStan
//! - Phase 2 publishes: fresh fast + fresh slow + cached PHPStan
//! - PHPStan worker publishes: cached fast+slow + fresh PHPStan
//!
//! This ensures diagnostics from all providers remain visible at all
//! times, without flickering when any single phase updates.
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

mod argument_count;
mod deprecated;
pub(crate) mod helpers;
pub(crate) mod unknown_classes;
pub(crate) mod unknown_functions;
pub(crate) mod unknown_members;
pub(crate) mod unresolved_member_access;
mod unused_imports;

use std::sync::atomic::Ordering;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::phpstan;

/// How long to wait after the last keystroke before publishing diagnostics.
const DIAGNOSTIC_DEBOUNCE_MS: u64 = 500;

/// How long to wait after the last keystroke before running PHPStan.
///
/// Longer than the normal debounce because PHPStan is extremely
/// expensive.  We want the user to be truly idle before spawning it.
const PHPSTAN_DEBOUNCE_MS: u64 = 2_000;

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
        // vendor URI prefixes are built during `initialized` from the
        // workspace root and each subproject's `config.vendor-dir`.
        {
            let prefixes = self.vendor_uri_prefixes.lock();
            if prefixes.iter().any(|p| uri_str.starts_with(p.as_str())) {
                return;
            }
        }

        let uri = match uri_str.parse::<Url>() {
            Ok(u) => u,
            Err(_) => return,
        };

        // ── Phase 1: fast diagnostics (cheap, no type resolution) ───────
        // Publish these immediately together with the *previous* slow
        // and PHPStan diagnostics so the editor dims unused imports and
        // strikes through deprecated symbols without waiting for the
        // heavier collectors, and without flickering away existing
        // warnings.
        let mut fast_diagnostics = Vec::new();

        // ── @deprecated usage diagnostics ───────────────────────────────
        self.collect_deprecated_diagnostics(uri_str, content, &mut fast_diagnostics);

        // ── Unused `use` dimming ────────────────────────────────────────
        self.collect_unused_import_diagnostics(uri_str, content, &mut fast_diagnostics);

        // Merge fresh fast diagnostics with stale slow + PHPStan
        // diagnostics so the editor never shows a gap where those
        // diagnostics vanish and then reappear.
        let mut phase1 = fast_diagnostics.clone();
        {
            let cache = self.diag_last_slow.lock();
            if let Some(prev_slow) = cache.get(uri_str) {
                phase1.extend(prev_slow.iter().cloned());
            }
        }
        {
            let cache = self.phpstan_last_diags.lock();
            if let Some(prev_phpstan) = cache.get(uri_str) {
                phase1.extend(prev_phpstan.iter().cloned());
            }
        }
        deduplicate_diagnostics(&mut phase1);
        client.publish_diagnostics(uri.clone(), phase1, None).await;

        // ── Phase 2: slow diagnostics (require type resolution) ─────────
        let mut slow_diagnostics = Vec::new();

        // ── Unknown class references ────────────────────────────────────
        self.collect_unknown_class_diagnostics(uri_str, content, &mut slow_diagnostics);

        // ── Unknown member access ───────────────────────────────────────
        self.collect_unknown_member_diagnostics(uri_str, content, &mut slow_diagnostics);

        // ── Unknown function calls ──────────────────────────────────────
        self.collect_unknown_function_diagnostics(uri_str, content, &mut slow_diagnostics);

        // ── Unresolved member access (opt-in) ───────────────────────
        self.collect_unresolved_member_access_diagnostics(uri_str, content, &mut slow_diagnostics);

        // ── Argument count errors ───────────────────────────────────
        self.collect_argument_count_diagnostics(uri_str, content, &mut slow_diagnostics);

        // Cache the fresh slow diagnostics for the next fast-phase merge.
        {
            let mut cache = self.diag_last_slow.lock();
            cache.insert(uri_str.to_string(), slow_diagnostics.clone());
        }

        // ── Combine fast + slow + cached PHPStan and deduplicate ────────
        let mut diagnostics = fast_diagnostics;
        diagnostics.extend(slow_diagnostics);
        {
            let cache = self.phpstan_last_diags.lock();
            if let Some(prev_phpstan) = cache.get(uri_str) {
                diagnostics.extend(prev_phpstan.iter().cloned());
            }
        }

        // When multiple providers flag the same span, suppress the
        // lower-value diagnostic.  In particular, suppress
        // `unresolved_member_access` (Hint) when a more specific
        // `unknown_member`, `scalar_member_access`, or `unknown_class`
        // diagnostic already covers an overlapping range.
        deduplicate_diagnostics(&mut diagnostics);

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
        {
            let mut pending = self.diag_pending_uris.lock();
            if !pending.contains(&uri) {
                pending.push(uri.clone());
            }
        }
        // Bump version so the worker knows there is fresh work.
        self.diag_version.fetch_add(1, Ordering::Release);
        // Wake the worker (no-op if it is already awake).
        self.diag_notify.notify_one();

        // Also schedule a PHPStan run for this file.
        self.schedule_phpstan(uri);
    }

    /// Schedule a diagnostic pass for every currently open file.
    ///
    /// Called when a class signature changes in one file, because
    /// diagnostics in other open files (unknown member, unknown class,
    /// deprecated usage) may depend on the changed class.  The edited
    /// file itself is excluded (it is already scheduled by the caller).
    pub(crate) fn schedule_diagnostics_for_open_files(&self, exclude_uri: &str) {
        let uris: Vec<String> = self
            .open_files
            .read()
            .keys()
            .filter(|u| u.as_str() != exclude_uri)
            .cloned()
            .collect();
        if uris.is_empty() {
            return;
        }
        {
            let mut pending = self.diag_pending_uris.lock();
            for uri in uris {
                if !pending.contains(&uri) {
                    pending.push(uri);
                }
            }
        }
        self.diag_version.fetch_add(1, Ordering::Release);
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

            // ── Step 3: snapshot all pending URIs ────────────────────
            let uris: Vec<String> = {
                let mut pending = self.diag_pending_uris.lock();
                std::mem::take(&mut *pending)
            };
            if uris.is_empty() {
                continue;
            }

            // ── Step 4: collect and publish for each URI ────────────
            // Snapshot content for each URI individually, releasing the
            // read lock before each async publish call so that
            // `did_change` is never blocked.
            for uri in &uris {
                let content = {
                    let files = self.open_files.read();
                    match files.get(uri) {
                        Some(c) => c.clone(),
                        None => continue,
                    }
                };
                self.publish_diagnostics_for_file(uri, &content).await;
            }
        }
    }

    // ── PHPStan worker ──────────────────────────────────────────────

    /// Schedule a PHPStan run for a single file.
    ///
    /// Only the most recent file is kept: if the user switches files or
    /// types rapidly, earlier requests are superseded.  This is
    /// intentional — PHPStan is too slow to queue up multiple files.
    fn schedule_phpstan(&self, uri: String) {
        *self.phpstan_pending_uri.lock() = Some(uri);
        self.phpstan_notify.notify_one();
    }

    /// Long-lived background task that runs PHPStan on pending files.
    ///
    /// Spawned once during `initialized`, alongside the main diagnostic
    /// worker.  This task is completely independent: native diagnostics
    /// (phases 1 and 2) are never blocked by PHPStan.
    ///
    /// ## Serialization guarantee
    ///
    /// At most one PHPStan process runs at a time.  The worker loop:
    ///
    /// 1. Wait for a notification (new edit arrived).
    /// 2. Debounce: sleep [`PHPSTAN_DEBOUNCE_MS`], checking whether new
    ///    edits arrived.  If so, restart the debounce.
    /// 3. Snapshot the pending URI and file content.
    /// 4. Resolve the PHPStan binary (skip if not found / disabled).
    /// 5. Run PHPStan (blocking — this is the slow part).
    /// 6. Cache the results and re-publish diagnostics for the file.
    /// 7. Loop back to step 1.
    ///
    /// If the user edits while step 5 is in progress, the pending URI
    /// is updated.  When step 5 finishes, the worker sees the new
    /// notification and loops back to step 1, starting a fresh run
    /// with the latest content.
    pub(crate) async fn phpstan_worker(&self) {
        loop {
            // ── Step 1: wait for work ───────────────────────────────
            self.phpstan_notify.notified().await;

            // Drain any extra stored permits so that notifications
            // that arrived between the last run finishing and this
            // `notified()` call don't cause an immediate second run.
            // `Notify::notify_one()` stores at most one permit, but
            // multiple `schedule_phpstan` calls during debounce or
            // execution could leave one behind.
            //
            // We consume it by polling a fresh `notified()` with a
            // zero timeout — if there's a stored permit it resolves
            // immediately, otherwise it times out harmlessly.
            let _ = tokio::time::timeout(std::time::Duration::ZERO, self.phpstan_notify.notified())
                .await;

            // ── Step 2: debounce (longer than normal diagnostics) ───
            loop {
                let version_before = self.diag_version.load(Ordering::Acquire);
                tokio::time::sleep(std::time::Duration::from_millis(PHPSTAN_DEBOUNCE_MS)).await;
                let version_after = self.diag_version.load(Ordering::Acquire);
                if version_before == version_after {
                    break;
                }
                // More edits arrived — loop and debounce again.
            }

            // ── Step 3: snapshot the pending URI ────────────────────
            let uri = {
                let mut pending = self.phpstan_pending_uri.lock();
                pending.take()
            };
            let uri = match uri {
                Some(u) => u,
                None => continue,
            };

            // Snapshot the file content.
            let content = {
                let files = self.open_files.read();
                match files.get(&uri) {
                    Some(c) => c.clone(),
                    None => continue,
                }
            };

            // ── Step 4: resolve PHPStan binary ──────────────────────
            let config = self.config();
            if config.phpstan.is_disabled() {
                continue;
            }

            let file_path = match uri.parse::<Url>().ok().and_then(|u| u.to_file_path().ok()) {
                Some(p) => p,
                None => continue,
            };

            let workspace_root = self.workspace_root.read().clone();
            let workspace_root = match workspace_root {
                Some(root) => root,
                None => continue,
            };

            let bin_dir: Option<String> =
                std::fs::read_to_string(workspace_root.join("composer.json"))
                    .ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .map(|json| crate::composer::get_bin_dir(&json));

            let resolved = match phpstan::resolve_phpstan(
                Some(&workspace_root),
                &config.phpstan,
                bin_dir.as_deref(),
            ) {
                Some(r) => r,
                None => continue,
            };

            // ── Step 5: run PHPStan (the slow part) ─────────────────
            // Move the blocking PHPStan execution onto a dedicated
            // OS thread via `spawn_blocking`.  This is critical:
            // `run_phpstan` contains a poll loop that blocks the
            // thread.  If we ran it inline, the tokio runtime could
            // schedule other futures (including a second iteration
            // of this very worker) on other threads, breaking the
            // "at most one PHPStan process" guarantee.  By awaiting
            // the `spawn_blocking` handle, this task is suspended
            // (not occupying a runtime thread) and no re-entry can
            // happen until the handle resolves.
            let phpstan_config = config.phpstan.clone();
            let phpstan_diags = {
                let result = tokio::task::spawn_blocking(move || {
                    phpstan::run_phpstan(
                        &resolved,
                        &content,
                        &file_path,
                        &workspace_root,
                        &phpstan_config,
                    )
                })
                .await;

                match result {
                    Ok(Ok(diags)) => diags,
                    Ok(Err(_e)) => {
                        // PHPStan failures are silently ignored to
                        // avoid flooding the editor with errors when
                        // PHPStan is misconfigured or the project
                        // doesn't use it.
                        continue;
                    }
                    Err(_join_err) => {
                        // The blocking task panicked or was cancelled.
                        continue;
                    }
                }
            };

            // ── Step 6: cache results and re-publish ────────────────
            {
                let mut cache = self.phpstan_last_diags.lock();
                cache.insert(uri.clone(), phpstan_diags);
            }

            // Re-publish diagnostics for this file so the editor
            // sees the fresh PHPStan results merged with the native
            // diagnostics.  We re-run the full publish (phases 1+2
            // + fresh PHPStan cache) rather than publishing PHPStan
            // results alone, because `publishDiagnostics` replaces
            // the entire diagnostic set for the file.
            let content = {
                let files = self.open_files.read();
                match files.get(&uri) {
                    Some(c) => c.clone(),
                    None => continue,
                }
            };
            self.publish_diagnostics_for_file(&uri, &content).await;
        }
    }

    /// Clear diagnostics for a file (e.g. on `did_close`).
    pub(crate) async fn clear_diagnostics_for_file(&self, uri_str: &str) {
        // Remove cached slow diagnostics so we don't leak memory.
        self.diag_last_slow.lock().remove(uri_str);
        // Remove cached PHPStan diagnostics too.
        self.phpstan_last_diags.lock().remove(uri_str);

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

// ── Deduplication ───────────────────────────────────────────────────────────

/// Suppress lower-priority diagnostics when a higher-priority one covers
/// an overlapping range.
///
/// Rules (in precedence order):
/// 1. `unknown_class` trumps `unresolved_member_access`
/// 2. `unknown_member` trumps `unresolved_member_access`
/// 3. `scalar_member_access` trumps `unresolved_member_access`
///
/// Also removes exact duplicates (same range + same message).
fn deduplicate_diagnostics(diagnostics: &mut Vec<Diagnostic>) {
    if diagnostics.is_empty() {
        return;
    }

    // Collect the ranges of "priority" diagnostics that should
    // suppress `unresolved_member_access` hints.
    let priority_codes: &[&str] = &[
        "unknown_class",
        "unknown_member",
        "scalar_member_access",
        "unknown_function",
    ];

    let priority_ranges: Vec<Range> = diagnostics
        .iter()
        .filter(|d| {
            d.code
                .as_ref()
                .map(|c| match c {
                    NumberOrString::String(s) => priority_codes.contains(&s.as_str()),
                    _ => false,
                })
                .unwrap_or(false)
        })
        .map(|d| d.range)
        .collect();

    // Collect lines that have at least one precise (sub-line)
    // diagnostic.  A diagnostic is "precise" when it does not span the
    // entire line, i.e. it has a meaningful character range rather than
    // `0..MAX`.  External tools like PHPStan only report a line number,
    // so their diagnostics stretch the full line.  When a native
    // diagnostic already pinpoints the exact location on that line, the
    // full-line underline is redundant noise.
    let mut lines_with_precise_diag = std::collections::HashSet::new();
    for d in diagnostics.iter() {
        if !is_full_line_range(&d.range) {
            lines_with_precise_diag.insert(d.range.start.line);
        }
    }

    diagnostics.retain(|d| {
        let is_unresolved = d
            .code
            .as_ref()
            .map(|c| match c {
                NumberOrString::String(s) => s == "unresolved_member_access",
                _ => false,
            })
            .unwrap_or(false);

        if is_unresolved {
            // Suppress if any priority diagnostic overlaps this range.
            return !priority_ranges
                .iter()
                .any(|pr| ranges_overlap(pr, &d.range));
        }

        // Suppress full-line diagnostics on lines where a more precise
        // diagnostic already exists.  This avoids the visual clutter of
        // a line-wide PHPStan underline next to a pinpointed native
        // error.  The suppressed diagnostic will reappear once the user
        // resolves the precise one.
        if is_full_line_range(&d.range) && lines_with_precise_diag.contains(&d.range.start.line) {
            return false;
        }

        true
    });

    // Remove exact duplicates (same range + same message).
    diagnostics.dedup_by(|a, b| a.range == b.range && a.message == b.message);
}

/// Returns `true` if the range spans a full line (character 0 to a
/// very large end character).  PHPStan and other line-only tools
/// produce these ranges because they don't report column information.
fn is_full_line_range(range: &Range) -> bool {
    range.start.line == range.end.line && range.start.character == 0 && range.end.character >= 1000
}

/// Check whether two LSP ranges overlap.
fn ranges_overlap(a: &Range, b: &Range) -> bool {
    // Two ranges overlap if neither ends before the other starts.
    !(a.end.line < b.start.line
        || (a.end.line == b.start.line && a.end.character <= b.start.character)
        || b.end.line < a.start.line
        || (b.end.line == a.start.line && b.end.character <= a.start.character))
}

// ── Helpers ─────────────────────────────────────────────────────────────────

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

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ─────────────────────────────────────────────────────

    fn make_range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> Range {
        Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        }
    }

    fn make_diagnostic(
        range: Range,
        severity: DiagnosticSeverity,
        code: &str,
        message: &str,
    ) -> Diagnostic {
        Diagnostic {
            range,
            severity: Some(severity),
            code: Some(NumberOrString::String(code.to_string())),
            code_description: None,
            source: Some("phpantom".to_string()),
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    // ── ranges_overlap ──────────────────────────────────────────────

    #[test]
    fn overlapping_ranges_on_same_line() {
        let a = make_range(5, 0, 5, 10);
        let b = make_range(5, 5, 5, 15);
        assert!(ranges_overlap(&a, &b));
        assert!(ranges_overlap(&b, &a));
    }

    #[test]
    fn non_overlapping_ranges_on_same_line() {
        let a = make_range(5, 0, 5, 5);
        let b = make_range(5, 5, 5, 10);
        assert!(!ranges_overlap(&a, &b));
        assert!(!ranges_overlap(&b, &a));
    }

    #[test]
    fn non_overlapping_ranges_on_different_lines() {
        let a = make_range(1, 0, 1, 10);
        let b = make_range(2, 0, 2, 10);
        assert!(!ranges_overlap(&a, &b));
    }

    #[test]
    fn identical_ranges_overlap() {
        let r = make_range(3, 5, 3, 10);
        assert!(ranges_overlap(&r, &r));
    }

    #[test]
    fn contained_range_overlaps() {
        let outer = make_range(1, 0, 10, 0);
        let inner = make_range(5, 5, 5, 10);
        assert!(ranges_overlap(&outer, &inner));
        assert!(ranges_overlap(&inner, &outer));
    }

    // ── deduplicate_diagnostics ─────────────────────────────────────

    #[test]
    fn suppresses_unresolved_member_when_unknown_class_overlaps() {
        let range = make_range(5, 0, 5, 15);
        let mut diags = vec![
            make_diagnostic(
                range,
                DiagnosticSeverity::WARNING,
                "unknown_class",
                "Unknown class X",
            ),
            make_diagnostic(
                range,
                DiagnosticSeverity::HINT,
                "unresolved_member_access",
                "Unresolved member access on X",
            ),
        ];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("unknown_class".to_string()))
        );
    }

    #[test]
    fn suppresses_unresolved_member_when_unknown_member_overlaps() {
        let range = make_range(10, 0, 10, 20);
        let mut diags = vec![
            make_diagnostic(
                range,
                DiagnosticSeverity::WARNING,
                "unknown_member",
                "Unknown member foo",
            ),
            make_diagnostic(
                range,
                DiagnosticSeverity::HINT,
                "unresolved_member_access",
                "Unresolved member access",
            ),
        ];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("unknown_member".to_string()))
        );
    }

    #[test]
    fn suppresses_unresolved_member_when_scalar_member_access_overlaps() {
        let range_outer = make_range(3, 0, 3, 20);
        let range_inner = make_range(3, 5, 3, 15);
        let mut diags = vec![
            make_diagnostic(
                range_outer,
                DiagnosticSeverity::ERROR,
                "scalar_member_access",
                "Cannot access member on scalar",
            ),
            make_diagnostic(
                range_inner,
                DiagnosticSeverity::HINT,
                "unresolved_member_access",
                "Unresolved member access",
            ),
        ];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String("scalar_member_access".to_string()))
        );
    }

    #[test]
    fn keeps_unresolved_member_when_no_priority_diagnostic() {
        let range = make_range(5, 0, 5, 15);
        let mut diags = vec![make_diagnostic(
            range,
            DiagnosticSeverity::HINT,
            "unresolved_member_access",
            "Unresolved member access",
        )];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 1);
    }

    #[test]
    fn keeps_unresolved_member_on_different_range() {
        let mut diags = vec![
            make_diagnostic(
                make_range(5, 0, 5, 10),
                DiagnosticSeverity::WARNING,
                "unknown_class",
                "Unknown class X",
            ),
            make_diagnostic(
                make_range(10, 0, 10, 10),
                DiagnosticSeverity::HINT,
                "unresolved_member_access",
                "Unresolved member access on Y",
            ),
        ];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn suppresses_multiple_unresolved_members_with_priority_overlap() {
        let range = make_range(5, 0, 5, 15);
        let mut diags = vec![
            make_diagnostic(
                range,
                DiagnosticSeverity::WARNING,
                "unknown_class",
                "Unknown class X",
            ),
            make_diagnostic(
                range,
                DiagnosticSeverity::HINT,
                "unresolved_member_access",
                "Unresolved 1",
            ),
            make_diagnostic(
                range,
                DiagnosticSeverity::HINT,
                "unresolved_member_access",
                "Unresolved 2",
            ),
            make_diagnostic(
                make_range(20, 0, 20, 10),
                DiagnosticSeverity::HINT,
                "unresolved_member_access",
                "Unresolved 3 (different range)",
            ),
        ];
        deduplicate_diagnostics(&mut diags);
        // Only the unknown_class + the one on a different range should survive.
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn no_op_when_no_diagnostics() {
        let mut diags: Vec<Diagnostic> = vec![];
        deduplicate_diagnostics(&mut diags);
        assert!(diags.is_empty());
    }

    #[test]
    fn suppresses_full_line_phpstan_when_precise_diagnostic_on_same_line() {
        let phpstan = Diagnostic {
            range: make_range(5, 0, 5, u32::MAX),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("argument.type".to_string())),
            source: Some("phpstan".to_string()),
            message: "Parameter #1 $x expects int, string given.".to_string(),
            ..Default::default()
        };
        let precise = Diagnostic {
            range: make_range(5, 10, 5, 20),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("unknown_class".to_string())),
            source: Some("phpantom".to_string()),
            message: "Class 'Foo' not found".to_string(),
            ..Default::default()
        };
        let mut diags = vec![phpstan, precise.clone()];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, precise.message);
    }

    #[test]
    fn keeps_full_line_phpstan_when_no_precise_diagnostic_on_line() {
        let phpstan = Diagnostic {
            range: make_range(5, 0, 5, u32::MAX),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("argument.type".to_string())),
            source: Some("phpstan".to_string()),
            message: "Parameter #1 $x expects int, string given.".to_string(),
            ..Default::default()
        };
        let precise_other_line = Diagnostic {
            range: make_range(10, 3, 10, 15),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("unknown_class".to_string())),
            source: Some("phpantom".to_string()),
            message: "Class 'Bar' not found".to_string(),
            ..Default::default()
        };
        let mut diags = vec![phpstan.clone(), precise_other_line.clone()];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn keeps_precise_phpstan_diagnostic_on_same_line() {
        // If a future PHPStan version provides column info, don't suppress it.
        let phpstan_precise = Diagnostic {
            range: make_range(5, 8, 5, 20),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("argument.type".to_string())),
            source: Some("phpstan".to_string()),
            message: "Parameter #1 $x expects int, string given.".to_string(),
            ..Default::default()
        };
        let native_precise = Diagnostic {
            range: make_range(5, 3, 5, 10),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("unknown_class".to_string())),
            source: Some("phpantom".to_string()),
            message: "Class 'Foo' not found".to_string(),
            ..Default::default()
        };
        let mut diags = vec![phpstan_precise.clone(), native_precise.clone()];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn suppresses_multiple_full_line_diags_when_precise_exists() {
        let phpstan1 = Diagnostic {
            range: make_range(5, 0, 5, u32::MAX),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("argument.type".to_string())),
            source: Some("phpstan".to_string()),
            message: "Error one".to_string(),
            ..Default::default()
        };
        let phpstan2 = Diagnostic {
            range: make_range(5, 0, 5, u32::MAX),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("return.type".to_string())),
            source: Some("phpstan".to_string()),
            message: "Error two".to_string(),
            ..Default::default()
        };
        let precise = Diagnostic {
            range: make_range(5, 2, 5, 8),
            severity: Some(DiagnosticSeverity::WARNING),
            code: Some(NumberOrString::String("unknown_member".to_string())),
            source: Some("phpantom".to_string()),
            message: "Method 'foo' not found".to_string(),
            ..Default::default()
        };
        let mut diags = vec![phpstan1, phpstan2, precise.clone()];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, precise.message);
    }
}
