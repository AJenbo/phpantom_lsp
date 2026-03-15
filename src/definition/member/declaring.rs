//! Declaration walk helpers for member definition resolution.
//!
//! These functions walk the class inheritance chain (parent classes,
//! traits, interfaces, mixins) to find the class that actually declares
//! a given member.  They are used by `resolve_member_definition_with`
//! to locate the source file and position of the member's declaration.

use crate::Backend;
use crate::types::*;
use std::sync::Arc;

use super::MemberAccessHint;

impl Backend {
    /// Resolve a trait `as` alias on a class.
    ///
    /// If `member_name` matches a trait alias declared on the class, returns
    /// the original method name and (optionally) the source trait name.
    /// Otherwise returns `member_name` unchanged with no trait hint.
    pub(in crate::definition) fn resolve_trait_alias(
        class: &ClassInfo,
        member_name: &str,
    ) -> (String, Option<String>) {
        for alias in &class.trait_aliases {
            if alias.alias.as_deref() == Some(member_name) {
                return (alias.method_name.clone(), alias.trait_name.clone());
            }
        }
        (member_name.to_string(), None)
    }

    /// Walk up the inheritance chain to find the class that actually declares
    /// the given member and the FQN (or best-known name) used to load it.
    ///
    /// Returns `Some((ClassInfo, fqn))` of the declaring class, or `None` if
    /// the member cannot be found in any ancestor.  The `fqn` is the name
    /// that was passed to `class_loader` to obtain the `ClassInfo`, which is
    /// a fully-qualified name for parents and traits.  For the class itself
    /// (when the member is declared directly), the FQN is reconstructed
    /// from `file_namespace` + `name` when a namespace is available so
    /// that `find_class_file_content` can disambiguate classes that share
    /// the same short name (e.g. `Eloquent\Builder` vs `Query\Builder`).
    pub(in crate::definition) fn find_declaring_class(
        class: &ClassInfo,
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    ) -> Option<(ClassInfo, String)> {
        // Check if this class directly declares the member.
        if Self::classify_member(class, member_name, MemberAccessHint::Unknown).is_some() {
            let fqn = match &class.file_namespace {
                Some(ns) if !ns.is_empty() => format!("{}\\{}", ns, class.name),
                _ => class.name.clone(),
            };
            return Some((class.clone(), fqn));
        }

        // Check traits used by this class.
        if let Some(found) =
            Self::find_declaring_in_traits(&class.used_traits, member_name, class_loader, 0)
        {
            return Some(found);
        }

        // Walk up the parent chain.
        let mut current = class.clone();
        for _ in 0..MAX_INHERITANCE_DEPTH {
            let parent_name = match current.parent_class.as_ref() {
                Some(name) => name.clone(),
                None => break,
            };
            let parent = match class_loader(&parent_name) {
                Some(p) => Arc::unwrap_or_clone(p),
                None => break,
            };
            if Self::classify_member(&parent, member_name, MemberAccessHint::Unknown).is_some() {
                return Some((parent, parent_name));
            }
            // Check traits used by the parent class.
            if let Some(found) =
                Self::find_declaring_in_traits(&parent.used_traits, member_name, class_loader, 0)
            {
                return Some(found);
            }
            current = parent;
        }

        // Check implemented interfaces (own + from parents).
        // Interfaces can declare `@method` / `@property` / `@property-read`
        // tags that should be resolvable via go-to-definition.
        {
            let mut all_iface_names: Vec<String> = class.interfaces.clone();
            let mut iface_current = class.clone();
            for _ in 0..MAX_INHERITANCE_DEPTH {
                let parent_name = match iface_current.parent_class.as_ref() {
                    Some(name) => name.clone(),
                    None => break,
                };
                let parent = match class_loader(&parent_name) {
                    Some(p) => Arc::unwrap_or_clone(p),
                    None => break,
                };
                for iface in &parent.interfaces {
                    if !all_iface_names.contains(iface) {
                        all_iface_names.push(iface.clone());
                    }
                }
                iface_current = parent;
            }
            for iface_name in &all_iface_names {
                if let Some(iface) = class_loader(iface_name).map(Arc::unwrap_or_clone) {
                    if Self::classify_member(&iface, member_name, MemberAccessHint::Unknown)
                        .is_some()
                    {
                        return Some((iface, iface_name.clone()));
                    }
                    // Walk the interface's own extends chain (interfaces
                    // stored in `parent_class` or `interfaces`).
                    let mut iface_ancestor = iface.clone();
                    for _ in 0..MAX_INHERITANCE_DEPTH {
                        for parent_iface in &iface_ancestor.interfaces {
                            if let Some(pi) = class_loader(parent_iface).map(Arc::unwrap_or_clone)
                                && Self::classify_member(
                                    &pi,
                                    member_name,
                                    MemberAccessHint::Unknown,
                                )
                                .is_some()
                            {
                                return Some((pi, parent_iface.clone()));
                            }
                        }
                        match iface_ancestor.parent_class.as_ref() {
                            Some(pn) => match class_loader(pn) {
                                Some(p) => iface_ancestor = Arc::unwrap_or_clone(p),
                                None => break,
                            },
                            None => break,
                        }
                    }
                }
            }
        }

        // Check @mixin classes — these have the lowest precedence.
        if let Some(found) =
            Self::find_declaring_in_mixins(&class.mixins, member_name, class_loader, 0)
        {
            return Some(found);
        }

        // Also check @mixin classes declared on ancestor classes.
        // e.g. `User extends Model` where `Model` has `@mixin Builder`.
        let mut ancestor = class.clone();
        for _ in 0..MAX_INHERITANCE_DEPTH {
            let parent_name = match ancestor.parent_class.as_ref() {
                Some(name) => name.clone(),
                None => break,
            };
            let parent = match class_loader(&parent_name) {
                Some(p) => Arc::unwrap_or_clone(p),
                None => break,
            };
            if !parent.mixins.is_empty()
                && let Some(found) =
                    Self::find_declaring_in_mixins(&parent.mixins, member_name, class_loader, 0)
            {
                return Some(found);
            }
            ancestor = parent;
        }

        None
    }

    /// Search through a list of trait names for one that declares `member_name`.
    ///
    /// Traits can themselves `use` other traits, so this recurses up to a
    /// depth limit to handle trait composition.
    ///
    /// Returns `(ClassInfo, fqn)` where `fqn` is the fully-qualified name
    /// that was used to load the declaring class from `class_loader`.
    pub(in crate::definition) fn find_declaring_in_traits(
        trait_names: &[String],
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        depth: usize,
    ) -> Option<(ClassInfo, String)> {
        if depth > MAX_TRAIT_DEPTH as usize {
            return None;
        }

        for trait_name in trait_names {
            let trait_info = if let Some(t) = class_loader(trait_name) {
                Arc::unwrap_or_clone(t)
            } else {
                continue;
            };
            if Self::classify_member(&trait_info, member_name, MemberAccessHint::Unknown).is_some()
            {
                return Some((trait_info, trait_name.clone()));
            }
            // Recurse into traits used by this trait.
            if let Some(found) = Self::find_declaring_in_traits(
                &trait_info.used_traits,
                member_name,
                class_loader,
                depth + 1,
            ) {
                return Some(found);
            }
            // Walk the parent_class (extends) chain so that interface
            // inheritance is resolved.  For example, BackedEnum extends
            // UnitEnum — looking up `cases` on BackedEnum should find
            // the declaring UnitEnum interface.
            let mut current = trait_info;
            let mut parent_depth = depth;
            while let Some(ref parent_name) = current.parent_class {
                parent_depth += 1;
                if parent_depth > MAX_TRAIT_DEPTH as usize {
                    break;
                }
                let parent = if let Some(p) = class_loader(parent_name) {
                    Arc::unwrap_or_clone(p)
                } else {
                    break;
                };
                if Self::classify_member(&parent, member_name, MemberAccessHint::Unknown).is_some()
                {
                    return Some((parent, parent_name.clone()));
                }
                if let Some(found) = Self::find_declaring_in_traits(
                    &parent.used_traits,
                    member_name,
                    class_loader,
                    parent_depth + 1,
                ) {
                    return Some(found);
                }
                current = parent;
            }
        }

        None
    }

    /// Search through `@mixin` class names for one that declares `member_name`.
    ///
    /// Mixin classes are resolved with their full inheritance chain (parent
    /// classes, traits) so that inherited members are found.  Only public
    /// members are considered since mixins proxy via magic methods.
    /// Mixin classes can themselves declare `@mixin`, so this recurses up
    /// to a depth limit.
    ///
    /// Returns `(ClassInfo, fqn)` where `fqn` is the fully-qualified name
    /// that was used to load the declaring class from `class_loader`.
    pub(in crate::definition) fn find_declaring_in_mixins(
        mixin_names: &[String],
        member_name: &str,
        class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
        depth: usize,
    ) -> Option<(ClassInfo, String)> {
        if depth > MAX_MIXIN_DEPTH as usize {
            return None;
        }

        for mixin_name in mixin_names {
            let mixin_class = if let Some(c) = class_loader(mixin_name) {
                Arc::unwrap_or_clone(c)
            } else {
                continue;
            };

            // Try to find the declaring class within the mixin's own
            // hierarchy (itself, its traits, its parents).
            if let Some((declaring_class, fqn)) =
                Self::find_declaring_class(&mixin_class, member_name, class_loader)
            {
                // When find_declaring_class finds the member directly on
                // the mixin class, it returns the short name (e.g.
                // "Builder") because ClassInfo.name is always short.
                // Replace it with the fully-qualified mixin_name so that
                // find_class_file_content can disambiguate classes that
                // share the same short name (e.g. Eloquent\Builder vs
                // Query\Builder).
                if !fqn.contains('\\') && fqn == mixin_class.name {
                    return Some((declaring_class, mixin_name.clone()));
                }
                return Some((declaring_class, fqn));
            }

            // Recurse into mixins declared by this mixin class.
            if !mixin_class.mixins.is_empty()
                && let Some(found) = Self::find_declaring_in_mixins(
                    &mixin_class.mixins,
                    member_name,
                    class_loader,
                    depth + 1,
                )
            {
                return Some(found);
            }
        }

        None
    }
}
