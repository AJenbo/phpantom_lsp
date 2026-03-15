//! String conversion utilities and model ancestry helpers.
//!
//! This module contains the case-conversion functions (`camel_to_snake`,
//! `snake_to_camel`, `snake_to_pascal`), the Eloquent Model ancestry
//! check (`extends_eloquent_model`), and go-to-definition helpers that
//! map virtual property names back to their declaring methods
//! (`legacy_accessor_method_name`, `accessor_method_candidates`).

use crate::types::{ClassInfo, MAX_INHERITANCE_DEPTH};
use std::sync::Arc;

use super::ELOQUENT_MODEL_FQN;

/// Walk the parent chain of `class` checking whether any ancestor
/// (including the class itself) satisfies `predicate`.
///
/// This is the shared implementation behind [`extends_eloquent_model`]
/// and `extends_eloquent_factory`.  The predicate receives a class name
/// (without a leading backslash normalisation — callers handle that
/// themselves) and returns `true` when the target base class is found.
pub(in crate::virtual_members::laravel) fn walks_parent_chain(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
    predicate: fn(&str) -> bool,
) -> bool {
    if predicate(&class.name) {
        return true;
    }

    let mut current = class.clone();
    let mut depth = 0u32;
    while let Some(ref parent_name) = current.parent_class {
        depth += 1;
        if depth > MAX_INHERITANCE_DEPTH {
            break;
        }
        if predicate(parent_name) {
            return true;
        }
        match class_loader(parent_name) {
            Some(parent) => {
                if predicate(&parent.name) {
                    return true;
                }
                current = Arc::unwrap_or_clone(parent);
            }
            None => break,
        }
    }

    false
}

/// Determine whether `class_name` is the Eloquent Model base class.
///
/// Checks against the FQN with and without a leading backslash.
pub(in crate::virtual_members::laravel) fn is_eloquent_model(class_name: &str) -> bool {
    class_name == ELOQUENT_MODEL_FQN
}

/// Walk the parent chain of `class` looking for
/// `Illuminate\Database\Eloquent\Model`.
///
/// Returns `true` if the class itself is `Model` or any ancestor is.
pub fn extends_eloquent_model(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    walks_parent_chain(class, class_loader, is_eloquent_model)
}

/// Convert a camelCase or PascalCase string to snake_case.
///
/// Inserts an underscore before each uppercase letter that follows a
/// lowercase letter or digit, and before an uppercase letter that is
/// followed by a lowercase letter when preceded by another uppercase
/// letter (to handle acronyms like `URL` → `u_r_l`).
///
/// `FullName` → `full_name`
/// `firstName` → `first_name`
/// `isAdmin` → `is_admin`
pub(crate) fn camel_to_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                // Insert underscore when: lowercase/digit → uppercase,
                // or uppercase → uppercase followed by lowercase (acronym boundary).
                if prev.is_lowercase() || prev.is_ascii_digit() {
                    result.push('_');
                } else if prev.is_uppercase() {
                    // Check next char for acronym boundary: "URL" + "Name" → "u_r_l_name"
                    if let Some(&next) = chars.get(i + 1)
                        && next.is_lowercase()
                    {
                        result.push('_');
                    }
                }
            }
            for lc in c.to_lowercase() {
                result.push(lc);
            }
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a snake_case string to camelCase.
///
/// `full_name` → `fullName`
/// `avatar_url` → `avatarUrl`
/// `name` → `name`
pub(crate) fn snake_to_camel(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Convert a snake_case string to PascalCase.
///
/// `full_name` → `FullName`
/// `avatar_url` → `AvatarUrl`
/// `name` → `Name`
pub(in crate::virtual_members::laravel) fn snake_to_pascal(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = true;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// Build the legacy accessor method name from a virtual property name.
///
/// `display_name` → `getDisplayNameAttribute`
/// `name` → `getNameAttribute`
pub(crate) fn legacy_accessor_method_name(property_name: &str) -> String {
    let pascal = snake_to_pascal(property_name);
    format!("get{pascal}Attribute")
}

/// Return candidate accessor method names for a virtual property name.
///
/// Go-to-definition uses this to map a snake_case virtual property back
/// to the method that produces it.  Returns both the legacy
/// (`getDisplayNameAttribute`) and modern (`displayName`) forms so the
/// caller can try each one.
pub(crate) fn accessor_method_candidates(property_name: &str) -> Vec<String> {
    vec![
        legacy_accessor_method_name(property_name),
        snake_to_camel(property_name),
    ]
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;
