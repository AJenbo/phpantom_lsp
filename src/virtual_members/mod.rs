//! Virtual member provider abstraction.
//!
//! Virtual members are methods and properties that do not exist as real
//! PHP declarations but are surfaced by magic methods (`__call`, `__get`,
//! `__set`, etc.) or framework conventions.  Three providers produce
//! virtual members today:
//!
//! 1. **Laravel model provider** — synthesizes members from
//!    framework-specific patterns (relationship properties, scope methods,
//!    Builder-as-static forwarding, convention-based `factory()` method).
//! 2. **Laravel factory provider** — synthesizes `create()` and `make()`
//!    methods on factory classes that return the corresponding model type,
//!    using the naming convention when no `@extends Factory<Model>`
//!    annotation is present.
//! 3. **PHPDoc provider** (`@method`, `@property`, `@property-read`,
//!    `@property-write`, `@mixin`) — documents magic members on a class.
//!    Within this provider, explicit `@method` / `@property` tags take
//!    precedence over members inherited from `@mixin` classes.
//!
//! All are unified behind the [`VirtualMemberProvider`] trait.
//! Providers are queried in priority order after base resolution
//! (own members + traits + parent chain) is complete.  A member
//! contributed by a higher-priority provider is never overwritten by a
//! lower-priority one, and all virtual members lose to real declared
//! members.
//!
//! # Caching
//!
//! [`resolve_class_fully`] is called from many code paths (completion,
//! hover, go-to-definition, call resolution, etc.) and often for the
//! same class within a single request.  The full resolution (inheritance
//! walk + virtual member providers + interface merging) is expensive, so
//! [`resolve_class_fully_cached`] accepts a [`ResolvedClassCache`] that
//! stores results keyed by fully-qualified class name.  The cache is
//! stored on `Backend` and cleared whenever a file is re-parsed
//! (`update_ast` / `parse_and_cache_content`), so stale entries never
//! survive an edit.
//!
//! # Precedence model
//!
//! ```text
//! 1. Real declared members (in PHP source code)
//! 2. Trait members (real implementations)
//! 3. Parent chain members (real implementations)
//! 4. Virtual member providers (in priority order):
//!    a. Laravel model provider  — richest type info
//!    b. Laravel factory provider — convention-based factory methods
//!    c. PHPDoc provider          — @method, @property, @mixin
//! ```

pub mod laravel;
pub mod phpdoc;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::Mutex;

use crate::inheritance::{
    apply_substitution, apply_substitution_to_method, apply_substitution_to_property,
    resolve_class_with_inheritance,
};
use crate::types::{ClassInfo, ConstantInfo, MethodInfo, PropertyInfo};
use crate::util::short_name;

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
pub type ResolvedClassCacheKey = (String, Vec<String>);

/// Thread-safe cache of fully-resolved classes, keyed by FQN + generic args.
///
/// Stored on [`Backend`](crate::Backend) and selectively invalidated
/// when a file is re-parsed (`update_ast` / `parse_and_cache_content`).
/// Within a single request cycle (completion, hover, etc.) the cache
/// eliminates redundant calls to [`resolve_class_fully`] for the same
/// class at the same generic instantiation.
pub type ResolvedClassCache = Arc<Mutex<HashMap<ResolvedClassCacheKey, Arc<ClassInfo>>>>;

/// Create a new, empty [`ResolvedClassCache`].
pub fn new_resolved_class_cache() -> ResolvedClassCache {
    Arc::new(Mutex::new(HashMap::new()))
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
pub fn evict_fqn(cache: &mut HashMap<ResolvedClassCacheKey, Arc<ClassInfo>>, fqn: &str) {
    // Collect the set of FQNs to evict, starting with the requested one.
    // After removing direct matches, scan remaining entries for classes
    // whose inheritance chain references any evicted FQN and repeat
    // until no new dependents are found (fixed-point).
    let mut evicted: Vec<String> = vec![fqn.to_string()];
    cache.retain(|(k, _), _| k != fqn);

    loop {
        let mut newly_evicted: Vec<String> = Vec::new();

        for ((cached_fqn, _), cls) in cache.iter() {
            if depends_on_any(cls, &evicted) && !evicted.contains(cached_fqn) {
                newly_evicted.push(cached_fqn.clone());
            }
        }

        if newly_evicted.is_empty() {
            break;
        }

        for dep_fqn in &newly_evicted {
            cache.retain(|(k, _), _| k != dep_fqn);
            evicted.push(dep_fqn.clone());
        }
    }
}

/// Check whether `cls` directly depends on any FQN in `fqns` through
/// its `parent_class`, `used_traits`, `interfaces`, `mixins`, or
/// `casts_definitions`.
///
/// Comparisons are done against both the raw field value and the short
/// name of the evicted FQN, because the cached `ClassInfo` may store
/// parent/trait/interface names as short names (same-file references)
/// or as FQNs (cross-file, post-resolution).
///
/// The `casts_definitions` check ensures that when a cast class is
/// edited (e.g. its `@implements CastsAttributes<T>` annotation
/// changes), models referencing that cast class via `$casts` are
/// transitively evicted from the resolved-class cache.
fn depends_on_any(cls: &ClassInfo, fqns: &[String]) -> bool {
    for fqn in fqns {
        let short = crate::util::short_name(fqn);

        // parent_class
        if let Some(ref parent) = cls.parent_class
            && (parent == fqn || parent == short)
        {
            return true;
        }

        // used_traits
        if cls.used_traits.iter().any(|t| t == fqn || t == short) {
            return true;
        }

        // interfaces
        if cls.interfaces.iter().any(|i| i == fqn || i == short) {
            return true;
        }

        // mixins
        if cls.mixins.iter().any(|m| m == fqn || m == short) {
            return true;
        }

        // casts_definitions — cast type values may reference class FQNs
        // (e.g. `"App\\Casts\\DecimalCast"` or `"DecimalCast:8:2"`).
        // Strip the `:argument` suffix before comparing.
        if let Some(laravel) = cls.laravel()
            && laravel.casts_definitions.iter().any(|(_, cast_type)| {
                let class_part = cast_type.split(':').next().unwrap_or(cast_type);
                class_part == fqn || class_part == short
            })
        {
            return true;
        }
    }
    false
}

/// Members synthesized by a provider.
///
/// Merged below real declared members, traits, and the parent chain.
/// Each provider returns a `VirtualMembers` value from its
/// [`provide`](VirtualMemberProvider::provide) method.
pub struct VirtualMembers {
    /// Virtual methods to add to the class.
    pub methods: Vec<MethodInfo>,
    /// Virtual properties to add to the class.
    pub properties: Vec<PropertyInfo>,
    /// Virtual constants to add to the class.
    pub constants: Vec<ConstantInfo>,
}

impl VirtualMembers {
    /// Whether this value contains no methods, properties, or constants.
    pub fn is_empty(&self) -> bool {
        self.methods.is_empty() && self.properties.is_empty() && self.constants.is_empty()
    }
}

/// A provider that contributes virtual members to a class.
///
/// Receives the class with traits and parents already merged (via
/// [`resolve_class_with_inheritance`](crate::inheritance::resolve_class_with_inheritance)),
/// but **without** other providers' contributions.  This prevents
/// circular loading when one provider's output would trigger another
/// provider.
///
/// Implementations must be cheap to construct and stateless.  All
/// contextual information is passed through the `class` and
/// `class_loader` arguments.
pub trait VirtualMemberProvider {
    /// Whether this provider has anything to say about this class.
    ///
    /// This is a cheap pre-check so the resolver can skip providers
    /// early without calling [`provide`](Self::provide).  Returning
    /// `false` means [`provide`](Self::provide) will not be called.
    fn applies_to(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> bool;

    /// Produce virtual members for this class.
    ///
    /// Only called when [`applies_to`](Self::applies_to) returned `true`.
    /// The returned members are merged into the class below all real
    /// declared members (own, trait, and parent chain).
    ///
    /// `cache` is the shared resolved-class cache.  Providers that need
    /// to fully resolve helper classes (e.g. the Laravel model provider
    /// resolving the Eloquent Builder) should use
    /// [`resolve_class_fully_cached`] via this cache to avoid redundant
    /// work across requests.
    fn provide(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        cache: Option<&ResolvedClassCache>,
    ) -> VirtualMembers;
}

/// Merge virtual members into a resolved `ClassInfo`.
///
/// For each method in `virtual.methods`, adds it to `class.methods` only
/// if no method with the same name and same staticness already exists.
/// This allows a provider to contribute both a static and an instance
/// variant of the same method (e.g. Laravel scope methods that are
/// accessible via both `User::active()` and `$user->active()`).
///
/// **Exception:** when the existing method has `has_scope_attribute: true`,
/// the virtual method **replaces** it.  `#[Scope]`-attributed methods
/// share their name with the synthesized scope method, but the original
/// is a `protected` implementation detail that should not appear in
/// completion results.  The virtual replacement is `public` with the
/// first `$query` parameter stripped, which is what callers actually see.
///
/// Properties are deduplicated by name.  When a property with the same
/// name already exists, the **more specific** type wins regardless of
/// which provider contributed it.  Specificity is ranked as:
///
///   `array<int, string>` > `array` > `mixed` > (absent)
///
/// More precisely:
/// - absent / empty / `mixed` is the weakest (score 0)
/// - a bare type like `array`, `string`, `Collection` (score 1)
/// - a type with generic parameters like `array<int>` (score 2)
///
/// This allows PHPDoc `@property array<string> $tags` to override a
/// bare `array` from `$casts`, and a `$casts` `array` to override
/// `mixed` from `$fillable`.
///
/// Constants are deduplicated by name only.
///
/// This ensures that real declared members (and contributions from
/// higher-priority providers that were merged earlier) are never
/// overwritten, unless the incoming property carries a more specific type.
pub fn merge_virtual_members(class: &mut ClassInfo, virtual_members: VirtualMembers) {
    for method in virtual_members.methods {
        let existing = class
            .methods
            .iter()
            .position(|m| m.name == method.name && m.is_static == method.is_static);
        match existing {
            Some(idx) if class.methods[idx].has_scope_attribute => {
                // Replace the #[Scope]-attributed original with the
                // synthesized virtual scope method.
                class.methods[idx] = method;
            }
            Some(_) => {
                // Real declared member — keep the original.
            }
            None => {
                class.methods.push(method);
            }
        }
    }
    for property in virtual_members.properties {
        if let Some(idx) = class
            .properties
            .iter()
            .position(|p| p.name == property.name)
        {
            // The property already exists.  Replace it only when the
            // incoming property carries a strictly more specific type.
            // This lets PHPDoc `@property array<string> $tags` override
            // a bare `array` from `$casts`, and a `$casts` `array`
            // override `mixed` from `$fillable`.
            if type_specificity(&property.type_hint)
                > type_specificity(&class.properties[idx].type_hint)
            {
                class.properties[idx] = property;
            }
        } else {
            class.properties.push(property);
        }
    }
    let mut const_names: HashSet<String> = class.constants.iter().map(|c| c.name.clone()).collect();
    for constant in virtual_members.constants {
        if const_names.insert(constant.name.clone()) {
            class.constants.push(constant);
        }
    }
}

/// Score a type hint by how specific it is.
///
/// The ranking (lowest to highest):
/// - **0** — absent, empty, or `mixed` (no useful type information)
/// - **1** — a bare type name without generic parameters
///   (e.g. `array`, `string`, `Collection`, `?Foo`)
/// - **2** — a type with generic parameters
///   (e.g. `array<int, string>`, `Collection<User>`)
///
/// When merging virtual properties, the property with the higher
/// specificity score wins.  Equal scores preserve the existing property
/// (first-writer-wins within the same specificity tier).
fn type_specificity(hint: &Option<String>) -> u8 {
    match hint {
        None => 0,
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed == "mixed" {
                0
            } else if trimmed.contains('<') {
                2
            } else {
                1
            }
        }
    }
}

/// Apply all registered providers to a base-resolved class.
///
/// Iterates over `providers` in order (highest priority first) and
/// merges each provider's virtual members into `class`.  Because
/// [`merge_virtual_members`] skips members that already exist,
/// higher-priority providers' contributions shadow lower-priority ones.
pub fn apply_virtual_members(
    class: &mut ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    providers: &[Box<dyn VirtualMemberProvider>],
    cache: Option<&ResolvedClassCache>,
) {
    for provider in providers {
        if provider.applies_to(class, class_loader) {
            let virtual_members = provider.provide(class, class_loader, cache);
            if !virtual_members.is_empty() {
                merge_virtual_members(class, virtual_members);
            }
        }
    }
}

/// Return the default set of virtual member providers in priority order.
///
/// Providers are queried in order; a member contributed by an earlier
/// provider is never overwritten by a later one.
///
/// 1. Laravel model provider (highest priority — richest type info)
/// 2. Laravel factory provider (convention-based create/make methods)
/// 3. PHPDoc provider (`@method` / `@property` / `@mixin` tags)
pub fn default_providers() -> Vec<Box<dyn VirtualMemberProvider>> {
    vec![
        // Laravel model provider — relationship properties, scopes, Builder
        // forwarding, convention-based factory() method.
        Box::new(laravel::LaravelModelProvider),
        // Laravel factory provider — convention-based create()/make() methods
        // for factory classes extending Illuminate\Database\Eloquent\Factories\Factory.
        Box::new(laravel::LaravelFactoryProvider),
        // PHPDoc provider — @method / @property / @mixin tags.
        Box::new(phpdoc::PHPDocProvider),
    ]
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
) -> ClassInfo {
    resolve_class_fully_inner(class, class_loader, None, &[])
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
) -> ClassInfo {
    resolve_class_fully_inner(class, class_loader, Some(cache), &[])
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
) -> ClassInfo {
    resolve_class_fully_inner(class, class_loader, cache, &[])
}

/// Compute the fully-qualified name used as the cache key.
///
/// Mirrors the FQN construction in `update_ast_inner` and
/// `parse_and_cache_content`: `namespace\ClassName` when a namespace
/// is present, or just the short name otherwise.
fn class_fqn(class: &ClassInfo) -> String {
    match &class.file_namespace {
        Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, class.name),
        _ => class.name.clone(),
    }
}

/// Shared implementation behind [`resolve_class_fully`] and
/// [`resolve_class_fully_cached`].
fn resolve_class_fully_inner(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    cache: Option<&ResolvedClassCache>,
    generic_args: &[String],
) -> ClassInfo {
    let fqn = class_fqn(class);
    let cache_key: ResolvedClassCacheKey = (fqn.clone(), generic_args.to_vec());

    // ── Cache lookup ────────────────────────────────────────────────
    if let Some(cache) = cache {
        let map = cache.lock();
        if let Some(cached) = map.get(&cache_key) {
            return ClassInfo::clone(cached);
        }
    }

    // ── Uncached resolution ─────────────────────────────────────────
    let mut merged = resolve_class_with_inheritance(class, class_loader);
    let providers = default_providers();
    if !providers.is_empty() {
        apply_virtual_members(&mut merged, class_loader, &providers, cache);
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
    let mut all_iface_names: Vec<String> = class.interfaces.clone();

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
    let mut all_implements_generics: Vec<(String, Vec<String>)> = class.implements_generics.clone();
    {
        let mut current = class.clone();
        let mut depth = 0u32;
        let mut active_subs: HashMap<String, String> = HashMap::new();

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
                                apply_substitution(arg, &active_subs).into_owned()
                            };
                            map.insert(param_name.clone(), resolved);
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
                    if !all_iface_names.contains(iface) {
                        all_iface_names.push(iface.clone());
                    }
                }

                // Collect parent's @implements generics with substitutions
                // applied so that template params resolve to concrete types.
                for (iface_name, args) in &parent.implements_generics {
                    let resolved_args: Vec<String> = if level_subs.is_empty() {
                        args.clone()
                    } else {
                        args.iter()
                            .map(|a| apply_substitution(a, &level_subs).into_owned())
                            .collect()
                    };
                    all_implements_generics.push((iface_name.clone(), resolved_args));
                }

                active_subs = level_subs;
                current = Arc::unwrap_or_clone(parent);
            } else {
                break;
            }
        }
    }

    for iface_name in &all_iface_names {
        if let Some(iface) = class_loader(iface_name) {
            // Build a substitution map from `@implements` generics for
            // this interface.  If the class (or a parent) declared
            // `@implements ThisInterface<Type1, Type2>`, map the
            // interface's template params to those concrete types.
            let iface_subs =
                build_implements_substitution_map(iface_name, &iface, &all_implements_generics);

            // When we have substitutions to apply, we cannot use a
            // cached bare-interface resolution because the cached version
            // has unsubstituted template parameters.  Only use the cache
            // for interfaces without generic substitutions.
            if iface_subs.is_empty() {
                let iface_key: ResolvedClassCacheKey = (class_fqn(&iface), Vec::new());
                if let Some(c) = cache {
                    let map = c.lock();
                    if let Some(cached) = map.get(&iface_key) {
                        let resolved_iface = ClassInfo::clone(cached);
                        drop(map);
                        merge_interface_members_into(&mut merged, resolved_iface, &iface_subs);
                        continue;
                    }
                }
            }

            let mut resolved_iface = resolve_class_with_inheritance(&iface, class_loader);
            if !providers.is_empty() {
                apply_virtual_members(&mut resolved_iface, class_loader, &providers, cache);
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
            .any(|(n, _)| short_name(n) == short_name(name))
        {
            merged
                .implements_generics
                .push((name.clone(), args.clone()));
        }
    }

    // ── Cache store ─────────────────────────────────────────────────
    if let Some(cache) = cache {
        cache.lock().insert(cache_key, Arc::new(merged.clone()));
    }

    merged
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
    iface_subs: &HashMap<String, String>,
) {
    // Apply @implements generic substitutions to the resolved
    // interface members before merging.
    if !iface_subs.is_empty() {
        for method in &mut resolved_iface.methods {
            apply_substitution_to_method(method, iface_subs);
        }
        for property in &mut resolved_iface.properties {
            apply_substitution_to_property(property, iface_subs);
        }
    }

    for iface_method in resolved_iface.methods {
        if let Some(existing) = merged
            .methods
            .iter_mut()
            .find(|m| m.name == iface_method.name)
        {
            // Fill in missing return type from the interface.
            if existing.return_type.is_none() && iface_method.return_type.is_some() {
                existing.return_type = iface_method.return_type.clone();
            }
            // Fill in missing template information from the interface.
            // When a class implements an interface with a generic method
            // (e.g. `@template T` + `@param class-string<T>` + `@return T`),
            // the implementing class's override typically lacks these
            // docblock annotations.  Inheriting them lets the call-site
            // resolver build template substitutions and resolve the
            // concrete return type.
            if existing.template_params.is_empty() && !iface_method.template_params.is_empty() {
                existing.template_params = iface_method.template_params;
                existing.template_param_bounds = iface_method.template_param_bounds;
                existing.template_bindings = iface_method.template_bindings;
                // Also inherit the return type if the interface provides
                // one and we haven't already set it above — template
                // return types like `T` only make sense when the template
                // params are present.
                if existing.return_type.is_none() {
                    existing.return_type = iface_method.return_type;
                }
            }
            // Fill in missing conditional return type from the interface.
            if existing.conditional_return.is_none() && iface_method.conditional_return.is_some() {
                existing.conditional_return = iface_method.conditional_return;
            }
            // Fill in missing type assertions from the interface.
            if existing.type_assertions.is_empty() && !iface_method.type_assertions.is_empty() {
                existing.type_assertions = iface_method.type_assertions;
            }
            // Fill in parameter docblock types from the interface.
            // When the class parameter's type_hint equals its native_type_hint
            // (meaning no @param docblock override was applied on the class),
            // the interface's substituted type is more specific and should
            // be used.  This handles cases like `map(object $entity)` where
            // the interface declares `@param TEntity $entity` and @implements
            // substitutes `TEntity` → `Boo`.
            for (existing_param, iface_param) in
                existing.parameters.iter_mut().zip(&iface_method.parameters)
            {
                let has_own_docblock_type = existing_param.type_hint.is_some()
                    && existing_param.type_hint != existing_param.native_type_hint;
                if !has_own_docblock_type && iface_param.type_hint.is_some() {
                    existing_param.type_hint = iface_param.type_hint.clone();
                }
            }
        } else {
            merged.methods.push(iface_method);
        }
    }
    let existing_props: HashSet<String> =
        merged.properties.iter().map(|p| p.name.clone()).collect();
    for property in resolved_iface.properties {
        if !existing_props.contains(&property.name) {
            merged.properties.push(property);
        }
    }
    let existing_consts: HashSet<String> =
        merged.constants.iter().map(|c| c.name.clone()).collect();
    for constant in resolved_iface.constants {
        if !existing_consts.contains(&constant.name) {
            merged.constants.push(constant);
        }
    }
}

/// Build a substitution map for an interface based on collected
/// `@implements` generics.
///
/// Searches `all_implements_generics` for an entry whose class name
/// matches `iface_name` (by short name comparison), then zips the
/// type arguments with the interface's `template_params`.
///
/// Returns an empty map if no matching `@implements` annotation exists
/// or if the interface has no template parameters.
fn build_implements_substitution_map(
    iface_name: &str,
    iface: &ClassInfo,
    all_implements_generics: &[(String, Vec<String>)],
) -> HashMap<String, String> {
    if iface.template_params.is_empty() || all_implements_generics.is_empty() {
        return HashMap::new();
    }

    let iface_short = short_name(iface_name);

    let type_args = all_implements_generics
        .iter()
        .find(|(name, _)| short_name(name) == iface_short)
        .map(|(_, args)| args);

    let type_args = match type_args {
        Some(args) => args,
        None => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for (i, param_name) in iface.template_params.iter().enumerate() {
        if let Some(arg) = type_args.get(i) {
            map.insert(param_name.clone(), arg.clone());
        }
    }
    map
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
