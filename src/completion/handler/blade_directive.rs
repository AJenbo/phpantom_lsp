//! Strategy: Blade directive-name completion.
//!
//! Always short-circuits: [`crate::blade::directive_completion::directive_prefix_at`]
//! only returns `Some` when the cursor sits in an HTML/directive position
//! of a Blade template, and any `@` typed there is a directive name being
//! written, not a member/variable/class reference — so even a prefix that
//! matches no known directive returns an empty list rather than falling
//! through to another strategy.

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionResponse, Position};

use crate::Backend;
use crate::blade::directives::DIRECTIVE_COMPLETIONS;
use crate::text_position::position_to_byte_offset;

impl Backend {
    /// The directive-name prefix already typed after an `@` in `uri`'s raw
    /// Blade source at `position`, when that `@` is in an HTML/directive
    /// position. `uri`'s raw content (not the preprocessed virtual PHP) is
    /// fetched fresh here since the incomplete directive name a user is
    /// mid-typing never survives Blade preprocessing (an unrecognised
    /// directive is masked as inert HTML — see
    /// `src/blade/directive_completion.rs`).
    pub(super) fn blade_directive_prefix_at(
        &self,
        uri: &str,
        position: Position,
    ) -> Option<String> {
        let content = self.get_file_content(uri)?;
        let offset = position_to_byte_offset(&content, position);
        crate::blade::directive_completion::directive_prefix_at(&content, offset)
            .map(|s| s.to_string())
    }

    /// Build the directive-name completion list for an already-typed
    /// `prefix` (the text after `@`, matched case-insensitively).
    pub(super) fn complete_blade_directive(&self, prefix: &str) -> CompletionResponse {
        let prefix_lower = prefix.to_lowercase();
        let items = DIRECTIVE_COMPLETIONS
            .iter()
            .filter(|completion| completion.name.to_lowercase().starts_with(&prefix_lower))
            .map(|completion| CompletionItem {
                label: format!("@{}", completion.name),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Blade directive".to_string()),
                filter_text: Some(format!("@{}", completion.name)),
                insert_text: Some(completion.insert_text.to_string()),
                insert_text_format: Some(if completion.is_snippet {
                    tower_lsp::lsp_types::InsertTextFormat::SNIPPET
                } else {
                    tower_lsp::lsp_types::InsertTextFormat::PLAIN_TEXT
                }),
                ..CompletionItem::default()
            })
            .collect();
        CompletionResponse::Array(items)
    }
}
