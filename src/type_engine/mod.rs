//! The shared type-resolution engine.
//!
//! Despite once living under `completion/`, this is the project's single
//! type-resolution engine — the code that answers "what is the type of this
//! expression here?" It is consumed by diagnostics, hover, go-to-definition,
//! and signature help, not just completion.
//!
//! ## Top-level modules
//!
//! - **resolver**: Resolving a subject expression to a concrete class type
//! - **call_resolution**: Call expression and callable target resolution (method
//!   calls, static calls, function calls, constructor calls, signature help,
//!   named-argument completion)
//! - **subject_expr / subject_extraction / subject_resolution**: Extracting and
//!   resolving the left-hand side of `->`, `?->`, and `::` operators
//!
//! ### `types/` — Type resolution
//!
//! - **resolution**: Type-hint string to `ClassInfo` mapping (unions,
//!   intersections, generics, type aliases, object shapes, property types)
//! - **narrowing**: instanceof / assert / custom type guard narrowing
//! - **conditional**: PHPStan conditional return type resolution at call sites
//!
//! ### `variable/` — Variable type resolution
//!
//! - **resolution**: Variable type resolution via assignment scanning
//! - **rhs_resolution**: Right-hand-side expression resolution for variable
//!   assignments (instantiation, array access, function/method/static calls,
//!   property access, match, ternary, clone)
//! - **forward_walk**: The forward walker shared by diagnostics, completion,
//!   hover, go-to-definition, and signature help
//! - **class_string_resolution**: Class-string variable resolution (`$cls = User::class`)
//! - **raw_type_inference**: Raw type inference for variable assignments (array shapes,
//!   array functions, generator yields)
//! - **foreach_resolution**: Foreach value/key and array destructuring type resolution
//! - **closure_resolution**: Closure and arrow-function parameter resolution

pub(crate) mod call_resolution;
pub(crate) mod resolver;
pub mod subject_expr;
pub(crate) mod subject_extraction;
pub(crate) mod subject_resolution;
pub mod types;
pub(crate) mod variable;

// ─── Re-exports ─────────────────────────────────────────────────────────────
//
// These preserve the `conditional_resolution` / `type_resolution` aliases used
// throughout the codebase.

pub use types::conditional as conditional_resolution;
pub(crate) use types::resolution as type_resolution;
