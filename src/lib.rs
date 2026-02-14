//! PHPantomLSP — a lightweight PHP language server.
//!
//! This crate is organised into the following modules:
//!
//! - [`types`]: Data structures for extracted PHP information (classes, methods, functions, etc.)
//! - [`parser`]: PHP parsing and AST extraction using mago_syntax
//! - [`completion`]: Completion logic (target extraction, type resolution, item building)
//! - [`composer`]: Composer autoload (PSR-4) parsing and class-to-file resolution
//! - [`server`]: The LSP `LanguageServer` trait implementation
//! - [`util`]: Utility helpers (position conversion, class lookup, logging)
//! - [`definition`]: Go-to-definition support for classes, members, and functions

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tower_lsp::Client;

// ─── Module declarations ────────────────────────────────────────────────────

mod completion;
pub mod composer;
mod definition;
mod parser;
mod server;
pub mod types;
mod util;

// ─── Re-exports ─────────────────────────────────────────────────────────────

// Re-export public types so that dependents (tests, main) can import them
// from the crate root, e.g. `use phpantom_lsp::{Backend, AccessKind}`.
pub use types::{
    AccessKind, ClassInfo, CompletionTarget, ConstantInfo, FunctionInfo, MethodInfo, ParameterInfo,
    PropertyInfo, Visibility,
};

// ─── Backend ────────────────────────────────────────────────────────────────

/// The main LSP backend that holds all server state.
///
/// Method implementations are spread across several modules:
/// - [`parser`]: `parse_php`, `update_ast`, AST extraction helpers
/// - [`completion::target`]: `extract_completion_target`, `detect_access_kind`
/// - [`completion::resolver`]: `resolve_target_class` and type-resolution helpers
/// - [`completion::builder`]: `build_completion_items`, `build_method_label`
/// - [`composer`]: PSR-4 autoload mapping and class file resolution
/// - [`server`]: `impl LanguageServer` (initialize, completion, did_open, …)
/// - [`util`]: `position_to_offset`, `find_class_at_offset`, `log`, `get_classes_for_uri`
/// - [`definition`]: `resolve_definition`, member resolution, function resolution
pub struct Backend {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) open_files: Arc<Mutex<HashMap<String, String>>>,
    /// Maps a file URI to a list of ClassInfo extracted from that file.
    pub(crate) ast_map: Arc<Mutex<HashMap<String, Vec<ClassInfo>>>>,
    pub(crate) client: Option<Client>,
    /// The root directory of the workspace (set during `initialize`).
    pub workspace_root: Arc<Mutex<Option<PathBuf>>>,
    /// PSR-4 autoload mappings parsed from `composer.json`.
    pub(crate) psr4_mappings: Arc<Mutex<Vec<composer::Psr4Mapping>>>,
    /// Maps a file URI to its `use` statement mappings (short name → fully qualified name).
    /// For example, `use Klarna\Rest\Resource;` produces `"Resource" → "Klarna\Rest\Resource"`.
    pub(crate) use_map: Arc<Mutex<HashMap<String, HashMap<String, String>>>>,
    /// Maps a file URI to its declared namespace (e.g. `"Klarna\Rest\Checkout"`).
    /// Files without a namespace declaration map to `None`.
    pub(crate) namespace_map: Arc<Mutex<HashMap<String, Option<String>>>>,
    /// Global function definitions indexed by function name (short name).
    ///
    /// The value is `(file_uri, FunctionInfo)` so we can jump to the definition.
    /// Populated from files listed in Composer's `autoload_files.php` at init
    /// time, and also from any opened/changed files that contain standalone
    /// function declarations.
    pub global_functions: Arc<Mutex<HashMap<String, (String, FunctionInfo)>>>,
    /// Index of fully-qualified class names to file URIs.
    ///
    /// This allows reliable lookup of classes that don't follow PSR-4
    /// conventions — e.g. classes defined in files listed by Composer's
    /// `autoload_files.php`.  The key is the FQN (e.g.
    /// `"Laravel\\Foundation\\Application"`) and the value is the file URI
    /// where the class is defined.
    ///
    /// Populated during `update_ast` (using the file's namespace + class
    /// short name) and during server initialization for autoload files.
    pub class_index: Arc<Mutex<HashMap<String, String>>>,
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
            workspace_root: Arc::new(Mutex::new(None)),
            psr4_mappings: Arc::new(Mutex::new(Vec::new())),
            use_map: Arc::new(Mutex::new(HashMap::new())),
            namespace_map: Arc::new(Mutex::new(HashMap::new())),
            global_functions: Arc::new(Mutex::new(HashMap::new())),
            class_index: Arc::new(Mutex::new(HashMap::new())),
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
            workspace_root: Arc::new(Mutex::new(None)),
            psr4_mappings: Arc::new(Mutex::new(Vec::new())),
            use_map: Arc::new(Mutex::new(HashMap::new())),
            namespace_map: Arc::new(Mutex::new(HashMap::new())),
            global_functions: Arc::new(Mutex::new(HashMap::new())),
            class_index: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create a `Backend` for tests with a specific workspace root and PSR-4
    /// mappings pre-configured.
    pub fn new_test_with_workspace(
        workspace_root: PathBuf,
        psr4_mappings: Vec<composer::Psr4Mapping>,
    ) -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.1.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: None,
            workspace_root: Arc::new(Mutex::new(Some(workspace_root))),
            psr4_mappings: Arc::new(Mutex::new(psr4_mappings)),
            use_map: Arc::new(Mutex::new(HashMap::new())),
            namespace_map: Arc::new(Mutex::new(HashMap::new())),
            global_functions: Arc::new(Mutex::new(HashMap::new())),
            class_index: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
