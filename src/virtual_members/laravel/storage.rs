//! Static resolution of `Storage::disk()` / `FilesystemManager::disk()` (and
//! the sibling `drive()`/`cloud()`/`build()`) return types from
//! `config/filesystems.php`.
//!
//! `FilesystemManager` declares all four as returning the
//! `Illuminate\Contracts\Filesystem\Filesystem` (or, for `cloud()`, `Cloud`)
//! contract, but every driver the framework ships builds a concrete
//! `Illuminate\Filesystem\FilesystemAdapter` (or a subclass of one). Since
//! `FilesystemAdapter` itself implements the `Cloud` contract, every built-in
//! driver's result satisfies the same concrete type regardless of which one
//! is configured, so no per-disk union is needed. Assertion helpers
//! (`assertExists()`, `assertMissing()`, …) and adapter-only methods
//! (`download()`, …) live only on the concrete adapter, so member access on
//! the declared contract falsely reports them as missing.
//!
//! This correction is only sound in the absence of a custom driver: a
//! project that calls `Storage::extend('name', …)` can bind a disk to
//! anything, and we cannot read the registered closure's return type
//! statically. So the patch only fires when every disk in
//! `config/filesystems.php` names a driver the framework ships
//! (`local`, `ftp`, `sftp`, `s3`, `scoped`); a disk with any other driver
//! name, or one whose driver cannot be read as a single literal, leaves the
//! declared contract untouched. Resolving a `Storage::extend()` closure's own
//! return type to fold custom drivers into the union is tracked separately
//! (see `docs/todo/laravel.md`).

use std::sync::Arc;

use crate::Backend;
use crate::atom::atom;
use crate::php_type::PhpType;
use crate::types::{ClassInfo, MethodInfo};

use super::config_values::ConfigNode;
use super::patches::{FILESYSTEM_ADAPTER_FQN, FILESYSTEM_CONTRACT_FQN, STORAGE_FACADE_FQN};

/// FQN of the concrete manager class `Storage::disk()` and friends forward
/// to via `__callStatic`.
pub(crate) const FILESYSTEM_MANAGER_FQN: &str = "Illuminate\\Filesystem\\FilesystemManager";

/// FQN of the `Cloud` contract that `cloud()` declares.
///
/// `Illuminate\Filesystem\FilesystemAdapter` implements this contract
/// directly, so every built-in driver's concrete result already satisfies
/// it; no separate cloud-specific concrete type is needed.
const CLOUD_CONTRACT_FQN: &str = "Illuminate\\Contracts\\Filesystem\\Cloud";

/// Driver names every supported Laravel version ships a
/// `FilesystemAdapter`-based implementation for. `scoped` delegates to
/// `build()`, which itself resolves through this same set, so it is sound to
/// include without following the delegation.
const BUILTIN_DRIVERS: &[&str] = &["local", "ftp", "sftp", "s3", "scoped"];

/// Public methods whose declared return type is the bare `Filesystem` /
/// `Cloud` contract but which always build a concrete `FilesystemAdapter`.
const DISK_RETURNING_METHODS: &[&str] = &["drive", "disk", "cloud", "build"];

/// Refine `FilesystemManager`/`Storage`'s disk-returning methods to the
/// concrete `FilesystemAdapter`, when `config/filesystems.php` proves every
/// configured disk uses a driver the framework ships.
///
/// Mirrors [`super::patch_auth_user_class`]: a vendor method whose declared
/// return type is only the abstract contract is refined once, so every
/// consumer (completion, hover, diagnostics, the forward walker) sees the
/// concrete adapter without any request-context plumbing. Returns `loaded`
/// unchanged when the class is not one of the two entry points, or when a
/// custom driver makes the correction unsound.
pub(crate) fn patch_storage_disk_type(backend: &Backend, loaded: Arc<ClassInfo>) -> Arc<ClassInfo> {
    let fqn = loaded.fqn();
    if fqn.as_str() != FILESYSTEM_MANAGER_FQN && fqn.as_str() != STORAGE_FACADE_FQN {
        return loaded;
    }
    if !storage_disks_are_safe_to_patch(backend) {
        return loaded;
    }

    let adapter = PhpType::named(atom(FILESYSTEM_ADAPTER_FQN));
    let mut patched = (*loaded).clone();
    let mut changed = false;

    for method in patched.methods.make_mut().iter_mut() {
        if !DISK_RETURNING_METHODS.contains(&method.name.as_str()) {
            continue;
        }
        if refine_disk_return_type(Arc::make_mut(method), &adapter) {
            changed = true;
        }
    }

    // `Storage`'s disk-returning methods exist only as `@method` tags on the
    // facade's class docblock, not as real methods.
    if let Some(doc) = patched.doc_members.as_ref()
        && doc
            .methods
            .iter()
            .any(|m| DISK_RETURNING_METHODS.contains(&m.name.as_str()))
    {
        let mut doc = (**doc).clone();
        for method in &mut doc.methods {
            if !DISK_RETURNING_METHODS.contains(&method.name.as_str()) {
                continue;
            }
            if refine_disk_return_type(Arc::make_mut(method), &adapter) {
                changed = true;
            }
        }
        patched.doc_members = Some(Arc::new(doc));
    }

    if changed { Arc::new(patched) } else { loaded }
}

/// Replace a method's return type with the concrete adapter, but only when
/// it honestly declares the bare `Filesystem` or `Cloud` contract. A
/// hand-written override with a different type is left untouched.
fn refine_disk_return_type(method: &mut MethodInfo, adapter: &PhpType) -> bool {
    let returns_contract = method
        .return_type
        .as_ref()
        .and_then(|rt| rt.class_name())
        .is_some_and(|name| {
            let name = name.trim_start_matches('\\');
            name == FILESYSTEM_CONTRACT_FQN || name == CLOUD_CONTRACT_FQN
        });
    if returns_contract {
        method.return_type = Some(adapter.clone());
    }
    returns_contract
}

/// Whether `config/filesystems.php` proves every configured disk is built by
/// a driver the framework ships, memoized on the [`Backend`].
///
/// Cleared whenever files are re-parsed (see `storage_disk_safe_cache`'s
/// declaration), so an edit to `config/filesystems.php` — adding a
/// `Storage::extend()`-only driver to a disk, for instance — takes effect
/// without a restart.
fn storage_disks_are_safe_to_patch(backend: &Backend) -> bool {
    if let Some(cached) = *backend.storage_disk_safe_cache.read() {
        return cached;
    }
    let trees = backend.cached_config_trees();
    let safe = trees
        .iter()
        .find(|(prefix, _)| prefix == "filesystems")
        .is_some_and(|(_, tree)| disks_use_builtin_drivers_only(tree));
    *backend.storage_disk_safe_cache.write() = Some(safe);
    safe
}

/// Whether every disk under `filesystems.disks` in a parsed config tree
/// names a single, statically-known built-in driver.
///
/// Returns `false` (never safe to patch) when there are no disks to check,
/// since that almost certainly means the tree could not be read rather than
/// a project that genuinely configures none.
fn disks_use_builtin_drivers_only(tree: &ConfigNode) -> bool {
    let Some(disks) = tree.get(&["disks"]) else {
        return false;
    };
    let names = disks.child_keys();
    if names.is_empty() {
        return false;
    }
    names.iter().all(|name| {
        let Some(driver) = tree.value_at(&["disks", name.as_str(), "driver"]) else {
            return false;
        };
        let (drivers, dynamic) = driver.as_strings();
        !dynamic && drivers.len() == 1 && BUILTIN_DRIVERS.contains(&drivers[0].as_str())
    })
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
