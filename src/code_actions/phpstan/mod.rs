//! PHPStan code actions.
//!
//! Code actions that respond to PHPStan diagnostics. Each action parses
//! the PHPStan error message, extracts the relevant information, and
//! offers a quickfix that modifies the source code to resolve the issue.
//!
//! Currently implemented:
//!
//! - **Add `@throws`** — when PHPStan reports a
//!   `missingType.checkedException` error, offer to add a `@throws`
//!   tag to the enclosing function/method docblock and import the
//!   exception class if needed.
//! - **Remove `@throws`** — when PHPStan reports `throws.unusedType`
//!   or `throws.notThrowable`, offer to remove the offending `@throws`
//!   line from the docblock.
//! - **Add `#[Override]`** — when PHPStan reports
//!   `method.missingOverride`, offer to insert `#[\Override]` above
//!   the method declaration.
//! - **Remove `#[Override]`** — when PHPStan reports
//!   `method.override` or `property.override`, offer to remove the
//!   `#[\Override]` attribute from the declaration.
//! - **Add `#[\ReturnTypeWillChange]`** — when PHPStan reports
//!   `method.tentativeReturnType`, offer to insert the attribute
//!   above the method declaration.
//! - **Fix PHPDoc type** — when PHPStan reports `return.phpDocType`,
//!   `parameter.phpDocType`, or `property.phpDocType` (a `@return`,
//!   `@param`, or `@var` tag whose type is incompatible with the
//!   native type hint), offer to update the tag type to match the
//!   native type or remove the tag entirely.
//! - **PHPStan ignore** — when the cursor is on a line with a PHPStan
//!   error, offer to add `@phpstan-ignore <identifier>`.  When PHPStan
//!   reports an unnecessary ignore, offer to remove it.

mod add_override;
pub(crate) mod add_return_type_will_change;
pub(crate) mod add_throws;
pub(crate) mod fix_phpdoc_type;
mod ignore;
pub(crate) mod new_static;
pub(crate) mod remove_override;
mod remove_throws;

use tower_lsp::lsp_types::*;

use crate::Backend;

/// Split a PHPStan diagnostic message into the primary message and optional tip.
///
/// `parse_phpstan_message()` in `phpstan.rs` appends the tip after a `\n`
/// separator when the PHPStan JSON includes a `"tip"` field.  This helper
/// reverses that so code actions can inspect the tip independently (e.g. to
/// extract a suggested type or attribute name).
///
/// Returns `(message, Some(tip))` when a tip is present, or
/// `(message, None)` when there is no tip.
#[allow(dead_code)] // consumers land with H4, H5, H12, H14, H15, H20
pub(crate) fn split_phpstan_tip(message: &str) -> (&str, Option<&str>) {
    match message.split_once('\n') {
        Some((msg, tip)) => (msg, Some(tip)),
        None => (message, None),
    }
}

impl Backend {
    /// Collect all PHPStan-specific code actions.
    ///
    /// Called from [`Backend::handle_code_action`](super) to gather every
    /// PHPStan quickfix that applies at the given cursor/range.
    pub(crate) fn collect_phpstan_actions(
        &self,
        uri: &str,
        content: &str,
        params: &CodeActionParams,
        out: &mut Vec<CodeActionOrCommand>,
    ) {
        // ── PHPStan ignore / remove unnecessary ignore ──────────────
        self.collect_phpstan_ignore_actions(uri, content, params, out);

        // ── Add @throws for checked exceptions ──────────────────────
        self.collect_add_throws_actions(uri, content, params, out);

        // ── Remove invalid/unused @throws ───────────────────────────
        self.collect_remove_throws_actions(uri, content, params, out);

        // ── Add #[Override] for overriding methods ──────────────────
        self.collect_add_override_actions(uri, content, params, out);

        // ── Remove #[Override] from non-overriding members ──────────
        self.collect_remove_override_actions(uri, content, params, out);

        // ── Add #[\ReturnTypeWillChange] for tentative return types ─
        self.collect_add_return_type_will_change_actions(uri, content, params, out);

        // ── Fix unsafe `new static()` ───────────────────────────────
        self.collect_new_static_actions(uri, content, params, out);

        // ── Fix PHPDoc type mismatch (@return, @param, @var) ────────
        self.collect_fix_phpdoc_type_actions(uri, content, params, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_message_with_tip() {
        let (msg, tip) = split_phpstan_tip("Some error.\nUse #[Override] to fix.");
        assert_eq!(msg, "Some error.");
        assert_eq!(tip, Some("Use #[Override] to fix."));
    }

    #[test]
    fn returns_none_when_no_tip() {
        let (msg, tip) = split_phpstan_tip("Some error.");
        assert_eq!(msg, "Some error.");
        assert_eq!(tip, None);
    }

    #[test]
    fn empty_tip_after_newline() {
        let (msg, tip) = split_phpstan_tip("Some error.\n");
        assert_eq!(msg, "Some error.");
        assert_eq!(tip, Some(""));
    }
}
