//! Array shape and object shape parsing.
//!
//! This submodule handles parsing PHPStan/Psalm array shape and object
//! shape type strings into their constituent entries, and looking up
//! value types by key.
//!
//! All parsing is delegated to `PhpType::parse()`, which uses
//! `mago_phpdoc_syntax` internally.
//!
//! Each public function accepts `&PhpType` directly, avoiding a
//! redundant re-parse when the caller already has a parsed type.

use crate::php_type::{PhpType, TypeKind};

/// Resolve implicit positional keys in shape entries.
///
/// Entries with `key: None` are assigned auto-incrementing string
/// indices (`"0"`, `"1"`, …), matching PHPStan's array shape semantics.
fn resolve_shape_keys(entries: &[crate::php_type::ShapeEntry]) -> Vec<crate::php_type::ShapeEntry> {
    let mut result = Vec::with_capacity(entries.len());
    let mut implicit_index: u32 = 0;

    for entry in entries {
        let key = match &entry.key {
            Some(k) => Some(k.clone()),
            None => {
                let k = implicit_index.to_string();
                implicit_index += 1;
                Some(k)
            }
        };

        result.push(crate::php_type::ShapeEntry {
            key,
            value_type: entry.value_type.clone(),
            optional: entry.optional,
        });
    }

    result
}

/// Unwrap nullable and extract an array shape from a `PhpType`.
///
/// Returns the shape entries if the (possibly nullable) type is an
/// array shape, or `None` otherwise.
fn unwrap_array_shape(ty: &PhpType) -> Option<&[crate::php_type::ShapeEntry]> {
    match ty.kind() {
        TypeKind::ArrayShape(entries) => Some(entries),
        TypeKind::Nullable(inner) => unwrap_array_shape(inner),
        _ => None,
    }
}

/// Unwrap nullable/intersection and extract an object shape from a `PhpType`.
///
/// Returns the shape entries if the (possibly nullable or intersected)
/// type contains an object shape, or `None` otherwise.
fn unwrap_object_shape(ty: &PhpType) -> Option<&[crate::php_type::ShapeEntry]> {
    match ty.kind() {
        TypeKind::ObjectShape(entries) => Some(entries),
        TypeKind::Nullable(inner) => unwrap_object_shape(inner),
        // `object{foo: int, bar: string}&\stdClass` parses as an
        // intersection; check each member.
        TypeKind::Intersection(members) => members.iter().find_map(|m| unwrap_object_shape(m)),
        _ => None,
    }
}

// ─── Array shape ────────────────────────────────────────────────────────────

/// Parse a pre-parsed `PhpType` as an array shape, returning its entries.
///
/// Handles both named and positional (implicit-key) entries, optional
/// keys (with `?` suffix), and nested types.
///
/// Returns `None` if the type is not an array shape.
pub fn parse_array_shape_typed(ty: &PhpType) -> Option<Vec<crate::php_type::ShapeEntry>> {
    let entries = unwrap_array_shape(ty)?;
    Some(resolve_shape_keys(entries))
}

/// Look up the value type for a specific key in an already-parsed array
/// shape `PhpType`.
///
/// Returns `None` if the type is not an array shape or the key is not found.
pub fn extract_array_shape_value_type_typed(ty: &PhpType, key: &str) -> Option<PhpType> {
    let entries = parse_array_shape_typed(ty)?;
    entries
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .map(|e| e.value_type)
}

// ─── Object shape ───────────────────────────────────────────────────────────

/// Parse a pre-parsed `PhpType` as an object shape, returning its entries.
///
/// Returns `None` if the type is not an object shape.
pub fn parse_object_shape_typed(ty: &PhpType) -> Option<Vec<crate::php_type::ShapeEntry>> {
    let entries = unwrap_object_shape(ty)?;
    Some(resolve_shape_keys(entries))
}

/// Return `true` if the given `PhpType` is an object shape type.
pub fn is_object_shape_typed(ty: &PhpType) -> bool {
    ty.is_object_shape()
}

/// Look up the value type for a specific property in an already-parsed
/// object shape `PhpType`.
///
/// Returns `None` if the type is not an object shape or the property
/// is not found.
pub fn extract_object_shape_property_type_typed(ty: &PhpType, prop: &str) -> Option<PhpType> {
    let entries = parse_object_shape_typed(ty)?;
    entries
        .into_iter()
        .find(|e| e.key.as_deref() == Some(prop))
        .map(|e| e.value_type)
}
