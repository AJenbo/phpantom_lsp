//! Laravel string key completion.
//!
//! Offers autocompletion for Laravel's string-addressed resources inside
//! their respective helper, facade, attribute, and typed-receiver calls:
//!
//! - `route('|')` / `to_route('|')` → route names
//! - `config('|')` / `Config::get('|')` → config keys
//! - `view('|')` / `View::make('|')` → view names
//! - `__('|')` / `trans('|')` / `Lang::get('|')` → translation keys
//! - `Cache::store('|')` / `Storage::disk('|')` → configured resource names

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
#[cfg(test)]
use crate::symbol_map::LaravelConfigResource;
use crate::symbol_map::{LaravelResourceReceiverRule, LaravelStringKind};
use crate::text_position::position_to_offset;
use crate::type_engine::resolver::{ResolutionCtx, resolve_target_classes};
use crate::types::{AccessKind, FileContext};

mod context;

use context::detect_laravel_string_key_context_inner;
#[cfg(test)]
use context::{
    callable_before_scalar_argument, chain_contains_laravel_facade,
    detect_laravel_string_key_context, enclosing_call_open_paren, is_connection_property_value,
    is_laravel_facade, rate_limited_constructor_class, string_literal_is_array_key,
};

// ─── Enumeration ────────────────────────────────────────────────────────────

type ConfigMetadata = (std::sync::Arc<Vec<String>>, std::sync::Arc<Vec<String>>);

#[inline]
fn config_metadata_snapshot(cache: &crate::LaravelStringKeyCache) -> Option<ConfigMetadata> {
    Some((
        std::sync::Arc::clone(cache.config_keys.as_ref()?),
        std::sync::Arc::clone(cache.config_open_prefixes.as_ref()?),
    ))
}

/// Publish a completed scan only when no invalidation happened while it was
/// running. A stale scan is discarded and rebuilt against the new generation.
fn publish_config_metadata(
    cache: &mut crate::LaravelStringKeyCache,
    generation: u64,
    metadata: &ConfigMetadata,
) -> bool {
    if cache.config_generation != generation {
        return false;
    }
    cache.config_keys = Some(std::sync::Arc::clone(&metadata.0));
    cache.config_open_prefixes = Some(std::sync::Arc::clone(&metadata.1));
    true
}

/// Keep a receiver-derived resource kind only when every possible receiver
/// resolves to the same family. Mixed or unresolved unions must not offer
/// names from an arbitrary branch.
#[inline]
fn unanimous_resource_kind(
    kinds: impl IntoIterator<Item = Option<LaravelStringKind>>,
) -> Option<LaravelStringKind> {
    let mut confirmed = None;
    for kind in kinds {
        let kind = kind?;
        match confirmed {
            Some(existing) if existing != kind => return None,
            None => confirmed = Some(kind),
            _ => {}
        }
    }
    confirmed
}

impl Backend {
    /// The configured Blade view root directories.
    ///
    /// Reads the `paths` array from `config/view.php` (falling back to
    /// the conventional `resources/views`) so that projects with custom
    /// view directories resolve `view()` names correctly. Only existing
    /// directories are returned. Read from disk, so unsaved edits to
    /// `config/view.php` are not reflected until saved.
    pub(crate) fn laravel_view_roots(&self) -> Vec<std::path::PathBuf> {
        match self.workspace.workspace_root.read().clone() {
            Some(root) => crate::blade::discover_view_paths(&root),
            None => Vec::new(),
        }
    }

    /// Enumerate all config keys and runtime-open subtrees by scanning
    /// `config/` files and package config files discovered from providers.
    fn enumerate_all_config_metadata(&self) -> (Vec<String>, Vec<String>) {
        use crate::virtual_members::laravel::{
            laravel_config_prefix_from_uri, scan_laravel_config_file,
        };

        let snapshot = self.user_file_symbol_maps();
        let mut keys = Vec::new();
        let mut open_prefixes = Vec::new();

        for (file_uri, _) in &snapshot {
            let Some(prefix) = laravel_config_prefix_from_uri(file_uri) else {
                continue;
            };
            let Some(content) = self.get_file_content(file_uri) else {
                continue;
            };
            let scan = scan_laravel_config_file(&content, &prefix);
            for d in scan.declarations {
                keys.push(d.key);
            }
            open_prefixes.extend(scan.open_prefixes);
        }

        let provider_configs = self
            .laravel_provider_resources
            .read()
            .config_files
            .iter()
            .map(|resource| (resource.path.clone(), resource.namespace.clone()))
            .collect::<Vec<_>>();
        for (path, namespace) in provider_configs {
            if let Some((_, content)) = self.laravel_config_file_content(&path) {
                let scan = scan_laravel_config_file(&content, &namespace);
                for d in scan.declarations {
                    keys.push(d.key);
                }
                open_prefixes.extend(scan.open_prefixes);
            }
        }

        if let Some(root) = self.workspace.workspace_root.read().clone() {
            let framework_config = root.join("vendor/laravel/framework/config");
            if framework_config.is_dir()
                && let Ok(entries) = std::fs::read_dir(&framework_config)
            {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.extension().is_some_and(|e| e == "php") {
                        continue;
                    }
                    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                        continue;
                    };
                    let prefix = stem.to_string();
                    if let Some((_, content)) = self.laravel_config_file_content(&path) {
                        let scan = scan_laravel_config_file(&content, &prefix);
                        for d in scan.declarations {
                            keys.push(d.key);
                        }
                        open_prefixes.extend(scan.open_prefixes);
                    }
                }
            }
        }

        keys.sort();
        keys.dedup();
        open_prefixes.sort();
        open_prefixes.dedup();
        (keys, open_prefixes)
    }

    /// Enumerate all translation keys by scanning `lang/` files and
    /// package translation directories discovered from service providers.
    ///
    /// Supports both PHP array files (`lang/en/messages.php` → `messages.key`)
    /// and JSON translation files (`lang/en.json` → raw key strings).
    /// Package translations use `namespace::file.key` syntax.
    fn enumerate_all_trans_keys(&self) -> Vec<String> {
        let snapshot = self.user_file_symbol_maps();
        let mut keys = Vec::new();

        for (file_uri, _) in &snapshot {
            if !(file_uri.contains("/lang/") || file_uri.contains("/resources/lang/")) {
                continue;
            }
            if !file_uri.ends_with(".php") {
                continue;
            }
            let Some(stem) = extract_lang_file_stem(file_uri) else {
                continue;
            };
            let Some(content) = self.get_file_content(file_uri) else {
                continue;
            };
            let decls =
                crate::virtual_members::laravel::collect_trans_declarations(&content, &stem);
            for d in decls {
                keys.push(d.key);
            }
        }

        collect_json_trans_keys(self, &mut keys);

        for res in &self.laravel_provider_resources.read().trans_dirs {
            collect_namespaced_trans_keys(&res.path, &res.namespace, &mut keys);
        }

        keys.sort();
        keys.dedup();
        keys
    }

    /// Enumerate every translation key alongside whether it names a
    /// translation group (a nested array) rather than a scalar string
    /// entry, merging the flag across every locale and file that
    /// declares the key.
    ///
    /// A key that is a group in *any* locale is recorded as a group even
    /// if another locale happens to declare it as a scalar — the return
    /// type narrowing this feeds is only safe when every locale agrees
    /// the entry is scalar.
    fn enumerate_all_trans_key_shapes(&self) -> HashMap<String, bool> {
        let snapshot = self.user_file_symbol_maps();
        let mut shapes = HashMap::new();

        for (file_uri, _) in &snapshot {
            if !(file_uri.contains("/lang/") || file_uri.contains("/resources/lang/")) {
                continue;
            }
            if !file_uri.ends_with(".php") {
                continue;
            }
            let Some(stem) = extract_lang_file_stem(file_uri) else {
                continue;
            };
            let Some(content) = self.get_file_content(file_uri) else {
                continue;
            };
            let decls =
                crate::virtual_members::laravel::collect_trans_declarations(&content, &stem);
            for d in decls {
                mark_trans_shape(&mut shapes, d.key, d.is_group);
            }
        }

        collect_json_trans_key_shapes(self, &mut shapes);

        for res in &self.laravel_provider_resources.read().trans_dirs {
            collect_namespaced_trans_key_shapes(&res.path, &res.namespace, &mut shapes);
        }

        shapes
    }

    /// Read one slot of [`LaravelStringKeyCache`], building it under
    /// `build_lock` when empty.
    ///
    /// The build is guarded rather than raced because every enumeration
    /// walks the workspace from disk: the parallel diagnostic pass
    /// otherwise has all N workers miss the same empty slot at once and
    /// each repeat the identical walk. Waiters re-check the slot after
    /// acquiring the guard, so exactly one walk happens per
    /// invalidation.
    pub(crate) fn cached_laravel_enumeration<T: Clone>(
        &self,
        build_lock: &parking_lot::Mutex<()>,
        read: impl Fn(&crate::LaravelStringKeyCache) -> Option<T>,
        store: impl Fn(&mut crate::LaravelStringKeyCache, T),
        build: impl FnOnce() -> T,
    ) -> T {
        if let Some(cached) = read(&self.laravel_string_key_cache.read()) {
            return cached;
        }
        let _build_guard = build_lock.lock();
        if let Some(cached) = read(&self.laravel_string_key_cache.read()) {
            return cached;
        }
        let value = build();
        store(&mut self.laravel_string_key_cache.write(), value.clone());
        value
    }

    /// Every named route in the project, with the URI it was registered with.
    pub(crate) fn cached_routes(
        &self,
    ) -> std::sync::Arc<Vec<crate::virtual_members::laravel::RouteEntry>> {
        self.cached_laravel_enumeration(
            &self.laravel_string_key_build_locks.routes,
            |cache| cache.routes.clone(),
            |cache, routes| cache.routes = Some(routes),
            || std::sync::Arc::new(crate::virtual_members::laravel::enumerate_all_routes(self)),
        )
    }

    pub(crate) fn cached_route_names(&self) -> Vec<String> {
        self.cached_routes()
            .iter()
            .map(|route| route.name.clone())
            .collect()
    }

    /// The sorted config keys and runtime-open subtrees from one shared scan.
    pub(crate) fn cached_config_metadata(
        &self,
    ) -> (std::sync::Arc<Vec<String>>, std::sync::Arc<Vec<String>>) {
        self.cached_config_metadata_with(|| self.enumerate_all_config_metadata())
    }

    /// Cache a config scan, retrying it when an invalidation overtakes the
    /// scan before publication.
    fn cached_config_metadata_with(
        &self,
        scan: impl FnMut() -> (Vec<String>, Vec<String>),
    ) -> ConfigMetadata {
        self.cached_config_metadata_with_snapshot(config_metadata_snapshot, scan)
    }

    fn cached_config_metadata_with_snapshot(
        &self,
        mut snapshot: impl FnMut(&crate::LaravelStringKeyCache) -> Option<ConfigMetadata>,
        mut scan: impl FnMut() -> (Vec<String>, Vec<String>),
    ) -> ConfigMetadata {
        loop {
            if let Some(metadata) = snapshot(&self.laravel_string_key_cache.read()) {
                return metadata;
            }

            let _build_guard = self.laravel_string_key_build_locks.config_keys.lock();
            let generation = {
                let cache = self.laravel_string_key_cache.read();
                if let Some(metadata) = snapshot(&cache) {
                    return metadata;
                }
                cache.config_generation
            };

            let (keys, open_prefixes) = scan();
            let metadata = (
                std::sync::Arc::new(keys),
                std::sync::Arc::new(open_prefixes),
            );
            let mut cache = self.laravel_string_key_cache.write();
            if publish_config_metadata(&mut cache, generation, &metadata) {
                return metadata;
            }
        }
    }

    pub(crate) fn cached_config_keys(&self) -> std::sync::Arc<Vec<String>> {
        self.cached_config_metadata().0
    }

    pub(crate) fn cached_view_names(&self) -> Vec<String> {
        self.cached_laravel_enumeration(
            &self.laravel_string_key_build_locks.view_names,
            |cache| cache.view_names.clone(),
            |cache, names| cache.view_names = Some(names),
            || self.blade_view_names(),
        )
    }

    /// Every authorization ability the project defines, from `Gate::define()`
    /// registrations and policy class methods.
    pub(crate) fn cached_gate_abilities(&self) -> Vec<String> {
        self.cached_laravel_enumeration(
            &self.laravel_string_key_build_locks.gate_abilities,
            |cache| cache.gate_abilities.clone(),
            |cache, names| cache.gate_abilities = Some(names),
            || crate::virtual_members::laravel::enumerate_gate_abilities(self),
        )
    }

    pub(crate) fn cached_trans_keys(&self) -> Vec<String> {
        self.cached_laravel_enumeration(
            &self.laravel_string_key_build_locks.trans_keys,
            |cache| cache.trans_keys.clone(),
            |cache, keys| cache.trans_keys = Some(keys),
            || self.enumerate_all_trans_keys(),
        )
    }

    /// Every translation key mapped to whether it names a group (nested
    /// array) rather than a scalar entry.  Used to narrow the return type
    /// of `__()`/`trans()`/`Lang::get()` at call sites whose key argument
    /// is a literal.
    pub(crate) fn cached_trans_key_shapes(&self) -> std::sync::Arc<HashMap<String, bool>> {
        self.cached_laravel_enumeration(
            &self.laravel_string_key_build_locks.trans_key_shapes,
            |cache| cache.trans_key_shapes.clone(),
            |cache, shapes| cache.trans_key_shapes = Some(shapes),
            || std::sync::Arc::new(self.enumerate_all_trans_key_shapes()),
        )
    }
}

/// Extract the file stem from a lang file URI for use as the translation
/// key prefix.
///
/// `file:///path/lang/en/messages.php` → `"messages"`
fn extract_lang_file_stem(uri: &str) -> Option<String> {
    let file = uri.rsplit('/').next()?;
    let stem = file.strip_suffix(".php")?;
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_string())
}

/// Scan the workspace for `lang/*.json` files and collect their top-level
/// keys into `out`.  Laravel's JSON translations are flat
/// `{ "Some phrase": "Translated phrase" }` objects where the key is used
/// directly in `__('Some phrase')`.
///
/// We scan the filesystem because JSON files are not PHP and therefore do
/// not appear in `user_file_symbol_maps()`.
fn collect_json_trans_keys(backend: &crate::Backend, out: &mut Vec<String>) {
    let root = match backend.workspace.workspace_root.read().clone() {
        Some(r) => r,
        None => return,
    };
    for sub in &["lang", "resources/lang"] {
        let dir = root.join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(content) = std::fs::read_to_string(&path)
                && let Ok(map) =
                    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            {
                for k in map.keys() {
                    out.push(k.clone());
                }
            }
        }
    }
}

/// Scan a package translation directory and collect keys in
/// `namespace::file.key` format (PHP files) or `namespace::raw_key`
/// (JSON files with empty namespace).
fn collect_namespaced_trans_keys(dir: &std::path::Path, namespace: &str, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_namespaced_trans_from_locale_dir(&path, namespace, out);
        } else if path.extension().is_some_and(|e| e == "json")
            && namespace.is_empty()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(map) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
        {
            for k in map.keys() {
                out.push(k.clone());
            }
        }
    }
}

fn collect_namespaced_trans_from_locale_dir(
    dir: &std::path::Path,
    namespace: &str,
    out: &mut Vec<String>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|e| e == "php") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let prefix = if namespace.is_empty() {
            stem.to_string()
        } else {
            format!("{namespace}::{stem}")
        };
        let decls = crate::virtual_members::laravel::collect_trans_declarations(&content, &prefix);
        for d in decls {
            out.push(d.key);
        }
    }
}

/// Record a key's group/scalar shape, OR-ing into any flag already
/// recorded for the same key from another locale or file.
fn mark_trans_shape(shapes: &mut HashMap<String, bool>, key: String, is_group: bool) {
    let existing = shapes.entry(key).or_insert(false);
    *existing = *existing || is_group;
}

/// The shape counterpart of [`collect_json_trans_keys`]: every JSON
/// translation key is a scalar phrase, never a group.
fn collect_json_trans_key_shapes(backend: &crate::Backend, out: &mut HashMap<String, bool>) {
    let root = match backend.workspace.workspace_root.read().clone() {
        Some(r) => r,
        None => return,
    };
    for sub in &["lang", "resources/lang"] {
        let dir = root.join(sub);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && let Ok(content) = std::fs::read_to_string(&path)
                && let Ok(map) =
                    serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
            {
                for k in map.keys() {
                    mark_trans_shape(out, k.clone(), false);
                }
            }
        }
    }
}

/// The shape counterpart of [`collect_namespaced_trans_keys`].
fn collect_namespaced_trans_key_shapes(
    dir: &std::path::Path,
    namespace: &str,
    out: &mut HashMap<String, bool>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_namespaced_trans_shapes_from_locale_dir(&path, namespace, out);
        } else if path.extension().is_some_and(|e| e == "json")
            && namespace.is_empty()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(map) =
                serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&content)
        {
            for k in map.keys() {
                mark_trans_shape(out, k.clone(), false);
            }
        }
    }
}

fn collect_namespaced_trans_shapes_from_locale_dir(
    dir: &std::path::Path,
    namespace: &str,
    out: &mut HashMap<String, bool>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|e| e == "php") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        let prefix = if namespace.is_empty() {
            stem.to_string()
        } else {
            format!("{namespace}::{stem}")
        };
        let decls = crate::virtual_members::laravel::collect_trans_declarations(&content, &prefix);
        for d in decls {
            mark_trans_shape(out, d.key, d.is_group);
        }
    }
}

// ─── Completion ─────────────────────────────────────────────────────────────

/// The icon an editor shows beside a completed string key: whatever the key
/// names is what it should look like.
fn string_key_item_kind(kind: &LaravelStringKind) -> CompletionItemKind {
    match kind {
        LaravelStringKind::Config | LaravelStringKind::ConfigResource(_) => {
            CompletionItemKind::PROPERTY
        }
        LaravelStringKind::View => CompletionItemKind::FILE,
        LaravelStringKind::Trans => CompletionItemKind::TEXT,
        LaravelStringKind::MorphAlias => CompletionItemKind::ENUM_MEMBER,
        LaravelStringKind::GateAbility => CompletionItemKind::METHOD,
        LaravelStringKind::Route
        | LaravelStringKind::Command
        | LaravelStringKind::Section
        | LaravelStringKind::Stack
        | LaravelStringKind::ContainerBinding
        | LaravelStringKind::RateLimiter
        | LaravelStringKind::QueueName => CompletionItemKind::VALUE,
    }
}

fn merge_sorted_unique(left: Vec<String>, right: Vec<String>) -> Vec<String> {
    let mut left = left.into_iter().peekable();
    let mut right = right.into_iter().peekable();
    let mut merged = Vec::with_capacity(left.size_hint().0 + right.size_hint().0);
    loop {
        match (left.peek(), right.peek()) {
            (Some(a), Some(b)) if a == b => {
                merged.push(left.next().expect("peeked value"));
                right.next();
            }
            (Some(a), Some(b)) if a < b => {
                merged.push(left.next().expect("peeked value"));
            }
            (Some(_), Some(_)) => {
                merged.push(right.next().expect("peeked value"));
            }
            (Some(_), None) => merged.push(left.next().expect("peeked value")),
            (None, Some(_)) => merged.push(right.next().expect("peeked value")),
            (None, None) => return merged,
        }
    }
}

impl Backend {
    /// Every name a string key of `kind` could be, unfiltered.
    ///
    /// Three kinds have no list to offer. A Blade section or stack name is
    /// completed from the raw template instead
    /// (`crate::completion::handler::blade_block_name`): what a name may be
    /// depends on the layouts above the file, and the edit has to land in
    /// Blade coordinates rather than in the virtual PHP this detection reads.
    /// A container binding key is written where a class name is equally
    /// valid, which ordinary class completion already offers, and the set of
    /// keys is open besides — a list of them would read as the whole answer
    /// when it is not.
    fn string_key_candidates(&self, kind: &LaravelStringKind) -> Vec<String> {
        match kind {
            LaravelStringKind::Route => self.cached_route_names(),
            LaravelStringKind::Config => self.cached_config_keys().as_ref().clone(),
            LaravelStringKind::ConfigResource(resource) => {
                let prefix =
                    crate::symbol_map::laravel_resources::descriptor(*resource).config_prefix;
                let keys = self.cached_config_keys();
                let first = keys.partition_point(|key| key.as_str() < prefix);
                let configured = keys[first..]
                    .iter()
                    .take_while(|key| key.starts_with(prefix))
                    .filter_map(|key| {
                        let short = key.strip_prefix(prefix)?;
                        (!short.is_empty() && !short.contains('.')).then(|| short.to_string())
                    })
                    .collect();
                let runtime = self
                    .laravel_source_strings
                    .read()
                    .runtime_config_resource_names(*resource);
                merge_sorted_unique(configured, runtime)
            }
            LaravelStringKind::View => self.cached_view_names(),
            LaravelStringKind::Trans => self.cached_trans_keys(),
            LaravelStringKind::Command => self.laravel_commands.read().all_names(),
            LaravelStringKind::MorphAlias => {
                let mut aliases = self.laravel_morph_map.read().all_aliases();
                aliases.sort();
                aliases
            }
            LaravelStringKind::GateAbility => self.cached_gate_abilities(),
            LaravelStringKind::RateLimiter => {
                self.laravel_source_strings.read().rate_limiter_names()
            }
            LaravelStringKind::QueueName => self.cached_queue_names(),
            LaravelStringKind::Section
            | LaravelStringKind::Stack
            | LaravelStringKind::ContainerBinding => Vec::new(),
        }
    }

    /// Confirm every syntactic `onQueue()` candidate at most once per
    /// workspace generation, then read the small incremental name index.
    fn cached_queue_names(&self) -> Vec<String> {
        loop {
            let generation = {
                let index = self.laravel_source_strings.read();
                if index.queue_names_are_complete() {
                    return index.queue_names();
                }
                index.queue_name_generation()
            };
            for (uri, map) in self.user_file_symbol_maps() {
                if map
                    .resource_receiver_sites
                    .iter()
                    .any(|site| site.rule == LaravelResourceReceiverRule::QueueName)
                {
                    self.typed_receiver_view_spans_for(&uri, &map);
                }
            }
            let mut index = self.laravel_source_strings.write();
            if index.mark_queue_names_complete(generation) {
                return index.queue_names();
            }
        }
    }

    /// Try Laravel string key completion.
    ///
    /// Detects the cursor inside the first string argument of `route()`,
    /// `config()`, `view()`, `__()`, etc. and offers matching key names.
    #[cfg(test)]
    pub(crate) fn try_laravel_string_key_completion(
        &self,
        content: &str,
        position: Position,
    ) -> Option<CompletionResponse> {
        self.try_laravel_string_key_completion_inner(content, position, None)
    }

    /// The live-request form, which can confirm calls whose receiver type or
    /// enclosing class decides which named-resource family they address.
    pub(crate) fn try_laravel_string_key_completion_in_file(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        file_ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        self.try_laravel_string_key_completion_inner(content, position, Some((uri, file_ctx)))
    }

    fn try_laravel_string_key_completion_inner(
        &self,
        content: &str,
        position: Position,
        file_ctx: Option<(&str, &FileContext)>,
    ) -> Option<CompletionResponse> {
        let mut ctx = detect_laravel_string_key_context_inner(
            content,
            position,
            file_ctx.and_then(|(_, context)| context.resolved_names.as_deref()),
        )?;
        if let Some(rule) = ctx.receiver_rule {
            let (uri, file_ctx) = file_ctx?;
            ctx.kind = self.confirm_completion_resource_kind(
                uri,
                content,
                position,
                file_ctx,
                rule,
                ctx.receiver_subject.as_deref(),
            )?;
        }

        let candidates = self.string_key_candidates(&ctx.kind);

        // Build the TextEdit range: from the start of the string content
        // (right after the opening quote) to the current cursor position.
        // This replaces the entire typed prefix with the selected name,
        // so dots in the name don't break the editor's word-based filter.
        let start_pos = crate::text_position::offset_to_position(content, ctx.content_start_offset);
        let edit_range = Range {
            start: start_pos,
            end: position,
        };

        let item_kind = string_key_item_kind(&ctx.kind);
        let prefix = ctx.prefix.as_bytes();
        let items: Vec<CompletionItem> = candidates
            .into_iter()
            .filter(|name| {
                name.as_bytes()
                    .get(..prefix.len())
                    .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
            })
            .enumerate()
            .map(|(i, name)| CompletionItem {
                label: name.clone(),
                kind: Some(item_kind),
                sort_text: Some(format!("{:05}", i)),
                filter_text: Some(name.clone()),
                text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                    range: edit_range,
                    new_text: name,
                })),
                ..Default::default()
            })
            .collect();

        if items.is_empty() {
            None
        } else {
            Some(CompletionResponse::Array(items))
        }
    }

    fn confirm_completion_resource_kind(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        file_ctx: &FileContext,
        rule: LaravelResourceReceiverRule,
        subject: Option<&str>,
    ) -> Option<LaravelStringKind> {
        let cursor_offset = position_to_offset(content, position);
        let current_class =
            crate::class_lookup::find_class_at_offset(&file_ctx.classes, cursor_offset);
        let class_loader = self.class_loader(file_ctx);

        if rule == LaravelResourceReceiverRule::ConnectionProperty {
            return crate::symbol_map::laravel_resources::classify_connection_property(
                current_class?,
                &class_loader,
            );
        }

        let function_loader = self.function_loader(file_ctx);
        let laravel_macro_this_resolver = self.laravel_macro_this_resolver(&class_loader);
        let rctx = ResolutionCtx {
            current_class,
            all_classes: &file_ctx.classes,
            content,
            cursor_offset,
            class_loader: &class_loader,
            backend: Some(self),
            laravel_macro_this_resolver: Some(&laravel_macro_this_resolver),
            resolved_class_cache: Some(&self.resolved_class_cache),
            function_loader: Some(&function_loader),
            scope_var_resolver: None,
            is_in_static_method: self
                .symbol_map_for(uri)
                .is_some_and(|map| map.is_in_static_method(cursor_offset)),
            preserve_static: false,
        };
        unanimous_resource_kind(
            resolve_target_classes(subject?, AccessKind::Arrow, &rctx)
                .into_iter()
                .map(|resolved_type| {
                    crate::symbol_map::laravel_resources::classify_receiver_type(
                        rule,
                        &resolved_type.type_string,
                        &class_loader,
                    )
                }),
        )
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::Position;

    fn completion_response_items(response: CompletionResponse) -> Vec<CompletionItem> {
        match response {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        }
    }

    /// Three kinds are recorded as spans but completed from somewhere else,
    /// or not at all, so they must offer nothing here rather than an empty
    /// list dressed up as the answer.
    #[test]
    fn the_kinds_completed_elsewhere_offer_no_candidates() {
        let backend = crate::test_fixtures::make_backend();
        for kind in [
            LaravelStringKind::Section,
            LaravelStringKind::Stack,
            LaravelStringKind::ContainerBinding,
        ] {
            assert!(
                backend.string_key_candidates(&kind).is_empty(),
                "{kind:?} should offer no candidates"
            );
        }
    }

    /// Whatever a key names decides the icon beside it.
    #[test]
    fn a_string_key_is_iconed_by_what_it_names() {
        use tower_lsp::lsp_types::CompletionItemKind;
        for (kind, expected) in [
            (LaravelStringKind::Config, CompletionItemKind::PROPERTY),
            (
                LaravelStringKind::ConfigResource(LaravelConfigResource::CacheStore),
                CompletionItemKind::PROPERTY,
            ),
            (LaravelStringKind::View, CompletionItemKind::FILE),
            (LaravelStringKind::Trans, CompletionItemKind::TEXT),
            (
                LaravelStringKind::MorphAlias,
                CompletionItemKind::ENUM_MEMBER,
            ),
            (LaravelStringKind::GateAbility, CompletionItemKind::METHOD),
            (LaravelStringKind::Route, CompletionItemKind::VALUE),
            (LaravelStringKind::Command, CompletionItemKind::VALUE),
            (LaravelStringKind::Section, CompletionItemKind::VALUE),
            (LaravelStringKind::Stack, CompletionItemKind::VALUE),
            (
                LaravelStringKind::ContainerBinding,
                CompletionItemKind::VALUE,
            ),
            (LaravelStringKind::RateLimiter, CompletionItemKind::VALUE),
            (LaravelStringKind::QueueName, CompletionItemKind::VALUE),
        ] {
            assert_eq!(string_key_item_kind(&kind), expected, "for {kind:?}");
        }
    }

    #[test]
    fn detects_route_call() {
        let content = "<?php\nroute('user.');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("user.").unwrap() as u32 + 5;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect route() context");
        assert!(matches!(ctx.kind, LaravelStringKind::Route));
        assert_eq!(ctx.prefix, "user.");
        assert_eq!(
            ctx.prefix.as_ptr(),
            content[content.find("user.").unwrap()..].as_ptr(),
            "detection should borrow the typed prefix instead of allocating it"
        );
    }

    #[test]
    fn rejects_a_cursor_before_any_string_content() {
        assert!(
            detect_laravel_string_key_context("<?php route('home');", Position::new(0, 0))
                .is_none()
        );
    }

    #[test]
    fn detects_to_route_call() {
        let content = "<?php\nto_route('home');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("home").unwrap() as u32 + 2;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect to_route() context");
        assert!(matches!(ctx.kind, LaravelStringKind::Route));
        assert_eq!(ctx.prefix, "ho");
    }

    /// The preprocessor compiles Blade's render directives into marker
    /// calls, so completion inside `@include('` and `@each('` reaches the
    /// view index through those names.
    #[test]
    fn detects_the_blade_render_directive_markers() {
        for marker in ["blade_view_directive", "blade_each_directive"] {
            let content = format!("<?php\n{marker} ('partials.');\n");
            let line_text = content.lines().nth(1).unwrap();
            let col = line_text.find("partials.").unwrap() as u32 + 9;
            let ctx = detect_laravel_string_key_context(&content, Position::new(1, col));
            let ctx = ctx.unwrap_or_else(|| panic!("should detect {marker} context"));
            assert!(matches!(ctx.kind, LaravelStringKind::View));
            assert_eq!(ctx.prefix, "partials.");
        }
    }

    #[test]
    fn detects_artisan_call_command() {
        let content = "<?php\nArtisan::call('app:');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("app:").unwrap() as u32 + 4;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect Artisan::call() context");
        assert!(matches!(ctx.kind, LaravelStringKind::Command));
    }

    #[test]
    fn detects_this_call_command() {
        let content = "<?php\n$this->call('app:');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("app:").unwrap() as u32 + 4;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect $this->call() context");
        assert!(matches!(ctx.kind, LaravelStringKind::Command));
    }

    #[test]
    fn rejects_non_this_call() {
        // `->call()` on an arbitrary object is not a command reference.
        let content = "<?php\n$service->call('doSomething');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("doSomething").unwrap() as u32 + 2;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        assert!(
            ctx.is_none(),
            "->call() on a non-$this receiver should not match"
        );
    }

    #[test]
    fn detects_gate_facade_ability() {
        let content = "<?php\nGate::allows('upd');\n";
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("upd").unwrap() as u32 + 3;
        let ctx = detect_laravel_string_key_context(content, Position::new(1, col))
            .expect("should detect Gate::allows() context");
        assert!(matches!(ctx.kind, LaravelStringKind::GateAbility));
        assert_eq!(ctx.prefix, "upd");
    }

    /// Every entry point that names an ability completes from the same set.
    #[test]
    fn detects_every_ability_call_shape() {
        for call in [
            // The facade's own API, including the registration itself.
            "Gate::allows('upd'",
            "Gate::denies('upd'",
            "Gate::check('upd'",
            "Gate::any('upd'",
            "Gate::none('upd'",
            "Gate::authorize('upd'",
            "Gate::inspect('upd'",
            "Gate::has('upd'",
            "Gate::define('upd'",
            // A chain rooted at the facade.
            "Gate::forUser($user)->allows('upd'",
            "Gate::forUser($user)->authorize('upd'",
            "Gate::forUser($user)->has('upd'",
            // A controller's own helper.
            "$this->authorize('upd'",
            // The user the check is about.
            "$user->can('upd'",
            "$user->cannot('upd'",
            "$user->canAny('upd'",
            // A route registration.
            "Route::get('/p', $a)->can('upd'",
            // The Blade `@can` directive, after preprocessing.
            "blade_can_directive('upd'",
        ] {
            let content = format!("<?php\n{call});\n");
            let line_text = content.lines().nth(1).unwrap();
            let col = line_text.rfind("upd").unwrap() as u32 + 3;
            let ctx = detect_laravel_string_key_context(&content, Position::new(1, col))
                .unwrap_or_else(|| panic!("`{call}` should offer abilities"));
            assert!(
                matches!(ctx.kind, LaravelStringKind::GateAbility),
                "`{call}` should offer abilities, got {:?}",
                ctx.kind
            );
            assert_eq!(ctx.prefix, "upd", "for `{call}`");
        }
    }

    #[test]
    fn detects_user_can_ability() {
        let content = "<?php\n$user->can('upd', $post);\n";
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("upd").unwrap() as u32 + 3;
        let ctx = detect_laravel_string_key_context(content, Position::new(1, col))
            .expect("should detect $user->can() context");
        assert!(matches!(ctx.kind, LaravelStringKind::GateAbility));
    }

    #[test]
    fn rejects_can_on_an_unrelated_receiver() {
        let content = "<?php\n$rules->can('read');\n";
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("read").unwrap() as u32 + 4;
        assert!(
            detect_laravel_string_key_context(content, Position::new(1, col)).is_none(),
            "->can() on a receiver that is not a user must not match"
        );
    }

    /// A `Gate::` call earlier in the file must not turn an unrelated
    /// `->has()` several statements later into an ability check.
    #[test]
    fn a_gate_call_in_an_earlier_statement_does_not_leak() {
        let content = "<?php\nGate::allows('update');\n$bag->has('key');\n";
        let line_text = content.lines().nth(2).unwrap();
        let col = line_text.find("key").unwrap() as u32 + 3;
        assert!(
            detect_laravel_string_key_context(content, Position::new(2, col)).is_none(),
            "an earlier Gate:: statement must not reach a later chain"
        );
    }

    #[test]
    fn detects_config_call() {
        let content = "<?php\nconfig('app.');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("app.").unwrap() as u32 + 4;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect config() context");
        assert!(matches!(ctx.kind, LaravelStringKind::Config));
        assert_eq!(ctx.prefix, "app.");
    }

    #[test]
    fn detects_config_static_get() {
        let content = "<?php\nConfig::get('app.name');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("app.").unwrap() as u32 + 4;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect Config::get() context");
        assert!(matches!(ctx.kind, LaravelStringKind::Config));
        assert_eq!(ctx.prefix, "app.");
    }

    #[test]
    fn detects_every_storage_disk_method() {
        for method in ["disk", "fake", "persistentFake", "forgetDisk"] {
            let content = format!("<?php\nStorage::{method}('arch');\n");
            let line_text = content.lines().nth(1).unwrap();
            let col = line_text.find("arch").unwrap() as u32 + 4;
            let ctx = detect_laravel_string_key_context(&content, Position::new(1, col))
                .unwrap_or_else(|| panic!("should detect Storage::{method}() context"));
            assert_eq!(
                ctx.kind,
                LaravelStringKind::ConfigResource(LaravelConfigResource::StorageDisk)
            );
            assert_eq!(ctx.prefix, "arch");
        }
    }

    #[test]
    fn rejects_other_storage_methods() {
        let content = "<?php\nStorage::extend('archive', fn () => null);\n";
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("archive").unwrap() as u32 + 3;
        assert!(
            detect_laravel_string_key_context(content, Position::new(1, col)).is_none(),
            "Storage::extend() names a driver, not a disk"
        );
    }

    #[test]
    fn detects_each_forget_disk_array_value() {
        for content in [
            "<?php\nStorage::forgetDisk(['archive', 'back']);\n",
            "<?php\nStorage::forgetDisk(array('archive', 'back'));\n",
        ] {
            for value in ["archive", "back"] {
                let line_text = content.lines().nth(1).unwrap();
                let col = line_text.find(value).unwrap() as u32 + value.len() as u32;
                let ctx = detect_laravel_string_key_context(content, Position::new(1, col))
                    .unwrap_or_else(|| panic!("should detect `{value}` as a disk name"));
                assert_eq!(
                    ctx.kind,
                    LaravelStringKind::ConfigResource(LaravelConfigResource::StorageDisk)
                );
                assert_eq!(ctx.prefix, value);
            }
        }
    }

    #[test]
    fn connection_values_require_a_property_or_promoted_parameter() {
        assert!(is_connection_property_value(
            "<?php class Job { public string $connection ="
        ));
        assert!(is_connection_property_value(
            "<?php class Job { public function __construct(public string $connection ="
        ));
        assert!(!is_connection_property_value(
            "<?php class Job { public function run(string $connection ="
        ));
        assert!(!is_connection_property_value(
            "<?php class Job { public static string $connection ="
        ));
        assert!(!is_connection_property_value("="));
    }

    #[test]
    fn argument_scanners_handle_escaped_and_nested_php_syntax() {
        let escaped_array = r#"<?php Log::stack(['it\'s', 'sla']);"#;
        let cursor = escaped_array.rfind("sla").unwrap() + 3;
        let ctx = detect_laravel_string_key_context(
            escaped_array,
            crate::text_position::offset_to_position(escaped_array, cursor),
        )
        .expect("an escaped quote in an earlier value must not hide the array call");
        assert_eq!(
            ctx.kind,
            LaravelStringKind::ConfigResource(LaravelConfigResource::LogChannel)
        );

        let escaped_key = r#"'key\'part' => 'daily'"#;
        assert!(string_literal_is_array_key(
            escaped_key,
            "'key".len(),
            b'\''
        ));
        assert!(!string_literal_is_array_key("'key\n", "'key".len(), b'\''));
        assert!(!string_literal_is_array_key("'key", "'key".len(), b'\''));

        let before_value = r#"Storage::fake(config: ['message' => 'it\'s', 'factory' => wrap(fn () => new class {})], disk:"#;
        assert_eq!(
            callable_before_scalar_argument(before_value),
            Some(("Storage::fake", Some("disk")))
        );
        assert!(callable_before_scalar_argument("Storage::disk(:").is_none());
        assert!(enclosing_call_open_paren("completed(); orphan").is_none());
        assert!(enclosing_call_open_paren("orphan").is_none());
    }

    #[test]
    fn facade_chain_scanning_handles_spacing_and_multiple_static_calls() {
        assert!(is_laravel_facade("Gate", "Gate"));
        assert!(is_laravel_facade(
            "\\Illuminate\\Support\\Facades\\Gate",
            "Gate"
        ));
        assert!(!is_laravel_facade("App\\Gate", "Gate"));

        assert!(chain_contains_laravel_facade(
            "Other::make()-> Gate \t::forUser($user)",
            0,
            None,
            "Gate"
        ));
        assert!(!chain_contains_laravel_facade(
            "Other::make()",
            0,
            None,
            "Gate"
        ));
        assert!(!chain_contains_laravel_facade("::", 0, None, "Gate"));
    }

    #[test]
    fn rejects_arrays_for_scalar_storage_methods() {
        let content = "<?php\nStorage::disk(['archive']);\n";
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("archive").unwrap() as u32 + 4;
        assert!(
            detect_laravel_string_key_context(content, Position::new(1, col)).is_none(),
            "Storage::disk() accepts one disk, not an array"
        );
    }

    #[test]
    fn detects_each_config_resource_family_and_array_shape() {
        for (call, expected) in [
            ("auth('na')", LaravelConfigResource::AuthGuard),
            ("Auth::guard('na')", LaravelConfigResource::AuthGuard),
            ("Cache::store('na')", LaravelConfigResource::CacheStore),
            ("Log::channel('na')", LaravelConfigResource::LogChannel),
            ("Log::stack(['na'])", LaravelConfigResource::LogChannel),
            ("Log::stack(array('na'))", LaravelConfigResource::LogChannel),
            (
                "DB::connection('na')",
                LaravelConfigResource::DatabaseConnection,
            ),
            (
                "Queue::connection('na')",
                LaravelConfigResource::QueueConnection,
            ),
            ("Mail::mailer('na')", LaravelConfigResource::Mailer),
            (
                "Broadcast::connection('na')",
                LaravelConfigResource::BroadcastConnection,
            ),
        ] {
            let content = format!("<?php\n{call};\n");
            let position =
                crate::text_position::offset_to_position(&content, content.find("na").unwrap() + 2);
            let ctx = detect_laravel_string_key_context(&content, position)
                .unwrap_or_else(|| panic!("should detect `{call}`"));
            assert_eq!(ctx.kind, LaravelStringKind::ConfigResource(expected));
        }
    }

    #[test]
    fn detects_fallback_static_and_instance_string_contexts() {
        for (call, expected) in [
            ("View::exists('name')", LaravelStringKind::View),
            ("Lang::choice('name')", LaravelStringKind::Trans),
            ("Schedule::command('name')", LaravelStringKind::Command),
            (
                "Model::getActualClassNameForMorph('name')",
                LaravelStringKind::MorphAlias,
            ),
            ("$router->route('name')", LaravelStringKind::Route),
        ] {
            let content = format!("<?php\n{call};\n");
            let position = crate::text_position::offset_to_position(
                &content,
                content.find("name").unwrap() + 4,
            );
            let ctx = detect_laravel_string_key_context(&content, position)
                .unwrap_or_else(|| panic!("should detect `{call}`"));
            assert_eq!(ctx.kind, expected, "context for `{call}`");
        }
    }

    #[test]
    fn rejects_wrong_named_arguments_and_array_shapes() {
        for expression in [
            "#[\\Illuminate\\Container\\Attributes\\Config(value: 'name')] class C {}",
            "View::make(['name'])",
            "auth(['name'])",
        ] {
            let content = format!("<?php\n{expression};\n");
            let position = crate::text_position::offset_to_position(
                &content,
                content.find("name").unwrap() + 4,
            );
            assert!(
                detect_laravel_string_key_context(&content, position).is_none(),
                "`{expression}` must not be treated as a supported string-key argument"
            );
        }
    }

    #[test]
    fn middleware_completion_replaces_only_a_named_payload() {
        for (literal, expected_kind, expected_prefix, expected_edit) in [
            (
                "auth:web, ad",
                LaravelStringKind::ConfigResource(LaravelConfigResource::AuthGuard),
                "ad",
                " ad",
            ),
            ("throttle:up", LaravelStringKind::RateLimiter, "up", "up"),
            ("can:upd", LaravelStringKind::GateAbility, "upd", "upd"),
        ] {
            let content = format!("<?php Route::middleware('{literal}');");
            let cursor = content.find(literal).unwrap() + literal.len();
            let ctx = detect_laravel_string_key_context(
                &content,
                crate::text_position::offset_to_position(&content, cursor),
            )
            .unwrap_or_else(|| panic!("should detect `{literal}`"));
            assert_eq!(ctx.kind, expected_kind);
            assert_eq!(ctx.prefix, expected_prefix);
            assert_eq!(
                &content[ctx.content_start_offset..cursor],
                expected_edit,
                "the completion edit must leave the middleware alias intact"
            );
        }

        for expression in [
            "$this->middleware('auth:we')",
            "Route::get('/', $handler)->middleware(['auth:we'])",
        ] {
            let content = format!("<?php {expression};");
            let cursor = content.find("auth:we").unwrap() + "auth:we".len();
            let ctx = detect_laravel_string_key_context(
                &content,
                crate::text_position::offset_to_position(&content, cursor),
            )
            .unwrap_or_else(|| panic!("should detect `{expression}`"));
            assert_eq!(
                ctx.kind,
                LaravelStringKind::ConfigResource(LaravelConfigResource::AuthGuard)
            );
            assert_eq!(ctx.prefix, "we");
        }

        let numeric = "<?php Route::middleware('throttle:60');";
        let cursor = numeric.find("throttle:60").unwrap() + "throttle:60".len();
        let ctx = detect_laravel_string_key_context(
            numeric,
            crate::text_position::offset_to_position(numeric, cursor),
        )
        .expect("a numeric limiter may also be a registered literal name");
        assert_eq!(ctx.kind, LaravelStringKind::RateLimiter);
        assert_eq!(ctx.prefix, "60");

        for literal in ["throttle:60|120,1", "can:update,Post"] {
            let content = format!("<?php Route::middleware('{literal}');");
            let cursor = content.find(literal).unwrap() + literal.len();
            assert!(
                detect_laravel_string_key_context(
                    &content,
                    crate::text_position::offset_to_position(&content, cursor),
                )
                .is_none(),
                "`{literal}` has no named payload at the cursor"
            );
        }
    }

    #[test]
    fn detects_short_and_fully_qualified_rate_limiter_constructors() {
        for class in [
            "RateLimited",
            "RateLimitedWithRedis",
            "\\Illuminate\\Queue\\Middleware\\RateLimited",
            "\\Illuminate\\Queue\\Middleware\\RateLimitedWithRedis",
        ] {
            let content = format!("<?php new {class}('api');");
            let cursor = content.find("api").unwrap() + 3;
            let ctx = detect_laravel_string_key_context(
                &content,
                crate::text_position::offset_to_position(&content, cursor),
            )
            .unwrap_or_else(|| panic!("should detect `new {class}`"));
            assert_eq!(ctx.kind, LaravelStringKind::RateLimiter);
        }
        let unrelated = "<?php new Acme\\RateLimited('api');";
        assert!(
            detect_laravel_string_key_context(
                unrelated,
                crate::text_position::offset_to_position(
                    unrelated,
                    unrelated.find("api").unwrap() + 3,
                ),
            )
            .is_none()
        );
        assert!(
            rate_limited_constructor_class("renew RateLimited").is_none(),
            "`new` must be a standalone keyword"
        );
    }

    #[test]
    fn type_dependent_completion_contexts_carry_their_confirmation_rule() {
        for (expression, rule) in [
            (
                "$manager->connection('name')",
                LaravelResourceReceiverRule::ConnectionMethod,
            ),
            (
                "$manager->connection(connection: 'name')",
                LaravelResourceReceiverRule::ConnectionMethod,
            ),
            (
                "$job->onConnection('name')",
                LaravelResourceReceiverRule::QueueableConnection,
            ),
            (
                "$job?->onQueue('name')",
                LaravelResourceReceiverRule::QueueName,
            ),
            (
                "protected $connection = 'name'",
                LaravelResourceReceiverRule::ConnectionProperty,
            ),
        ] {
            let content = format!("<?php class C {{ {expression}; }}");
            let cursor = content.find("name").unwrap() + 4;
            let ctx = detect_laravel_string_key_context(
                &content,
                crate::text_position::offset_to_position(&content, cursor),
            )
            .unwrap_or_else(|| panic!("should detect `{expression}`"));
            assert_eq!(ctx.receiver_rule, Some(rule));
        }

        let static_property = "<?php class C { protected static $connection = 'name'; }";
        assert!(
            detect_laravel_string_key_context(
                static_property,
                crate::text_position::offset_to_position(
                    static_property,
                    static_property.find("name").unwrap() + 4,
                ),
            )
            .is_none()
        );
    }

    #[test]
    fn detects_view_call() {
        let content = "<?php\nview('users.');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("users.").unwrap() as u32 + 6;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect view() context");
        assert!(matches!(ctx.kind, LaravelStringKind::View));
    }

    #[test]
    fn detects_trans_double_underscore() {
        let content = "<?php\n__('messages.');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("messages.").unwrap() as u32 + 9;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect __() context");
        assert!(matches!(ctx.kind, LaravelStringKind::Trans));
    }

    #[test]
    fn detects_empty_prefix() {
        let content = "<?php\nroute('');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("''").unwrap() as u32 + 1;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect empty prefix");
        assert!(matches!(ctx.kind, LaravelStringKind::Route));
        assert_eq!(ctx.prefix, "");
    }

    #[test]
    fn rejects_second_arg() {
        let content = "<?php\nroute('name', 'param');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("param").unwrap() as u32 + 2;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        assert!(ctx.is_none(), "Second argument should not match");
    }

    #[test]
    fn rejects_non_laravel_function() {
        let content = "<?php\nfoo('bar');\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("bar").unwrap() as u32 + 1;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        assert!(ctx.is_none(), "Non-Laravel function should not match");
    }

    #[test]
    fn rejects_a_string_passed_to_an_invoked_expression() {
        let content = "<?php\nfactory()('name');\n";
        let cursor = content.find("name").unwrap() + "name".len();
        assert!(
            detect_laravel_string_key_context(
                content,
                crate::text_position::offset_to_position(content, cursor),
            )
            .is_none()
        );
    }

    #[test]
    fn lang_file_stem_extraction() {
        assert_eq!(
            extract_lang_file_stem("file:///app/lang/en/messages.php"),
            Some("messages".to_string())
        );
        assert_eq!(
            extract_lang_file_stem("file:///app/resources/lang/en/validation.php"),
            Some("validation".to_string())
        );
    }

    #[test]
    fn detects_config_attribute_with_import() {
        let content =
            "<?php\nuse Illuminate\\Container\\Attributes\\Config;\n#[Config('app.timezone')]\n";
        let line = 2;
        let line_text = content.lines().nth(2).unwrap();
        let col = line_text.find("app.timezone").unwrap() as u32 + 12;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect #[Config()] with verified import");
        assert!(matches!(ctx.kind, LaravelStringKind::Config));
        assert_eq!(ctx.prefix, "app.timezone");
    }

    #[test]
    fn rejects_config_attribute_without_import() {
        let content = "<?php\n#[Config('app.timezone')]\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("app.timezone").unwrap() as u32 + 12;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        assert!(
            ctx.is_none(),
            "Should reject #[Config()] without verified import"
        );
    }

    #[test]
    fn unrelated_attribute_does_not_break_detection() {
        let content = "<?php\nclass Foo {\n    #[Override]\n    public function bar(): void {\n        route('');\n    }\n}\n";
        let line = 4;
        let line_text = content.lines().nth(line).unwrap();
        let col = line_text.find("''").unwrap() as u32 + 1;
        let ctx = detect_laravel_string_key_context(content, Position::new(line as u32, col));
        assert!(
            ctx.is_some(),
            "route('') must be detected even when #[Override] exists earlier in the file"
        );
        assert!(matches!(ctx.unwrap().kind, LaravelStringKind::Route));
    }

    #[test]
    fn detects_fqn_config_attribute() {
        let content = "<?php\n#[\\Illuminate\\Container\\Attributes\\Config('app.')]\n";
        let line = 1;
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("app.").unwrap() as u32 + 4;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        let ctx = ctx.expect("should detect FQN #[Config()] attribute");
        assert!(matches!(ctx.kind, LaravelStringKind::Config));
        assert_eq!(ctx.prefix, "app.");
    }

    #[test]
    fn detects_fqn_storage_attribute() {
        let content = "<?php\n#[\\Illuminate\\Container\\Attributes\\Storage('arch')]\n";
        let line_text = content.lines().nth(1).unwrap();
        let col = line_text.find("arch").unwrap() as u32 + 4;
        let ctx = detect_laravel_string_key_context(content, Position::new(1, col))
            .expect("should detect FQN #[Storage()] attribute");
        assert_eq!(
            ctx.kind,
            LaravelStringKind::ConfigResource(LaravelConfigResource::StorageDisk)
        );
        assert_eq!(ctx.prefix, "arch");
    }

    #[test]
    fn detects_route_in_module_controller() {
        let content = "<?php\n\
\n\
namespace Acme\\User\\Http\\Controllers;\n\
\n\
use App\\Http\\Controllers\\Abstracts\\BaseController;\n\
use Illuminate\\Http\\RedirectResponse;\n\
use Illuminate\\Http\\Request;\n\
\n\
final class UserPermissionController extends BaseController\n\
{\n\
    public function copy(Request $request): RedirectResponse\n\
    {\n\
        route('');\n\
\n\
        return to_route('admin::user.permissions.edit', 1);\n\
    }\n\
}\n";
        // route('') is on line 12 (0-indexed), cursor at character 15 (between quotes)
        let line = content
            .lines()
            .enumerate()
            .find(|(_, l)| l.contains("route('')"))
            .map(|(i, _)| i as u32)
            .expect("should find route('') line");
        let line_text = content.lines().nth(line as usize).unwrap();
        let col = line_text.find("''").unwrap() as u32 + 1;
        let ctx = detect_laravel_string_key_context(content, Position::new(line, col));
        assert!(
            ctx.is_some(),
            "should detect route('') context in module controller at line {}, col {}",
            line,
            col,
        );
        let ctx = ctx.unwrap();
        assert!(matches!(ctx.kind, LaravelStringKind::Route));
    }

    #[test]
    fn route_completion_end_to_end() {
        let backend = crate::Backend::new_test();

        let route_uri = "file:///app/routes/web.php";
        let route_content = "<?php\n\
            use Illuminate\\Support\\Facades\\Route;\n\
            Route::get('/home', fn() => 'home')->name('home');\n\
            Route::get('/about', fn() => 'about')->name('about');\n";
        backend.open_files.write().insert(
            route_uri.to_string(),
            std::sync::Arc::new(route_content.to_string()),
        );
        backend.update_ast(route_uri, route_content);

        let test_content = "<?php\nroute('');\n";
        backend.update_ast("file:///app/Http/Controllers/Test.php", test_content);

        let names = backend.cached_route_names();
        assert!(
            names.contains(&"home".to_string()),
            "cached_route_names should contain 'home', got: {:?}",
            names
        );

        let response = backend.try_laravel_string_key_completion(test_content, Position::new(1, 7));
        assert!(
            response.is_some(),
            "try_laravel_string_key_completion should return Some for route('')"
        );
        let items = completion_response_items(response.expect("route completion response"));
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"home"),
            "completion should include 'home', got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"about"),
            "completion should include 'about', got: {:?}",
            labels
        );
    }

    #[test]
    fn completion_item_test_helper_accepts_list_responses() {
        let response = CompletionResponse::List(CompletionList {
            is_incomplete: false,
            items: vec![CompletionItem::default()],
        });
        assert_eq!(completion_response_items(response).len(), 1);
    }

    #[test]
    fn sorted_config_candidates_merge_without_duplicates() {
        let names = |values: &[&str]| values.iter().map(|value| (*value).to_string()).collect();
        assert_eq!(
            merge_sorted_unique(
                names(&["alpha", "charlie"]),
                names(&["alpha", "bravo", "delta"]),
            ),
            names(&["alpha", "bravo", "charlie", "delta"])
        );
        assert_eq!(
            merge_sorted_unique(names(&["alpha"]), Vec::new()),
            names(&["alpha"])
        );
        assert_eq!(
            merge_sorted_unique(Vec::new(), names(&["bravo"])),
            names(&["bravo"])
        );

        let backend = crate::Backend::new_test();
        {
            let mut cache = backend.laravel_string_key_cache.write();
            cache.config_keys = Some(std::sync::Arc::new(names(&["app.name", "cache.default"])));
            cache.config_open_prefixes = Some(std::sync::Arc::new(Vec::new()));
        }
        assert_eq!(
            backend.string_key_candidates(&LaravelStringKind::Config),
            names(&["app.name", "cache.default"])
        );
    }

    #[test]
    fn config_metadata_publication_rejects_stale_scans() {
        let keys = std::sync::Arc::new(vec!["cache.default".to_string()]);
        let open_prefixes = std::sync::Arc::new(vec!["services.runtime.".to_string()]);
        let metadata = (
            std::sync::Arc::clone(&keys),
            std::sync::Arc::clone(&open_prefixes),
        );
        let mut cache = crate::LaravelStringKeyCache::default();

        assert!(config_metadata_snapshot(&cache).is_none());
        cache.config_keys = Some(std::sync::Arc::clone(&keys));
        assert!(
            config_metadata_snapshot(&cache).is_none(),
            "a partially published pair must remain a cache miss"
        );

        cache.config_keys = None;
        cache.config_generation = 2;
        assert!(!publish_config_metadata(&mut cache, 1, &metadata));
        assert!(cache.config_keys.is_none());
        assert!(cache.config_open_prefixes.is_none());

        assert!(publish_config_metadata(&mut cache, 2, &metadata));
        let (published_keys, published_prefixes) =
            config_metadata_snapshot(&cache).expect("matching generations publish atomically");
        assert!(std::sync::Arc::ptr_eq(&published_keys, &keys));
        assert!(std::sync::Arc::ptr_eq(&published_prefixes, &open_prefixes));
    }

    #[test]
    fn config_metadata_cache_retries_a_scan_overtaken_by_invalidation() {
        let backend = crate::Backend::new_test();
        let scans = std::cell::Cell::new(0);
        let (keys, open_prefixes) = backend.cached_config_metadata_with(|| {
            let scan = scans.get();
            scans.set(scan + 1);
            if scan == 0 {
                backend.laravel_string_key_cache.write().config_generation += 1;
            }
            (
                vec!["cache.default".to_string()],
                vec!["services.runtime.".to_string()],
            )
        });

        assert_eq!(scans.get(), 2, "the stale first scan must be rebuilt");
        assert_eq!(keys.as_slice(), &["cache.default"]);
        assert_eq!(open_prefixes.as_slice(), &["services.runtime."]);
    }

    #[test]
    fn config_metadata_cache_reuses_a_build_completed_before_lock_acquisition() {
        let backend = crate::Backend::new_test();
        let expected_keys = std::sync::Arc::new(vec!["cache.default".to_string()]);
        let expected_prefixes = std::sync::Arc::new(vec!["services.runtime.".to_string()]);
        {
            let mut cache = backend.laravel_string_key_cache.write();
            cache.config_keys = Some(std::sync::Arc::clone(&expected_keys));
            cache.config_open_prefixes = Some(std::sync::Arc::clone(&expected_prefixes));
        }

        let snapshots = std::cell::Cell::new(0);
        let (keys, prefixes) = backend.cached_config_metadata_with_snapshot(
            |cache| {
                let snapshot = snapshots.get();
                snapshots.set(snapshot + 1);
                (snapshot > 0)
                    .then(|| config_metadata_snapshot(cache))
                    .flatten()
            },
            std::default::Default::default,
        );

        assert_eq!(snapshots.get(), 2);
        assert!(std::sync::Arc::ptr_eq(&keys, &expected_keys));
        assert!(std::sync::Arc::ptr_eq(&prefixes, &expected_prefixes));
    }

    #[test]
    fn receiver_unions_require_one_resource_family() {
        let cache = LaravelStringKind::ConfigResource(LaravelConfigResource::CacheStore);
        let database = LaravelStringKind::ConfigResource(LaravelConfigResource::DatabaseConnection);

        assert_eq!(unanimous_resource_kind([]), None);
        assert_eq!(unanimous_resource_kind([Some(cache)]), Some(cache));
        assert_eq!(
            unanimous_resource_kind([Some(cache), Some(cache)]),
            Some(cache)
        );
        assert_eq!(unanimous_resource_kind([Some(cache), None]), None);
        assert_eq!(unanimous_resource_kind([Some(cache), Some(database)]), None);
    }

    /// Concurrent first callers must share one build, not run one each:
    /// every enumeration behind these accessors walks the workspace from
    /// disk, and the diagnostic pass hits them from all N workers at once.
    #[test]
    fn concurrent_first_callers_build_the_enumeration_once() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let backend = crate::Backend::new_test();
        let builds = AtomicUsize::new(0);
        let build_lock = parking_lot::Mutex::new(());
        let start = std::sync::Barrier::new(17);

        let results: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|_| {
                    let backend = &backend;
                    let builds = &builds;
                    let build_lock = &build_lock;
                    let start = &start;
                    scope.spawn(move || {
                        start.wait();
                        backend.cached_laravel_enumeration(
                            build_lock,
                            |cache| cache.view_names.clone(),
                            |cache, names| cache.view_names = Some(names),
                            || {
                                builds.fetch_add(1, Ordering::SeqCst);
                                vec!["home".to_string()]
                            },
                        )
                    })
                })
                .collect();
            start.wait();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });

        assert_eq!(
            builds.load(Ordering::SeqCst),
            1,
            "the enumeration must be built once and shared, not once per caller"
        );
        for names in &results {
            assert_eq!(names, &vec!["home".to_string()]);
        }
    }
}
