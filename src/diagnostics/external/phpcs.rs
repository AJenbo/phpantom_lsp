//! PHPCS proxy diagnostics: schedule function and background worker.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::phpcs;

impl Backend {
    // ── PHPCS worker ────────────────────────────────────────────────

    /// Schedule a PHPCS run for a single file.
    ///
    /// Only the most recent file is kept: if the user switches files or
    /// types rapidly, earlier requests are superseded. This is
    /// intentional — PHPCS is too slow to queue up multiple files.
    pub(crate) fn schedule_phpcs(&self, uri: String) {
        *self.phpcs_tool.pending_uri.lock() = Some(uri);
        self.phpcs_tool.notify.notify_one();
    }

    /// Long-lived background task that runs PHPCS on pending files.
    ///
    /// Spawned once during `initialized`, alongside the main diagnostic
    /// worker and the PHPStan worker. This task is completely
    /// independent: native diagnostics and PHPStan are never blocked.
    ///
    /// ## Serialization guarantee
    ///
    /// At most one PHPCS process runs at a time. The worker loop:
    ///
    /// 1. Wait for a notification (new edit arrived).
    /// 2. Debounce: sleep [`PHPCS_DEBOUNCE_MS`], checking whether new
    ///    edits arrived. If so, restart the debounce.
    /// 3. Snapshot the pending URI and file content.
    /// 4. Resolve the PHPCS binary (skip if not found / disabled).
    /// 5. Run PHPCS (blocking — this is the slow part).
    /// 6. Cache the results and re-publish diagnostics for the file.
    /// 7. Loop back to step 1.
    ///
    /// If the user edits while step 5 is in progress, the pending URI
    /// is updated. When step 5 finishes, the worker sees the new
    /// notification and loops back to step 1, starting a fresh run
    /// with the latest content.
    pub(crate) async fn phpcs_worker(&self) {
        loop {
            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // ── Step 1: wait for work ───────────────────────────────
            self.phpcs_tool.notify.notified().await;

            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // Drain any extra stored permits (same rationale as the
            // PHPStan worker).
            let _ =
                tokio::time::timeout(std::time::Duration::ZERO, self.phpcs_tool.notify.notified())
                    .await;

            // ── Step 2: snapshot the pending URI ────────────────────
            let uri = match self.phpcs_tool.pending_uri.lock().take() {
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

            // ── Step 4: resolve PHPCS binary ────────────────────────
            let config = self.config();
            if config.phpcs.is_disabled() {
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

            let resolved = match phpcs::resolve_phpcs(
                Some(&workspace_root),
                &config.phpcs,
                bin_dir.as_deref(),
            ) {
                Some(r) => r,
                None => continue,
            };

            // ── Step 5: run PHPCS (the slow part) ───────────────────
            let phpcs_config = config.phpcs.clone();
            let shutdown_flag = Arc::clone(&self.shutdown_flag);
            let phpcs_diags = {
                let result = tokio::task::spawn_blocking(move || {
                    phpcs::run_phpcs(
                        &resolved,
                        &content,
                        &file_path,
                        &workspace_root,
                        &phpcs_config,
                        &shutdown_flag,
                    )
                })
                .await;

                match result {
                    Ok(Ok(diags)) => diags,
                    Ok(Err(_e)) => {
                        // PHPCS failures are silently ignored to
                        // avoid flooding the editor with errors when
                        // PHPCS is misconfigured or the project
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
            // Verify the file is still open before caching (same
            // rationale as the PHPStan worker).
            {
                let files = self.open_files.read();
                if !files.contains_key(&uri) {
                    continue;
                }
            }

            {
                let mut cache = self.phpcs_tool.last_diags.lock();
                cache.insert(uri.clone(), phpcs_diags);
            }

            // Assemble and push so the editor sees fresh PHPCS
            // results merged with cached native diagnostics.
            self.assemble_and_push(&uri).await;

            // In pull mode the editor must be told to re-pull so it
            // picks up the new PHPCS results.
            if self.supports_pull_diagnostics.load(Ordering::Acquire)
                && let Some(client) = &self.client
            {
                let _ = client.workspace_diagnostic_refresh().await;
            }
        }
    }
}
