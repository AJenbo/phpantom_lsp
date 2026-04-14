/// Variable resolution sub-modules.
///
/// This group contains modules related to resolving variable types:
/// - **resolution**: Variable type resolution via assignment scanning
/// - **completion**: Variable name completions and scope collection
/// - **rhs_resolution**: Right-hand-side expression resolution for variable assignments
/// - **class_string_resolution**: Class-string variable resolution (`$cls = User::class`)
/// - **raw_type_inference**: Raw type inference for variable assignments (array shapes, etc.)
/// - **foreach_resolution**: Foreach value/key and array destructuring type resolution
/// - **closure_resolution**: Closure and arrow-function parameter resolution
pub(crate) mod class_string_resolution;
pub(crate) mod closure_resolution;
pub(crate) mod completion;
pub(crate) mod foreach_resolution;
pub(crate) mod forward_walk;
pub(crate) mod raw_type_inference;
pub(crate) mod resolution;
pub(crate) mod rhs_resolution;

// ─── PHP array function classifications ─────────────────────────────────────
//
// These constants encode domain knowledge about which PHP standard
// library functions preserve array types vs extract single elements.
// They are consumed by `raw_type_inference` and `call_resolution`.

/// Known array functions whose output preserves the input array's
/// element type (the first positional argument).
pub(crate) const ARRAY_PRESERVING_FUNCS: &[&str] = &[
    "array_filter",
    "array_values",
    "array_unique",
    "array_reverse",
    "array_slice",
    "array_splice",
    "array_chunk",
    "array_diff",
    "array_diff_assoc",
    "array_diff_key",
    "array_diff_uassoc",
    "array_diff_ukey",
    "array_udiff",
    "array_udiff_assoc",
    "array_udiff_uassoc",
    "array_intersect",
    "array_intersect_assoc",
    "array_intersect_uassoc",
    "array_intersect_ukey",
    "array_uintersect",
    "array_uintersect_assoc",
    "array_uintersect_uassoc",
    "array_merge",
];

/// Known array functions that extract a single element from the input
/// array (the element type is the output type, not wrapped in an array).
pub(crate) const ARRAY_ELEMENT_FUNCS: &[&str] = &[
    "array_pop",
    "array_shift",
    "current",
    "end",
    "reset",
    "next",
    "prev",
    "array_first",
    "array_last",
    "array_find",
];
