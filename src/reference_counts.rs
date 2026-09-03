//! Cached member reference counts for the declaration inlay hints.
//!
//! A count shown next to a declaration has to mean "references to *this*
//! symbol", which is the search Find References runs: resolve the receiver
//! of every candidate access and keep the ones whose type is in the
//! declaring class' hierarchy.  That search is far too slow for the
//! inlay-hint request path (hundreds of milliseconds for one member on a
//! large project), so counts are computed on a background thread and
//! served from this cache.
//!
//! A hint is emitted only for a member whose count is already cached, and
//! a cached value keeps being served once it goes stale so the annotation
//! does not blink out between edits.  The reference index marks entries
//! stale rather than dropping them, and the next inlay-hint request queues
//! them for recomputation.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::{Mutex, RwLock};

use crate::Backend;
use crate::atom::{Atom, AtomMap};
use crate::class_lookup::find_class_at_offset;

/// Upper bound on cached member names.  Reached only by browsing tens of
/// thousands of declarations in one session, and the cache is then dropped
/// whole: keeping it in LRU order would cost more than recomputing.
const MAX_CACHED_MEMBERS: usize = 20_000;

/// A member declaration whose count still has to be computed.
#[derive(Clone, PartialEq, Eq, Hash)]
struct PendingCount {
    uri: Arc<str>,
    /// Offset of the declaration's name, used to find the enclosing class.
    offset: u32,
    class_fqn: Atom,
    member: Atom,
    is_static: bool,
}

#[derive(Clone, Copy)]
struct CachedCount {
    count: u32,
    /// Set when the reference index changed in a way that can affect this
    /// count.  The value is still served; it is only a recompute request.
    stale: bool,
}

/// Per-member-name counts, keyed by the class that declares the member.
///
/// The member name is the outer key because that is the granularity the
/// reference index can invalidate at: a file that gains or loses an access
/// to `save` can only change counts of members named `save`.  The two
/// slots are the instance and static member of that name.
type MemberCounts = AtomMap<[Option<CachedCount>; 2]>;

#[derive(Default)]
pub(crate) struct MemberRefCounts {
    counts: RwLock<AtomMap<MemberCounts>>,
    pending: Mutex<HashSet<PendingCount>>,
    /// Per-file digest of the inheritance each class declares, so a file
    /// that starts extending something can be told from one that only
    /// changed a method body.
    class_shapes: RwLock<HashMap<String, u64>>,
    /// Set while a background computation runs, so a burst of inlay-hint
    /// requests schedules one job rather than one each.
    computing: AtomicBool,
}

fn slot(is_static: bool) -> usize {
    usize::from(is_static)
}

impl MemberRefCounts {
    fn get(&self, class_fqn: Atom, member: Atom, is_static: bool) -> Option<CachedCount> {
        self.counts.read().get(&member)?.get(&class_fqn)?[slot(is_static)]
    }

    /// Store a freshly computed count, returning whether it differs from
    /// the one the editor was last given.
    fn store(&self, class_fqn: Atom, member: Atom, is_static: bool, count: u32) -> bool {
        let mut counts = self.counts.write();
        if counts.len() >= MAX_CACHED_MEMBERS {
            counts.clear();
        }
        let entry = &mut counts
            .entry(member)
            .or_default()
            .entry(class_fqn)
            .or_default()[slot(is_static)];
        let changed = entry.is_none_or(|cached| cached.count != count);
        *entry = Some(CachedCount {
            count,
            stale: false,
        });
        changed
    }

    /// Mark every count for members of this name as needing recomputation.
    pub(crate) fn invalidate_member(&self, member: Atom) {
        let mut counts = self.counts.write();
        let Some(entries) = counts.get_mut(&member) else {
            return;
        };
        for slots in entries.values_mut() {
            for cached in slots.iter_mut().flatten() {
                cached.stale = true;
            }
        }
    }

    /// Mark every cached count as needing recomputation.
    ///
    /// Used when a class' place in the inheritance graph changes, since
    /// that moves which accesses belong to which declaration.
    pub(crate) fn invalidate_all(&self) {
        let mut counts = self.counts.write();
        for entries in counts.values_mut() {
            for slots in entries.values_mut() {
                for cached in slots.iter_mut().flatten() {
                    cached.stale = true;
                }
            }
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        !self.pending.lock().is_empty()
    }

    /// Whether anything is cached at all.  Nothing is, until a declaration
    /// hint has been asked for, and the reference index skips its
    /// invalidation bookkeeping until then.
    pub(crate) fn is_empty(&self) -> bool {
        self.counts.read().is_empty()
    }
}

impl Backend {
    /// Whether the inheritance the file's classes declare differs from the
    /// last time it was indexed, recording the new shape either way.
    ///
    /// A file whose shape was never recorded counts as changed: the
    /// recording starts when the first count is cached, so the first edit
    /// to a file after that has nothing to compare against.
    pub(crate) fn class_shape_changed(&self, uri: &str) -> bool {
        let shape = self.class_shape(uri);
        let mut shapes = self.member_ref_counts.class_shapes.write();
        match (shapes.get(uri).copied(), shape) {
            (previous, Some(shape)) if previous != Some(shape) => {
                shapes.insert(uri.to_string(), shape);
                true
            }
            (Some(_), None) => {
                shapes.remove(uri);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn forget_class_shape(&self, uri: &str) {
        self.member_ref_counts.class_shapes.write().remove(uri);
    }

    /// A digest of every class the file declares and what it inherits
    /// from, or `None` when the file declares no class.
    fn class_shape(&self, uri: &str) -> Option<u64> {
        let classes = self.symbols.uri_classes_index.read().get(uri).cloned()?;
        if classes.is_empty() {
            return None;
        }
        let mut hasher = DefaultHasher::new();
        for class in classes.iter() {
            class.fqn().hash(&mut hasher);
            class.parent_class.hash(&mut hasher);
            class.interfaces.hash(&mut hasher);
            class.used_traits.hash(&mut hasher);
        }
        Some(hasher.finish())
    }

    /// The cached reference count for a member declaration, queuing a
    /// background computation when there is none or the cached one is
    /// stale.
    pub(crate) fn member_ref_count_cached(
        &self,
        uri: &str,
        offset: u32,
        class_fqn: Atom,
        member: Atom,
        is_static: bool,
    ) -> Option<u32> {
        let cached = self.member_ref_counts.get(class_fqn, member, is_static);
        if cached.is_none_or(|cached| cached.stale) {
            self.member_ref_counts.pending.lock().insert(PendingCount {
                uri: Arc::from(uri),
                offset,
                class_fqn,
                member,
                is_static,
            });
        }
        cached.map(|cached| cached.count)
    }

    /// Compute every queued member reference count.
    ///
    /// Returns `true` when at least one count changed, which is the signal
    /// to ask the editor to re-pull inlay hints.  Runs the search Find
    /// References runs, so the number matches what the user gets when they
    /// follow it.
    pub(crate) fn compute_pending_member_ref_counts(&self) -> bool {
        // Taken rather than drained: a request that arrives while this
        // runs sees the counts it wants still stale and queues them
        // again, and clearing them at the end keeps that from buying a
        // second pass over work this one has already done.
        let pending: Vec<PendingCount> = self
            .member_ref_counts
            .pending
            .lock()
            .iter()
            .cloned()
            .collect();
        if pending.is_empty() {
            return false;
        }

        let _chain_guard = crate::type_engine::resolver::with_chain_resolution_cache();
        let _resolver_guard = crate::type_engine::call_resolution::activate_type_engine_caches();

        let mut changed = false;
        for item in &pending {
            // The declaration may have moved or gone since the hint was
            // requested, and recomputing against a stale offset would scope
            // the search to the wrong class (or to none at all, which falls
            // back to counting every member of that name).
            if !self.declaration_still_at(item) {
                continue;
            }
            let count = self.member_declaration_reference_count(
                &item.uri,
                item.offset,
                &item.member,
                item.is_static,
            );
            changed |= self.member_ref_counts.store(
                item.class_fqn,
                item.member,
                item.is_static,
                count as u32,
            );
        }

        let mut queue = self.member_ref_counts.pending.lock();
        for item in &pending {
            queue.remove(item);
        }
        changed
    }

    fn declaration_still_at(&self, item: &PendingCount) -> bool {
        let classes = {
            let index = self.symbols.uri_classes_index.read();
            match index.get(item.uri.as_ref()) {
                Some(classes) => classes.clone(),
                None => return false,
            }
        };
        find_class_at_offset(&classes, item.offset)
            .is_some_and(|class| class.fqn() == item.class_fqn)
    }

    /// Run the queued member reference counts on a background thread and
    /// ask the editor to re-pull inlay hints once they land.
    ///
    /// At most one computation runs at a time: the counts a viewport needs
    /// are queued again by the next request, so a dropped schedule costs
    /// nothing but the wait.
    pub(crate) fn schedule_member_ref_counts(&self) {
        if !self.member_ref_counts.has_pending()
            || self
                .member_ref_counts
                .computing
                .swap(true, Ordering::AcqRel)
        {
            return;
        }

        let backend = self.clone_for_blocking();
        tokio::spawn(async move {
            let worker = backend.clone_for_blocking();
            let changed = crate::server::run_blocking_cancel_safe("member ref counts", move || {
                let changed = worker.compute_pending_member_ref_counts();
                worker
                    .member_ref_counts
                    .computing
                    .store(false, Ordering::Release);
                changed
            })
            .await;

            match changed {
                Some(true) => {
                    if backend.supports_inlay_hint_refresh.load(Ordering::Acquire)
                        && let Some(ref client) = backend.client
                    {
                        let _ = client.inlay_hint_refresh().await;
                    }
                }
                Some(false) => {}
                // A panicking task never cleared the flag; without this the
                // counts would never be computed again this session.
                None => backend
                    .member_ref_counts
                    .computing
                    .store(false, Ordering::Release),
            }
        });
    }
}

pub(crate) fn new_member_ref_counts() -> Arc<MemberRefCounts> {
    Arc::new(MemberRefCounts::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tower_lsp::lsp_types::{
        InlayHint, InlayHintLabel, InlayHintParams, Position, Range, TextDocumentIdentifier, Url,
    };

    const URI: &str = "file:///test.php";

    fn parse_extra(backend: &Backend, uri: &str, content: &str) {
        backend
            .open_files
            .write()
            .insert(uri.to_string(), Arc::new(content.to_string()));
        backend.update_ast(uri, content);
        backend
            .workspace_indexed
            .store(true, std::sync::atomic::Ordering::Release);
    }

    fn parse(backend: &Backend, content: &str) {
        parse_extra(backend, URI, content);
    }

    fn hints_for(backend: &Backend, uri: &str, content: &str) -> Vec<InlayHint> {
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: content.lines().count() as u32,
                character: 0,
            },
        };
        backend
            .handle_inlay_hints(uri, content, range)
            .unwrap_or_default()
    }

    fn hints(backend: &Backend, content: &str) -> Vec<InlayHint> {
        hints_for(backend, URI, content)
    }

    fn count_on_line(hints: &[InlayHint], line: u32) -> Option<String> {
        hints
            .iter()
            .find(|hint| hint.position.line == line)
            .map(|hint| match &hint.label {
                InlayHintLabel::String(label) => label.clone(),
                InlayHintLabel::LabelParts(parts) => {
                    parts.iter().map(|part| part.value.as_str()).collect()
                }
            })
    }

    const ONE_CALL: &str = r#"<?php
class Order {
    public function save(): void {}
}
function persist(Order $order): void {
    $order->save();
}
"#;

    #[test]
    fn an_edit_that_adds_an_access_recomputes_the_count() {
        let backend = Backend::new_test();
        parse(&backend, ONE_CALL);
        hints(&backend, ONE_CALL);
        backend.compute_pending_member_ref_counts();
        assert_eq!(
            count_on_line(&hints(&backend, ONE_CALL), 2).as_deref(),
            Some(" 1 reference")
        );

        let edited = ONE_CALL.replace("$order->save();", "$order->save();\n    $order->save();");
        parse(&backend, &edited);

        // The count the editor already has keeps being served until the
        // new one is ready, so the annotation does not blink out.
        assert_eq!(
            count_on_line(&hints(&backend, &edited), 2).as_deref(),
            Some(" 1 reference")
        );
        assert!(backend.compute_pending_member_ref_counts());
        assert_eq!(
            count_on_line(&hints(&backend, &edited), 2).as_deref(),
            Some(" 2 references")
        );
    }

    #[test]
    fn an_edit_that_leaves_the_accesses_alone_recomputes_nothing() {
        let backend = Backend::new_test();
        parse(&backend, ONE_CALL);
        hints(&backend, ONE_CALL);
        backend.compute_pending_member_ref_counts();

        // The first edit is the one that records what the file's classes
        // inherit, so start measuring from the second.
        let edited = format!("{ONE_CALL}// a trailing comment\n");
        parse(&backend, &edited);
        hints(&backend, &edited);
        backend.compute_pending_member_ref_counts();

        let edited_again = format!("{edited}// another trailing comment\n");
        parse(&backend, &edited_again);
        assert_eq!(
            count_on_line(&hints(&backend, &edited_again), 2).as_deref(),
            Some(" 1 reference")
        );
        assert!(
            !backend.member_ref_counts.has_pending(),
            "an edit that touches no access should not queue a recomputation"
        );
    }

    #[tokio::test]
    async fn the_request_path_counts_off_the_request() {
        let backend = Backend::new_test();
        parse(&backend, ONE_CALL);

        let params = InlayHintParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse(URI).unwrap(),
            },
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: ONE_CALL.lines().count() as u32,
                    character: 0,
                },
            },
            work_done_progress_params: Default::default(),
        };

        let first = backend
            .inlay_hint_request(params.clone())
            .await
            .unwrap()
            .unwrap_or_default();
        assert_eq!(
            count_on_line(&first, 2),
            None,
            "the first request answers before the count is known"
        );

        for _ in 0..200 {
            if !backend.member_ref_counts.is_empty() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        let second = backend
            .inlay_hint_request(params)
            .await
            .unwrap()
            .unwrap_or_default();
        assert_eq!(count_on_line(&second, 2).as_deref(), Some(" 1 reference"));
    }

    #[test]
    fn a_new_parent_class_recomputes_the_count() {
        let backend = Backend::new_test();
        let unrelated = r#"<?php
class Model {
    public function save(): void {}
}
class Order {
    public function save(): void {}
}
function persist(Order $order): void {
    $order->save();
}
"#;
        parse(&backend, unrelated);
        hints(&backend, unrelated);
        backend.compute_pending_member_ref_counts();
        assert_eq!(
            count_on_line(&hints(&backend, unrelated), 2).as_deref(),
            Some(" 0 references")
        );

        let inherited = unrelated.replace(
            "class Order {\n    public function save(): void {}",
            "class Order extends Model {",
        );
        parse(&backend, &inherited);
        hints(&backend, &inherited);
        assert!(backend.compute_pending_member_ref_counts());
        assert_eq!(
            count_on_line(&hints(&backend, &inherited), 2).as_deref(),
            Some(" 1 reference")
        );
    }

    #[test]
    fn chain_cache_does_not_leak_a_resolution_across_files() {
        let backend = Backend::new_test();

        const URI_A: &str = "file:///PenA.php";
        const URI_B: &str = "file:///PenB.php";
        const URI_CONSUMER_A: &str = "file:///ConsumerA.php";
        const URI_CONSUMER_B: &str = "file:///ConsumerB.php";

        // Two unrelated classes that happen to share a bare name and an
        // identically-shaped `make()->write()` chain, each imported under
        // that bare name by its own consumer file.
        let pen_a = r#"<?php
namespace App\A;

class Pen {
    public static function make(): self {
        return new self();
    }

    public function write(): void {}
}
"#;
        let pen_b = pen_a.replace("App\\A", "App\\B");

        let consumer_a = r#"<?php
namespace App;

use App\A\Pen;

function useA(): void {
    Pen::make()->write();
}
"#;
        let consumer_b = consumer_a
            .replace("App\\A", "App\\B")
            .replace("useA", "useB");

        parse_extra(&backend, URI_A, pen_a);
        parse_extra(&backend, URI_B, &pen_b);
        parse_extra(&backend, URI_CONSUMER_A, consumer_a);
        parse_extra(&backend, URI_CONSUMER_B, &consumer_b);

        // Queue both `write()` declarations for a count, then resolve them
        // in the same `compute_pending_member_ref_counts` pass so both
        // consumer files are scanned under one chain-cache activation —
        // the scenario where a text-only cache key leaks a resolution from
        // one file's `use` scope into the other's.
        hints_for(&backend, URI_A, pen_a);
        hints_for(&backend, URI_B, &pen_b);
        backend.compute_pending_member_ref_counts();

        assert_eq!(
            count_on_line(&hints_for(&backend, URI_A, pen_a), 8).as_deref(),
            Some(" 1 reference"),
            "App\\A\\Pen::write must only count ConsumerA's call, not ConsumerB's \
             identically-spelled `Pen::make()->write()` against `App\\B\\Pen`"
        );
        assert_eq!(
            count_on_line(&hints_for(&backend, URI_B, &pen_b), 8).as_deref(),
            Some(" 1 reference"),
            "App\\B\\Pen::write must only count ConsumerB's call"
        );
    }
}
