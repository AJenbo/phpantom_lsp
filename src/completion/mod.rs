/// Completion-related modules.
///
/// This sub-module groups all completion logic:
/// - **target**: Extracting the completion target (access operator and subject)
/// - **resolver**: Resolving the subject to a concrete class type
/// - **builder**: Building LSP `CompletionItem`s from resolved class info
/// - **phpdoc**: PHPDoc tag completion inside `/** … */` blocks
/// - **named_args**: Named argument completion inside function/method call parens
pub mod builder;
pub mod named_args;
pub mod phpdoc;
pub mod resolver;
pub mod target;
