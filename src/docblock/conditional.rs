//! PHPStan conditional return type parsing.
//!
//! This submodule handles annotations like:
//! ```text
//! @return ($abstract is class-string<TClass> ? TClass
//!           : ($abstract is null ? \Illuminate\Foundation\Application : mixed))
//! ```
//!
//! The main entry point is [`extract_conditional_return_type`], which
//! returns a [`TypeKind::Conditional`] tree that downstream code can
//! evaluate at call-sites by matching the actual argument types against
//! the declared conditions.

use mago_docblock::document::TagKind;

use crate::php_type::{PhpType, TypeKind};

use super::parser::{DocblockInfo, collapse_newlines, parse_docblock_for_tags};

// ─── Public API ─────────────────────────────────────────────────────────────

/// Extract a PHPStan conditional return type from a `@return` tag.
///
/// Handles annotations like:
/// ```text
/// @return ($abstract is class-string<TClass> ? TClass
///           : ($abstract is null ? \Illuminate\Foundation\Application : mixed))
/// ```
///
/// Returns `None` if the `@return` tag is missing or is not a conditional
/// (i.e. does not start with `(`).
pub fn extract_conditional_return_type(docblock: &str) -> Option<PhpType> {
    extract_conditional_return_type_from_info(&parse_docblock_for_tags(docblock)?)
}

/// Like [`extract_conditional_return_type`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_conditional_return_type_from_info(info: &DocblockInfo) -> Option<PhpType> {
    let raw = extract_raw_return_content_from_info(info)?;
    let trimmed = raw.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    let parsed = PhpType::parse(trimmed);
    // Only return if parsing produced a Conditional variant.
    if matches!(parsed.kind(), TypeKind::Conditional { .. }) {
        Some(parsed)
    } else {
        None
    }
}

// ─── Internals ──────────────────────────────────────────────────────────────

/// Extract the raw content after `@return` from a pre-parsed docblock.
///
/// mago-docblock already handles `/** */` stripping, leading `*`
/// removal, and multi-line tag continuation.  The description is
/// returned with internal `\n` normalised to spaces so that downstream
/// parsers see a single-line string (matching the old behaviour of
/// joining continuation lines with spaces).
fn extract_raw_return_content_from_info(info: &DocblockInfo) -> Option<String> {
    let tag = info
        .first_tag_by_kind(TagKind::PhpstanReturn)
        .or_else(|| info.first_tag_by_kind(TagKind::PsalmReturn))
        .or_else(|| info.first_tag_by_kind(TagKind::Return))?;

    let desc = tag.description.trim();
    if desc.is_empty() {
        return None;
    }

    Some(collapse_newlines(desc))
}
