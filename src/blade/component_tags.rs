//! Raw-text scanning of `<x-…>` component tags in Blade source: the
//! attributes each call site passes, the lowest-priority variable source
//! in the declaration chain documented in [`super::signature`].
//!
//! Component tags are HTML syntax, not PHP, so they cannot be read from
//! the mago AST the way a `view()` call site can (see
//! [`super::call_site_inference`]). The virtual PHP the preprocessor
//! emits only carries a bound attribute's *expression* forward, as a
//! `blade_directive(...)` call; the tag name and any plain string
//! attribute never appear in it at all. This module scans the original
//! Blade source directly instead.

use crate::php_type::PhpType;

use super::signature::mask_inert_regions;

/// One `<x-…>` tag occurrence whose tag name matched one of the requested
/// component names.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct ComponentTagCall {
    /// Plain (non-bound) attributes, already typed from their literal
    /// text: `(camelCase name, type)`.
    pub(crate) literal: Vec<(String, PhpType)>,
    /// Bound attributes (`:name="expr"` / the `:$var` shorthand):
    /// `(camelCase name, index into the file's `blade_directive(...)`
    /// call sequence)`.
    ///
    /// The preprocessor emits exactly one `blade_directive` call per
    /// bound attribute, on every HTML tag in the file, in document
    /// order — the same order this scan counts them in — so the index
    /// correlates the two without needing to translate byte offsets
    /// between the Blade source and the virtual PHP.
    pub(crate) bound: Vec<(String, usize)>,
}

/// The bare tag names (without the `x-` prefix) a Blade file's own view
/// names make it addressable by: `components.brand.boxes` becomes
/// `brand.boxes` (so `<x-brand.boxes>` matches it), and a namespaced name
/// (`mail::message`) is kept as-is (so `<x-mail::message>` matches it).
///
/// A view name outside the `components.` namespace with no `::` is not
/// addressable as a bare tag and contributes nothing.
pub(crate) fn component_tag_names(view_names: &[String]) -> Vec<String> {
    view_names
        .iter()
        .filter_map(|name| {
            if let Some(bare) = name.strip_prefix("components.") {
                Some(bare.to_string())
            } else if name.contains("::") {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect()
}

/// The inverse of [`component_tag_names`]: the view name a tag written as
/// `<x-{tag}>` resolves to.
pub(crate) fn view_name_for_component_tag(tag: &str) -> String {
    if tag.contains("::") {
        tag.to_string()
    } else {
        format!("components.{tag}")
    }
}

/// Every distinct tag name referenced by an `<x-…>` occurrence in
/// `content`. Used in the reverse direction from [`scan_component_tag_calls`]:
/// given a file that was just edited, which components does it call?
pub(crate) fn referenced_component_tags(content: &str) -> Vec<String> {
    if !content.contains("<x-") {
        return Vec::new();
    }
    let masked = mask_inert_regions(content, true);
    let bytes = masked.as_bytes();
    let mut tags = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"<x-") {
            let name_start = i + 3;
            let mut j = name_start;
            while j < bytes.len() && is_tag_name_char(bytes[j]) {
                j += 1;
            }
            if j > name_start {
                let name = masked[name_start..j].to_string();
                if !tags.contains(&name) {
                    tags.push(name);
                }
            }
            i = j.max(name_start + 1);
        } else {
            i += 1;
        }
    }
    tags
}

/// Scan `content` for `<x-…>` occurrences whose tag name (after the `x-`
/// prefix) is one of `tag_names`, and collect the attributes each passes.
///
/// Every bound attribute on *any* tag in the file is counted — not just a
/// matching one — because the preprocessor's `blade_directive` call
/// sequence includes them all; skipping a non-matching tag's bound
/// attributes here would desynchronise this scan's count against that
/// sequence.
pub(crate) fn scan_component_tag_calls(
    content: &str,
    tag_names: &[String],
) -> Vec<ComponentTagCall> {
    if tag_names.is_empty() || !content.contains("<x-") {
        return Vec::new();
    }
    let masked = mask_inert_regions(content, true);
    let bytes = masked.as_bytes();
    let mut results = Vec::new();
    let mut bound_index = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        match bytes.get(i + 1) {
            Some(b'/') => {
                // A closing tag has no attributes to scan.
                i = find_byte(&masked, i, b'>').map_or(bytes.len(), |end| end + 1);
                continue;
            }
            Some(c) if c.is_ascii_alphabetic() => {}
            _ => {
                i += 1;
                continue;
            }
        }
        let name_start = i + 1;
        let mut j = name_start;
        while j < bytes.len() && is_tag_name_char(bytes[j]) {
            j += 1;
        }
        let tag_name = &masked[name_start..j];
        let is_match = tag_name
            .strip_prefix("x-")
            .is_some_and(|bare| tag_names.iter().any(|n| n == bare));
        let (end, call) = scan_tag_attributes(&masked, j, &mut bound_index);
        if is_match {
            results.push(call);
        }
        i = end;
    }
    results
}

fn find_byte(content: &str, from: usize, needle: u8) -> Option<usize> {
    content.as_bytes()[from..]
        .iter()
        .position(|&b| b == needle)
        .map(|pos| from + pos)
}

fn is_tag_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b':' | b'_')
}

fn is_attr_name_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b':' | b'_' | b'@')
}

/// Parse the attribute list of a tag starting right after its name, up to
/// (and past) the tag's closing `>` or self-closing `/>`. Returns the
/// offset just past the close, plus the literal and bound attributes
/// found; `bound_index` is threaded through and bumped for every bound
/// attribute encountered, matching or not, to stay in sync with the
/// file-wide `blade_directive` call count.
fn scan_tag_attributes(
    masked: &str,
    start: usize,
    bound_index: &mut usize,
) -> (usize, ComponentTagCall) {
    let bytes = masked.as_bytes();
    let mut i = start;
    let mut call = ComponentTagCall::default();
    let literal = &mut call.literal;
    let bound = &mut call.bound;

    loop {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        match bytes.get(i) {
            None => break,
            Some(b'>') => {
                i += 1;
                break;
            }
            Some(b'/') if bytes.get(i + 1) == Some(&b'>') => {
                i += 2;
                break;
            }
            Some(b'/') => {
                i += 1;
                continue;
            }
            _ => {}
        }

        // `:$var` shorthand: bound, named after the variable.
        if bytes[i] == b':' && bytes.get(i + 1) == Some(&b'$') {
            let name_start = i + 2;
            let mut j = name_start;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
                j += 1;
            }
            if j > name_start {
                bound.push((masked[name_start..j].to_string(), *bound_index));
            }
            *bound_index += 1;
            i = j;
            continue;
        }

        // A single leading `:` marks a bound attribute; `::` is an
        // escaped literal colon (e.g. `::class`), and the attribute name
        // drops just the escape, not the real colon it protects.
        let is_bound = bytes[i] == b':' && bytes.get(i + 1) != Some(&b':');
        let name_start = if bytes[i] == b':' { i + 1 } else { i };
        let mut j = name_start;
        while j < bytes.len() && is_attr_name_char(bytes[j]) {
            j += 1;
        }
        if j == name_start {
            // Not an attribute token (e.g. a stray `<`); skip one byte so
            // a malformed tag cannot spin this loop forever.
            i += 1;
            continue;
        }
        let name = camel_case_attr_name(&masked[name_start..j]);
        i = j;

        if bytes.get(i) != Some(&b'=') {
            // A bare attribute (`disabled`) is `true`. A bare *bound*
            // attribute (`:disabled`, no `=`) never reaches the
            // preprocessor's `blade_directive` emission (it requires a
            // quoted value), so there is nothing to correlate for it.
            if !is_bound {
                literal.push((name, PhpType::bool()));
            }
            continue;
        }
        i += 1;

        let value_start = i;
        let quoted = matches!(bytes.get(i), Some(b'"') | Some(b'\''));
        let value_end = if quoted {
            let q = bytes[i];
            let mut k = i + 1;
            while k < bytes.len() && bytes[k] != q {
                k += 1;
            }
            k
        } else {
            let mut k = i;
            while k < bytes.len() && !bytes[k].is_ascii_whitespace() && bytes[k] != b'>' {
                k += 1;
            }
            k
        };

        if is_bound {
            // An unquoted bound value is never recognised by the
            // preprocessor either; only a quoted one produced a
            // `blade_directive` call to correlate against.
            if quoted {
                bound.push((name, *bound_index));
                *bound_index += 1;
            }
        } else {
            let raw = &masked[value_start + usize::from(quoted)..value_end];
            let ty = if raw.contains("{{") || raw.contains("{!!") {
                // A literal attribute embedding a Blade echo is not a
                // constant string; fall back to a generic type rather
                // than reporting the raw `{{ $expr }}` text as the value.
                PhpType::string()
            } else {
                PhpType::literal_string_value(raw)
            };
            literal.push((name, ty));
        }

        i = if quoted { value_end + 1 } else { value_end };
    }

    let end = i;
    (end, call)
}

/// Convert a kebab-case attribute name to the camelCase variable name
/// Blade exposes it as (`Illuminate\Support\Str::camel`). A PHP variable
/// name cannot contain a hyphen, so only the camelCase form of a
/// hyphenated attribute is ever accessible inside the template.
fn camel_case_attr_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut upper_next = false;
    for ch in name.chars() {
        if ch == '-' || ch == '_' {
            upper_next = true;
            continue;
        }
        if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(vars: &[(String, PhpType)]) -> Vec<&str> {
        vars.iter().map(|(n, _)| n.as_str()).collect()
    }

    #[test]
    fn component_tag_names_strips_the_components_prefix() {
        assert_eq!(
            component_tag_names(&["components.brand.boxes".to_string()]),
            vec!["brand.boxes"]
        );
    }

    #[test]
    fn component_tag_names_keeps_a_namespaced_name_as_is() {
        assert_eq!(
            component_tag_names(&["mail::message".to_string()]),
            vec!["mail::message"]
        );
    }

    #[test]
    fn component_tag_names_skips_a_bare_non_component_view() {
        assert!(component_tag_names(&["emails.welcome".to_string()]).is_empty());
    }

    #[test]
    fn view_name_for_component_tag_round_trips() {
        assert_eq!(
            view_name_for_component_tag("brand.boxes"),
            "components.brand.boxes"
        );
        assert_eq!(
            view_name_for_component_tag("mail::message"),
            "mail::message"
        );
    }

    #[test]
    fn referenced_component_tags_collects_distinct_names() {
        let content = r#"<x-brand.boxes /><x-brand.boxes /><x-alert type="danger" />"#;
        let mut tags = referenced_component_tags(content);
        tags.sort();
        assert_eq!(tags, vec!["alert", "brand.boxes"]);
    }

    #[test]
    fn scans_a_literal_string_attribute() {
        let calls =
            scan_component_tag_calls(r#"<x-alert type="danger" />"#, &["alert".to_string()]);
        assert_eq!(calls.len(), 1);
        assert_eq!(names(&calls[0].literal), vec!["type"]);
        assert_eq!(
            calls[0].literal[0].1,
            PhpType::literal_string_value("danger")
        );
    }

    #[test]
    fn scans_a_bare_boolean_attribute() {
        let calls = scan_component_tag_calls(r#"<x-alert disabled />"#, &["alert".to_string()]);
        assert_eq!(
            calls[0].literal,
            vec![("disabled".to_string(), PhpType::bool())]
        );
    }

    #[test]
    fn camel_cases_a_hyphenated_attribute_name() {
        let calls =
            scan_component_tag_calls(r#"<x-alert hair-analysis="x" />"#, &["alert".to_string()]);
        assert_eq!(names(&calls[0].literal), vec!["hairAnalysis"]);
    }

    #[test]
    fn a_bound_attribute_is_indexed_in_document_order() {
        let calls = scan_component_tag_calls(
            r#"<x-alert :hairAnalysis="$model->hairAnalysis" />"#,
            &["alert".to_string()],
        );
        assert_eq!(calls[0].bound, vec![("hairAnalysis".to_string(), 0)]);
    }

    #[test]
    fn a_non_matching_tags_bound_attribute_still_advances_the_index() {
        let calls = scan_component_tag_calls(
            r#"<div :class="$active"></div><x-alert :message="$msg" />"#,
            &["alert".to_string()],
        );
        // The `<div>` binding is index 0; `alert`'s own binding must
        // therefore be index 1, or the caller correlates it against the
        // wrong `blade_directive` call.
        assert_eq!(calls[0].bound, vec![("message".to_string(), 1)]);
    }

    #[test]
    fn a_shorthand_bound_attribute_is_named_after_its_variable() {
        let calls = scan_component_tag_calls(r#"<x-alert :$message />"#, &["alert".to_string()]);
        assert_eq!(calls[0].bound, vec![("message".to_string(), 0)]);
    }

    #[test]
    fn a_non_matching_tag_contributes_nothing() {
        let calls = scan_component_tag_calls(r#"<x-widget foo="bar" />"#, &["alert".to_string()]);
        assert!(calls.is_empty());
    }

    #[test]
    fn an_echo_interpolated_literal_falls_back_to_a_generic_string() {
        let calls = scan_component_tag_calls(
            r#"<x-alert title="Hello {{ $name }}" />"#,
            &["alert".to_string()],
        );
        assert_eq!(calls[0].literal[0].1, PhpType::string());
    }

    #[test]
    fn a_component_tag_inside_a_comment_is_ignored() {
        let calls = scan_component_tag_calls(
            r#"{{-- <x-alert type="danger" /> --}}"#,
            &["alert".to_string()],
        );
        assert!(calls.is_empty());
    }
}
