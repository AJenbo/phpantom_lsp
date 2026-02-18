//! PHPantomLSP — a lightweight PHP language server.
//!
//! This crate is organised into the following modules:
//!
//! - [`types`]: Data structures for extracted PHP information (classes, methods, functions, etc.)
//! - [`parser`]: PHP parsing and AST extraction using mago_syntax
//! - [`completion`]: Completion logic (target extraction, type resolution, item building,
//!   and the top-level completion request handler)
//! - [`composer`]: Composer autoload (PSR-4, classmap) parsing and class-to-file resolution
//! - [`server`]: The LSP `LanguageServer` trait implementation (thin wrapper that delegates
//!   to feature-specific modules)
//! - [`util`]: Utility helpers (position conversion, class lookup, logging)
//! - [`definition`]: Go-to-definition support for classes, members, and functions
//! - [`inheritance`]: Class inheritance resolution — merging members from parent
//!   classes, traits, and `@mixin` classes into a unified `ClassInfo`
//! - [`resolution`]: Class and function lookup / name resolution (multi-phase:
//!   class_index → ast_map → classmap → PSR-4 → stubs)
//! - [`subject_extraction`]: Shared helpers for extracting the left-hand side of
//!   `->`, `?->`, and `::` access operators (used by both completion and definition)
//! - [`docblock`]: PHPDoc block parsing, split into submodules:
//!   - `docblock::tags` — tag extraction (`@return`, `@var`, `@property`, `@method`,
//!     `@mixin`, `@deprecated`, `@phpstan-assert`, docblock text retrieval)
//!   - `docblock::conditional` — PHPStan conditional return type parsing
//!   - `docblock::types` — type cleaning utilities (`clean_type`, `strip_nullable`,
//!     `is_scalar`, `split_type_token`) and PHPStan array shape parsing
//!     (`parse_array_shape`, `extract_array_shape_value_type`)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tower_lsp::Client;

// ─── Module declarations ────────────────────────────────────────────────────

pub mod completion;
pub mod composer;
mod definition;
pub mod docblock;
pub(crate) mod inheritance;
mod parser;
mod resolution;
mod server;
pub mod stubs;
pub(crate) mod subject_extraction;
pub mod types;
mod util;

// ─── Re-exports ─────────────────────────────────────────────────────────────

// Re-export public types so that dependents (tests, main) can import them
// from the crate root, e.g. `use phpantom_lsp::{Backend, AccessKind}`.
pub use types::{
    AccessKind, ArrayShapeEntry, AssertionKind, ClassInfo, CompletionTarget, ConstantInfo,
    FunctionInfo, MethodInfo, ParameterInfo, PropertyInfo, TypeAssertion, Visibility,
};

// ─── Backend ────────────────────────────────────────────────────────────────

/// The main LSP backend that holds all server state.
///
/// Method implementations are spread across several modules:
/// - [`parser`]: `parse_php`, `update_ast`, AST extraction helpers
/// - [`completion::handler`]: Top-level completion request orchestration
/// - [`completion::target`]: `extract_completion_target`, `detect_access_kind`
/// - [`completion::resolver`]: `resolve_target_class` and type-resolution helpers
/// - [`completion::builder`]: `build_completion_items`, `build_method_label`
/// - [`composer`]: PSR-4 autoload mapping and class file resolution
/// - [`server`]: `impl LanguageServer` (initialize, completion, did_open, …)
/// - [`resolution`]: `find_or_load_class`, `find_or_load_function`, `resolve_class_name`,
///   `resolve_function_name`
/// - [`inheritance`]: `resolve_class_with_inheritance`, trait/mixin/parent merging
/// - [`subject_extraction`]: Shared subject extraction helpers for `->`, `?->`, `::` operators
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
    /// Global constants defined via `define('NAME', value)` calls.
    ///
    /// Maps constant name → file URI where it was defined.
    /// Populated from files listed in Composer's `autoload_files.php` at init
    /// time, and also from any opened/changed files that contain `define()`
    /// calls.  Used to offer constant name completions alongside class names.
    pub global_defines: Arc<Mutex<HashMap<String, String>>>,
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
    /// Composer classmap: fully-qualified class name → file path on disk.
    ///
    /// Parsed from `<vendor>/composer/autoload_classmap.php` during server
    /// initialization.  This provides a direct FQN-to-file lookup that
    /// covers classes not discoverable via PSR-4 — and when the user runs
    /// `composer install -o`, Composer converts *all* PSR-0/PSR-4
    /// mappings into a classmap, giving complete class coverage.
    ///
    /// Consulted by `find_or_load_class` as a resolution step between
    /// the ast_map scan (Phase 1) and PSR-4 resolution (Phase 2).
    pub classmap: Arc<Mutex<HashMap<String, PathBuf>>>,
    /// Embedded PHP stubs for built-in classes/interfaces (e.g. `UnitEnum`,
    /// `BackedEnum`, `Iterator`, `Countable`, …).
    /// Maps class short name → raw PHP source code.
    ///
    /// Built once during construction via [`stubs::build_stub_class_index`].
    /// Consulted by `find_or_load_class` as a final fallback after the
    /// `ast_map` and PSR-4 resolution.  Stub files are parsed lazily on
    /// first access and cached in `ast_map` under `phpantom-stub://` URIs.
    pub(crate) stub_index: HashMap<&'static str, &'static str>,
    /// Embedded PHP stubs for built-in functions (e.g. `array_map`,
    /// `str_contains`, …).  Maps function name → raw PHP source code.
    ///
    /// Built once during construction via [`stubs::build_stub_function_index`].
    /// Can be consulted to resolve return types of built-in function calls.
    pub(crate) stub_function_index: HashMap<&'static str, &'static str>,
    /// Embedded PHP stubs for built-in constants (e.g. `PHP_EOL`,
    /// `SORT_ASC`, …).  Maps constant name → raw PHP source code.
    ///
    /// Built once during construction via [`stubs::build_stub_constant_index`].
    /// Can be consulted when resolving standalone constant references.
    pub stub_constant_index: HashMap<&'static str, &'static str>,
}

impl Backend {
    /// Shared defaults for all Backend constructors.
    ///
    /// Returns a `Backend` with no LSP client, empty maps, and the full
    /// embedded stub indices.  Each public constructor customises only the
    /// fields that differ.
    fn defaults() -> Self {
        Self {
            name: "PHPantomLSP".to_string(),
            version: "0.2.0".to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            client: None,
            workspace_root: Arc::new(Mutex::new(None)),
            psr4_mappings: Arc::new(Mutex::new(Vec::new())),
            use_map: Arc::new(Mutex::new(HashMap::new())),
            namespace_map: Arc::new(Mutex::new(HashMap::new())),
            global_functions: Arc::new(Mutex::new(HashMap::new())),
            global_defines: Arc::new(Mutex::new(HashMap::new())),
            class_index: Arc::new(Mutex::new(HashMap::new())),
            classmap: Arc::new(Mutex::new(HashMap::new())),
            stub_index: stubs::build_stub_class_index(),
            stub_function_index: stubs::build_stub_function_index(),
            stub_constant_index: stubs::build_stub_constant_index(),
        }
    }

    /// Create a new `Backend` connected to an LSP client.
    pub fn new(client: Client) -> Self {
        Self {
            client: Some(client),
            ..Self::defaults()
        }
    }

    /// Create a `Backend` without an LSP client (for unit / integration tests).
    pub fn new_test() -> Self {
        Self::defaults()
    }

    /// Create a `Backend` for tests with custom stub class index.
    ///
    /// This allows tests to inject minimal stub content (e.g. `UnitEnum`,
    /// `BackedEnum`) without depending on `composer install` having been run.
    pub fn new_test_with_stubs(stub_index: HashMap<&'static str, &'static str>) -> Self {
        Self {
            stub_index,
            ..Self::defaults()
        }
    }

    /// Create a `Backend` for tests with custom class, function, and constant
    /// stub indices.
    ///
    /// This allows tests to inject minimal stub content so that they are
    /// fully self-contained and do not depend on `composer install`.
    pub fn new_test_with_all_stubs(
        stub_index: HashMap<&'static str, &'static str>,
        stub_function_index: HashMap<&'static str, &'static str>,
        stub_constant_index: HashMap<&'static str, &'static str>,
    ) -> Self {
        Self {
            stub_index,
            stub_function_index,
            stub_constant_index,
            ..Self::defaults()
        }
    }

    /// Create a `Backend` for tests with a specific workspace root and PSR-4
    /// mappings pre-configured.
    pub fn new_test_with_workspace(
        workspace_root: PathBuf,
        psr4_mappings: Vec<composer::Psr4Mapping>,
    ) -> Self {
        Self {
            workspace_root: Arc::new(Mutex::new(Some(workspace_root))),
            psr4_mappings: Arc::new(Mutex::new(psr4_mappings)),
            ..Self::defaults()
        }
    }
}
