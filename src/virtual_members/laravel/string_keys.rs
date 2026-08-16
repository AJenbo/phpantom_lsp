//! Go-to-definition and find-references for Laravel string-key spans.
//!
//! Laravel encodes several kinds of navigable references as plain string
//! literals: `config('app.name')`, `view('emails.welcome')`,
//! `route('users.index')`, `__('messages.saved')`.  The symbol map records
//! these as [`crate::symbol_map::LaravelStringKey`] spans; this module turns
//! a span (kind + key) into concrete definition/reference [`Location`]s.

use super::{
    find_all_config_references, resolve_config_key_declaration,
    resolve_config_key_declaration_exact,
};
use super::{route_names, trans_keys, view_names};

use tower_lsp::lsp_types::Location;

/// Unified go-to-definition entry point for all Laravel string-key spans.
///
/// Dispatches on [`crate::symbol_map::LaravelStringKind`] so callers in
/// `definition/resolve.rs` only need one import and one call site.  Adding a
/// new Laravel navigation feature only requires a new match arm here, not a
/// new `pub(crate) use` in the parent module.
///
/// `uri` is the file the key was written in.  Most kinds name something the
/// project holds exactly one of and ignore it; a Blade section or stack name
/// only means anything relative to the template that wrote it, since its
/// other half is in whatever renders that template.
pub(crate) fn resolve_laravel_string_key(
    backend: &crate::Backend,
    kind: &crate::symbol_map::LaravelStringKind,
    key: &str,
    uri: &str,
) -> Vec<Location> {
    use crate::symbol_map::LaravelStringKind;
    match kind {
        LaravelStringKind::Section => {
            backend.blade_block_definitions(uri, crate::blade::blocks::BlockKind::Section, key)
        }
        LaravelStringKind::Stack => {
            backend.blade_block_definitions(uri, crate::blade::blocks::BlockKind::Stack, key)
        }
        LaravelStringKind::Config => {
            if let Some((resource, name)) =
                crate::symbol_map::laravel_resources::resource_from_config_key(key)
            {
                if let Some(location) = resolve_config_key_declaration_exact(backend, key) {
                    vec![location]
                } else {
                    source_definition_locations(
                        backend,
                        backend
                            .laravel_source_strings
                            .read()
                            .runtime_config_resource_definitions(resource, name),
                    )
                }
            } else {
                resolve_config_key_declaration(backend, key)
                    .into_iter()
                    .collect()
            }
        }
        LaravelStringKind::ConfigResource(resource) => {
            let config_key = crate::symbol_map::laravel_resources::config_key(*resource, key);
            if let Some(location) = resolve_config_key_declaration_exact(backend, &config_key) {
                vec![location]
            } else {
                source_definition_locations(
                    backend,
                    backend
                        .laravel_source_strings
                        .read()
                        .runtime_config_resource_definitions(*resource, key),
                )
            }
        }
        LaravelStringKind::View => view_names::resolve_view_definitions(backend, key),
        LaravelStringKind::Route => route_names::resolve_route_definitions(backend, key),
        LaravelStringKind::Trans => trans_keys::resolve_trans_definitions(backend, key),
        LaravelStringKind::Command => resolve_command_definition(backend, key)
            .into_iter()
            .collect(),
        LaravelStringKind::MorphAlias => resolve_morph_alias_definitions(backend, key),
        LaravelStringKind::GateAbility => resolve_gate_ability_definitions(backend, key),
        LaravelStringKind::ContainerBinding => resolve_container_binding_definitions(backend, key),
        LaravelStringKind::RateLimiter => source_definition_locations(
            backend,
            backend
                .laravel_source_strings
                .read()
                .rate_limiter_definitions(key),
        ),
        LaravelStringKind::QueueName => resolve_queue_name_definitions(backend, key),
    }
}

/// Resolve a container binding key to the service-provider registration that
/// bound it and to the class it resolves to.
///
/// The registration comes first, matching every other string kind: a config
/// key jumps to the config file, a morph alias to its `morphMap()` entry.  A
/// core alias the framework declares has no registration of its own, so the
/// bound class is the only answer it has.
fn resolve_container_binding_definitions(backend: &crate::Backend, key: &str) -> Vec<Location> {
    use tower_lsp::lsp_types::Url;

    let Some(target) = backend.container_binding_target(key) else {
        return Vec::new();
    };

    let mut locations = Vec::new();
    if let Some(site) = target.site
        && let Ok(parsed_uri) = Url::parse(&site.uri)
        && let Some(content) = backend.get_file_content(&site.uri)
    {
        let position = crate::text_position::offset_to_position(&content, site.offset as usize);
        locations.push(crate::definition::point_location(parsed_uri, position));
    }
    if let Some(location) = backend.class_declaration_location(&target.fqn) {
        locations.push(location);
    }
    locations
}

/// Resolve an authorization ability to every place it is declared: the
/// `Gate::define()` call that registers it, and the policy methods that
/// implement it.
///
/// A gate definition comes first — it applies to any model, so it is the
/// broadest answer — followed by each policy method, ordered by policy FQN so
/// the list is stable between requests.
fn resolve_gate_ability_definitions(backend: &crate::Backend, ability: &str) -> Vec<Location> {
    use tower_lsp::lsp_types::Url;

    let mut locations = Vec::new();

    let definition = backend
        .laravel_gates
        .read()
        .definition(ability)
        .map(|target| (target.uri.clone(), target.offset));
    if let Some((uri, offset)) = definition
        && let Ok(parsed_uri) = Url::parse(&uri)
        && let Some(content) = backend.get_file_content(&uri)
    {
        let position = crate::text_position::offset_to_position(&content, offset as usize);
        locations.push(crate::definition::point_location(parsed_uri, position));
    }

    for (policy, method) in super::gates::policy_methods_named(backend, ability) {
        if let Some(location) = policy_method_location(backend, &policy, &method) {
            crate::references::push_unique_location(
                &mut locations,
                &location.uri,
                location.range.start,
                location.range.end,
            );
        }
    }

    locations
}

/// The [`Location`] of a policy method's name token.
///
/// `policy` is the class that *declares* the method, so its `name_offset`
/// indexes that class's own file.  A synthesized member — an `@method` tag —
/// has no offset at all, and there the policy's declaration is the closest
/// thing to a location the ability has.
fn policy_method_location(
    backend: &crate::Backend,
    policy: &crate::types::ClassInfo,
    method: &crate::types::MethodInfo,
) -> Option<Location> {
    use tower_lsp::lsp_types::Url;

    let fqn = policy.fqn();
    let at_declaration = (method.name_offset != 0)
        .then(|| {
            let uri = backend
                .symbols
                .fqn_uri_index
                .read()
                .get(fqn.as_str())
                .cloned()?;
            let content = backend.get_file_content(&uri)?;
            let position =
                crate::text_position::offset_to_position(&content, method.name_offset as usize);
            Some(crate::definition::point_location(
                Url::parse(&uri).ok()?,
                position,
            ))
        })
        .flatten();

    at_declaration.or_else(|| backend.class_declaration_location(&fqn))
}

/// Resolve a morph alias to its `Relation::morphMap()` registration and to the
/// model it maps to.
///
/// The registration comes first, matching every other string kind (a config key
/// jumps to the config file, a view name to the template).  The mapped model
/// follows so that go-to-definition also offers the class the alias stands for.
fn resolve_morph_alias_definitions(backend: &crate::Backend, alias: &str) -> Vec<Location> {
    use tower_lsp::lsp_types::Url;

    let Some((fqn, uri, offset)) = backend
        .laravel_morph_map
        .read()
        .get(alias)
        .map(|target| (target.fqn.clone(), target.uri.clone(), target.offset))
    else {
        return Vec::new();
    };

    let mut locations = Vec::new();
    if let Ok(parsed_uri) = Url::parse(&uri)
        && let Some(content) = backend.get_file_content(&uri)
    {
        let position = crate::text_position::offset_to_position(&content, offset as usize);
        locations.push(crate::definition::point_location(parsed_uri, position));
    }
    if let Some(location) = backend.class_declaration_location(&fqn) {
        locations.push(location);
    }
    locations
}

/// Resolve an Artisan command name to the declaration site inside its
/// command class (the `$signature` / `$name` / `#[AsCommand]` literal).
fn resolve_command_definition(backend: &crate::Backend, name: &str) -> Option<Location> {
    use tower_lsp::lsp_types::Url;
    let index = backend.laravel_commands.read();
    let entry = index.get(name)?;
    let uri = Url::parse(&entry.uri).ok()?;
    let content = backend.get_file_content(&entry.uri)?;
    let position = crate::text_position::offset_to_position(&content, entry.name_offset as usize);
    Some(crate::definition::point_location(uri, position))
}

/// Unified find-references entry point for all Laravel string-key spans.
///
/// Dispatches on [`crate::symbol_map::LaravelStringKind`] — see
/// [`resolve_laravel_string_key`] for the same rationale.
pub(crate) fn find_laravel_string_key_references(
    backend: &crate::Backend,
    kind: &crate::symbol_map::LaravelStringKind,
    key: &str,
    uri: &str,
    snapshot: &[(String, std::sync::Arc<crate::symbol_map::SymbolMap>)],
    include_declaration: bool,
) -> Vec<Location> {
    use crate::symbol_map::LaravelStringKind;
    let mut locations = match kind {
        LaravelStringKind::Config | LaravelStringKind::ConfigResource(_) => {
            let mut locations =
                find_all_config_references(backend, kind, key, snapshot, include_declaration);
            let declarations = runtime_config_definition_locations(backend, kind, key);
            if include_declaration {
                for declaration in declarations {
                    crate::references::push_unique_location(
                        &mut locations,
                        &declaration.uri,
                        declaration.range.start,
                        declaration.range.end,
                    );
                }
            } else if !declarations.is_empty() {
                // A generic `config('…')` lookup and its `Config::set('…')`
                // declaration share the same symbol kind, so the indexed
                // usage scan sees both. Honour ReferenceContext by removing
                // the exact write locations when declarations were excluded.
                locations.retain(|location| !declarations.contains(location));
            }
            locations
        }
        // Two unrelated pages that both fill `content` fill two different
        // sections, so the span index's project-wide answer is the wrong
        // one: only the templates that render each other share a name.
        LaravelStringKind::Section => {
            return backend.blade_block_references(
                uri,
                crate::blade::blocks::BlockKind::Section,
                key,
            );
        }
        LaravelStringKind::Stack => {
            return backend.blade_block_references(
                uri,
                crate::blade::blocks::BlockKind::Stack,
                key,
            );
        }
        LaravelStringKind::View
        | LaravelStringKind::Route
        | LaravelStringKind::Trans
        | LaravelStringKind::Command
        | LaravelStringKind::MorphAlias
        | LaravelStringKind::GateAbility
        | LaravelStringKind::ContainerBinding
        | LaravelStringKind::RateLimiter
        | LaravelStringKind::QueueName => {
            find_string_key_usages(kind, key, backend, snapshot, include_declaration)
        }
    };

    if include_declaration && !kind.is_config_backed() {
        for decl in resolve_laravel_string_key(backend, kind, key, uri) {
            crate::references::push_unique_location(
                &mut locations,
                &decl.uri,
                decl.range.start,
                decl.range.end,
            );
        }
    }

    locations
}

/// Runtime config-resource declarations are indexed independently of symbol
/// maps so they remain navigable after their source file is closed. Both the
/// full config key and its resource-specific spelling address the same entry.
fn runtime_config_definition_locations(
    backend: &crate::Backend,
    kind: &crate::symbol_map::LaravelStringKind,
    key: &str,
) -> Vec<Location> {
    let resource = match kind {
        crate::symbol_map::LaravelStringKind::Config => {
            crate::symbol_map::laravel_resources::resource_from_config_key(key)
        }
        crate::symbol_map::LaravelStringKind::ConfigResource(resource) => Some((*resource, key)),
        _ => None,
    };
    let Some((resource, name)) = resource else {
        return Vec::new();
    };
    let definitions = backend
        .laravel_source_strings
        .read()
        .runtime_config_resource_definitions(resource, name);
    source_definition_locations(backend, definitions)
}

/// Scan pre-built [`crate::symbol_map::SymbolMap`] spans for all call sites
/// matching `kind` + `key` — zero file re-parses, O(total spans) memory walk.
fn find_string_key_usages(
    kind: &crate::symbol_map::LaravelStringKind,
    key: &str,
    backend: &crate::Backend,
    snapshot: &[(String, std::sync::Arc<crate::symbol_map::SymbolMap>)],
    include_declaration: bool,
) -> Vec<Location> {
    use crate::references::push_unique_location;
    use crate::text_position::offset_to_position;
    use tower_lsp::lsp_types::Url;

    let mut locations = Vec::new();
    for (file_uri, symbol_map) in snapshot {
        // A render site whose receiver only a type settles is not in the
        // map, so ask for the file's confirmed extras — but only when a
        // candidate names this very key, so the type resolution is paid
        // for the handful of files that could contribute a hit.
        let has_candidate = symbol_map
            .view_receiver_sites
            .iter()
            .any(|site| *kind == crate::symbol_map::LaravelStringKind::View && site.key == key)
            || symbol_map
                .resource_receiver_sites
                .iter()
                .any(|site| site.key == key && site.rule.candidate_kinds().contains(kind));
        let extra =
            has_candidate.then(|| backend.typed_receiver_view_spans_for(file_uri, symbol_map));

        // First pass: check if this file even has ANY LaravelStringKey matches.
        // This avoids reading file content from disk for thousands of unrelated files.
        let has_match = symbol_map
            .spans
            .iter()
            .chain(extra.iter().flat_map(|spans| spans.iter()))
            .any(|span| string_key_span_matches(span, kind, key, include_declaration));

        if !has_match {
            continue;
        }

        let Ok(parsed_uri) = Url::parse(file_uri) else {
            continue;
        };
        let Some(content) = backend.get_file_content_arc(file_uri) else {
            continue;
        };
        for span in symbol_map
            .spans
            .iter()
            .chain(extra.iter().flat_map(|spans| spans.iter()))
        {
            if string_key_span_matches(span, kind, key, include_declaration) {
                let start = offset_to_position(&content, span.start as usize);
                let end = offset_to_position(&content, span.end as usize);
                push_unique_location(&mut locations, &parsed_uri, start, end);
            }
        }
    }
    locations
}

fn string_key_span_matches(
    span: &crate::symbol_map::SymbolSpan,
    kind: &crate::symbol_map::LaravelStringKind,
    key: &str,
    include_declaration: bool,
) -> bool {
    matches!(
        &span.kind,
        crate::symbol_map::SymbolKind::LaravelStringKey {
            kind: span_kind,
            key: span_key,
            is_write,
            ..
        } if span_kind == kind
            && span_key == key
            && (include_declaration
                || *kind != crate::symbol_map::LaravelStringKind::RateLimiter
                || !is_write)
    )
}

/// Materialize the lazy typed queue-name index once per workspace generation,
/// then resolve this and subsequent names with one ordered-map lookup.
fn resolve_queue_name_definitions(backend: &crate::Backend, key: &str) -> Vec<Location> {
    resolve_queue_name_definitions_with(backend, key, |backend| {
        for (uri, map) in backend.user_file_symbol_maps() {
            if map
                .resource_receiver_sites
                .iter()
                .any(|site| site.rule == crate::symbol_map::LaravelResourceReceiverRule::QueueName)
            {
                backend.typed_receiver_view_spans_for(&uri, &map);
            }
        }
    })
}

fn resolve_queue_name_definitions_with(
    backend: &crate::Backend,
    key: &str,
    mut materialize_candidates: impl FnMut(&crate::Backend),
) -> Vec<Location> {
    loop {
        let generation = {
            let index = backend.laravel_source_strings.read();
            if index.queue_names_are_complete() {
                let definitions = index.queue_name_definitions(key);
                drop(index);
                return source_definition_locations(backend, definitions);
            }
            index.queue_name_generation()
        };
        materialize_candidates(backend);
        let mut index = backend.laravel_source_strings.write();
        let definitions = complete_queue_name_scan(&mut index, generation, key);
        drop(index);
        if let Some(definitions) = definitions {
            return source_definition_locations(backend, definitions);
        }
    }
}

/// Atomically publish a typed queue scan and read its result. A generation
/// change rejects the whole attempt so the caller can rescan current maps.
fn complete_queue_name_scan(
    index: &mut crate::laravel_string_index::LaravelSourceStringIndex,
    generation: u64,
    key: &str,
) -> Option<Vec<crate::laravel_string_index::LaravelSourceStringDefinition>> {
    index
        .mark_queue_names_complete(generation)
        .then(|| index.queue_name_definitions(key))
}

fn source_definition_locations(
    backend: &crate::Backend,
    definitions: Vec<crate::laravel_string_index::LaravelSourceStringDefinition>,
) -> Vec<Location> {
    let mut locations = Vec::with_capacity(definitions.len());
    append_source_definitions(backend, &mut locations, definitions);
    locations
}

fn append_source_definitions(
    backend: &crate::Backend,
    locations: &mut Vec<Location>,
    definitions: Vec<crate::laravel_string_index::LaravelSourceStringDefinition>,
) {
    use crate::references::push_unique_location;
    use crate::text_position::offset_to_position;
    use tower_lsp::lsp_types::Url;

    let mut current_uri: Option<std::sync::Arc<str>> = None;
    let mut current_source: Option<(Url, std::sync::Arc<String>)> = None;
    for definition in definitions {
        if current_uri.as_deref() != Some(definition.uri.as_ref()) {
            current_source = Url::parse(&definition.uri)
                .ok()
                .zip(backend.get_file_content_arc(&definition.uri));
            current_uri = Some(std::sync::Arc::clone(&definition.uri));
        }
        let Some((uri, content)) = &current_source else {
            continue;
        };
        push_unique_location(
            locations,
            uri,
            offset_to_position(content, definition.start as usize),
            offset_to_position(content, definition.end as usize),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::symbol_map::{
        LaravelConfigResource, LaravelStringKind, SymbolMap, extract_symbol_map,
    };

    fn map(source: &str) -> Arc<SymbolMap> {
        Arc::new(crate::parser::with_parsed_program(
            source,
            "laravel_string_key_references",
            extract_symbol_map,
        ))
    }

    #[test]
    fn full_config_resource_keys_prefer_files_then_runtime_writes() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config/cache.php");
        let runtime = dir.path().join("bootstrap/resources.php");
        std::fs::create_dir_all(config.parent().unwrap()).unwrap();
        std::fs::create_dir_all(runtime.parent().unwrap()).unwrap();
        std::fs::write(
            &config,
            "<?php return ['stores' => ['redis' => ['driver' => 'redis']]];\n",
        )
        .unwrap();
        let runtime_source = "<?php Config::set('cache.stores.tenant', []);\n";
        std::fs::write(&runtime, runtime_source).unwrap();

        let backend = crate::Backend::new_test();
        *backend.workspace.workspace_root.write() = Some(dir.path().to_path_buf());
        let runtime_uri = crate::util::path_to_uri(&runtime);
        backend.update_ast(runtime_uri.as_str(), runtime_source);

        let exact = resolve_laravel_string_key(
            &backend,
            &LaravelStringKind::Config,
            "cache.stores.redis",
            runtime_uri.as_str(),
        );
        assert_eq!(exact.len(), 1);
        assert_eq!(
            exact[0].uri,
            tower_lsp::lsp_types::Url::from_file_path(config).unwrap()
        );

        let runtime_locations = resolve_laravel_string_key(
            &backend,
            &LaravelStringKind::Config,
            "cache.stores.tenant",
            runtime_uri.as_str(),
        );
        assert_eq!(runtime_locations.len(), 1);
        assert_eq!(runtime_locations[0].uri.as_str(), runtime_uri.as_str());
    }

    #[test]
    fn scoped_blade_reference_kinds_return_their_local_results_directly() {
        let backend = crate::Backend::new_test();
        let uri = "file:///template.blade.php";

        assert!(
            find_laravel_string_key_references(
                &backend,
                &LaravelStringKind::Section,
                "content",
                uri,
                &[],
                false,
            )
            .is_empty()
        );
        assert!(
            find_laravel_string_key_references(
                &backend,
                &LaravelStringKind::Stack,
                "scripts",
                uri,
                &[],
                false,
            )
            .is_empty()
        );
    }

    #[test]
    fn usage_scan_skips_nonmatches_invalid_uris_and_missing_contents() {
        let backend = crate::Backend::new_test();
        let route = LaravelStringKind::Route;

        assert!(
            find_string_key_usages(
                &route,
                "target",
                &backend,
                &[(
                    "file:///other.php".to_string(),
                    map("<?php route('other');")
                )],
                true,
            )
            .is_empty()
        );
        assert!(
            find_string_key_usages(
                &route,
                "target",
                &backend,
                &[("not a URI".to_string(), map("<?php route('target');"))],
                true,
            )
            .is_empty()
        );
        assert!(
            find_string_key_usages(
                &route,
                "target",
                &backend,
                &[(
                    "file:///missing-route.php".to_string(),
                    map("<?php route('target');"),
                )],
                true,
            )
            .is_empty()
        );
    }

    #[test]
    fn rate_limiter_usage_scan_can_exclude_the_registration() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("routes.php");
        let source =
            "<?php RateLimiter::for('api', fn () => null); Route::middleware('throttle:api');\n";
        std::fs::write(&source_path, source).unwrap();
        let uri = crate::util::path_to_uri(&source_path);
        let snapshot = [(uri.to_string(), map(source))];

        let locations = find_string_key_usages(
            &LaravelStringKind::RateLimiter,
            "api",
            &crate::Backend::new_test(),
            &snapshot,
            false,
        );

        assert_eq!(locations.len(), 1);
        assert!(
            find_string_key_usages(
                &LaravelStringKind::RateLimiter,
                "api",
                &crate::Backend::new_test(),
                &[(
                    "file:///missing-registration-only.php".to_string(),
                    map("<?php RateLimiter::for('api', fn () => null);"),
                )],
                false,
            )
            .is_empty()
        );
    }

    #[test]
    fn duplicate_source_definitions_in_one_file_all_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("app/Providers/RateLimitProvider.php");
        let source = "<?php RateLimiter::for('api', fn () => null); RateLimiter::for('api', fn () => null);\n";
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, source).unwrap();

        let backend = crate::Backend::new_test();
        let uri = crate::util::path_to_uri(&source_path);
        backend.update_ast(uri.as_str(), source);

        let locations = resolve_laravel_string_key(
            &backend,
            &LaravelStringKind::RateLimiter,
            "api",
            uri.as_str(),
        );
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].range.start.line, locations[1].range.start.line);
        assert!(locations[0].range.start.character < locations[1].range.start.character);
    }

    #[test]
    fn queue_definition_scan_materializes_candidate_files_once() {
        let dir = tempfile::tempdir().unwrap();
        let source_path = dir.path().join("app/Job.php");
        let source = "<?php $job->onQueue('high');\n";
        std::fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        std::fs::write(&source_path, source).unwrap();

        let backend = crate::Backend::new_test();
        *backend.workspace.workspace_root.write() = Some(dir.path().to_path_buf());
        let uri = crate::util::path_to_uri(&source_path);
        backend.update_ast(uri.as_str(), source);

        assert!(
            resolve_laravel_string_key(
                &backend,
                &LaravelStringKind::QueueName,
                "high",
                uri.as_str(),
            )
            .is_empty()
        );
        assert!(
            backend
                .laravel_source_strings
                .read()
                .queue_names_are_complete()
        );
        assert!(
            resolve_laravel_string_key(
                &backend,
                &LaravelStringKind::QueueName,
                "high",
                uri.as_str(),
            )
            .is_empty()
        );
    }

    #[test]
    fn queue_scan_publication_rejects_stale_generations() {
        let backend = crate::Backend::new_test();
        let mut scans = 0;
        let locations = resolve_queue_name_definitions_with(&backend, "high", |backend| {
            scans += 1;
            if scans == 1 {
                let mut candidate_map = SymbolMap::default();
                candidate_map.resource_receiver_sites.push(
                    crate::symbol_map::LaravelResourceReceiverSite {
                        start: 1,
                        end: 5,
                        key: "high".to_string(),
                        rule: crate::symbol_map::LaravelResourceReceiverRule::QueueName,
                    },
                );
                backend
                    .laravel_source_strings
                    .write()
                    .set_symbol_map_contributions(
                    "file:///job.php",
                    crate::laravel_string_index::LaravelSourceStringContributions::from_symbol_map(
                        &candidate_map,
                    ),
                );
            }
        });

        assert!(locations.is_empty());
        assert_eq!(scans, 2, "a stale publication must retry the scan");
        assert!(
            backend
                .laravel_source_strings
                .read()
                .queue_names_are_complete()
        );
    }

    #[test]
    fn runtime_definition_and_location_helpers_reject_unusable_inputs() {
        let backend = crate::Backend::new_test();
        assert!(
            runtime_config_definition_locations(&backend, &LaravelStringKind::RateLimiter, "api")
                .is_empty()
        );

        let mut locations = Vec::new();
        append_source_definitions(
            &backend,
            &mut locations,
            vec![
                crate::laravel_string_index::LaravelSourceStringDefinition {
                    uri: Arc::from("not a URI"),
                    start: 0,
                    end: 1,
                },
                crate::laravel_string_index::LaravelSourceStringDefinition {
                    uri: Arc::from("file:///missing-source.php"),
                    start: 0,
                    end: 1,
                },
            ],
        );
        assert!(locations.is_empty());

        assert!(
            resolve_laravel_string_key(
                &backend,
                &LaravelStringKind::ConfigResource(LaravelConfigResource::CacheStore),
                "missing",
                "file:///usage.php",
            )
            .is_empty()
        );
    }
}
