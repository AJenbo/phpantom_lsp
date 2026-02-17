//! Type cleaning and classification utilities for PHPDoc types.
//!
//! This submodule provides helpers for normalising raw type strings
//! extracted from docblocks: stripping leading backslashes, generic
//! parameters, nullable wrappers, and classifying scalars.

/// Scalar / built-in type names that can never be an object and therefore
/// must not be overridden by a class-name docblock annotation.
pub(crate) const SCALAR_TYPES: &[&str] = &[
    "int", "integer", "float", "double", "string", "bool", "boolean", "void", "never", "null",
    "false", "true", "array", "callable", "iterable", "resource",
];

/// Split off the first type token from `s`, respecting `<…>` nesting.
///
/// Returns `(type_token, remainder)` where `type_token` is the full type
/// (e.g. `Collection<int, User>`) and `remainder` is whatever follows.
pub(crate) fn split_type_token(s: &str) -> (&str, &str) {
    let mut angle_depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '<' => angle_depth += 1,
            '>' => {
                angle_depth -= 1;
                // If we just closed the outermost `<`, the type ends here.
                if angle_depth == 0 {
                    let end = i + c.len_utf8();
                    return (&s[..end], &s[end..]);
                }
            }
            c if c.is_whitespace() && angle_depth == 0 => {
                return (&s[..i], &s[i..]);
            }
            _ => {}
        }
    }
    (s, "")
}

/// Clean a raw type string from a docblock:
///   - Strip leading `\` (PHP fully-qualified prefix)
///   - Handle `TypeName|null` → `?TypeName` normalisation is intentionally
///     NOT done here so that downstream code (which already strips `?`) can
///     handle it uniformly.
pub fn clean_type(raw: &str) -> String {
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
pub(crate) fn strip_nullable(type_str: &str) -> &str {
    type_str.strip_prefix('?').unwrap_or(type_str)
}

/// Check whether a type name is a built-in scalar (i.e. can never be an object).
pub(crate) fn is_scalar(type_name: &str) -> bool {
    let lower = type_name.to_ascii_lowercase();
    SCALAR_TYPES.contains(&lower.as_str())
}

/// Extract the element (value) type from a generic iterable type annotation.
///
/// Handles the most common PHPDoc generic iterable patterns:
///   - `list<User>`              → `Some("User")`
///   - `array<User>`             → `Some("User")`
///   - `array<int, User>`        → `Some("User")`
///   - `iterable<User>`          → `Some("User")`
///   - `iterable<int, User>`     → `Some("User")`
///   - `User[]`                  → `Some("User")`
///   - `Collection<int, User>`   → `Some("User")` (any generic class)
///   - `?list<User>`             → `Some("User")` (nullable)
///   - `\Foo\Bar[]`              → `Some("Bar")`
///
/// Returns `None` if the type is not a recognised generic iterable or the
/// element type is a scalar (e.g. `list<int>`).
pub fn extract_generic_value_type(raw_type: &str) -> Option<String> {
    let s = raw_type.strip_prefix('\\').unwrap_or(raw_type);
    let s = s.strip_prefix('?').unwrap_or(s);

    // ── Handle `Type[]` shorthand ───────────────────────────────────────
    if let Some(base) = s.strip_suffix("[]") {
        let cleaned = clean_type(base);
        if !cleaned.is_empty() && !is_scalar(&cleaned) {
            return Some(cleaned);
        }
        // e.g. `int[]` — no class element type
        return None;
    }

    // ── Handle `GenericType<…>` ─────────────────────────────────────────
    let angle_pos = s.find('<')?;
    let inner = s.get(angle_pos + 1..)?.strip_suffix('>')?.trim();
    if inner.is_empty() {
        return None;
    }

    // Split the generic parameters on `,` while respecting `<…>` nesting
    // so that `array<int, Collection<string, User>>` splits correctly into
    // `["int", "Collection<string, User>"]`.
    let value_part = split_last_generic_param(inner);
    let cleaned = clean_type(value_part.trim());

    if cleaned.is_empty() || is_scalar(&cleaned) {
        return None;
    }
    Some(cleaned)
}

/// Split a comma-separated generic parameter list and return the **last**
/// parameter, respecting `<…>` nesting.
///
/// - `"User"`             → `"User"`
/// - `"int, User"`        → `"User"`
/// - `"int, list<User>"`  → `"list<User>"`
fn split_last_generic_param(s: &str) -> &str {
    let mut depth = 0i32;
    let mut last_comma = None;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => last_comma = Some(i),
            _ => {}
        }
    }
    match last_comma {
        Some(pos) => &s[pos + 1..],
        None => s,
    }
}
