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

pub mod cache;
pub mod laravel;
pub mod phpdoc;
pub mod resolve;

pub use cache::{
    ResolvedClassCache, ResolvedClassCacheKey, active_resolved_class_cache, evict_fqn,
    new_resolved_class_cache, with_active_resolved_class_cache,
};
pub use resolve::{
    populate_from_sorted, resolve_class_fully, resolve_class_fully_cached,
    resolve_class_fully_maybe_cached, resolve_class_fully_with_generics,
    resolve_class_fully_with_type_args,
};

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::php_type::PhpType;
use crate::types::{ClassInfo, ConstantInfo, MethodInfo, PropertyInfo};

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
    // Build an index of (name, is_static) → position for O(1) dedup
    // instead of a linear scan per virtual method.  With hundreds of
    // methods on Eloquent models this turns O(N×M) memcmp into O(M).
    // Keys are lowercased: PHP method names are case-insensitive, so a
    // `@method getvalue` tag matches a declared `getValue()`.
    let mut method_index: HashMap<(String, bool), usize> = class
        .methods
        .iter()
        .enumerate()
        .map(|(i, m)| ((m.name.to_ascii_lowercase(), m.is_static), i))
        .collect();

    for method in virtual_members.methods {
        let key = (method.name.to_ascii_lowercase(), method.is_static);
        if let Some(&idx) = method_index.get(&key) {
            if class.methods[idx].has_scope_attribute
                || matches!(method.name.as_str(), "query" | "newQuery" | "newModelQuery")
            {
                // Replace the original with the synthesized virtual method.
                // For scope attributes, the original is an implementation detail.
                // For query methods, we want to return the custom builder.
                class.methods.make_mut()[idx] = Arc::new(method);
            }
            // Otherwise: real declared member — keep the original.
        } else {
            let new_idx = class.methods.len();
            class.methods.push(Arc::new(method));
            method_index.insert(key, new_idx);
        }
    }

    // Build a property name → position index for O(1) dedup.
    let mut prop_index: HashMap<String, usize> = class
        .properties
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.to_string(), i))
        .collect();

    for property in virtual_members.properties {
        if let Some(&idx) = prop_index.get(&property.name.to_string()) {
            // The property already exists.  Replace it only when the
            // incoming property carries a strictly more specific type.
            // This lets PHPDoc `@property array<string> $tags` override
            // a bare `array` from `$casts`, and a `$casts` `array`
            // override `mixed` from `$fillable`.
            if property_type_specificity(&property)
                > property_type_specificity(&class.properties[idx])
            {
                class.properties.make_mut()[idx] = property;
            }
        } else {
            let new_idx = class.properties.len();
            prop_index.insert(property.name.to_string(), new_idx);
            class.properties.push(property);
        }
    }
    let mut const_names: HashSet<String> =
        class.constants.iter().map(|c| c.name.to_string()).collect();
    for constant in virtual_members.constants {
        if const_names.insert(constant.name.to_string()) {
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
fn type_specificity(hint: &Option<PhpType>) -> u8 {
    match hint {
        None => 0,
        Some(t) if t.is_mixed() => 0,
        Some(PhpType::Raw(s)) if s.trim().is_empty() => 0,
        Some(t) if t.has_type_structure() => 2,
        Some(_) => 1,
    }
}

/// Score a property's type by how specific it is, considering both
/// native and effective type hints.
///
/// The function first checks the effective type hint (docblock override),
/// then falls back to the native type hint if the effective type is
/// absent or non-specific.
///
/// This ensures that properties with actual PHP type declarations
/// (e.g., `public string $name`) are ranked higher than those without
/// any type information, even when docblocks are absent.
fn property_type_specificity(property: &PropertyInfo) -> u8 {
    // First check the effective type hint (may include docblock override)
    let effective_score = type_specificity(&property.type_hint);

    // If effective type is specific enough, use it
    if effective_score > 0 {
        return effective_score;
    }

    // Otherwise, fall back to native type hint
    type_specificity(&property.native_type_hint)
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
///
/// When `is_laravel` is `false`, the two Laravel providers are omitted so
/// that a non-Laravel project does not pay for their parent-chain walks.
/// The framework-agnostic PHPDoc provider is always included.
pub fn default_providers(is_laravel: bool) -> Vec<Box<dyn VirtualMemberProvider>> {
    let mut providers: Vec<Box<dyn VirtualMemberProvider>> = Vec::new();
    if is_laravel {
        // Laravel model provider — relationship properties, scopes, Builder
        // forwarding, convention-based factory() method.
        providers.push(Box::new(laravel::LaravelModelProvider));
        // Laravel factory provider — convention-based create()/make() methods
        // for factory classes extending Illuminate\Database\Eloquent\Factories\Factory.
        providers.push(Box::new(laravel::LaravelFactoryProvider));
    }
    // PHPDoc provider — @method / @property / @mixin tags.
    providers.push(Box::new(phpdoc::PHPDocProvider));
    providers
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;
