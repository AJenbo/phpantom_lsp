//! PHPDoc virtual member provider.
//!
//! Extracts `@method`, `@property` / `@property-read` / `@property-write`,
//! and `@mixin` tags from the class-level docblock and presents them as
//! virtual members.  This is the second-highest-priority virtual member
//! provider: framework providers (e.g. Laravel) take precedence, but
//! PHPDoc-sourced members beat all other virtual member sources.
//!
//! Within this provider, `@method` and `@property` tags take precedence
//! over `@mixin` members: if a class declares both `@property int $id`
//! and `@mixin SomeClass` where `SomeClass` also has an `$id` property,
//! the `@property` tag wins.
//!
//! Previously `@method` / `@property` and `@mixin` were handled by two
//! separate providers (`PHPDocProvider` and `MixinProvider`).  Since both
//! are driven by PHPDoc tags, they are now unified into a single provider
//! with internal precedence rules.

use std::collections::HashSet;
use std::sync::Arc;

use crate::docblock;
use crate::types::{
    ClassInfo, ConstantInfo, MAX_INHERITANCE_DEPTH, MAX_MIXIN_DEPTH, MethodInfo, PropertyInfo,
    Visibility,
};

/// Tracks member names already seen during mixin collection.
///
/// Passed through [`collect_mixin_members`] (including recursive calls)
/// so that every addition is checked in O(1) instead of scanning the
/// accumulated vectors and base class members.
struct MixinDedup {
    /// Method names from the base class + accumulated virtual methods.
    methods: HashSet<String>,
    /// Property names from the base class + accumulated virtual properties.
    properties: HashSet<String>,
    /// Constant names from the base class + accumulated virtual constants.
    constants: HashSet<String>,
}

use super::{VirtualMemberProvider, VirtualMembers};

/// Virtual member provider for `@method`, `@property`, and `@mixin` docblock tags.
///
/// When a class declares `@method` or `@property` tags in its class-level
/// docblock, those tags describe magic members accessible via `__call`,
/// `__get`, and `__set`.  When a class declares `@mixin ClassName`, all
/// public members of `ClassName` (and its inheritance chain) become
/// available via magic methods.
///
/// Resolution order within this provider:
/// 1. `@method` and `@property` tags (highest precedence)
/// 2. `@mixin` class members (lower precedence, never overwrite tags)
///
/// Mixins are inherited: if `User extends Model` and `Model` has
/// `@mixin Builder`, then `User` also gains Builder's public members.
/// The provider walks the parent chain to collect mixin declarations
/// from ancestors.
///
/// Mixin classes can themselves declare `@mixin`, so the provider
/// recurses up to [`MAX_MIXIN_DEPTH`] levels.
pub struct PHPDocProvider;

impl VirtualMemberProvider for PHPDocProvider {
    /// Returns `true` if the class has a non-empty class-level docblock
    /// or declares `@mixin` tags (directly or via ancestors).
    ///
    /// This is a cheap pre-check. No parsing is performed.
    fn applies_to(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> bool {
        // Has a non-empty docblock with potential @method/@property tags.
        if class.class_docblock.as_ref().is_some_and(|d| !d.is_empty()) {
            return true;
        }

        // Has used traits that might have @method/@property tags.
        for trait_name in &class.used_traits {
            if let Some(trait_info) = class_loader(trait_name)
                && trait_info
                    .class_docblock
                    .as_ref()
                    .is_some_and(|d| !d.is_empty())
            {
                return true;
            }
        }

        // Has direct @mixin declarations.
        if !class.mixins.is_empty() {
            return true;
        }

        // Walk the parent chain to check for ancestor mixins or docblocks
        // with @method/@property tags.
        let mut current = class.clone();
        let mut depth = 0u32;
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
            if !parent.mixins.is_empty() {
                return true;
            }
            if parent
                .class_docblock
                .as_ref()
                .is_some_and(|d| !d.is_empty())
            {
                return true;
            }
            current = Arc::unwrap_or_clone(parent);
        }

        false
    }

    /// Parse `@method`, `@property`, and `@mixin` tags from the class.
    ///
    /// Uses the existing [`docblock::extract_method_tags`] and
    /// [`docblock::extract_property_tags`] functions for tag parsing.
    /// Then collects public members from `@mixin` classes.  Within the
    /// provider, `@method` / `@property` tags take precedence over
    /// `@mixin` members.
    fn provide(
        &self,
        class: &ClassInfo,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        _cache: Option<&super::ResolvedClassCache>,
    ) -> VirtualMembers {
        let mut methods = Vec::new();
        let mut properties = Vec::new();
        let mut constants = Vec::new();

        // Dedup sets for O(1) membership checks.  Seeded from the
        // base-resolved class members (real + inherited) and updated
        // as virtual members are collected.
        //
        // `seen_props` is NOT seeded from existing class properties.
        // Phase 1 (`@property` tags) always emits its properties so
        // that `merge_virtual_members` can compare type specificity
        // and keep the most specific type (e.g. `array<string>` from
        // `@property` beats bare `array` from `$casts`).  After
        // phase 1 emits, names are added to `seen_props` to prevent
        // lower-priority sources (trait tags, parent tags, `@mixin`
        // members) from overriding them.
        let mut seen_methods: HashSet<String> =
            class.methods.iter().map(|m| m.name.clone()).collect();
        let mut seen_props: HashSet<String> = HashSet::new();
        let seen_consts: HashSet<String> = class.constants.iter().map(|c| c.name.clone()).collect();

        // ── Phase 1: @method and @property tags (higher precedence) ─────

        if let Some(doc_text) = class.class_docblock.as_deref()
            && !doc_text.is_empty()
        {
            for m in docblock::extract_method_tags(doc_text) {
                seen_methods.insert(m.name.clone());
                methods.push(m);
            }

            for (name, type_str) in docblock::extract_property_tags(doc_text) {
                seen_props.insert(name.clone());
                properties.push(PropertyInfo {
                    name,
                    name_offset: 0,
                    type_hint: if type_str.is_empty() {
                        None
                    } else {
                        Some(type_str)
                    },
                    native_type_hint: None,
                    description: None,
                    is_static: false,
                    visibility: Visibility::Public,
                    deprecation_message: None,
                    deprecated_replacement: None,
                    is_virtual: true,
                });
            }
        }

        // ── Phase 1b: @method and @property tags from used traits ───────
        //
        // When a class uses a trait that declares `@method` or `@property`
        // tags in its docblock, those virtual members should propagate to
        // the consuming class.  Real trait methods are already merged by
        // `merge_traits_into`, but virtual members from docblock tags are
        // not — they only exist as text in the trait's `class_docblock`.
        for trait_name in &class.used_traits {
            let trait_info = if let Some(t) = class_loader(trait_name) {
                t
            } else {
                continue;
            };

            if let Some(doc_text) = trait_info.class_docblock.as_deref()
                && !doc_text.is_empty()
            {
                for m in docblock::extract_method_tags(doc_text) {
                    if seen_methods.insert(m.name.clone()) {
                        methods.push(m);
                    }
                }

                for (name, type_str) in docblock::extract_property_tags(doc_text) {
                    if seen_props.insert(name.clone()) {
                        properties.push(PropertyInfo {
                            name,
                            name_offset: 0,
                            type_hint: if type_str.is_empty() {
                                None
                            } else {
                                Some(type_str)
                            },
                            native_type_hint: None,
                            description: None,
                            is_static: false,
                            visibility: Visibility::Public,
                            deprecation_message: None,
                            deprecated_replacement: None,
                            is_virtual: true,
                        });
                    }
                }
            }
        }

        // ── Phase 1c: @method and @property tags from parent classes ────
        //
        // When a parent class declares `@method` or `@property` tags in
        // its docblock, those virtual members should be visible on child
        // classes.  Real inherited methods are already merged by
        // `resolve_class_with_inheritance`, but virtual members from
        // docblock tags are not — they only exist as text in the parent's
        // `class_docblock`.  Walk the parent chain and collect them.
        {
            let mut current = class.clone();
            let mut depth = 0u32;
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

                if let Some(doc_text) = parent.class_docblock.as_deref()
                    && !doc_text.is_empty()
                {
                    for m in docblock::extract_method_tags(doc_text) {
                        if seen_methods.insert(m.name.clone()) {
                            methods.push(m);
                        }
                    }

                    for (name, type_str) in docblock::extract_property_tags(doc_text) {
                        if seen_props.insert(name.clone()) {
                            properties.push(PropertyInfo {
                                name,
                                name_offset: 0,
                                type_hint: if type_str.is_empty() {
                                    None
                                } else {
                                    Some(type_str)
                                },
                                native_type_hint: None,
                                description: None,
                                is_static: false,
                                visibility: Visibility::Public,
                                deprecation_message: None,
                                deprecated_replacement: None,
                                is_virtual: true,
                            });
                        }
                    }
                }

                current = Arc::unwrap_or_clone(parent);
            }
        }

        // ── Phase 2: @mixin members (lower precedence) ─────────────────

        let mut mixin_dedup = MixinDedup {
            methods: seen_methods,
            properties: seen_props,
            constants: seen_consts,
        };

        // Collect from the class's own mixins.
        collect_mixin_members(
            &class.mixins,
            class_loader,
            &mut methods,
            &mut properties,
            &mut constants,
            0,
            &mut mixin_dedup,
        );

        // Collect from ancestor mixins.
        let mut current = class.clone();
        let mut depth = 0u32;
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
            if !parent.mixins.is_empty() {
                collect_mixin_members(
                    &parent.mixins,
                    class_loader,
                    &mut methods,
                    &mut properties,
                    &mut constants,
                    0,
                    &mut mixin_dedup,
                );
            }
            current = Arc::unwrap_or_clone(parent);
        }

        VirtualMembers {
            methods,
            properties,
            constants,
        }
    }
}

/// Recursively collect public members from mixin classes.
///
/// For each mixin name, loads the class via `class_loader`, resolves its
/// full inheritance chain (via [`crate::inheritance::resolve_class_with_inheritance`]),
/// and adds its public members to the output vectors.  Only members whose
/// names are not already present in `class` (the target class with base
/// resolution already applied) or in the output vectors are added.
/// This means `@method` / `@property` tags collected before this function
/// is called take precedence over mixin members.
///
/// Recurses into mixins declared on the mixin classes themselves, up to
/// [`MAX_MIXIN_DEPTH`] levels.
fn collect_mixin_members(
    mixin_names: &[String],
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    methods: &mut Vec<MethodInfo>,
    properties: &mut Vec<PropertyInfo>,
    constants: &mut Vec<ConstantInfo>,
    depth: u32,
    dedup: &mut MixinDedup,
) {
    if depth > MAX_MIXIN_DEPTH {
        return;
    }

    for mixin_name in mixin_names {
        let mixin_class = if let Some(c) = class_loader(mixin_name) {
            c
        } else {
            continue;
        };

        // Resolve the mixin class with its own inheritance so we see
        // all of its inherited/trait members too.  Use base resolution
        // (not resolve_class_fully) to avoid circular provider calls.
        let resolved_mixin =
            crate::inheritance::resolve_class_with_inheritance(&mixin_class, class_loader);

        // Only merge public members — mixins proxy via magic methods
        // which only expose public API.
        for method in &resolved_mixin.methods {
            if method.visibility != Visibility::Public {
                continue;
            }
            // Skip if the base-resolved class already has this method,
            // or if a previous @method tag or mixin already contributed it.
            if !dedup.methods.insert(method.name.clone()) {
                continue;
            }
            let method = method.clone();
            // `@return $this` / `self` / `static` in mixin methods are
            // left as-is.  When the method is later called on the
            // consuming class, `$this` resolves to the consumer (not the
            // mixin), which is the correct semantic: fluent chains
            // continue with the consumer's full API (own methods + all
            // mixin methods).  In the builder-as-static forwarding path,
            // the substitution map rewrites `$this` to
            // `\Illuminate\Database\Eloquent\Builder<Model>`, so the
            // return type must still be the raw keyword at this stage.
            methods.push(method);
        }

        for property in &resolved_mixin.properties {
            if property.visibility != Visibility::Public {
                continue;
            }
            if !dedup.properties.insert(property.name.clone()) {
                continue;
            }
            properties.push(property.clone());
        }

        for constant in &resolved_mixin.constants {
            if constant.visibility != Visibility::Public {
                continue;
            }
            if !dedup.constants.insert(constant.name.clone()) {
                continue;
            }
            constants.push(constant.clone());
        }

        // Recurse into mixins declared by the mixin class itself.
        if !mixin_class.mixins.is_empty() {
            collect_mixin_members(
                &mixin_class.mixins,
                class_loader,
                methods,
                properties,
                constants,
                depth + 1,
                dedup,
            );
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "phpdoc_tests.rs"]
mod tests;
