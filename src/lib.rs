//! PHPantomLSP — a lightweight PHP language server.
//!
//! This crate is organised into the following modules:
//!
//! - [`types`]: Data structures for extracted PHP information (classes, methods, etc.)
//! - [`parser`]: PHP parsing and AST extraction using mago_syntax
//! - [`completion`]: Completion logic (target extraction, type resolution, item building)
//! - [`server`]: The LSP `LanguageServer` trait implementation
//! - [`util`]: Utility helpers (position conversion, class lookup, logging)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tower_lsp::Client;

// ─── Module declarations ────────────────────────────────────────────────────

mod completion;
mod parser;
mod server;
pub mod types;
mod util;

// ─── Re-exports ─────────────────────────────────────────────────────────────

// Re-export public types so that dependents (tests, main) can import them
// from the crate root, e.g. `use phpantom_lsp::{Backend, AccessKind}`.
pub use types::{
    AccessKind, ClassInfo, CompletionTarget, ConstantInfo, MethodInfo, ParameterInfo, PropertyInfo,
};

// ─── Backend ────────────────────────────────────────────────────────────────

/// The main LSP backend that holds all server state.
///
/// Method implementations are spread across several modules:
/// - [`parser`]: `parse_php`, `update_ast`, AST extraction helpers
/// - [`completion::target`]: `extract_completion_target`, `detect_access_kind`
/// - [`completion::resolver`]: `resolve_target_class` and type-resolution helpers
/// - [`completion::builder`]: `build_completion_items`, `build_method_label`
/// - [`server`]: `impl LanguageServer` (initialize, completion, did_open, …)
/// - [`util`]: `position_to_offset`, `find_class_at_offset`, `log`, `get_classes_for_uri`
pub struct Backend {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) open_files: Arc<Mutex<HashMap<String, String>>>,
    /// Maps a file URI to a list of ClassInfo extracted from that file.
    pub(crate) ast_map: Arc<Mutex<HashMap<String, Vec<ClassInfo>>>>,
    pub(crate) client: Option<Client>,
}

impl Backend {
    /// Create a new `Backend` connected to an LSP client.
    pub fn new(client: Client) -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: Some(client),
        }
    }

    /// Create a `Backend` without an LSP client (for unit / integration tests).
    pub fn new_test() -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: None,
        }
    }
}
