/// Foreach and destructuring variable type resolution.
///
/// This submodule handles resolving types for variables that appear as:
///
///   - **Foreach value/key variables:** `foreach ($items as $key => $item)`
///     where the iterated expression has a generic iterable type annotation.
///   - **Array/list destructuring:** `[$a, $b] = getUsers()` or
///     `['name' => $name] = $data` where the RHS has a generic iterable
///     or array shape type annotation.
///
/// These functions are self-contained: they receive a [`VarResolutionCtx`]
/// and push resolved [`ResolvedType`] values into a results vector.
use std::sync::Arc;

use crate::php_type::PhpType;
use crate::types::{ClassInfo, ResolvedType};
use crate::util::short_name;

use crate::completion::resolver::VarResolutionCtx;

/// Resolve an expression's structured type via the unified pipeline.
///
/// Wraps `resolve_rhs_expression` + `types_joined` into a single
/// `Option<PhpType>`.  Returns `None` when the unified pipeline
/// produces no results or an empty type string.
pub(crate) fn resolve_expression_type<'b>(
    expr: &'b mago_syntax::ast::Expression<'b>,
    ctx: &VarResolutionCtx<'_>,
) -> Option<PhpType> {
    let resolved = super::rhs_resolution::resolve_rhs_expression(expr, ctx);
    if resolved.is_empty() {
        return None;
    }
    Some(ResolvedType::types_joined(&resolved))
}

// ─── Helpers ────────────────────────────────────────────────────────

// ─── Foreach Resolution ─────────────────────────────────────────────

/// Known interface/class names whose generic parameters describe
/// iteration types in PHP's `foreach`.
const ITERABLE_IFACE_NAMES: &[&str] = &[
    "Iterator",
    "IteratorAggregate",
    "Traversable",
    "ArrayAccess",
    "Enumerable",
];

/// Extract the iterable **value** (element) type from a class's generic
/// annotations.
///
/// When a collection class like `PaymentOptionLocaleCollection` has
/// `@extends Collection<int, PaymentOptionLocale>` or
/// `@implements IteratorAggregate<int, PaymentOptionLocale>`, this
/// function returns `Some("PaymentOptionLocale")`.
///
/// Checks (in order of priority):
/// 1. `implements_generics` for known iterable interfaces
/// 2. `extends_generics` for any parent with generic type args
///
/// Returns `None` when no generic iterable annotation is found or
/// when the element type is a scalar (scalars have no completable
/// members).
pub(in crate::completion) fn extract_iterable_element_type_from_class(
    class: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> Option<PhpType> {
    // 1. Check implements_generics for known iterable interfaces.
    for (name, args) in &class.implements_generics {
        let short = short_name(name);
        if ITERABLE_IFACE_NAMES.contains(&short) && !args.is_empty() {
            let value = args.last().unwrap();
            if !value.is_scalar() {
                return Some(value.clone());
            }
        }
    }

    // 1b. Check implements_generics for interfaces that transitively
    //     extend a known iterable interface (e.g. `TypedCollection`
    //     extends `IteratorAggregate`).
    for (name, args) in &class.implements_generics {
        let short = short_name(name);
        if !ITERABLE_IFACE_NAMES.contains(&short)
            && !args.is_empty()
            && let Some(iface) = class_loader(name)
            && is_transitive_iterable(&iface, class_loader)
        {
            let value = args.last().unwrap();
            if !value.is_scalar() {
                return Some(value.clone());
            }
        }
    }

    // 2. Check extends_generics — common for collection subclasses
    //    like `@extends Collection<int, User>`.
    for (_, args) in &class.extends_generics {
        if !args.is_empty() {
            let value = args.last().unwrap();
            if !value.is_scalar() {
                return Some(value.clone());
            }
        }
    }

    None
}

/// Check whether an interface transitively extends a known iterable
/// interface (e.g. `TypedCollection extends IteratorAggregate`).
fn is_transitive_iterable(
    iface: &ClassInfo,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> bool {
    // Check direct interfaces.
    for parent in &iface.interfaces {
        let s = short_name(parent);
        if ITERABLE_IFACE_NAMES.contains(&s) {
            return true;
        }
    }
    // Check extends_generics for the interface-extends-interface pattern.
    for (name, _) in &iface.extends_generics {
        let s = short_name(name);
        if ITERABLE_IFACE_NAMES.contains(&s) {
            return true;
        }
    }
    // Check parent class (interfaces use `parent_class` for extends).
    if let Some(ref parent_name) = iface.parent_class {
        let s = short_name(parent_name);
        if ITERABLE_IFACE_NAMES.contains(&s) {
            return true;
        }
        if let Some(parent) = class_loader(parent_name) {
            return is_transitive_iterable(&parent, class_loader);
        }
    }
    false
}
