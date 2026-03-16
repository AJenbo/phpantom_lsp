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
//! - `semantic_tokens` — Semantic tokens (`textDocument/semanticTokens/full`).
//!   Type-aware syntax highlighting that goes beyond TextMate grammars.
//!   Maps `SymbolMap` spans to LSP semantic token types (class, interface,
//!   enum, method, property, parameter, variable, function, constant) with
//!   modifiers (declaration, static, readonly, deprecated, abstract).
//!   Resolves `ClassReference` spans to distinguish classes from interfaces,
//!   enums, and traits.  Template parameter names from `@template` tags are
//!   emitted as `typeParameter` tokens.
//! - `code_actions` — Code actions (`textDocument/codeAction`). Provides:
//!   - `code_actions::import_class` — Import class quick-fix (add a `use`
//!     statement for unresolved class names)
//!   - `code_actions::remove_unused_import` — Remove unused import quick-fix
//!     (delete individual or all unused `use` statements)
//! - [`diagnostics`] — Diagnostic collection and delivery.  Supports both
//!   pull diagnostics (`textDocument/diagnostic`, LSP 3.17) and push
//!   diagnostics (`textDocument/publishDiagnostics`) as a fallback.
//!   Currently implemented providers:
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
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use parking_lot::{Mutex, RwLock};
use tower_lsp::Client;

/// A single parse error entry: `(message, start_byte_offset, end_byte_offset)`.
///
/// Stored per file in [`Backend::parse_errors`] during `update_ast` and
/// consumed by the syntax-error diagnostic collector.
pub(crate) type ParseErrorEntry = (String, u32, u32);

// ─── Module declarations ────────────────────────────────────────────────────

pub mod classmap_scanner;
mod code_actions;
mod code_lens;
pub mod completion;
pub mod composer;
pub mod config;
mod definition;
pub mod diagnostics;
pub mod docblock;
mod document_links;
mod document_symbols;
mod folding;
mod formatting;
mod highlight;
mod hover;
pub(crate) mod inheritance;
mod inlay_hints;
mod parser;
mod phpstan;
mod references;
mod rename;
mod resolution;
mod selection_range;
mod semantic_tokens;
mod server;
mod signature_help;
pub mod stubs;
pub mod subject_expr;
pub(crate) mod subject_extraction;
pub(crate) mod symbol_map;
mod type_hierarchy;
pub mod types;
mod util;
pub(crate) mod virtual_members;
mod workspace_symbols;

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
    /// The name of the LSP client (IDE/editor) connected to this server.
    ///
    /// Populated from `InitializeParams.client_info.name` during the
    /// `initialize` handshake.  Used for quirks-mode adjustments when
    /// certain editors need non-standard behavior (e.g. Helix, Neovim).
    /// Empty string when the client does not report its identity.
    pub(crate) client_name: Mutex<String>,
    pub(crate) open_files: Arc<RwLock<HashMap<String, Arc<String>>>>,
    /// Maps a file URI to a list of ClassInfo extracted from that file.
    pub(crate) ast_map: Arc<RwLock<HashMap<String, Vec<Arc<ClassInfo>>>>>,
    /// Per-file precomputed symbol location maps for O(log n) lookup.
    ///
    /// Built during `update_ast` by walking the AST and recording every
    /// navigable symbol occurrence (class references, member accesses,
    /// variables, function calls, etc.).  Consulted by `resolve_definition`
    /// to replace character-level backward-walking with a binary search.
    pub(crate) symbol_maps: Arc<RwLock<HashMap<String, Arc<symbol_map::SymbolMap>>>>,
    /// Per-file parse errors from the Mago parser.
    ///
    /// Each entry is `(message, start_byte_offset, end_byte_offset)`.
    /// Populated during `update_ast` from `Program::errors` and consumed
    /// by the syntax-error diagnostic collector.  When the parser panics
    /// (caught by `catch_unwind`), a single "Parse failed" entry is
    /// stored instead.
    pub(crate) parse_errors: Arc<RwLock<HashMap<String, Vec<ParseErrorEntry>>>>,
    pub(crate) client: Option<Client>,
    /// The root directory of the workspace (set during `initialize`).
    pub(crate) workspace_root: Arc<RwLock<Option<PathBuf>>>,
    /// PSR-4 autoload mappings parsed from `composer.json`.
    pub(crate) psr4_mappings: Arc<RwLock<Vec<composer::Psr4Mapping>>>,
    /// Maps a file URI to its `use` statement mappings (short name → fully qualified name).
    /// For example, `use Klarna\Rest\Resource;` produces `"Resource" → "Klarna\Rest\Resource"`.
    pub(crate) use_map: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
    /// Maps a file URI to its declared namespace (e.g. `"Klarna\Rest\Checkout"`).
    /// Files without a namespace declaration map to `None`.
    pub(crate) namespace_map: Arc<RwLock<HashMap<String, Option<String>>>>,
    /// Global function definitions indexed by function name (short name).
    ///
    /// The value is `(file_uri, FunctionInfo)` so we can jump to the definition.
    /// Populated from files listed in Composer's `autoload_files.php` at init
    /// time, and also from any opened/changed files that contain standalone
    /// function declarations.
    pub(crate) global_functions: Arc<RwLock<HashMap<String, (String, FunctionInfo)>>>,
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
    pub(crate) global_defines: Arc<RwLock<HashMap<String, DefineInfo>>>,
    /// Autoload function index: function FQN → file path on disk.
    ///
    /// Populated by the lightweight `find_symbols` byte-level scan
    /// during initialization.  For non-Composer projects the full-scan
    /// walks all workspace files; for Composer projects it scans the
    /// files listed in `autoload_files.php` (and their `require_once`
    /// chains).  Maps standalone function names to the file that
    /// defines them so that [`find_or_load_function`] can lazily call
    /// `update_ast` on first access instead of eagerly parsing every
    /// file at startup.
    pub(crate) autoload_function_index: Arc<RwLock<HashMap<String, PathBuf>>>,
    /// Autoload constant index: constant name → file path on disk.
    ///
    /// Populated alongside `autoload_function_index` by the
    /// `find_symbols` byte-level scan during initialization.  Maps
    /// `define()` constants and top-level `const` declarations to
    /// the file that defines them for lazy resolution via
    /// `update_ast` on first access.
    pub(crate) autoload_constant_index: Arc<RwLock<HashMap<String, PathBuf>>>,
    /// Paths of all files discovered through Composer's
    /// `autoload_files.php` (and their `require_once` chains).
    ///
    /// The byte-level `find_symbols` scanner only discovers top-level
    /// function and constant declarations.  Functions wrapped in
    /// `if (! function_exists(...))` guards (common in Laravel
    /// helpers) are at brace depth 1 and are missed by the scanner.
    /// This list is the safety net: when `find_or_load_function` or
    /// `resolve_constant_definition` cannot find a symbol in any
    /// index or stubs, it lazily parses each of these files via
    /// `update_ast` until the symbol is found.  Each file is parsed
    /// at most once (subsequent lookups hit `global_functions` /
    /// `global_defines`).
    pub(crate) autoload_file_paths: Arc<RwLock<Vec<PathBuf>>>,
    /// Index of fully-qualified class names to file URIs.
    ///
    /// This allows reliable lookup of classes that don't follow PSR-4
    /// conventions, e.g. classes defined in files listed by Composer's
    /// `autoload_files.php`.  The key is the FQN (e.g.
    /// `"Laravel\\Foundation\\Application"`) and the value is the file URI
    /// where the class is defined.
    ///
    /// Populated from three sources:
    /// - `update_ast` (using the file's namespace + class short name)
    ///   whenever a file is opened or changed.
    /// - The `find_symbols` byte-level scan of Composer autoload files
    ///   during server initialization (so classes in autoload files are
    ///   discoverable by `find_or_load_class` without an eager AST parse).
    /// - The workspace full-scan for non-Composer projects.
    pub(crate) class_index: Arc<RwLock<HashMap<String, String>>>,
    /// Secondary index mapping fully-qualified class names directly to
    /// their parsed `ClassInfo`.
    ///
    /// This turns every Phase 1 lookup in [`find_or_load_class`] into an
    /// O(1) hash lookup instead of scanning all files in `ast_map`.
    /// Maintained alongside `class_index` in `update_ast_inner` and
    /// `parse_and_cache_content_versioned`.
    pub(crate) fqn_index: Arc<RwLock<HashMap<String, Arc<ClassInfo>>>>,
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
    pub(crate) classmap: Arc<RwLock<HashMap<String, PathBuf>>>,
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
    // NOTE: php_version, vendor_uri_prefixes, vendor_dir_paths, config,
    // and diag_pending_uris use parking_lot::Mutex (not RwLock) because
    // they are rarely accessed or always written.
    /// `file://` URI prefixes for all known vendor directories, used to
    /// skip diagnostics, find references, and rename for vendor files.
    ///
    /// Built during `initialized` from the workspace root and
    /// `composer.json`'s `config.vendor-dir` (default `"vendor"`).
    /// Example: `["file:///home/user/project/vendor/"]`.
    ///
    /// In monorepo mode, contains one prefix per discovered subproject
    /// vendor directory.  When empty, vendor-skipping is disabled.
    pub(crate) vendor_uri_prefixes: Mutex<Vec<String>>,
    /// Absolute paths of all known vendor directories.
    ///
    /// Cached during `initialized` so that cross-file scans (find
    /// references, go-to-implementation) can skip vendor directories
    /// without re-reading `composer.json` on every request.
    ///
    /// In monorepo mode, contains one path per discovered subproject
    /// vendor directory.  For single-project workspaces, contains
    /// exactly one entry.
    pub(crate) vendor_dir_paths: Mutex<Vec<PathBuf>>,
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
    /// File URIs that need a diagnostic pass, set by
    /// [`schedule_diagnostics`](Self::schedule_diagnostics) and consumed
    /// by the diagnostic worker.  When a class signature changes, all
    /// open files are queued so that cross-file diagnostics (unknown
    /// member, unknown class, deprecated usage) are refreshed.
    ///
    /// Wrapped in `Arc` so the diagnostic worker task (spawned during
    /// `initialized`) shares the same slot as the main `Backend`.
    pub(crate) diag_pending_uris: Arc<Mutex<Vec<String>>>,
    /// Last-published slow diagnostics (unknown classes, unknown members, etc.)
    /// per file URI.  Used by the two-phase diagnostic publisher: the fast
    /// phase merges fresh fast diagnostics with the previous slow diagnostics
    /// so the editor never shows a flicker where slow diagnostics disappear
    /// and then reappear.
    pub(crate) diag_last_slow: Arc<Mutex<HashMap<String, Vec<tower_lsp::lsp_types::Diagnostic>>>>,
    /// Notification handle used to wake the PHPStan worker task.
    ///
    /// The PHPStan worker is a dedicated background task, separate from
    /// the main diagnostic worker, because PHPStan is extremely slow
    /// and resource-intensive.  Running it in its own task ensures that
    /// native diagnostics (fast + slow phases) are never blocked by a
    /// PHPStan invocation that may take tens of seconds.
    ///
    /// At most one PHPStan process runs at a time.  If the user edits
    /// a file while PHPStan is running, the pending URI is updated and
    /// the worker picks it up after the current run finishes.
    pub(crate) phpstan_notify: Arc<tokio::sync::Notify>,
    /// The single file URI that the PHPStan worker should analyse next.
    ///
    /// Only the most recent file is kept: if the user switches files or
    /// edits rapidly, earlier requests are superseded.  This is
    /// intentional — PHPStan is too slow to queue up multiple files.
    pub(crate) phpstan_pending_uri: Arc<Mutex<Option<String>>>,
    /// Last-published PHPStan diagnostics per file URI.
    ///
    /// The fast and slow diagnostic phases merge these cached results
    /// into their publish calls so that PHPStan errors remain visible
    /// while the user edits (without waiting for a fresh PHPStan run).
    /// The PHPStan worker updates this cache after each successful run
    /// and triggers a re-publish of the affected file.
    pub(crate) phpstan_last_diags:
        Arc<Mutex<HashMap<String, Vec<tower_lsp::lsp_types::Diagnostic>>>>,
    /// Per-file `resultId` for pull diagnostics (`textDocument/diagnostic`).
    ///
    /// Maps file URI → monotonically increasing counter.  Bumped whenever
    /// the diagnostics for a file change (on every `did_change` or when
    /// PHPStan finishes).  The client sends the previous `resultId` back
    /// in the next pull request; if it matches, the server returns
    /// `Unchanged` instead of recomputing.
    pub(crate) diag_result_ids: Arc<Mutex<HashMap<String, u64>>>,
    /// Combined diagnostic cache for pull diagnostics.
    ///
    /// Stores the last-computed full diagnostic set (fast + slow + PHPStan)
    /// per file URI.  When the client pulls diagnostics, the server
    /// returns this cached set.  Updated by the background diagnostic
    /// worker after each pass and by the PHPStan worker after each run.
    pub(crate) diag_last_full: Arc<Mutex<HashMap<String, Vec<tower_lsp::lsp_types::Diagnostic>>>>,
    /// Whether the client supports pull diagnostics.
    ///
    /// Set during `initialize` based on the client's
    /// `textDocument.diagnostic` capability.  When `true`, the server
    /// uses pull diagnostics (`textDocument/diagnostic`) as the primary
    /// path and sends `workspace/diagnostic/refresh` instead of
    /// `schedule_diagnostics_for_open_files`.  When `false`, the server
    /// falls back to the push model (`textDocument/publishDiagnostics`).
    pub(crate) supports_pull_diagnostics: Arc<std::sync::atomic::AtomicBool>,
    // NOTE: resolved_class_cache uses parking_lot::Mutex because it is
    // frequently written (cache stores) and RwLock read→write upgrades
    // are error-prone.
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
            client_name: Mutex::new(String::new()),
            open_files: Arc::new(RwLock::new(HashMap::new())),
            ast_map: Arc::new(RwLock::new(HashMap::new())),
            symbol_maps: Arc::new(RwLock::new(HashMap::new())),
            parse_errors: Arc::new(RwLock::new(HashMap::new())),
            client: None,
            workspace_root: Arc::new(RwLock::new(None)),
            vendor_uri_prefixes: Mutex::new(Vec::new()),
            vendor_dir_paths: Mutex::new(Vec::new()),
            psr4_mappings: Arc::new(RwLock::new(Vec::new())),
            use_map: Arc::new(RwLock::new(HashMap::new())),
            namespace_map: Arc::new(RwLock::new(HashMap::new())),
            global_functions: Arc::new(RwLock::new(HashMap::new())),
            global_defines: Arc::new(RwLock::new(HashMap::new())),
            autoload_function_index: Arc::new(RwLock::new(HashMap::new())),
            autoload_constant_index: Arc::new(RwLock::new(HashMap::new())),
            autoload_file_paths: Arc::new(RwLock::new(Vec::new())),
            class_index: Arc::new(RwLock::new(HashMap::new())),
            fqn_index: Arc::new(RwLock::new(HashMap::new())),
            classmap: Arc::new(RwLock::new(HashMap::new())),
            stub_index: stubs::build_stub_class_index(),
            stub_function_index: stubs::build_stub_function_index(),
            stub_constant_index: stubs::build_stub_constant_index(),
            resolved_class_cache: virtual_members::new_resolved_class_cache(),
            php_version: Mutex::new(types::PhpVersion::default()),
            diag_version: Arc::new(AtomicU64::new(0)),
            diag_notify: Arc::new(tokio::sync::Notify::new()),
            diag_pending_uris: Arc::new(Mutex::new(Vec::new())),
            diag_last_slow: Arc::new(Mutex::new(HashMap::new())),
            phpstan_notify: Arc::new(tokio::sync::Notify::new()),
            phpstan_pending_uri: Arc::new(Mutex::new(None)),
            phpstan_last_diags: Arc::new(Mutex::new(HashMap::new())),
            diag_result_ids: Arc::new(Mutex::new(HashMap::new())),
            diag_last_full: Arc::new(Mutex::new(HashMap::new())),
            supports_pull_diagnostics: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
            workspace_root: Arc::new(RwLock::new(Some(workspace_root))),
            psr4_mappings: Arc::new(RwLock::new(psr4_mappings)),
            ..Self::defaults()
        }
    }

    // ── Public accessors for integration tests ──────────────────────────

    /// Borrow the workspace root mutex (used by integration tests to set a
    /// custom workspace directory).
    pub fn workspace_root(&self) -> &Arc<RwLock<Option<PathBuf>>> {
        &self.workspace_root
    }

    /// Borrow the global functions mutex (used by integration tests to
    /// inject user-defined functions or inspect the cache).
    pub fn global_functions(&self) -> &Arc<RwLock<HashMap<String, (String, FunctionInfo)>>> {
        &self.global_functions
    }

    /// Borrow the global defines mutex (used by integration tests to
    /// inject user-defined constants or inspect the cache).
    pub fn global_defines(&self) -> &Arc<RwLock<HashMap<String, DefineInfo>>> {
        &self.global_defines
    }

    /// Borrow the class index mutex (used by integration tests to
    /// populate discovered class entries).
    pub fn class_index(&self) -> &Arc<RwLock<HashMap<String, String>>> {
        &self.class_index
    }

    /// Borrow the PSR-4 mappings mutex (used by integration tests to
    /// configure autoload mappings).
    pub fn psr4_mappings(&self) -> &Arc<RwLock<Vec<composer::Psr4Mapping>>> {
        &self.psr4_mappings
    }

    /// Borrow the classmap mutex (used by integration tests to populate
    /// Composer classmap entries).
    pub fn classmap(&self) -> &Arc<RwLock<HashMap<String, PathBuf>>> {
        &self.classmap
    }

    /// Borrow the stub constant index (used by integration tests to
    /// verify built-in constants are present).
    pub fn stub_constant_index(&self) -> &HashMap<&'static str, &'static str> {
        &self.stub_constant_index
    }

    /// Borrow the autoload function index (used by integration tests to
    /// populate discovered function entries for non-Composer projects).
    pub fn autoload_function_index(&self) -> &Arc<RwLock<HashMap<String, PathBuf>>> {
        &self.autoload_function_index
    }

    /// Borrow the autoload constant index (used by integration tests to
    /// populate discovered constant entries for non-Composer projects).
    pub fn autoload_constant_index(&self) -> &Arc<RwLock<HashMap<String, PathBuf>>> {
        &self.autoload_constant_index
    }

    /// Borrow the autoload file paths list (used by integration tests
    /// to simulate Composer autoload file discovery).
    pub fn autoload_file_paths(&self) -> &Arc<RwLock<Vec<PathBuf>>> {
        &self.autoload_file_paths
    }

    /// Borrow the open files map (used by integration tests to inject
    /// file content without going through the LSP `didOpen` path).
    pub fn open_files(&self) -> &Arc<RwLock<HashMap<String, Arc<String>>>> {
        &self.open_files
    }

    /// Return the configured PHP version.
    pub fn php_version(&self) -> types::PhpVersion {
        *self.php_version.lock()
    }

    /// Create a shallow clone of this `Backend` that shares every
    /// `Arc`-wrapped field with the original.
    ///
    /// Non-`Arc` fields (`php_version`, `vendor_uri_prefixes`,
    /// `vendor_dir_paths`) are snapshotted at call time.  The stub
    /// indices (`stub_index`, `stub_function_index`,
    /// `stub_constant_index`) are cloned (they are static `&str`
    /// maps, so this is cheap).
    ///
    /// Used by `initialized()` to build a `Backend` value that can be
    /// moved into the `tokio::spawn`-ed diagnostic worker task while
    /// still observing every mutation the "real" `Backend` makes to
    /// the shared `Arc<Mutex<…>>` maps.
    ///
    /// Also used by [`clone_for_blocking`](Self::clone_for_blocking).
    pub(crate) fn clone_for_diagnostic_worker(&self) -> Self {
        Self {
            name: self.name.clone(),
            version: self.version.clone(),
            client_name: Mutex::new(self.client_name.lock().clone()),
            open_files: Arc::clone(&self.open_files),
            ast_map: Arc::clone(&self.ast_map),
            symbol_maps: Arc::clone(&self.symbol_maps),
            parse_errors: Arc::clone(&self.parse_errors),
            // RwLock fields are shared by Arc::clone — the diagnostic
            // worker reads them concurrently with the main Backend.
            client: self.client.clone(),
            workspace_root: Arc::clone(&self.workspace_root),
            psr4_mappings: Arc::clone(&self.psr4_mappings),
            use_map: Arc::clone(&self.use_map),
            namespace_map: Arc::clone(&self.namespace_map),
            global_functions: Arc::clone(&self.global_functions),
            global_defines: Arc::clone(&self.global_defines),
            autoload_function_index: Arc::clone(&self.autoload_function_index),
            autoload_constant_index: Arc::clone(&self.autoload_constant_index),
            autoload_file_paths: Arc::clone(&self.autoload_file_paths),
            class_index: Arc::clone(&self.class_index),
            fqn_index: Arc::clone(&self.fqn_index),
            classmap: Arc::clone(&self.classmap),
            stub_index: self.stub_index.clone(),
            resolved_class_cache: Arc::clone(&self.resolved_class_cache),
            stub_function_index: self.stub_function_index.clone(),
            stub_constant_index: self.stub_constant_index.clone(),
            php_version: Mutex::new(self.php_version()),
            vendor_uri_prefixes: Mutex::new(self.vendor_uri_prefixes.lock().clone()),
            vendor_dir_paths: Mutex::new(self.vendor_dir_paths.lock().clone()),
            diag_version: Arc::clone(&self.diag_version),
            diag_notify: Arc::clone(&self.diag_notify),
            diag_pending_uris: Arc::clone(&self.diag_pending_uris),
            diag_last_slow: Arc::clone(&self.diag_last_slow),
            phpstan_notify: Arc::clone(&self.phpstan_notify),
            phpstan_pending_uri: Arc::clone(&self.phpstan_pending_uri),
            phpstan_last_diags: Arc::clone(&self.phpstan_last_diags),
            diag_result_ids: Arc::clone(&self.diag_result_ids),
            diag_last_full: Arc::clone(&self.diag_last_full),
            supports_pull_diagnostics: Arc::clone(&self.supports_pull_diagnostics),
            config: Mutex::new(self.config.lock().clone()),
        }
    }

    /// Cheap clone that shares all `Arc`-wrapped state with the original.
    ///
    /// Used by `goto_implementation` and `references` to move the
    /// blocking sync work onto a `spawn_blocking` thread while keeping
    /// the async runtime free to flush progress notifications.
    pub(crate) fn clone_for_blocking(&self) -> Self {
        self.clone_for_diagnostic_worker()
    }

    /// Return the current project configuration.
    ///
    /// Returns a clone of the [`Config`](config::Config) loaded from
    /// `.phpantom.toml` (or the default config when the file is missing).
    pub fn config(&self) -> config::Config {
        self.config.lock().clone()
    }

    /// Set the PHP version (used by integration tests and during
    /// server initialization after reading `composer.json`).
    pub fn set_php_version(&self, version: types::PhpVersion) {
        *self.php_version.lock() = version;
    }
}
