//! PHPantom — a fast, lightweight PHP language server.
//!
//! Diagnostics are debounced: each `did_change` bumps a per-file version
//! counter and spawns a delayed task. The task only publishes if its
//! version still matches (i.e. no newer edit arrived in the meantime).
//!
//! This crate is organised into the following modules:
//!
//! - [`types`] — Data structures for extracted PHP information (classes, methods, functions, etc.)
//! - `parser` — PHP parsing and AST extraction using mago_syntax
//! - [`completion`] — Completion logic (target extraction, type resolution, item building,
//!   and the top-level completion request handler)
//! - [`composer`] — Composer autoload (PSR-4, classmap) parsing and class-to-file resolution
//! - `server` — The LSP `LanguageServer` trait implementation (thin wrapper that delegates
//!   to feature-specific modules)
//! - `util` — Utility helpers (position conversion, class lookup, logging)
//! - `hover` — Hover support (`textDocument/hover`). Resolves the symbol under the
//!   cursor and returns type information, method signatures, and docblock descriptions
//! - `signature_help` — Signature help (`textDocument/signatureHelp`). Shows parameter
//!   hints while typing function/method arguments, with active-parameter tracking
//! - `definition` — Go-to-definition support for classes, members, and functions
//! - `inheritance` — Base class inheritance resolution. Merges members from parent
//!   classes and traits into a unified `ClassInfo`
//! - `virtual_members` — Virtual member provider abstraction. Defines the
//!   [`VirtualMemberProvider`](virtual_members::VirtualMemberProvider) trait and
//!   merge logic for members synthesized from `@method`/`@property` tags,
//!   `@mixin` classes, and framework-specific patterns (e.g. Laravel)
//! - `resolution` — Class and function lookup / name resolution (multi-phase:
//!   class_index → ast_map → classmap → PSR-4 → stubs)
//! - `subject_extraction` — Shared helpers for extracting the left-hand side of
//!   `->`, `?->`, and `::` access operators (used by both completion and definition)
//! - `highlight` — Document highlighting (`textDocument/documentHighlight`).
//!   When the cursor lands on a symbol, returns all other occurrences in the
//!   current file so the editor can highlight them.  Uses the precomputed
//!   `SymbolMap` with no additional parsing.  Variables are scoped to their
//!   enclosing function/closure; class names, members, functions, and constants
//!   are file-global.
//! - `code_actions` — Code actions (`textDocument/codeAction`). Provides:
//!   - `code_actions::import_class` — Import class quick-fix (add a `use`
//!     statement for unresolved class names)
//!   - `code_actions::remove_unused_import` — Remove unused import quick-fix
//!     (delete individual or all unused `use` statements)
//! - [`diagnostics`] — Diagnostics publishing (`textDocument/publishDiagnostics`).
//!   Collects and publishes diagnostics on `didOpen` / `didChange`, clears on
//!   `didClose`.  Currently implemented providers:
//!   - `diagnostics::deprecated` — `@deprecated` usage diagnostics (strikethrough
//!     via `DiagnosticTag::Deprecated` on references to deprecated symbols)
//!   - `diagnostics::unused_imports` — unused `use` dimming
//!     (`DiagnosticTag::Unnecessary` on imports with no references in the file)
//!   - `diagnostics::unknown_classes` — unknown class diagnostics
//!     (`Severity::Warning` on `ClassReference` spans that cannot be resolved
//!     through any resolution phase)
//!   - `diagnostics::unresolved_member_access` — opt-in diagnostic
//!     (`Severity::Hint` on `MemberAccess` spans where the subject type
//!     cannot be resolved at all; enabled via `[diagnostics]
//!     unresolved-member-access = true` in `.phpantom.toml`)
//! - [`docblock`] — PHPDoc block parsing, split into submodules:
//!   - `docblock::tags` — tag extraction (`@return`, `@var`, `@property`, `@method`,
//!     `@mixin`, `@deprecated`, `@phpstan-assert`, docblock text retrieval)
//!   - `docblock::conditional` — PHPStan conditional return type parsing
//!   - `docblock::types` — type cleaning utilities (`clean_type`, `strip_nullable`,
//!     `is_scalar`, `split_type_token`), PHPStan array shape parsing
//!     (`parse_array_shape`, `extract_array_shape_value_type`), and object shape
//!     parsing (`parse_object_shape`, `extract_object_shape_property_type`,
//!     `is_object_shape`)

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tower_lsp::Client;

// ─── Module declarations ────────────────────────────────────────────────────

pub mod classmap_scanner;
mod code_actions;
pub mod completion;
pub mod composer;
pub mod config;
mod definition;
pub mod diagnostics;
pub mod docblock;
mod highlight;
mod hover;
pub(crate) mod inheritance;
mod parser;
mod references;
mod rename;
mod resolution;
mod server;
mod signature_help;
pub mod stubs;
pub mod subject_expr;
pub(crate) mod subject_extraction;
pub(crate) mod symbol_map;
pub mod types;
mod util;
pub(crate) mod virtual_members;

#[cfg(test)]
pub mod test_fixtures;

// ─── Re-exports ─────────────────────────────────────────────────────────────

// Re-export public types so that dependents (tests, main) can import them
// from the crate root, e.g. `use phpantom_lsp::{Backend, AccessKind}`.
pub use completion::target::extract_completion_target;
pub use types::{AccessKind, ClassInfo, DefineInfo, FunctionInfo, Visibility};
pub use virtual_members::resolve_class_fully;

// ─── Backend ────────────────────────────────────────────────────────────────

/// The main LSP backend that holds all server state.
///
/// Method implementations are spread across several modules:
/// - `parser` — `parse_php`, `update_ast`, and module-level AST extraction helpers
///   (`extract_hint_string`, `extract_parameters`, `extract_visibility`, `extract_property_info`)
/// - `completion::handler` — Top-level completion request orchestration
/// - `completion::target` — module-level `extract_completion_target`
/// - `completion::resolver` — `resolve_target_classes` and type-resolution helpers
/// - `completion::builder` — module-level `build_completion_items`, `build_method_label`
/// - [`composer`] — PSR-4 autoload mapping and class file resolution
/// - `server` — `impl LanguageServer` (initialize, completion, did_open, …)
/// - `resolution` — `find_or_load_class`, `find_or_load_function`, `resolve_class_name`,
///   `resolve_function_name`
/// - `inheritance` — `resolve_class_with_inheritance` (base resolution), trait/parent merging
/// - `virtual_members` — `resolve_class_fully` (base resolution + virtual member providers),
///   `VirtualMemberProvider` trait, merge logic, provider registry
/// - `subject_extraction` — Shared subject extraction helpers for `->`, `?->`, `::` operators
/// - `util` — module-level `position_to_offset`, `find_class_at_offset`,
///   `find_class_by_name`, plus `log`, `get_classes_for_uri`
/// - `definition` — `resolve_definition`, member resolution, function resolution
/// - `diagnostics` — `publish_diagnostics_for_file`, `clear_diagnostics_for_file`,
///   `collect_deprecated_diagnostics`, `collect_unused_import_diagnostics`,
///   `collect_unknown_class_diagnostics`, `collect_unknown_member_diagnostics`,
///   `collect_unresolved_member_access_diagnostics`
/// - `highlight` — `handle_document_highlight` (same-file symbol occurrence highlighting)
pub struct Backend {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) open_files: Arc<Mutex<HashMap<String, String>>>,
    /// Maps a file URI to a list of ClassInfo extracted from that file.
    pub(crate) ast_map: Arc<Mutex<HashMap<String, Vec<ClassInfo>>>>,
    /// Per-file precomputed symbol location maps for O(log n) lookup.
    ///
    /// Built during `update_ast` by walking the AST and recording every
    /// navigable symbol occurrence (class references, member accesses,
    /// variables, function calls, etc.).  Consulted by `resolve_definition`
    /// to replace character-level backward-walking with a binary search.
    pub(crate) symbol_maps: Arc<Mutex<HashMap<String, symbol_map::SymbolMap>>>,
    pub(crate) client: Option<Client>,
    /// The root directory of the workspace (set during `initialize`).
    pub(crate) workspace_root: Arc<Mutex<Option<PathBuf>>>,
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
    pub(crate) global_functions: Arc<Mutex<HashMap<String, (String, FunctionInfo)>>>,
    /// Global constants defined via `define('NAME', value)` calls or
    /// top-level `const NAME = value;` statements.
    ///
    /// Maps constant name → [`DefineInfo`] containing the file URI,
    /// byte offset of the definition, and the initializer value text.
    ///
    /// Populated from files listed in Composer's `autoload_files.php` at
    /// init time, and also from any opened/changed files that contain
    /// `define()` calls or `const` statements.  Used for constant name
    /// completions, hover (showing the value), and go-to-definition.
    pub(crate) global_defines: Arc<Mutex<HashMap<String, DefineInfo>>>,
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
    pub(crate) class_index: Arc<Mutex<HashMap<String, String>>>,
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
    pub(crate) classmap: Arc<Mutex<HashMap<String, PathBuf>>>,
    /// Embedded PHP stubs for built-in classes/interfaces (e.g. `UnitEnum`,
    /// `BackedEnum`, `Iterator`, `Countable`, …).
    /// Maps class short name → raw PHP source code.
    ///
    /// Built once during construction via [`stubs::build_stub_class_index`].
    /// Consulted by `find_or_load_class` as a final fallback after the
    /// `ast_map` and PSR-4 resolution.  Stub files are parsed lazily on
    /// first access and cached in `ast_map` under `phpantom-stub://` URIs.
    pub(crate) stub_index: HashMap<&'static str, &'static str>,
    /// Cache of fully-resolved classes (inheritance + virtual members).
    ///
    /// Keyed by fully-qualified class name.  Populated lazily by
    /// [`resolve_class_fully_cached`](crate::virtual_members::resolve_class_fully_cached)
    /// and cleared whenever a file is re-parsed (`update_ast` /
    /// `parse_and_cache_content`) so that stale results never survive
    /// an edit.
    pub(crate) resolved_class_cache: virtual_members::ResolvedClassCache,
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
    pub(crate) stub_constant_index: HashMap<&'static str, &'static str>,
    /// The target PHP version used for version-aware stub filtering.
    ///
    /// Detected from `composer.json` (`require.php`) during server
    /// initialization.  When no version constraint is found, defaults
    /// to PHP 8.5.  Stub elements annotated with
    /// `#[PhpStormStubsElementAvailable]` are filtered against this
    /// version so that only the correct variant is presented.
    ///
    /// Wrapped in a `Mutex` so that `set_php_version` can be called
    /// during `initialized` (which receives `&self`, not `&mut self`).
    pub(crate) php_version: Mutex<types::PhpVersion>,
    /// The `file://` URI prefix for the vendor directory, used to skip
    /// diagnostics for vendor files.
    ///
    /// Built during `initialized` from the workspace root and
    /// `composer.json`'s `config.vendor-dir` (default `"vendor"`).
    /// Example: `"file:///home/user/project/vendor/"`.
    ///
    /// When empty (no workspace root), vendor-skipping is disabled.
    pub(crate) vendor_uri_prefix: Mutex<String>,
    /// The vendor directory name (e.g. `"vendor"` or a custom path from
    /// `composer.json`'s `config.vendor-dir`).
    ///
    /// Cached during `initialized` so that cross-file scans (find
    /// references, go-to-implementation) can skip the vendor directory
    /// without re-reading `composer.json` on every request.
    pub(crate) vendor_dir_name: Mutex<String>,
    /// Monotonically increasing version counter for diagnostic debouncing.
    ///
    /// Bumped on every `did_change`.  A background diagnostic task
    /// checks this counter after a quiet period and only publishes
    /// results when the counter hasn't moved, meaning the user
    /// stopped typing.
    pub(crate) diag_version: Arc<AtomicU64>,
    /// Notification handle used to wake the diagnostic worker task.
    ///
    /// [`schedule_diagnostics`](Self::schedule_diagnostics) calls
    /// `notify_one()` after bumping `diag_version`; the worker awaits
    /// `notified()` in its main loop.
    pub(crate) diag_notify: Arc<tokio::sync::Notify>,
    /// The file URI that needs a diagnostic pass, set by
    /// [`schedule_diagnostics`](Self::schedule_diagnostics) and consumed
    /// by the diagnostic worker.  Only the most recent URI is kept;
    /// earlier requests are superseded.
    ///
    /// Wrapped in `Arc` so the diagnostic worker task (spawned during
    /// `initialized`) shares the same slot as the main `Backend`.
    pub(crate) diag_pending_uri: Arc<Mutex<Option<String>>>,
    /// Per-project configuration loaded from `.phpantom.toml`.
    ///
    /// Read once during `initialized` from the workspace root directory.
    /// When the file is missing or cannot be parsed, all settings use
    /// their defaults.  Wrapped in a `Mutex` so that `initialized`
    /// (which receives `&self`) can set it after loading the file.
    /// The diagnostic worker snapshots the value at spawn time.
    pub(crate) config: Mutex<config::Config>,
}

impl Backend {
    /// Shared defaults for all Backend constructors.
    ///
    /// Returns a `Backend` with no LSP client, empty maps, and the full
    /// embedded stub indices.  Each public constructor customises only the
    /// fields that differ.
    fn defaults() -> Self {
        Self {
            name: "PHPantom".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            open_files: Arc::new(Mutex::new(HashMap::new())),
            ast_map: Arc::new(Mutex::new(HashMap::new())),
            symbol_maps: Arc::new(Mutex::new(HashMap::new())),
            client: None,
            workspace_root: Arc::new(Mutex::new(None)),
            vendor_uri_prefix: Mutex::new(String::new()),
            vendor_dir_name: Mutex::new("vendor".to_string()),
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
            resolved_class_cache: virtual_members::new_resolved_class_cache(),
            php_version: Mutex::new(types::PhpVersion::default()),
            diag_version: Arc::new(AtomicU64::new(0)),
            diag_notify: Arc::new(tokio::sync::Notify::new()),
            diag_pending_uri: Arc::new(Mutex::new(None)),
            config: Mutex::new(config::Config::default()),
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

    // ── Public accessors for integration tests ──────────────────────────

    /// Borrow the workspace root mutex (used by integration tests to set a
    /// custom workspace directory).
    pub fn workspace_root(&self) -> &Arc<Mutex<Option<PathBuf>>> {
        &self.workspace_root
    }

    /// Borrow the global functions mutex (used by integration tests to
    /// inject user-defined functions or inspect the cache).
    pub fn global_functions(&self) -> &Arc<Mutex<HashMap<String, (String, FunctionInfo)>>> {
        &self.global_functions
    }

    /// Borrow the global defines mutex (used by integration tests to
    /// inject user-defined constants or inspect the cache).
    pub fn global_defines(&self) -> &Arc<Mutex<HashMap<String, DefineInfo>>> {
        &self.global_defines
    }

    /// Borrow the class index mutex (used by integration tests to
    /// populate discovered class entries).
    pub fn class_index(&self) -> &Arc<Mutex<HashMap<String, String>>> {
        &self.class_index
    }

    /// Borrow the classmap mutex (used by integration tests to populate
    /// Composer classmap entries).
    pub fn classmap(&self) -> &Arc<Mutex<HashMap<String, PathBuf>>> {
        &self.classmap
    }

    /// Borrow the stub constant index (used by integration tests to
    /// verify built-in constants are present).
    pub fn stub_constant_index(&self) -> &HashMap<&'static str, &'static str> {
        &self.stub_constant_index
    }

    /// Return the configured PHP version.
    pub fn php_version(&self) -> types::PhpVersion {
        self.php_version
            .lock()
            .map(|guard| *guard)
            .unwrap_or_default()
    }

    /// Create a shallow clone of this `Backend` that shares every
    /// `Arc`-wrapped field with the original.
    ///
    /// Non-`Arc` fields (`php_version`, `vendor_uri_prefix`,
    /// `vendor_dir_name`) are snapshotted at call time.  The stub
    /// indices (`stub_index`, `stub_function_index`,
    /// `stub_constant_index`) are cloned (they are static `&str`
    /// maps, so this is cheap).
    ///
    /// Used by `initialized()` to build a `Backend` value that can be
    /// moved into the `tokio::spawn`-ed diagnostic worker task while
    /// still observing every mutation the "real" `Backend` makes to
    /// the shared `Arc<Mutex<…>>` maps.
    pub(crate) fn clone_for_diagnostic_worker(&self) -> Self {
        Self {
            name: self.name.clone(),
            version: self.version.clone(),
            open_files: Arc::clone(&self.open_files),
            ast_map: Arc::clone(&self.ast_map),
            symbol_maps: Arc::clone(&self.symbol_maps),
            client: self.client.clone(),
            workspace_root: Arc::clone(&self.workspace_root),
            psr4_mappings: Arc::clone(&self.psr4_mappings),
            use_map: Arc::clone(&self.use_map),
            namespace_map: Arc::clone(&self.namespace_map),
            global_functions: Arc::clone(&self.global_functions),
            global_defines: Arc::clone(&self.global_defines),
            class_index: Arc::clone(&self.class_index),
            classmap: Arc::clone(&self.classmap),
            stub_index: self.stub_index.clone(),
            resolved_class_cache: Arc::clone(&self.resolved_class_cache),
            stub_function_index: self.stub_function_index.clone(),
            stub_constant_index: self.stub_constant_index.clone(),
            php_version: Mutex::new(self.php_version()),
            vendor_uri_prefix: Mutex::new(
                self.vendor_uri_prefix
                    .lock()
                    .map(|g| g.clone())
                    .unwrap_or_default(),
            ),
            vendor_dir_name: Mutex::new(
                self.vendor_dir_name
                    .lock()
                    .map(|g| g.clone())
                    .unwrap_or_else(|_| "vendor".to_string()),
            ),
            diag_version: Arc::clone(&self.diag_version),
            diag_notify: Arc::clone(&self.diag_notify),
            diag_pending_uri: Arc::clone(&self.diag_pending_uri),
            config: Mutex::new(self.config.lock().map(|g| g.clone()).unwrap_or_default()),
        }
    }

    /// Return the current project configuration.
    ///
    /// Returns a clone of the [`Config`](config::Config) loaded from
    /// `.phpantom.toml` (or the default config when the file is missing).
    pub fn config(&self) -> config::Config {
        self.config.lock().map(|g| g.clone()).unwrap_or_default()
    }

    /// Set the PHP version (used by integration tests and during
    /// server initialization after reading `composer.json`).
    pub fn set_php_version(&self, version: types::PhpVersion) {
        if let Ok(mut v) = self.php_version.lock() {
            *v = version;
        }
    }
}
