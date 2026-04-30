//! Centralized stub patch system for phpstorm-stubs deficiencies.
//!
//! The embedded [phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs)
//! sometimes lack `@template` annotations or have incomplete generic
//! interface declarations. We solve this by patching the parsed
//! [`FunctionInfo`] / [`ClassInfo`] at load time.
//!
//! This module provides two entry points:
//!
//! - [`apply_function_stub_patches`]: patches a freshly-parsed `FunctionInfo`
//!   (called from `find_or_load_function` after stub parsing).
//! - [`apply_class_stub_patches`]: patches a freshly-parsed `ClassInfo`
//!   (called from `parse_and_cache_content_versioned` for stub URIs).
//!
//! ## When to add a patch here vs. hardcoded logic elsewhere
//!
//! If the correct behaviour can be expressed with `@template` / `@return` /
//! `@implements` annotations (i.e. PHPStan's own stubs already have the
//! fix), it belongs here as a `FunctionInfo` or `ClassInfo` patch.  If the
//! behaviour requires inspecting call-site argument *values* at resolution
//! time (e.g. `array_map`'s callback return type), it must stay as hardcoded
//! logic in `rhs_resolution.rs` / `raw_type_inference.rs`.
//!
//! ## Patch inventory
//!
//! ### Function patches
//!
//! None. All function patches have been upstreamed to phpstorm-stubs.
//!
//! ### Class patches
//!
//! 1. **`WeakMap`** -- phpstorm-stubs have `@template TKey of object`,
//!    `@template TValue`, `@template-implements IteratorAggregate<TKey, TValue>`
//!    but are still missing `@template-implements ArrayAccess<TKey, TValue>`.
//!
//! 2. **`IteratorIterator`** -- phpstorm-stubs lack `@template` and `@mixin`.
//!    PHPStan adds `@template TKey`, `@template TValue`,
//!    `@template TIterator of Traversable<TKey, TValue>`,
//!    `@implements OuterIterator<TKey, TValue>`,
//!    `@mixin TIterator`.  The `@mixin` makes methods from the wrapped
//!    iterator available on the wrapper.
//!    PHPStan ref: `stubs/iterable.stub`
//!
//! ## Removing patches
//!
//! When phpstorm-stubs gains proper annotations for a patched symbol,
//! delete the corresponding patch function here and remove its dispatch
//! from the entry point.  Run the test suite to verify that the stub's
//! own annotations produce the same result.

use crate::atom::atom;
use crate::php_type::PhpType;
use crate::types::{ClassInfo, FunctionInfo};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Function patches
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Apply all registered stub patches to a freshly-parsed function.
///
/// Called from [`find_or_load_function`](crate::resolution) after a
/// `FunctionInfo` is parsed from embedded phpstorm-stubs, before it is
/// cached in `global_functions`.  Only functions with known deficiencies
/// are patched; all others pass through unchanged.
pub fn apply_function_stub_patches(_func: &mut FunctionInfo) {
    // All function patches have been upstreamed to phpstorm-stubs.
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Class patches
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Apply all registered stub patches to a freshly-parsed class.
///
/// Called from [`parse_and_cache_content_versioned`](crate::resolution)
/// after a `ClassInfo` is parsed from embedded phpstorm-stubs, before it
/// is cached in `ast_map` and `fqn_index`.  Only classes with known
/// deficiencies are patched; all others pass through unchanged.
///
/// This is the class-level counterpart of [`apply_function_stub_patches`].
pub fn apply_class_stub_patches(class: &mut ClassInfo) {
    match class.name.as_str() {
        "WeakMap" => patch_weak_map(class),
        "IteratorIterator" => patch_iterator_iterator(class),
        "NoRewindIterator" => patch_no_rewind_iterator(class),
        _ => {}
    }
}

/// Add `@implements ArrayAccess<TKey, TValue>` for WeakMap.
///
/// Upstream phpstorm-stubs have `@template TKey of object`, `@template TValue`,
/// and `@template-implements IteratorAggregate<TKey, TValue>`, but are still
/// missing `@template-implements ArrayAccess<TKey, TValue>`.
fn patch_weak_map(class: &mut ClassInfo) {
    add_implements_generics(class, "ArrayAccess", &["TKey", "TValue"]);
}

/// Add `@template TKey`, `@template TValue`,
/// `@template TIterator of Traversable<TKey, TValue>`,
/// `@implements OuterIterator<TKey, TValue>`,
/// `@mixin TIterator`.
///
/// PHPStan ref: `stubs/iterable.stub`
fn patch_iterator_iterator(class: &mut ClassInfo) {
    if !class.template_params.is_empty() {
        return;
    }
    add_templates(class, &[("TKey", None), ("TValue", None)]);
    // TIterator has a complex bound `Traversable<TKey, TValue>` — add it
    // manually since `add_templates` only handles simple string bounds.
    let t_iter = atom("TIterator");
    if !class.template_params.contains(&t_iter) {
        class.template_params.push(t_iter);
    }
    class
        .template_param_bounds
        .entry(atom("TIterator"))
        .or_insert_with(|| {
            PhpType::Generic(
                "Traversable".to_string(),
                vec![
                    PhpType::Named("TKey".to_string()),
                    PhpType::Named("TValue".to_string()),
                ],
            )
        });
    add_implements_generics(class, "OuterIterator", &["TKey", "TValue"]);
    // Add @mixin TIterator so that methods from the wrapped iterator
    // are available on the wrapper.
    if !class.mixins.contains(&t_iter) {
        class.mixins.push(t_iter);
    }
    // Patch the constructor: add template binding TIterator → $iterator
    // so that `new IteratorIterator(new Subject())` infers TIterator = Subject.
    if let Some(ctor_idx) = class
        .methods
        .iter()
        .position(|m| m.name.as_str() == "__construct")
    {
        let mut ctor = (*class.methods[ctor_idx]).clone();
        let binding = (atom("TIterator"), atom("$iterator"));
        if !ctor.template_bindings.iter().any(|(t, _)| t == &binding.0) {
            ctor.template_bindings.push(binding);
        }
        // Update the parameter type hint from Traversable to TIterator
        // so that classify_template_binding recognises a Direct binding.
        if let Some(param) = ctor.parameters.iter_mut().find(|p| p.name == "$iterator") {
            param.type_hint = Some(PhpType::Named("TIterator".to_string()));
        }
        class.methods.make_mut()[ctor_idx] = std::sync::Arc::new(ctor);
    }
}

/// Add `@template TKey`, `@template TValue`,
/// `@template TIterator of Iterator<TKey, TValue>`,
/// `@extends IteratorIterator<TKey, TValue, TIterator>`,
/// and constructor template binding `TIterator → $iterator`.
///
/// Without this patch, `new NoRewindIterator(generator())` resolves as
/// bare `NoRewindIterator` without propagating the generator's type params.
fn patch_no_rewind_iterator(class: &mut ClassInfo) {
    if !class.template_params.is_empty() {
        return;
    }
    add_templates(class, &[("TKey", None), ("TValue", None)]);
    let t_iter = atom("TIterator");
    if !class.template_params.contains(&t_iter) {
        class.template_params.push(t_iter);
    }
    class
        .template_param_bounds
        .entry(atom("TIterator"))
        .or_insert_with(|| {
            PhpType::Generic(
                "Iterator".to_string(),
                vec![
                    PhpType::Named("TKey".to_string()),
                    PhpType::Named("TValue".to_string()),
                ],
            )
        });
    // Add @extends generics so inheritance resolves TKey/TValue.
    let iter_iter_atom = atom("IteratorIterator");
    if !class
        .extends_generics
        .iter()
        .any(|(n, _)| *n == iter_iter_atom)
    {
        class.extends_generics.push((
            iter_iter_atom,
            vec![
                PhpType::Named("TKey".to_string()),
                PhpType::Named("TValue".to_string()),
                PhpType::Named("TIterator".to_string()),
            ],
        ));
    }
    // Patch constructor: bind TIterator from the $iterator parameter.
    if let Some(ctor_idx) = class
        .methods
        .iter()
        .position(|m| m.name.as_str() == "__construct")
    {
        let mut ctor = (*class.methods[ctor_idx]).clone();
        let binding = (atom("TIterator"), atom("$iterator"));
        if !ctor.template_bindings.iter().any(|(t, _)| t == &binding.0) {
            ctor.template_bindings.push(binding);
        }
        if let Some(param) = ctor.parameters.iter_mut().find(|p| p.name == "$iterator") {
            param.type_hint = Some(PhpType::Named("TIterator".to_string()));
        }
        class.methods.make_mut()[ctor_idx] = std::sync::Arc::new(ctor);
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Helpers
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Add template parameters with optional upper bounds.
///
/// Each entry is `(param_name, optional_bound)`.  The bound, if present,
/// is parsed into a `PhpType` and stored in `template_param_bounds`.
fn add_templates(class: &mut ClassInfo, templates: &[(&str, Option<&str>)]) {
    for &(name, bound) in templates {
        let param = atom(name);
        if !class.template_params.contains(&param) {
            class.template_params.push(param);
        }
        if let Some(bound_str) = bound {
            class
                .template_param_bounds
                .entry(atom(name))
                .or_insert_with(|| PhpType::parse(bound_str));
        }
    }
}

/// Add an `@implements InterfaceName<Param1, Param2, ...>` entry where
/// all type arguments are template parameter names (the common case).
fn add_implements_generics(class: &mut ClassInfo, iface_name: &str, params: &[&str]) {
    let args: Vec<PhpType> = params
        .iter()
        .map(|p| PhpType::Named((*p).to_string()))
        .collect();
    add_implements_generics_typed(class, iface_name, &args);
}

/// Add an `@implements InterfaceName<Type1, Type2, ...>` entry with
/// pre-built `PhpType` arguments.
fn add_implements_generics_typed(class: &mut ClassInfo, iface_name: &str, args: &[PhpType]) {
    // Don't add duplicate entries.
    if class
        .implements_generics
        .iter()
        .any(|(n, _)| n.as_str() == iface_name)
    {
        return;
    }
    class
        .implements_generics
        .push((atom(iface_name), args.to_vec()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atom::atom;
    use crate::php_type::PhpType;

    fn empty_class(name: &str) -> ClassInfo {
        ClassInfo {
            name: atom(name),
            ..ClassInfo::default()
        }
    }

    #[test]
    fn weak_map_gets_array_access_generics() {
        let mut class = empty_class("WeakMap");
        apply_class_stub_patches(&mut class);

        assert!(
            class
                .implements_generics
                .iter()
                .any(|(n, args)| n.as_str() == "ArrayAccess"
                    && args.len() == 2
                    && args[0] == PhpType::Named("TKey".to_string())
                    && args[1] == PhpType::Named("TValue".to_string())),
            "Should have @implements ArrayAccess<TKey, TValue>"
        );
    }

    #[test]
    fn unrelated_class_not_patched() {
        let mut class = empty_class("MyApp\\Foo");
        let original_params = class.template_params.clone();

        apply_class_stub_patches(&mut class);

        assert_eq!(class.template_params, original_params);
        assert!(class.implements_generics.is_empty());
    }

    #[test]
    fn iterator_iterator_gets_templates_and_mixin() {
        let mut class = empty_class("IteratorIterator");
        apply_class_stub_patches(&mut class);

        assert_eq!(
            class.template_params,
            vec![atom("TKey"), atom("TValue"), atom("TIterator")]
        );
        assert!(
            class
                .implements_generics
                .iter()
                .any(|(n, args)| n.as_str() == "OuterIterator" && args.len() == 2),
            "Should have @implements OuterIterator<TKey, TValue>"
        );
        assert_eq!(class.mixins, vec![atom("TIterator")]);
        assert!(
            class.template_param_bounds.contains_key(&atom("TIterator")),
            "TIterator should have a bound"
        );
    }
}
