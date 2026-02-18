/// Class inheritance resolution.
///
/// This module handles merging members from parent classes, traits, and
/// `@mixin` classes into a single `ClassInfo`.  The resulting merged class
/// contains the complete set of members visible on an instance / static
/// access, respecting PHP's precedence rules:
///
///   class own > traits > parent chain > mixins
///
/// It also supports **generic type substitution**: when a child class
/// declares `@extends Parent<ConcreteType1, ConcreteType2>` and the parent
/// has `@template T1` / `@template T2`, the inherited methods and properties
/// have their template parameter references replaced with the concrete types.
use std::collections::HashMap;

use crate::Backend;
use crate::types::Visibility;
use crate::types::{ClassInfo, MethodInfo, PropertyInfo};

impl Backend {
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
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) -> ClassInfo {
        let mut merged = class.clone();

        // 1. Merge traits used by this class.
        //    PHP precedence: class methods > trait methods > inherited methods.
        //    Since `merged` already contains the class's own members, we only
        //    add trait members that don't collide with existing ones.
        Self::merge_traits_into(
            &mut merged,
            &class.used_traits,
            &class.use_generics,
            class_loader,
            0,
        );

        // 2. Walk up the `extends` chain and merge parent members.
        let mut current = class.clone();
        let mut depth = 0;
        const MAX_DEPTH: u32 = 20;

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
            if depth > MAX_DEPTH {
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
            let level_subs = build_substitution_map(&current, &parent, &active_subs);

            // Merge traits used by the parent class as well, so that
            // grandparent-level trait members are visible.
            Self::merge_traits_into(
                &mut merged,
                &parent.used_traits,
                &parent.use_generics,
                class_loader,
                0,
            );

            // Merge parent methods — skip private, skip if child already has one with same name
            for method in &parent.methods {
                if method.visibility == Visibility::Private {
                    continue;
                }
                if merged.methods.iter().any(|m| m.name == method.name) {
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
                if merged.properties.iter().any(|p| p.name == property.name) {
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
                if merged.constants.iter().any(|c| c.name == constant.name) {
                    continue;
                }
                merged.constants.push(constant.clone());
            }

            // Carry the substitution map forward for the next level.
            // If `Collection` extends `AbstractCollection<TKey, TValue>`,
            // we need to apply the current substitutions to those type
            // arguments so that `TKey` → `int` flows through.
            active_subs = level_subs;
            current = parent;
        }

        // 3. Merge members from @mixin classes.
        //    Mixin members have the lowest precedence — they only fill in
        //    members that are not already provided by the class itself,
        //    its traits, or its parent chain.  This models the PHP pattern
        //    where `@mixin` documents that magic methods (__call, __get,
        //    etc.) proxy to another class.
        //
        //    Mixins are inherited: if `User extends Model` and `Model`
        //    has `@mixin Builder`, then `User` also gains Builder's
        //    members.  We merge the class's own mixins first, then walk
        //    up the parent chain again to collect ancestor mixins.
        Self::merge_mixins_into(&mut merged, &class.mixins, class_loader);

        // Also merge mixins declared on ancestor classes.
        let mut ancestor = class.clone();
        let mut mixin_depth = 0u32;
        while let Some(ref parent_name) = ancestor.parent_class {
            mixin_depth += 1;
            if mixin_depth > MAX_DEPTH {
                break;
            }
            let parent = if let Some(p) = class_loader(parent_name) {
                p
            } else {
                break;
            };
            if !parent.mixins.is_empty() {
                Self::merge_mixins_into(&mut merged, &parent.mixins, class_loader);
            }
            ancestor = parent;
        }

        merged
    }

    /// Merge public members from `@mixin` classes into `merged`.
    ///
    /// Mixins are resolved with full inheritance (the mixin class itself
    /// may extend another class, use traits, etc.), and only **public**
    /// members that don't already exist in `merged` are added.  This
    /// gives mixins the lowest precedence in the resolution chain:
    ///
    ///   class own > traits > parent chain > mixins
    ///
    /// Mixin classes can themselves declare `@mixin`, so this recurses
    /// up to a depth limit to handle mixin chains.
    fn merge_mixins_into(
        merged: &mut ClassInfo,
        mixin_names: &[String],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
    ) {
        const MAX_MIXIN_DEPTH: u32 = 10;
        Self::merge_mixins_into_recursive(merged, mixin_names, class_loader, 0, MAX_MIXIN_DEPTH);
    }

    fn merge_mixins_into_recursive(
        merged: &mut ClassInfo,
        mixin_names: &[String],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: u32,
        max_depth: u32,
    ) {
        if depth > max_depth {
            return;
        }

        for mixin_name in mixin_names {
            let mixin_class = if let Some(c) = class_loader(mixin_name) {
                c
            } else {
                continue;
            };

            // Resolve the mixin class with its own inheritance so we see
            // all of its inherited/trait members too.
            let resolved_mixin = Self::resolve_class_with_inheritance(&mixin_class, class_loader);

            // Only merge public members — mixins proxy via magic methods
            // which only expose public API.
            for method in &resolved_mixin.methods {
                if method.visibility != Visibility::Public {
                    continue;
                }
                if merged.methods.iter().any(|m| m.name == method.name) {
                    continue;
                }
                let mut method = method.clone();
                // `@return $this` in the mixin class refers to the mixin
                // instance, NOT the consuming class.  Rewrite the return
                // type to the concrete mixin class name so that resolution
                // produces the mixin class rather than the consumer.
                if matches!(
                    method.return_type.as_deref(),
                    Some("$this" | "self" | "static")
                ) {
                    method.return_type = Some(mixin_class.name.clone());
                }
                merged.methods.push(method);
            }

            for property in &resolved_mixin.properties {
                if property.visibility != Visibility::Public {
                    continue;
                }
                if merged.properties.iter().any(|p| p.name == property.name) {
                    continue;
                }
                merged.properties.push(property.clone());
            }

            for constant in &resolved_mixin.constants {
                if constant.visibility != Visibility::Public {
                    continue;
                }
                if merged.constants.iter().any(|c| c.name == constant.name) {
                    continue;
                }
                merged.constants.push(constant.clone());
            }

            // Recurse into mixins declared by the mixin class itself.
            if !mixin_class.mixins.is_empty() {
                Self::merge_mixins_into_recursive(
                    merged,
                    &mixin_class.mixins,
                    class_loader,
                    depth + 1,
                    max_depth,
                );
            }
        }
    }

    /// Recursively merge members from the given traits into `merged`.
    ///
    /// Traits can themselves `use` other traits (composition), so this
    /// method recurses up to `MAX_TRAIT_DEPTH` levels.  Members that
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
        use_generics: &[(String, Vec<String>)],
        class_loader: &dyn Fn(&str) -> Option<ClassInfo>,
        depth: u32,
    ) {
        const MAX_TRAIT_DEPTH: u32 = 20;
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
            let trait_subs = build_trait_substitution_map(trait_name, &trait_info, use_generics);

            // Recursively merge traits used by this trait (trait composition).
            // The sub-trait's own `@use` generics (from the trait's docblock)
            // apply, not the outer class's.
            if !trait_info.used_traits.is_empty() {
                Self::merge_traits_into(
                    merged,
                    &trait_info.used_traits,
                    &trait_info.use_generics,
                    class_loader,
                    depth + 1,
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
                    Self::merge_traits_into(
                        merged,
                        &parent.used_traits,
                        &parent.use_generics,
                        class_loader,
                        parent_depth + 1,
                    );
                }

                // Merge parent methods (skip private, skip duplicates)
                for method in &parent.methods {
                    if method.visibility == Visibility::Private {
                        continue;
                    }
                    if merged.methods.iter().any(|m| m.name == method.name) {
                        continue;
                    }
                    merged.methods.push(method.clone());
                }

                // Merge parent properties
                for property in &parent.properties {
                    if property.visibility == Visibility::Private {
                        continue;
                    }
                    if merged.properties.iter().any(|p| p.name == property.name) {
                        continue;
                    }
                    merged.properties.push(property.clone());
                }

                // Merge parent constants
                for constant in &parent.constants {
                    if constant.visibility == Visibility::Private {
                        continue;
                    }
                    if merged.constants.iter().any(|c| c.name == constant.name) {
                        continue;
                    }
                    merged.constants.push(constant.clone());
                }

                current = parent;
            }

            // Merge trait methods — skip if already present.
            // Apply generic substitution if a `@use` mapping exists.
            for method in &trait_info.methods {
                if merged.methods.iter().any(|m| m.name == method.name) {
                    continue;
                }
                let mut method = method.clone();
                if !trait_subs.is_empty() {
                    apply_substitution_to_method(&mut method, &trait_subs);
                }
                merged.methods.push(method);
            }

            // Merge trait properties — apply substitution.
            for property in &trait_info.properties {
                if merged.properties.iter().any(|p| p.name == property.name) {
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
                if merged.constants.iter().any(|c| c.name == constant.name) {
                    continue;
                }
                merged.constants.push(constant.clone());
            }
        }
    }
}

// ─── Generic Type Substitution ──────────────────────────────────────────────

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
                apply_substitution(arg, active_subs)
            };
            map.insert(param_name.clone(), resolved);
        }
    }

    map
}

/// Apply generic type substitution to a method's return type and parameter
/// type hints.
fn apply_substitution_to_method(method: &mut MethodInfo, subs: &HashMap<String, String>) {
    if let Some(ref mut ret) = method.return_type {
        let substituted = apply_substitution(ret, subs);
        if substituted != *ret {
            *ret = substituted;
        }
    }
    for param in &mut method.parameters {
        if let Some(ref mut hint) = param.type_hint {
            let substituted = apply_substitution(hint, subs);
            if substituted != *hint {
                *hint = substituted;
            }
        }
    }
}

/// Apply generic type substitution to a property's type hint.
fn apply_substitution_to_property(property: &mut PropertyInfo, subs: &HashMap<String, String>) {
    if let Some(ref mut hint) = property.type_hint {
        let substituted = apply_substitution(hint, subs);
        if substituted != *hint {
            *hint = substituted;
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
fn apply_substitution(type_str: &str, subs: &HashMap<String, String>) -> String {
    let s = type_str.trim();
    if s.is_empty() {
        return s.to_string();
    }

    // Handle nullable prefix.
    if let Some(inner) = s.strip_prefix('?') {
        let resolved = apply_substitution(inner, subs);
        return format!("?{resolved}");
    }

    // Handle union types: split on `|` at depth 0.
    if let Some(parts) = split_at_depth_0(s, '|') {
        let resolved: Vec<String> = parts.iter().map(|p| apply_substitution(p, subs)).collect();
        return resolved.join("|");
    }

    // Handle intersection types: split on `&` at depth 0.
    if let Some(parts) = split_at_depth_0(s, '&') {
        let resolved: Vec<String> = parts.iter().map(|p| apply_substitution(p, subs)).collect();
        return resolved.join("&");
    }

    // Handle generic types: `Base<Arg1, Arg2>`.
    if let Some(angle_pos) = find_angle_at_depth_0(s) {
        let base = &s[..angle_pos];
        // Find the matching closing `>`.
        let rest = &s[angle_pos + 1..];
        let close_pos = find_matching_close_angle(rest);
        let inner = &rest[..close_pos];
        let after = &rest[close_pos + 1..]; // anything after `>`

        // Resolve the base (it might itself be a template param).
        let resolved_base = apply_substitution(base, subs);

        // Split inner on commas at depth 0 and resolve each arg.
        let args = split_generic_args_at_depth_0(inner);
        let resolved_args: Vec<String> = args
            .iter()
            .map(|a| apply_substitution(a.trim(), subs))
            .collect();

        let mut result = format!("{resolved_base}<{}>", resolved_args.join(", "));
        if !after.is_empty() {
            result.push_str(after);
        }
        return result;
    }

    // Handle array shorthand: `TValue[]`.
    if let Some(base) = s.strip_suffix("[]") {
        let resolved = apply_substitution(base, subs);
        return format!("{resolved}[]");
    }

    // Strip parentheses from DNF types like `(A&B)`.
    if s.starts_with('(') && s.ends_with(')') {
        let inner = &s[1..s.len() - 1];
        let resolved = apply_substitution(inner, subs);
        return format!("({resolved})");
    }

    // Base case: direct lookup.
    if let Some(replacement) = subs.get(s) {
        return replacement.clone();
    }

    // Template parameter names resolved in a no-namespace context get a
    // leading `\` prefix (e.g. `T` → `\T`), but the substitution map
    // keys are bare names.  Try again without the prefix.
    if let Some(stripped) = s.strip_prefix('\\')
        && let Some(replacement) = subs.get(stripped)
    {
        return replacement.clone();
    }

    // No match — return as-is.
    s.to_string()
}

/// Extract the short (unqualified) class name from a potentially
/// fully-qualified name.
fn short_name(name: &str) -> &str {
    name.rsplit('\\').next().unwrap_or(name)
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

/// Find the position of the matching `>` for an opening `<` that has
/// already been consumed.  `s` starts right after the `<`.
fn find_matching_close_angle(s: &str) -> usize {
    let mut depth = 1i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    // Fallback: end of string (malformed type).
    s.len()
}

/// Split generic arguments on commas at depth 0, respecting `<…>` and
/// `(…)` nesting.
fn split_generic_args_at_depth_0(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            ',' if depth_angle == 0 && depth_paren == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
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
    result
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_substitution_direct() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "Language".to_string());
        subs.insert("TKey".to_string(), "int".to_string());

        assert_eq!(apply_substitution("TValue", &subs), "Language");
        assert_eq!(apply_substitution("TKey", &subs), "int");
        assert_eq!(apply_substitution("string", &subs), "string");
    }

    #[test]
    fn test_apply_substitution_nullable() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "Language".to_string());

        assert_eq!(apply_substitution("?TValue", &subs), "?Language");
    }

    #[test]
    fn test_apply_substitution_union() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "Language".to_string());

        assert_eq!(apply_substitution("TValue|null", &subs), "Language|null");
        assert_eq!(
            apply_substitution("TValue|string", &subs),
            "Language|string"
        );
    }

    #[test]
    fn test_apply_substitution_intersection() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "Language".to_string());

        assert_eq!(
            apply_substitution("TValue&Countable", &subs),
            "Language&Countable"
        );
    }

    #[test]
    fn test_apply_substitution_generic() {
        let mut subs = HashMap::new();
        subs.insert("TKey".to_string(), "int".to_string());
        subs.insert("TValue".to_string(), "Language".to_string());

        assert_eq!(
            apply_substitution("array<TKey, TValue>", &subs),
            "array<int, Language>"
        );
    }

    #[test]
    fn test_apply_substitution_nested_generic() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "User".to_string());

        assert_eq!(
            apply_substitution("Collection<int, list<TValue>>", &subs),
            "Collection<int, list<User>>"
        );
    }

    #[test]
    fn test_apply_substitution_array_shorthand() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "User".to_string());

        assert_eq!(apply_substitution("TValue[]", &subs), "User[]");
    }

    #[test]
    fn test_apply_substitution_no_match() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "User".to_string());

        assert_eq!(apply_substitution("string", &subs), "string");
        assert_eq!(apply_substitution("void", &subs), "void");
        assert_eq!(apply_substitution("$this", &subs), "$this");
    }

    #[test]
    fn test_apply_substitution_complex_union_with_generic() {
        let mut subs = HashMap::new();
        subs.insert("TKey".to_string(), "int".to_string());
        subs.insert("TValue".to_string(), "User".to_string());

        assert_eq!(
            apply_substitution("array<TKey, TValue>|null", &subs),
            "array<int, User>|null"
        );
    }

    #[test]
    fn test_apply_substitution_dnf_parens() {
        let mut subs = HashMap::new();
        subs.insert("T".to_string(), "User".to_string());

        assert_eq!(
            apply_substitution("(T&Countable)", &subs),
            "(User&Countable)"
        );
    }

    #[test]
    fn test_build_substitution_map_basic() {
        let child = ClassInfo {
            name: "LanguageCollection".to_string(),
            parent_class: Some("Collection".to_string()),
            is_final: true,
            extends_generics: vec![(
                "Collection".to_string(),
                vec!["int".to_string(), "Language".to_string()],
            )],
            ..ClassInfo::default()
        };

        let parent = ClassInfo {
            name: "Collection".to_string(),
            template_params: vec!["TKey".to_string(), "TValue".to_string()],
            ..ClassInfo::default()
        };

        let subs = build_substitution_map(&child, &parent, &HashMap::new());
        assert_eq!(subs.get("TKey").unwrap(), "int");
        assert_eq!(subs.get("TValue").unwrap(), "Language");
    }

    #[test]
    fn test_build_substitution_map_chained() {
        // Simulates: C extends B<Foo>, B extends A<T>, A has @template U
        // When resolving A's methods for C, active_subs = {T => Foo}
        // B's @extends A<T> should resolve to A<Foo>, giving {U => Foo}

        let current_b = ClassInfo {
            name: "B".to_string(),
            parent_class: Some("A".to_string()),
            template_params: vec!["T".to_string()],
            extends_generics: vec![("A".to_string(), vec!["T".to_string()])],
            ..ClassInfo::default()
        };

        let parent_a = ClassInfo {
            name: "A".to_string(),
            template_params: vec!["U".to_string()],
            ..ClassInfo::default()
        };

        let mut active = HashMap::new();
        active.insert("T".to_string(), "Foo".to_string());

        let subs = build_substitution_map(&current_b, &parent_a, &active);
        assert_eq!(subs.get("U").unwrap(), "Foo");
    }

    #[test]
    fn test_short_name() {
        assert_eq!(short_name("Collection"), "Collection");
        assert_eq!(short_name("Illuminate\\Support\\Collection"), "Collection");
        assert_eq!(short_name("\\Collection"), "Collection");
    }

    #[test]
    fn test_apply_substitution_to_method_modifies_return_and_params() {
        let mut subs = HashMap::new();
        subs.insert("TValue".to_string(), "Language".to_string());
        subs.insert("TKey".to_string(), "int".to_string());

        let mut method = MethodInfo {
            name: "first".to_string(),
            parameters: vec![crate::types::ParameterInfo {
                name: "$key".to_string(),
                is_required: false,
                type_hint: Some("TKey".to_string()),
                is_variadic: false,
                is_reference: false,
            }],
            return_type: Some("TValue".to_string()),
            is_static: false,
            visibility: Visibility::Public,
            conditional_return: None,
            is_deprecated: false,
        };

        apply_substitution_to_method(&mut method, &subs);

        assert_eq!(method.return_type.as_deref(), Some("Language"));
        assert_eq!(method.parameters[0].type_hint.as_deref(), Some("int"));
    }
}
