/// Type-hint resolution to concrete `ClassInfo` values.
///
/// This module contains the logic for mapping type-hint strings (as they
/// appear in return types, property annotations, and PHPDoc tags) to
/// resolved `ClassInfo` values that the completion, hover, and definition
/// engines can work with.
///
/// Split from [`super::resolver`] for navigability.  The entry points are:
///
/// - [`type_hint_to_classes`]: the public facade that maps a
///   type-hint string to all matching `ClassInfo` values (handles unions,
///   intersections, generics, `self`/`static`/`$this`, nullable types,
///   object shapes, and type alias expansion).
/// - [`resolve_type_alias`]: fully expands a type alias defined
///   via `@phpstan-type` / `@psalm-type` / `@phpstan-import-type`.
/// - [`resolve_property_types`]: resolves a property's type hint
///   on a class to all candidate `ClassInfo` values.
/// - [`resolve_imported_type_alias`]: resolves a single imported
///   type alias reference.
use std::sync::Arc;

use crate::inheritance::{apply_generic_args, build_generic_subs};
use crate::php_type::PhpType;
use crate::types::*;
use crate::util::{find_class_by_name, short_name};
use crate::virtual_members::{self, laravel};

/// Look up a property's type hint and resolve all candidate classes.
///
/// When the type hint is a union (e.g. `A|B`), every resolvable part
/// is returned.
pub(crate) fn resolve_property_types(
    prop_name: &str,
    class_info: &ClassInfo,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Vec<ClassInfo> {
    // Resolve inheritance so that inherited (and generic-substituted)
    // properties are visible.  For example, if `ConfigWrapper extends
    // Wrapper<Config>` and `Wrapper` has `/** @var T */ public $value`,
    // the merged class will have `$value` with type `Config`.
    let type_hint =
        match crate::inheritance::resolve_property_type_hint(class_info, prop_name, class_loader) {
            Some(h) => h,
            None => return vec![],
        };
    type_hint_to_classes_typed(&type_hint, &class_info.name, all_classes, class_loader)
}

/// Map a type-hint string to all matching `ClassInfo` values.
///
/// Handles:
///   - Nullable types: `?Foo` → strips `?`, resolves `Foo`
///   - Union types: `A|B|C` → resolves each part independently
///     (respects `<…>` nesting so `Collection<int|string>` is not split)
///   - Intersection types: `A&B` → resolves each part independently
///   - Generic types: `Collection<int, User>` → resolves `Collection`,
///     then applies generic substitution (`TKey→int`, `TValue→User`)
///   - `self` / `static` / `$this` → owning class
///   - Scalar/built-in types (`int`, `string`, `bool`, `float`, `array`,
///     `void`, `null`, `mixed`, `never`, `object`, `callable`, `iterable`,
///     `false`, `true`) → skipped (not class types)
///
/// Each resolvable class-like part is returned as a separate entry.
pub(crate) fn type_hint_to_classes(
    type_hint: &str,
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Vec<ClassInfo> {
    type_hint_to_classes_depth(type_hint, owning_class_name, all_classes, class_loader, 0)
}

/// Like [`type_hint_to_classes`], but accepts a pre-parsed [`PhpType`]
/// to avoid a parse→stringify→reparse round-trip when the caller already
/// has a structured type.
pub(crate) fn type_hint_to_classes_typed(
    ty: &PhpType,
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Vec<ClassInfo> {
    type_hint_to_classes_typed_depth(ty, owning_class_name, all_classes, class_loader, 0)
}

/// Inner implementation of [`type_hint_to_classes`] with a recursion
/// depth guard to prevent infinite loops from circular type aliases.
fn type_hint_to_classes_depth(
    type_hint: &str,
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    depth: u8,
) -> Vec<ClassInfo> {
    if depth > MAX_ALIAS_DEPTH {
        return vec![];
    }

    // Parse first — PhpType::parse handles `?Foo`, `(A&B)|C`, and all
    // other syntax natively, making manual string preprocessing redundant.
    // Alias resolution is handled by `resolve_named_type` when the parsed
    // type turns out to be a `Named` variant.
    let parsed = PhpType::parse(type_hint);
    type_hint_to_classes_typed_depth(&parsed, owning_class_name, all_classes, class_loader, depth)
}

/// Inner implementation that operates on a pre-parsed [`PhpType`].
///
/// Union and intersection members are recursed directly without
/// stringifying and re-parsing.  Named types still go through
/// string-based alias resolution when necessary.
fn type_hint_to_classes_typed_depth(
    ty: &PhpType,
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    depth: u8,
) -> Vec<ClassInfo> {
    if depth > MAX_ALIAS_DEPTH {
        return vec![];
    }

    match ty {
        // ── Nullable → unwrap inner ────────────────────────────────
        PhpType::Nullable(inner) => type_hint_to_classes_typed_depth(
            inner,
            owning_class_name,
            all_classes,
            class_loader,
            depth,
        ),

        // ── Union type ─────────────────────────────────────────────
        PhpType::Union(members) => {
            let mut results = Vec::new();
            for member in members {
                let resolved = type_hint_to_classes_typed_depth(
                    member,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth,
                );
                ClassInfo::extend_unique(&mut results, resolved);
            }
            results
        }

        // ── Intersection type ──────────────────────────────────────
        PhpType::Intersection(members) => {
            let mut results = Vec::new();
            for member in members {
                let resolved = type_hint_to_classes_typed_depth(
                    member,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth,
                );
                ClassInfo::extend_unique(&mut results, resolved);
            }
            results
        }

        // ── Object shape ───────────────────────────────────────────
        PhpType::ObjectShape(entries) => {
            let properties = SharedVec::from_vec(
                entries
                    .iter()
                    .map(|e| PropertyInfo {
                        name: e.key.clone().unwrap_or_default(),
                        name_offset: 0,
                        type_hint: Some(e.value_type.clone()),
                        native_type_hint: None,
                        description: None,
                        is_static: false,
                        visibility: Visibility::Public,
                        deprecation_message: None,
                        deprecated_replacement: None,
                        see_refs: Vec::new(),
                        is_virtual: true,
                    })
                    .collect(),
            );

            vec![ClassInfo {
                name: "__object_shape".to_string(),
                properties,
                ..ClassInfo::default()
            }]
        }

        // ── Named type (class name, keyword, or alias) ─────────────
        PhpType::Named(name) => resolve_named_type(
            name,
            &[],
            owning_class_name,
            all_classes,
            class_loader,
            depth,
        ),

        // ── Generic type ───────────────────────────────────────────
        PhpType::Generic(name, args) => resolve_named_type(
            name,
            args,
            owning_class_name,
            all_classes,
            class_loader,
            depth,
        ),

        // ── Array slice (T[]) ──────────────────────────────────────
        // Not a class type itself; skip.
        PhpType::Array(_)
        | PhpType::ArrayShape(_)
        | PhpType::Callable { .. }
        | PhpType::ClassString(_)
        | PhpType::InterfaceString(_)
        | PhpType::KeyOf(_)
        | PhpType::ValueOf(_)
        | PhpType::IntRange(..)
        | PhpType::IndexAccess(..)
        | PhpType::Literal(_)
        | PhpType::Conditional { .. }
        | PhpType::Raw(_) => vec![],
    }
}

/// Resolve a named type (with optional generic args) to `ClassInfo`.
///
/// Handles `self`/`static`/`$this`/`parent`, type alias expansion,
/// template parameter bound fallback, and class lookup with generic
/// substitution.
fn resolve_named_type(
    name: &str,
    generic_args: &[PhpType],
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    depth: u8,
) -> Vec<ClassInfo> {
    // ── Type alias resolution ──────────────────────────────────────
    if let Some(alias_type) = resolve_type_alias(name, owning_class_name, all_classes, class_loader)
    {
        return type_hint_to_classes_typed_depth(
            &alias_type,
            owning_class_name,
            all_classes,
            class_loader,
            depth + 1,
        );
    }

    // ── self / static / $this ──────────────────────────────────────
    if matches!(name, "self" | "static" | "$this") {
        if !generic_args.is_empty() {
            // `self<RuleError>` → rewrite to `OwningClass<RuleError>`.
            let rewritten = PhpType::Generic(owning_class_name.to_string(), generic_args.to_vec());
            return type_hint_to_classes_typed_depth(
                &rewritten,
                owning_class_name,
                all_classes,
                class_loader,
                depth,
            );
        }
        return all_classes
            .iter()
            .find(|c| c.name == owning_class_name)
            .map(|c| ClassInfo::clone(c))
            .or_else(|| class_loader(owning_class_name).map(Arc::unwrap_or_clone))
            .into_iter()
            .collect();
    }

    // ── parent ─────────────────────────────────────────────────────
    if name == "parent" {
        let parent_name = all_classes
            .iter()
            .find(|c| c.name == owning_class_name)
            .and_then(|c| c.parent_class.clone())
            .or_else(|| class_loader(owning_class_name).and_then(|c| c.parent_class.clone()));
        if let Some(parent) = parent_name {
            return all_classes
                .iter()
                .find(|c| c.name == parent)
                .map(|c| ClassInfo::clone(c))
                .or_else(|| class_loader(&parent).map(Arc::unwrap_or_clone))
                .into_iter()
                .collect();
        }
        return vec![];
    }

    // ── Resolve static/self/$this inside generic arguments ────────
    let resolved_generic_args: Vec<PhpType>;
    let generic_args: &[PhpType] = if generic_args.iter().any(
        |a| matches!(a, PhpType::Named(n) if matches!(n.as_str(), "static" | "self" | "$this")),
    ) {
        resolved_generic_args = generic_args
            .iter()
            .map(|arg| {
                if let PhpType::Named(n) = arg
                    && matches!(n.as_str(), "static" | "self" | "$this")
                {
                    PhpType::Named(owning_class_name.to_string())
                } else {
                    arg.clone()
                }
            })
            .collect();
        &resolved_generic_args
    } else {
        generic_args
    };

    let short = short_name(name);

    // ── Class lookup ───────────────────────────────────────────────
    let found = find_class_by_name(all_classes, name)
        .map(|arc| ClassInfo::clone(arc))
        .or_else(|| class_loader(name).map(Arc::unwrap_or_clone));

    match found {
        Some(cls) => {
            // ── Eloquent custom collection swapping ────────────────
            let cls = laravel::try_swap_custom_collection(
                cls,
                name,
                generic_args,
                all_classes,
                class_loader,
            );

            // Apply generic substitution if the type hint carried
            // generic arguments and the class has template parameters.
            if !generic_args.is_empty() && !cls.template_params.is_empty() {
                let resolved = if let Some(cache) = virtual_members::active_resolved_class_cache() {
                    virtual_members::resolve_class_fully_cached(&cls, class_loader, cache)
                } else {
                    virtual_members::resolve_class_fully(&cls, class_loader)
                };
                let mut result = apply_generic_args(&resolved, generic_args);

                // ── Template-param mixin resolution ────────────────
                // When a class declares `@mixin TParam` where `TParam`
                // is a template parameter, the mixin cannot be resolved
                // during `resolve_class_fully` because the concrete type
                // is not yet known.  Now that generic args are concrete,
                // resolve those mixins and merge their members.
                if cls.mixins.iter().any(|m| cls.template_params.contains(m)) {
                    let subs = build_generic_subs(&cls, generic_args);
                    if !subs.is_empty() {
                        let mixin_members = virtual_members::phpdoc::resolve_template_param_mixins(
                            &cls,
                            &subs,
                            class_loader,
                        );
                        if !mixin_members.is_empty() {
                            virtual_members::merge_virtual_members(&mut result, mixin_members);
                        }
                    }
                }

                // ── Eloquent Builder scope injection ───────────────
                laravel::try_inject_builder_scopes(
                    &mut result,
                    &cls,
                    name,
                    generic_args,
                    class_loader,
                );

                // ── Inherited Builder mixin scope injection ────────
                // When a class inherits `@mixin Builder<TRelatedModel>`
                // from an ancestor (e.g. HasMany inherits it from
                // Relation), the mixin expansion adds Builder's own
                // methods but not model-specific scopes.  Now that
                // generic args are concrete, resolve the model type
                // and inject its scopes.
                laravel::try_inject_mixin_builder_scopes(
                    &mut result,
                    &cls,
                    generic_args,
                    class_loader,
                );

                vec![result]
            } else {
                vec![cls]
            }
        }
        None => {
            // ── Template parameter bound fallback ──────────────────
            let loaded;
            let owning = match all_classes.iter().find(|c| c.name == owning_class_name) {
                Some(c) => Some(c.as_ref()),
                None => {
                    loaded = class_loader(owning_class_name);
                    loaded.as_deref()
                }
            };

            if let Some(owner) = owning
                && owner.template_params.contains(&short.to_string())
                && let Some(bound) = owner.template_param_bounds.get(short)
            {
                return type_hint_to_classes_typed_depth(
                    bound,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth + 1,
                );
            }

            // ── stdClass fallback ──────────────────────────────────
            if short == "stdClass" {
                return vec![ClassInfo {
                    name: "stdClass".to_string(),
                    ..ClassInfo::default()
                }];
            }

            vec![]
        }
    }
}

/// Look up a type alias by name and fully expand alias chains.
///
/// Returns the fully expanded type definition string if `hint` is a
/// known alias, or `None` if it is not. Follows up to 10 levels of
/// alias indirection to handle aliases that reference other aliases.
///
/// For imported aliases, the source class is loaded and the original
/// alias is resolved from its `type_aliases` map.
///
/// Pass an empty `owning_class_name` to search all classes without
/// priority (used by the array-key completion path).
pub(crate) fn resolve_type_alias(
    hint: &str,
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    let mut current = hint.to_string();
    let mut resolved_any = false;

    for _ in 0..10 {
        // Only bare identifiers can be type aliases.  Skip anything that
        // looks like a complex type expression to avoid false matches.
        if !matches!(PhpType::parse(&current), PhpType::Named(_)) {
            break;
        }

        let expanded =
            resolve_type_alias_once(&current, owning_class_name, all_classes, class_loader);

        match expanded {
            Some(def) => {
                current = def;
                resolved_any = true;
            }
            None => break,
        }
    }

    if resolved_any {
        Some(PhpType::parse(&current))
    } else {
        None
    }
}

/// Single-level alias lookup (no chaining).
fn resolve_type_alias_once(
    hint: &str,
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    // Find the owning class to check its type_aliases.
    let owning_class = all_classes.iter().find(|c| c.name == owning_class_name);

    if let Some(cls) = owning_class
        && let Some(def) = cls.type_aliases.get(hint)
    {
        return expand_type_alias_def(def, all_classes, class_loader);
    }

    // Also check all classes in the file — the type alias might be
    // referenced from a method inside a different class that uses the
    // owning class's return type.  This is rare but handles the case
    // where the owning class name is empty (top-level code) or when
    // the type is used in a context where the owning class is not the
    // declaring class.
    for cls in all_classes {
        if cls.name == owning_class_name {
            continue; // Already checked above.
        }
        if let Some(def) = cls.type_aliases.get(hint) {
            return expand_type_alias_def(def, all_classes, class_loader);
        }
    }

    None
}

/// Expand a [`TypeAliasDef`] into a type definition string.
///
/// For local aliases, returns the `PhpType`'s string representation.
/// For imports, loads the source class and returns the original alias
/// definition.
fn expand_type_alias_def(
    def: &TypeAliasDef,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    match def {
        TypeAliasDef::Local(php_type) => Some(php_type.to_string()),
        TypeAliasDef::Import {
            source_class,
            original_name,
        } => resolve_imported_type_alias(source_class, original_name, all_classes, class_loader),
    }
}

/// Resolve an imported type alias reference.
///
/// Loads the source class by `source_class_name` and looks up
/// `original_name` in its `type_aliases` map.
pub(crate) fn resolve_imported_type_alias(
    source_class_name: &str,
    original_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    // Try to find the source class.
    let lookup = source_class_name
        .rsplit('\\')
        .next()
        .unwrap_or(source_class_name);
    let source_class = all_classes
        .iter()
        .find(|c| c.name == lookup)
        .map(|c| ClassInfo::clone(c))
        .or_else(|| class_loader(source_class_name).map(Arc::unwrap_or_clone));

    let source_class = source_class?;
    let def = source_class.type_aliases.get(original_name)?;

    // Don't follow nested imports — just return the local definition.
    match def {
        TypeAliasDef::Local(php_type) => Some(php_type.to_string()),
        TypeAliasDef::Import { .. } => None,
    }
}
