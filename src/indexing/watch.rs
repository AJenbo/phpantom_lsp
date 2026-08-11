//! Watched-file change application.
//!
//! Applies a `workspace/didChangeWatchedFiles` batch to the symbol
//! indexes on a blocking thread.

use std::path::PathBuf;

use tower_lsp::lsp_types::*;

use crate::Backend;

impl Backend {
    /// Apply a `workspace/didChangeWatchedFiles` batch to the indexes.
    ///
    /// Returns `true` if any PHP file or composer change was acted on (so the
    /// caller can ask the editor to re-pull diagnostics).  Runs entirely on a
    /// blocking thread; it parses no files on the async runtime.
    ///
    /// Editors cannot watch the filesystem while the window is unfocused, so
    /// on refocus they resynchronise by reporting the *entire* workspace as
    /// "changed" in one notification (hundreds of KiB of events).  Almost
    /// none of those files actually changed, and most were never parsed:
    /// PHPantom loads class details lazily, holding only a name→file pointer
    /// in the discovery index until something resolves the class.  Re-reading
    /// and re-scanning every reported file from disk would do thousands of
    /// wasted syscalls on every refocus.
    ///
    /// So a plain content change is only acted on for files we have actually
    /// parsed (whose cached details would otherwise go stale).  Created and
    /// deleted files are always handled: a creation makes a new class
    /// discoverable, and a deletion must purge a now-dangling entry, both of
    /// which matter even for files we never loaded.
    pub(crate) fn apply_watched_file_changes(
        &self,
        params: &DidChangeWatchedFilesParams,
        root: &std::path::Path,
    ) -> bool {
        let mut composer_changed = false;
        let mut schema_full_rebuild = false;
        let mut migration_changes: Vec<(PathBuf, FileChangeType)> = Vec::new();
        let mut php_changes: Vec<(String, PathBuf, FileChangeType)> = Vec::new();
        let mut migration_discovery =
            crate::virtual_members::laravel::database_schema::MigrationDiscovery::default();
        let is_laravel = self.resolved_class_cache.read().is_laravel();
        {
            let open = self.open_files.read();
            let parsed = self.parsed_uris.read();
            let laravel_config = self.config().laravel;
            for change in &params.changes {
                let path_str = change.uri.path();
                if path_str.ends_with("/composer.json") || path_str.ends_with("/composer.lock") {
                    composer_changed = true;
                    continue;
                }
                if is_laravel
                    && let Ok(file_path) = change.uri.to_file_path()
                    && crate::virtual_members::laravel::database_schema::SchemaIndex::watched_path_affects_schema(
                        root,
                        &laravel_config,
                        &file_path,
                    )
                {
                    if laravel_config.migrations.enabled()
                        && crate::virtual_members::laravel::database_schema::is_migration_php_file(
                            root,
                            &laravel_config.migrations,
                            &file_path,
                        )
                    {
                        // A deletion only ever removes a file the initial
                        // scan itself put in the plan, so it needs no
                        // discovery check -- and the file is gone from disk
                        // by now, so a walk could not find it anyway.
                        if change.typ == FileChangeType::DELETED
                            || migration_discovery.is_discoverable(
                                root,
                                &laravel_config.migrations,
                                &file_path,
                            )
                        {
                            migration_changes.push((file_path, change.typ));
                        }
                    } else {
                        schema_full_rebuild = true;
                    }
                    continue;
                }
                if !path_str.ends_with(".php") {
                    continue;
                }

                // Open files are already tracked via did_open/did_change.
                let uri_str = change.uri.to_string();
                if open.contains_key(&uri_str) {
                    continue;
                }
                let Ok(file_path) = change.uri.to_file_path() else {
                    continue;
                };

                if change.typ == FileChangeType::CHANGED {
                    // `parsed_uris` records the editor URI for open files and
                    // the canonical `file://` URI for lazily loaded ones;
                    // check both spellings.
                    let canonical_uri = crate::util::path_to_uri(&file_path);
                    let loaded =
                        parsed.contains(&uri_str) || parsed.contains(canonical_uri.as_str());
                    if !loaded {
                        continue;
                    }
                }

                php_changes.push((uri_str, file_path, change.typ));
            }
        }

        if php_changes.is_empty()
            && !composer_changed
            && !schema_full_rebuild
            && migration_changes.is_empty()
        {
            return false;
        }

        if !php_changes.is_empty() {
            tracing::info!(
                "PHPantom: {} watched PHP file(s) changed on disk, refreshing indexes",
                php_changes.len()
            );
            self.reindex_files_batch(&php_changes);
            // A class that was previously "not found" may now exist, and
            // resolved class info / member completions may be stale for a
            // class whose file changed.
            self.clear_class_not_found_cache();
            self.resolved_class_cache.write().clear();
            self.auth_user_type_cache.write().clear();
            *self.storage_disk_type_cache.write() = None;
            *self.laravel_aliases.write() = None;
            self.member_completion_cache.lock().clear();
        }

        if composer_changed {
            tracing::info!("PHPantom: composer files changed, rescanning vendor");
            self.rescan_composer_indexes(root);
        }

        if schema_full_rebuild {
            tracing::info!("PHPantom: Laravel schema files changed, reloading schema index");
            self.reload_laravel_schema_index(root);
        } else if !migration_changes.is_empty() {
            tracing::info!(
                "PHPantom: {} migration file(s) changed, incremental schema update",
                migration_changes.len()
            );
            self.update_laravel_migrations(&migration_changes);
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_laravel_projects_ignore_schema_watch_changes() {
        let dir = tempfile::tempdir().unwrap();
        let schema = dir.path().join("database/schema/default-schema.sql");
        std::fs::create_dir_all(schema.parent().unwrap()).unwrap();
        std::fs::write(&schema, "CREATE TABLE users (id bigint);").unwrap();

        let params = DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Url::from_file_path(&schema).unwrap(),
                typ: FileChangeType::CREATED,
            }],
        };

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(false);
        assert!(!backend.apply_watched_file_changes(&params, dir.path()));

        // The gate is on the project type alone: the same event in a
        // Laravel workspace still rebuilds the schema index.
        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        assert!(backend.apply_watched_file_changes(&params, dir.path()));
    }

    /// Deleting one of two files that declare the same class hands the
    /// name to the surviving file.  The purge used to drop every index
    /// entry the deleted file owned, so a class shipped in two variants
    /// behind a `class_exists` guard became unresolvable.
    #[test]
    fn deleting_one_of_two_declaring_files_keeps_the_class() {
        let backend = Backend::new_test();

        backend.update_ast(
            "file:///a_rich.php",
            "<?php namespace Vendor; class Variant { public function rich() {} }",
        );
        backend.update_ast(
            "file:///b_bare.php",
            "<?php namespace Vendor; class Variant { public function bare() {} }",
        );

        backend.reindex_files_batch(&[(
            "file:///a_rich.php".to_string(),
            PathBuf::from("/a_rich.php"),
            FileChangeType::DELETED,
        )]);

        assert_eq!(
            backend
                .symbols
                .fqn_uri_index
                .read()
                .get("Vendor\\Variant")
                .cloned(),
            Some("file:///b_bare.php".to_string()),
            "the surviving file should take over the name"
        );
        assert!(
            backend
                .symbols
                .fqn_class_index
                .read()
                .get("Vendor\\Variant")
                .is_some_and(|cls| cls.methods.iter().any(|m| m.name == "bare")),
            "the class index must describe the surviving declaration"
        );
        assert!(
            backend
                .symbols
                .method_store
                .read()
                .contains_key(&("Vendor\\Variant".to_string(), "bare".to_string())),
            "the method store must follow the surviving declaration"
        );
        assert!(
            !backend
                .symbols
                .method_store
                .read()
                .contains_key(&("Vendor\\Variant".to_string(), "rich".to_string())),
            "the deleted file's members must be gone"
        );
    }

    /// ...and the name still goes away once the last declaring file is
    /// deleted.
    #[test]
    fn deleting_the_last_declaring_file_drops_the_class() {
        let backend = Backend::new_test();

        backend.update_ast(
            "file:///a_rich.php",
            "<?php namespace Vendor; class Variant { public function rich() {} }",
        );
        backend.update_ast(
            "file:///b_bare.php",
            "<?php namespace Vendor; class Variant { public function bare() {} }",
        );

        backend.reindex_files_batch(&[
            (
                "file:///a_rich.php".to_string(),
                PathBuf::from("/a_rich.php"),
                FileChangeType::DELETED,
            ),
            (
                "file:///b_bare.php".to_string(),
                PathBuf::from("/b_bare.php"),
                FileChangeType::DELETED,
            ),
        ]);

        assert!(
            backend
                .symbols
                .fqn_uri_index
                .read()
                .get("Vendor\\Variant")
                .is_none(),
            "no file declares Vendor\\Variant any more"
        );
        assert!(
            backend
                .symbols
                .fqn_class_index
                .read()
                .get("Vendor\\Variant")
                .is_none(),
            "the class index must not outlive the last declaration"
        );
    }
}
