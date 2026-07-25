//! Mago lint and Mago analyze proxy diagnostics: schedule functions and
//! background workers.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::mago;

impl Backend {
    // ── Mago lint worker ────────────────────────────────────────────

    /// Schedule a Mago lint run for a single file.
    ///
    /// Only the most recent file is kept: if the user switches files or
    /// types rapidly, earlier requests are superseded.
    pub(crate) fn schedule_mago_lint(&self, uri: String) {
        *self.mago_lint_tool.pending_uri.lock() = Some(uri);
        self.mago_lint_tool.notify.notify_one();
    }

    /// Long-lived background task that runs `mago lint` on pending files.
    ///
    /// Spawned once during `initialized`. This task is completely
    /// independent: native diagnostics, PHPStan, PHPCS, and Mago
    /// analyze are never blocked.
    ///
    /// At most one `mago lint` process runs at a time. The worker
    /// loop follows the same pattern as the PHPCS worker.
    pub(crate) async fn mago_lint_worker(&self) {
        loop {
            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // ── Step 1: wait for work ───────────────────────────────
            self.mago_lint_tool.notify.notified().await;

            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // Drain any extra stored permits.
            let _ = tokio::time::timeout(
                std::time::Duration::ZERO,
                self.mago_lint_tool.notify.notified(),
            )
            .await;

            // ── Step 2: snapshot the pending URI ────────────────────
            let uri = match self.mago_lint_tool.pending_uri.lock().take() {
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

            // ── Step 4: resolve Mago binary ─────────────────────────
            let config = self.config();
            if config.mago.is_disabled() {
                continue;
            }

            let workspace_root = self.workspace.workspace_root.read().clone();
            let workspace_root = match workspace_root {
                Some(root) => root,
                None => continue,
            };

            // Mago requires mago.toml to operate.
            if !mago::has_mago_config(&workspace_root) {
                continue;
            }

            let file_path = match uri.parse::<Url>().ok().and_then(|u| u.to_file_path().ok()) {
                Some(p) => p,
                None => continue,
            };

            let bin_dir: Option<String> = crate::composer::read_composer_package(&workspace_root)
                .map(|pkg| crate::composer::get_bin_dir(&pkg));

            let resolved =
                match mago::resolve_mago(Some(&workspace_root), &config.mago, bin_dir.as_deref()) {
                    Some(r) => r,
                    None => continue,
                };

            // ── Step 5: run mago lint (the slow part) ───────────────
            let mago_config = config.mago.clone();
            let shutdown_flag = Arc::clone(&self.shutdown_flag);
            let mago_diags = {
                let result = tokio::task::spawn_blocking(move || {
                    mago::run_mago_lint(
                        &resolved,
                        &content,
                        &file_path,
                        &workspace_root,
                        &mago_config,
                        &shutdown_flag,
                    )
                })
                .await;

                match result {
                    Ok(Ok(diags)) => diags,
                    Ok(Err(_e)) => continue,
                    Err(_join_err) => continue,
                }
            };

            // ── Step 6: cache results and re-publish ────────────────
            {
                let files = self.open_files.read();
                if !files.contains_key(&uri) {
                    continue;
                }
            }

            {
                let mut cache = self.mago_lint_tool.last_diags.lock();
                cache.insert(uri.clone(), mago_diags);
            }

            self.assemble_and_push(&uri).await;

            // In pull mode the editor must be told to re-pull so it
            // picks up the new Mago lint results.
            if self.supports_pull_diagnostics.load(Ordering::Acquire)
                && let Some(client) = &self.client
            {
                let _ = client.workspace_diagnostic_refresh().await;
            }
        }
    }

    // ── Mago analyze worker ─────────────────────────────────────────

    /// Schedule a Mago analyze run for a single file.
    ///
    /// Only the most recent file is kept: if the user switches files or
    /// types rapidly, earlier requests are superseded.
    pub(crate) fn schedule_mago_analyze(&self, uri: String) {
        *self.mago_analyze_tool.pending_uri.lock() = Some(uri);
        self.mago_analyze_tool.notify.notify_one();
    }

    /// Long-lived background task that runs `mago analyze` on pending files.
    ///
    /// Spawned once during `initialized`. This task is completely
    /// independent: native diagnostics, PHPStan, PHPCS, and Mago lint
    /// are never blocked.
    ///
    /// At most one `mago analyze` process runs at a time. The worker
    /// loop follows the same pattern as the PHPStan worker.
    pub(crate) async fn mago_analyze_worker(&self) {
        loop {
            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // ── Step 1: wait for work ───────────────────────────────
            self.mago_analyze_tool.notify.notified().await;

            if self.shutdown_flag.load(Ordering::Acquire) {
                return;
            }

            // Drain any extra stored permits.
            let _ = tokio::time::timeout(
                std::time::Duration::ZERO,
                self.mago_analyze_tool.notify.notified(),
            )
            .await;

            // ── Step 2: snapshot the pending URI ────────────────────
            let uri = match self.mago_analyze_tool.pending_uri.lock().take() {
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

            // ── Step 4: resolve Mago binary ─────────────────────────
            let config = self.config();
            if config.mago.is_disabled() {
                continue;
            }

            let workspace_root = self.workspace.workspace_root.read().clone();
            let workspace_root = match workspace_root {
                Some(root) => root,
                None => continue,
            };

            // Mago requires mago.toml to operate.
            if !mago::has_mago_config(&workspace_root) {
                continue;
            }

            let file_path = match uri.parse::<Url>().ok().and_then(|u| u.to_file_path().ok()) {
                Some(p) => p,
                None => continue,
            };

            let bin_dir: Option<String> = crate::composer::read_composer_package(&workspace_root)
                .map(|pkg| crate::composer::get_bin_dir(&pkg));

            let resolved =
                match mago::resolve_mago(Some(&workspace_root), &config.mago, bin_dir.as_deref()) {
                    Some(r) => r,
                    None => continue,
                };

            // ── Step 5: run mago analyze (the slow part) ────────────
            let mago_config = config.mago.clone();
            let shutdown_flag = Arc::clone(&self.shutdown_flag);
            let mago_diags = {
                let result = tokio::task::spawn_blocking(move || {
                    mago::run_mago_analyze(
                        &resolved,
                        &content,
                        &file_path,
                        &workspace_root,
                        &mago_config,
                        &shutdown_flag,
                    )
                })
                .await;

                match result {
                    Ok(Ok(diags)) => diags,
                    Ok(Err(_e)) => continue,
                    Err(_join_err) => continue,
                }
            };

            // ── Step 6: cache results and re-publish ────────────────
            {
                let files = self.open_files.read();
                if !files.contains_key(&uri) {
                    continue;
                }
            }

            {
                let mut cache = self.mago_analyze_tool.last_diags.lock();
                cache.insert(uri.clone(), mago_diags);
            }

            self.assemble_and_push(&uri).await;

            // In pull mode the editor must be told to re-pull so it
            // picks up the new Mago analyze results.
            if self.supports_pull_diagnostics.load(Ordering::Acquire)
                && let Some(client) = &self.client
            {
                let _ = client.workspace_diagnostic_refresh().await;
            }
        }
    }
}
