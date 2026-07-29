//! The workspace symbol indexes, grouped out of `Backend`.
//!
//! These are the class, function, and constant lookup tables that answer
//! "where is symbol X defined?" across the whole workspace. All fields are
//! `Arc`-wrapped, so `#[derive(Clone)]` shares them with a cloned `Backend`
//! (the same semantics the per-request clone had as individual fields).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use crate::ci_map::{CiMap, CiSet};
use crate::types::{ClassInfo, DefineInfo, FunctionInfo, MethodStore};
use crate::{ClassCompletionOrigin, UriGlobals};

/// Class, function, and constant discovery and lookup indexes.
#[derive(Clone)]
pub(crate) struct SymbolIndex {
    /// Maps a file URI to a list of `ClassInfo` extracted from that file.
    pub(crate) uri_classes_index: Arc<RwLock<HashMap<String, Vec<Arc<ClassInfo>>>>>,
    /// Global function definitions indexed by (case-insensitive) name.
    pub(crate) global_functions: Arc<RwLock<CiMap<(String, FunctionInfo)>>>,
    /// Global constants from `define()` / top-level `const` statements.
    pub(crate) global_defines: Arc<RwLock<HashMap<String, DefineInfo>>>,
    /// Per-URI record of the global function/constant symbols each file
    /// contributed at its last parse, for targeted eviction.
    pub(crate) uri_globals_index: Arc<RwLock<HashMap<String, UriGlobals>>>,
    /// Autoload function index: function FQN → file path on disk.
    pub(crate) autoload_function_index: Arc<RwLock<CiMap<PathBuf>>>,
    /// Completion provenance for autoloaded function symbols.
    pub(crate) autoload_function_origin_index: Arc<RwLock<CiMap<ClassCompletionOrigin>>>,
    /// Autoload constant index: constant name → file path on disk.
    pub(crate) autoload_constant_index: Arc<RwLock<HashMap<String, PathBuf>>>,
    /// Completion provenance for autoloaded constant symbols.
    pub(crate) autoload_constant_origin_index: Arc<RwLock<HashMap<String, ClassCompletionOrigin>>>,
    /// Paths of all files discovered through Composer's `autoload_files.php`.
    pub(crate) autoload_file_paths: Arc<RwLock<Vec<PathBuf>>>,
    /// Index of fully-qualified class names to file URIs.
    pub(crate) fqn_uri_index: Arc<RwLock<CiMap<String>>>,
    /// Completion provenance for fully-qualified class names.
    pub(crate) fqn_origin_index: Arc<RwLock<CiMap<ClassCompletionOrigin>>>,
    /// Secondary index mapping FQNs directly to their parsed `ClassInfo`.
    pub(crate) fqn_class_index: Arc<RwLock<CiMap<Arc<ClassInfo>>>>,
    /// Negative-result cache for `find_or_load_class`.
    pub(crate) class_not_found_cache: Arc<RwLock<CiSet>>,
    /// Global method store: `(class_fqn, method_name)` → `Arc<MethodInfo>`.
    pub(crate) method_store: MethodStore,
    /// Reverse inheritance index: parent FQN → list of child FQNs.
    pub(crate) gti_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Identity of this set of indexes, unique for the life of the process
    /// and shared by every clone (a cloned `Backend` shares the indexes
    /// themselves).  Per-thread caches key on it so that a worker thread
    /// reused across `Backend`s — nextest runs many in one process — can
    /// never serve one project's classes to another.
    id: u64,
    /// Generation of the inputs to [`Backend::find_or_load_class`]:
    /// `fqn_class_index` and `class_not_found_cache`.  See
    /// [`note_class_lookup_change`](Self::note_class_lookup_change).
    class_lookup_generation: Arc<AtomicU64>,
}

impl SymbolIndex {
    pub(crate) fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            class_lookup_generation: Arc::new(AtomicU64::new(0)),
            uri_classes_index: Arc::new(RwLock::new(HashMap::new())),
            global_functions: Arc::new(RwLock::new(CiMap::new())),
            global_defines: Arc::new(RwLock::new(HashMap::new())),
            uri_globals_index: Arc::new(RwLock::new(HashMap::new())),
            autoload_function_index: Arc::new(RwLock::new(CiMap::new())),
            autoload_function_origin_index: Arc::new(RwLock::new(CiMap::new())),
            autoload_constant_index: Arc::new(RwLock::new(HashMap::new())),
            autoload_constant_origin_index: Arc::new(RwLock::new(HashMap::new())),
            autoload_file_paths: Arc::new(RwLock::new(Vec::new())),
            fqn_uri_index: Arc::new(RwLock::new(CiMap::new())),
            fqn_origin_index: Arc::new(RwLock::new(CiMap::new())),
            fqn_class_index: Arc::new(RwLock::new(CiMap::new())),
            class_not_found_cache: Arc::new(RwLock::new(CiSet::new())),
            method_store: Arc::new(RwLock::new(HashMap::new())),
            gti_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Identity of this index, for keying per-thread caches.
    pub(crate) fn id(&self) -> u64 {
        self.id
    }

    /// The generation a class-lookup answer must be stamped with to still
    /// be valid.  Read it *before* performing the lookup: a bump that
    /// lands afterwards then invalidates the answer, rather than the
    /// answer being recorded as fresh when it already missed a change.
    pub(crate) fn class_lookup_generation(&self) -> u64 {
        self.class_lookup_generation.load(Ordering::Acquire)
    }

    /// Record that class lookups may now resolve differently, retiring
    /// every memoised answer in `class_loader_memo`.
    ///
    /// Call this *after* mutating `fqn_class_index` or clearing
    /// `class_not_found_cache` — the two structures every
    /// `find_or_load_class` answer is derived from.  Bumping afterwards is
    /// what makes the memo no staler than those caches themselves: a
    /// reader that already loaded the old generation stamps its answer
    /// with a value that no longer matches, so the answer is discarded
    /// rather than served.
    pub(crate) fn note_class_lookup_change(&self) {
        self.class_lookup_generation.fetch_add(1, Ordering::Release);
    }
}
