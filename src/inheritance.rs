/// Class inheritance resolution.
///
/// This module handles merging members from parent classes, traits, and
/// `@mixin` classes into a single `ClassInfo`.  The resulting merged class
/// contains the complete set of members visible on an instance / static
/// access, respecting PHP's precedence rules:
///
///   class own > traits > parent chain > mixins
use crate::Backend;
use crate::types::ClassInfo;
use crate::types::Visibility;

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
        Self::merge_traits_into(&mut merged, &class.used_traits, class_loader, 0);

        // 2. Walk up the `extends` chain and merge parent members.
        let mut current = class.clone();
        let mut depth = 0;
        const MAX_DEPTH: u32 = 20;

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

            // Merge traits used by the parent class as well, so that
            // grandparent-level trait members are visible.
            Self::merge_traits_into(&mut merged, &parent.used_traits, class_loader, 0);

            // Merge parent methods — skip private, skip if child already has one with same name
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
    fn merge_traits_into(
        merged: &mut ClassInfo,
        trait_names: &[String],
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

            // Recursively merge traits used by this trait (trait composition).
            if !trait_info.used_traits.is_empty() {
                Self::merge_traits_into(merged, &trait_info.used_traits, class_loader, depth + 1);
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

            // Merge trait methods — skip if already present
            for method in &trait_info.methods {
                if merged.methods.iter().any(|m| m.name == method.name) {
                    continue;
                }
                merged.methods.push(method.clone());
            }

            // Merge trait properties
            for property in &trait_info.properties {
                if merged.properties.iter().any(|p| p.name == property.name) {
                    continue;
                }
                merged.properties.push(property.clone());
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
