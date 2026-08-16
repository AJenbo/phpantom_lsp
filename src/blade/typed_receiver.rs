//! Laravel string sites whose receiver only a *type* settles.
//!
//! The symbol map decides what is a view name syntactically, so the
//! render sites it records as [`crate::symbol_map::SymbolKind::LaravelStringKey`]
//! spans are the ones a receiver's spelling settles: `$this->view(…)`
//! inside a mailable, the `View` facade, and the factory reached through a
//! bare `view()` helper call. A constructor-injected `Factory $views`
//! behind `$this->views->make('page')`, or a mailable held in a local
//! (`$mail = new OrderShipped(); $mail->view('emails.shipped')`), names a
//! template just as surely, but nothing about how it is written says so.
//!
//! [`crate::symbol_map::extract_symbol_map`] cannot tell: it runs during
//! `update_ast`, before the file's own classes are resolved, and a forward
//! walk per method call would be paid on every keystroke. It records the
//! candidate sites instead (see [`ViewReceiverSite`] and
//! [`LaravelResourceReceiverSite`]), and this module
//! answers them lazily — once per file, cached until the file is re-parsed
//! — by asking the shared type engine what the receiver is.
//!
//! The answer is a set of extra string-key spans, indistinguishable from the
//! ones the symbol map records, that every consumer reads alongside the map.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use mago_span::HasSpan;
use mago_syntax::cst::literal::Literal;
use mago_syntax::cst::*;

use crate::Backend;
use crate::parser::with_parsed_program;
use crate::symbol_map::{
    LaravelResourceReceiverRule, LaravelResourceReceiverSite, SymbolMap, SymbolSpan,
    ViewReceiverSite,
};
use crate::type_engine::resolver::{Loaders, VarResolutionCtx};
use crate::types::ClassInfo;

/// One file's typed-receiver cache entry.
///
/// A weak reference identifies the exact symbol-map allocation without
/// keeping a replaced map alive. Its allocation also prevents an address
/// from being reused while the cache still refers to it.
pub(crate) struct TypedReceiverSpans {
    /// The exact map this entry belongs to.
    pub(crate) symbol_map: Weak<SymbolMap>,
    /// Whether resolution is running, complete, or was invalidated by an edit.
    pub(crate) state: TypedReceiverSpansState,
}

/// Publication state for one typed-receiver cache entry.
pub(crate) enum TypedReceiverSpansState {
    /// Resolution is in progress. The token lets concurrent readers share the
    /// computation generation and detects a cache clear while they resolve.
    Pending(Arc<()>),
    /// Confirmed spans for the entry's exact symbol map.
    Ready(Arc<Vec<SymbolSpan>>),
    /// The map was evicted but may still be visible until its replacement is
    /// committed. Readers must not recompute or publish against it.
    Invalidated,
}

impl TypedReceiverSpans {
    fn belongs_to(&self, map: &Arc<SymbolMap>) -> bool {
        std::ptr::eq(self.symbol_map.as_ptr(), Arc::as_ptr(map))
    }
}

enum TypedReceiverCacheLookup {
    Ready(Arc<Vec<SymbolSpan>>),
    Pending(Arc<()>),
    Invalidated,
    Missing,
}

/// Read one entry through its exact map identity. Keeping this state-machine
/// projection in one place makes the read-fast and write-after-miss paths
/// obey identical publication rules.
fn lookup_cached_spans(
    entry: Option<&TypedReceiverSpans>,
    map: &Arc<SymbolMap>,
) -> TypedReceiverCacheLookup {
    let Some(entry) = entry.filter(|entry| entry.belongs_to(map)) else {
        return TypedReceiverCacheLookup::Missing;
    };
    match &entry.state {
        TypedReceiverSpansState::Ready(spans) => TypedReceiverCacheLookup::Ready(Arc::clone(spans)),
        TypedReceiverSpansState::Pending(token) => {
            TypedReceiverCacheLookup::Pending(Arc::clone(token))
        }
        TypedReceiverSpansState::Invalidated => TypedReceiverCacheLookup::Invalidated,
    }
}

/// Claim the pending generation while the cache's write lock is held. A
/// concurrent reader may have filled the entry since the read-fast lookup;
/// in that case its state wins unchanged.
fn claim_pending_generation(
    cache: &mut HashMap<String, TypedReceiverSpans>,
    uri: &str,
    map: &Arc<SymbolMap>,
) -> TypedReceiverCacheLookup {
    match lookup_cached_spans(cache.get(uri), map) {
        TypedReceiverCacheLookup::Missing => {
            let token = Arc::new(());
            let pending = TypedReceiverSpans {
                symbol_map: Arc::downgrade(map),
                state: TypedReceiverSpansState::Pending(Arc::clone(&token)),
            };
            if let Some(entry) = cache.get_mut(uri) {
                *entry = pending;
            } else {
                cache.insert(uri.to_string(), pending);
            }
            TypedReceiverCacheLookup::Pending(token)
        }
        cached => cached,
    }
}

impl Backend {
    /// The Laravel string spans of `uri` whose call/property is recognised by
    /// resolved type rather than spelling alone.
    ///
    /// Empty — without any work at all — for a file whose symbol map
    /// recorded no candidate sites, which is every file that does not call
    /// a view factory or a mailable through a typed receiver.
    ///
    /// Takes the file's symbol map rather than looking it up, because every
    /// caller is already reading the map's own view spans and merging these
    /// into them.
    pub(crate) fn typed_receiver_view_spans_for(
        &self,
        uri: &str,
        map: &SymbolMap,
    ) -> Arc<Vec<SymbolSpan>> {
        if map.view_receiver_sites.is_empty() && map.resource_receiver_sites.is_empty() {
            return empty_spans();
        }
        let pending_token = {
            // Pair the cache lookup/insertion with a live map lookup. A map
            // replacement cannot become visible between those two checks.
            let maps = self.symbol_maps.read();
            let Some(current_map) = maps.get(uri) else {
                return empty_spans();
            };
            if !std::ptr::eq(current_map.as_ref(), map) {
                return empty_spans();
            }

            let mut cached = {
                let cache = self.typed_receiver_view_spans_cache.read();
                lookup_cached_spans(cache.get(uri), current_map)
            };
            loop {
                match cached {
                    TypedReceiverCacheLookup::Ready(spans) => return spans,
                    TypedReceiverCacheLookup::Invalidated => return empty_spans(),
                    TypedReceiverCacheLookup::Pending(token) => break token,
                    TypedReceiverCacheLookup::Missing => {
                        let mut cache = self.typed_receiver_view_spans_cache.write();
                        cached = claim_pending_generation(&mut cache, uri, current_map);
                    }
                }
            }
        };

        let Some(content) = self.effective_content(uri) else {
            return empty_spans();
        };
        // The recorded offsets index the text the map was built from; a
        // buffer that has grown or shrunk since is a different text.
        if !map.matches_source(&content) {
            return empty_spans();
        }

        let has_queue_name_candidates = map
            .resource_receiver_sites
            .iter()
            .any(|site| site.rule == LaravelResourceReceiverRule::QueueName);
        let spans = Arc::new(self.confirm_receiver_sites(
            uri,
            &content,
            &map.view_receiver_sites,
            &map.resource_receiver_sites,
        ));

        // Hold the map read lock through both publications. An edit may evict
        // the pending entry while resolution runs; only the same pending
        // generation against the same still-current map may publish.
        let maps = self.symbol_maps.read();
        let Some(current_map) = maps.get(uri) else {
            return empty_spans();
        };
        if !std::ptr::eq(current_map.as_ref(), map) {
            return empty_spans();
        }
        let mut cache = self.typed_receiver_view_spans_cache.write();
        match cache.get(uri) {
            Some(entry) if entry.belongs_to(current_map) => match &entry.state {
                TypedReceiverSpansState::Pending(token) if Arc::ptr_eq(token, &pending_token) => {}
                TypedReceiverSpansState::Ready(existing) => return Arc::clone(existing),
                TypedReceiverSpansState::Pending(_) | TypedReceiverSpansState::Invalidated => {
                    return empty_spans();
                }
            },
            _ => return empty_spans(),
        }

        if has_queue_name_candidates {
            self.laravel_source_strings
                .write()
                .set_typed_spans(uri, &spans);
        }
        if let Some(entry) = cache.get_mut(uri) {
            *entry = TypedReceiverSpans {
                symbol_map: Arc::downgrade(current_map),
                state: TypedReceiverSpansState::Ready(Arc::clone(&spans)),
            };
        }
        spans
    }

    /// Invalidate one file's confirmed spans. Readers holding its old map get
    /// no answer; the first reader of the replacement map recomputes them.
    pub(crate) fn evict_typed_receiver_view_spans(&self, uri: &str) {
        let current_map = {
            let maps = self.symbol_maps.read();
            maps.get(uri)
                .filter(|map| {
                    !map.view_receiver_sites.is_empty() || !map.resource_receiver_sites.is_empty()
                })
                .cloned()
        };
        let Some(current_map) = current_map else {
            // This is the path for almost every parsed file. Avoid taking the
            // global cache's exclusive lock when this URI never had an entry.
            if !self
                .typed_receiver_view_spans_cache
                .read()
                .contains_key(uri)
            {
                return;
            }
            self.typed_receiver_view_spans_cache.write().remove(uri);
            return;
        };

        let mut cache = self.typed_receiver_view_spans_cache.write();
        let invalidated = TypedReceiverSpans {
            symbol_map: Arc::downgrade(&current_map),
            state: TypedReceiverSpansState::Invalidated,
        };
        if let Some(entry) = cache.get_mut(uri) {
            *entry = invalidated;
        } else {
            cache.insert(uri.to_string(), invalidated);
        }
    }

    /// The source every offset in a file's symbol map indexes: a Blade
    /// template's virtual PHP, and any other file's own text.
    fn effective_content(&self, uri: &str) -> Option<String> {
        if let Some(virtual_php) = self.blade_virtual_content.read().get(uri) {
            return Some(virtual_php.clone());
        }
        self.get_file_content(uri)
    }

    /// Resolve every candidate receiver/enclosing class and keep the spans
    /// whose Laravel meaning the type confirms.
    fn confirm_receiver_sites(
        &self,
        uri: &str,
        content: &str,
        view_sites: &[ViewReceiverSite],
        resource_sites: &[LaravelResourceReceiverSite],
    ) -> Vec<SymbolSpan> {
        let call_resource_count = resource_sites
            .iter()
            .filter(|site| site.rule != LaravelResourceReceiverRule::ConnectionProperty)
            .count();
        let mut sites_by_offset = HashMap::<u32, ReceiverCandidates<'_>>::with_capacity(
            view_sites.len() + call_resource_count,
        );
        for site in view_sites {
            sites_by_offset.entry(site.start).or_default().view = Some(site);
        }
        for site in resource_sites
            .iter()
            .filter(|site| site.rule != LaravelResourceReceiverRule::ConnectionProperty)
        {
            sites_by_offset.entry(site.start).or_default().resource = Some(site);
        }

        let file_ctx = self.file_context(uri);
        let class_loader = self.class_loader(&file_ctx);
        let mut confirmed = Vec::new();
        for site in resource_sites
            .iter()
            .filter(|site| site.rule == LaravelResourceReceiverRule::ConnectionProperty)
        {
            let Some(class) =
                crate::class_lookup::find_class_at_offset(&file_ctx.classes, site.start)
            else {
                continue;
            };
            if let Some(kind) = crate::symbol_map::laravel_resources::classify_connection_property(
                class,
                &class_loader,
            ) {
                confirmed.push(site.to_span(kind));
            }
        }
        if sites_by_offset.is_empty() {
            confirmed.sort_by_key(|span| span.start);
            return confirmed;
        }

        let function_loader = self.function_loader(&file_ctx);
        let function_loader_cl = |name: &str, offset: u32| function_loader(name, offset);

        with_parsed_program(content, "blade_typed_receiver", |program, content| {
            let mut calls: Vec<ReceiverCall<'_, '_, '_>> = Vec::new();
            let walker = ReceiverCallWalker {
                sites: &sites_by_offset,
            };
            let mut ctx = CollectCtx { calls: &mut calls };
            for stmt in program.statements.iter() {
                mago_syntax::walker::Walker::walk_statement(&walker, stmt, &mut ctx);
            }

            let default_class = ClassInfo::default();
            for call in calls {
                let offset = call.receiver.span().start.offset;
                let current_class =
                    crate::class_lookup::find_class_at_offset(&file_ctx.classes, offset)
                        .unwrap_or(&default_class);
                let var_ctx = VarResolutionCtx {
                    var_name: "",
                    top_level_scope: None,
                    current_class,
                    all_classes: &file_ctx.classes,
                    content,
                    // The receiver is resolved where the call is written, so
                    // a local reassigned earlier reads as it does there.
                    cursor_offset: offset,
                    class_loader: &class_loader,
                    backend: Some(self),
                    loaders: Loaders::with_function(Some(&function_loader_cl)),
                    resolved_class_cache: Some(&self.resolved_class_cache),
                    enclosing_return_type: None,
                    branch_aware: false,
                    match_arm_narrowing: HashMap::new(),
                    scope_var_resolver: None,
                };
                let Some(ty) =
                    crate::type_engine::variable::foreach_resolution::resolve_expression_type(
                        call.receiver,
                        &var_ctx,
                    )
                else {
                    continue;
                };
                for site in call.sites {
                    match site {
                        ReceiverSite::View(site) => {
                            if crate::class_lookup::is_subtype_of_named(
                                &ty,
                                site.receiver.fqn(),
                                &class_loader,
                            ) {
                                confirmed.push(site.to_span());
                            }
                        }
                        ReceiverSite::Resource(site) => {
                            if let Some(kind) =
                                crate::symbol_map::laravel_resources::classify_receiver_type(
                                    site.rule,
                                    &ty,
                                    &class_loader,
                                )
                            {
                                confirmed.push(site.to_span(kind));
                            }
                        }
                    }
                }
            }

            confirmed.sort_by_key(|span| span.start);
            confirmed
        })
    }
}

/// The shared empty answer, so the overwhelmingly common "this file has no
/// candidate sites" case costs a refcount bump rather than an allocation.
fn empty_spans() -> Arc<Vec<SymbolSpan>> {
    static EMPTY: std::sync::OnceLock<Arc<Vec<SymbolSpan>>> = std::sync::OnceLock::new();
    Arc::clone(EMPTY.get_or_init(|| Arc::new(Vec::new())))
}

/// One method call that owns at least one type-dependent Laravel string.
struct ReceiverCall<'ast, 'arena, 'sites> {
    receiver: &'ast Expression<'arena>,
    sites: Vec<ReceiverSite<'sites>>,
}

#[derive(Clone, Copy)]
enum ReceiverSite<'sites> {
    View(&'sites ViewReceiverSite),
    Resource(&'sites LaravelResourceReceiverSite),
}

#[derive(Clone, Copy, Default)]
struct ReceiverCandidates<'sites> {
    view: Option<&'sites ViewReceiverSite>,
    resource: Option<&'sites LaravelResourceReceiverSite>,
}

struct CollectCtx<'w, 'ast, 'arena, 'sites> {
    calls: &'w mut Vec<ReceiverCall<'ast, 'arena, 'sites>>,
}

/// Finds the method calls the candidate offsets belong to.
///
/// A candidate is keyed by the offset of its view-name string's contents,
/// which is unique in the file, so a call owns exactly the candidates its
/// own argument list spells.
struct ReceiverCallWalker<'a, 'sites> {
    sites: &'a HashMap<u32, ReceiverCandidates<'sites>>,
}

impl<'sites> ReceiverCallWalker<'_, 'sites> {
    /// The candidates one argument holds: the string itself, or the entries
    /// of the array a `first(['a', 'b'])` names.
    fn matching_sites(&self, expr: &Expression<'_>, matches: &mut Vec<ReceiverSite<'sites>>) {
        match expr {
            Expression::Literal(Literal::String(s)) => {
                let start = s.span.start.offset + 1;
                if let Some(candidates) = self.sites.get(&start) {
                    if let Some(site) = candidates.view {
                        matches.push(ReceiverSite::View(site));
                    }
                    if let Some(site) = candidates.resource {
                        matches.push(ReceiverSite::Resource(site));
                    }
                }
            }
            Expression::Array(array) => {
                for element in array.elements.iter() {
                    if let ArrayElement::Value(value) = element {
                        self.matching_sites(value.value, matches);
                    }
                }
            }
            Expression::LegacyArray(array) => {
                for element in array.elements.iter() {
                    if let ArrayElement::Value(value) = element {
                        self.matching_sites(value.value, matches);
                    }
                }
            }
            _ => {}
        }
    }
}

impl<'ast, 'arena, 'w, 'sites>
    mago_syntax::walker::Walker<'ast, 'arena, CollectCtx<'w, 'ast, 'arena, 'sites>>
    for ReceiverCallWalker<'_, 'sites>
{
    fn walk_in_method_call(
        &self,
        node: &'ast MethodCall<'arena>,
        ctx: &mut CollectCtx<'w, 'ast, 'arena, 'sites>,
    ) {
        let mut sites = Vec::new();
        for argument in node.argument_list.arguments.iter() {
            self.matching_sites(argument.value(), &mut sites);
        }
        if !sites.is_empty() {
            ctx.calls.push(ReceiverCall {
                receiver: node.object,
                sites,
            });
        }
    }

    fn walk_in_null_safe_method_call(
        &self,
        node: &'ast NullSafeMethodCall<'arena>,
        ctx: &mut CollectCtx<'w, 'ast, 'arena, 'sites>,
    ) {
        let mut sites = Vec::new();
        for argument in node.argument_list.arguments.iter() {
            self.matching_sites(argument.value(), &mut sites);
        }
        if !sites.is_empty() {
            ctx.calls.push(ReceiverCall {
                receiver: node.object,
                sites,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTENT: &str = "<?php $job->onQueue('critical');";

    fn queue_name_map() -> Arc<SymbolMap> {
        let start = CONTENT.find("critical").unwrap() as u32;
        Arc::new(SymbolMap {
            resource_receiver_sites: vec![LaravelResourceReceiverSite {
                start,
                end: start + "critical".len() as u32,
                key: "critical".to_string(),
                rule: LaravelResourceReceiverRule::QueueName,
            }],
            source_len: CONTENT.len() as u32,
            ..Default::default()
        })
    }

    #[test]
    fn cache_identity_is_the_exact_symbol_map_allocation() {
        let first = Arc::new(SymbolMap::default());
        let replacement = Arc::new(SymbolMap::default());
        let entry = TypedReceiverSpans {
            symbol_map: Arc::downgrade(&first),
            state: TypedReceiverSpansState::Pending(Arc::new(())),
        };

        assert!(entry.belongs_to(&first));
        assert!(!entry.belongs_to(&replacement));
        drop(first);
        assert!(entry.symbol_map.upgrade().is_none());
    }

    #[test]
    fn cache_lookup_preserves_every_publication_state() {
        let map = Arc::new(SymbolMap::default());
        let replacement = Arc::new(SymbolMap::default());
        let ready_spans = Arc::new(Vec::new());
        let ready = TypedReceiverSpans {
            symbol_map: Arc::downgrade(&map),
            state: TypedReceiverSpansState::Ready(Arc::clone(&ready_spans)),
        };
        assert!(matches!(
            lookup_cached_spans(Some(&ready), &map),
            TypedReceiverCacheLookup::Ready(found) if Arc::ptr_eq(&found, &ready_spans)
        ));

        let pending_token = Arc::new(());
        let pending = TypedReceiverSpans {
            symbol_map: Arc::downgrade(&map),
            state: TypedReceiverSpansState::Pending(Arc::clone(&pending_token)),
        };
        assert!(matches!(
            lookup_cached_spans(Some(&pending), &map),
            TypedReceiverCacheLookup::Pending(found) if Arc::ptr_eq(&found, &pending_token)
        ));

        let invalidated = TypedReceiverSpans {
            symbol_map: Arc::downgrade(&map),
            state: TypedReceiverSpansState::Invalidated,
        };
        assert!(matches!(
            lookup_cached_spans(Some(&invalidated), &map),
            TypedReceiverCacheLookup::Invalidated
        ));
        assert!(matches!(
            lookup_cached_spans(Some(&invalidated), &replacement),
            TypedReceiverCacheLookup::Missing
        ));
        assert!(matches!(
            lookup_cached_spans(None, &map),
            TypedReceiverCacheLookup::Missing
        ));
    }

    #[test]
    fn pending_claim_reuses_or_replaces_the_lock_protected_state() {
        let uri = "phpantom-test://typed-claim.php";
        let map = Arc::new(SymbolMap::default());
        let replacement = Arc::new(SymbolMap::default());
        let mut cache = HashMap::new();

        assert!(matches!(
            claim_pending_generation(&mut cache, uri, &map),
            TypedReceiverCacheLookup::Pending(_)
        ));
        assert!(matches!(
            claim_pending_generation(&mut cache, uri, &map),
            TypedReceiverCacheLookup::Pending(_)
        ));

        let ready_spans = Arc::new(Vec::new());
        cache.get_mut(uri).unwrap().state =
            TypedReceiverSpansState::Ready(Arc::clone(&ready_spans));
        assert!(matches!(
            claim_pending_generation(&mut cache, uri, &map),
            TypedReceiverCacheLookup::Ready(found) if Arc::ptr_eq(&found, &ready_spans)
        ));

        cache.get_mut(uri).unwrap().state = TypedReceiverSpansState::Invalidated;
        assert!(matches!(
            claim_pending_generation(&mut cache, uri, &map),
            TypedReceiverCacheLookup::Invalidated
        ));

        assert!(matches!(
            claim_pending_generation(&mut cache, uri, &replacement),
            TypedReceiverCacheLookup::Pending(_)
        ));
        assert!(cache.get(uri).unwrap().belongs_to(&replacement));
    }

    #[test]
    fn candidate_free_eviction_only_removes_an_existing_entry() {
        let backend = Backend::new_test();
        let uri = "phpantom-test://typed-no-candidates.php";
        let map = Arc::new(SymbolMap::default());
        backend
            .symbol_maps
            .write()
            .insert(uri.to_string(), Arc::clone(&map));

        backend.evict_typed_receiver_view_spans(uri);
        assert!(backend.typed_receiver_view_spans_cache.read().is_empty());

        backend.typed_receiver_view_spans_cache.write().insert(
            uri.to_string(),
            TypedReceiverSpans {
                symbol_map: Arc::downgrade(&map),
                state: TypedReceiverSpansState::Ready(Arc::new(Vec::new())),
            },
        );
        backend.evict_typed_receiver_view_spans(uri);
        assert!(backend.typed_receiver_view_spans_cache.read().is_empty());
    }

    #[test]
    fn typed_cache_rejects_missing_replaced_and_invalidated_maps() {
        let backend = Backend::new_test();
        let uri = "phpantom-test://typed-cache.php";
        let requested = queue_name_map();

        assert!(
            backend
                .typed_receiver_view_spans_for(uri, requested.as_ref())
                .is_empty()
        );

        let replacement = queue_name_map();
        backend
            .symbol_maps
            .write()
            .insert(uri.to_string(), Arc::clone(&replacement));
        assert!(
            backend
                .typed_receiver_view_spans_for(uri, requested.as_ref())
                .is_empty()
        );

        backend.evict_typed_receiver_view_spans(uri);
        assert!(matches!(
            backend
                .typed_receiver_view_spans_cache
                .read()
                .get(uri)
                .map(|entry| &entry.state),
            Some(TypedReceiverSpansState::Invalidated)
        ));
        assert!(
            backend
                .typed_receiver_view_spans_for(uri, replacement.as_ref())
                .is_empty()
        );
    }

    #[test]
    fn an_existing_pending_generation_is_resolved_and_published() {
        let backend = Backend::new_test();
        let uri = "phpantom-test://typed-pending.php";
        let map = queue_name_map();
        backend
            .open_files
            .write()
            .insert(uri.to_string(), Arc::new(CONTENT.to_string()));
        backend
            .symbol_maps
            .write()
            .insert(uri.to_string(), Arc::clone(&map));
        backend.typed_receiver_view_spans_cache.write().insert(
            uri.to_string(),
            TypedReceiverSpans {
                symbol_map: Arc::downgrade(&map),
                state: TypedReceiverSpansState::Pending(Arc::new(())),
            },
        );

        assert!(
            backend
                .typed_receiver_view_spans_for(uri, map.as_ref())
                .is_empty()
        );
        assert!(matches!(
            backend
                .typed_receiver_view_spans_cache
                .read()
                .get(uri)
                .map(|entry| &entry.state),
            Some(TypedReceiverSpansState::Ready(_))
        ));
    }

    #[test]
    fn property_sites_without_an_enclosing_class_and_legacy_arrays_are_ignored() {
        let backend = Backend::new_test();
        let property_site = LaravelResourceReceiverSite {
            start: 1,
            end: 6,
            key: "mysql".to_string(),
            rule: LaravelResourceReceiverRule::ConnectionProperty,
        };
        assert!(
            backend
                .confirm_receiver_sites(
                    "phpantom-test://orphan-property.php",
                    "",
                    &[],
                    &[property_site],
                )
                .is_empty()
        );

        let content = "<?php $unknown->first(array('legacy.view'));";
        let start = content.find("legacy.view").unwrap() as u32;
        let view_site = ViewReceiverSite {
            start,
            end: start + "legacy.view".len() as u32,
            key: "legacy.view".to_string(),
            is_optional: true,
            receiver: crate::symbol_map::ViewReceiverClass::Factory,
        };
        assert!(
            backend
                .confirm_receiver_sites(
                    "phpantom-test://legacy-view.php",
                    content,
                    &[view_site],
                    &[],
                )
                .is_empty()
        );
    }

    #[derive(Clone, Copy)]
    enum PublicationMutation {
        RemoveMap,
        ReplaceMap,
        PublishReady,
        ReplacePending,
        RemoveEntry,
    }

    fn resolve_while_mutating_publication(mutation: PublicationMutation) -> usize {
        let backend = Backend::new_test();
        let uri = "phpantom-test://typed-publication-race.php";
        let map = queue_name_map();
        backend
            .open_files
            .write()
            .insert(uri.to_string(), Arc::new(CONTENT.to_string()));
        backend
            .symbol_maps
            .write()
            .insert(uri.to_string(), Arc::clone(&map));

        // Resolution reads this index after installing its pending generation.
        // Holding the write lock gives the test a deterministic, production
        // synchronization point at which to model an edit or concurrent reader.
        let class_index = backend.symbols.uri_classes_index.write();
        let worker_backend = backend.clone_for_blocking();
        let worker_map = Arc::clone(&map);
        let worker_uri = uri.to_string();
        let worker = std::thread::spawn(move || {
            worker_backend.typed_receiver_view_spans_for(&worker_uri, worker_map.as_ref())
        });

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let is_pending = backend
                .typed_receiver_view_spans_cache
                .read()
                .get(uri)
                .is_some_and(|entry| matches!(&entry.state, TypedReceiverSpansState::Pending(_)));
            if is_pending {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "typed-receiver worker did not publish its pending generation"
            );
            std::thread::yield_now();
        }

        match mutation {
            PublicationMutation::RemoveMap => {
                backend.symbol_maps.write().remove(uri);
            }
            PublicationMutation::ReplaceMap => {
                backend
                    .symbol_maps
                    .write()
                    .insert(uri.to_string(), queue_name_map());
            }
            PublicationMutation::PublishReady => {
                let ready = Arc::new(vec![
                    map.resource_receiver_sites[0]
                        .to_span(crate::symbol_map::LaravelStringKind::QueueName),
                ]);
                backend
                    .typed_receiver_view_spans_cache
                    .write()
                    .get_mut(uri)
                    .unwrap()
                    .state = TypedReceiverSpansState::Ready(ready);
            }
            PublicationMutation::ReplacePending => {
                backend
                    .typed_receiver_view_spans_cache
                    .write()
                    .get_mut(uri)
                    .unwrap()
                    .state = TypedReceiverSpansState::Pending(Arc::new(()));
            }
            PublicationMutation::RemoveEntry => {
                backend.typed_receiver_view_spans_cache.write().remove(uri);
            }
        }

        drop(class_index);
        worker.join().unwrap().len()
    }

    #[test]
    fn stale_resolutions_cannot_publish_over_newer_maps_or_generations() {
        assert_eq!(
            resolve_while_mutating_publication(PublicationMutation::RemoveMap),
            0
        );
        assert_eq!(
            resolve_while_mutating_publication(PublicationMutation::ReplaceMap),
            0
        );
        assert_eq!(
            resolve_while_mutating_publication(PublicationMutation::PublishReady),
            1
        );
        assert_eq!(
            resolve_while_mutating_publication(PublicationMutation::ReplacePending),
            0
        );
        assert_eq!(
            resolve_while_mutating_publication(PublicationMutation::RemoveEntry),
            0
        );
    }
}
