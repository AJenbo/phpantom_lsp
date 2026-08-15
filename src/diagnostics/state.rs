//! Diagnostic scheduling and pull-model cache state.
//!
//! Grouped out of `Backend` so the diagnostic subsystem's fields live
//! together. All fields are `Arc`-wrapped, so `#[derive(Clone)]` shares
//! them with a cloned `Backend` (the same semantics the per-request clone
//! had when these were individual `Backend` fields).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

use parking_lot::Mutex;
use tokio::sync::Notify;
use tower_lsp::lsp_types::Diagnostic;

use crate::diagnostics::workspace::WorkspaceDiagnostics;

/// Diagnostic debouncing state and the pull-model diagnostic caches.
#[derive(Clone)]
pub(crate) struct DiagnosticState {
    /// Monotonically increasing version counter for diagnostic debouncing.
    pub(crate) version: Arc<AtomicU64>,
    /// Notification handle used to wake the diagnostic worker task.
    pub(crate) notify: Arc<Notify>,
    /// File URIs that need a diagnostic pass, drained by the worker.
    pub(crate) pending_uris: Arc<Mutex<HashSet<String>>>,
    /// Last-published slow diagnostics per file URI.
    pub(crate) last_slow: Arc<Mutex<HashMap<String, Vec<Diagnostic>>>>,
    /// Last-computed fast diagnostics per file URI.
    pub(crate) last_fast: Arc<Mutex<HashMap<String, Vec<Diagnostic>>>>,
    /// Per-file `resultId` for pull diagnostics.
    pub(crate) result_ids: Arc<Mutex<HashMap<String, u64>>>,
    /// Combined diagnostic cache (fast + slow + external tools) per file URI.
    pub(crate) last_full: Arc<Mutex<HashMap<String, Vec<Diagnostic>>>>,
    /// Diagnostics to suppress from the next publish cycle.
    pub(crate) suppressed: Arc<Mutex<Vec<Diagnostic>>>,
    /// Diagnostics for files not open in the editor (background pass).
    pub(crate) workspace_diags: Arc<Mutex<WorkspaceDiagnostics>>,
    /// Prevents duplicate background workspace diagnostics passes.
    pub(crate) workspace_diag_pass_started: Arc<AtomicBool>,
    /// Whether the client has sent at least one `workspace/diagnostic`
    /// pull.  In pull mode the background workspace pass waits for this:
    /// its results are only deliverable through workspace pull responses,
    /// so computing them for a client that never asks is wasted work.
    pub(crate) workspace_pull_seen: Arc<AtomicBool>,
    /// Wakes the deferred background workspace pass when the first
    /// `workspace/diagnostic` pull arrives.
    pub(crate) workspace_pull_notify: Arc<Notify>,
    /// What each open file declared when it was last opened or saved, used
    /// to work out which other open files a save can affect.  See
    /// [`crate::diagnostics::cross_file`].
    pub(crate) decl_baselines: Arc<Mutex<crate::diagnostics::cross_file::DeclarationBaselines>>,
}

impl DiagnosticState {
    pub(crate) fn new() -> Self {
        Self {
            version: Arc::new(AtomicU64::new(0)),
            notify: Arc::new(Notify::new()),
            pending_uris: Arc::new(Mutex::new(HashSet::new())),
            last_slow: Arc::new(Mutex::new(HashMap::new())),
            last_fast: Arc::new(Mutex::new(HashMap::new())),
            result_ids: Arc::new(Mutex::new(HashMap::new())),
            last_full: Arc::new(Mutex::new(HashMap::new())),
            suppressed: Arc::new(Mutex::new(Vec::new())),
            workspace_diags: Arc::new(Mutex::new(WorkspaceDiagnostics::default())),
            workspace_diag_pass_started: Arc::new(AtomicBool::new(false)),
            workspace_pull_seen: Arc::new(AtomicBool::new(false)),
            workspace_pull_notify: Arc::new(Notify::new()),
            decl_baselines: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
