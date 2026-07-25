//! The workspace symbol indexes, grouped out of `Backend`.
//!
//! These are the class, function, and constant lookup tables that answer
//! "where is symbol X defined?" across the whole workspace. All fields are
//! `Arc`-wrapped, so `#[derive(Clone)]` shares them with a cloned `Backend`
//! (the same semantics the per-request clone had as individual fields).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

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
}

impl SymbolIndex {
    pub(crate) fn new() -> Self {
        Self {
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
}
