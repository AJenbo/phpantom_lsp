#![allow(dead_code)]

use phpantom_lsp::Backend;
use std::fs;

pub fn create_test_backend() -> Backend {
    Backend::new_test()
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
