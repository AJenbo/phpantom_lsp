//! Eager resolved-class population for the LSP server.
//!
//! Mirrors the CLI analyse pipeline: after the workspace is fully
//! indexed, every known class is resolved in dependency-first order so
//! that interactive requests (completion, hover, diagnostics,
//! go-to-definition) read pre-resolved metadata from the cache instead
//! of recursing into class resolution.  Incremental re-population on
//! file change is handled separately in `parser/ast_update.rs`, which
//! re-resolves only the classes evicted by the edit.

use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::Backend;

impl Backend {
    /// Wait until `initialized` has finished, returning `false` when
    /// the server shuts down first.
    ///
    /// Startup tasks that populate resolution caches must wait for
    /// this: `initialized` clears those caches after the startup scan,
    /// which would discard any population that ran earlier.
    pub(crate) async fn wait_for_init_complete(&self) -> bool {
        while !self.init_complete.load(Ordering::Acquire) {
            if self.shutdown_flag.load(Ordering::Acquire) {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        true
    }

    /// Resolve every class in `uri_classes_index` in topological
    /// (dependency-first) order, populating `resolved_class_cache`.
    ///
    /// Skips classes that are already cached, so calling this again
    /// (e.g. from the workspace diagnostics pass after the full-index
    /// task already populated) is cheap.
    pub(crate) async fn eager_populate_resolved_classes(&self) {
        let backend = self.clone_for_blocking();
        crate::server::run_blocking_cancel_safe(move || {
            let sorted_fqns = {
                let uri_classes_index = backend.symbols.uri_classes_index.read();
                crate::toposort::toposort_from_uri_classes_index(&uri_classes_index)
            };
            // Dedicated large-stack thread: class resolution walks
            // parsed ASTs and can nest when the toposort misses
            // dependencies (stubs, on-demand vendor loads).
            std::thread::scope(|s| {
                let backend = &backend;
                let sorted_fqns = &sorted_fqns;
                std::thread::Builder::new()
                    .name("eager-populate".into())
                    .stack_size(crate::PARSE_WORKER_STACK_SIZE)
                    .spawn_scoped(s, move || {
                        let class_loader = |name: &str| backend.find_or_load_class(name);
                        crate::virtual_members::populate_from_sorted(
                            sorted_fqns,
                            &backend.resolved_class_cache,
                            &class_loader,
                        );
                    })
                    .expect("failed to spawn eager-populate thread");
            });
        })
        .await;
    }
}
