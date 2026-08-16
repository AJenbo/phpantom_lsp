//! Incremental names defined by Laravel string literals in application code.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use crate::symbol_map::{
    LaravelConfigResource, LaravelStringKind, SymbolKind, SymbolMap, SymbolSpan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedSpan {
    name: String,
    start: u32,
    end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigResourceSpan {
    kind: LaravelConfigResource,
    name: String,
    start: u32,
    end: u32,
}

/// One exact source occurrence indexed as a Laravel string declaration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct LaravelSourceStringDefinition {
    pub(crate) uri: Arc<str>,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

/// The source-defined names that can be read directly from one symbol map.
///
/// Building this value clones and sorts the discovered names, so callers do
/// that work before taking the shared source-index write lock.
#[derive(Default)]
pub(crate) struct LaravelSourceStringContributions {
    rate_limiters: Vec<NamedSpan>,
    config_resources: Vec<ConfigResourceSpan>,
    dynamic_rate_limiter: bool,
    has_queue_candidates: bool,
}

impl LaravelSourceStringContributions {
    /// Extract, sort, and deduplicate one map's direct contributions.
    pub(crate) fn from_symbol_map(map: &SymbolMap) -> Self {
        let mut rate_limiters = Vec::new();
        let mut config_resources = Vec::new();
        for span in &map.spans {
            match &span.kind {
                SymbolKind::LaravelStringKey {
                    kind: LaravelStringKind::RateLimiter,
                    key,
                    is_write: true,
                    ..
                } => rate_limiters.push(NamedSpan {
                    name: key.clone(),
                    start: span.start,
                    end: span.end,
                }),
                SymbolKind::LaravelStringKey {
                    kind: LaravelStringKind::Config,
                    key,
                    is_write: true,
                    ..
                } => {
                    if let Some((kind, name)) =
                        crate::symbol_map::laravel_resources::resource_from_config_key(key)
                    {
                        config_resources.push(ConfigResourceSpan {
                            kind,
                            name: name.to_string(),
                            start: span.start,
                            end: span.end,
                        });
                    }
                }
                _ => {}
            }
        }
        rate_limiters.sort_unstable_by(|left, right| {
            (&left.name, left.start, left.end).cmp(&(&right.name, right.start, right.end))
        });
        rate_limiters.dedup();
        config_resources.sort_unstable_by(|left, right| {
            (resource_order(left.kind), &left.name, left.start, left.end).cmp(&(
                resource_order(right.kind),
                &right.name,
                right.start,
                right.end,
            ))
        });
        config_resources.dedup();

        Self {
            rate_limiters,
            config_resources,
            dynamic_rate_limiter: map.has_dynamic_rate_limiter,
            has_queue_candidates: map
                .resource_receiver_sites
                .iter()
                .any(|site| site.rule == crate::symbol_map::LaravelResourceReceiverRule::QueueName),
        }
    }

    fn is_empty(&self) -> bool {
        self.rate_limiters.is_empty()
            && self.config_resources.is_empty()
            && !self.dynamic_rate_limiter
            && !self.has_queue_candidates
    }
}

#[derive(Default)]
struct UriNames {
    rate_limiters: Vec<NamedSpan>,
    config_resources: Vec<ConfigResourceSpan>,
    queue_names: Vec<NamedSpan>,
    dynamic_rate_limiter: bool,
    has_queue_candidates: bool,
}

#[derive(Default)]
struct ProviderUriNames {
    rate_limiters: Vec<NamedSpan>,
    dynamic_rate_limiter: bool,
}

/// Small incrementally-maintained index for source-defined names that have no
/// config/file declaration to enumerate.
#[derive(Default)]
pub(crate) struct LaravelSourceStringIndex {
    by_uri: HashMap<String, UriNames>,
    provider_by_uri: HashMap<String, ProviderUriNames>,
    rate_limiters: BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
    queue_names: BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
    config_resources:
        HashMap<LaravelConfigResource, BTreeMap<String, Vec<LaravelSourceStringDefinition>>>,
    dynamic_rate_limiter_files: u32,
    generation: u64,
    queue_names_generation: Option<u64>,
}

impl LaravelSourceStringIndex {
    /// Approximate owned heap bytes for the optional memory-audit build.
    #[cfg(feature = "mem-audit")]
    pub(crate) fn audit_heap(&self) -> usize {
        let mut bytes = self.by_uri.capacity() * std::mem::size_of::<(String, UriNames)>();
        for (uri, names) in &self.by_uri {
            bytes += uri.capacity();
            bytes += names.rate_limiters.capacity() * std::mem::size_of::<NamedSpan>();
            bytes += names.config_resources.capacity() * std::mem::size_of::<ConfigResourceSpan>();
            bytes += names.queue_names.capacity() * std::mem::size_of::<NamedSpan>();
            bytes += names
                .rate_limiters
                .iter()
                .chain(&names.queue_names)
                .map(|span| span.name.capacity())
                .sum::<usize>();
            bytes += names
                .config_resources
                .iter()
                .map(|span| span.name.capacity())
                .sum::<usize>();
        }
        bytes +=
            self.provider_by_uri.capacity() * std::mem::size_of::<(String, ProviderUriNames)>();
        for (uri, names) in &self.provider_by_uri {
            bytes += uri.capacity();
            bytes += names.rate_limiters.capacity() * std::mem::size_of::<NamedSpan>();
            bytes += names
                .rate_limiters
                .iter()
                .map(|span| span.name.capacity())
                .sum::<usize>();
        }
        for map in [&self.rate_limiters, &self.queue_names] {
            bytes += definition_map_heap(map);
        }
        bytes += self.config_resources.capacity()
            * std::mem::size_of::<(
                LaravelConfigResource,
                BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
            )>();
        for map in self.config_resources.values() {
            bytes += definition_map_heap(map);
        }
        bytes
    }

    /// Replace the direct symbol-map contributions of one file. Confirmed
    /// type-dependent names are cleared until that file's lazy type pass runs.
    pub(crate) fn set_symbol_map_contributions(
        &mut self,
        uri: &str,
        contributions: LaravelSourceStringContributions,
    ) {
        if self.by_uri.get(uri).is_some_and(|current| {
            !current.has_queue_candidates
                && current.queue_names.is_empty()
                && !contributions.has_queue_candidates
                && current.rate_limiters == contributions.rate_limiters
                && current.config_resources == contributions.config_resources
                && current.dynamic_rate_limiter == contributions.dynamic_rate_limiter
        }) {
            return;
        }
        let invalidates_queue_names = contributions.has_queue_candidates
            || self
                .by_uri
                .get(uri)
                .is_some_and(|names| names.has_queue_candidates || !names.queue_names.is_empty());
        self.remove_direct_contributions(uri);
        if invalidates_queue_names {
            self.generation = self.generation.wrapping_add(1);
            self.queue_names_generation = None;
        }
        add_named_spans(&mut self.rate_limiters, uri, &contributions.rate_limiters);
        add_config_resource_spans(
            &mut self.config_resources,
            uri,
            &contributions.config_resources,
        );
        if contributions.dynamic_rate_limiter {
            self.dynamic_rate_limiter_files += 1;
        }
        if !contributions.is_empty() {
            self.by_uri.insert(
                uri.to_string(),
                UriNames {
                    rate_limiters: contributions.rate_limiters,
                    config_resources: contributions.config_resources,
                    queue_names: Vec::new(),
                    dynamic_rate_limiter: contributions.dynamic_rate_limiter,
                    has_queue_candidates: contributions.has_queue_candidates,
                },
            );
        }
    }

    /// Atomically replace the registered service-provider layer.
    ///
    /// Provider files can live outside the ordinary workspace map set, so
    /// their rate-limiter registrations have a separate lifecycle. Keeping
    /// that layer separate also means rebuilding the provider set cannot
    /// erase an application file's direct contribution at the same URI.
    pub(crate) fn replace_provider_contributions(
        &mut self,
        contributions: Vec<(String, LaravelSourceStringContributions)>,
    ) {
        for (uri, names) in self.provider_by_uri.drain() {
            remove_named_spans(&mut self.rate_limiters, &uri, &names.rate_limiters);
            if names.dynamic_rate_limiter {
                self.dynamic_rate_limiter_files = self.dynamic_rate_limiter_files.saturating_sub(1);
            }
        }

        self.provider_by_uri.reserve(contributions.len());
        for (uri, contribution) in contributions {
            if contribution.rate_limiters.is_empty() && !contribution.dynamic_rate_limiter {
                continue;
            }
            add_named_spans(&mut self.rate_limiters, &uri, &contribution.rate_limiters);
            if contribution.dynamic_rate_limiter {
                self.dynamic_rate_limiter_files += 1;
            }
            self.provider_by_uri.insert(
                uri,
                ProviderUriNames {
                    rate_limiters: contribution.rate_limiters,
                    dynamic_rate_limiter: contribution.dynamic_rate_limiter,
                },
            );
        }
    }

    /// Replace one registered provider's rate-limiter registrations after an
    /// editor update without rebuilding any unrelated provider contribution.
    pub(crate) fn set_provider_contributions(
        &mut self,
        uri: &str,
        contribution: LaravelSourceStringContributions,
    ) {
        self.remove_provider_contributions(uri);
        if contribution.rate_limiters.is_empty() && !contribution.dynamic_rate_limiter {
            return;
        }
        add_named_spans(&mut self.rate_limiters, uri, &contribution.rate_limiters);
        if contribution.dynamic_rate_limiter {
            self.dynamic_rate_limiter_files += 1;
        }
        self.provider_by_uri.insert(
            uri.to_string(),
            ProviderUriNames {
                rate_limiters: contribution.rate_limiters,
                dynamic_rate_limiter: contribution.dynamic_rate_limiter,
            },
        );
    }

    /// Replace one file's queue names after typed receiver confirmation.
    pub(crate) fn set_typed_spans(&mut self, uri: &str, spans: &[SymbolSpan]) {
        let mut names = spans
            .iter()
            .filter_map(|span| match &span.kind {
                SymbolKind::LaravelStringKey {
                    kind: LaravelStringKind::QueueName,
                    key,
                    ..
                } => Some(NamedSpan {
                    name: key.clone(),
                    start: span.start,
                    end: span.end,
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        names.sort_unstable_by(|left, right| {
            (&left.name, left.start, left.end).cmp(&(&right.name, right.start, right.end))
        });
        names.dedup();

        let Some(entry) = self.by_uri.get_mut(uri) else {
            if names.is_empty() {
                return;
            }
            add_named_spans(&mut self.queue_names, uri, &names);
            self.by_uri.insert(
                uri.to_string(),
                UriNames {
                    queue_names: names,
                    ..UriNames::default()
                },
            );
            return;
        };
        if entry.queue_names == names {
            return;
        }
        remove_named_spans(&mut self.queue_names, uri, &entry.queue_names);
        add_named_spans(&mut self.queue_names, uri, &names);
        entry.queue_names = names;
        if entry.rate_limiters.is_empty()
            && entry.config_resources.is_empty()
            && entry.queue_names.is_empty()
            && !entry.dynamic_rate_limiter
            && !entry.has_queue_candidates
        {
            self.by_uri.remove(uri);
        }
    }

    /// Remove every contribution of one file.
    pub(crate) fn remove(&mut self, uri: &str) {
        let invalidates_queue_names = self
            .by_uri
            .get(uri)
            .is_some_and(|names| names.has_queue_candidates || !names.queue_names.is_empty());
        self.remove_direct_contributions(uri);
        self.remove_provider_contributions(uri);
        if invalidates_queue_names {
            // A candidate with no materialized name can still have a typed
            // scan in flight, so removing it must retire that scan too.
            self.generation = self.generation.wrapping_add(1);
            self.queue_names_generation = None;
        }
    }

    fn remove_direct_contributions(&mut self, uri: &str) -> bool {
        let Some(old) = self.by_uri.remove(uri) else {
            return false;
        };
        remove_named_spans(&mut self.rate_limiters, uri, &old.rate_limiters);
        remove_named_spans(&mut self.queue_names, uri, &old.queue_names);
        for span in old.config_resources {
            let remove_family =
                self.config_resources
                    .get_mut(&span.kind)
                    .is_some_and(|definitions| {
                        remove_definition(definitions, &span.name, uri, span.start, span.end);
                        definitions.is_empty()
                    });
            if remove_family {
                self.config_resources.remove(&span.kind);
            }
        }
        if old.dynamic_rate_limiter {
            self.dynamic_rate_limiter_files = self.dynamic_rate_limiter_files.saturating_sub(1);
        }
        true
    }

    fn remove_provider_contributions(&mut self, uri: &str) -> bool {
        let Some(old) = self.provider_by_uri.remove(uri) else {
            return false;
        };
        remove_named_spans(&mut self.rate_limiters, uri, &old.rate_limiters);
        if old.dynamic_rate_limiter {
            self.dynamic_rate_limiter_files = self.dynamic_rate_limiter_files.saturating_sub(1);
        }
        true
    }

    pub(crate) fn rate_limiter_names(&self) -> Vec<String> {
        self.rate_limiters.keys().cloned().collect()
    }

    /// Whether at least one statically enumerable rate limiter is registered.
    pub(crate) fn has_rate_limiters(&self) -> bool {
        !self.rate_limiters.is_empty()
    }

    /// Whether `name` has at least one statically enumerable registration.
    pub(crate) fn has_rate_limiter(&self, name: &str) -> bool {
        self.rate_limiters.contains_key(name)
    }

    /// Every exact registration literal for `name`, in stable source order.
    pub(crate) fn rate_limiter_definitions(
        &self,
        name: &str,
    ) -> Vec<LaravelSourceStringDefinition> {
        cloned_unique_definitions(self.rate_limiters.get(name).map(Vec::as_slice))
    }

    pub(crate) fn queue_names(&self) -> Vec<String> {
        self.queue_names.keys().cloned().collect()
    }

    /// Every confirmed queue-name occurrence for `name`.
    pub(crate) fn queue_name_definitions(&self, name: &str) -> Vec<LaravelSourceStringDefinition> {
        cloned_unique_definitions(self.queue_names.get(name).map(Vec::as_slice))
    }

    /// Runtime-defined direct config-resource children for one family.
    pub(crate) fn runtime_config_resource_names(&self, kind: LaravelConfigResource) -> Vec<String> {
        self.config_resources
            .get(&kind)
            .map(|resources| resources.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether application code directly defines this resource at runtime.
    pub(crate) fn has_runtime_config_resource(
        &self,
        kind: LaravelConfigResource,
        name: &str,
    ) -> bool {
        self.config_resources
            .get(&kind)
            .is_some_and(|resources| resources.contains_key(name))
    }

    /// Exact `Config::set()` literals that declare one runtime resource.
    pub(crate) fn runtime_config_resource_definitions(
        &self,
        kind: LaravelConfigResource,
        name: &str,
    ) -> Vec<LaravelSourceStringDefinition> {
        cloned_unique_definitions(
            self.config_resources
                .get(&kind)
                .and_then(|resources| resources.get(name))
                .map(Vec::as_slice),
        )
    }

    pub(crate) fn rate_limiter_space_is_open(&self) -> bool {
        self.dynamic_rate_limiter_files != 0
    }

    pub(crate) fn queue_name_generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn queue_names_are_complete(&self) -> bool {
        self.queue_names_generation == Some(self.generation)
    }

    /// Mark the typed scan complete only if no symbol-map publication raced
    /// it. Returns whether the caller's snapshot is still current.
    pub(crate) fn mark_queue_names_complete(&mut self, generation: u64) -> bool {
        if self.generation != generation {
            // The exact-map typed cache prevents an old map from publishing
            // after its replacement, while publishing the new symbol map
            // removes that URI's old queue spans. Keep the still-current
            // files' entries: a retry can reuse their Ready typed results,
            // which intentionally do not republish into this index.
            self.queue_names_generation = None;
            return false;
        }
        self.queue_names_generation = Some(generation);
        true
    }
}

fn add_named_spans(
    definitions: &mut BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
    uri: &str,
    spans: &[NamedSpan],
) {
    if spans.is_empty() {
        return;
    }
    let uri: Arc<str> = Arc::from(uri);
    for span in spans {
        add_definition(definitions, &span.name, &uri, span.start, span.end);
    }
}

fn add_config_resource_spans(
    definitions: &mut HashMap<
        LaravelConfigResource,
        BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
    >,
    uri: &str,
    spans: &[ConfigResourceSpan],
) {
    if spans.is_empty() {
        return;
    }
    let uri: Arc<str> = Arc::from(uri);
    for span in spans {
        add_definition(
            definitions.entry(span.kind).or_default(),
            &span.name,
            &uri,
            span.start,
            span.end,
        );
    }
}

fn remove_named_spans(
    definitions: &mut BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
    uri: &str,
    spans: &[NamedSpan],
) {
    for span in spans {
        remove_definition(definitions, &span.name, uri, span.start, span.end);
    }
}

fn add_definition(
    definitions: &mut BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
    name: &str,
    uri: &Arc<str>,
    start: u32,
    end: u32,
) {
    definitions
        .entry(name.to_string())
        .or_default()
        .push(LaravelSourceStringDefinition {
            uri: Arc::clone(uri),
            start,
            end,
        });
}

fn remove_definition(
    definitions: &mut BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
    name: &str,
    uri: &str,
    start: u32,
    end: u32,
) {
    let remove_name = definitions.get_mut(name).is_some_and(|locations| {
        if let Some(index) = locations.iter().position(|location| {
            location.uri.as_ref() == uri && location.start == start && location.end == end
        }) {
            locations.swap_remove(index);
        }
        locations.is_empty()
    });
    if remove_name {
        definitions.remove(name);
    }
}

fn cloned_unique_definitions(
    definitions: Option<&[LaravelSourceStringDefinition]>,
) -> Vec<LaravelSourceStringDefinition> {
    let Some(definitions) = definitions else {
        return Vec::new();
    };
    let mut result = definitions.to_vec();
    result.sort_unstable();
    result.dedup();
    result
}

const fn resource_order(kind: LaravelConfigResource) -> u8 {
    match kind {
        LaravelConfigResource::AuthGuard => 0,
        LaravelConfigResource::CacheStore => 1,
        LaravelConfigResource::LogChannel => 2,
        LaravelConfigResource::StorageDisk => 3,
        LaravelConfigResource::DatabaseConnection => 4,
        LaravelConfigResource::QueueConnection => 5,
        LaravelConfigResource::Mailer => 6,
        LaravelConfigResource::BroadcastConnection => 7,
    }
}

#[cfg(feature = "mem-audit")]
fn definition_map_heap(
    definitions: &BTreeMap<String, Vec<LaravelSourceStringDefinition>>,
) -> usize {
    let mut bytes = definitions.len()
        * (std::mem::size_of::<(String, Vec<LaravelSourceStringDefinition>)>()
            + 3 * std::mem::size_of::<usize>());
    for (name, locations) in definitions {
        bytes += name.capacity();
        bytes += locations.capacity() * std::mem::size_of::<LaravelSourceStringDefinition>();
        bytes += locations
            .iter()
            .map(|location| location.uri.len())
            .sum::<usize>();
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol_map::{SymbolSpan, extract_symbol_map};

    fn map(source: &str) -> SymbolMap {
        crate::parser::with_parsed_program(source, "source_string_index", |program, source| {
            extract_symbol_map(program, source)
        })
    }

    fn set_map(index: &mut LaravelSourceStringIndex, uri: &str, source: &str) {
        let map = map(source);
        let contributions = LaravelSourceStringContributions::from_symbol_map(&map);
        index.set_symbol_map_contributions(uri, contributions);
    }

    #[test]
    fn replacing_and_removing_a_uri_keeps_name_counts_exact() {
        let mut index = LaravelSourceStringIndex::default();
        set_map(
            &mut index,
            "file:///one.php",
            "<?php RateLimiter::for('api', fn () => null);",
        );
        set_map(
            &mut index,
            "file:///one.php",
            "<?php RateLimiter::for('api', fn () => null);",
        );
        set_map(
            &mut index,
            "file:///two.php",
            "<?php RateLimiter::for('api', fn () => null); RateLimiter::for('mail', fn () => null);",
        );
        assert_eq!(index.rate_limiter_names(), ["api", "mail"]);
        assert!(index.has_rate_limiters());
        assert!(index.has_rate_limiter("api"));
        assert!(!index.has_rate_limiter("missing"));

        set_map(&mut index, "file:///two.php", "<?php");
        assert_eq!(index.rate_limiter_names(), ["api"]);
        index.remove("file:///one.php");
        assert!(index.rate_limiter_names().is_empty());
        assert!(!index.has_rate_limiters());
    }

    #[test]
    fn exact_rate_limiter_definitions_survive_duplicate_names_and_layers() {
        let mut index = LaravelSourceStringIndex::default();
        let uri = "file:///provider.php";
        let source =
            "<?php RateLimiter::for('api', fn () => null); RateLimiter::for('api', fn () => null);";
        let parsed = map(source);
        index.set_symbol_map_contributions(
            uri,
            LaravelSourceStringContributions::from_symbol_map(&parsed),
        );

        let definitions = index.rate_limiter_definitions("api");
        assert_eq!(definitions.len(), 2);
        assert!(
            definitions
                .iter()
                .all(|definition| definition.uri.as_ref() == uri)
        );
        assert!(definitions[0].start < definitions[1].start);

        index.replace_provider_contributions(vec![(
            uri.to_string(),
            LaravelSourceStringContributions::from_symbol_map(&parsed),
        )]);
        assert_eq!(index.rate_limiter_definitions("api"), definitions);

        index.replace_provider_contributions(Vec::new());
        assert_eq!(index.rate_limiter_definitions("api"), definitions);
    }

    #[test]
    fn provider_layer_replaces_names_and_dynamic_openness_atomically() {
        let mut index = LaravelSourceStringIndex::default();
        let literal = map("<?php RateLimiter::for('vendor-api', fn () => null);");
        let dynamic = map("<?php RateLimiter::for($name, fn () => null);");

        index.replace_provider_contributions(vec![(
            "file:///vendor/provider.php".to_string(),
            LaravelSourceStringContributions::from_symbol_map(&literal),
        )]);
        assert_eq!(index.rate_limiter_names(), ["vendor-api"]);
        assert_eq!(index.rate_limiter_definitions("vendor-api").len(), 1);
        assert!(!index.rate_limiter_space_is_open());

        index.replace_provider_contributions(vec![(
            "file:///vendor/provider.php".to_string(),
            LaravelSourceStringContributions::from_symbol_map(&dynamic),
        )]);
        assert!(index.rate_limiter_names().is_empty());
        assert!(index.rate_limiter_space_is_open());

        index.replace_provider_contributions(Vec::new());
        assert!(!index.rate_limiter_space_is_open());
    }

    #[test]
    fn typed_queue_names_replace_without_touching_rate_limiters() {
        let mut index = LaravelSourceStringIndex::default();
        set_map(
            &mut index,
            "file:///job.php",
            "<?php RateLimiter::for('api', fn () => null);",
        );
        let queue_span = |key: &str| SymbolSpan {
            start: 1,
            end: 1 + key.len() as u32,
            kind: SymbolKind::LaravelStringKey {
                kind: LaravelStringKind::QueueName,
                key: key.to_string(),
                is_write: false,
                is_optional: true,
            },
        };
        index.set_typed_spans("file:///job.php", &[queue_span("high")]);
        assert_eq!(index.queue_names(), ["high"]);
        assert_eq!(
            index.queue_name_definitions("high"),
            [LaravelSourceStringDefinition {
                uri: Arc::from("file:///job.php"),
                start: 1,
                end: 5,
            }]
        );
        index.set_typed_spans("file:///job.php", &[queue_span("high")]);
        assert_eq!(index.queue_names(), ["high"]);
        index.set_typed_spans("file:///job.php", &[queue_span("low")]);
        assert_eq!(index.queue_names(), ["low"]);
        assert_eq!(index.rate_limiter_names(), ["api"]);
    }

    #[test]
    fn one_provider_can_be_replaced_without_rebuilding_the_provider_layer() {
        let mut index = LaravelSourceStringIndex::default();
        let uri = "file:///vendor/provider.php";
        let contribution = LaravelSourceStringContributions::from_symbol_map(&map(
            "<?php RateLimiter::for('vendor-api', fn () => null); RateLimiter::for($name, fn () => null);",
        ));

        index.set_provider_contributions(uri, contribution);
        assert_eq!(index.rate_limiter_names(), ["vendor-api"]);
        assert!(index.rate_limiter_space_is_open());

        index.set_provider_contributions(uri, LaravelSourceStringContributions::default());
        assert!(index.rate_limiter_names().is_empty());
        assert!(!index.rate_limiter_space_is_open());
        assert!(index.provider_by_uri.is_empty());
    }

    #[test]
    fn direct_config_writes_define_only_direct_resource_children() {
        let mut index = LaravelSourceStringIndex::default();
        set_map(
            &mut index,
            "file:///bootstrap.php",
            "<?php Config::set('cache.stores.tenant', []); Config::set('cache.stores.tenant.driver', 'array'); Config::get('cache.stores.read-only'); Config::set('filesystems.disks.logs', []);",
        );

        assert_eq!(
            index.runtime_config_resource_names(LaravelConfigResource::CacheStore),
            ["tenant"]
        );
        assert_eq!(
            index.runtime_config_resource_names(LaravelConfigResource::StorageDisk),
            ["logs"]
        );
        let definition =
            index.runtime_config_resource_definitions(LaravelConfigResource::CacheStore, "tenant");
        assert_eq!(definition.len(), 1);
        assert_eq!(definition[0].uri.as_ref(), "file:///bootstrap.php");

        set_map(&mut index, "file:///bootstrap.php", "<?php");
        assert!(
            index
                .runtime_config_resource_names(LaravelConfigResource::CacheStore)
                .is_empty()
        );
        assert!(
            index
                .runtime_config_resource_names(LaravelConfigResource::StorageDisk)
                .is_empty()
        );
    }

    #[test]
    fn dynamic_registration_opens_and_closes_the_namespace() {
        let mut index = LaravelSourceStringIndex::default();
        set_map(
            &mut index,
            "file:///provider.php",
            "<?php RateLimiter::for($name, fn () => null);",
        );
        assert!(index.rate_limiter_space_is_open());
        set_map(&mut index, "file:///provider.php", "<?php");
        assert!(!index.rate_limiter_space_is_open());
    }

    #[test]
    fn a_typed_queue_scan_cannot_mark_a_newer_generation_complete() {
        let mut index = LaravelSourceStringIndex::default();
        set_map(
            &mut index,
            "file:///one.php",
            "<?php $job->onQueue('high');",
        );
        let generation = index.queue_name_generation();
        assert!(index.mark_queue_names_complete(generation));
        assert!(index.queue_names_are_complete());

        set_map(&mut index, "file:///two.php", "<?php $job->onQueue('low');");
        assert!(!index.queue_names_are_complete());
        assert!(!index.mark_queue_names_complete(generation));
        assert!(index.mark_queue_names_complete(index.queue_name_generation()));
    }

    #[test]
    fn generation_retry_preserves_names_from_still_current_files() {
        let mut index = LaravelSourceStringIndex::default();
        set_map(
            &mut index,
            "file:///job.php",
            "<?php $job->onQueue('stale');",
        );
        let stale_generation = index.queue_name_generation();

        set_map(
            &mut index,
            "file:///other.php",
            "<?php $job->onQueue('other');",
        );
        let stale_queue_span = SymbolSpan {
            start: 1,
            end: 5,
            kind: SymbolKind::LaravelStringKey {
                kind: LaravelStringKind::QueueName,
                key: "stale".to_string(),
                is_write: false,
                is_optional: true,
            },
        };
        index.set_typed_spans("file:///job.php", &[stale_queue_span]);
        assert_eq!(index.queue_names(), ["stale"]);

        assert!(!index.mark_queue_names_complete(stale_generation));
        assert_eq!(index.queue_names(), ["stale"]);
        assert!(index.by_uri.contains_key("file:///job.php"));
    }

    #[test]
    fn files_without_source_names_do_not_consume_per_uri_storage() {
        let mut index = LaravelSourceStringIndex::default();
        let original_generation = index.queue_name_generation();
        assert!(index.mark_queue_names_complete(original_generation));

        set_map(&mut index, "file:///plain.php", "<?php echo 'plain';");

        assert!(index.by_uri.is_empty());
        assert_eq!(index.queue_name_generation(), original_generation);
        assert!(index.queue_names_are_complete());
    }

    #[test]
    fn removing_an_unmaterialized_file_still_invalidates_typed_scans() {
        let mut index = LaravelSourceStringIndex::default();
        set_map(
            &mut index,
            "file:///plain.php",
            "<?php $job->onQueue('high');",
        );
        let generation = index.queue_name_generation();

        index.remove("file:///plain.php");

        assert_ne!(index.queue_name_generation(), generation);
        assert!(!index.mark_queue_names_complete(generation));
        assert!(index.by_uri.is_empty());
    }

    #[test]
    fn empty_typed_results_do_not_create_or_retain_empty_entries() {
        let mut index = LaravelSourceStringIndex::default();
        index.set_typed_spans("file:///job.php", &[]);
        assert!(index.by_uri.is_empty());

        let queue_span = SymbolSpan {
            start: 1,
            end: 5,
            kind: SymbolKind::LaravelStringKey {
                kind: LaravelStringKind::QueueName,
                key: "high".to_string(),
                is_write: false,
                is_optional: true,
            },
        };
        index.set_typed_spans("file:///job.php", std::slice::from_ref(&queue_span));
        index.set_typed_spans("file:///other-job.php", &[queue_span]);
        assert_eq!(index.queue_names(), ["high"]);

        index.set_typed_spans("file:///job.php", &[]);
        assert!(!index.by_uri.contains_key("file:///job.php"));
        assert_eq!(index.queue_names(), ["high"]);

        index.set_typed_spans("file:///other-job.php", &[]);
        assert!(index.by_uri.is_empty());
        assert!(index.queue_names().is_empty());
    }

    #[test]
    fn every_config_resource_has_a_stable_sort_order() {
        assert_eq!(resource_order(LaravelConfigResource::AuthGuard), 0);
        assert_eq!(resource_order(LaravelConfigResource::CacheStore), 1);
        assert_eq!(resource_order(LaravelConfigResource::LogChannel), 2);
        assert_eq!(resource_order(LaravelConfigResource::StorageDisk), 3);
        assert_eq!(resource_order(LaravelConfigResource::DatabaseConnection), 4);
        assert_eq!(resource_order(LaravelConfigResource::QueueConnection), 5);
        assert_eq!(resource_order(LaravelConfigResource::Mailer), 6);
        assert_eq!(
            resource_order(LaravelConfigResource::BroadcastConnection),
            7
        );
    }
}
