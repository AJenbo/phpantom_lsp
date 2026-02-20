/// Completion-related modules.
///
/// This sub-module groups all completion logic:
/// - **handler**: Top-level completion request orchestration
/// - **target**: Extracting the completion target (access operator and subject)
/// - **resolver**: Resolving the subject to a concrete class type
/// - **builder**: Building LSP `CompletionItem`s from resolved class info
/// - **class_completion**: Class name, constant, and function completions
/// - **variable_completion**: Variable name completions and scope collection
/// - **phpdoc**: PHPDoc tag completion inside `/** … */` blocks
/// - **named_args**: Named argument completion inside function/method call parens
/// - **array_shape**: Array shape key completion (`$arr['` → suggest known keys)
///   and raw variable type resolution for array shape value chaining
/// - **catch_completion**: Smart exception type completion inside `catch()` clauses
/// - **conditional_resolution**: PHPStan conditional return type resolution at call sites
/// - **type_narrowing**: instanceof / assert / custom type guard narrowing
/// - **type_hint_completion**: Type completion inside function/method parameter lists,
///   return types, and property declarations (offers native PHP types + class names)
/// - **variable_resolution**: Variable type resolution via assignment scanning
/// - **closure_resolution**: Closure and arrow-function parameter resolution
///
/// Class inheritance merging (traits, mixins, parent chain) lives in the
/// top-level [`crate::inheritance`] module since it is shared infrastructure
/// used by completion, definition, and future features (hover, references).
pub mod array_shape;
pub mod builder;
pub(crate) mod catch_completion;
pub mod class_completion;
pub mod closure_resolution;
pub mod conditional_resolution;
pub(crate) mod handler;
pub mod named_args;
pub mod phpdoc;
pub mod resolver;
pub mod target;
pub(crate) mod type_hint_completion;
pub mod type_narrowing;
pub mod use_edit;
pub mod variable_completion;
pub mod variable_resolution;
