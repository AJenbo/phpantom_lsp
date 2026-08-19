//! Workspace-level configuration and location metadata, grouped out of
//! `Backend`.
//!
//! Unlike the other extracted groups, `Clone` is implemented by hand rather
//! than derived: the `Arc<RwLock<…>>` fields are shared by `Arc::clone`,
//! while the `parking_lot::Mutex` fields (which are rarely accessed or always
//! written) are deep-copied into a fresh `Mutex`. This exactly preserves the
//! per-field clone semantics `Backend`'s clone had when these were individual
//! fields.

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::{Mutex, RwLock};

use crate::ClassCompletionOrigin;
use crate::composer;
use crate::config;
use crate::types::PhpVersion;

/// Workspace root, PSR-4 mappings, vendor locations, PHP version, and the
/// loaded `.phpantom.toml` configuration.
pub(crate) struct WorkspaceEnv {
    /// The root directory of the workspace (set during `initialize`).
    pub(crate) workspace_root: Arc<RwLock<Option<PathBuf>>>,
    /// PSR-4 autoload mappings parsed from `composer.json`.
    pub(crate) psr4_mappings: Arc<RwLock<Vec<composer::Psr4Mapping>>>,
    /// `file://` URI prefixes for all known vendor directories.
    pub(crate) vendor_uri_prefixes: Mutex<Vec<String>>,
    /// Absolute raw and canonical paths of all known vendor directories.
    pub(crate) vendor_dir_paths: Mutex<Vec<PathBuf>>,
    /// Canonical vendor package roots paired with completion provenance.
    pub(crate) vendor_package_origin_roots:
        Arc<RwLock<Vec<(PathBuf, ClassCompletionOrigin, String)>>>,
    /// The target PHP version used for version-aware stub filtering.
    pub(crate) php_version: Mutex<PhpVersion>,
    /// Per-project configuration loaded from `.phpantom.toml`.
    ///
    /// Shared by `Arc` (unlike `php_version` and the other plain `Mutex`
    /// fields above) because, unlike those, it is written again after
    /// startup: a config-file watcher reloads it on a cloned `Backend`
    /// (the blocking-task and background-worker clones), and that reload
    /// must be visible to every other clone, including the long-lived one
    /// that answers LSP requests.
    pub(crate) config: Arc<Mutex<config::Config>>,
    /// Where the global `.phpantom.toml` layer is read from, or `None`
    /// to load the project config on its own.
    ///
    /// Fixed for the lifetime of the session (the file's *contents* are
    /// re-read on change, its location never moves), so unlike `config`
    /// it needs no interior mutability. Test backends set it to `None`
    /// so a `.phpantom.toml` in the developer's own config directory
    /// cannot change what the suite asserts.
    pub(crate) global_config_path: Option<PathBuf>,
}

impl WorkspaceEnv {
    /// The environment a real session runs in: the global config layer
    /// comes from the platform config directory.
    pub(crate) fn new() -> Self {
        Self::with_global_config(config::global_config_path())
    }

    /// An environment with no global config layer, for tests.
    pub(crate) fn new_isolated() -> Self {
        Self::with_global_config(None)
    }

    fn with_global_config(global_config_path: Option<PathBuf>) -> Self {
        Self {
            workspace_root: Arc::new(RwLock::new(None)),
            psr4_mappings: Arc::new(RwLock::new(Vec::new())),
            vendor_uri_prefixes: Mutex::new(Vec::new()),
            vendor_dir_paths: Mutex::new(Vec::new()),
            vendor_package_origin_roots: Arc::new(RwLock::new(Vec::new())),
            php_version: Mutex::new(PhpVersion::default()),
            config: Arc::new(Mutex::new(config::Config::default())),
            global_config_path,
        }
    }
}

impl Clone for WorkspaceEnv {
    fn clone(&self) -> Self {
        Self {
            workspace_root: Arc::clone(&self.workspace_root),
            psr4_mappings: Arc::clone(&self.psr4_mappings),
            vendor_uri_prefixes: Mutex::new(self.vendor_uri_prefixes.lock().clone()),
            vendor_dir_paths: Mutex::new(self.vendor_dir_paths.lock().clone()),
            vendor_package_origin_roots: Arc::clone(&self.vendor_package_origin_roots),
            php_version: Mutex::new(*self.php_version.lock()),
            config: Arc::clone(&self.config),
            global_config_path: self.global_config_path.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Backend;

    /// A test that writes a `.phpantom.toml` into a temp workspace has to
    /// be judged against that file alone. When the test constructors
    /// carry the platform global config path, whatever sits in the
    /// developer's own config directory merges underneath it and quietly
    /// changes the result on that machine but not on a clean CI runner.
    #[test]
    fn test_constructors_carry_no_global_config() {
        let backends = [
            ("new_test", Backend::new_test()),
            (
                "new_test_with_workspace",
                Backend::new_test_with_workspace(std::path::PathBuf::from("/tmp"), Vec::new()),
            ),
            (
                "new_test_with_stubs",
                Backend::new_test_with_stubs(Default::default()),
            ),
            (
                "new_test_with_full_stubs",
                Backend::new_test_with_full_stubs(),
            ),
        ];

        for (name, backend) in backends {
            assert!(
                backend.workspace.global_config_path.is_none(),
                "Backend::{name} must not read the global .phpantom.toml"
            );
        }
    }

    /// The clones a diagnostic worker or blocking task runs on reload the
    /// config themselves, so they have to keep pointing at the same
    /// global file the original was built with.
    #[test]
    fn clone_preserves_the_global_config_path() {
        let path = std::path::PathBuf::from("/tmp/global/.phpantom.toml");
        let mut backend = Backend::new_test();
        backend.workspace.global_config_path = Some(path.clone());

        let clone = backend.clone_for_diagnostic_worker();

        assert_eq!(clone.workspace.global_config_path.as_deref(), Some(&*path));
    }
}
