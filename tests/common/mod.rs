#![allow(dead_code)]

use phpantom_lsp::Backend;
use std::collections::HashMap;
use std::fs;

pub fn create_test_backend() -> Backend {
    Backend::new_test()
}

// Minimal PHP stubs for UnitEnum and BackedEnum so that tests exercising
// the "embedded stub" code-path work without `composer install`.
static UNIT_ENUM_STUB: &str = "\
<?php
interface UnitEnum
{
    public static function cases(): array;
    public readonly string $name;
}
";

static BACKED_ENUM_STUB: &str = "\
<?php
interface BackedEnum extends UnitEnum
{
    public static function from(int|string $value): static;
    public static function tryFrom(int|string $value): ?static;
    public readonly int|string $value;
}
";

/// Create a test backend whose `stub_index` contains minimal `UnitEnum`
/// and `BackedEnum` stubs.  This makes "embedded stub" tests fully
/// self-contained — they no longer require a prior `composer install`.
pub fn create_test_backend_with_stubs() -> Backend {
    let mut stubs: HashMap<&'static str, &'static str> = HashMap::new();
    stubs.insert("UnitEnum", UNIT_ENUM_STUB);
    stubs.insert("BackedEnum", BACKED_ENUM_STUB);
    Backend::new_test_with_stubs(stubs)
}

/// Helper: create a temp workspace with a composer.json and PHP files,
/// then return a Backend configured with that workspace root + PSR-4 mappings.
pub fn create_psr4_workspace(
    composer_json: &str,
    files: &[(&str, &str)],
) -> (Backend, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(dir.path().join("composer.json"), composer_json)
        .expect("failed to write composer.json");
    for (rel_path, content) in files {
        let full = dir.path().join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("failed to create dirs");
        }
        fs::write(&full, content).expect("failed to write PHP file");
    }

    let mappings = phpantom_lsp::composer::parse_composer_json(dir.path());
    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);
    (backend, dir)
}
