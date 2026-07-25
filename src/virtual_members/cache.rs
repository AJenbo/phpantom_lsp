//! Resolved-class cache.
//!
//! The data structure behind [`ResolvedClassCache`]: the
//! `(FQN, generic args)`-keyed map of fully-resolved classes, the
//! auxiliary indices that keep eviction proportional to the dependent
//! set, the thread-local active-cache guard, and the transitive
//! eviction walk (`evict_fqn`).  The resolution logic that populates
//! the cache lives in [`super::resolve`].

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;

use crate::atom::Atom;
use crate::types::ClassInfo;
use crate::virtual_members::laravel::database_schema::SchemaIndex;

/// Cache key for [`ResolvedClassCache`]: fully-qualified class name
/// paired with the concrete generic type arguments used at this
/// instantiation site.
///
/// For non-generic classes the argument list is empty, keeping the
/// common case cheap.  For generic classes like
/// `Illuminate\Database\Eloquent\Builder<App\Models\User>`, the key
/// would be `("Illuminate\\Database\\Eloquent\\Builder", vec!["App\\Models\\User"])`.
///
/// Generic args are stored normalized (fully qualified, sorted when
/// order-independent) to avoid near-miss cache entries.
pub type ResolvedClassCacheKey = (Atom, Vec<String>);

/// The inner store behind a [`ResolvedClassCache`].
///
/// Holds the resolved-class map plus two auxiliary indices that make
/// eviction proportional to the number of dependent classes rather than
/// the size of the whole cache:
///
/// * `fqn_keys` maps a class FQN to every generic-arg variant stored
///   under it, so all variants of a class can be removed without scanning.
/// * `reverse_deps` maps a dependency string (a parent / trait / interface
///   / mixin / cast value, exactly as stored on the dependent class) to the
///   set of cache keys that reference it, so the transitive eviction
///   cascade is a graph walk rather than repeated full-cache scans.
///
/// All three structures are kept consistent by going through [`Self::insert`],
/// [`Self::clear`], and [`evict_fqn`]; callers never mutate the map directly.
pub struct ResolvedCacheInner {
    /// Resolved classes keyed by FQN + concrete generic args.
    map: HashMap<ResolvedClassCacheKey, Arc<ClassInfo>>,
    /// FQN → all cache keys (generic-arg variants) stored under it.
    fqn_keys: HashMap<String, HashSet<ResolvedClassCacheKey>>,
    /// Dependency string → cache keys whose class directly depends on it.
    ///
    /// The string is stored verbatim from the dependent class (it may be a
    /// fully-qualified name or a short name), mirroring the dual FQN /
    /// short-name matching that eviction performs.
    reverse_deps: HashMap<String, HashSet<ResolvedClassCacheKey>>,
    /// Whether the current project uses Laravel (or a standalone Illuminate
    /// component).  When `false`, `resolve_class_fully_inner` skips the
    /// Laravel virtual member providers, the contract to concrete-class
    /// mixin bindings, and the post-resolution Laravel patches, so a
    /// non-Laravel project pays none of that scanning cost.
    ///
    /// Defaults to `true`: Laravel analysis stays enabled unless the server
    /// has parsed `composer.json` and confirmed no Laravel/Illuminate
    /// dependency.  This keeps behaviour unchanged for the CLI test
    /// backends and any code path that resolves without a project context.
    /// Not touched by [`Self::clear`] — it reflects the project, not the
    /// per-edit cache contents.
    is_laravel: bool,
    schema_index: SchemaIndex,
    /// Classes currently being resolved by `resolve_class_fully_inner`,
    /// keyed per thread so one thread's in-progress resolution never
    /// degrades another thread's independent resolution of the same
    /// class.  When resolution of a class re-enters itself on the same
    /// thread (a genuine dependency cycle, e.g. two Eloquent models
    /// whose relationship providers resolve each other), the re-entrant
    /// call detects its own marker here and returns a base-inheritance
    /// partial result instead of recursing forever.  Entries live only
    /// for the duration of one synchronous resolution call tree; they
    /// are removed by an RAII guard, so a panic cannot leak them.
    in_flight: HashSet<(std::thread::ThreadId, Atom)>,
}

impl Default for ResolvedCacheInner {
    fn default() -> Self {
        ResolvedCacheInner {
            map: HashMap::new(),
            fqn_keys: HashMap::new(),
            reverse_deps: HashMap::new(),
            is_laravel: true,
            schema_index: SchemaIndex::default(),
            in_flight: HashSet::new(),
        }
    }
}

impl ResolvedCacheInner {
    /// Look up a resolved class by its `(FQN, generic args)` key.
    pub fn get(&self, key: &ResolvedClassCacheKey) -> Option<&Arc<ClassInfo>> {
        self.map.get(key)
    }

    /// Whether a key is present in the cache.
    pub fn contains_key(&self, key: &ResolvedClassCacheKey) -> bool {
        self.map.contains_key(key)
    }

    /// Whether the project uses Laravel / Illuminate.  See the
    /// [`is_laravel`](ResolvedCacheInner::is_laravel) field.
    pub fn is_laravel(&self) -> bool {
        self.is_laravel
    }

    /// Record whether the project uses Laravel / Illuminate.
    ///
    /// Set once by the server after parsing `composer.json`.  Left
    /// untouched by [`Self::clear`] so cache invalidation on file edits
    /// does not lose the project classification.
    pub fn set_laravel(&mut self, is_laravel: bool) {
        self.is_laravel = is_laravel;
    }

    /// Mark `fqn` as being resolved on the current thread.  Returns
    /// `true` when freshly marked (caller should proceed with full
    /// resolution) and `false` on re-entry (caller should break the
    /// cycle with a partial result).
    pub fn mark_in_flight(&mut self, fqn: Atom) -> bool {
        self.in_flight.insert((std::thread::current().id(), fqn))
    }

    /// Remove the current thread's in-flight marker for `fqn`.
    pub fn unmark_in_flight(&mut self, fqn: Atom) {
        self.in_flight.remove(&(std::thread::current().id(), fqn));
    }

    pub fn schema_index(&self) -> &SchemaIndex {
        &self.schema_index
    }

    pub fn set_schema_index(&mut self, schema_index: SchemaIndex) {
        self.schema_index = schema_index;
    }

    /// Number of cached entries (used by eviction tests).
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the cache is empty (used by eviction tests).
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Insert a resolved class, updating the FQN and reverse-dependency
    /// indices so eviction stays cheap.
    pub fn insert(&mut self, key: ResolvedClassCacheKey, value: Arc<ClassInfo>) {
        // When replacing an existing entry, drop its old reverse-dep edges
        // first so a changed dependency set does not leave stale edges.
        if let Some(old_deps) = self.map.get(&key).map(|c| dependency_keys(c)) {
            for dep in old_deps {
                self.unlink_reverse_edge(&dep, &key);
            }
        }

        let fqn = key.0.to_string();
        for dep in dependency_keys(&value) {
            self.reverse_deps
                .entry(dep)
                .or_default()
                .insert(key.clone());
        }
        self.fqn_keys.entry(fqn).or_default().insert(key.clone());
        self.map.insert(key, value);
    }

    /// Remove all entries and indices.
    ///
    /// `in_flight` is left untouched: its entries belong to resolution
    /// call trees currently running on other threads, not to the cached
    /// contents being invalidated.
    pub fn clear(&mut self) {
        self.map.clear();
        self.fqn_keys.clear();
        self.reverse_deps.clear();
    }

    /// Remove `key` from the reverse-dependency edge under `dep`, pruning
    /// the entry entirely when it becomes empty.
    fn unlink_reverse_edge(&mut self, dep: &str, key: &ResolvedClassCacheKey) {
        if let Some(set) = self.reverse_deps.get_mut(dep) {
            set.remove(key);
            if set.is_empty() {
                self.reverse_deps.remove(dep);
            }
        }
    }

    /// Remove every generic-arg variant of `fqn`, cleaning up both indices.
    ///
    /// Returns `true` when at least one entry was present and removed.
    fn remove_all_variants(&mut self, fqn: &str) -> bool {
        let keys = match self.fqn_keys.remove(fqn) {
            Some(set) => set,
            None => return false,
        };
        for key in keys {
            if let Some(value) = self.map.remove(&key) {
                for dep in dependency_keys(&value) {
                    self.unlink_reverse_edge(&dep, &key);
                }
            }
        }
        true
    }
}

/// Thread-safe cache of fully-resolved classes, keyed by FQN + generic args.
///
/// Stored on [`Backend`](crate::Backend) and selectively invalidated
/// when a file is re-parsed (`update_ast` / `parse_and_cache_content`).
/// Within a single request cycle (completion, hover, etc.) the cache
/// eliminates redundant calls to
/// [`resolve_class_fully`](super::resolve_class_fully) for the same
/// class at the same generic instantiation.
///
/// Uses an [`RwLock`] rather than a mutex because the cache is read-mostly
/// during resolution (lookups vastly outnumber inserts), so concurrent
/// requests under a sustained keystroke barrage can read in parallel
/// instead of serializing on a single lock.
pub type ResolvedClassCache = Arc<RwLock<ResolvedCacheInner>>;

/// Create a new, empty [`ResolvedClassCache`].
pub fn new_resolved_class_cache() -> ResolvedClassCache {
    Arc::new(RwLock::new(ResolvedCacheInner::default()))
}

// ─── Thread-local resolved-class cache access ───────────────────────────────
//
// Many code paths (e.g. `type_hint_to_classes_typed`) call `resolve_class_fully`
// without access to the `Backend`'s `resolved_class_cache`.  Rather than
// threading the cache through dozens of function signatures, we use the
// same thread-local guard pattern as the parse cache: the caller activates
// the cache at a high level (e.g. the diagnostic loop), and inner functions
// pick it up via `active_resolved_class_cache()`.

thread_local! {
    /// Raw pointer to the currently-active [`ResolvedClassCache`], or null.
    ///
    /// Set by [`with_active_resolved_class_cache`], read by
    /// [`active_resolved_class_cache`].  The pointer is valid for the
    /// lifetime of the [`ResolvedCacheGuard`] that set it.
    static ACTIVE_RESOLVED_CACHE: Cell<*const ResolvedClassCache> = const { Cell::new(std::ptr::null()) };
}

/// RAII guard that clears the thread-local cache pointer on drop.
pub struct ResolvedCacheGuard {
    prev: *const ResolvedClassCache,
}

impl Drop for ResolvedCacheGuard {
    fn drop(&mut self) {
        ACTIVE_RESOLVED_CACHE.with(|c| c.set(self.prev));
    }
}

/// Activate a [`ResolvedClassCache`] for the current thread.
///
/// While the returned guard is alive, [`active_resolved_class_cache`]
/// returns `Some(&cache)`.  Supports nesting (restores the previous
/// cache on drop).
///
/// # Example
///
/// ```ignore
/// let _guard = with_active_resolved_class_cache(&backend.resolved_class_cache);
/// // … deep call stacks can now use active_resolved_class_cache()
/// ```
pub fn with_active_resolved_class_cache(cache: &ResolvedClassCache) -> ResolvedCacheGuard {
    let prev = ACTIVE_RESOLVED_CACHE.with(|c| c.get());
    ACTIVE_RESOLVED_CACHE.with(|c| c.set(cache as *const _));
    ResolvedCacheGuard { prev }
}

/// Return the currently-active [`ResolvedClassCache`], if any.
///
/// Returns `None` when no [`with_active_resolved_class_cache`] guard
/// is alive on this thread.
///
/// # Safety
///
/// The returned reference borrows the cache through a raw pointer set
/// by [`with_active_resolved_class_cache`].  This is safe because the
/// pointer is only non-null while the [`ResolvedCacheGuard`] (which
/// holds a borrow of the original `&ResolvedClassCache`) is alive.
pub fn active_resolved_class_cache() -> Option<&'static ResolvedClassCache> {
    let ptr = ACTIVE_RESOLVED_CACHE.with(|c| c.get());
    if ptr.is_null() {
        None
    } else {
        // SAFETY: ptr was set from a valid &ResolvedClassCache reference
        // and the ResolvedCacheGuard that owns the borrow is still alive
        // (it clears the pointer on drop).
        Some(unsafe { &*ptr })
    }
}

/// Evict all cache entries whose FQN matches the given name, then
/// transitively evict any cached class that depends on the evicted
/// FQN through `parent_class`, `used_traits`, `interfaces`, or
/// `mixins`.
///
/// Because the cache is keyed by `(FQN, generic_args)`, a single FQN
/// may have multiple entries (one per distinct generic instantiation).
/// This helper removes all of them, which is used during targeted
/// cache invalidation when a class definition changes.
///
/// Transitive eviction is necessary because a cached child class
/// (e.g. `ChildJob extends ScheduledJob`) embeds fully-merged members
/// from its parent.  When the parent's `@property` docblock changes,
/// the child's cache entry still holds the old inherited property and
/// must be discarded.
///
/// The dependent set is found by walking the reverse-dependency index
/// maintained by [`ResolvedCacheInner`], so the cost is proportional to
/// the number of dependents rather than the size of the whole cache.
/// The returned vector lists the seed FQN followed by every transitively
/// evicted dependent, or is empty when nothing matched.
pub fn evict_fqn(cache: &mut ResolvedCacheInner, fqn: &str) -> Vec<String> {
    // Fast path: nothing to evict from an empty cache.
    if cache.map.is_empty() {
        return vec![];
    }

    // Walk the reverse-dependency graph starting from `fqn`, collecting the
    // seed plus every transitively dependent FQN.  Each step is an O(1) index
    // lookup, so the cost is proportional to the dependent set rather than the
    // whole cache.  `evicted` preserves [seed, ...dependents] discovery order.
    let mut evicted: Vec<String> = vec![fqn.to_string()];
    let mut seen: HashSet<String> = HashSet::from([fqn.to_string()]);
    let mut frontier: Vec<String> = vec![fqn.to_string()];

    while let Some(current) = frontier.pop() {
        // A dependent class stores the dependency either as the FQN or as the
        // short name, so look under both (mirroring the previous dual match).
        let short = crate::util::short_name(&current);
        let mut dependents: Vec<String> = Vec::new();
        if let Some(set) = cache.reverse_deps.get(current.as_str()) {
            dependents.extend(set.iter().map(|k| k.0.to_string()));
        }
        if short != current.as_str()
            && let Some(set) = cache.reverse_deps.get(short)
        {
            dependents.extend(set.iter().map(|k| k.0.to_string()));
        }

        for dep in dependents {
            if seen.insert(dep.clone()) {
                evicted.push(dep.clone());
                frontier.push(dep);
            }
        }
    }

    // Remove every generic-arg variant of each collected FQN.  All dependents
    // come from the reverse index (so they are present), but the seed may not
    // have been cached — when nothing was actually removed, report no eviction
    // to match the previous "return empty when nothing matched" behaviour.
    let mut any_removed = false;
    for f in &evicted {
        if cache.remove_all_variants(f) {
            any_removed = true;
        }
    }

    if any_removed { evicted } else { vec![] }
}

/// Collect the dependency strings a class references through its
/// `parent_class`, `used_traits`, `interfaces`, `mixins`, and
/// `casts_definitions`.
///
/// Each value is stored verbatim as it appears on the class: it may be a
/// fully-qualified name (cross-file, post-resolution) or a short name
/// (same-file reference).  Eviction looks up the reverse index under both
/// the evicted FQN and its short name, so storing the raw value here
/// reproduces the dual FQN / short-name matching.
///
/// Cast values may carry a `:argument` suffix (e.g. `"DecimalCast:8:2"`);
/// only the class portion is recorded.
fn dependency_keys(cls: &ClassInfo) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();

    if let Some(ref parent) = cls.parent_class {
        keys.push(parent.to_string());
    }
    keys.extend(cls.used_traits.iter().map(|t| t.to_string()));
    keys.extend(cls.interfaces.iter().map(|i| i.to_string()));
    keys.extend(cls.mixins.iter().map(|m| m.to_string()));

    if let Some(laravel) = cls.laravel() {
        for (_, cast_type) in &laravel.casts_definitions {
            let class_part = cast_type.split(':').next().unwrap_or(cast_type);
            keys.push(class_part.to_string());
        }
    }

    keys
}
