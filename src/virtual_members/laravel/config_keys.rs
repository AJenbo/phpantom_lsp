use std::sync::Arc;

use mago_allocator::LocalArena;
use mago_database::file::FileId;
use mago_syntax::cst::*;
use tower_lsp::lsp_types::{Location, Position, Url};

use crate::Backend;
use crate::references::push_unique_location;
use crate::symbol_map::{LaravelStringKind, SymbolKind, SymbolMap, SymbolSpan};
use crate::text_position::offset_to_position;

#[derive(Debug)]
pub(crate) struct ConfigKeyMatch {
    pub key: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug)]
pub(crate) struct ConfigFileScan {
    pub declarations: Vec<ConfigKeyMatch>,
    /// Dot prefixes whose child keys are supplied by an expression that
    /// cannot be enumerated statically (spread, variable, conditional, etc.).
    pub open_prefixes: Vec<String>,
}

/// Try to determine the dot-notated configuration prefix for a given file URI.
///
/// For example, `file:///path/to/project/config/app.php` returns `Some("app")`.
/// Supports nested directories: `config/api/keys.php` returns `Some("api.keys")`.
pub(crate) fn laravel_config_prefix_from_uri(uri: &str) -> Option<String> {
    let parsed = Url::parse(uri).ok()?;
    let path = parsed.path();
    // Match the nearest `config` directory to the file path. This avoids
    // false negatives when an ancestor directory is also named `config`.
    let (_, relative) = path.rsplit_once("/config/")?;
    let stem = relative.strip_suffix(".php")?;
    if stem.is_empty() || stem.ends_with('/') {
        return None;
    }
    Some(stem.replace('/', "."))
}

/// Collect Laravel config declaration keys from a `config/*.php` file.
///
/// Produces keys in dot notation (`app.mail.from.address`) and records
/// source spans for the key literal content (inside quotes).
pub(crate) fn collect_laravel_config_declarations(
    content: &str,
    prefix: &str,
) -> Vec<ConfigKeyMatch> {
    scan_laravel_config_file(content, prefix).declarations
}

/// Parse one config file once, collecting both known declaration keys and
/// subtrees whose runtime children are unknowable.
pub(crate) fn scan_laravel_config_file(content: &str, prefix: &str) -> ConfigFileScan {
    let arena = LocalArena::new();
    let file_id = FileId::new(b"input.php");
    let program = mago_syntax::parser::parse_file_content(&arena, file_id, content.as_bytes());
    let mut out = Vec::new();
    let mut open_prefixes = Vec::new();

    let mut returned_var_name: Option<&[u8]> = None;
    let mut return_expr: Option<&Expression<'_>> = None;

    for stmt in program.statements.iter() {
        if let Statement::Return(ret) = stmt {
            if let Some(val) = ret.value {
                match val {
                    Expression::Variable(Variable::Direct(dv)) => {
                        returned_var_name = Some(dv.name);
                    }
                    _ => {
                        return_expr = Some(val);
                    }
                }
            }
            break;
        }
    }

    let mut path = String::new();
    if let Some(expr) = return_expr {
        collect_expr_declarations(
            expr,
            content,
            prefix,
            &mut path,
            &mut out,
            &mut open_prefixes,
        );
    } else if let Some(var_name) = returned_var_name {
        for stmt in program.statements.iter() {
            if let Statement::Expression(expr_stmt) = stmt
                && let Expression::Assignment(assign) = expr_stmt.expression
                && let Expression::Variable(Variable::Direct(dv)) = assign.lhs
                && dv.name == var_name
            {
                collect_expr_declarations(
                    assign.rhs,
                    content,
                    prefix,
                    &mut path,
                    &mut out,
                    &mut open_prefixes,
                );
            }
        }
    }

    open_prefixes.sort();
    open_prefixes.dedup();
    ConfigFileScan {
        declarations: out,
        open_prefixes,
    }
}

// ─── Declaration walker ───────────────────────────────────────────────────────

fn collect_expr_declarations(
    expr: &Expression<'_>,
    content: &str,
    prefix: &str,
    path: &mut String,
    out: &mut Vec<ConfigKeyMatch>,
    open_prefixes: &mut Vec<String>,
) {
    match expr {
        Expression::Array(arr) => {
            collect_array_declarations(
                arr.elements.iter(),
                content,
                prefix,
                path,
                out,
                open_prefixes,
            );
        }
        Expression::LegacyArray(arr) => {
            collect_array_declarations(
                arr.elements.iter(),
                content,
                prefix,
                path,
                out,
                open_prefixes,
            );
        }
        Expression::Parenthesized(p) => {
            collect_expr_declarations(p.expression, content, prefix, path, out, open_prefixes);
        }
        Expression::Call(Call::Function(fc))
            if matches!(fc.function, Expression::Identifier(ident)
                if ident.value().eq_ignore_ascii_case(b"array_merge")) =>
        {
            for arg in fc.argument_list.arguments.iter() {
                let arg_expr = match arg {
                    Argument::Positional(pos) => pos.value,
                    Argument::Named(named) => named.value,
                };
                collect_expr_declarations(arg_expr, content, prefix, path, out, open_prefixes);
            }
        }
        // Literal values cannot contribute child config keys. Everything
        // else may evaluate to an array at runtime, so its subtree remains
        // open rather than producing diagnostics from incomplete evidence.
        Expression::Literal(_) | Expression::CompositeString(_) | Expression::MagicConstant(_) => {}
        _ => open_prefixes.push(config_path(prefix, path)),
    }
}

fn collect_array_declarations<'a>(
    elements: impl Iterator<Item = &'a ArrayElement<'a>>,
    content: &str,
    prefix: &str,
    path: &mut String,
    out: &mut Vec<ConfigKeyMatch>,
    open_prefixes: &mut Vec<String>,
) {
    for element in elements {
        let ArrayElement::KeyValue(kv) = element else {
            if !matches!(element, ArrayElement::Missing(_)) {
                open_prefixes.push(config_path(prefix, path));
            }
            continue;
        };
        let (key_text, key_start, key_end) =
            match super::helpers::extract_string_literal(kv.key, content) {
                Some(k) => k,
                None => {
                    open_prefixes.push(config_path(prefix, path));
                    continue;
                }
            };

        let previous_len = path.len();
        if previous_len != 0 {
            path.push('.');
        }
        path.push_str(key_text);
        out.push(ConfigKeyMatch {
            key: config_path(prefix, path),
            start: key_start,
            end: key_end,
        });

        collect_expr_declarations(kv.value, content, prefix, path, out, open_prefixes);
        path.truncate(previous_len);
    }
}

fn config_path(prefix: &str, path: &str) -> String {
    if path.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix}.{path}")
    }
}

// ─── Public cross-file query API ──────────────────────────────────────────────

/// Find all references for a Laravel config key across the project.
///
/// Uses pre-built [`SymbolKind::LaravelStringKey`] spans to avoid re-parsing
/// every file at request time (same pattern as `find_member_references`).
pub(crate) fn find_config_references(
    backend: &Backend,
    uri: &str,
    content: &str,
    position: Position,
    include_declaration: bool,
) -> Option<Vec<Location>> {
    // Fast path: cursor is on a usage site — symbol map already has the key.
    let (target_kind, target_key) = if let Some(sym) =
        backend.lookup_symbol_at_position(uri, content, position)
    {
        match sym.kind {
            SymbolKind::LaravelStringKey { kind, key, .. } if kind.is_config_backed() => {
                (kind, key)
            }
            _ => return None,
        }
    } else {
        // Fallback: cursor is on a declaration key inside config/*.php.
        // This re-parses the current (single) config file — acceptable.
        let prefix = config_prefix_for_uri(backend, uri)?;
        let cursor_offset = crate::text_position::position_to_offset(content, position) as usize;
        let key = collect_laravel_config_declarations(content, &prefix)
            .into_iter()
            .find(|d| cursor_offset >= d.start && cursor_offset <= d.end)
            .map(|d| d.key)?;
        (LaravelStringKind::Config, key)
    };

    let snapshot = backend.user_file_symbol_maps();
    let locations = find_all_config_references(
        backend,
        &target_kind,
        &target_key,
        &snapshot,
        include_declaration,
    );

    if locations.is_empty() {
        return None;
    }

    Some(locations)
}

/// Called from `resolve_from_symbol` when the symbol map contains a
/// [`SymbolKind::LaravelStringKey`] span with `kind == Config` at the cursor —
/// no file re-parse is needed for the usage side.
pub(crate) fn resolve_config_key_declaration(backend: &Backend, key: &str) -> Option<Location> {
    resolve_config_key_declaration_inner(backend, key, true)
}

/// Resolve only an exact config declaration, without falling back to the
/// start of the file that owns the key's root.
pub(crate) fn resolve_config_key_declaration_exact(
    backend: &Backend,
    key: &str,
) -> Option<Location> {
    resolve_config_key_declaration_inner(backend, key, false)
}

fn resolve_config_key_declaration_inner(
    backend: &Backend,
    key: &str,
    allow_file_fallback: bool,
) -> Option<Location> {
    let root = backend.workspace.workspace_root.read().clone()?;
    let config_dir = root.join("config");
    let mut fallback = None;
    let mut relative_path = std::path::PathBuf::new();
    let mut stem = String::with_capacity(key.len());

    for part in key.split('.') {
        relative_path.push(part);
        if !stem.is_empty() {
            stem.push('.');
        }
        stem.push_str(part);
        let config_path = config_dir.join(&relative_path).with_extension("php");

        if !config_path.is_file() {
            continue;
        }
        let Some((target_uri, target_content)) = backend.laravel_config_file_content(&config_path)
        else {
            continue;
        };
        fallback.get_or_insert_with(|| {
            crate::definition::point_location(target_uri.clone(), Position::new(0, 0))
        });
        if let Some(location) = exact_config_declaration(&target_uri, &target_content, &stem, key) {
            return Some(location);
        }
    }

    let provider_configs = backend
        .laravel_provider_resources
        .read()
        .config_files
        .iter()
        .map(|resource| (resource.path.clone(), resource.namespace.clone()))
        .collect::<Vec<_>>();
    for (path, namespace) in provider_configs {
        if !config_key_belongs_to_namespace(key, &namespace) || !path.is_file() {
            continue;
        }
        let Some((target_uri, target_content)) = backend.laravel_config_file_content(&path) else {
            continue;
        };
        fallback.get_or_insert_with(|| {
            crate::definition::point_location(target_uri.clone(), Position::new(0, 0))
        });
        if let Some(location) =
            exact_config_declaration(&target_uri, &target_content, &namespace, key)
        {
            return Some(location);
        }
    }

    if let Some((path, prefix)) = framework_config_source(&root, key)
        && let Some((target_uri, target_content)) = backend.laravel_config_file_content(&path)
    {
        fallback.get_or_insert_with(|| {
            crate::definition::point_location(target_uri.clone(), Position::new(0, 0))
        });
        if let Some(location) = exact_config_declaration(&target_uri, &target_content, prefix, key)
        {
            return Some(location);
        }
    }

    if allow_file_fallback { fallback } else { None }
}

impl Backend {
    /// Read one config source, preferring any open URI spelling of the file.
    ///
    /// Provider registrations may retain a lexical path containing `..` or a
    /// symlink while editors open its canonical spelling. Exact URI probes
    /// keep the common path constant-time; the filename-filtered fallback
    /// handles the uncommon inverse alias without canonicalizing unrelated
    /// buffers.
    pub(crate) fn laravel_config_file_content(
        &self,
        path: &std::path::Path,
    ) -> Option<(Url, Arc<String>)> {
        let raw_uri = Url::from_file_path(path).ok()?;
        let alias_candidates = {
            let open_files = self.open_files.read();
            if let Some(content) = open_files.get(raw_uri.as_str()) {
                return Some((raw_uri, Arc::clone(content)));
            }
            if open_files.is_empty() {
                return std::fs::read_to_string(path)
                    .ok()
                    .map(|content| (raw_uri, Arc::new(content)));
            }

            let file_name = path.file_name();
            open_files
                .iter()
                .filter_map(|(uri, content)| {
                    let parsed_uri = Url::parse(uri).ok()?;
                    let open_path = parsed_uri.to_file_path().ok()?;
                    (open_path.file_name() == file_name).then_some((
                        parsed_uri,
                        open_path,
                        Arc::clone(content),
                    ))
                })
                .collect::<Vec<_>>()
        };

        if alias_candidates.is_empty() {
            return std::fs::read_to_string(path)
                .ok()
                .map(|content| (raw_uri, Arc::new(content)));
        }
        let canonical_path = path.canonicalize().ok();
        for (uri, open_path, content) in alias_candidates {
            if open_path == path
                || canonical_path.as_ref().is_some_and(|canonical| {
                    open_path
                        .canonicalize()
                        .is_ok_and(|open_canonical| open_canonical == *canonical)
                })
            {
                return Some((uri, content));
            }
        }

        std::fs::read_to_string(path)
            .ok()
            .map(|content| (raw_uri, Arc::new(content)))
    }
}

fn config_prefix_for_uri(backend: &Backend, uri: &str) -> Option<String> {
    if let Some(prefix) = laravel_config_prefix_from_uri(uri) {
        return Some(prefix);
    }
    let edited_path = Url::parse(uri).ok()?.to_file_path().ok()?;
    let candidates = {
        let resources = backend.laravel_provider_resources.read();
        let mut candidates = Vec::new();
        for resource in &resources.config_files {
            if edited_path == resource.path {
                return Some(resource.namespace.clone());
            }
            if edited_path.file_name() == resource.path.file_name() {
                candidates.push((resource.path.clone(), resource.namespace.clone()));
            }
        }
        candidates
    };
    if candidates.is_empty() {
        return None;
    }
    let edited_path = edited_path.canonicalize().ok()?;
    for (candidate, namespace) in candidates {
        if candidate
            .canonicalize()
            .is_ok_and(|path| path == edited_path)
        {
            return Some(namespace);
        }
    }
    None
}

fn exact_config_declaration(uri: &Url, content: &str, prefix: &str, key: &str) -> Option<Location> {
    let declaration = collect_laravel_config_declarations(content, prefix)
        .into_iter()
        .find(|declaration| declaration.key == key)?;
    Some(crate::definition::point_location(
        uri.clone(),
        offset_to_position(content, declaration.start),
    ))
}

fn config_key_belongs_to_namespace(key: &str, namespace: &str) -> bool {
    key == namespace
        || key
            .strip_prefix(namespace)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn framework_config_source<'a>(
    root: &std::path::Path,
    key: &'a str,
) -> Option<(std::path::PathBuf, &'a str)> {
    let prefix = key.split('.').next()?;
    let path = root
        .join("vendor/laravel/framework/config")
        .join(format!("{prefix}.php"));
    path.is_file().then_some((path, prefix))
}

/// Find all references for a Laravel config key across the project.
///
/// Iterates pre-built [`SymbolKind::LaravelStringKey`] spans for usages
/// (zero re-parses per file, same pattern as `find_member_references`).
/// Declaration lookup in `config/*.php` still uses an AST walk, but that
/// set is small (typically < 20 files) and each parse is cheap.
pub(crate) fn find_all_config_references(
    backend: &Backend,
    target_kind: &LaravelStringKind,
    target_key: &str,
    snapshot: &[(String, Arc<SymbolMap>)],
    include_declaration: bool,
) -> Vec<Location> {
    if !target_kind.is_config_backed() {
        return Vec::new();
    }
    let mut locations = Vec::new();

    // Usages: walk pre-built symbol spans — no file re-parse needed.
    for (file_uri, symbol_map) in snapshot {
        let has_candidate = symbol_map.resource_receiver_sites.iter().any(|site| {
            site.rule
                .candidate_kinds()
                .iter()
                .any(|candidate| config_keys_match(target_kind, target_key, candidate, &site.key))
        });
        let extra =
            has_candidate.then(|| backend.typed_receiver_view_spans_for(file_uri, symbol_map));
        if !symbol_map
            .spans
            .iter()
            .chain(extra.iter().flat_map(|spans| spans.iter()))
            .any(|span| config_span_matches(span, target_kind, target_key))
        {
            continue;
        }
        let Ok(parsed_uri) = Url::parse(file_uri) else {
            continue;
        };
        let Some(file_content) = backend.get_file_content_arc(file_uri) else {
            continue;
        };
        for span in symbol_map
            .spans
            .iter()
            .chain(extra.iter().flat_map(|spans| spans.iter()))
        {
            if config_span_matches(span, target_kind, target_key) {
                let start = offset_to_position(&file_content, span.start as usize);
                let end = offset_to_position(&file_content, span.end as usize);
                push_unique_location(&mut locations, &parsed_uri, start, end);
            }
        }
    }

    // Declarations: keys in config/*.php (small set, AST walk acceptable).
    if include_declaration {
        let declaration_key = canonical_config_key(target_kind, target_key);
        if let Some(root) = backend.workspace.workspace_root.read().clone() {
            let config_dir = root.join("config");
            let mut app_configs = Vec::new();
            collect_php_files(&config_dir, &mut app_configs);
            app_configs.sort_unstable();
            for path in app_configs {
                let relative = path
                    .strip_prefix(&config_dir)
                    .expect("collected config file must remain below its scan root");
                let prefix = config_prefix_from_relative_path(relative);
                let Some((parsed_uri, file_content)) = backend.laravel_config_file_content(&path)
                else {
                    continue;
                };
                for decl in collect_laravel_config_declarations(&file_content, &prefix) {
                    if decl.key == declaration_key {
                        push_unique_location(
                            &mut locations,
                            &parsed_uri,
                            offset_to_position(&file_content, decl.start),
                            offset_to_position(&file_content, decl.end),
                        );
                    }
                }
            }
        }

        // Package providers can merge config from outside the application's
        // own `config/` directory. Those files are vendor-filtered from the
        // symbol-map snapshot but remain real declaration destinations.
        let provider_configs = backend
            .laravel_provider_resources
            .read()
            .config_files
            .iter()
            .map(|resource| (resource.path.clone(), resource.namespace.clone()))
            .collect::<Vec<_>>();
        for (path, namespace) in provider_configs {
            let Some((parsed_uri, content)) = backend.laravel_config_file_content(&path) else {
                continue;
            };
            for declaration in collect_laravel_config_declarations(&content, &namespace) {
                if declaration.key == declaration_key {
                    push_unique_location(
                        &mut locations,
                        &parsed_uri,
                        offset_to_position(&content, declaration.start),
                        offset_to_position(&content, declaration.end),
                    );
                }
            }
        }

        // Laravel's unpublished defaults are completion candidates too, so
        // their declarations participate in references exactly like app and
        // package config entries.
        if let Some(root) = backend.workspace.workspace_root.read().clone()
            && let Some((path, prefix)) = framework_config_source(&root, &declaration_key)
            && let Some((parsed_uri, content)) = backend.laravel_config_file_content(&path)
        {
            for declaration in collect_laravel_config_declarations(&content, prefix) {
                if declaration.key == declaration_key {
                    push_unique_location(
                        &mut locations,
                        &parsed_uri,
                        offset_to_position(&content, declaration.start),
                        offset_to_position(&content, declaration.end),
                    );
                }
            }
        }
    }

    locations
}

fn config_prefix_from_relative_path(relative: &std::path::Path) -> String {
    let stem = relative.with_extension("");
    let mut prefix = String::new();
    for component in stem.components() {
        if !prefix.is_empty() {
            prefix.push('.');
        }
        prefix.push_str(&component.as_os_str().to_string_lossy());
    }
    prefix
}

fn config_span_matches(
    span: &SymbolSpan,
    target_kind: &LaravelStringKind,
    target_key: &str,
) -> bool {
    matches!(
        &span.kind,
        SymbolKind::LaravelStringKey { kind, key, .. }
            if config_keys_match(target_kind, target_key, kind, key)
    )
}

fn collect_php_files(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_php_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "php") {
            files.push(path);
        }
    }
}

fn config_keys_match(
    left_kind: &LaravelStringKind,
    left_key: &str,
    right_kind: &LaravelStringKind,
    right_key: &str,
) -> bool {
    match (left_kind, right_kind) {
        (LaravelStringKind::Config, LaravelStringKind::Config) => left_key == right_key,
        (LaravelStringKind::ConfigResource(left), LaravelStringKind::ConfigResource(right)) => {
            left == right && left_key == right_key
        }
        (LaravelStringKind::Config, LaravelStringKind::ConfigResource(resource)) => {
            crate::symbol_map::laravel_resources::matches_config_key(*resource, right_key, left_key)
        }
        (LaravelStringKind::ConfigResource(resource), LaravelStringKind::Config) => {
            crate::symbol_map::laravel_resources::matches_config_key(*resource, left_key, right_key)
        }
        _ => false,
    }
}

fn canonical_config_key(kind: &LaravelStringKind, key: &str) -> String {
    match kind {
        LaravelStringKind::Config => key.to_string(),
        LaravelStringKind::ConfigResource(resource) => {
            crate::symbol_map::laravel_resources::config_key(*resource, key)
        }
        _ => String::new(),
    }
}

/// Fallback for "go to definition" on a key inside config/*.php.
///
/// Since array keys are not indexed in the symbol map, the generic
/// resolution returns None.  This re-parses the current file to see
/// if the cursor is on a known config key, and if so, returns a Location
/// pointing to the same file (enabling Find All References for that key).
pub(crate) fn resolve_config_key_definition_fallback(
    backend: &Backend,
    uri: &str,
    content: &str,
    position: Position,
) -> Option<Location> {
    let prefix = config_prefix_for_uri(backend, uri)?;
    let cursor_offset = crate::text_position::position_to_offset(content, position) as usize;
    let decls = collect_laravel_config_declarations(content, &prefix);
    let match_ = decls
        .into_iter()
        .find(|d| cursor_offset >= d.start && cursor_offset <= d.end)?;

    let target_uri = Url::parse(uri).ok()?;
    let pos = crate::text_position::offset_to_position(content, match_.start);
    Some(crate::definition::point_location(target_uri, pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_prefix_from_uri_normal() {
        assert_eq!(
            laravel_config_prefix_from_uri("file:///project/config/app.php"),
            Some("app".to_string())
        );
    }

    #[test]
    fn config_prefix_from_uri_root_level() {
        assert_eq!(
            laravel_config_prefix_from_uri("file:///config/app.php"),
            Some("app".to_string())
        );
    }

    #[test]
    fn config_prefix_from_uri_not_in_config_dir() {
        assert_eq!(
            laravel_config_prefix_from_uri("file:///project/src/Service.php"),
            None
        );
    }

    #[test]
    fn config_prefix_from_uri_file_named_config() {
        assert_eq!(
            laravel_config_prefix_from_uri("file:///project/config.php"),
            None
        );
    }

    #[test]
    fn config_prefix_from_uri_supports_subdirectory() {
        assert_eq!(
            laravel_config_prefix_from_uri("file:///project/config/mail/transport.php"),
            Some("mail.transport".to_string())
        );
        assert_eq!(
            config_prefix_from_relative_path(std::path::Path::new("mail/transport.php")),
            "mail.transport"
        );
    }

    #[test]
    fn config_prefix_from_uri_uses_nearest_config_segment() {
        assert_eq!(
            laravel_config_prefix_from_uri(
                "file:///workspace/config/vendor/project/config/app.php"
            ),
            Some("app".to_string())
        );
    }

    #[test]
    fn config_prefix_from_uri_rejects_empty_file_stems() {
        assert_eq!(
            laravel_config_prefix_from_uri("file:///project/config/.php"),
            None
        );
        assert_eq!(
            laravel_config_prefix_from_uri("file:///project/config/nested/.php"),
            None
        );
    }

    #[test]
    fn test_collect_declarations_variable_return() {
        let content = "<?php
$config = [
    'name' => 'Laravel',
];
return $config;";
        let prefix = "app";
        let decls = collect_laravel_config_declarations(content, prefix);
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0].key, "app.name");
    }

    #[test]
    fn test_collect_declarations_array_merge() {
        let content = "<?php
return array_merge([
    'name' => 'Laravel',
], [
    'env' => 'production',
]);";
        let prefix = "app";
        let decls = collect_laravel_config_declarations(content, prefix);
        assert_eq!(decls.len(), 2);
        assert_eq!(decls[0].key, "app.name");
        assert_eq!(decls[1].key, "app.env");
    }

    #[test]
    fn scan_marks_only_the_dynamic_config_subtree_open() {
        let content = "<?php
return [
    'stores' => array_merge([
        'array' => ['driver' => 'array'],
    ], $packageStores),
    'default' => 'array',
];";

        let scan = scan_laravel_config_file(content, "cache");
        let keys = scan
            .declarations
            .iter()
            .map(|declaration| declaration.key.as_str())
            .collect::<Vec<_>>();

        assert!(keys.contains(&"cache.stores.array"));
        assert!(keys.contains(&"cache.default"));
        assert_eq!(scan.open_prefixes, ["cache.stores"]);
    }

    #[test]
    fn scan_handles_legacy_arrays_named_merge_arguments_and_dynamic_elements() {
        let content = r#"<?php
return array_merge(
    first: (array('legacy' => array('leaf' => true))),
    second: [
        'mixed' => [...$spread, $dynamicKey => [], 'unkeyed'],
    ],
);"#;

        let scan = scan_laravel_config_file(content, "app");
        let keys = scan
            .declarations
            .iter()
            .map(|declaration| declaration.key.as_str())
            .collect::<Vec<_>>();

        assert!(keys.contains(&"app.legacy"));
        assert!(keys.contains(&"app.legacy.leaf"));
        assert!(keys.contains(&"app.mixed"));
        assert_eq!(scan.open_prefixes, ["app.mixed"]);
        assert_eq!(config_path("app", ""), "app");
    }

    #[test]
    fn config_usage_reference_lookup_accepts_config_backed_symbol_kinds() {
        let backend = Backend::new_test();
        let uri = "file:///config-usage.php";
        let content = Arc::new("<?php config('cache.default');".to_string());
        backend
            .open_files
            .write()
            .insert(uri.to_string(), Arc::clone(&content));
        backend.update_ast(uri, &content);
        let position = offset_to_position(&content, content.find("cache.default").unwrap());

        let locations = find_config_references(&backend, uri, &content, position, false).unwrap();

        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].uri.as_str(), uri);

        backend.open_files.write().remove(uri);
        assert!(find_config_references(&backend, uri, &content, position, false).is_none());
    }

    #[test]
    fn framework_config_resolution_falls_back_to_its_source_file() {
        let dir = tempfile::tempdir().unwrap();
        let framework_config = dir.path().join("vendor/laravel/framework/config/cache.php");
        let unrelated_provider = dir.path().join("vendor/package/config/settings.php");
        std::fs::create_dir_all(framework_config.parent().unwrap()).unwrap();
        std::fs::create_dir_all(unrelated_provider.parent().unwrap()).unwrap();
        std::fs::write(&framework_config, "<?php return ['default' => 'array'];\n").unwrap();
        std::fs::write(&unrelated_provider, "<?php return ['value' => true];\n").unwrap();

        let backend = Backend::new_test();
        *backend.workspace.workspace_root.write() = Some(dir.path().to_path_buf());
        backend
            .laravel_provider_resources
            .write()
            .config_files
            .push(crate::virtual_members::laravel::ProviderResource {
                path: unrelated_provider.clone(),
                namespace: "package".to_string(),
            });

        let provider_location =
            resolve_config_key_declaration_inner(&backend, "package.value", false).unwrap();
        assert_eq!(
            provider_location.uri,
            Url::from_file_path(&unrelated_provider).unwrap()
        );
        let provider_fallback =
            resolve_config_key_declaration_inner(&backend, "package.missing", true).unwrap();
        assert_eq!(
            provider_fallback.uri,
            Url::from_file_path(&unrelated_provider).unwrap()
        );
        assert_eq!(provider_fallback.range.start, Position::new(0, 0));

        let location =
            resolve_config_key_declaration_inner(&backend, "cache.missing", true).unwrap();
        assert_eq!(location.uri, Url::from_file_path(framework_config).unwrap());
        assert_eq!(location.range.start, Position::new(0, 0));
    }

    #[test]
    fn config_content_ignores_an_unrelated_open_file_with_the_same_name() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("package/config/settings.php");
        let unrelated = dir.path().join("other/config/settings.php");
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
        std::fs::write(&target, "<?php return ['source' => 'disk'];\n").unwrap();
        std::fs::write(&unrelated, "<?php return [];\n").unwrap();

        let backend = Backend::new_test();
        let unrelated_uri = Url::from_file_path(&unrelated).unwrap();
        backend.open_files.write().insert(
            unrelated_uri.to_string(),
            Arc::new("<?php return ['source' => 'buffer'];\n".to_string()),
        );

        let (uri, content) = backend.laravel_config_file_content(&target).unwrap();
        assert_eq!(uri, Url::from_file_path(target).unwrap());
        assert!(content.contains("'disk'"));
    }

    #[test]
    fn unreadable_config_sources_are_ignored_at_every_precedence_layer() {
        let dir = tempfile::tempdir().unwrap();
        let app_config = dir.path().join("config/cache.php");
        let empty_stem_config = dir.path().join("config/.php");
        let provider_config = dir.path().join("package/settings.php");
        std::fs::create_dir_all(app_config.parent().unwrap()).unwrap();
        std::fs::create_dir_all(provider_config.parent().unwrap()).unwrap();
        std::fs::write(&app_config, [0xff_u8, 0xfe]).unwrap();
        std::fs::write(&empty_stem_config, "<?php return ['default' => true];").unwrap();
        std::fs::write(&provider_config, [0xff_u8, 0xfe]).unwrap();

        let backend = Backend::new_test();
        *backend.workspace.workspace_root.write() = Some(dir.path().to_path_buf());
        backend
            .laravel_provider_resources
            .write()
            .config_files
            .push(crate::virtual_members::laravel::ProviderResource {
                path: provider_config,
                namespace: "package".to_string(),
            });

        assert!(resolve_config_key_declaration_inner(&backend, "cache.default", false).is_none());
        assert!(resolve_config_key_declaration_inner(&backend, "package.value", false).is_none());
        assert!(
            find_all_config_references(
                &backend,
                &LaravelStringKind::Config,
                "cache.default",
                &[],
                true,
            )
            .is_empty()
        );
    }

    #[test]
    fn provider_config_prefix_matches_exact_and_canonical_paths() {
        let dir = tempfile::tempdir().unwrap();
        let target_dir = dir.path().join("package/resources");
        let detour = target_dir.join("detour");
        let target = target_dir.join("settings.php");
        std::fs::create_dir_all(&detour).unwrap();
        std::fs::write(&target, "<?php return [];\n").unwrap();

        let backend = Backend::new_test();
        let target_uri = Url::from_file_path(&target).unwrap();
        assert_eq!(config_prefix_for_uri(&backend, target_uri.as_str()), None);
        backend
            .laravel_provider_resources
            .write()
            .config_files
            .push(crate::virtual_members::laravel::ProviderResource {
                path: target.clone(),
                namespace: "package".to_string(),
            });

        assert_eq!(
            config_prefix_for_uri(&backend, target_uri.as_str()),
            Some("package".to_string())
        );

        let aliased = detour.join("..").join("settings.php");
        backend.laravel_provider_resources.write().config_files[0].path = aliased;
        assert_eq!(
            config_prefix_for_uri(&backend, target_uri.as_str()),
            Some("package".to_string())
        );

        backend.laravel_provider_resources.write().config_files[0].path =
            dir.path().join("missing/settings.php");
        assert_eq!(config_prefix_for_uri(&backend, target_uri.as_str()), None);
    }

    #[test]
    fn config_usage_scan_rejects_invalid_and_missing_matching_sources() {
        let backend = Backend::new_test();
        let symbol_map = Arc::new(crate::parser::with_parsed_program(
            "<?php config('cache.default');",
            "config_reference_guards",
            crate::symbol_map::extract_symbol_map,
        ));

        for uri in ["not a URI", "file:///missing-config-usage.php"] {
            assert!(
                find_all_config_references(
                    &backend,
                    &LaravelStringKind::Config,
                    "cache.default",
                    &[(uri.to_string(), Arc::clone(&symbol_map))],
                    false,
                )
                .is_empty()
            );
        }

        assert!(
            find_all_config_references(
                &backend,
                &LaravelStringKind::Config,
                "cache.default",
                &[],
                true,
            )
            .is_empty()
        );
    }

    #[test]
    fn config_reference_helpers_reject_unrelated_kinds_and_missing_sources() {
        let dir = tempfile::tempdir().unwrap();
        let backend = Backend::new_test();
        *backend.workspace.workspace_root.write() = Some(dir.path().to_path_buf());
        backend
            .laravel_provider_resources
            .write()
            .config_files
            .push(crate::virtual_members::laravel::ProviderResource {
                path: dir.path().join("missing/settings.php"),
                namespace: "package".to_string(),
            });

        assert!(
            find_all_config_references(
                &backend,
                &LaravelStringKind::RateLimiter,
                "api",
                &[],
                true,
            )
            .is_empty()
        );
        assert!(
            find_all_config_references(
                &backend,
                &LaravelStringKind::Config,
                "package.value",
                &[],
                true,
            )
            .is_empty()
        );
        assert!(!config_keys_match(
            &LaravelStringKind::RateLimiter,
            "api",
            &LaravelStringKind::Config,
            "cache.default",
        ));
        assert_eq!(
            canonical_config_key(&LaravelStringKind::RateLimiter, "api"),
            ""
        );
    }
}
