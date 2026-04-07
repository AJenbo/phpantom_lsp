//! Eloquent cast type resolution.
//!
//! This module maps Eloquent `$casts` type strings (e.g. `datetime`,
//! `boolean`, `App\Casts\MoneyCast`) to their corresponding PHP types
//! for virtual property synthesis.  It handles built-in cast strings,
//! `decimal:N` and `datetime:format` variants, custom cast classes
//! (via `get()` return type inspection), enum casts, `Castable`
//! implementations, and `@implements CastsAttributes<TGet, TSet>`
//! fallback resolution.

use crate::php_type::PhpType;
use crate::types::{ClassInfo, ClassLikeKind};
use crate::util::short_name;
use std::sync::Arc;

/// The short name of the `CastsAttributes` interface, used to look up
/// `@implements` generic arguments on custom cast classes.
const CASTS_ATTRIBUTES_SHORT: &str = "CastsAttributes";

/// The fully-qualified name of the `CastsAttributes` interface.
const CASTS_ATTRIBUTES_FQN: &str = "Illuminate\\Contracts\\Database\\Eloquent\\CastsAttributes";

/// Maps Eloquent cast type strings to their corresponding PHP types.
///
/// When a model declares `protected $casts = ['col' => 'datetime']`, the
/// column is treated as `\Carbon\Carbon` in completions.  This table
/// covers all built-in Laravel cast types.
const CAST_TYPE_MAP: &[(&str, &str)] = &[
    ("datetime", "Carbon\\Carbon"),
    ("date", "Carbon\\Carbon"),
    ("timestamp", "int"),
    ("immutable_datetime", "Carbon\\CarbonImmutable"),
    ("immutable_date", "Carbon\\CarbonImmutable"),
    ("boolean", "bool"),
    ("bool", "bool"),
    ("integer", "int"),
    ("int", "int"),
    ("float", "float"),
    ("double", "float"),
    ("real", "float"),
    ("string", "string"),
    ("array", "array"),
    ("json", "array"),
    ("object", "object"),
    ("collection", "Illuminate\\Support\\Collection"),
    ("encrypted", "string"),
    ("encrypted:array", "array"),
    ("encrypted:collection", "Illuminate\\Support\\Collection"),
    ("encrypted:object", "object"),
    ("hashed", "string"),
];

/// The fully-qualified name of the `Castable` contract.
const CASTABLE_FQN: &str = "Illuminate\\Contracts\\Database\\Eloquent\\Castable";

/// Map an Eloquent cast type string to a PHP type.
///
/// Handles built-in cast strings (`datetime`, `boolean`, `array`, etc.),
/// `decimal:N` variants (e.g. `decimal:2` → `float`), custom cast
/// classes (inspects the `get()` return type), enum classes (the
/// property type is the enum itself), and `Castable` implementations
/// (the property type is the class itself).
///
/// When a custom cast class's `get()` method has no return type (native
/// or docblock), the resolver falls back to the first generic argument
/// from an `@implements CastsAttributes<TGet, TSet>` annotation on the
/// cast class.
///
/// Class-based cast types may carry a `:argument` suffix (e.g.
/// `Address::class.':nullable'`).  The suffix is stripped before
/// resolving the class.
pub(super) fn cast_type_to_php_type(
    cast_type: &str,
    class_loader: &dyn Fn(&str) -> Option<Arc<ClassInfo>>,
) -> PhpType {
    // 1. Check the built-in mapping table.
    let lower = cast_type.to_lowercase();
    for &(key, php_type) in CAST_TYPE_MAP {
        if lower == key {
            return PhpType::Named(php_type.to_string());
        }
    }

    // 2. Handle `decimal:N` variants (e.g. `decimal:2`, `decimal:8`).
    if lower.starts_with("decimal:") || lower == "decimal" {
        return PhpType::float();
    }

    // 3. Handle `datetime:format` variants (e.g. `datetime:Y-m-d`).
    if lower.starts_with("datetime:") {
        return PhpType::Named("Carbon\\Carbon".to_string());
    }

    // 4. Handle `date:format` variants.
    if lower.starts_with("date:") {
        return PhpType::Named("Carbon\\Carbon".to_string());
    }

    // 5. Handle `immutable_datetime:format` variants.
    if lower.starts_with("immutable_datetime:") {
        return PhpType::Named("Carbon\\CarbonImmutable".to_string());
    }

    // 6. Handle `immutable_date:format` variants.
    if lower.starts_with("immutable_date:") {
        return PhpType::Named("Carbon\\CarbonImmutable".to_string());
    }

    // 7. Assume it's a class-based cast.  Strip any `:argument` suffix
    //    (e.g. `App\Casts\Address:nullable` → `App\Casts\Address`).
    let class_name = cast_type.split(':').next().unwrap_or(cast_type);

    if let Some(cast_class) = class_loader(class_name) {
        // 7a. Enums — the property type is the enum itself.
        if cast_class.kind == ClassLikeKind::Enum {
            return PhpType::Named(class_name.to_string());
        }

        // 7b. Castable implementations — the property type is the
        //     class itself.  Castable classes declare `castUsing()`
        //     which returns a CastsAttributes instance, but the
        //     developer-facing type is the Castable class.
        if is_castable(&cast_class) {
            return PhpType::Named(class_name.to_string());
        }

        // 7c. `@implements CastsAttributes<TGet, TSet>` — the canonical
        //     type declaration.  The class-level generic annotation is
        //     the strongest signal because it is the developer's
        //     explicit contract.  The `get()` method's return type is
        //     an implementation detail that may be `mixed`, less
        //     specific, or missing entirely.
        if let Some(tget) = extract_tget_from_implements_generics(&cast_class) {
            return tget;
        }

        // 7d. Fallback: inspect the `get()` method's return type.
        //     When no `@implements` generics are declared, the concrete
        //     return type on `get()` is the next best signal.  Skip
        //     `mixed` — it carries no useful type information and is
        //     the default native hint on the interface method.
        if let Some(get_method) = cast_class.methods.iter().find(|m| m.name == "get")
            && let Some(ref rt) = get_method.return_type
            && !rt.is_mixed()
        {
            return rt.clone();
        }
    }

    // 8. Fallback: unknown cast type.
    PhpType::mixed()
}

/// Extract the `TGet` type from a cast class's `@implements CastsAttributes<TGet, TSet>`.
///
/// Returns the first generic argument if the class declares an
/// `@implements` annotation for `CastsAttributes` (matched by short
/// name or FQN, with or without leading backslash).
fn extract_tget_from_implements_generics(class: &ClassInfo) -> Option<PhpType> {
    for (name, args) in &class.implements_generics {
        if (name == CASTS_ATTRIBUTES_FQN
            || name == CASTS_ATTRIBUTES_SHORT
            || short_name(name) == CASTS_ATTRIBUTES_SHORT)
            && let Some(tget) = args.first()
        {
            // Skip empty/blank type arguments (e.g. from malformed docblocks).
            if matches!(tget, PhpType::Named(s) | PhpType::Raw(s) if s.is_empty()) {
                continue;
            }
            return Some(tget.clone());
        }
    }
    None
}

/// Check whether a class implements the `Castable` contract.
///
/// Looks for `Illuminate\Contracts\Database\Eloquent\Castable` in the
/// class's `interfaces` list (with or without leading backslash, and
/// also matches the short name `Castable`).
fn is_castable(class: &ClassInfo) -> bool {
    class
        .interfaces
        .iter()
        .any(|iface| iface == CASTABLE_FQN || iface == "Castable")
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "casts_tests.rs"]
mod tests;
