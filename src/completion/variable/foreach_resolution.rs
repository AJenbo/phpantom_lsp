use mago_span::HasSpan;
use mago_syntax::ast::*;
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

use crate::docblock;
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

// ─── Destructuring Resolution ───────────────────────────────────────

/// Check whether the target variable appears inside an array/list
/// destructuring LHS and, if so, resolve its type from the RHS's
/// generic element type or array shape entry.
///
/// Supported patterns:
///   - `[$a, $b] = getUsers()`           — function call RHS (generic)
///   - `list($a, $b) = $users`           — variable RHS with `@var`/`@param`
///   - `[$a, $b] = $this->m()`           — method/static-method call RHS
///   - `['user' => $p] = $data`          — named key from array shape
///   - `[0 => $first, 1 => $second] = $data` — numeric key from array shape
///
/// When the RHS type is an array shape (`array{key: Type, …}`), the
/// destructured variable's key is matched against the shape entries.
/// For positional (value-only) elements, the 0-based index is used as
/// the key.  Falls back to `PhpType::extract_value_type` for generic
/// iterable types (`list<User>`, `array<int, User>`, `User[]`).
pub(in crate::completion) fn try_resolve_destructured_type<'b>(
    assignment: &'b Assignment<'b>,
    ctx: &VarResolutionCtx<'_>,
    results: &mut Vec<ResolvedType>,
    conditional: bool,
) {
    // ── 1. Collect the elements from the LHS ────────────────────────
    let elements = match assignment.lhs {
        Expression::Array(arr) => &arr.elements,
        Expression::List(list) => &list.elements,
        _ => return,
    };

    // ── 2. Find our target variable and extract its destructuring key
    //
    // For `KeyValue` elements like `'user' => $person`, extract the
    // string/integer key.  For positional `Value` elements, track
    // the 0-based index so we can look up positional shape entries.
    let var_name = ctx.var_name;
    let mut shape_key: Option<String> = None;
    let mut found = false;
    let mut positional_index: usize = 0;

    for elem in elements.iter() {
        match elem {
            ArrayElement::KeyValue(kv) => {
                if let Expression::Variable(Variable::Direct(dv)) = kv.value
                    && dv.name == var_name
                {
                    found = true;
                    // Extract the key from the LHS expression.
                    shape_key = extract_destructuring_key(kv.key);
                    break;
                }
            }
            ArrayElement::Value(val) => {
                if let Expression::Variable(Variable::Direct(dv)) = val.value
                    && dv.name == var_name
                {
                    found = true;
                    // Use the positional index as the shape key.
                    shape_key = Some(positional_index.to_string());
                    break;
                }
                positional_index += 1;
            }
            _ => {}
        }
    }
    if !found {
        return;
    }

    let current_class_name: &str = &ctx.current_class.name;
    let all_classes = ctx.all_classes;
    let content = ctx.content;
    let class_loader = ctx.class_loader;

    // ── 3. Try inline `/** @var … */` annotation ────────────────────
    // Handles both:
    //   `/** @var list<User> */`             (no variable name)
    //   `/** @var array{user: User} $data */` (with variable name)
    let stmt_offset = assignment.span().start.offset as usize;
    if let Some((var_type, _var_name_opt)) =
        docblock::find_inline_var_docblock(content, stmt_offset)
    {
        if let Some(ref key) = shape_key
            && let Some(entry_type) = var_type.shape_value_type(key)
        {
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                entry_type,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                let resolved_types =
                    ResolvedType::from_classes_with_hint(resolved, entry_type.clone());
                if !conditional {
                    results.clear();
                }
                ResolvedType::extend_unique(results, resolved_types);
                return;
            }
        }

        if let Some(element_type) = var_type.extract_value_type(true) {
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                element_type,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                let resolved_types =
                    ResolvedType::from_classes_with_hint(resolved, element_type.clone());
                if !conditional {
                    results.clear();
                }
                ResolvedType::extend_unique(results, resolved_types);
                return;
            }
        }
    }

    // ── 4. Try to resolve the iterable type from the RHS ────────────
    let raw_type: Option<PhpType> = resolve_expression_type(assignment.rhs, ctx);

    // ── Expand type aliases before shape/generic extraction ─────────
    // Same as the foreach value/key paths: when the raw type is a type
    // alias (e.g. `UserData` defined via `@phpstan-type`), expand it so
    // that `extract_array_shape_value_type` and
    // `PhpType::extract_value_type` can see the underlying type.
    let raw_type = raw_type.map(|rt| {
        crate::completion::type_resolution::resolve_type_alias_typed(
            &rt,
            current_class_name,
            all_classes,
            class_loader,
        )
        .unwrap_or(rt)
    });

    if let Some(ref raw) = raw_type {
        // First try array shape lookup with the destructured key.
        if let Some(ref key) = shape_key
            && let Some(entry_type) = raw.shape_value_type(key)
        {
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                entry_type,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                let resolved_types =
                    ResolvedType::from_classes_with_hint(resolved, entry_type.clone());
                if !conditional {
                    results.clear();
                }
                ResolvedType::extend_unique(results, resolved_types);
                return;
            }
        }

        // Fall back to generic element type extraction.
        if let Some(element_type) = raw.extract_value_type(true) {
            let resolved = crate::completion::type_resolution::type_hint_to_classes_typed(
                element_type,
                current_class_name,
                all_classes,
                class_loader,
            );
            if !resolved.is_empty() {
                let resolved_types =
                    ResolvedType::from_classes_with_hint(resolved, element_type.clone());
                if !conditional {
                    results.clear();
                }
                ResolvedType::extend_unique(results, resolved_types);
            }
        }
    }
}

/// Extract a string key from a destructuring key expression.
///
/// Handles string literals (`'user'`, `"user"`) and integer literals
/// (`0`, `1`).  Returns `None` for dynamic or unsupported key
/// expressions.
fn extract_destructuring_key(key_expr: &Expression<'_>) -> Option<String> {
    match key_expr {
        Expression::Literal(Literal::String(lit_str)) => {
            // `value` strips the quotes; fall back to `raw` trimmed.
            lit_str
                .value
                .map(|v| v.to_string())
                .or_else(|| crate::util::unquote_php_string(lit_str.raw).map(|s| s.to_string()))
        }
        Expression::Literal(Literal::Integer(lit_int)) => Some(lit_int.raw.to_string()),
        _ => None,
    }
}
