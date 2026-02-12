/// Completion-related modules.
///
/// This sub-module groups all completion logic:
/// - **target**: Extracting the completion target (access operator and subject)
/// - **resolver**: Resolving the subject to a concrete class type
/// - **builder**: Building LSP `CompletionItem`s from resolved class info
pub mod builder;
pub mod resolver;
pub mod target;
