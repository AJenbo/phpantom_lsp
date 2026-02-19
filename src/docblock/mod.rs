//! PHPDoc block parsing.
//!
//! This module extracts type information from PHPDoc comments (`/** ... */`),
//! specifically `@return`, `@var`, `@property`, `@method`, `@mixin`,
//! `@deprecated`, and `@phpstan-assert` / `@psalm-assert` tags.  It also
//! provides a compatibility check so that a docblock type only overrides a
//! native type hint when the native hint is broad enough to be refined
//! (e.g. `object`, `mixed`, or another class name) and is *not* a concrete
//! scalar that could never be an object.
//!
//! Additionally, it supports PHPStan conditional return type annotations
//! such as:
//! ```text
//! @return ($abstract is class-string<TClass> ? TClass : mixed)
//! ```
//!
//! # Submodules
//!
//! - [`tags`]: PHPDoc tag extraction (`@return`, `@var`, `@property`,
//!   `@method`, `@mixin`, `@deprecated`, `@phpstan-assert`, docblock text
//!   retrieval, and type override logic).
//! - [`conditional`]: PHPStan conditional return type parsing.
//! - [`types`]: Type cleaning utilities (`clean_type`, `strip_nullable`,
//!   `is_scalar`, `split_type_token`).

mod conditional;
mod tags;
pub(crate) mod types;

// ─── Re-exports ─────────────────────────────────────────────────────────────
//
// Everything below was previously a public or crate-visible item in the
// single-file `docblock.rs`.  Re-exporting here keeps all existing call
// sites (`use crate::docblock;` and `use phpantom_lsp::docblock::*;`)
// working without modification.

// Tags
pub use tags::{
    extract_generics_tag, extract_method_tags, extract_mixin_tags, extract_param_raw_type,
    extract_property_tags, extract_return_type, extract_template_params, extract_type_aliases,
    extract_type_assertions, extract_var_type, extract_var_type_with_name,
    find_inline_var_docblock, find_iterable_raw_type_in_source, find_var_raw_type_in_source,
    get_docblock_text_for_node, has_deprecated_tag, resolve_effective_type, should_override_type,
    synthesize_template_conditional,
};

// Conditional return types
pub use conditional::extract_conditional_return_type;

// Type utilities
pub use types::{
    base_class_name, clean_type, extract_array_shape_value_type, extract_generic_key_type,
    extract_generic_value_type, extract_object_shape_property_type, is_object_shape,
    parse_array_shape, parse_object_shape, split_intersection_depth0,
};
