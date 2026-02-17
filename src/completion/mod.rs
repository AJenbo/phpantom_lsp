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
/// - **conditional_resolution**: PHPStan conditional return type resolution at call sites
/// - **type_narrowing**: instanceof / assert / custom type guard narrowing
/// - **variable_resolution**: Variable type resolution via assignment scanning
/// - **closure_resolution**: Closure and arrow-function parameter resolution
///
/// Class inheritance merging (traits, mixins, parent chain) lives in the
/// top-level [`crate::inheritance`] module since it is shared infrastructure
/// used by completion, definition, and future features (hover, references).
pub mod builder;
pub mod class_completion;
pub mod closure_resolution;
pub mod conditional_resolution;
pub(crate) mod handler;
pub mod named_args;
pub mod phpdoc;
pub mod resolver;
pub mod target;
pub mod type_narrowing;
pub mod use_edit;
pub mod variable_completion;
pub mod variable_resolution;
