//! Watched-file change application.
//!
//! Applies a `workspace/didChangeWatchedFiles` batch to the symbol
//! indexes on a blocking thread.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::*;

use crate::Backend;

struct PendingPhpChange {
    uri: String,
    path: PathBuf,
    typ: FileChangeType,
    loaded: bool,
}

struct LaravelPhpRefresh {
    change_index: usize,
    content: String,
}

/// Compare a watched path with a path saved by discovery.  The raw spelling
/// is the overwhelmingly common fast path; the fallback covers macOS' alias
/// paths and a deleted file whose parent still exists.
fn paths_refer_to_same_file(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    comparable_path(left)
        .is_some_and(|left| comparable_path(right).is_some_and(|right| left == right))
}

fn comparable_path(path: &Path) -> Option<PathBuf> {
    path.canonicalize().ok().or_else(|| {
        let parent = path.parent()?.canonicalize().ok()?;
        Some(parent.join(path.file_name()?))
    })
}

fn relative_path<'a>(path: &'a Path, directory: &Path) -> Option<Cow<'a, Path>> {
    path.strip_prefix(directory)
        .map(Cow::Borrowed)
        .ok()
        .or_else(|| {
            let path = comparable_path(path)?;
            let directory = comparable_path(directory)?;
            path.strip_prefix(directory)
                .map(|relative| Cow::Owned(relative.to_path_buf()))
                .ok()
        })
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    memchr::memmem::find(haystack, needle).is_some()
        || haystack
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

/// Whether a PHP file can contribute one of the source-defined resource
/// names.  Imports keep the original `Config` / `RateLimiter` basename even
/// when the call uses an alias, so these cheap token pairs avoid a full parse
/// for ordinary watched-file noise without losing aliased registrations.
fn has_laravel_source_name_tokens(content: &str) -> bool {
    let bytes = content.as_bytes();
    contains_ascii_case_insensitive(bytes, b"onQueue")
        || (contains_ascii_case_insensitive(bytes, b"Config")
            && contains_ascii_case_insensitive(bytes, b"set"))
        || (contains_ascii_case_insensitive(bytes, b"RateLimiter")
            && contains_ascii_case_insensitive(bytes, b"for"))
}

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
    /// So a plain content change is normally only acted on for files we have
    /// actually parsed (whose cached details would otherwise go stale).
    /// Laravel config files are the narrow exception because their workspace
    /// index must stay current before any class lookup opens them. Created and
    /// deleted files are always handled: a creation makes a new class
    /// discoverable (and newly-created project files are cheaply probed for
    /// source-name registrations), while a deletion must purge a now-dangling
    /// entry. Both matter even for files we never loaded.
    pub(crate) fn apply_watched_file_changes(
        &self,
        params: &DidChangeWatchedFilesParams,
        root: &std::path::Path,
    ) -> bool {
        let mut composer_changed = false;
        let mut schema_full_rebuild = false;
        let mut migration_changes: Vec<(PathBuf, FileChangeType)> = Vec::new();
        let mut php_candidates: Vec<PendingPhpChange> = Vec::new();
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

                let loaded = if change.typ == FileChangeType::CHANGED {
                    // `parsed_uris` records the editor URI for open files and
                    // the canonical `file://` URI for lazily loaded ones;
                    // check both spellings.
                    let canonical_uri = crate::util::path_to_uri(&file_path);
                    parsed.contains(&uri_str) || parsed.contains(canonical_uri.as_str())
                } else {
                    false
                };

                php_candidates.push(PendingPhpChange {
                    uri: uri_str,
                    path: file_path,
                    typ: change.typ,
                    loaded,
                });
            }
        }

        let provider_config_paths = if is_laravel && !php_candidates.is_empty() {
            self.laravel_provider_resources
                .read()
                .config_files
                .iter()
                .map(|resource| resource.path.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let vendor_paths = if is_laravel && !php_candidates.is_empty() {
            self.workspace.vendor_dir_paths.lock().clone()
        } else {
            Vec::new()
        };
        let mut php_changes: Vec<(String, PathBuf, FileChangeType)> = Vec::new();
        let mut laravel_refreshes: Vec<LaravelPhpRefresh> = Vec::new();
        let mut config_invalidations: Vec<(usize, bool)> = Vec::new();

        for candidate in php_candidates {
            let is_provider_config = is_laravel
                && provider_config_paths
                    .iter()
                    .filter(|path| path.file_name() == candidate.path.file_name())
                    .any(|path| paths_refer_to_same_file(&candidate.path, path));
            let relative = relative_path(&candidate.path, root);
            let is_conventional_config = relative
                .as_deref()
                .is_some_and(|relative| relative.starts_with("config"));
            let is_in_conventional_vendor = relative
                .as_deref()
                .is_some_and(|relative| relative.starts_with("vendor"));
            let is_config = is_laravel && (is_provider_config || is_conventional_config);
            let is_project_source = is_laravel
                && relative.is_some()
                && !is_in_conventional_vendor
                && !vendor_paths
                    .iter()
                    .any(|path| relative_path(&candidate.path, path).is_some());
            // A loaded dependency may already contribute source names and
            // therefore needs a replacement after the destructive batch
            // withdrawal.  Newly-created dependency trees stay on the cheap
            // discovery-only path unless the exact file is a registered
            // config resource.
            let should_probe_source =
                candidate.loaded || (is_project_source && candidate.typ == FileChangeType::CREATED);
            let content =
                if candidate.typ != FileChangeType::DELETED && (is_config || should_probe_source) {
                    std::fs::read_to_string(&candidate.path).ok()
                } else {
                    None
                };
            let has_source_names = content
                .as_deref()
                .is_some_and(has_laravel_source_name_tokens);

            // Preserve the refocus optimisation for untouched, unparsed PHP:
            // only config resources and files that can define Laravel names
            // bypass the normal loaded-file gate.
            if candidate.typ == FileChangeType::CHANGED
                && !candidate.loaded
                && !is_config
                && !has_source_names
            {
                continue;
            }

            let change_index = php_changes.len();
            php_changes.push((candidate.uri, candidate.path, candidate.typ));

            if candidate.typ == FileChangeType::DELETED || (is_config && content.is_none()) {
                if is_config {
                    config_invalidations.push((change_index, is_provider_config));
                }
                continue;
            }
            if (is_config || has_source_names)
                && let Some(content) = content
            {
                laravel_refreshes.push(LaravelPhpRefresh {
                    change_index,
                    content,
                });
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

            // `reindex_files_batch` deliberately removes stale per-file
            // source names. Rebuild only the files whose lexical gate says
            // they can contribute one, plus config resources whose cache
            // generation must advance even when their PHP has no such token.
            for refresh in laravel_refreshes {
                let uri = &php_changes[refresh.change_index].0;
                self.update_ast(uri, &refresh.content);
                if let Some(map) = self.symbol_map_for(uri)
                    && map.resource_receiver_sites.iter().any(|site| {
                        site.rule == crate::symbol_map::LaravelResourceReceiverRule::QueueName
                    })
                {
                    self.typed_receiver_view_spans_for(uri, &map);
                }
            }
            if !config_invalidations.is_empty() {
                let mut cache = self.laravel_string_key_cache.write();
                for (change_index, is_provider_config) in config_invalidations {
                    cache.invalidate_for_uri(&php_changes[change_index].0, "", is_provider_config);
                }
            }
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

    fn watched_change(path: &Path, typ: FileChangeType) -> DidChangeWatchedFilesParams {
        DidChangeWatchedFilesParams {
            changes: vec![FileEvent {
                uri: Url::from_file_path(path).unwrap(),
                typ,
            }],
        }
    }

    #[test]
    fn path_comparisons_handle_lexical_aliases_and_deleted_files() {
        let dir = tempfile::tempdir().unwrap();
        let detour = dir.path().join("detour");
        std::fs::create_dir(&detour).unwrap();
        let file = dir.path().join("resource.php");
        std::fs::write(&file, "<?php").unwrap();
        let aliased_file = detour.join("..").join("resource.php");

        assert!(paths_refer_to_same_file(&aliased_file, &file));
        assert_eq!(
            comparable_path(&dir.path().join("deleted.php")),
            Some(dir.path().canonicalize().unwrap().join("deleted.php"))
        );
        assert_eq!(
            relative_path(&file, &detour.join("..")).as_deref(),
            Some(Path::new("resource.php"))
        );
    }

    #[test]
    fn source_name_token_gate_is_ascii_case_insensitive() {
        assert!(has_laravel_source_name_tokens(
            "<?php ratelimiter::FOR('api', fn () => null);"
        ));
        assert!(!has_laravel_source_name_tokens(
            "<?php final class RateLimiterFactory {}"
        ));
    }

    #[test]
    fn lexical_workspace_root_aliases_still_classify_project_sources() {
        let dir = tempfile::tempdir().unwrap();
        let detour = dir.path().join("detour");
        let source = dir.path().join("app/Service.php");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::create_dir(&detour).unwrap();
        std::fs::write(&source, "<?php final class Service {}\n").unwrap();

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        assert!(backend.apply_watched_file_changes(
            &watched_change(&source, FileChangeType::CREATED),
            &detour.join(".."),
        ));
    }

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

    #[test]
    fn changed_unparsed_config_file_invalidates_config_cache() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config/cache.php");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, "<?php return ['stores' => ['redis' => []]];").unwrap();

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        {
            let mut cache = backend.laravel_string_key_cache.write();
            cache.config_generation = 7;
            cache.config_keys = Some(std::sync::Arc::new(vec!["old.key".to_string()]));
        }

        assert!(backend.apply_watched_file_changes(
            &watched_change(&config, FileChangeType::CHANGED),
            dir.path(),
        ));
        let cache = backend.laravel_string_key_cache.read();
        assert_eq!(cache.config_generation, 8);
        assert!(cache.config_keys.is_none());
    }

    #[test]
    fn registered_config_file_outside_config_is_refreshed_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("package/resources/settings.php");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::write(&config, "<?php return ['stores' => ['tenant' => []]];").unwrap();

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        backend
            .laravel_provider_resources
            .write()
            .config_files
            .push(crate::virtual_members::laravel::ProviderResource {
                path: config.clone(),
                namespace: "package".to_string(),
            });
        {
            let mut cache = backend.laravel_string_key_cache.write();
            cache.config_generation = 12;
            cache.config_keys = Some(std::sync::Arc::new(vec!["old.key".to_string()]));
        }

        assert!(backend.apply_watched_file_changes(
            &watched_change(&config, FileChangeType::CHANGED),
            dir.path(),
        ));
        {
            let cache = backend.laravel_string_key_cache.read();
            assert_eq!(cache.config_generation, 13);
            assert!(cache.config_keys.is_none());
        }

        {
            let mut cache = backend.laravel_string_key_cache.write();
            cache.config_keys = Some(std::sync::Arc::new(vec!["package.stores".to_string()]));
        }
        std::fs::remove_file(&config).unwrap();
        assert!(backend.apply_watched_file_changes(
            &watched_change(&config, FileChangeType::DELETED),
            dir.path(),
        ));
        let cache = backend.laravel_string_key_cache.read();
        assert_eq!(cache.config_generation, 14);
        assert!(cache.config_keys.is_none());
    }

    #[test]
    fn watched_aliased_runtime_registrations_replace_source_definitions() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("app/Providers/ResourceProvider.php");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            r#"<?php
use Illuminate\Support\Facades\Config as Settings;
use Illuminate\Support\Facades\RateLimiter as Limits;
Settings::set('cache.stores.tenant', []);
Limits::for('api', fn () => null);
"#,
        )
        .unwrap();

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        let uri = crate::util::path_to_uri(&source);

        assert!(backend.apply_watched_file_changes(
            &watched_change(&source, FileChangeType::CREATED),
            dir.path(),
        ));
        {
            let names = backend.laravel_source_strings.read();
            assert_eq!(names.rate_limiter_definitions("api").len(), 1);
            assert_eq!(
                names
                    .runtime_config_resource_definitions(
                        crate::symbol_map::LaravelConfigResource::CacheStore,
                        "tenant",
                    )
                    .len(),
                1
            );
        }
        assert!(backend.symbol_map_for(&uri).is_some());

        std::fs::write(&source, "<?php final class ResourceProvider {}\n").unwrap();
        assert!(backend.apply_watched_file_changes(
            &watched_change(&source, FileChangeType::CHANGED),
            dir.path(),
        ));
        let names = backend.laravel_source_strings.read();
        assert!(names.rate_limiter_definitions("api").is_empty());
        assert!(
            names
                .runtime_config_resource_definitions(
                    crate::symbol_map::LaravelConfigResource::CacheStore,
                    "tenant",
                )
                .is_empty()
        );
    }

    #[test]
    fn watched_queue_names_are_reconfirmed_against_the_new_symbol_map() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("app/Jobs/ReportJob.php");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        let source_for = |queue: &str| {
            format!(
                r#"<?php
namespace Illuminate\Contracts\Queue {{ interface ShouldQueue {{}} }}
namespace App\Jobs {{
    final class ReportJob implements \Illuminate\Contracts\Queue\ShouldQueue {{
        public function dispatch(): void {{ $this->onQueue('{queue}'); }}
        public function onQueue(string $queue): self {{ return $this; }}
    }}
}}
"#
            )
        };
        std::fs::write(&source, source_for("high")).unwrap();

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        assert!(backend.apply_watched_file_changes(
            &watched_change(&source, FileChangeType::CREATED),
            dir.path(),
        ));
        assert_eq!(
            backend
                .laravel_source_strings
                .read()
                .queue_name_definitions("high")
                .len(),
            1
        );

        std::fs::write(&source, source_for("low")).unwrap();
        assert!(backend.apply_watched_file_changes(
            &watched_change(&source, FileChangeType::CHANGED),
            dir.path(),
        ));
        let names = backend.laravel_source_strings.read();
        assert!(names.queue_name_definitions("high").is_empty());
        assert_eq!(names.queue_name_definitions("low").len(), 1);
    }

    #[test]
    fn created_vendor_source_is_not_fully_parsed_for_laravel_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let vendor = dir.path().join("vendor");
        let source = vendor.join("package/src/Provider.php");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            "<?php RateLimiter::for('vendor-api', fn () => null); class Provider {}\n",
        )
        .unwrap();

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        backend.add_vendor_dir(&vendor);
        let uri = crate::util::path_to_uri(&source);

        assert!(backend.apply_watched_file_changes(
            &watched_change(&source, FileChangeType::CREATED),
            dir.path(),
        ));
        assert!(backend.symbol_map_for(&uri).is_none());
        assert!(
            backend
                .laravel_source_strings
                .read()
                .rate_limiter_definitions("vendor-api")
                .is_empty()
        );
    }

    #[test]
    fn unparsed_changed_source_keeps_the_refocus_fast_path() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("app/Providers/UnloadedProvider.php");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(
            &source,
            "<?php RateLimiter::for('unloaded', fn () => null); class UnloadedProvider {}\n",
        )
        .unwrap();

        let backend = Backend::new_test();
        backend.resolved_class_cache.write().set_laravel(true);
        let uri = crate::util::path_to_uri(&source);

        assert!(!backend.apply_watched_file_changes(
            &watched_change(&source, FileChangeType::CHANGED),
            dir.path(),
        ));
        assert!(backend.symbol_map_for(&uri).is_none());
        assert!(
            backend
                .laravel_source_strings
                .read()
                .rate_limiter_definitions("unloaded")
                .is_empty()
        );
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
