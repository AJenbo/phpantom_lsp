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
///   type alias reference (`from:ClassName:OriginalName`).
use std::sync::Arc;

use crate::docblock;
use crate::docblock::types::{
    parse_generic_args, split_intersection_depth0, split_union_depth0, strip_generics,
};
use crate::inheritance::apply_generic_args;
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
    type_hint_to_classes(&type_hint, &class_info.name, all_classes, class_loader)
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

    let hint = type_hint.strip_prefix('?').unwrap_or(type_hint);

    // Strip surrounding parentheses that appear in DNF types like `(A&B)|C`.
    let hint = hint
        .strip_prefix('(')
        .and_then(|h| h.strip_suffix(')'))
        .unwrap_or(hint);

    // ── Type alias resolution ──────────────────────────────────────
    // Check if `hint` is a type alias defined on the owning class
    // (via `@phpstan-type` / `@psalm-type` / `@phpstan-import-type`).
    // If so, expand the alias and resolve the underlying definition.
    //
    // This runs before union/intersection splitting because the alias
    // itself may expand to a union or intersection type.
    if let Some(alias_def) = resolve_type_alias(hint, owning_class_name, all_classes, class_loader)
    {
        return type_hint_to_classes_depth(
            &alias_def,
            owning_class_name,
            all_classes,
            class_loader,
            depth + 1,
        );
    }

    // ── Union type: split on `|` at depth 0, respecting `<…>` nesting ──
    let union_parts = split_union_depth0(hint);
    if union_parts.len() > 1 {
        let mut results = Vec::new();
        for part in union_parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            // Recursively resolve each part (handles self/static, scalars,
            // intersection components, etc.)
            let resolved = type_hint_to_classes_depth(
                part,
                owning_class_name,
                all_classes,
                class_loader,
                depth,
            );
            ClassInfo::extend_unique(&mut results, resolved);
        }
        return results;
    }

    // ── Intersection type: split on `&` at depth 0 and resolve each part ──
    // `User&JsonSerializable` means the value satisfies *all* listed
    // types, so completions should include members from every part.
    // Uses depth-aware splitting so that `&` inside `{…}` or `<…>`
    // (e.g. `object{foo: A&B}`) is not treated as a top-level split.
    let intersection_parts = split_intersection_depth0(hint);
    if intersection_parts.len() > 1 {
        let mut results = Vec::new();
        for part in intersection_parts {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let resolved = type_hint_to_classes_depth(
                part,
                owning_class_name,
                all_classes,
                class_loader,
                depth,
            );
            ClassInfo::extend_unique(&mut results, resolved);
        }
        return results;
    }

    // ── Object shape: `object{foo: int, bar: string}` ──────────────
    // Synthesise a ClassInfo with public properties from the shape
    // entries so that `$var->foo` resolves through normal property
    // resolution.  Object shape properties are read-only.
    if docblock::types::is_object_shape(hint)
        && let Some(entries) = docblock::parse_object_shape(hint)
    {
        let properties = entries
            .into_iter()
            .map(|e| PropertyInfo {
                name: e.key,
                name_offset: 0,
                type_hint: Some(e.value_type),
                native_type_hint: None,
                description: None,
                is_static: false,
                visibility: Visibility::Public,
                deprecation_message: None,
                deprecated_replacement: None,
                is_virtual: true,
            })
            .collect();

        let synthetic = ClassInfo {
            name: "__object_shape".to_string(),
            properties,
            ..ClassInfo::default()
        };
        return vec![synthetic];
    }

    // self / static / $this always refer to the owning class.
    // In docblocks `@return $this` means "the instance the method is
    // called on" — identical to `static` for inheritance, but when the
    // method comes from a `@mixin` the return type is rewritten to the
    // mixin class name during merge (see `PHPDocProvider` in
    // `virtual_members/phpdoc.rs`).
    if hint == "self" || hint == "static" || hint == "$this" {
        return all_classes
            .iter()
            .find(|c| c.name == owning_class_name)
            .map(|c| ClassInfo::clone(c))
            .or_else(|| class_loader(owning_class_name).map(Arc::unwrap_or_clone))
            .into_iter()
            .collect();
    }

    // ── Parse generic arguments (if any) ──
    // `Collection<int, User>` → base_hint = `Collection`, generic_args = ["int", "User"]
    // `Foo`                   → base_hint = `Foo`,        generic_args = []
    let (base_hint, raw_generic_args) = parse_generic_args(hint);

    // ── Resolve static/self/$this inside generic arguments ────────
    // When a method returns e.g. `Builder<static>`, the generic arg
    // `static` must be resolved to the owning class name so that
    // `Brand::with('english')->` resolves to `Builder<Brand>` and
    // scope injection (and other generic substitution) works correctly.
    let resolved_generic_args: Vec<String> = raw_generic_args
        .iter()
        .map(|arg| {
            let trimmed = arg.trim();
            if trimmed == "static" || trimmed == "self" || trimmed == "$this" {
                owning_class_name.to_string()
            } else {
                trimmed.to_string()
            }
        })
        .collect();
    let generic_args: Vec<&str> = resolved_generic_args.iter().map(|s| s.as_str()).collect();

    // For class lookup, strip any remaining generics from the base
    // (should already be clean, but defensive) and use the short name.
    let base_clean = strip_generics(base_hint);
    let short = short_name(&base_clean);

    // Try local (current-file) lookup by last segment.
    //
    // When the type hint is namespace-qualified (e.g.
    // `Illuminate\Database\Eloquent\Builder`), match against each
    // class's `file_namespace` so that we pick the right one when
    // multiple classes share the same short name but live in
    // different namespace blocks (e.g. `Demo\Builder` vs
    // `Illuminate\Database\Eloquent\Builder` in example.php).
    let found = find_class_by_name(all_classes, &base_clean)
        .cloned()
        .or_else(|| class_loader(base_hint).map(Arc::unwrap_or_clone));

    match found {
        Some(cls) => {
            // ── Eloquent custom collection swapping ────────────────
            let cls = laravel::try_swap_custom_collection(
                cls,
                &base_clean,
                &generic_args,
                all_classes,
                class_loader,
            );

            // Apply generic substitution if the type hint carried
            // generic arguments and the class has template parameters.
            // Resolve the class fully first (including trait methods,
            // parent methods, and virtual members) so that methods
            // inherited from traits also receive the substitution.
            // Without this, a method like `first()` inherited from
            // `BuildsQueries` via `@use BuildsQueries<TModel>` would
            // keep its raw `TModel` return type instead of being
            // substituted to the concrete model class.
            if !generic_args.is_empty() && !cls.template_params.is_empty() {
                let resolved = virtual_members::resolve_class_fully(&cls, class_loader);
                let mut result = apply_generic_args(&resolved, &generic_args);

                // ── Eloquent Builder scope injection ───────────────
                laravel::try_inject_builder_scopes(
                    &mut result,
                    &cls,
                    &base_clean,
                    &generic_args,
                    class_loader,
                );

                vec![result]
            } else {
                vec![cls]
            }
        }
        None => {
            // ── Template parameter bound fallback ──────────────────
            // When the type hint doesn't match any known class, check
            // whether it is a template parameter declared on the
            // owning class.  If it has an `of` bound (e.g.
            // `@template TNode of PDependNode`), resolve the bound
            // type so that completion and go-to-definition still work.
            let loaded;
            let owning = match all_classes.iter().find(|c| c.name == owning_class_name) {
                Some(c) => Some(c.as_ref()),
                None => {
                    loaded = class_loader(owning_class_name);
                    loaded.as_deref()
                }
            };

            // Try class-level template param bounds on the owning class.
            if let Some(owner) = owning
                && owner.template_params.contains(&short.to_string())
                && let Some(bound) = owner.template_param_bounds.get(short)
            {
                return type_hint_to_classes_depth(
                    bound,
                    owning_class_name,
                    all_classes,
                    class_loader,
                    depth + 1,
                );
            }

            // ── stdClass fallback ──────────────────────────────────
            // `stdClass` is a universal PHP built-in that accepts
            // arbitrary properties.  When stubs are not installed the
            // class loader won't find it, but we still need a
            // `ClassInfo` so that downstream consumers (diagnostics,
            // completion) can recognise the type and suppress
            // unknown-member warnings.
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
/// For imported aliases (`from:ClassName:OriginalName`), the source
/// class is loaded and the original alias is resolved from its
/// `type_aliases` map.
///
/// Pass an empty `owning_class_name` to search all classes without
/// priority (used by the array-key completion path).
pub(crate) fn resolve_type_alias(
    hint: &str,
    owning_class_name: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    let mut current = hint.to_string();
    let mut resolved_any = false;

    for _ in 0..10 {
        // Only bare identifiers (no `<`, `{`, `|`, `&`, `?`, `\`) can be
        // type aliases.  Skip anything that looks like a complex type
        // expression to avoid false matches.
        if current.contains('<')
            || current.contains('{')
            || current.contains('|')
            || current.contains('&')
            || current.contains('?')
            || current.contains('\\')
            || current.contains('$')
        {
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

    if resolved_any { Some(current) } else { None }
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
        // Handle imported type aliases: `from:ClassName:OriginalName`
        if let Some(import_ref) = def.strip_prefix("from:") {
            return resolve_imported_type_alias(import_ref, all_classes, class_loader);
        }
        return Some(def.clone());
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
            if let Some(import_ref) = def.strip_prefix("from:") {
                return resolve_imported_type_alias(import_ref, all_classes, class_loader);
            }
            return Some(def.clone());
        }
    }

    None
}

/// Resolve an imported type alias reference.
///
/// The `import_ref` string has the format `ClassName:OriginalName`
/// (the `from:` prefix has already been stripped by the caller).
/// Loads the source class and returns the original alias definition.
pub(crate) fn resolve_imported_type_alias(
    import_ref: &str,
    all_classes: &[Arc<ClassInfo>],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<String> {
    let (source_class_name, original_name) = import_ref.split_once(':')?;

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

    // Don't follow nested imports — just return the definition.
    if def.starts_with("from:") {
        return None;
    }

    Some(def.clone())
}
