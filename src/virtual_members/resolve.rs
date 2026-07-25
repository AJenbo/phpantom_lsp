//! Full class resolution.
//!
//! The `resolve_class_fully*` entry points and their shared
//! implementation: base inheritance merging (via
//! [`resolve_class_with_inheritance`]), virtual member providers,
//! transitive interface merging with `@implements` generic
//! substitution, and Laravel patches.  Results are stored in the
//! [`ResolvedClassCache`] defined in [`super::cache`], which also
//! tracks in-flight resolutions so genuine dependency cycles (e.g.
//! two Eloquent models whose relationship providers resolve each
//! other) break with a base-inheritance partial result instead of
//! recursing forever.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::atom::atom;
use crate::inheritance::{
    ClassRef, apply_substitution_to_method, apply_substitution_to_property,
    enrich_method_from_ancestor, enrich_property_from_ancestor, resolve_class_with_inheritance,
};
use crate::php_type::PhpType;
use crate::types::ClassInfo;
use crate::util::short_name;
use crate::virtual_members::laravel::patches::apply_laravel_patches;

use super::laravel;
use super::{ResolvedClassCache, ResolvedClassCacheKey, apply_virtual_members, default_providers};

// ─── Cycle breaking ─────────────────────────────────────────────────────────
//
// `resolve_class_fully_inner` can re-enter itself when a virtual member
// provider or interface merge resolves a class that (transitively)
// depends on the class being resolved — a genuine dependency cycle,
// e.g. two Eloquent models whose relationship providers resolve each
// other.  Eager population in toposorted order makes such re-entry rare
// (dependencies are usually already cached), but cycles broken by the
// toposort and classes loaded on demand mid-resolution can still hit
// it.  The [`ResolvedCacheInner::mark_in_flight`] set — keyed by
// `(thread, FQN)` so concurrent resolutions of the same class on other
// threads proceed independently — detects the re-entry, and the
// re-entrant call returns a base-inheritance partial result instead of
// recursing forever.

/// RAII guard that removes a class's in-flight marker when its
/// resolution call tree unwinds (normally or by panic).
struct InFlightGuard<'a> {
    cache: &'a ResolvedClassCache,
    fqn: crate::atom::Atom,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.cache.write().unmark_in_flight(self.fqn);
    }
}

// ─── Eager class population ─────────────────────────────────────────────────

/// Pre-populate the [`ResolvedClassCache`] from a pre-computed
/// topologically sorted list of class FQNs.
///
/// Because the topological sort guarantees that all dependencies of a
/// class are processed before the class itself, each call to
/// [`resolve_class_fully_inner`] finds its parents, traits, interfaces,
/// and mixins already in the cache.  Provider callbacks (which call
/// `resolve_class_fully` for related classes) hit the cache immediately
/// instead of recursing, eliminating the unbounded mutual recursion that
/// previously caused stack overflow on large codebases.
///
/// This function should be called once after initial indexing is complete
/// (all files parsed into `uri_classes_index`) and again incrementally when files
/// change (see ER4 in `docs/todo/eager-resolution.md`).
///
/// # Arguments
///
/// * `sorted_fqns` — class FQNs in topological (dependency-first) order,
///   e.g. from [`crate::toposort::toposort_from_uri_classes_index`].
/// * `cache` — the [`ResolvedClassCache`] to populate (typically
///   `Backend.resolved_class_cache`).
/// * `class_loader` — a closure that resolves a class FQN to its raw
///   (un-merged) `ClassInfo`.  Typically `Backend::class_loader` or a
///   closure over `fqn_index` + `find_or_load_class`.
pub fn populate_from_sorted(
    sorted_fqns: &[String],
    cache: &ResolvedClassCache,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) {
    for fqn in sorted_fqns {
        // Skip classes already in the cache (e.g. from a previous
        // incremental population run).
        let cache_key: ResolvedClassCacheKey = (atom(fqn), Vec::new());
        {
            let map = cache.read();
            if map.contains_key(&cache_key) {
                continue;
            }
        }

        // Load the raw (un-merged) class.  If the class_loader cannot
        // find it (e.g. it was removed between toposort and now), skip.
        let raw_class = match class_loader(fqn) {
            Some(c) => c,
            None => continue,
        };

        // resolve_class_fully_inner will:
        // 1. Check cache (miss for this class, but hits for deps)
        // 2. Run resolve_class_with_inheritance (parent chain walk —
        //    parents are raw, but the walk is bounded by MAX_INHERITANCE_DEPTH)
        // 3. Run apply_virtual_members — providers call resolve_class_fully
        //    for related classes, which hit the cache (already populated)
        // 4. Merge interfaces — also hit cache for resolved interfaces
        // 5. Store result in cache
        resolve_class_fully_inner(&raw_class, class_loader, Some(cache));
    }
}

// ─── Full class resolution ──────────────────────────────────────────────────

/// Resolve a class with full inheritance and virtual member providers.
///
/// This is the primary entry point for completion, go-to-definition,
/// and any other feature that needs the complete set of members
/// visible on a class instance or static access.
///
/// The resolution proceeds in two phases:
///
/// 1. **Base resolution** via
///    [`resolve_class_with_inheritance`]: merges own members, trait
///    members, and parent chain members, applying generic type
///    substitution along the way.
///
/// 2. **Virtual member providers**: queries each registered provider
///    in priority order and merges their contributions.  Virtual
///    members never overwrite real declared members or contributions
///    from higher-priority providers.
///
/// Code that needs only the base resolution (e.g. providers
/// themselves, to avoid circular loading) should call
/// [`resolve_class_with_inheritance`] directly.
pub fn resolve_class_fully(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Arc<ClassInfo> {
    resolve_class_fully_inner(class, class_loader, None)
}

/// Cached variant of [`resolve_class_fully`].
///
/// Identical semantics, but stores and retrieves results from `cache`
/// so that repeated resolutions of the same class within a single
/// request cycle (or across requests between edits) are free.
///
/// The cache is keyed by the class's fully-qualified name
/// (`namespace\ClassName` or just `ClassName` for the global namespace).
/// Callers that apply post-resolution transforms (e.g.
/// [`apply_generic_args`](crate::inheritance::apply_generic_args)) should
/// still call this function for the base resolution and apply the
/// transform to the returned value.
pub fn resolve_class_fully_cached(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    cache: &ResolvedClassCache,
) -> Arc<ClassInfo> {
    resolve_class_fully_inner(class, class_loader, Some(cache))
}

/// Resolve a class fully, using the cache when available.
///
/// This is the preferred entry point for code paths that may or may
/// not have access to a [`ResolvedClassCache`] (e.g. context structs
/// where the cache field is `Option<&ResolvedClassCache>`).  When
/// `cache` is `Some`, behaves like [`resolve_class_fully_cached`];
/// when `None`, behaves like [`resolve_class_fully`].
pub fn resolve_class_fully_maybe_cached(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    cache: Option<&ResolvedClassCache>,
) -> Arc<ClassInfo> {
    let started = std::time::Instant::now();
    let result = resolve_class_fully_inner(class, class_loader, cache);
    let elapsed = started.elapsed();
    if elapsed >= std::time::Duration::from_millis(50) {
        tracing::info!(
            "PHPantom: resolve_class_fully_cached({}) took {:?}",
            class.fqn(),
            elapsed
        );
    }
    result
}

/// Resolve a class fully and apply generic type argument substitution,
/// caching the combined result under `(FQN, generic_arg_strings)`.
///
/// For generic classes like `Builder<Product>`, calling
/// [`resolve_class_fully_maybe_cached`] followed by
/// [`apply_generic_args`](crate::inheritance::apply_generic_args) is
/// expensive because `apply_generic_args` clones the entire class and
/// substitutes template parameters in every method and property.  On
/// an Eloquent model with hundreds of virtual members this takes
/// milliseconds per call, and a large service file can trigger it
/// hundreds of times for the same `(FQN, generic_args)` pair.
///
/// This function fuses the two steps and caches the result so the
/// substitution is performed at most once per `(FQN, generic_args)`.
/// When `generic_args` is empty or the class has no template
/// parameters, this behaves identically to
/// [`resolve_class_fully_maybe_cached`].
pub fn resolve_class_fully_with_generics(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    cache: Option<&ResolvedClassCache>,
    generic_arg_strings: &[String],
    generic_args: &[crate::php_type::PhpType],
) -> Arc<ClassInfo> {
    // Fast path: no generics — just do the base resolution.
    if generic_args.is_empty() {
        return resolve_class_fully_inner(class, class_loader, cache);
    }

    // Check the cache for (FQN, generic_args).
    let fqn = class.fqn();
    let cache_key: ResolvedClassCacheKey = (fqn, generic_arg_strings.to_vec());

    if let Some(c) = cache {
        let map = c.read();
        if let Some(cached) = map.get(&cache_key) {
            return Arc::clone(cached);
        }
    }

    let started = std::time::Instant::now();
    // Resolve the base class (cached at (FQN, [])).
    let base = resolve_class_fully_inner(class, class_loader, cache);

    // Apply generic substitution.
    let result = if !base.template_params.is_empty() {
        Arc::new(crate::inheritance::apply_generic_args(&base, generic_args))
    } else {
        base
    };

    // Store the substituted result.
    if let Some(c) = cache {
        c.write().insert(cache_key, Arc::clone(&result));
    }

    let elapsed = started.elapsed();
    if elapsed >= std::time::Duration::from_millis(50) {
        tracing::info!(
            "PHPantom: resolve_class_fully_with_generics({}, {:?}) took {:?}",
            class.fqn(),
            generic_arg_strings,
            elapsed
        );
    }

    result
}

/// Resolve a class fully and apply generic type arguments, deriving the
/// cache key strings from the structured [`PhpType`] arguments.
///
/// Use this on hot paths that already have concrete type arguments and would
/// otherwise call `resolve_class_fully_maybe_cached` followed by
/// `apply_generic_args`. The substituted result is cached under
/// `(FQN, generic_args)` while the base resolved class is still shared under
/// `(FQN, [])`.
pub fn resolve_class_fully_with_type_args(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    cache: Option<&ResolvedClassCache>,
    generic_args: &[crate::php_type::PhpType],
) -> Arc<ClassInfo> {
    if generic_args.is_empty() {
        return resolve_class_fully_inner(class, class_loader, cache);
    }

    let generic_arg_strings: Vec<String> = generic_args.iter().map(|arg| arg.to_string()).collect();
    resolve_class_fully_with_generics(
        class,
        class_loader,
        cache,
        &generic_arg_strings,
        generic_args,
    )
}

/// Shared implementation behind [`resolve_class_fully`] and
/// [`resolve_class_fully_cached`].
///
/// This always produces the *base* resolution, cached under the bare
/// FQN key `(FQN, [])`. Generic specialisation is a separate, much
/// cheaper stage layered on top by
/// [`resolve_class_fully_with_generics`]: it looks up this base result
/// and applies [`apply_generic_args`](crate::inheritance::apply_generic_args),
/// caching the substituted class under `(FQN, generic_args)`. Keeping the
/// two stages distinct means the expensive inheritance/trait/virtual-member
/// merge runs once per class, no matter how many generic instantiations
/// (`Builder<User>`, `Builder<Order>`, …) are requested.
fn resolve_class_fully_inner(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    cache: Option<&ResolvedClassCache>,
) -> Arc<ClassInfo> {
    // Cycle breaking needs somewhere to record in-flight resolutions,
    // so cache-less callers get a throwaway cache scoped to this call
    // tree.  Nested provider resolutions also reuse it, so shared
    // dependencies within the tree resolve once instead of repeatedly.
    let temp_cache;
    let cache: &ResolvedClassCache = match cache {
        Some(c) => c,
        None => {
            temp_cache = super::cache::new_resolved_class_cache();
            &temp_cache
        }
    };

    let fqn = class.fqn();
    let cache_key: ResolvedClassCacheKey = (fqn, Vec::new());

    // ── Reload raw class from class_loader ──────────────────────────
    // Callers sometimes pass a ClassInfo that has already been through
    // `apply_generic_args` (e.g. `MockBuilder<Rule>` where template
    // parameters are substituted with concrete types).  If we resolve
    // from that substituted class and cache the result under the bare
    // FQN key `(MockBuilder, [])`, every subsequent lookup gets the
    // contaminated version — causing non-deterministic diagnostics
    // depending on which thread/file was processed first.
    //
    // To prevent this, always try to reload the raw (un-substituted)
    // class from the class_loader.  The class_loader returns the
    // parsed ClassInfo from uri_classes_index/stubs, which has unsubstituted
    // template parameters.  Fall back to the passed-in `class` only
    // when the loader cannot find it (e.g. anonymous classes).
    //
    // Guard against a same-short-name import hijacking the reload: a
    // global class like `\Redis` has the bare FQN `Redis`, and a file
    // that imports `use Illuminate\Support\Facades\Redis;` would make
    // the import-aware `class_loader("Redis")` return the facade
    // instead.  Only accept the reload when it names the same class;
    // otherwise keep the already-resolved `class`.
    let raw_class_arc =
        class_loader(fqn.as_str()).filter(|raw| raw.fqn().eq_ignore_ascii_case(fqn.as_str()));
    let effective_class: &ClassInfo = match &raw_class_arc {
        Some(raw) => raw,
        None => class,
    };

    // ── Cache lookup ────────────────────────────────────────────────
    {
        let map = cache.read();
        if let Some(cached) = map.get(&cache_key) {
            return Arc::clone(cached);
        }
    }

    // ── Cycle break ─────────────────────────────────────────────────
    // If this class is already being resolved by this thread (re-entrant
    // call from a virtual member provider or interface merge on a
    // dependency cycle), return a partial result with base inheritance
    // only.  This breaks the mutual recursion that would otherwise
    // overflow the stack.
    if !cache.write().mark_in_flight(fqn) {
        return Arc::new(resolve_class_with_inheritance(
            effective_class,
            class_loader,
        ));
    }
    let _in_flight = InFlightGuard { cache, fqn };

    // ── Uncached resolution ─────────────────────────────────────────
    let mut merged = resolve_class_with_inheritance(effective_class, class_loader);

    // Whether Laravel-specific resolution should run.  A throwaway
    // cache defaults to `true`, so behaviour is unchanged for
    // cache-less code paths and tests.
    let is_laravel = cache.read().is_laravel();

    // ── Pre-provider patches ────────────────────────────────────────
    // Inject missing `@mixin` annotations before virtual member
    // providers run, so that `collect_mixin_members` picks them up.
    // All of these target framework classes, so they are skipped
    // entirely for non-Laravel projects.
    if is_laravel {
        if fqn.as_str() == "Illuminate\\Redis\\Connections\\Connection" {
            let mixin = atom("Redis");
            if !merged.mixins.contains(&mixin) {
                merged.mixins.push(mixin);
            }
        }
        if fqn.as_str() == "Illuminate\\Database\\Eloquent\\Builder" {
            let mixin = atom("Illuminate\\Database\\Query\\Builder");
            if !merged.mixins.contains(&mixin) {
                merged.mixins.push(mixin);
            }
        }
        // Bind core Illuminate contracts to their default concrete class as
        // a mixin, so unknown-method diagnostics resolve through the
        // concrete's `__call` (e.g. `Contracts\View\View` → `View\View`,
        // which uses `Macroable`).  The concrete is injected pre-provider
        // so `collect_mixin_members` merges its members.
        if let Some(concrete) = laravel::patches::contract_concrete_mixin(fqn.as_str()) {
            let mixin = atom(concrete);
            if !merged.mixins.contains(&mixin) {
                merged.mixins.push(mixin);
            }
        }
    }

    let providers = default_providers(is_laravel);
    if !providers.is_empty() {
        apply_virtual_members(&mut merged, class_loader, &providers, Some(cache));
    }

    // 3. Merge members from implemented interfaces.
    //    Interfaces can declare `@method` / `@property` / `@property-read`
    //    tags that should be visible on implementing classes.  We collect
    //    interfaces from the class itself and from every parent in the
    //    extends chain, then fully resolve each interface (which applies
    //    its own virtual member providers) and merge any members that
    //    don't already exist.
    //
    //    When a class declares `@implements SomeInterface<ConcreteType>`,
    //    the interface's template parameters are substituted with the
    //    concrete types before merging.  This mirrors how `@extends`
    //    generics are handled in the parent chain walk.  Substitutions
    //    from `@implements` on parent classes are also collected, with
    //    the `@extends` chain substitutions applied so that template
    //    parameters from intermediate classes resolve correctly.
    let mut all_iface_names: Vec<String> = effective_class
        .interfaces
        .iter()
        .map(|a| a.to_string())
        .collect();

    // Collect all `@implements` generics from the class and its parent
    // chain.  As we walk up the `extends` chain we apply the active
    // substitution map so that template parameter references in parent
    // `@implements` annotations resolve to concrete types.
    //
    // For example, given:
    //   class Test1<TKey> implements MyIterator<TKey, string>
    //   class Test2 extends Test1<int>
    //
    // Walking from Test2: active_subs starts empty, then after loading
    // Test1 we get {TKey => int}.  Test1's `@implements MyIterator<TKey, string>`
    // becomes `@implements MyIterator<int, string>` after substitution.
    let mut all_implements_generics: Vec<(String, Vec<PhpType>)> = effective_class
        .implements_generics
        .iter()
        .map(|(a, v)| (a.to_string(), v.clone()))
        .collect();
    {
        let mut current: ClassRef<'_> = ClassRef::Borrowed(effective_class);
        let mut depth = 0u32;
        let mut active_subs: HashMap<String, PhpType> = HashMap::new();

        // Seed initial subs from the root class's @extends generics
        // so that if the root class itself has template params referenced
        // in its @implements, they can be resolved.

        while let Some(ref parent_name) = current.parent_class {
            depth += 1;
            if depth > 20 {
                break;
            }
            if let Some(parent) = class_loader(parent_name) {
                // Build the substitution map for this parent level,
                // mirroring the logic in resolve_class_with_inheritance.
                let parent_short = short_name(&parent.name);
                let type_args = current
                    .extends_generics
                    .iter()
                    .chain(current.implements_generics.iter())
                    .find(|(name, _)| short_name(name) == parent_short)
                    .map(|(_, args)| args);

                let mut level_subs = if let Some(args) = type_args {
                    let mut map = HashMap::new();
                    for (i, param_name) in parent.template_params.iter().enumerate() {
                        if let Some(arg) = args.get(i) {
                            let resolved = if active_subs.is_empty() {
                                arg.clone()
                            } else {
                                arg.substitute(&active_subs)
                            };
                            map.insert(param_name.to_string(), resolved);
                        }
                    }
                    map
                } else {
                    active_subs.clone()
                };

                // If no explicit @extends generics matched but there are
                // active subs, carry them forward.
                if level_subs.is_empty() && !active_subs.is_empty() {
                    level_subs = active_subs.clone();
                }

                for iface in &parent.interfaces {
                    let iface_str = iface.to_string();
                    if !all_iface_names.contains(&iface_str) {
                        all_iface_names.push(iface_str);
                    }
                }

                // Collect parent's @implements generics with substitutions
                // applied so that template params resolve to concrete types.
                for (iface_name, args) in &parent.implements_generics {
                    let resolved_args: Vec<PhpType> = if level_subs.is_empty() {
                        args.clone()
                    } else {
                        args.iter().map(|a| a.substitute(&level_subs)).collect()
                    };
                    all_implements_generics.push((iface_name.to_string(), resolved_args));
                }

                active_subs = level_subs;
                current = ClassRef::Owned(parent);
            } else {
                break;
            }
        }
    }

    // Use an index-based loop so that we can grow `all_iface_names`
    // while iterating — each resolved interface may itself extend
    // additional interfaces that need to be collected transitively.
    let mut iface_idx = 0;
    while iface_idx < all_iface_names.len() {
        let iface_name = all_iface_names[iface_idx].clone();
        iface_idx += 1;

        if let Some(iface) = class_loader(&iface_name) {
            // Collect interfaces that this interface itself extends.
            // For example, CarbonInterface extends DateTimeInterface,
            // JsonSerializable, UnitValue — all of those need to be
            // resolved and their members merged transitively.
            for child_iface in &iface.interfaces {
                let child_iface_str = child_iface.to_string();
                if !all_iface_names.contains(&child_iface_str) {
                    all_iface_names.push(child_iface_str);
                }
            }

            // Build a substitution map from `@implements` generics for
            // this interface.  If the class (or a parent) declared
            // `@implements ThisInterface<Type1, Type2>`, map the
            // interface's template params to those concrete types.
            let mut iface_subs =
                build_implements_substitution_map(&iface_name, &iface, &all_implements_generics);

            // When no @implements generics were provided but the interface
            // declares template parameters, substitute each template param
            // with its declared upper bound (or `mixed`).  Without this,
            // raw template names like `TKey` / `TValue` leak through
            // inherited method signatures into downstream consumers.
            if iface_subs.is_empty() && !iface.template_params.is_empty() {
                for param in &iface.template_params {
                    let bound = iface
                        .template_param_bounds
                        .get(param)
                        .cloned()
                        .unwrap_or_else(PhpType::mixed);
                    iface_subs.insert(param.to_string(), bound);
                }
            }

            // Collect @extends / @implements generics from the
            // interface so that template substitutions flow through
            // transitive interface chains.  Apply the substitution map
            // we just built so that template params like `T` in
            // `@extends IteratorAggregate<T>` are resolved to concrete
            // types (e.g. `ReflectionArgument`) before propagation.
            for (name, args) in &iface.extends_generics {
                if !all_implements_generics.iter().any(|(n, _)| n == name) {
                    let resolved_args: Vec<PhpType> = if iface_subs.is_empty() {
                        args.clone()
                    } else {
                        args.iter().map(|a| a.substitute(&iface_subs)).collect()
                    };
                    all_implements_generics.push((name.to_string(), resolved_args));
                }
            }
            for (name, args) in &iface.implements_generics {
                if !all_implements_generics
                    .iter()
                    .any(|(n, _)| n.as_str() == name.as_str())
                {
                    let resolved_args: Vec<PhpType> = if iface_subs.is_empty() {
                        args.clone()
                    } else {
                        args.iter().map(|a| a.substitute(&iface_subs)).collect()
                    };
                    all_implements_generics.push((name.to_string(), resolved_args));
                }
            }

            // When we have substitutions to apply, we cannot use a
            // cached bare-interface resolution because the cached version
            // has unsubstituted template parameters.  Only use the cache
            // for interfaces without generic substitutions.
            if iface_subs.is_empty() {
                let iface_key: ResolvedClassCacheKey = (iface.fqn(), Vec::new());
                let map = cache.read();
                if let Some(cached) = map.get(&iface_key) {
                    let resolved_iface = ClassInfo::clone(cached);
                    drop(map);
                    merge_interface_members_into(&mut merged, resolved_iface, &iface_subs);
                    continue;
                }
            }

            let mut resolved_iface = resolve_class_with_inheritance(&iface, class_loader);
            if !providers.is_empty() {
                apply_virtual_members(&mut resolved_iface, class_loader, &providers, Some(cache));
            }

            merge_interface_members_into(&mut merged, resolved_iface, &iface_subs);
        }
    }

    // Store the accumulated `@implements` generics (with `@extends`
    // chain substitutions applied) on the merged class so that
    // downstream consumers like foreach resolution can see generics
    // from parent classes too.  For example, when `Test2 extends
    // Test1<int>` and `Test1` has `@implements MyIterator<TKey, string>`,
    // the merged Test2 class gets `implements_generics` containing
    // `("MyIterator", ["int", "string"])`.
    for (name, args) in &all_implements_generics {
        if !merged
            .implements_generics
            .iter()
            .any(|(n, _)| n.as_str() == name.as_str())
        {
            merged.implements_generics.push((atom(name), args.clone()));
        }
    }

    // ── Laravel class patches ──────────────────────────────────────
    // Apply centralized post-resolution patches for Laravel classes.
    // These modify existing members' type information (e.g. fixing
    // return types) rather than adding new members.  See
    // `laravel/patches.rs` for the full patch inventory.  Skipped for
    // non-Laravel projects.
    if is_laravel {
        apply_laravel_patches(&mut merged, &fqn);
    }

    // ── Cache store ─────────────────────────────────────────────────
    merged.rebuild_method_index();
    let result = Arc::new(merged);
    cache.write().insert(cache_key, Arc::clone(&result));

    result
}

/// Merge resolved interface members into a class, applying `@implements`
/// generic substitutions.
///
/// For methods and properties that already exist on the class, this fills
/// in missing type information from the interface declaration.  When a
/// class declares `boo()` with no return type but the interface has
/// `@return Y`, the substituted interface return type is applied to the
/// class method.  Similarly, parameter docblock types from the interface
/// are applied when the class parameter lacks a type hint or has a
/// less-specific native hint (e.g. `object`) while the interface provides
/// a concrete docblock type.
///
/// Members that don't already exist on the class are added directly.
fn merge_interface_members_into(
    merged: &mut ClassInfo,
    mut resolved_iface: ClassInfo,
    iface_subs: &HashMap<String, PhpType>,
) {
    // Apply @implements generic substitutions to the resolved
    // interface members before merging.
    if !iface_subs.is_empty() {
        for method in resolved_iface.methods.make_mut().iter_mut() {
            apply_substitution_to_method(Arc::make_mut(method), iface_subs);
        }
        for property in resolved_iface.properties.make_mut().iter_mut() {
            apply_substitution_to_property(property, iface_subs);
        }
    }

    // For small method counts, linear scan is cheaper than building a
    // HashMap (avoids N String allocations for the keys).  The threshold
    // is chosen so that the HashMap overhead is amortised by the number
    // of interface methods that need lookup.
    const HASHMAP_THRESHOLD: usize = 32;

    let method_index: Option<HashMap<String, usize>> = if merged.methods.len() >= HASHMAP_THRESHOLD
    {
        Some(
            merged
                .methods
                .iter()
                .enumerate()
                .map(|(i, m)| (m.name.to_string(), i))
                .collect(),
        )
    } else {
        None
    };

    for iface_method in resolved_iface.methods.into_vec() {
        // Find the existing method index — O(1) via HashMap or O(N) linear scan.
        let existing_idx = if let Some(ref index) = method_index {
            index.get(&iface_method.name.to_string()).copied()
        } else {
            merged
                .methods
                .iter()
                .position(|m| m.name == iface_method.name)
        };

        if let Some(idx) = existing_idx {
            let existing = &mut merged.methods.make_mut()[idx];
            // Delegate to the shared enrichment helper which handles
            // return types, parameters, descriptions, template params,
            // conditional returns, and type assertions uniformly.
            enrich_method_from_ancestor(Arc::make_mut(existing), &iface_method);
        } else {
            merged.methods.push(iface_method);
        }
    }
    // Merge interface properties — enrich existing ones, add new ones.
    for property in resolved_iface.properties.into_vec() {
        if let Some(existing) = merged
            .properties
            .make_mut()
            .iter_mut()
            .find(|p| p.name == property.name)
        {
            enrich_property_from_ancestor(existing, &property);
        } else {
            merged.properties.push(property);
        }
    }
    let existing_consts: HashSet<String> = merged
        .constants
        .iter()
        .map(|c| c.name.to_string())
        .collect();
    for constant in resolved_iface.constants.into_vec() {
        if !existing_consts.contains(&constant.name.to_string()) {
            merged.constants.push(constant);
        }
    }
}

/// Build a substitution map for an interface based on collected
/// `@implements` generics.
///
/// Searches `all_implements_generics` for an entry whose class name
/// matches `iface_name` (by FQN comparison), then zips the
/// type arguments with the interface's `template_params`.
///
/// Returns an empty map if no matching `@implements` annotation exists
/// or if the interface has no template parameters.
fn build_implements_substitution_map(
    iface_name: &str,
    iface: &ClassInfo,
    all_implements_generics: &[(String, Vec<PhpType>)],
) -> HashMap<String, PhpType> {
    if iface.template_params.is_empty() || all_implements_generics.is_empty() {
        return HashMap::new();
    }

    let type_args = all_implements_generics
        .iter()
        .find(|(name, _)| name == iface_name)
        .map(|(_, args)| args);

    let type_args = match type_args {
        Some(args) => args,
        None => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for (i, param_name) in iface.template_params.iter().enumerate() {
        if let Some(arg) = type_args.get(i) {
            map.insert(param_name.to_string(), arg.clone());
        }
    }
    map
}
