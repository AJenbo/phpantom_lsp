//! Laravel Eloquent Model virtual member provider.
//!
//! Synthesizes virtual members for classes that extend
//! `Illuminate\Database\Eloquent\Model`.  This is the highest-priority
//! virtual member provider: its contributions beat `@method` /
//! `@property` / `@mixin` members (PHPDocProvider).
//!
//! Currently implements:
//!
//! - **Relationship properties.** Methods returning a known Eloquent
//!   relationship type (e.g. `HasOne`, `HasMany`, `BelongsTo`) produce
//!   a virtual property with the same name.  The property type is
//!   inferred from the relationship's generic parameters (Larastan-style
//!   `@return HasMany<Post, $this>` annotations) or, as a fallback,
//!   from the first `::class` argument in the method body text.
//!
//! - **Scope methods.** Methods whose name starts with `scope` (e.g.
//!   `scopeActive`, `scopeVerified`) produce a virtual method with the
//!   `scope` prefix stripped and the first letter lowercased (e.g.
//!   `active`, `verified`).  Methods decorated with `#[Scope]`
//!   (Laravel 11+) are also recognized: their own name is used
//!   directly as the public-facing scope name (e.g.
//!   `#[Scope] protected function active()` becomes `active()`).
//!   The first `$query` parameter is removed.
//!   Scope methods are available as both static and instance methods
//!   so they resolve for `User::active()` and `$user->active()`.
//!
//! - **Builder-as-static forwarding.** Laravel's `Model::__callStatic()`
//!   forwards static calls to `static::query()`, which returns an
//!   Eloquent Builder.  This provider loads
//!   `\Illuminate\Database\Eloquent\Builder`, fully resolves it
//!   (including its `@mixin` on `Query\Builder`), and presents its
//!   public instance methods as static virtual methods on the model.
//!   Return types are mapped so that `static`/`$this`/`self` resolve
//!   to `Builder<ConcreteModel>` (the chain continues on the builder)
//!   and template parameters like `TModel` resolve to the concrete
//!   model class.  This makes `User::where(...)->orderBy(...)->get()`
//!   resolve end-to-end.
//!
//! - **Cast properties.** Entries in the `$casts` property array or
//!   `casts()` method body produce typed virtual properties.  Cast type
//!   strings are mapped to PHP types (e.g. `datetime` → `\Carbon\Carbon`,
//!   `boolean` → `bool`, `decimal:2` → `float`).  Custom cast classes
//!   are resolved by loading the class and inspecting the `get()`
//!   method's return type.  When the `get()` method has no return type,
//!   the resolver falls back to the first generic argument from an
//!   `@implements CastsAttributes<TGet, TSet>` annotation on the cast
//!   class.  Enum casts resolve to the enum class itself.  Classes
//!   implementing `Castable` also resolve to themselves.  A `:argument`
//!   suffix (e.g. `Address::class.':nullable'`) is stripped before
//!   resolution.
//!
//! - **Attribute default properties.** Entries in the `$attributes`
//!   property array produce typed virtual properties as a fallback.
//!   Types are inferred from the literal default values: strings,
//!   booleans, integers, floats, `null`, and arrays.  Columns that
//!   already have a `$casts` entry are skipped, so casts always take
//!   priority.
//!
//! - **Column name properties.** Column names from `$fillable`,
//!   `$guarded`, and `$hidden` produce `mixed`-typed virtual
//!   properties as a last-resort fallback.  Columns already covered
//!   by `$casts` or `$attributes` are skipped.

mod accessors;
mod builder;
mod casts;
mod factory;
mod helpers;
mod relationships;
mod scopes;

pub use helpers::extends_eloquent_model;
pub(crate) use helpers::{accessor_method_candidates, camel_to_snake};

pub(crate) use accessors::is_accessor_method;
use accessors::{
    extract_modern_accessor_type, is_legacy_accessor, is_modern_accessor,
    legacy_accessor_property_name,
};

pub(crate) use relationships::count_property_to_relationship_method;
pub use relationships::infer_relationship_from_body;
use relationships::{
    RelationshipKind, build_property_type, classify_relationship, count_property_name,
    extract_related_type,
};

pub use scopes::build_scope_methods_for_builder;
use scopes::{build_scope_methods, is_scope_method};

use builder::build_builder_forwarded_methods;
use casts::cast_type_to_php_type;
pub use factory::LaravelFactoryProvider;
pub(crate) use factory::{factory_to_model_fqn, model_to_factory_fqn};

use crate::types::{ClassInfo, PropertyInfo};

use super::{ResolvedClassCache, VirtualMemberProvider, VirtualMembers};

/// The fully-qualified name of the Eloquent base model.
pub(crate) const ELOQUENT_MODEL_FQN: &str = "Illuminate\\Database\\Eloquent\\Model";

/// The fully-qualified name of the Eloquent Builder class.
pub const ELOQUENT_BUILDER_FQN: &str = "Illuminate\\Database\\Eloquent\\Builder";

// ─── Type-resolution helpers ────────────────────────────────────────────────
//
// Called from `completion/resolver.rs` (`type_hint_to_classes_depth`) to
// apply Eloquent-specific post-processing after a class has been resolved
// and generic substitution applied.  Keeping the framework logic here
// rather than inline in the generic resolver avoids coupling the type
// engine to Laravel conventions.

/// Swap a resolved Eloquent Collection to a model's custom collection.
///
/// When the resolved class is `Illuminate\Database\Eloquent\Collection`
/// and one of the generic type arguments is a model with a
/// `custom_collection` declared (via `#[CollectedBy]` or
/// `@use HasCollection<X>`), returns the custom collection class
/// instead.  This handles the common chain pattern:
///
/// ```php
/// Model::where(...)->get()  // returns Collection<int, TModel>
/// ```
///
/// where `TModel` has been substituted to the concrete model and the
/// model declares a custom collection like `ProductCollection`.
///
/// Returns `None` when the class is not the Eloquent Collection, has no
/// generic args, or the model does not declare a custom collection.
pub(crate) fn try_swap_custom_collection(
    cls: ClassInfo,
    base_fqn: &str,
    generic_args: &[&str],
    all_classes: &[ClassInfo],
    class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
) -> ClassInfo {
    let bc = base_fqn.strip_prefix('\\').unwrap_or(base_fqn);
    if bc != crate::types::ELOQUENT_COLLECTION_FQN || generic_args.is_empty() {
        return cls;
    }

    // The last generic arg is typically the model type.
    let model_arg = generic_args.last().unwrap();
    let model_clean = model_arg.strip_prefix('\\').unwrap_or(model_arg);
    let model_class = find_class_in(all_classes, model_clean)
        .cloned()
        .or_else(|| class_loader(model_clean));

    if let Some(ref mc) = model_class
        && let Some(coll_name) = mc.laravel().and_then(|l| l.custom_collection.as_ref())
    {
        let coll_clean = coll_name.strip_prefix('\\').unwrap_or(coll_name);
        find_class_in(all_classes, coll_clean)
            .cloned()
            .or_else(|| class_loader(coll_clean))
            .unwrap_or(cls)
    } else {
        cls
    }
}

/// Inject scope methods and model virtual methods onto a resolved Builder.
///
/// When the resolved class is the Eloquent Builder and the first generic
/// argument is a concrete model name, injects:
///
/// 1. **Scope methods** — `scopeX` and `#[Scope]` methods from the model,
///    with the `scope` prefix stripped and the first `$query` parameter
///    removed.
///
/// 2. **Model `@method` tags** — virtual methods declared via `@method`
///    on the model or its traits (e.g. `SoftDeletes`'s `withTrashed`).
///    Laravel's `Builder::__call` forwards unknown calls to the model,
///    so these methods are effectively available on the Builder instance.
///    Return types containing `static` are remapped to
///    `Builder<ConcreteModel>` to keep the chain on the builder.
///
/// The `cls` parameter is the Builder **after** generic substitution has
/// been applied.  `raw_cls` is the pre-substitution class (needed to
/// check the FQN via `file_namespace`).
pub(crate) fn try_inject_builder_scopes(
    result: &mut ClassInfo,
    raw_cls: &ClassInfo,
    base_fqn: &str,
    generic_args: &[&str],
    class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
) {
    if !is_eloquent_builder_fqn(base_fqn, raw_cls) || generic_args.is_empty() {
        return;
    }

    // The first (or only) generic arg is the model type.
    let model_arg = generic_args.first().unwrap();
    let model_clean = model_arg.strip_prefix('\\').unwrap_or(model_arg);

    // 1. Inject scope methods.
    let scope_methods = build_scope_methods_for_builder(model_clean, class_loader);
    for method in scope_methods {
        if !result
            .methods
            .iter()
            .any(|m| m.name == method.name && m.is_static == method.is_static)
        {
            result.methods.push(method);
        }
    }

    // 2. Inject @method virtual methods from the model.
    inject_model_virtual_methods(result, model_clean, class_loader);
}

/// Inject `@method`-declared virtual methods from a model onto a Builder.
///
/// Laravel's `Builder::__call()` forwards unknown method calls to the
/// model instance.  This means `@method` tags on the model (including
/// those inherited from traits like `SoftDeletes`) are callable on the
/// Builder.  For example:
///
/// ```php
/// // SoftDeletes declares: @method static Builder<static> withTrashed()
/// // Customer uses SoftDeletes
/// Customer::groupBy('email')->withTrashed()->first()
/// //                          ^^^^^^^^^^^^^ needs to resolve on Builder<Customer>
/// ```
///
/// This function loads the fully-resolved model, finds virtual methods
/// (those with `is_virtual = true`, which come from `@method` tags),
/// and injects them as **instance** methods on the Builder.  Return
/// types containing `static`, `self`, or `$this` are substituted with
/// `Builder<ConcreteModel>` so the chain continues on the builder.
fn inject_model_virtual_methods(
    builder: &mut ClassInfo,
    model_name: &str,
    class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
) {
    use std::collections::HashMap;

    use crate::inheritance::apply_substitution;

    let model_class = match class_loader(model_name) {
        Some(c) => c,
        None => return,
    };

    if !extends_eloquent_model(&model_class, class_loader) {
        return;
    }

    // Resolve the model fully so that @method tags from traits and
    // parent classes are included.
    let resolved_model = crate::virtual_members::resolve_class_fully(&model_class, class_loader);

    // Build a substitution map: `static`/`self`/`$this` in return
    // types should become the concrete model name.  The `@method`
    // tags already declare the full return type (e.g.
    // `Builder<static>`), so substituting `static` → model name
    // produces `Builder<Customer>`.  Using `Builder<Model>` here
    // would double-wrap to `Builder<Builder<Customer>>`.
    let model_fqn = format!("\\{model_name}");
    let mut subs = HashMap::new();
    subs.insert("static".to_string(), model_fqn.clone());
    subs.insert("$this".to_string(), model_fqn.clone());
    subs.insert("self".to_string(), model_fqn);

    for method in &resolved_model.methods {
        // Only inject virtual methods (from @method tags).  Real
        // methods on the model are not forwarded through Builder.
        if !method.is_virtual {
            continue;
        }

        // Skip methods already present on the builder (real methods,
        // scope methods, or previously injected methods).
        if builder
            .methods
            .iter()
            .any(|m| m.name.eq_ignore_ascii_case(&method.name))
        {
            continue;
        }

        // Clone and convert to an instance method on the builder.
        let mut forwarded = method.clone();
        forwarded.is_static = false;

        // Substitute self-referencing return types.
        if let Some(ref mut ret) = forwarded.return_type {
            *ret = apply_substitution(ret, &subs);
        }

        builder.methods.push(forwarded);
    }
}

/// Check whether a base FQN and/or a `ClassInfo` refer to the Eloquent Builder.
///
/// Handles the three forms a Builder can appear as:
/// 1. The type hint FQN itself (e.g. from `@return Builder<User>`).
/// 2. The `ClassInfo.name` field (short name or FQN depending on source).
/// 3. The FQN constructed from `file_namespace + name` (PSR-4 loaded classes
///    where `name` is the short name only).
fn is_eloquent_builder_fqn(base_fqn: &str, cls: &ClassInfo) -> bool {
    let bc = base_fqn.strip_prefix('\\').unwrap_or(base_fqn);
    let cn = cls.name.strip_prefix('\\').unwrap_or(&cls.name);
    let fqn_from_ns = cls
        .file_namespace
        .as_ref()
        .map(|ns| format!("{ns}\\{}", cls.name));
    let fqn_clean = fqn_from_ns
        .as_deref()
        .map(|f| f.strip_prefix('\\').unwrap_or(f));
    bc == ELOQUENT_BUILDER_FQN
        || cn == ELOQUENT_BUILDER_FQN
        || fqn_clean == Some(ELOQUENT_BUILDER_FQN)
}

/// Find a class in a slice by name (short or FQN).
///
/// Minimal local lookup used by the collection-swap helper.  Prefers
/// namespace-aware matching when the name contains backslashes.
fn find_class_in<'a>(all_classes: &'a [ClassInfo], name: &str) -> Option<&'a ClassInfo> {
    let clean = name.strip_prefix('\\').unwrap_or(name);
    let short = clean.rsplit('\\').next().unwrap_or(clean);

    if clean.contains('\\') {
        let expected_ns = clean.rsplit_once('\\').map(|(ns, _)| ns);
        all_classes
            .iter()
            .find(|c| c.name == short && c.file_namespace.as_deref() == expected_ns)
    } else {
        all_classes.iter().find(|c| c.name == short)
    }
}

/// Virtual member provider for Laravel Eloquent models.
///
/// When a class extends `Illuminate\Database\Eloquent\Model` (directly
/// or through an intermediate parent), this provider scans its methods
/// for Eloquent relationship return types and synthesizes virtual
/// properties for each one.
///
/// For example, a method `posts()` returning `HasMany<Post, $this>`
/// produces a virtual property `$posts` with type
/// `\Illuminate\Database\Eloquent\Collection<Post>`.
pub struct LaravelModelProvider;

impl VirtualMemberProvider for LaravelModelProvider {
    /// Returns `true` if the class extends `Illuminate\Database\Eloquent\Model`.
    fn applies_to(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> bool {
        extends_eloquent_model(class, class_loader)
    }

    /// Scan the class's methods for Eloquent relationship return types,
    /// scope methods, Builder-as-static forwarded methods, `$casts`
    /// definitions, `$attributes` defaults, and `$fillable`/`$guarded`/
    /// `$hidden` column names.
    fn provide(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        cache: Option<&ResolvedClassCache>,
    ) -> VirtualMembers {
        let mut properties = Vec::new();
        let mut methods = Vec::new();
        let mut seen_props: std::collections::HashSet<String> = std::collections::HashSet::new();

        // ── Cast properties ─────────────────────────────────────────
        if let Some(laravel) = class.laravel() {
            for (column, cast_type) in &laravel.casts_definitions {
                let php_type = cast_type_to_php_type(cast_type, class_loader);
                seen_props.insert(column.clone());
                properties.push(PropertyInfo::virtual_property(column, Some(&php_type)));
            }

            // ── Attribute default properties (fallback) ─────────────
            // Only add properties for columns not already covered by $casts.
            for (column, php_type) in &laravel.attributes_definitions {
                if !seen_props.insert(column.clone()) {
                    continue;
                }
                properties.push(PropertyInfo::virtual_property(column, Some(php_type)));
            }

            // ── Column name properties (last-resort fallback) ───────
            // $fillable, $guarded, and $hidden provide column names without
            // type information.  Only add for columns not already covered.
            for column in &laravel.column_names {
                if !seen_props.insert(column.clone()) {
                    continue;
                }
                properties.push(PropertyInfo::virtual_property(column, Some("mixed")));
            }
        }

        for method in &class.methods {
            // ── Scope methods ───────────────────────────────────────
            if is_scope_method(method) {
                // Skip `#[Scope]`-attributed methods that also use
                // the `scopeX` prefix — the attribute takes priority
                // and the name is used as-is (no prefix stripping).
                let [instance_method, static_method] = build_scope_methods(method);
                methods.push(instance_method);
                methods.push(static_method);
                continue;
            }

            // ── Legacy accessors (getXAttribute) ────────────────────
            if is_legacy_accessor(method) {
                let prop_name = legacy_accessor_property_name(&method.name);
                properties.push(PropertyInfo {
                    deprecation_message: method.deprecation_message.clone(),
                    ..PropertyInfo::virtual_property(&prop_name, method.return_type.as_deref())
                });
                continue;
            }

            // ── Modern accessors (Laravel 9+ Attribute casts) ───────
            if is_modern_accessor(method) {
                let prop_name = camel_to_snake(&method.name);
                let accessor_type = extract_modern_accessor_type(method);
                properties.push(PropertyInfo {
                    deprecation_message: method.deprecation_message.clone(),
                    ..PropertyInfo::virtual_property(&prop_name, Some(&accessor_type))
                });
                continue;
            }

            // ── Relationship properties ─────────────────────────────
            let return_type = match method.return_type.as_deref() {
                Some(rt) => rt,
                None => continue,
            };

            let kind = match classify_relationship(return_type) {
                Some(k) => k,
                None => continue,
            };

            let related_type = extract_related_type(return_type);

            // For collection relationships, use the *related* model's
            // custom_collection, not the owning model's.  For example,
            // if Product has `#[CollectedBy(ProductCollection)]` and
            // Review has `#[CollectedBy(ReviewCollection)]`, then
            // `Product::reviews()` returning `HasMany<Review, $this>`
            // should produce `ReviewCollection<Review>`, not
            // `ProductCollection<Review>`.
            let custom_collection = if kind == RelationshipKind::Collection {
                related_type
                    .as_deref()
                    .and_then(|rt| {
                        let clean = rt.strip_prefix('\\').unwrap_or(rt);
                        class_loader(clean)
                    })
                    .and_then(|related_class| {
                        related_class
                            .laravel
                            .and_then(|l| l.custom_collection.clone())
                    })
            } else {
                None
            };

            let type_hint =
                build_property_type(kind, related_type.as_deref(), custom_collection.as_deref());

            if let Some(ref th) = type_hint {
                properties.push(PropertyInfo::virtual_property(&method.name, Some(th)));
            }
        }

        // ── Relationship count properties (`*_count`) ───────────────
        // `withCount`/`loadCount` is one of the most common Eloquent
        // patterns.  For each relationship method, synthesize a
        // `{snake_name}_count` property typed as `int`.  Skip if a
        // property with that name already exists (e.g. from an explicit
        // `@property` tag).
        for method in &class.methods {
            let return_type = match method.return_type.as_deref() {
                Some(rt) => rt,
                None => continue,
            };
            if classify_relationship(return_type).is_none() {
                continue;
            }
            let count_name = count_property_name(&method.name);
            if !seen_props.insert(count_name.clone()) {
                continue;
            }
            properties.push(PropertyInfo::virtual_property(&count_name, Some("int")));
        }

        // ── Builder-as-static forwarding ────────────────────────────
        let forwarded = build_builder_forwarded_methods(class, class_loader, cache);
        methods.extend(forwarded);

        VirtualMembers {
            methods,
            properties,
            constants: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests;
