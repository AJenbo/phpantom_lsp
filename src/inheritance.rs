use std::collections::{HashMap, HashSet};
/// Base class inheritance resolution.
///
/// This module handles merging members from parent classes and traits
/// into a single `ClassInfo`.  The resulting merged class contains the
/// base set of members visible on an instance / static access,
/// respecting PHP's precedence rules:
///
///   class own > traits > parent chain
///
/// `@mixin` members are handled separately by
/// [`PHPDocProvider`](crate::virtual_members::phpdoc::PHPDocProvider) in
/// the virtual member provider layer.
///
/// This module also supports **generic type substitution**: when a child
/// class declares `@extends Parent<ConcreteType1, ConcreteType2>` and the
/// parent has `@template T1` / `@template T2`, the inherited methods and
/// properties have their template parameter references replaced with the
/// concrete types.
use std::sync::Arc;

/// Bundles the trait-level configuration passed through
/// [`merge_traits_into`] so the function stays within clippy's
/// argument-count limit.
use std::borrow::Cow;

/// A borrow-or-owned handle to a `ClassInfo`, used to walk the parent
/// chain in [`resolve_class_with_inheritance`] without cloning the root
/// class.
///
/// The first iteration borrows the caller-provided `&ClassInfo` (zero
/// allocation).  Subsequent iterations hold the `Arc<ClassInfo>` returned
/// by the class loader (a cheap Arc move).
pub(crate) enum ClassRef<'a> {
    Borrowed(&'a ClassInfo),
    Owned(Arc<ClassInfo>),
}

impl std::ops::Deref for ClassRef<'_> {
    type Target = ClassInfo;
    #[inline]
    fn deref(&self) -> &ClassInfo {
        match self {
            ClassRef::Borrowed(r) => r,
            ClassRef::Owned(a) => a,
        }
    }
}

pub(crate) struct TraitContext<'a> {
    /// Generic type arguments for `@use Trait<Type>` declarations.
    pub use_generics: &'a [(String, Vec<PhpType>)],
    /// `insteadof` precedence declarations.
    pub precedences: &'a [TraitPrecedence],
    /// `as` alias declarations.
    pub aliases: &'a [TraitAlias],
}

/// Tracks member names already present during inheritance merging.
///
/// Passed through `resolve_class_with_inheritance` and `merge_traits_into`
/// (including recursive calls) so that every addition is checked in O(1)
/// instead of scanning the full member vectors.
pub(crate) struct MergeDedup {
    /// Method names already merged.
    pub methods: HashSet<String>,
    /// Property names already merged.
    pub properties: HashSet<String>,
    /// Constant names already merged.
    pub constants: HashSet<String>,
}

impl MergeDedup {
    /// Build from the members already present on a `ClassInfo`.
    fn from_class(class: &ClassInfo) -> Self {
        Self {
            methods: class.methods.iter().map(|m| m.name.clone()).collect(),
            properties: class.properties.iter().map(|p| p.name.clone()).collect(),
            constants: class.constants.iter().map(|c| c.name.clone()).collect(),
        }
    }
}

use crate::php_type::PhpType;
use crate::types::{
    ClassInfo, MAX_INHERITANCE_DEPTH, MAX_TRAIT_DEPTH, MethodInfo, ParameterInfo, PropertyInfo,
    TraitAlias, TraitPrecedence, Visibility,
};
use crate::util::short_name;
use crate::virtual_members::laravel::{
    extends_eloquent_model, factory_to_model_fqn, model_to_factory_fqn,
};

// ─── Docblock Enrichment ────────────────────────────────────────────────────

/// Whether a child's effective type equals its native type, meaning no
/// docblock override was applied.
///
/// Returns `true` when the child wrote no `@return` / `@var` / `@param`
/// tag (so the effective type is just the native hint).  Returns `false`
/// when the child provided its own docblock type — in that case the
/// child's type is an intentional override and should not be replaced.
fn lacks_docblock_override(effective: &Option<PhpType>, native: &Option<PhpType>) -> bool {
    match (effective, native) {
        // No effective type at all — nothing to override.
        (None, _) => true,
        // Effective type present but no native type — the child wrote
        // a docblock-only type (e.g. `@return list<Pen>` with no native
        // hint).  That is an intentional override.
        (Some(_), None) => false,
        // Both present — if they are equivalent, the child didn't write
        // a docblock (the effective type is just the native hint echoed).
        (Some(eff), Some(nat)) => eff.equivalent(nat),
    }
}

/// Whether an ancestor's type is richer than the child's native type.
///
/// Returns `true` when the ancestor has an effective type that differs
/// from its own native type (meaning the ancestor wrote a docblock).
fn ancestor_has_richer_type(effective: &Option<PhpType>, native: &Option<PhpType>) -> bool {
    match (effective, native) {
        // Ancestor has an effective type but no native type — it came
        // from a docblock (e.g. interface method with `@return list<Pen>`
        // and no native hint).
        (Some(_), None) => true,
        // Both present — richer if they differ (docblock overrides native).
        (Some(eff), Some(nat)) => !eff.equivalent(nat),
        // No effective type — nothing richer to offer.
        _ => false,
    }
}

/// Enrich a child method with docblock information from an ancestor method.
///
/// Propagates return types, parameter types, descriptions, template
/// parameters, conditional return types, and type assertions from the
/// ancestor when the child lacks its own docblock overrides.
///
/// **Return type rule:** If the child's `return_type` equals its
/// `native_return_type` (no docblock), and the ancestor's `return_type`
/// differs from its `native_return_type` (has docblock), copy the
/// ancestor's `return_type` to the child.  If the child has no
/// `return_type` at all, always inherit the ancestor's.
///
/// **Parameter rule:** Match by position (not by name, since the child
/// may rename parameters).  Same effective-vs-native comparison as
/// return types.
///
/// **Description rule:** Inherit `description` and `return_description`
/// when the child has `None`.
pub(crate) fn enrich_method_from_ancestor(existing: &mut MethodInfo, ancestor: &MethodInfo) {
    // ── Return type ─────────────────────────────────────────────
    // Propagate when (a) the child has no return type at all, or
    // (b) the child's effective type equals its native type (no
    // docblock override) and the ancestor has a richer docblock type.
    if existing.return_type.is_none() && ancestor.return_type.is_some()
        || lacks_docblock_override(&existing.return_type, &existing.native_return_type)
            && ancestor_has_richer_type(&ancestor.return_type, &ancestor.native_return_type)
    {
        existing.return_type = ancestor.return_type.clone();
    }

    // ── Template parameters ─────────────────────────────────────
    if existing.template_params.is_empty() && !ancestor.template_params.is_empty() {
        existing.template_params = ancestor.template_params.clone();
        existing.template_param_bounds = ancestor.template_param_bounds.clone();
        existing.template_bindings = ancestor.template_bindings.clone();
        // Template return types like `T` only make sense when the
        // template params are present — inherit the return type too
        // if we haven't already set it.
        if existing.return_type.is_none() {
            existing.return_type = ancestor.return_type.clone();
        }
    }

    // ── Conditional return type ─────────────────────────────────
    if existing.conditional_return.is_none() && ancestor.conditional_return.is_some() {
        existing.conditional_return = ancestor.conditional_return.clone();
    }

    // ── Type assertions ─────────────────────────────────────────
    if existing.type_assertions.is_empty() && !ancestor.type_assertions.is_empty() {
        existing.type_assertions = ancestor.type_assertions.clone();
    }

    // ── Parameters (by position) ────────────────────────────────
    enrich_parameters_from_ancestor(&mut existing.parameters, &ancestor.parameters);

    // ── Descriptions ────────────────────────────────────────────
    if existing.description.is_none() && ancestor.description.is_some() {
        existing.description = ancestor.description.clone();
    }
    if existing.return_description.is_none() && ancestor.return_description.is_some() {
        existing.return_description = ancestor.return_description.clone();
    }
}

/// Enrich child parameters from ancestor parameters, matched by position.
///
/// When a child parameter's `type_hint` equals its `native_type_hint`
/// (no docblock override) and the ancestor parameter has a richer type,
/// copy the ancestor's `type_hint`.  Also inherit `description` when
/// the child parameter has none.
fn enrich_parameters_from_ancestor(
    existing_params: &mut [ParameterInfo],
    ancestor_params: &[ParameterInfo],
) {
    for (existing_param, ancestor_param) in existing_params.iter_mut().zip(ancestor_params) {
        // Type hint enrichment
        if lacks_docblock_override(&existing_param.type_hint, &existing_param.native_type_hint)
            && ancestor_param.type_hint.is_some()
        {
            existing_param.type_hint = ancestor_param.type_hint.clone();
        }
        // Description enrichment
        if existing_param.description.is_none() && ancestor_param.description.is_some() {
            existing_param.description = ancestor_param.description.clone();
        }
    }
}

/// Enrich a child property with docblock information from an ancestor
/// property.
///
/// Propagates type hints and descriptions from the ancestor when the
/// child lacks its own docblock overrides.  The same
/// effective-vs-native comparison is used as for method return types.
pub(crate) fn enrich_property_from_ancestor(existing: &mut PropertyInfo, ancestor: &PropertyInfo) {
    // ── Type hint ───────────────────────────────────────────────
    // Same logic as method return types: propagate when the child
    // has no type or has only the native hint without a docblock
    // override, and the ancestor provides a richer type.
    if existing.type_hint.is_none() && ancestor.type_hint.is_some()
        || lacks_docblock_override(&existing.type_hint, &existing.native_type_hint)
            && ancestor_has_richer_type(&ancestor.type_hint, &ancestor.native_type_hint)
    {
        existing.type_hint = ancestor.type_hint.clone();
    }

    // ── Description ─────────────────────────────────────────────
    if existing.description.is_none() && ancestor.description.is_some() {
        existing.description = ancestor.description.clone();
    }
}

/// Resolve a class together with all inherited members from its parent
/// chain.
///
/// Walks up the `extends` chain via `class_loader`, collecting public and
/// protected methods, properties, and constants from each ancestor.
/// If a child already defines a member with the same name as a parent
/// member, the child's version wins (even if the signatures differ).
///
/// Private members are never inherited.
///
/// When the child declares `@extends Parent<Type1, Type2>` and the parent
/// has `@template` parameters, the inherited members have their template
/// parameter types replaced with the concrete types from the `@extends`
/// annotation.  This substitution chains through the entire ancestry.
///
/// A depth limit of 20 prevents infinite loops from circular inheritance.
pub(crate) fn resolve_class_with_inheritance(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> ClassInfo {
    let mut merged = class.clone();

    // Build dedup sets from the class's own members.  These are passed
    // through trait merging and the parent chain walk so that every
    // addition is tracked in O(1) across all recursion levels.
    let mut dedup = MergeDedup::from_class(&merged);

    // 1. Merge traits used by this class.
    //    PHP precedence: class methods > trait methods > inherited methods.
    //    Since `merged` already contains the class's own members, we only
    //    add trait members that don't collide with existing ones.
    merge_traits_into(
        &mut merged,
        &class.used_traits,
        &TraitContext {
            use_generics: &class.use_generics,
            precedences: &class.trait_precedences,
            aliases: &class.trait_aliases,
        },
        class_loader,
        0,
        &mut dedup,
    );

    // 2. Walk up the `extends` chain and merge parent members.
    //
    // `current` holds a reference to the class whose `parent_class`,
    // `extends_generics`, `used_traits`, etc. we read at each level.
    // For the first iteration this is the root `class` (a borrow —
    // zero allocation).  After that it becomes the `Arc<ClassInfo>`
    // returned by `class_loader` (a cheap Arc move).
    let mut current: ClassRef<'_> = ClassRef::Borrowed(class);
    let mut depth = 0;

    // The substitution map accumulates as we walk the chain.
    // It maps template parameter names → concrete types, and is
    // re-computed at each level based on the `@extends` generics
    // of the current class and the `@template` params of the parent.
    let mut active_subs: HashMap<String, PhpType> = HashMap::new();

    // Seed the initial substitution map from the root class's
    // `@extends` generics.  If the root class has
    // `@extends Collection<int, Language>`, this will be applied
    // when we load `Collection` as the first parent.
    //
    // We don't apply it yet — it's matched against the parent's
    // template_params in the loop below.

    while let Some(ref parent_name) = current.parent_class {
        depth += 1;
        if depth > MAX_INHERITANCE_DEPTH {
            break;
        }

        let parent = if let Some(p) = class_loader(parent_name) {
            p
        } else {
            break;
        };

        // Build the substitution map for this parent level.
        //
        // Look through current's `extends_generics` for an entry
        // whose class name matches this parent, and zip its type
        // arguments with the parent's `template_params`.
        let mut level_subs = build_substitution_map(&current, &parent, &active_subs);

        // ── Convention-based Factory fallback ────────────────────
        // When a factory class extends `Factory` without
        // `@extends Factory<Model>`, derive the model class from
        // the naming convention (e.g. `Database\Factories\UserFactory`
        // → `App\Models\User`) and substitute `TModel` automatically.
        if level_subs.is_empty()
            && !parent.template_params.is_empty()
            && is_factory_class(parent_name)
        {
            let factory_fqn = match &current.file_namespace {
                Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, current.name),
                _ => current.name.clone(),
            };
            if let Some(model_fqn) = factory_to_model_fqn(&factory_fqn)
                && class_loader(&model_fqn).is_some()
            {
                for param in &parent.template_params {
                    level_subs.insert(param.clone(), PhpType::parse(&model_fqn));
                }
            }
        }

        // Merge traits used by the parent class as well, so that
        // grandparent-level trait members are visible.
        // Apply the current level's template substitutions to the
        // parent's `@use` generics.  Without this, a chain like:
        //
        //   /** @extends DataCollection<int, DeliveryOption> */
        //   class DeliveryOptionCollection extends DataCollection
        //
        // where DataCollection has:
        //   /** @use EnumerableMethods<TKey, TValue> */
        //
        // would pass the raw `TKey`/`TValue` template params to the
        // trait instead of the concrete `int`/`DeliveryOption` types.
        let substituted_use_generics: Vec<(String, Vec<PhpType>)> = if level_subs.is_empty() {
            parent.use_generics.clone()
        } else {
            parent
                .use_generics
                .iter()
                .map(|(name, args)| {
                    let substituted_args: Vec<PhpType> =
                        args.iter().map(|arg| arg.substitute(&level_subs)).collect();
                    (name.clone(), substituted_args)
                })
                .collect()
        };

        merge_traits_into(
            &mut merged,
            &parent.used_traits,
            &TraitContext {
                use_generics: &substituted_use_generics,
                precedences: &parent.trait_precedences,
                aliases: &parent.trait_aliases,
            },
            class_loader,
            0,
            &mut dedup,
        );

        // Merge parent methods — skip private.
        // When the child already has a method with the same name,
        // enrich it with the parent's richer docblock types instead
        // of silently discarding the parent's type information.
        for method in &parent.methods {
            if method.visibility == Visibility::Private {
                continue;
            }
            let mut ancestor_method = method.clone();
            if !level_subs.is_empty() {
                apply_substitution_to_method(&mut ancestor_method, &level_subs);
            }
            if !dedup.methods.insert(method.name.clone()) {
                // Child already has this method — enrich it from parent.
                if let Some(existing) = merged
                    .methods
                    .make_mut()
                    .iter_mut()
                    .find(|m| m.name == method.name)
                {
                    enrich_method_from_ancestor(existing, &ancestor_method);
                }
                continue;
            }
            merged.methods.push(ancestor_method);
        }

        // Merge parent properties — same enrichment logic.
        for property in &parent.properties {
            if property.visibility == Visibility::Private {
                continue;
            }
            let mut ancestor_property = property.clone();
            if !level_subs.is_empty() {
                apply_substitution_to_property(&mut ancestor_property, &level_subs);
            }
            if !dedup.properties.insert(property.name.clone()) {
                // Child already has this property — enrich it from parent.
                if let Some(existing) = merged
                    .properties
                    .make_mut()
                    .iter_mut()
                    .find(|p| p.name == property.name)
                {
                    enrich_property_from_ancestor(existing, &ancestor_property);
                }
                continue;
            }
            merged.properties.push(ancestor_property);
        }

        // Merge parent constants
        for constant in &parent.constants {
            if constant.visibility == Visibility::Private {
                continue;
            }
            if !dedup.constants.insert(constant.name.clone()) {
                continue;
            }
            merged.constants.push(constant.clone());
        }

        // Carry the substitution map forward for the next level.
        // If `Collection` extends `AbstractCollection<TKey, TValue>`,
        // we need to apply the current substitutions to those type
        // arguments so that `TKey` → `int` flows through.
        active_subs = level_subs;
        current = ClassRef::Owned(parent);
    }

    merged
}

/// Look up a method's return type through the inheritance chain.
///
/// Resolves inheritance for `class`, finds the method named
/// `method_name`, and returns its `return_type`.  This is a
/// convenience wrapper around [`resolve_class_fully`](crate::virtual_members::resolve_class_fully)
/// that eliminates the repeated merge → find → extract pattern
/// used across many modules.
///
/// Uses full resolution (base inheritance + virtual member providers)
/// so that virtual methods from `@method` tags, `@mixin` classes,
/// and framework providers are included.
pub(crate) fn resolve_method_return_type(
    class: &ClassInfo,
    method_name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    let merged = crate::virtual_members::resolve_class_fully(class, class_loader);
    merged
        .methods
        .iter()
        .find(|m| m.name == method_name)
        .and_then(|m| m.return_type.clone())
}

/// Look up a property's type hint through the inheritance chain.
///
/// Resolves inheritance for `class`, finds the property named
/// `prop_name`, and returns its `type_hint`.  This is a
/// convenience wrapper around [`resolve_class_fully`](crate::virtual_members::resolve_class_fully)
/// that eliminates the repeated merge → find → extract pattern
/// used across many modules.
///
/// Uses full resolution (base inheritance + virtual member providers)
/// so that virtual properties from `@property` tags, `@mixin` classes,
/// and framework providers are included.
pub(crate) fn resolve_property_type_hint(
    class: &ClassInfo,
    prop_name: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    let merged = crate::virtual_members::resolve_class_fully(class, class_loader);
    merged
        .properties
        .iter()
        .find(|p| p.name == prop_name)
        .and_then(|p| p.type_hint.clone())
}

/// Recursively merge members from the given traits into `merged`.
///
/// Traits can themselves `use` other traits (composition), so this
/// function recurses up to `MAX_TRAIT_DEPTH` levels.  Members that
/// already exist in `merged` (by name) are skipped — this naturally
/// implements the PHP precedence rule where the current class's own
/// members win over trait members, and earlier-listed traits win
/// over later ones.
///
/// Private trait members *are* merged (unlike parent class private
/// members), because PHP copies trait members into the using class
/// regardless of visibility.
///
/// When `use_generics` contains an entry for a trait (e.g.
/// `@use SomeTrait<ConcreteType>`) and the trait declares
/// `@template T`, the inherited methods and properties have their
/// template parameter types replaced with the concrete types.
fn merge_traits_into(
    merged: &mut ClassInfo,
    trait_names: &[String],
    ctx: &TraitContext<'_>,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    depth: u32,
    dedup: &mut MergeDedup,
) {
    if depth > MAX_TRAIT_DEPTH {
        return;
    }

    for trait_name in trait_names {
        let trait_info = if let Some(t) = class_loader(trait_name) {
            t
        } else {
            continue;
        };

        // Build a substitution map for this trait if the using class
        // declared `@use TraitName<Type1, Type2>` and the trait has
        // `@template` parameters.
        let mut trait_subs =
            build_trait_substitution_map(trait_name, &trait_info, ctx.use_generics);

        // ── Convention-based HasFactory fallback ─────────────────
        // When a model uses `HasFactory` without `@use HasFactory<X>`,
        // derive the factory class from the naming convention
        // (e.g. `App\Models\User` → `Database\Factories\UserFactory`)
        // and substitute `TFactory` automatically.
        if trait_subs.is_empty()
            && !trait_info.template_params.is_empty()
            && is_has_factory_trait(trait_name)
            && extends_eloquent_model(merged, class_loader)
        {
            let model_fqn = match &merged.file_namespace {
                Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, merged.name),
                _ => merged.name.clone(),
            };
            let factory_fqn = model_to_factory_fqn(&model_fqn);
            if class_loader(&factory_fqn).is_some() {
                for param in &trait_info.template_params {
                    trait_subs.insert(param.clone(), PhpType::parse(&factory_fqn));
                }
            }
        }

        // Recursively merge traits used by this trait (trait composition).
        // The sub-trait's own `@use` generics (from the trait's docblock)
        // apply, not the outer class's.
        if !trait_info.used_traits.is_empty() {
            merge_traits_into(
                merged,
                &trait_info.used_traits,
                &TraitContext {
                    use_generics: &trait_info.use_generics,
                    precedences: &trait_info.trait_precedences,
                    aliases: &trait_info.trait_aliases,
                },
                class_loader,
                depth + 1,
                dedup,
            );
        }

        // Walk the `parent_class` (extends) chain so that interface
        // inheritance is resolved.  For example, `BackedEnum extends
        // UnitEnum` — loading `BackedEnum` alone would miss `UnitEnum`'s
        // members (`cases()`, `$name`) unless we follow the chain here.
        // The same depth counter is shared to prevent infinite loops.
        let mut current = trait_info.clone();
        let mut parent_depth = depth;
        while let Some(ref parent_name) = current.parent_class {
            parent_depth += 1;
            if parent_depth > MAX_TRAIT_DEPTH {
                break;
            }
            let parent = if let Some(p) = class_loader(parent_name) {
                p
            } else {
                break;
            };

            // Also follow the parent's own used_traits.
            if !parent.used_traits.is_empty() {
                merge_traits_into(
                    merged,
                    &parent.used_traits,
                    &TraitContext {
                        use_generics: &parent.use_generics,
                        precedences: &parent.trait_precedences,
                        aliases: &parent.trait_aliases,
                    },
                    class_loader,
                    parent_depth + 1,
                    dedup,
                );
            }

            // Merge parent methods (skip private, skip duplicates)
            for method in &parent.methods {
                if method.visibility == Visibility::Private {
                    continue;
                }
                if !dedup.methods.insert(method.name.clone()) {
                    continue;
                }
                merged.methods.push(method.clone());
            }

            // Merge parent properties
            for property in &parent.properties {
                if property.visibility == Visibility::Private {
                    continue;
                }
                if !dedup.properties.insert(property.name.clone()) {
                    continue;
                }
                merged.properties.push(property.clone());
            }

            // Merge parent constants
            for constant in &parent.constants {
                if constant.visibility == Visibility::Private {
                    continue;
                }
                if !dedup.constants.insert(constant.name.clone()) {
                    continue;
                }
                merged.constants.push(constant.clone());
            }

            current = parent;
        }

        // Merge trait methods — skip if already present.
        // Apply generic substitution if a `@use` mapping exists.
        // Also skip methods excluded by `insteadof` declarations.
        for method in &trait_info.methods {
            // Check if this method from this trait is excluded by an
            // `insteadof` declaration.  For example, if the class has
            // `TraitA::method insteadof TraitB`, then when merging
            // TraitB's methods, `method` should be skipped.
            let excluded = ctx.precedences.iter().any(|p| {
                p.method_name == method.name
                    && p.insteadof.iter().any(|excluded_trait| {
                        excluded_trait == trait_name
                            || short_name(excluded_trait) == short_name(trait_name)
                    })
            });
            if excluded {
                continue;
            }

            if !dedup.methods.insert(method.name.clone()) {
                continue;
            }
            let mut method = method.clone();

            // Apply visibility-only `as` changes (no alias name).
            // For example, `TraitA::method as protected` changes the
            // visibility of `method` without creating an alias.
            for alias in ctx.aliases {
                if alias.method_name == method.name
                    && alias.alias.is_none()
                    && let Some(vis) = alias.visibility
                {
                    // Check trait name matches (if specified)
                    let name_matches = alias
                        .trait_name
                        .as_ref()
                        .is_none_or(|t| t == trait_name || short_name(t) == short_name(trait_name));
                    if name_matches {
                        method.visibility = vis;
                    }
                }
            }

            if !trait_subs.is_empty() {
                apply_substitution_to_method(&mut method, &trait_subs);
            }
            merged.methods.push(method);
        }

        // Merge trait properties — apply substitution.
        for property in &trait_info.properties {
            if !dedup.properties.insert(property.name.clone()) {
                continue;
            }
            let mut property = property.clone();
            if !trait_subs.is_empty() {
                apply_substitution_to_property(&mut property, &trait_subs);
            }
            merged.properties.push(property);
        }

        // Merge trait constants
        for constant in &trait_info.constants {
            if !dedup.constants.insert(constant.name.clone()) {
                continue;
            }
            merged.constants.push(constant.clone());
        }

        // Apply `as` alias declarations that create new method names.
        // For example, `TraitB::method as traitBMethod` creates a copy
        // of `method` accessible as `traitBMethod`.
        for alias in ctx.aliases {
            // Only process aliases that have a new name.
            let alias_name = match &alias.alias {
                Some(name) => name,
                None => continue,
            };

            // Check trait name matches (if specified).
            let name_matches = alias
                .trait_name
                .as_ref()
                .is_none_or(|t| t == trait_name || short_name(t) == short_name(trait_name));
            if !name_matches {
                continue;
            }

            // Find the source method in this trait.
            let source_method = trait_info
                .methods
                .iter()
                .find(|m| m.name == alias.method_name);
            let source_method = match source_method {
                Some(m) => m,
                None => continue,
            };

            // Skip if an alias with this name already exists.
            if !dedup.methods.insert(alias_name.clone()) {
                continue;
            }

            let mut aliased = source_method.clone();
            aliased.name = alias_name.clone();
            if let Some(vis) = alias.visibility {
                aliased.visibility = vis;
            }
            if !trait_subs.is_empty() {
                apply_substitution_to_method(&mut aliased, &trait_subs);
            }
            merged.methods.push(aliased);
        }
    }
}

// ─── Generic Type Substitution ──────────────────────────────────────────────

/// Check whether a trait name is the Laravel `HasFactory` trait.
///
/// Matches the FQN `Illuminate\Database\Eloquent\Factories\HasFactory`
/// as well as the short name `HasFactory` (common in same-file tests).
fn is_has_factory_trait(trait_name: &str) -> bool {
    trait_name == "Illuminate\\Database\\Eloquent\\Factories\\HasFactory"
        || trait_name == "HasFactory"
}

/// Check whether a parent class name is the Laravel
/// `Illuminate\Database\Eloquent\Factories\Factory` base class.
fn is_factory_class(class_name: &str) -> bool {
    class_name == "Illuminate\\Database\\Eloquent\\Factories\\Factory" || class_name == "Factory"
}

/// Build a substitution map for a trait based on `@use` generics and the
/// trait's `@template` parameters.
///
/// If the using class declares `@use HasFactory<UserFactory>` and the
/// trait `HasFactory` has `@template TFactory`, the returned map is
/// `{TFactory => UserFactory}`.
fn build_trait_substitution_map(
    trait_name: &str,
    trait_info: &ClassInfo,
    use_generics: &[(String, Vec<PhpType>)],
) -> HashMap<String, PhpType> {
    if trait_info.template_params.is_empty() || use_generics.is_empty() {
        return HashMap::new();
    }

    let trait_short = short_name(trait_name);

    // Find the @use entry that matches this trait.
    let type_args = use_generics
        .iter()
        .find(|(name, _)| {
            let name_short = short_name(name);
            name_short == trait_short
        })
        .map(|(_, args)| args);

    let type_args = match type_args {
        Some(args) => args,
        None => return HashMap::new(),
    };

    let mut map = HashMap::new();
    for (i, param_name) in trait_info.template_params.iter().enumerate() {
        if let Some(arg) = type_args.get(i) {
            map.insert(param_name.clone(), arg.clone());
        }
    }
    map
}

/// Build a substitution map for a parent class based on the child's
/// `@extends` generics and the parent's `@template` parameters.
///
/// If the child declares `@extends Collection<int, Language>` and the
/// parent `Collection` has `@template TKey` and `@template TValue`,
/// the returned map is `{TKey => int, TValue => Language}`.
///
/// When `active_subs` is non-empty (from a higher-level ancestor), the
/// type arguments are first resolved through those substitutions.  This
/// handles chained generics like:
///
/// ```text
/// class A { @template U }
/// class B extends A { @template T, @extends A<T> }
/// class C extends B { @extends B<Foo> }
/// ```
///
/// When resolving `C`: at level 1 (B), `active_subs` is empty and we
/// build `{T => Foo}`.  At level 2 (A), `current` is B whose
/// `@extends A<T>` gets the active substitution `{T => Foo}` applied,
/// yielding `{U => Foo}`.
fn build_substitution_map(
    current: &ClassInfo,
    parent: &ClassInfo,
    active_subs: &HashMap<String, PhpType>,
) -> HashMap<String, PhpType> {
    if parent.template_params.is_empty() {
        return active_subs.clone();
    }

    let parent_short = short_name(&parent.name);

    // Search `current.extends_generics` for an entry matching this parent.
    // Also check `implements_generics` for interface inheritance.
    let type_args = current
        .extends_generics
        .iter()
        .chain(current.implements_generics.iter())
        .find(|(name, _)| {
            let name_short = short_name(name);
            name_short == parent_short
        })
        .map(|(_, args)| args);

    let type_args = match type_args {
        Some(args) => args,
        None => {
            // No @extends/@implements generics for this parent.
            // Carry forward any active substitutions — they may still
            // apply if the parent's methods reference template params
            // from a grandchild.
            return active_subs.clone();
        }
    };

    let mut map = HashMap::new();

    for (i, param_name) in parent.template_params.iter().enumerate() {
        if let Some(arg) = type_args.get(i) {
            // Apply any active substitutions to the type argument.
            // This handles chaining: if arg is "T" and active_subs has
            // {T => Foo}, the result is {param_name => Foo}.
            let resolved = if active_subs.is_empty() {
                arg.clone()
            } else {
                arg.substitute(active_subs)
            };
            map.insert(param_name.clone(), resolved);
        }
    }

    map
}

/// Apply generic type substitution to a method's return type and parameter
/// type hints.
pub(crate) fn apply_substitution_to_method(
    method: &mut MethodInfo,
    subs: &HashMap<String, PhpType>,
) {
    if let Some(ref mut ret) = method.return_type {
        *ret = ret.substitute(subs);
    }
    if let Some(ref mut cond) = method.conditional_return {
        apply_substitution_to_conditional(cond, subs);
    }
    for param in &mut method.parameters {
        if let Some(ref mut hint) = param.type_hint {
            *hint = hint.substitute(subs);
        }
    }
}

/// Apply generic type substitution to a conditional return type tree.
///
/// Delegates to [`PhpType::substitute`] which recursively walks all
/// type variants (including nested conditionals) and replaces template
/// parameter names with their concrete types.
pub(crate) fn apply_substitution_to_conditional(
    cond: &mut PhpType,
    subs: &HashMap<String, PhpType>,
) {
    *cond = cond.substitute(subs);
}

/// Apply generic type substitution to a property's type hint.
pub(crate) fn apply_substitution_to_property(
    property: &mut PropertyInfo,
    subs: &HashMap<String, PhpType>,
) {
    if let Some(ref mut hint) = property.type_hint {
        *hint = hint.substitute(subs);
    }
}

/// Apply a substitution map to a type string.
///
/// Handles:
///   - Direct match: `"TValue"` → `"Language"`
///   - Nullable: `"?TValue"` → `"?Language"`
///   - Union types: `"TValue|null"` → `"Language|null"`
///   - Intersection types: `"TValue&Countable"` → `"Language&Countable"`
///   - Generic params: `"array<TKey, TValue>"` → `"array<int, Language>"`
///   - Nested generics: `"Collection<TKey, list<TValue>>"` →
///     `"Collection<int, list<Language>>"`
///   - Combinations: `"?Collection<TKey, TValue>|null"` → resolved correctly
///
/// Internally delegates to [`PhpType::substitute`] which walks the
/// parsed type tree.  This wrapper preserves the `&str → Cow<str>` API
/// so that existing callers do not need to change.
pub(crate) fn apply_substitution<'a>(
    type_str: &'a str,
    subs: &HashMap<String, PhpType>,
) -> Cow<'a, str> {
    let s = type_str.trim();
    if s.is_empty() || subs.is_empty() {
        return Cow::Borrowed(s);
    }

    // ── Early exit: if the type string doesn't contain any of the
    // substitution keys as a substring, no replacement can happen.
    // This skips the vast majority of type strings that don't reference
    // template parameters, avoiding all allocation and recursion.
    if !subs.keys().any(|key| s.contains(key.as_str())) {
        return Cow::Borrowed(s);
    }

    let parsed = PhpType::parse(s);
    let substituted = parsed.substitute(subs);
    let result = substituted.to_string();

    // If the result is identical to the input, return borrowed to
    // avoid unnecessary allocation in callers that check for changes.
    if result == s {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(result)
    }
}

/// Build a substitution map from a class's template parameters and
/// concrete type arguments.
///
/// Handles right-alignment when fewer arguments than template parameters
/// are provided (see [`apply_generic_args`] for details on the heuristic).
///
/// Returns an empty map when no substitutions can be made (e.g. when
/// `template_params` or `type_args` is empty).
pub(crate) fn build_generic_subs(
    class: &ClassInfo,
    type_args: &[&str],
) -> HashMap<String, PhpType> {
    if class.template_params.is_empty() || type_args.is_empty() {
        return HashMap::new();
    }

    // When fewer type arguments are provided than template parameters,
    // right-align the args so that trailing (value) params get bound
    // and leading key-like params stay unbound.  This handles the
    // common PHP pattern of writing `Collection<Model>` instead of
    // `Collection<int, Model>` — the single arg should bind to
    // `TValue`/`TModel`, not `TKey`.
    //
    // The heuristic only activates when every skipped leading param
    // has an `array-key` (or `int` / `string`) bound, which is the
    // universal convention for collection key parameters.
    let offset = if type_args.len() < class.template_params.len() {
        let skip = class.template_params.len() - type_args.len();
        let all_skipped_are_key_like = class.template_params[..skip].iter().all(|param| {
            class
                .template_param_bounds
                .get(param)
                .is_some_and(is_key_like_bound)
        });
        if all_skipped_are_key_like { skip } else { 0 }
    } else {
        0
    };

    let mut subs = HashMap::new();
    for (i, param_name) in class.template_params.iter().enumerate() {
        if i < offset {
            continue;
        }
        if let Some(arg) = type_args.get(i - offset) {
            subs.insert(param_name.clone(), PhpType::parse(arg));
        }
    }

    subs
}

/// Apply explicit generic type arguments to a class's members.
///
/// When a type hint includes generic parameters (e.g. `Collection<int, User>`),
/// this function maps them to the class's `@template` parameters and rewrites
/// all method return types, method parameter types, and property type hints
/// with the concrete types.
///
/// If the class has no `template_params` or no `type_args` are provided,
/// returns a clone of the class unchanged.
///
/// # Example
///
/// Given a `Collection` class with `@template TKey` and `@template TValue`,
/// calling `apply_generic_args(&collection_class, &["int", "User"])` will
/// substitute every occurrence of `TKey` with `int` and `TValue` with `User`
/// in the class's methods and properties.
pub(crate) fn apply_generic_args(class: &ClassInfo, type_args: &[&str]) -> ClassInfo {
    let subs = build_generic_subs(class, type_args);

    if subs.is_empty() {
        return class.clone();
    }

    let mut result = class.clone();
    for method in result.methods.make_mut() {
        apply_substitution_to_method(method, &subs);
    }
    for property in result.properties.make_mut() {
        apply_substitution_to_property(property, &subs);
    }

    // Substitute template params in generic annotations so that
    // downstream consumers (e.g. foreach element-type extraction)
    // see concrete types instead of raw template param names.
    // For example, `@implements IteratorAggregate<TKey, TValue>`
    // becomes `@implements IteratorAggregate<int, Customer>` when
    // TKey=int, TValue=Customer.
    apply_substitution_to_generics(&mut result.implements_generics, &subs);
    apply_substitution_to_generics(&mut result.extends_generics, &subs);
    apply_substitution_to_generics(&mut result.use_generics, &subs);

    result
}

/// Whether a template parameter bound represents a key-like type.
///
/// Returns `true` for `array-key`, `int`, `string`, and other types
/// that are conventionally used as collection key bounds.  This is
/// used by [`apply_generic_args`] to right-align generic arguments
/// when fewer arguments than template parameters are provided.
fn is_key_like_bound(bound: &PhpType) -> bool {
    match bound {
        PhpType::Named(n) => matches!(n.as_str(), "array-key" | "int" | "string"),
        PhpType::Union(members) => {
            // `int|string` is equivalent to `array-key`.
            !members.is_empty()
                && members.iter().all(
                    |m| matches!(m, PhpType::Named(n) if matches!(n.as_str(), "int" | "string")),
                )
        }
        _ => false,
    }
}

/// Apply a substitution map to a list of generic annotations.
///
/// Each entry is `(ClassName, [TypeArg1, TypeArg2, …])`.  Only the type
/// arguments are substituted; the class name is left unchanged.
fn apply_substitution_to_generics(
    generics: &mut [(String, Vec<PhpType>)],
    subs: &HashMap<String, PhpType>,
) {
    for (_class_name, type_args) in generics.iter_mut() {
        for arg in type_args.iter_mut() {
            let substituted = arg.substitute(subs);
            if substituted != *arg {
                *arg = substituted;
            }
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "inheritance_tests.rs"]
mod tests;
