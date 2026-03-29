//! Diagnostics — collect and deliver LSP diagnostics for PHP files.
//!
//! This module collects diagnostics from multiple providers and delivers
//! them to the editor.  Two delivery models are supported:
//!
//! - **Pull model** (`textDocument/diagnostic`, LSP 3.17) — the editor
//!   requests diagnostics when it needs them.  Only visible files are
//!   diagnosed.  Cross-file invalidation uses `workspace/diagnostic/refresh`.
//!   This is the preferred model when the client supports it.
//!
//! - **Push model** (`textDocument/publishDiagnostics`) — the server
//!   pushes diagnostics after every edit.  Used as a fallback for clients
//!   that do not advertise pull-diagnostic support.
//!
//! Providers are grouped into three phases so that cheap results appear
//! immediately and expensive external tools never block native feedback:
//!
//! ## Phase 1 — fast (no type resolution)
//!
//! - **Syntax error diagnostics** — surface parse errors from the Mago
//!   parser as Error-severity diagnostics.  The most fundamental
//!   diagnostic: without it, a user with a typo gets no feedback until
//!   they try to run the code.
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
//! - **Implementation error diagnostics** — report concrete classes that
//!   fail to implement all required methods from their interfaces or
//!   abstract parents.  Reuses the same missing-method detection as the
//!   "Implement missing methods" code action.
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
//! Fast diagnostics are **always pushed** immediately via
//! `textDocument/publishDiagnostics`, merged with cached slow and
//! PHPStan results so the editor never shows a gap.  This gives
//! instant feedback (strikethrough, dimming) regardless of client
//! capabilities.
//!
//! Slow diagnostics are then computed by the background worker:
//!
//! - **Pull mode** — the worker caches the full result (fast + fresh
//!   slow + cached PHPStan) and sends `workspace/diagnostic/refresh`.
//!   The editor re-pulls and gets the complete set.  No second push
//!   is needed.
//!
//! - **Push mode** (fallback) — the worker pushes the full result
//!   (fast + fresh slow + cached PHPStan) via `publishDiagnostics`,
//!   replacing the Phase 1 snapshot.
//!
//! - **PHPStan worker** — caches its results and triggers a re-deliver
//!   (refresh in pull mode, full re-publish in push mode).
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
mod implementation_errors;
mod syntax_errors;
pub(crate) mod unknown_classes;
pub(crate) mod unknown_functions;
pub(crate) mod unknown_members;
pub(crate) mod unresolved_member_access;
mod unused_imports;

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::phpstan;
use crate::util::ranges_overlap;

// ── Shared helpers ──────────────────────────────────────────────────────────

impl Backend {
    /// Returns `true` if the URI should be skipped for diagnostics
    /// (stub files and vendor files).
    fn should_skip_diagnostics(&self, uri_str: &str) -> bool {
        if uri_str.starts_with("phpantom-stub://") || uri_str.starts_with("phpantom-stub-fn://") {
            return true;
        }
        let prefixes = self.vendor_uri_prefixes.lock();
        prefixes.iter().any(|p| uri_str.starts_with(p.as_str()))
    }

    /// Collect Phase 1 (fast) diagnostics: syntax errors, unused
    /// imports.  These are cheap — no type resolution.
    pub(crate) fn collect_fast_diagnostics(
        &self,
        uri_str: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        self.collect_syntax_error_diagnostics(uri_str, content, out);
        self.collect_unused_import_diagnostics(uri_str, content, out);
    }

    /// Collect Phase 2 (slow) diagnostics: unknown class/member/function,
    /// argument count, implementation errors, deprecated usage.  These
    /// require type resolution and are expensive.
    pub(crate) fn collect_slow_diagnostics(
        &self,
        uri_str: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        self.collect_unknown_class_diagnostics(uri_str, content, out);
        self.collect_unknown_member_diagnostics(uri_str, content, out);
        self.collect_unknown_function_diagnostics(uri_str, content, out);
        // NOTE: unresolved_member_access diagnostics are now emitted
        // inside collect_unknown_member_diagnostics (in the Untyped arm)
        // to avoid a second full walk with duplicate type resolution.
        self.collect_argument_count_diagnostics(uri_str, content, out);
        self.collect_implementation_error_diagnostics(uri_str, content, out);
        self.collect_deprecated_diagnostics(uri_str, content, out);
    }

    /// Build a merged diagnostic set from fresh fast diagnostics,
    /// cached slow diagnostics, and cached PHPStan diagnostics.
    ///
    /// Stale PHPStan diagnostics are eagerly pruned when the current
    /// file content no longer matches the condition that triggered
    /// them.  This gives instant visual feedback after applying a
    /// code action without waiting for the next PHPStan run:
    ///
    /// - `throws.*` diagnostics are pruned when the `@throws` tag
    ///   they reference has been added or removed.
    /// - Any PHPStan diagnostic is pruned when its line now contains
    ///   a `@phpstan-ignore` comment that covers the identifier.
    fn merge_fast_with_cached(&self, uri_str: &str, fast: &[Diagnostic]) -> Vec<Diagnostic> {
        let mut merged = fast.to_vec();
        {
            let cache = self.diag_last_slow.lock();
            if let Some(prev_slow) = cache.get(uri_str) {
                merged.extend(prev_slow.iter().cloned());
            }
        }
        {
            let content: Option<Arc<String>> = self.open_files.read().get(uri_str).cloned();
            let mut cache = self.phpstan_last_diags.lock();
            if let Some(prev_phpstan) = cache.get(uri_str) {
                let filtered: Vec<Diagnostic> = prev_phpstan
                    .iter()
                    .filter(|d| {
                        if let Some(ref text) = content {
                            !is_stale_phpstan_diagnostic(d, text)
                        } else {
                            true
                        }
                    })
                    .cloned()
                    .collect();
                if filtered.len() != prev_phpstan.len() {
                    cache.insert(uri_str.to_string(), filtered.clone());
                }
                merged.extend(filtered);
            }
        }
        deduplicate_diagnostics(&mut merged);
        merged
    }
}

/// Check whether a cached PHPStan diagnostic is stale given the current
/// file content.
///
/// A diagnostic is stale when the user has already fixed the underlying
/// issue (via a code action or manual edit) but PHPStan hasn't re-run
/// yet to clear it:
///
/// - `throws.unusedType` / `throws.notThrowable`: the `@throws` tag
///   was removed — stale if the type no longer appears after `@throws`.
/// - `missingType.checkedException`: the `@throws` tag was added —
///   stale if the exception short name now appears after `@throws`.
/// - `method.missingOverride`: the `#[Override]` attribute was added —
///   stale if a `#[...]` line containing `Override` appears near the
///   diagnostic line.
/// - **Any identifier**: the line now contains a `@phpstan-ignore`
///   comment that covers the diagnostic's identifier.
fn is_stale_phpstan_diagnostic(diag: &Diagnostic, content: &str) -> bool {
    let identifier = match &diag.code {
        Some(NumberOrString::String(s)) => s.as_str(),
        _ => return false,
    };

    // ── @phpstan-ignore covers this diagnostic ──────────────────────
    // If the line where the diagnostic appears now has a
    // `@phpstan-ignore` comment listing this identifier, the user
    // already suppressed it and the diagnostic is stale.
    if !identifier.is_empty()
        && identifier != "phpstan"
        && !identifier.starts_with("ignore.unmatched")
        && line_has_ignore_for(content, diag.range.start.line, identifier)
    {
        return true;
    }

    // The per-identifier heuristics for `throws.unusedType`,
    // `missingType.checkedException`, and `method.missingOverride`
    // have been removed.  These diagnostics are now cleared eagerly
    // by `codeAction/resolve` when the user picks a PHPStan quickfix
    // (see `clear_phpstan_diagnostics_after_resolve` in code_actions).
    // The `@phpstan-ignore` check above still covers manual edits.

    // ── method.override / property.override / property.overrideAttribute ─
    // The user may remove the attribute by hand, so check whether
    // `#[Override]` is still present near the diagnostic line.
    if identifier == "method.override"
        || identifier == "property.override"
        || identifier == "property.overrideAttribute"
    {
        return crate::code_actions::phpstan::remove_override::is_remove_override_stale(
            content,
            diag.range.start.line as usize,
        );
    }

    // ── method.tentativeReturnType — #[\ReturnTypeWillChange] added ─
    // The user may add the attribute by hand, so check whether it is
    // now present near the diagnostic line.
    if identifier == "method.tentativeReturnType" {
        return crate::code_actions::phpstan::add_return_type_will_change::is_add_return_type_will_change_stale(
            content,
            diag.range.start.line as usize,
        );
    }

    // ── new.static — check if the user manually fixed the class ─────
    // Unlike the actions above, `new.static` fixes are commonly applied
    // by hand (adding `final` to the class or constructor), so we keep
    // a content-based heuristic here.
    if identifier == "new.static" {
        return crate::code_actions::phpstan::new_static::is_new_static_stale(
            content,
            diag.range.start.line as usize,
        );
    }

    false
}

// The following helpers were used by the per-identifier stale detection
// branches that have been removed.  They are kept under `#[cfg(test)]`
// because existing tests exercise them directly.

#[cfg(test)]
#[allow(dead_code)]
/// Extract the type name from a `throws.unusedType` or
/// `throws.notThrowable` message.
fn extract_throws_diag_type(message: &str, identifier: &str) -> Option<String> {
    if identifier == "throws.unusedType" {
        let start = message.find(" has ")? + 5;
        let rest = &message[start..];
        let end = rest.find(" in PHPDoc @throws tag")?;
        Some(rest[..end].trim().to_string())
    } else {
        let start = message.find("@throws with type ")? + 18;
        let rest = &message[start..];
        let end = rest.find(" is not subtype")?;
        Some(rest[..end].trim().to_string())
    }
}

#[cfg(test)]
#[allow(dead_code)]
/// Extract the exception FQN from a `missingType.checkedException` message.
fn extract_checked_exception_fqn(message: &str) -> Option<String> {
    let marker = "throws checked exception ";
    let start = message.find(marker)? + marker.len();
    let rest = &message[start..];
    let end = rest.find(" but")?;
    let fqn = rest[..end].trim().trim_start_matches('\\');
    if fqn.is_empty() {
        return None;
    }
    Some(fqn.to_string())
}

/// Check whether the diagnostic's line (or the line before it) has a
/// `@phpstan-ignore` comment that lists the given identifier.
///
/// PHPStan ignore comments can appear:
/// - On the same line as the code: `$x = foo(); // @phpstan-ignore id`
/// - On the line before: `// @phpstan-ignore id`
///
/// Only the per-identifier form (`@phpstan-ignore id1, id2`) is
/// checked.  The blanket `@phpstan-ignore-line` and
/// `@phpstan-ignore-next-line` variants are **not** treated as a
/// match — our code action only produces per-identifier ignores, so
/// we should not eagerly clear diagnostics that happen to sit on a
/// line with a blanket suppression the user added independently.
fn line_has_ignore_for(content: &str, diag_line: u32, identifier: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    let line_idx = diag_line as usize;

    // Check the diagnostic line itself and the line before it.
    for idx in [line_idx, line_idx.wrapping_sub(1)] {
        if idx >= lines.len() {
            continue;
        }
        let line = lines[idx];
        if let Some(ignore_pos) = line.find("@phpstan-ignore") {
            let after = &line[ignore_pos + "@phpstan-ignore".len()..];
            // `@phpstan-ignore-line` and `@phpstan-ignore-next-line`
            // suppress everything — we can't attribute them to any
            // single identifier, so skip them.
            if after.starts_with("-line") || after.starts_with("-next-line") {
                continue;
            }
            // Parse the comma-separated identifier list.
            let ids_text = after.trim_start();
            // Stop at `*/`, ` (reason)`, or end of string.
            let ids_end = ids_text
                .find("*/")
                .or_else(|| ids_text.find(" ("))
                .unwrap_or(ids_text.len());
            let ids = &ids_text[..ids_end];
            if ids.split(',').any(|id| id.trim() == identifier) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
#[allow(dead_code)]
/// Find the docblock text for the function/method enclosing `diag_line`.
///
/// Searches backward from `diag_line` to find the nearest `function`
/// keyword (which may be on the diagnostic line itself, e.g. on the
/// signature, or on a preceding line when the diagnostic is inside the
/// body or in the docblock above).  Then looks for a preceding
/// `/** ... */` block.  Returns the raw docblock text (from `/**` to
/// `*/` inclusive) if found, or an empty string if no docblock exists.
fn enclosing_docblock_text(content: &str, diag_line: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if diag_line >= lines.len() {
        return String::new();
    }

    // Scan backward from `diag_line` looking for a line that contains
    // the `function` keyword.  This handles three cases:
    //   1. Diagnostic inside the function body → walks up to the
    //      signature line.
    //   2. Diagnostic on the signature line → matches immediately.
    //   3. Diagnostic on the docblock above → walks down would be
    //      needed, but PHPStan diagnostics land on the signature or
    //      body, not the docblock lines.  If we reach the docblock
    //      line we still need to find the function below it.  As a
    //      pragmatic fallback we also scan forward a few lines.
    let mut func_line: Option<usize> = None;
    for idx in (0..=diag_line).rev() {
        if line_has_function_keyword(lines[idx]) {
            func_line = Some(idx);
            break;
        }
    }

    // Fallback: if the diagnostic is on a docblock line above the
    // function, scan forward a few lines to find the signature.
    if func_line.is_none() {
        let start = diag_line + 1;
        let limit = (diag_line + 10).min(lines.len());
        for (i, line) in lines[start..limit].iter().enumerate() {
            if line_has_function_keyword(line) {
                func_line = Some(start + i);
                break;
            }
        }
    }

    let func_line = match func_line {
        Some(l) => l,
        None => return String::new(),
    };

    // Compute the byte offset of the `function` keyword on that line.
    let line_byte_start: usize = lines.iter().take(func_line).map(|l| l.len() + 1).sum();
    let func_kw_rel = match lines[func_line].find("function") {
        Some(p) => p,
        None => return String::new(),
    };
    let func_kw_pos = line_byte_start + func_kw_rel;

    // Look for a `/** ... */` block before the function keyword
    // (skipping modifiers and whitespace).
    let before_func = &content[..func_kw_pos];
    let trimmed = before_func.trim_end();

    // Strip trailing modifier keywords.
    let modifiers = [
        "public",
        "protected",
        "private",
        "static",
        "abstract",
        "final",
        "readonly",
    ];
    let mut result = trimmed;
    loop {
        let t = result.trim_end();
        let mut found = false;
        for kw in &modifiers {
            if let Some(prefix) = t.strip_suffix(kw)
                && (prefix.is_empty()
                    || prefix
                        .as_bytes()
                        .last()
                        .is_some_and(|&b| !b.is_ascii_alphanumeric() && b != b'_'))
            {
                result = prefix;
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
    }

    let after_mods = result.trim_end();
    if after_mods.ends_with("*/")
        && let Some(open) = after_mods.rfind("/**")
    {
        return after_mods[open..].to_string();
    }

    String::new()
}

#[cfg(test)]
#[allow(dead_code)]
/// Check whether `line` contains the `function` keyword (not part of
/// a larger identifier like `myfunction`).
fn line_has_function_keyword(line: &str) -> bool {
    let Some(pos) = line.find("function") else {
        return false;
    };
    let before_ok = pos == 0 || line.as_bytes()[pos - 1].is_ascii_whitespace();
    let after_pos = pos + "function".len();
    let after_ok = after_pos >= line.len() || {
        let b = line.as_bytes()[after_pos];
        !b.is_ascii_alphanumeric() && b != b'_'
    };
    before_ok && after_ok
}

#[cfg(test)]
#[allow(dead_code)]
/// Check whether `scope` (typically a single docblock) contains
/// `@throws <short_name>` (case-insensitive).
fn scope_has_throws_tag(scope: &str, short_name: &str) -> bool {
    use mago_docblock::document::TagKind;

    let info = match crate::docblock::parser::parse_docblock_for_tags(scope) {
        Some(info) => info,
        None => return false,
    };

    let lower = short_name.to_lowercase();
    for tag in info.tags_by_kind(TagKind::Throws) {
        let rest = tag.description.trim();
        if let Some(tag_type) = rest.split_whitespace().next() {
            let tag_short = tag_type.trim_start_matches('\\');
            let tag_short = tag_short.rsplit('\\').next().unwrap_or(tag_short);
            if tag_short.to_lowercase() == lower {
                return true;
            }
        }
    }
    false
}

/// How long to wait after the last keystroke before publishing diagnostics.
const DIAGNOSTIC_DEBOUNCE_MS: u64 = 500;

/// How long to wait after the last keystroke before running PHPStan.
///
/// Longer than the normal debounce because PHPStan is extremely
/// expensive.  We want the user to be truly idle before spawning it.
const PHPSTAN_DEBOUNCE_MS: u64 = 2_000;

impl Backend {
    /// Deliver diagnostics for a single file.
    ///
    /// Called from the background diagnostic worker after debouncing.
    ///
    /// **Phase 1 (instant, both modes):** Run fast collectors (syntax
    /// errors, deprecated, unused imports), merge with *cached* slow
    /// and PHPStan results, and push via `publishDiagnostics`.  The
    /// editor shows strikethrough and dimming within milliseconds.
    ///
    /// **Phase 2 (background, mode-dependent):**
    ///
    /// - **Pull mode:** Compute slow diagnostics, build the full set
    ///   (fast + fresh slow + cached PHPStan), cache it in
    ///   `diag_last_full`, bump the `resultId`, and send
    ///   `workspace/diagnostic/refresh`.  The editor re-pulls and
    ///   gets the complete set.  Push always serves cached slow, so
    ///   no second push is needed.
    ///
    /// - **Push mode (fallback):** Compute slow diagnostics, then
    ///   push the full set (fast + fresh slow + cached PHPStan),
    ///   replacing the Phase 1 snapshot.
    pub(crate) async fn publish_diagnostics_for_file(&self, uri_str: &str, content: &str) {
        let client = match &self.client {
            Some(c) => c,
            None => return,
        };

        if self.should_skip_diagnostics(uri_str) {
            return;
        }

        let pull_mode = self.supports_pull_diagnostics.load(Ordering::Acquire);

        // ── Phase 1: push fast diagnostics immediately ──────────────
        // Merge fresh fast with cached slow + PHPStan so the editor
        // never shows a gap where those diagnostics vanish then
        // reappear.
        //
        // Even in pull mode we push Phase 1 via publish_diagnostics so
        // users see syntax errors and unused-import warnings instantly,
        // without waiting for a pull round-trip. Phase 2 (slow) results
        // are delivered via pull after workspace/diagnostic/refresh.
        // This is intentional — do not remove the push here.
        let mut fast_diagnostics = Vec::new();
        self.collect_fast_diagnostics(uri_str, content, &mut fast_diagnostics);

        let phase1 = self.merge_fast_with_cached(uri_str, &fast_diagnostics);

        // Filter out any diagnostics that were eagerly suppressed by
        // a `codeAction/resolve` handler (e.g. unused-import removal).
        let phase1 = self.filter_suppressed(phase1);

        let uri = match uri_str.parse::<Url>() {
            Ok(u) => u,
            Err(_) => return,
        };
        client.publish_diagnostics(uri.clone(), phase1, None).await;

        // ── Phase 2: compute slow diagnostics ───────────────────────
        // The resolved-class cache guard must not cross an `.await`
        // point (it contains a raw pointer and is !Send).  Scope it
        // tightly around the synchronous diagnostic collection.
        let mut slow_diagnostics = Vec::new();
        {
            let _cache_guard = crate::virtual_members::with_active_resolved_class_cache(
                &self.resolved_class_cache,
            );
            // Activate the diagnostic subject cache so that
            // collect_unknown_member_diagnostics and
            // collect_argument_count_diagnostics share resolved
            // subjects instead of re-resolving them independently.
            let _subj_guard = crate::completion::resolver::with_diagnostic_subject_cache();

            // Provide scope boundaries so the diagnostic subject cache
            // can distinguish variables in different methods of the same
            // class (prevents cross-method cache pollution).
            if let Some(sm) = self.symbol_maps.read().get(uri_str) {
                crate::completion::resolver::set_diagnostic_subject_cache_scopes(
                    sm.scopes.clone(),
                    sm.var_defs.clone(),
                    sm.narrowing_blocks.clone(),
                    sm.assert_narrowing_offsets.clone(),
                );
            }

            self.collect_slow_diagnostics(uri_str, content, &mut slow_diagnostics);
        }

        // Cache fresh slow diagnostics for the next Phase 1 merge.
        {
            let mut cache = self.diag_last_slow.lock();
            cache.insert(uri_str.to_string(), slow_diagnostics.clone());
        }

        // Build the full set: fast + fresh slow + cached PHPStan.
        let mut full = fast_diagnostics;
        full.extend(slow_diagnostics);
        let phpstan_before: Vec<Diagnostic> = {
            let cache = self.phpstan_last_diags.lock();
            match cache.get(uri_str) {
                Some(diags) => diags.clone(),
                None => Vec::new(),
            }
        };
        full.extend(phpstan_before.iter().cloned());
        deduplicate_diagnostics(&mut full);

        // Filter out any diagnostics suppressed by codeAction/resolve.
        let full = self.filter_suppressed(full);

        // If deduplication suppressed any full-line PHPStan diagnostics
        // (because a precise native diagnostic covers the same line),
        // prune them from the PHPStan cache too.  Without this, the
        // next Phase 1 merge would resurrect the stale full-line
        // diagnostic as soon as the user fixes the precise error (the
        // precise diagnostic disappears from the slow cache, so the
        // full-line one would no longer be suppressed).
        if !phpstan_before.is_empty() {
            let pruned: Vec<Diagnostic> = phpstan_before
                .into_iter()
                .filter(|d| full.iter().any(|f| f.range == d.range))
                .collect();
            let mut cache = self.phpstan_last_diags.lock();
            cache.insert(uri_str.to_string(), pruned);
        }

        if pull_mode {
            // Cache for pull handlers, bump resultId, signal refresh.
            {
                let mut cache = self.diag_last_full.lock();
                cache.insert(uri_str.to_string(), full);
            }
            {
                let mut ids = self.diag_result_ids.lock();
                let id = ids.entry(uri_str.to_string()).or_insert(0);
                *id += 1;
            }
            let _ = client.workspace_diagnostic_refresh().await;
        } else {
            // Push the full set, replacing the Phase 1 snapshot.
            client.publish_diagnostics(uri, full, None).await;
        }
    }

    /// Notify the diagnostic system that a file needs fresh diagnostics.
    ///
    /// Queues the file for the background diagnostic worker.  In pull
    /// mode, also invalidates the cached full diagnostics so the worker
    /// recomputes them.  The pull handlers only ever return cached data,
    /// so they never block the LSP request thread.
    ///
    /// This returns immediately — all diagnostic computation happens
    /// in the background so that completion, hover, and signature help
    /// are never blocked.
    pub(crate) fn schedule_diagnostics(&self, uri: String) {
        let pull_mode = self.supports_pull_diagnostics.load(Ordering::Acquire);

        if pull_mode {
            // Invalidate the cached full diagnostics so the worker
            // knows this file needs recomputation.
            self.diag_last_full.lock().remove(&uri);
        }

        // In both modes, queue for the background worker.
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

    /// Invalidate diagnostics for all open files after a cross-file change.
    ///
    /// Called when a class signature changes in one file, because
    /// diagnostics in other open files (unknown member, unknown class,
    /// deprecated usage) may depend on the changed class.  The edited
    /// file itself is excluded (it is already scheduled by the caller).
    ///
    /// Queues all open files for the background worker.  In pull mode,
    /// also invalidates the cached full diagnostics so the worker
    /// recomputes them.
    pub(crate) fn schedule_diagnostics_for_open_files(&self, exclude_uri: &str) {
        let pull_mode = self.supports_pull_diagnostics.load(Ordering::Acquire);

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

        if pull_mode {
            // Invalidate cached full diagnostics so the worker
            // recomputes them.
            let mut cache = self.diag_last_full.lock();
            for uri in &uris {
                cache.remove(uri);
            }
        }

        // In both modes, queue all files for the background worker.
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
            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // ── Step 1: wait for work ───────────────────────────────
            self.diag_notify.notified().await;

            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

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
            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // ── Step 1: wait for work ───────────────────────────────
            self.phpstan_notify.notified().await;

            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

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

            let bin_dir: Option<String> = crate::composer::read_composer_package(&workspace_root)
                .map(|pkg| crate::composer::get_bin_dir(&pkg));

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
            let shutdown_flag = Arc::clone(&self.shutdown_flag);
            let phpstan_diags = {
                let result = tokio::task::spawn_blocking(move || {
                    phpstan::run_phpstan(
                        &resolved,
                        &content,
                        &file_path,
                        &workspace_root,
                        &phpstan_config,
                        &shutdown_flag,
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
            // Read the file content and verify the file is still open
            // *before* writing to the cache.  If the file was closed
            // while PHPStan was running, `clear_diagnostics_for_file`
            // already purged the cache entry — writing it back would
            // leave stale diagnostics that resurface on the next
            // `did_open`.
            let content = {
                let files = self.open_files.read();
                match files.get(&uri) {
                    Some(c) => c.clone(),
                    None => continue,
                }
            };

            {
                let mut cache = self.phpstan_last_diags.lock();
                cache.insert(uri.clone(), phpstan_diags);
            }

            // Re-deliver diagnostics for this file so the editor sees
            // the fresh PHPStan results merged with native diagnostics.
            self.publish_diagnostics_for_file(&uri, &content).await;
        }
    }

    /// Clear diagnostics for a file (e.g. on `did_close`).
    pub(crate) async fn clear_diagnostics_for_file(&self, uri_str: &str) {
        // Remove cached slow diagnostics so we don't leak memory.
        self.diag_last_slow.lock().remove(uri_str);
        // Remove cached PHPStan diagnostics too.
        self.phpstan_last_diags.lock().remove(uri_str);
        // Remove pull-diagnostic caches.
        self.diag_result_ids.lock().remove(uri_str);
        self.diag_last_full.lock().remove(uri_str);

        let client = match &self.client {
            Some(c) => c,
            None => return,
        };

        let uri = match uri_str.parse::<Url>() {
            Ok(u) => u,
            Err(_) => return,
        };

        // Always push empty diagnostics to clear any Phase 1 snapshot.
        client.publish_diagnostics(uri, Vec::new(), None).await;

        if self.supports_pull_diagnostics.load(Ordering::Acquire) {
            // Also send a refresh so the editor re-pulls (and gets
            // empty results for the now-closed file).
            let _ = client.workspace_diagnostic_refresh().await;
        }
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
/// 4. Any precise (sub-line) diagnostic suppresses full-line diagnostics
///    on the same line (PHPStan only reports line numbers).
///
/// Each source's diagnostics are authoritative: if PHPStan reports five
/// issues on a line, all five are shown; if PHPantom reports two issues
/// on the same span, both are shown.  Cross-source overlap is handled
/// by rule 4 above, not by collapsing identical ranges.
impl Backend {
    /// Remove diagnostics that were eagerly suppressed by a
    /// `codeAction/resolve` handler and drain the suppression list.
    ///
    /// This is called during `publish_diagnostics_for_file` so that
    /// the squiggly line disappears before the text edit is applied.
    fn filter_suppressed(&self, mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
        let mut suppressed = self.diag_suppressed.lock();
        if suppressed.is_empty() {
            return diagnostics;
        }
        diagnostics.retain(|d| {
            !suppressed
                .iter()
                .any(|s| d.range == s.range && d.message == s.message && d.code == s.code)
        });
        suppressed.clear();
        diagnostics
    }
}

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

    // Sort by range for stable output order.
    diagnostics.sort_by(|a, b| {
        a.range
            .start
            .line
            .cmp(&b.range.start.line)
            .then_with(|| a.range.start.character.cmp(&b.range.start.character))
            .then_with(|| a.range.end.line.cmp(&b.range.end.line))
            .then_with(|| a.range.end.character.cmp(&b.range.end.character))
    });
}

/// Returns `true` if the range spans a full line (character 0 to a
/// very large end character).  PHPStan and other line-only tools
/// produce these ranges because they don't report column information.
fn is_full_line_range(range: &Range) -> bool {
    range.start.line == range.end.line && range.start.character == 0 && range.end.character >= 1000
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
    Some(crate::util::offset_to_position(content, byte_offset))
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

    #[test]
    fn keeps_multiple_diagnostics_on_same_range() {
        // Each source is authoritative — two PHPantom diagnostics on
        // the same span are both shown.
        let range = make_range(7, 3, 7, 12);
        let diag1 = make_diagnostic(
            range,
            DiagnosticSeverity::WARNING,
            "unknown_member",
            "Method 'foo' not found on class Bar",
        );
        let diag2 = make_diagnostic(
            range,
            DiagnosticSeverity::HINT,
            "deprecated",
            "Method 'foo' is deprecated",
        );
        let mut diags = vec![diag1, diag2];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 2);
    }

    #[test]
    fn keeps_multiple_phpstan_diagnostics_on_same_line() {
        // If PHPStan reports five issues on a line, all five survive.
        let make_phpstan = |code: &str, msg: &str| Diagnostic {
            range: make_range(10, 0, 10, u32::MAX),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(code.to_string())),
            source: Some("phpstan".to_string()),
            message: msg.to_string(),
            ..Default::default()
        };
        let mut diags = vec![
            make_phpstan("argument.type", "Parameter #1 expects int, string given."),
            make_phpstan("return.type", "Should return int but returns string."),
            make_phpstan("missingType.return", "Method has no return type."),
        ];
        deduplicate_diagnostics(&mut diags);
        assert_eq!(diags.len(), 3);
    }

    // ── is_stale_phpstan_diagnostic ─────────────────────────────────

    /// Helper: build a PHPStan-style full-line diagnostic.
    fn make_phpstan_diag(line: u32, code: &str, message: &str) -> Diagnostic {
        Diagnostic {
            range: make_range(line, 0, line, 200),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(code.to_string())),
            source: Some("PHPStan".to_string()),
            message: message.to_string(),
            ..Default::default()
        }
    }

    // ── Per-identifier heuristics removed ───────────────────────────
    //
    // The throws.unusedType, missingType.checkedException, and
    // method.missingOverride stale-detection branches have been
    // removed.  These diagnostics are now cleared eagerly by
    // `codeAction/resolve` (see `clear_phpstan_diagnostics_after_resolve`).
    // The tests below verify they are no longer considered stale by
    // `is_stale_phpstan_diagnostic` alone.

    #[test]
    fn throws_unused_type_not_stale_via_heuristic() {
        // Previously this was detected as stale because the @throws
        // tag was removed.  Now only codeAction/resolve clears it.
        let content = "<?php\nclass Foo {\n    public function bar(): void {}\n}\n";
        let diag = make_phpstan_diag(
            2,
            "throws.unusedType",
            "Method App\\Foo::bar() has App\\Exceptions\\FooException in PHPDoc @throws tag but it's not thrown.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "throws.unusedType should NOT be stale via heuristic (cleared by resolve instead)"
        );
    }

    #[test]
    fn missing_checked_exception_not_stale_via_heuristic() {
        // Previously this was detected as stale because a @throws
        // tag was added.  Now only codeAction/resolve clears it.
        let content = "<?php\nclass Foo {\n    /**\n     * @throws FooException\n     */\n    public function bar(): void {}\n}\n";
        let diag = make_phpstan_diag(
            5,
            "missingType.checkedException",
            "Method App\\Foo::bar() throws checked exception App\\Exceptions\\FooException but it's missing from the PHPDoc @throws tag.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "missingType.checkedException should NOT be stale via heuristic (cleared by resolve instead)"
        );
    }

    #[test]
    fn stale_when_phpstan_ignore_covers_identifier() {
        let content = "<?php\nclass Foo {\n    public function bar(): void {} // @phpstan-ignore return.type\n}\n";
        let diag = make_phpstan_diag(
            2,
            "return.type",
            "Method App\\Foo::bar() should return string but returns void.",
        );
        assert!(
            is_stale_phpstan_diagnostic(&diag, content),
            "should be stale when @phpstan-ignore lists the identifier"
        );
    }

    #[test]
    fn not_stale_when_phpstan_ignore_covers_different_identifier() {
        let content = "<?php\nclass Foo {\n    public function bar(): void {} // @phpstan-ignore argument.type\n}\n";
        let diag = make_phpstan_diag(
            2,
            "return.type",
            "Method App\\Foo::bar() should return string but returns void.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "should NOT be stale when @phpstan-ignore lists a different identifier"
        );
    }

    #[test]
    fn not_stale_for_phpstan_ignore_line_blanket() {
        // @phpstan-ignore-line suppresses everything, but we don't
        // eagerly prune for it — only per-identifier ignores count.
        let content =
            "<?php\nclass Foo {\n    public function bar(): void {} // @phpstan-ignore-line\n}\n";
        let diag = make_phpstan_diag(
            2,
            "return.type",
            "Method App\\Foo::bar() should return string but returns void.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "should NOT be stale for blanket @phpstan-ignore-line"
        );
    }

    // ── method.missingOverride stale detection ──────────────────────

    // ── method.missingOverride — heuristic removed ──────────────────

    #[test]
    fn missing_override_not_stale_via_heuristic() {
        // Previously this was detected as stale because #[Override]
        // was found above the method.  Now only codeAction/resolve
        // clears it.
        let content = "<?php\nclass Foo extends Bar {\n    #[\\Override]\n    public function baz(): void {}\n}\n";
        let diag = make_phpstan_diag(
            3,
            "method.missingOverride",
            "Method Foo::baz() overrides method Bar::baz() but is missing the #[\\Override] attribute.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "method.missingOverride should NOT be stale via heuristic (cleared by resolve instead)"
        );
    }

    #[test]
    fn not_stale_for_phpstan_ignore_next_line_blanket() {
        let content = "<?php\nclass Foo {\n    // @phpstan-ignore-next-line\n    public function bar(): void {}\n}\n";
        let diag = make_phpstan_diag(
            3,
            "return.type",
            "Method App\\Foo::bar() should return string but returns void.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "should NOT be stale for blanket @phpstan-ignore-next-line"
        );
    }

    #[test]
    fn stale_when_phpstan_ignore_on_previous_line() {
        let content = "<?php\nclass Foo {\n    // @phpstan-ignore return.type\n    public function bar(): void {}\n}\n";
        let diag = make_phpstan_diag(
            3,
            "return.type",
            "Method App\\Foo::bar() should return string but returns void.",
        );
        assert!(
            is_stale_phpstan_diagnostic(&diag, content),
            "should be stale when @phpstan-ignore on previous line lists the identifier"
        );
    }

    #[test]
    fn stale_phpstan_ignore_with_multiple_ids() {
        let content = "<?php\nclass Foo {\n    public function bar(): void {} // @phpstan-ignore return.type, argument.type\n}\n";
        let return_diag = make_phpstan_diag(
            2,
            "return.type",
            "Method App\\Foo::bar() should return string but returns void.",
        );
        let arg_diag = make_phpstan_diag(
            2,
            "argument.type",
            "Parameter #1 $x expects string, int given.",
        );
        let other_diag = make_phpstan_diag(2, "method.notFound", "Call to undefined method.");
        assert!(
            is_stale_phpstan_diagnostic(&return_diag, content),
            "return.type should be stale (listed in ignore)"
        );
        assert!(
            is_stale_phpstan_diagnostic(&arg_diag, content),
            "argument.type should be stale (listed in ignore)"
        );
        assert!(
            !is_stale_phpstan_diagnostic(&other_diag, content),
            "method.notFound should NOT be stale (not listed)"
        );
    }

    #[test]
    fn diag_with_no_code_is_never_stale() {
        let content = "<?php\n// @phpstan-ignore return.type\nfoo();";
        let diag = Diagnostic {
            range: make_range(1, 0, 1, 200),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            source: Some("PHPStan".to_string()),
            message: "Some error.".to_string(),
            ..Default::default()
        };
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "diagnostic without a code should never be considered stale"
        );
    }

    #[test]
    fn ignore_unmatched_diag_is_never_stale_via_ignore_check() {
        // ignore.unmatched diagnostics should not be pruned by the
        // @phpstan-ignore check (they ARE the ignore comment).
        let content = "<?php\n$x = 1; // @phpstan-ignore ignore.unmatchedIdentifier\n";
        let diag = make_phpstan_diag(
            1,
            "ignore.unmatchedIdentifier",
            "No error with identifier foo is reported on line 2.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "ignore.unmatched* diagnostics must not be pruned by the ignore check"
        );
    }

    // ── B16: scoped docblock checks ─────────────────────────────────
    //
    // The scoped docblock heuristics have been removed alongside the
    // per-identifier stale detection.  These tests verify the new
    // behaviour: throws/override diagnostics are never stale via
    // heuristic (they are cleared by codeAction/resolve instead).

    #[test]
    fn throws_not_stale_even_when_tag_on_same_function() {
        // Previously this was stale because @throws FooException was
        // found on baz()'s own docblock.  Now it's not — resolve
        // handles clearing.
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    public function bar(): void {}\n",
            "    /**\n",
            "     * @throws FooException\n",
            "     */\n",
            "    public function baz(): void {\n",
            "        throw new FooException();\n",
            "    }\n",
            "}\n",
        );
        let diag = make_phpstan_diag(
            7,
            "missingType.checkedException",
            "Method App\\Foo::baz() throws checked exception App\\Exceptions\\FooException but it's missing from the PHPDoc @throws tag.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&diag, content),
            "missingType.checkedException should NOT be stale via heuristic"
        );
    }

    #[test]
    fn unused_throws_not_stale_via_heuristic_even_when_tag_removed() {
        // Previously baz()'s diagnostic was stale because the tag was
        // removed.  Now neither is stale via heuristic.
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @throws FooException\n",
            "     */\n",
            "    public function bar(): void {\n",
            "    }\n",
            "    public function baz(): void {\n",
            "    }\n",
            "}\n",
        );
        let bar_diag = make_phpstan_diag(
            5,
            "throws.unusedType",
            "Method App\\Foo::bar() has App\\Exceptions\\FooException in PHPDoc @throws tag but it's not thrown.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&bar_diag, content),
            "bar()'s throws.unusedType should NOT be stale via heuristic"
        );

        let baz_diag = make_phpstan_diag(
            7,
            "throws.unusedType",
            "Method App\\Foo::baz() has App\\Exceptions\\FooException in PHPDoc @throws tag but it's not thrown.",
        );
        assert!(
            !is_stale_phpstan_diagnostic(&baz_diag, content),
            "baz()'s throws.unusedType should NOT be stale via heuristic"
        );
    }

    #[test]
    fn enclosing_docblock_text_finds_correct_docblock() {
        let content = concat!(
            "<?php\nclass Foo {\n",
            "    /**\n",
            "     * @throws BarException\n",
            "     */\n",
            "    public function bar(): void {\n",
            "        // line 6\n",
            "    }\n",
            "    /**\n",
            "     * @throws BazException\n",
            "     */\n",
            "    public function baz(): void {\n",
            "        // line 12\n",
            "    }\n",
            "}\n",
        );
        let bar_doc = enclosing_docblock_text(content, 6);
        assert!(
            bar_doc.contains("BarException"),
            "bar()'s docblock should mention BarException, got: {}",
            bar_doc
        );
        assert!(
            !bar_doc.contains("BazException"),
            "bar()'s docblock should NOT mention BazException, got: {}",
            bar_doc
        );

        let baz_doc = enclosing_docblock_text(content, 12);
        assert!(
            baz_doc.contains("BazException"),
            "baz()'s docblock should mention BazException, got: {}",
            baz_doc
        );
        assert!(
            !baz_doc.contains("BarException"),
            "baz()'s docblock should NOT mention BarException, got: {}",
            baz_doc
        );
    }

    #[test]
    fn enclosing_docblock_text_returns_empty_when_no_docblock() {
        let content = "<?php\nfunction foo(): void {\n    // line 2\n}\n";
        let doc = enclosing_docblock_text(content, 2);
        assert!(
            doc.is_empty(),
            "should return empty when no docblock exists, got: {}",
            doc
        );
    }
}
