//! PHPDoc block parsing.
//!
//! This module extracts type information from PHPDoc comments (`/** ... */`).
//! It is split into focused submodules:
//!
//! # Submodules
//!
//! - `tags`: Core PHPDoc tag extraction (`@return`, `@var`, `@param`,
//!   `@mixin`, `@deprecated`, `@phpstan-assert`, docblock text retrieval,
//!   and type override logic).
//! - `templates`: Template, generics, and type alias tag extraction
//!   (`@template`, `@extends`, `@implements`, `@use`, `@phpstan-type`,
//!   `@phpstan-import-type`, and `class-string<T>` conditional synthesis).
//! - `virtual_members`: Virtual member tag extraction (`@property`,
//!   `@property-read`, `@property-write`, `@method`).
//! - `conditional`: PHPStan conditional return type parsing.
//! - `types`: Type cleaning utilities, split into focused sub-files:
//!   - `type_strings`: Foundational type string manipulation (constants,
//!     splitting, cleaning, stripping, scalar checks, self/static replacement)
//!   - `generics`: Generic argument parsing and iterable element/key extraction
//!   - `shapes`: Array shape and object shape parsing
//!   - `callable_types`: Callable/Closure return type and parameter extraction,
//!     Generator TSend/TValue extraction

mod conditional;
mod tags;
mod templates;
pub(crate) mod types;
mod virtual_members;

// Type sub-modules — declared here (sibling files to `types.rs`) so
// the Rust module system can find them.  `types.rs` re-exports their
// public items so existing `use …::types::*` call sites keep working.
pub(crate) mod callable_types;
pub(crate) mod generics;
pub(crate) mod shapes;
pub(crate) mod type_strings;

// ─── Re-exports ─────────────────────────────────────────────────────────────
//
// Everything below was previously a public or crate-visible item in the
// single-file `docblock.rs`.  Re-exporting here keeps all existing call
// sites (`use crate::docblock;` and `use phpantom_lsp::docblock::*;`)
// working without modification.

// Core tags
pub use tags::{
    extract_all_param_tags, extract_deprecation_message, extract_deprecation_with_see,
    extract_link_urls, extract_mixin_tags, extract_param_closure_this, extract_param_description,
    extract_param_raw_type, extract_return_description, extract_return_type,
    extract_see_references, extract_type_assertions, extract_var_type, extract_var_type_with_name,
    find_enclosing_return_type, find_inline_var_docblock, find_iterable_raw_type_in_source,
    find_var_raw_type_in_source, get_docblock_text_for_node, has_deprecated_tag,
    resolve_effective_type, should_override_type,
};

// Template / generics / type alias tags
pub use templates::{
    extract_generics_tag, extract_template_param_bindings, extract_template_params,
    extract_template_params_full, extract_template_params_with_bounds, extract_type_aliases,
    synthesize_template_conditional,
};

// Virtual member tags
pub use virtual_members::{extract_method_tags, extract_property_tags};

// Conditional return types
pub use conditional::extract_conditional_return_type;

// Type utilities
pub use types::{
    base_class_name, clean_type, extract_array_shape_value_type, extract_callable_param_types,
    extract_callable_return_type, extract_generator_send_type, extract_generator_value_type_raw,
    extract_generic_key_type, extract_generic_value_type, extract_iterable_element_type,
    extract_object_shape_property_type, is_object_shape, parse_array_shape, parse_object_shape,
    replace_self_in_type, split_intersection_depth0,
};
