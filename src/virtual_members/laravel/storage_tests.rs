use super::super::config_values::parse_config_tree;
use super::*;
use crate::types::MethodInfo;

fn tree(config: &str) -> ConfigNode {
    parse_config_tree(config).unwrap()
}

/// A fresh Laravel install's `local`/`public` disks: both built-in, so the
/// correction is safe.
#[test]
fn only_local_disks_are_safe() {
    let tree = tree(
        r#"<?php return [
            'disks' => [
                'local' => ['driver' => 'local', 'root' => 'storage/app'],
                'public' => ['driver' => 'local', 'root' => 'storage/app/public'],
            ],
        ];"#,
    );
    assert!(disks_use_builtin_drivers_only(&tree));
}

/// Every framework-shipped driver, including `scoped` (which delegates to
/// another disk's driver via `build()` rather than building anything itself)
/// is safe.
#[test]
fn every_builtin_driver_is_safe() {
    let tree = tree(
        r#"<?php return [
            'disks' => [
                'local' => ['driver' => 'local'],
                'ftp_disk' => ['driver' => 'ftp'],
                'sftp_disk' => ['driver' => 'sftp'],
                's3' => ['driver' => 's3'],
                'cdn' => ['driver' => 'scoped', 'disk' => 's3', 'prefix' => 'public'],
            ],
        ];"#,
    );
    assert!(disks_use_builtin_drivers_only(&tree));
}

/// A disk built by a `Storage::extend()`-registered driver is not a
/// `FilesystemAdapter`, so the correction would be unsound.
#[test]
fn custom_driver_is_unsafe() {
    let tree = tree(
        r#"<?php return [
            'disks' => [
                'local' => ['driver' => 'local'],
                'dropbox' => ['driver' => 'dropbox'],
            ],
        ];"#,
    );
    assert!(!disks_use_builtin_drivers_only(&tree));
}

/// An env-overridable driver could resolve to anything at runtime, so it is
/// treated the same as an unreadable custom driver.
#[test]
fn dynamic_driver_is_unsafe() {
    let tree = tree(
        r#"<?php return [
            'disks' => [
                'local' => ['driver' => env('DISK_DRIVER', 'local')],
            ],
        ];"#,
    );
    assert!(!disks_use_builtin_drivers_only(&tree));
}

/// A disk with no readable `driver` key at all cannot be classified, so it
/// is treated as unsafe rather than silently ignored.
#[test]
fn missing_driver_key_is_unsafe() {
    let tree = tree(
        r#"<?php return [
            'disks' => [
                'local' => ['root' => 'storage/app'],
            ],
        ];"#,
    );
    assert!(!disks_use_builtin_drivers_only(&tree));
}

/// A config with no `disks` array at all (the tree could not be read as a
/// real `filesystems.php`) is unsafe, not vacuously safe.
#[test]
fn missing_disks_key_is_unsafe() {
    let tree = tree(r#"<?php return ['default' => 'local'];"#);
    assert!(!disks_use_builtin_drivers_only(&tree));
}

/// An empty `disks` array is unsafe for the same reason: a project that
/// genuinely configures zero disks is not something the correction should
/// have to reason about, and it is indistinguishable from an unreadable one.
#[test]
fn empty_disks_is_unsafe() {
    let tree = tree(r#"<?php return ['disks' => []];"#);
    assert!(!disks_use_builtin_drivers_only(&tree));
}

fn make_method_typed(name: &str, return_type: Option<PhpType>) -> MethodInfo {
    MethodInfo {
        return_type,
        ..MethodInfo::virtual_method(name, None)
    }
}

#[test]
fn filesystem_contract_return_becomes_adapter() {
    let adapter = PhpType::named(atom(FILESYSTEM_ADAPTER_FQN));
    let mut method = make_method_typed("disk", Some(PhpType::named(atom(FILESYSTEM_CONTRACT_FQN))));
    assert!(refine_disk_return_type(&mut method, &adapter));
    assert_eq!(method.return_type.unwrap().to_string(), adapter.to_string());
}

#[test]
fn cloud_contract_return_becomes_adapter() {
    let adapter = PhpType::named(atom(FILESYSTEM_ADAPTER_FQN));
    let mut method = make_method_typed("cloud", Some(PhpType::named(atom(CLOUD_CONTRACT_FQN))));
    assert!(refine_disk_return_type(&mut method, &adapter));
    assert_eq!(method.return_type.unwrap().to_string(), adapter.to_string());
}

#[test]
fn non_contract_return_is_left_untouched() {
    let adapter = PhpType::named(atom(FILESYSTEM_ADAPTER_FQN));
    let original = PhpType::named(atom("App\\Storage\\CustomAdapter"));
    let mut method = make_method_typed("disk", Some(original.clone()));
    assert!(!refine_disk_return_type(&mut method, &adapter));
    assert_eq!(
        method.return_type.unwrap().to_string(),
        original.to_string()
    );
}
