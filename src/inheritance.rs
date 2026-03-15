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

pub(crate) struct TraitContext<'a> {
    /// Generic type arguments for `@use Trait<Type>` declarations.
    pub use_generics: &'a [(String, Vec<String>)],
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

use crate::docblock::types::{find_matching_close, split_generic_args};
use crate::types::{
    ClassInfo, ConditionalReturnType, MAX_INHERITANCE_DEPTH, MAX_TRAIT_DEPTH, MethodInfo,
    ParamCondition, PropertyInfo, TraitAlias, TraitPrecedence, Visibility,
};
use crate::util::short_name;
use crate::virtual_members::laravel::{
    extends_eloquent_model, factory_to_model_fqn, model_to_factory_fqn,
};

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
    let mut current = class.clone();
    let mut depth = 0;

    // The substitution map accumulates as we walk the chain.
    // It maps template parameter names → concrete types, and is
    // re-computed at each level based on the `@extends` generics
    // of the current class and the `@template` params of the parent.
    let mut active_subs: HashMap<String, String> = HashMap::new();

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
                    level_subs.insert(param.clone(), model_fqn.clone());
                }
            }
        }

        // Merge traits used by the parent class as well, so that
        // grandparent-level trait members are visible.
        merge_traits_into(
            &mut merged,
            &parent.used_traits,
            &TraitContext {
                use_generics: &parent.use_generics,
                precedences: &parent.trait_precedences,
                aliases: &parent.trait_aliases,
            },
            class_loader,
            0,
            &mut dedup,
        );

        // Merge parent methods — skip private, skip if child already has one with same name
        for method in &parent.methods {
            if method.visibility == Visibility::Private {
                continue;
            }
            if !dedup.methods.insert(method.name.clone()) {
                continue;
            }
            let mut method = method.clone();
            if !level_subs.is_empty() {
                apply_substitution_to_method(&mut method, &level_subs);
            }
            merged.methods.push(method);
        }

        // Merge parent properties
        for property in &parent.properties {
            if property.visibility == Visibility::Private {
                continue;
            }
            if !dedup.properties.insert(property.name.clone()) {
                continue;
            }
            let mut property = property.clone();
            if !level_subs.is_empty() {
                apply_substitution_to_property(&mut property, &level_subs);
            }
            merged.properties.push(property);
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
        current = Arc::unwrap_or_clone(parent);
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
) -> Option<String> {
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
) -> Option<String> {
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
                    trait_subs.insert(param.clone(), factory_fqn.clone());
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
    use_generics: &[(String, Vec<String>)],
) -> HashMap<String, String> {
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
    active_subs: &HashMap<String, String>,
) -> HashMap<String, String> {
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
                apply_substitution(arg, active_subs).into_owned()
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
    subs: &HashMap<String, String>,
) {
    if let Some(ref mut ret) = method.return_type {
        let substituted = apply_substitution(ret, subs);
        if substituted.as_ref() != ret.as_str() {
            *ret = substituted.into_owned();
        }
    }
    if let Some(ref mut cond) = method.conditional_return {
        apply_substitution_to_conditional(cond, subs);
    }
    for param in &mut method.parameters {
        if let Some(ref mut hint) = param.type_hint {
            let substituted = apply_substitution(hint, subs);
            if substituted.as_ref() != hint.as_str() {
                *hint = substituted.into_owned();
            }
        }
    }
}

/// Apply generic type substitution to a conditional return type tree.
///
/// Recursively walks the [`ConditionalReturnType`] and applies
/// [`apply_substitution`] to every concrete type string (both terminal
/// `Concrete` nodes and `IsType` condition strings).
pub(crate) fn apply_substitution_to_conditional(
    cond: &mut ConditionalReturnType,
    subs: &HashMap<String, String>,
) {
    match cond {
        ConditionalReturnType::Concrete(ty) => {
            let substituted = apply_substitution(ty, subs);
            if substituted.as_ref() != ty.as_str() {
                *ty = substituted.into_owned();
            }
        }
        ConditionalReturnType::Conditional {
            condition,
            then_type,
            else_type,
            ..
        } => {
            if let ParamCondition::IsType(type_str) = condition {
                let substituted = apply_substitution(type_str, subs);
                if substituted.as_ref() != type_str.as_str() {
                    *type_str = substituted.into_owned();
                }
            }
            apply_substitution_to_conditional(then_type, subs);
            apply_substitution_to_conditional(else_type, subs);
        }
    }
}

/// Apply generic type substitution to a property's type hint.
pub(crate) fn apply_substitution_to_property(
    property: &mut PropertyInfo,
    subs: &HashMap<String, String>,
) {
    if let Some(ref mut hint) = property.type_hint {
        let substituted = apply_substitution(hint, subs);
        if substituted.as_ref() != hint.as_str() {
            *hint = substituted.into_owned();
        }
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
pub(crate) fn apply_substitution<'a>(
    type_str: &'a str,
    subs: &HashMap<String, String>,
) -> Cow<'a, str> {
    let s = type_str.trim();
    if s.is_empty() {
        return Cow::Borrowed(s);
    }

    // ── Early exit: if the type string doesn't contain any of the
    // substitution keys as a substring, no replacement can happen.
    // This skips the vast majority of type strings that don't reference
    // template parameters, avoiding all allocation and recursion.
    if !subs.is_empty() && !subs.keys().any(|key| s.contains(key.as_str())) {
        return Cow::Borrowed(s);
    }

    // Handle nullable prefix.
    if let Some(inner) = s.strip_prefix('?') {
        let resolved = apply_substitution(inner, subs);
        return Cow::Owned(format!("?{resolved}"));
    }

    // Handle union types: split on `|` at depth 0.
    if let Some(parts) = split_at_depth_0(s, '|') {
        let resolved: Vec<Cow<str>> = parts.iter().map(|p| apply_substitution(p, subs)).collect();
        if resolved
            .iter()
            .zip(parts.iter())
            .all(|(r, &p)| matches!(r, Cow::Borrowed(b) if *b == p))
        {
            return Cow::Borrowed(s);
        }
        return Cow::Owned(
            resolved
                .iter()
                .map(|c| c.as_ref())
                .collect::<Vec<_>>()
                .join("|"),
        );
    }

    // Handle intersection types: split on `&` at depth 0.
    if let Some(parts) = split_at_depth_0(s, '&') {
        let resolved: Vec<Cow<str>> = parts.iter().map(|p| apply_substitution(p, subs)).collect();
        if resolved
            .iter()
            .zip(parts.iter())
            .all(|(r, &p)| matches!(r, Cow::Borrowed(b) if *b == p))
        {
            return Cow::Borrowed(s);
        }
        return Cow::Owned(
            resolved
                .iter()
                .map(|c| c.as_ref())
                .collect::<Vec<_>>()
                .join("&"),
        );
    }

    // Handle generic types: `Base<Arg1, Arg2>`.
    if let Some(angle_pos) = find_angle_at_depth_0(s) {
        let base = &s[..angle_pos];
        // Find the matching closing `>`.
        let rest = &s[angle_pos + 1..];
        let close_pos = find_matching_close(rest);
        let inner = &rest[..close_pos];
        let after = &rest[close_pos + 1..]; // anything after `>`

        // Resolve the base (it might itself be a template param).
        let resolved_base = apply_substitution(base, subs);

        // Split inner on commas at depth 0 and resolve each arg.
        let args = split_generic_args(inner);
        let resolved_args: Vec<Cow<str>> =
            args.iter().map(|a| apply_substitution(a, subs)).collect();

        let mut result = format!(
            "{resolved_base}<{}>",
            resolved_args
                .iter()
                .map(|c| c.as_ref())
                .collect::<Vec<_>>()
                .join(", ")
        );
        if !after.is_empty() {
            result.push_str(after);
        }
        return Cow::Owned(result);
    }

    // Handle callable/Closure signatures: `callable(TValue): RetType` or
    // `Closure(A, B): C`.  Recurse into the parameter types and the
    // return type so that template parameters are properly substituted.
    {
        let callable_prefix = if s.starts_with("callable(") {
            Some("callable")
        } else if s.starts_with("Closure(") {
            Some("Closure")
        } else {
            None
        };
        if let Some(prefix) = callable_prefix {
            let after_prefix = &s[prefix.len()..]; // starts with '('
            let inner = &after_prefix[1..]; // skip '('

            // Find the matching ')' respecting nesting.
            let mut depth = 1i32;
            let mut close_pos = None;
            for (i, c) in inner.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            close_pos = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(close) = close_pos {
                let params_str = &inner[..close];
                let after_paren = &inner[close + 1..];

                // Substitute each comma-separated parameter type.
                let resolved_params = if params_str.trim().is_empty() {
                    String::new()
                } else {
                    let param_parts = split_generic_args(params_str);
                    let resolved: Vec<Cow<str>> = param_parts
                        .iter()
                        .map(|p| apply_substitution(p, subs))
                        .collect();
                    resolved
                        .iter()
                        .map(|c| c.as_ref())
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                // Check for a return type after `): RetType`.
                let rest_trimmed = after_paren.trim_start();
                if let Some(after_colon) = rest_trimmed.strip_prefix(':') {
                    let ret_type = after_colon.trim_start();
                    if !ret_type.is_empty() {
                        let resolved_ret = apply_substitution(ret_type, subs);
                        return Cow::Owned(format!("{prefix}({resolved_params}): {resolved_ret}"));
                    }
                }
                return Cow::Owned(format!("{prefix}({resolved_params}){after_paren}"));
            }
        }
    }

    // Handle array shorthand: `TValue[]`.
    if let Some(base) = s.strip_suffix("[]") {
        let resolved = apply_substitution(base, subs);
        return Cow::Owned(format!("{resolved}[]"));
    }

    // Strip parentheses from DNF types like `(A&B)`.
    if s.starts_with('(') && s.ends_with(')') {
        let inner = &s[1..s.len() - 1];
        let resolved = apply_substitution(inner, subs);
        return Cow::Owned(format!("({resolved})"));
    }

    // Base case: direct lookup.
    if let Some(replacement) = subs.get(s) {
        return Cow::Owned(replacement.clone());
    }

    // No match — return as-is (borrowed, no allocation).
    Cow::Borrowed(s)
}

/// Split a type string on `delimiter` at nesting depth 0 (respecting
/// `<…>` and `(…)` nesting).
///
/// Returns `None` if the delimiter does not appear at depth 0 (i.e. the
/// string cannot be split).  Returns `Some(parts)` with at least 2 parts
/// otherwise.
fn split_at_depth_0(s: &str, delimiter: char) -> Option<Vec<&str>> {
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut found = false;

    // First pass: check if splitting is needed.
    for ch in s.chars() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            c if c == delimiter && depth_angle == 0 && depth_paren == 0 => {
                found = true;
                break;
            }
            _ => {}
        }
    }

    if !found {
        return None;
    }

    // Second pass: collect parts.
    let mut parts = Vec::new();
    depth_angle = 0;
    depth_paren = 0;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            c if c == delimiter && depth_angle == 0 && depth_paren == 0 => {
                parts.push(&s[start..i]);
                start = i + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);

    Some(parts)
}

/// Find the position of the first `<` at nesting depth 0.
fn find_angle_at_depth_0(s: &str) -> Option<usize> {
    let mut depth_paren = 0i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '<' if depth_paren == 0 => return Some(i),
            _ => {}
        }
    }
    None
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
    if class.template_params.is_empty() || type_args.is_empty() {
        return class.clone();
    }

    let mut subs = HashMap::new();
    for (i, param_name) in class.template_params.iter().enumerate() {
        if let Some(arg) = type_args.get(i) {
            subs.insert(param_name.clone(), (*arg).to_string());
        }
    }

    if subs.is_empty() {
        return class.clone();
    }

    let mut result = class.clone();
    for method in &mut result.methods {
        apply_substitution_to_method(method, &subs);
    }
    for property in &mut result.properties {
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

/// Apply a substitution map to a list of generic annotations.
///
/// Each entry is `(ClassName, [TypeArg1, TypeArg2, …])`.  Only the type
/// arguments are substituted; the class name is left unchanged.
fn apply_substitution_to_generics(
    generics: &mut [(String, Vec<String>)],
    subs: &HashMap<String, String>,
) {
    for (_class_name, type_args) in generics.iter_mut() {
        for arg in type_args.iter_mut() {
            let substituted = apply_substitution(arg, subs);
            if substituted.as_ref() != arg.as_str() {
                *arg = substituted.into_owned();
            }
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "inheritance_tests.rs"]
mod tests;
