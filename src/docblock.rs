//! PHPDoc block parsing.
//!
//! This module extracts type information from PHPDoc comments (`/** ... */`),
//! specifically `@return` and `@var` tags.  It also provides a compatibility
//! check so that a docblock type only overrides a native type hint when the
//! native hint is broad enough to be refined (e.g. `object`, `mixed`, or
//! another class name) and is *not* a concrete scalar that could never be
//! an object.

use mago_span::HasSpan;
use mago_syntax::ast::*;

/// Scalar / built-in type names that can never be an object and therefore
/// must not be overridden by a class-name docblock annotation.
const SCALAR_TYPES: &[&str] = &[
    "int", "integer", "float", "double", "string", "bool", "boolean", "void", "never", "null",
    "false", "true", "array", "callable", "iterable", "resource",
];

// ─── Public API ─────────────────────────────────────────────────────────────

/// Extract the type from a `@return` PHPDoc tag.
///
/// Handles common formats:
///   - `@return TypeName`
///   - `@return TypeName Some description text`
///   - `@return ?TypeName`
///   - `@return \Fully\Qualified\Name`
///   - `@return TypeName|null`
///
/// Returns the cleaned type string (leading `\` stripped) or `None` if no
/// `@return` tag is found.
pub fn extract_return_type(docblock: &str) -> Option<String> {
    extract_tag_type(docblock, "@return")
}

/// Extract the type from a `@var` PHPDoc tag.
///
/// Used for property type annotations like:
///   - `/** @var Session */`
///   - `/** @var \App\Models\User */`
pub fn extract_var_type(docblock: &str) -> Option<String> {
    extract_tag_type(docblock, "@var")
}

/// Decide whether a docblock type should override a native type hint.
///
/// Returns `true` when:
///   - The native hint is a class/interface name (can be refined to subclass)
///   - The native hint is `object` or `mixed` (can be narrowed to a concrete class)
///   - The native hint is `self`, `static`, or `parent`
///   - The native hint is nullable (`?Foo`) where `Foo` is non-scalar
///
/// Returns `false` when:
///   - The native hint is a scalar (`int`, `string`, `bool`, `float`, …)
///   - The native hint is a union/intersection composed entirely of scalars
///   - The docblock type itself looks like a scalar (no point overriding)
pub fn should_override_type(docblock_type: &str, native_type: &str) -> bool {
    // If the docblock type is itself a scalar, there's no value in
    // overriding — it wouldn't help with class resolution anyway.
    let clean_doc = strip_nullable(docblock_type);
    if is_scalar(clean_doc) {
        return false;
    }

    // Strip nullable wrapper from the native hint for analysis.
    let clean_native = strip_nullable(native_type);

    // If the native type is a union or intersection, check each component.
    if clean_native.contains('|') || clean_native.contains('&') {
        let parts: Vec<&str> = clean_native
            .split(['|', '&'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        // If ALL parts are scalar, the docblock can't override.
        // If ANY part is non-scalar, it's plausible to refine.
        return !parts.iter().all(|p| is_scalar(strip_nullable(p)));
    }

    // Simple case: if the native type is a scalar, don't override.
    !is_scalar(clean_native)
}

/// Look up the docblock comment (if any) for a class-like member and return
/// its raw text.
///
/// This uses the program's trivia list to find the `/** ... */` comment that
/// immediately precedes the given AST node.  The `content` parameter is the
/// full source text and is used to verify there is no code between the
/// docblock and the node.
pub fn get_docblock_text_for_node<'a>(
    trivia: &'a [Trivia<'a>],
    content: &str,
    node: &impl HasSpan,
) -> Option<&'a str> {
    let node_start = node.span().start.offset;
    let candidate_idx = trivia.partition_point(|t| t.span.start.offset < node_start);
    if candidate_idx == 0 {
        return None;
    }

    let content_bytes = content.as_bytes();
    let mut covered_from = node_start;

    for i in (0..candidate_idx).rev() {
        let t = &trivia[i];
        let t_end = t.span.end.offset;

        // Check for non-whitespace content in the gap between this trivia
        // and the region we've already covered.
        let gap = content_bytes
            .get(t_end as usize..covered_from as usize)
            .unwrap_or(&[]);
        if !gap.iter().all(u8::is_ascii_whitespace) {
            return None;
        }

        match t.kind {
            TriviaKind::DocBlockComment => return Some(t.value),
            TriviaKind::WhiteSpace
            | TriviaKind::SingleLineComment
            | TriviaKind::MultiLineComment
            | TriviaKind::HashComment => {
                covered_from = t.span.start.offset;
            }
        }
    }

    None
}

/// Apply docblock type override logic to a type hint.
///
/// If the docblock provides a type that is compatible as an override for the
/// native type hint (or there is no native type hint), the docblock type is
/// returned.  Otherwise the native type hint is kept.
///
/// When both are `None`, returns `None`.
pub fn resolve_effective_type(
    native_type: Option<&str>,
    docblock_type: Option<&str>,
) -> Option<String> {
    match (native_type, docblock_type) {
        // Docblock provided, no native hint → use docblock.
        (None, Some(doc)) => Some(doc.to_string()),
        // Both present → override only if compatible.
        (Some(native), Some(doc)) => {
            if should_override_type(doc, native) {
                Some(doc.to_string())
            } else {
                Some(native.to_string())
            }
        }
        // Native only → keep it.
        (Some(native), None) => Some(native.to_string()),
        // Neither → nothing.
        (None, None) => None,
    }
}

// ─── Internals ──────────────────────────────────────────────────────────────

/// Generic tag extraction: find `@tag TypeName` and return the cleaned type.
fn extract_tag_type(docblock: &str, tag: &str) -> Option<String> {
    // Strip the `/**` opening and `*/` closing delimiters so that we only
    // deal with the inner content.  This handles both single-line
    // (`/** @return Foo */`) and multi-line docblocks.
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    for line in inner.lines() {
        // Strip leading whitespace and the `*` gutter common in docblocks.
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix(tag) {
            // The tag must be followed by whitespace (or be exactly `@tag`
            // at end-of-line, which is invalid and we skip).
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // The type is the first whitespace-delimited token.
            let type_str = rest.split_whitespace().next()?;

            return Some(clean_type(type_str));
        }
    }
    None
}

/// Clean a raw type string from a docblock:
///   - Strip leading `\` (PHP fully-qualified prefix)
///   - Handle `TypeName|null` → `?TypeName` normalisation is intentionally
///     NOT done here so that downstream code (which already strips `?`) can
///     handle it uniformly.
fn clean_type(raw: &str) -> String {
    let s = raw.strip_prefix('\\').unwrap_or(raw);

    // Strip generic parameters like `Collection<int, Model>` → `Collection`
    // Our type resolution only works with simple class names.
    let s = if let Some(idx) = s.find('<') {
        &s[..idx]
    } else {
        s
    };

    // Also strip trailing punctuation that could leak from docblocks
    // (e.g. trailing `.` or `,` in descriptions).
    let s = s.trim_end_matches(['.', ',']);

    // Handle `TypeName|null` → extract the non-null part
    if s.contains('|') {
        let parts: Vec<&str> = s
            .split('|')
            .map(|p| p.trim())
            .filter(|p| !p.eq_ignore_ascii_case("null"))
            .collect();

        if parts.len() == 1 {
            return parts[0].to_string();
        }
        // Multiple non-null parts → keep as union
        return parts.join("|");
    }

    s.to_string()
}

/// Strip the nullable `?` prefix from a type string.
fn strip_nullable(type_str: &str) -> &str {
    type_str.strip_prefix('?').unwrap_or(type_str)
}

/// Check whether a type name is a built-in scalar (i.e. can never be an object).
fn is_scalar(type_name: &str) -> bool {
    let lower = type_name.to_ascii_lowercase();
    SCALAR_TYPES.contains(&lower.as_str())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_return_type ─────────────────────────────────────────────

    #[test]
    fn return_type_simple() {
        let doc = "/** @return Application */";
        assert_eq!(extract_return_type(doc), Some("Application".into()));
    }

    #[test]
    fn return_type_fqn() {
        let doc = "/** @return \\Illuminate\\Session\\Store */";
        assert_eq!(
            extract_return_type(doc),
            Some("Illuminate\\Session\\Store".into())
        );
    }

    #[test]
    fn return_type_nullable() {
        let doc = "/** @return ?Application */";
        assert_eq!(extract_return_type(doc), Some("?Application".into()));
    }

    #[test]
    fn return_type_with_description() {
        let doc = "/** @return Application The main app instance */";
        assert_eq!(extract_return_type(doc), Some("Application".into()));
    }

    #[test]
    fn return_type_multiline() {
        let doc = concat!(
            "/**\n",
            " * Some method.\n",
            " *\n",
            " * @param string $key\n",
            " * @return \\Illuminate\\Session\\Store\n",
            " */",
        );
        assert_eq!(
            extract_return_type(doc),
            Some("Illuminate\\Session\\Store".into())
        );
    }

    #[test]
    fn return_type_none_when_missing() {
        let doc = "/** This is a docblock without a return tag */";
        assert_eq!(extract_return_type(doc), None);
    }

    #[test]
    fn return_type_nullable_union() {
        let doc = "/** @return Application|null */";
        assert_eq!(extract_return_type(doc), Some("Application".into()));
    }

    #[test]
    fn return_type_generic_stripped() {
        let doc = "/** @return Collection<int, Model> */";
        assert_eq!(extract_return_type(doc), Some("Collection".into()));
    }

    // ── extract_var_type ────────────────────────────────────────────────

    #[test]
    fn var_type_simple() {
        let doc = "/** @var Session */";
        assert_eq!(extract_var_type(doc), Some("Session".into()));
    }

    #[test]
    fn var_type_fqn() {
        let doc = "/** @var \\App\\Models\\User */";
        assert_eq!(extract_var_type(doc), Some("App\\Models\\User".into()));
    }

    #[test]
    fn var_type_none_when_missing() {
        let doc = "/** just a comment */";
        assert_eq!(extract_var_type(doc), None);
    }

    // ── should_override_type ────────────────────────────────────────────

    #[test]
    fn override_object_with_class() {
        assert!(should_override_type("Session", "object"));
    }

    #[test]
    fn override_mixed_with_class() {
        assert!(should_override_type("Session", "mixed"));
    }

    #[test]
    fn override_class_with_subclass() {
        assert!(should_override_type("ConcreteSession", "SessionInterface"));
    }

    #[test]
    fn no_override_int_with_class() {
        assert!(!should_override_type("Session", "int"));
    }

    #[test]
    fn no_override_string_with_class() {
        assert!(!should_override_type("Session", "string"));
    }

    #[test]
    fn no_override_bool_with_class() {
        assert!(!should_override_type("Session", "bool"));
    }

    #[test]
    fn no_override_array_with_class() {
        assert!(!should_override_type("Session", "array"));
    }

    #[test]
    fn no_override_void_with_class() {
        assert!(!should_override_type("Session", "void"));
    }

    #[test]
    fn no_override_nullable_int_with_class() {
        assert!(!should_override_type("Session", "?int"));
    }

    #[test]
    fn override_nullable_object_with_class() {
        assert!(should_override_type("Session", "?object"));
    }

    #[test]
    fn no_override_scalar_union_with_class() {
        assert!(!should_override_type("Session", "string|int"));
    }

    #[test]
    fn override_union_with_object_part() {
        // `SomeClass|null` has a non-scalar part → overridable
        assert!(should_override_type("ConcreteClass", "SomeClass|null"));
    }

    #[test]
    fn no_override_when_docblock_is_scalar() {
        // Even if native is object, if docblock says `int`, no point overriding
        assert!(!should_override_type("int", "object"));
    }

    #[test]
    fn override_self_with_class() {
        assert!(should_override_type("ConcreteClass", "self"));
    }

    #[test]
    fn override_static_with_class() {
        assert!(should_override_type("ConcreteClass", "static"));
    }

    // ── resolve_effective_type ──────────────────────────────────────────

    #[test]
    fn effective_type_docblock_only() {
        assert_eq!(
            resolve_effective_type(None, Some("Session")),
            Some("Session".into())
        );
    }

    #[test]
    fn effective_type_native_only() {
        assert_eq!(
            resolve_effective_type(Some("int"), None),
            Some("int".into())
        );
    }

    #[test]
    fn effective_type_both_compatible() {
        assert_eq!(
            resolve_effective_type(Some("object"), Some("Session")),
            Some("Session".into())
        );
    }

    #[test]
    fn effective_type_both_incompatible() {
        assert_eq!(
            resolve_effective_type(Some("int"), Some("Session")),
            Some("int".into())
        );
    }

    #[test]
    fn effective_type_neither() {
        assert_eq!(resolve_effective_type(None, None), None);
    }

    // ── clean_type ──────────────────────────────────────────────────────

    #[test]
    fn clean_leading_backslash() {
        assert_eq!(clean_type("\\Foo\\Bar"), "Foo\\Bar");
    }

    #[test]
    fn clean_generic() {
        assert_eq!(clean_type("Collection<int, Model>"), "Collection");
    }

    #[test]
    fn clean_nullable_union() {
        assert_eq!(clean_type("Foo|null"), "Foo");
    }

    #[test]
    fn clean_trailing_punctuation() {
        assert_eq!(clean_type("Foo."), "Foo");
    }
}
