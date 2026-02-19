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

/// Split off the first type token from `s`, respecting `<…>` and `{…}`
/// nesting (the latter is needed for PHPStan array shape syntax like
/// `array{name: string, age: int}`).
///
/// Returns `(type_token, remainder)` where `type_token` is the full type
/// (e.g. `Collection<int, User>` or `array{name: string}`) and
/// `remainder` is whatever follows.
pub(crate) fn split_type_token(s: &str) -> (&str, &str) {
    let mut angle_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';

    for (i, c) in s.char_indices() {
        // Handle string literals inside array shape keys — skip everything
        // inside quotes so that `{`, `}`, `,`, `:` etc. are not
        // misinterpreted as structural delimiters.
        if in_single_quote {
            if c == '\'' && prev_char != '\\' {
                in_single_quote = false;
            }
            prev_char = c;
            continue;
        }
        if in_double_quote {
            if c == '"' && prev_char != '\\' {
                in_double_quote = false;
            }
            prev_char = c;
            continue;
        }

        match c {
            '\'' if brace_depth > 0 => in_single_quote = true,
            '"' if brace_depth > 0 => in_double_quote = true,
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => {
                angle_depth -= 1;
                // If we just closed the outermost `<`, the type ends here
                // (but only when we're not also inside braces).
                if angle_depth == 0 && brace_depth == 0 {
                    let end = i + c.len_utf8();
                    return (&s[..end], &s[end..]);
                }
            }
            '{' => brace_depth += 1,
            '}' => {
                brace_depth -= 1;
                // If we just closed the outermost `{`, the type ends here
                // (but only when we're not also inside angle brackets).
                if brace_depth == 0 && angle_depth == 0 {
                    let end = i + c.len_utf8();
                    return (&s[..end], &s[end..]);
                }
            }
            c if c.is_whitespace() && angle_depth == 0 && brace_depth == 0 => {
                return (&s[..i], &s[i..]);
            }
            _ => {}
        }
        prev_char = c;
    }
    (s, "")
}

/// Split a type string on `|` at nesting depth 0, respecting `<…>`,
/// `(…)`, and `{…}` nesting.
///
/// Returns a `Vec` with at least one element.  If there is no `|` at
/// depth 0, the returned vector contains the entire input as a single
/// element.
///
/// # Examples
///
/// - `"Foo|null"` → `["Foo", "null"]`
/// - `"Collection<int|string, User>|null"` → `["Collection<int|string, User>", "null"]`
/// - `"array{name: string|int}|null"` → `["array{name: string|int}", "null"]`
/// - `"Foo"` → `["Foo"]`
pub(crate) fn split_union_depth0(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            '|' if depth_angle == 0 && depth_paren == 0 && depth_brace == 0 => {
                parts.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Split a type string on `&` (intersection) at depth 0, respecting
/// `<…>`, `(…)`, and `{…}` nesting.
///
/// This is necessary so that intersection operators inside generic
/// parameters or object/array shapes (e.g. `object{foo: A&B}`) are not
/// mistaken for top-level intersection splits.
///
/// # Examples
///
/// - `"User&JsonSerializable"` → `["User", "JsonSerializable"]`
/// - `"object{foo: int}&\stdClass"` → `["object{foo: int}", "\stdClass"]`
/// - `"object{foo: A&B}"` → `["object{foo: A&B}"]` (no split — `&` is nested)
pub fn split_intersection_depth0(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut start = 0;

    for (i, c) in s.char_indices() {
        match c {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            '&' if depth_angle == 0 && depth_paren == 0 && depth_brace == 0 => {
                parts.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Clean a raw type string from a docblock, **preserving** generic
/// parameters so that downstream resolution can apply generic
/// substitution.
///
/// Specifically this function:
///   - Strips leading `\` (PHP fully-qualified prefix)
///   - Strips trailing punctuation (`.`, `,`) that could leak from
///     docblock descriptions
///   - Handles `TypeName|null` → `TypeName` (using depth-0 splitting so
///     that `Collection<int|string, User>|null` is handled correctly)
///
/// Generic parameters like `<int, User>` are **not** stripped.  Use
/// [`base_class_name`] when you need just the unparameterised class name.
pub fn clean_type(raw: &str) -> String {
    let s = raw.strip_prefix('\\').unwrap_or(raw);

    // Strip trailing punctuation that could leak from docblocks
    // (e.g. trailing `.` or `,` in descriptions).
    // Be careful not to strip `,` or `.` that is inside `<…>`.
    let s = s.trim_end_matches(['.', ',']);

    // Handle `TypeName|null` → extract the non-null part, using depth-0
    // splitting so that `|` inside `<…>` is not mistaken for a union
    // separator.
    let parts = split_union_depth0(s);
    if parts.len() > 1 {
        let non_null: Vec<&str> = parts
            .into_iter()
            .map(|p| p.trim())
            .filter(|p| !p.eq_ignore_ascii_case("null"))
            .collect();

        if non_null.len() == 1 {
            return non_null[0].to_string();
        }
        // Multiple non-null parts → keep as union
        if non_null.len() > 1 {
            return non_null.join("|");
        }
    }

    s.to_string()
}

/// Extract the base (unparameterised) class name from a type string,
/// stripping any generic parameters.
///
/// This is the function to use when you need a plain class name for
/// lookups (e.g. mixin resolution, type assertion matching) and do
/// **not** want to carry generic arguments forward.
///
/// # Examples
///
/// - `"Collection<int, User>"` → `"Collection"`
/// - `"\\App\\Models\\User"` → `"App\\Models\\User"`
/// - `"?Foo"` → `"Foo"`
/// - `"Foo|null"` → `"Foo"`
pub fn base_class_name(raw: &str) -> String {
    let cleaned = clean_type(raw);
    strip_generics(&cleaned)
}

/// Strip generic parameters and array shape braces from a (already
/// cleaned) type string.
///
/// `"Collection<int, User>"` → `"Collection"`
/// `"array{name: string}"` → `"array"`
/// `"Foo"` → `"Foo"`
pub(crate) fn strip_generics(s: &str) -> String {
    // Find the earliest `<` or `{` — both delimit parameterisation.
    let angle = s.find('<');
    let brace = s.find('{');
    let idx = match (angle, brace) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    };
    if let Some(i) = idx {
        s[..i].to_string()
    } else {
        s.to_string()
    }
}

/// Parse a type string into its base class name and generic arguments.
///
/// Returns `(base_name, args)` where `args` is empty if the type has no
/// generic parameters.
///
/// **Note:** This only handles `<…>` generics. For array shape syntax
/// (`array{…}`), use [`parse_array_shape`] instead.
///
/// # Examples
///
/// - `"Collection<int, User>"` → `("Collection", ["int", "User"])`
/// - `"array<int, list<User>>"` → `("array", ["int", "list<User>"])`
/// - `"Foo"` → `("Foo", [])`
pub(crate) fn parse_generic_args(type_str: &str) -> (&str, Vec<&str>) {
    let angle_pos = match type_str.find('<') {
        Some(pos) => pos,
        None => return (type_str, vec![]),
    };

    let base = &type_str[..angle_pos];

    // Find the matching closing `>`
    let rest = &type_str[angle_pos + 1..];
    let close_pos = find_matching_close(rest);
    let inner = &rest[..close_pos];

    let args = split_generic_params(inner);
    (base, args)
}

/// Find the position of the matching `>` for an opening `<` that has
/// already been consumed.  `s` starts right after the `<`.
fn find_matching_close(s: &str) -> usize {
    let mut depth = 1i32;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
    }
    // Fallback: end of string (malformed type).
    s.len()
}

/// Find the position of the matching `}` for an opening `{` that has
/// already been consumed.  `s` starts right after the `{`.
fn find_matching_brace_close(s: &str) -> usize {
    let mut depth = 1i32;
    let mut angle_depth = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';

    for (i, ch) in s.char_indices() {
        // Skip characters inside quoted strings so that `{`, `}`, etc.
        // inside array shape keys like `"host}?"` are not misinterpreted.
        if in_single_quote {
            if ch == '\'' && prev_char != '\\' {
                in_single_quote = false;
            }
            prev_char = ch;
            continue;
        }
        if in_double_quote {
            if ch == '"' && prev_char != '\\' {
                in_double_quote = false;
            }
            prev_char = ch;
            continue;
        }

        match ch {
            '\'' => in_single_quote = true,
            '"' => in_double_quote = true,
            '{' => depth += 1,
            '}' if angle_depth == 0 => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            '<' => angle_depth += 1,
            '>' if angle_depth > 0 => angle_depth -= 1,
            _ => {}
        }
        prev_char = ch;
    }
    // Fallback: end of string (malformed type).
    s.len()
}

/// Split generic arguments on commas at depth 0, respecting `<…>`,
/// `(…)`, and `{…}` nesting.
fn split_generic_params(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ',' if depth_angle == 0 && depth_paren == 0 && depth_brace == 0 => {
                parts.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = s[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

/// Strip the nullable `?` prefix from a type string.
pub(crate) fn strip_nullable(type_str: &str) -> &str {
    type_str.strip_prefix('?').unwrap_or(type_str)
}

/// Check whether a type name is a built-in scalar (i.e. can never be an object).
pub(crate) fn is_scalar(type_name: &str) -> bool {
    // Strip generic parameters and array shape braces before checking so
    // that `array<int, User>` and `array{name: string}` are still
    // recognised as scalar base types.
    let base = if let Some(idx_angle) = type_name.find('<') {
        let idx_brace = type_name.find('{').unwrap_or(usize::MAX);
        &type_name[..idx_angle.min(idx_brace)]
    } else if let Some(idx) = type_name.find('{') {
        &type_name[..idx]
    } else {
        type_name
    };
    let lower = base.to_ascii_lowercase();
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
///   - `Generator<int, User>`    → `Some("User")` (TValue = 2nd param)
///   - `Generator<int, User, mixed, void>` → `Some("User")` (TValue = 2nd param)
///
/// For PHP's `Generator<TKey, TValue, TSend, TReturn>`, the **value** (yield)
/// type is always the second generic parameter regardless of how many params
/// are provided.  For all other generic types the last parameter is used.
///
/// Returns `None` if the type is not a recognised generic iterable or the
/// element type is a scalar (e.g. `list<int>`).
pub fn extract_generic_value_type(raw_type: &str) -> Option<String> {
    let s = raw_type.strip_prefix('\\').unwrap_or(raw_type);
    let s = s.strip_prefix('?').unwrap_or(s);

    // ── Handle `Type[]` shorthand ───────────────────────────────────────
    if let Some(base) = s.strip_suffix("[]") {
        let cleaned = clean_type(base);
        let base_name = strip_generics(&cleaned);
        if !base_name.is_empty() && !is_scalar(&base_name) {
            return Some(cleaned);
        }
        // e.g. `int[]` — no class element type
        return None;
    }

    // ── Handle `GenericType<…>` ─────────────────────────────────────────
    let angle_pos = s.find('<')?;
    let base_type = &s[..angle_pos];
    let inner = s.get(angle_pos + 1..)?.strip_suffix('>')?.trim();
    if inner.is_empty() {
        return None;
    }

    // ── Special-case `Generator<TKey, TValue, TSend, TReturn>` ──────────
    // The yield/value type is always the **second** generic parameter
    // (index 1).  When only one param is given (`Generator<User>`), it is
    // treated as the value type (consistent with single-param behaviour).
    let value_part = if base_type == "Generator" {
        split_second_generic_param(inner).unwrap_or_else(|| split_last_generic_param(inner))
    } else {
        // Default: use the last generic parameter (works for array, list,
        // iterable, Collection, etc.).
        split_last_generic_param(inner)
    };

    let cleaned = clean_type(value_part.trim());
    let base_name = strip_generics(&cleaned);

    if base_name.is_empty() || is_scalar(&base_name) {
        return None;
    }
    Some(cleaned)
}

/// Extract the key type from a generic iterable type annotation.
///
/// Handles the most common PHPDoc generic iterable patterns:
///   - `array<int, User>`        → `Some("int")`
///   - `array<string, User>`     → `Some("string")`
///   - `iterable<string, User>`  → `Some("string")`
///   - `Collection<User, Order>` → `Some("User")` (first param of 2+ param generic)
///   - `Generator<int, User>`    → `None` (key is `int`, scalar)
///   - `Generator<Request, User, mixed, void>` → `Some("Request")` (TKey = 1st param)
///   - `list<User>`              → `None` (single-param list → key is always `int`, scalar)
///   - `User[]`                  → `None` (shorthand → key is always `int`, scalar)
///   - `array<User>`             → `None` (single-param array → key is `int`, scalar)
///
/// For PHP's `Generator<TKey, TValue, TSend, TReturn>`, the key type is the
/// first generic parameter — which is the same as the default behaviour, so
/// no special-casing is needed.
///
/// Returns `None` if the type is not a recognised generic iterable with an
/// explicit key type, or if the key type is a scalar (e.g. `int`, `string`).
pub fn extract_generic_key_type(raw_type: &str) -> Option<String> {
    let s = raw_type.strip_prefix('\\').unwrap_or(raw_type);
    let s = s.strip_prefix('?').unwrap_or(s);

    // ── `Type[]` shorthand — key is always int (scalar) ─────────────────
    if s.ends_with("[]") {
        return None;
    }

    // ── Handle `GenericType<…>` ─────────────────────────────────────────
    let angle_pos = s.find('<')?;
    let inner = s.get(angle_pos + 1..)?.strip_suffix('>')?.trim();
    if inner.is_empty() {
        return None;
    }

    // Only two-or-more-parameter generics have an explicit key type.
    // Single-parameter generics (e.g. `list<User>`, `array<User>`) have
    // an implicit `int` key which is scalar — nothing to resolve.
    let key_part = split_first_generic_param(inner)?;
    let cleaned = clean_type(key_part.trim());
    let base_name = strip_generics(&cleaned);

    if base_name.is_empty() || is_scalar(&base_name) {
        return None;
    }
    Some(cleaned)
}

/// Split a comma-separated generic parameter list and return the **first**
/// parameter, but only when there are at least two parameters.
/// Respects `<…>` nesting.
///
/// - `"int, User"`             → `Some("int")`
/// - `"Request, Response"`     → `Some("Request")`
/// - `"User"`                  → `None` (single param)
/// - `"int, list<User>"`       → `Some("int")`
/// - `"Collection<A, B>, User"` → `Some("Collection<A, B>")`
fn split_first_generic_param(s: &str) -> Option<&str> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth -= 1,
            ',' if depth == 0 => {
                // Found the first comma at depth 0 → there are 2+ params.
                return Some(s[..i].trim());
            }
            _ => {}
        }
    }
    // No comma at depth 0 → single parameter.
    None
}

/// Split a comma-separated generic parameter list and return the **second**
/// parameter (index 1), but only when there are at least two parameters.
/// Respects `<…>` and `{…}` nesting.
///
/// This is used for `Generator<TKey, TValue, …>` where the value type is
/// always the second parameter.
///
/// - `"int, User"`                      → `Some("User")`
/// - `"int, User, mixed, void"`         → `Some("User")`
/// - `"int, Collection<string, Order>"` → `Some("Collection<string, Order>")`
/// - `"User"`                           → `None` (single param)
fn split_second_generic_param(s: &str) -> Option<&str> {
    let mut depth_angle = 0i32;
    let mut depth_brace = 0i32;
    let mut first_comma: Option<usize> = None;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ',' if depth_angle == 0 && depth_brace == 0 => {
                if let Some(first) = first_comma {
                    // Found second comma — everything between first and
                    // second comma is the 2nd parameter.
                    return Some(s[first + 1..i].trim());
                } else {
                    first_comma = Some(i);
                }
            }
            _ => {}
        }
    }
    // If we found exactly one comma, everything after it is the 2nd param.
    first_comma.map(|pos| s[pos + 1..].trim())
}

/// Split a comma-separated generic parameter list and return the **last**
/// parameter, respecting `<…>` and `{…}` nesting.
///
/// - `"User"`             → `"User"`
/// - `"int, User"`        → `"User"`
/// - `"int, list<User>"`  → `"list<User>"`
fn split_last_generic_param(s: &str) -> &str {
    let mut depth_angle = 0i32;
    let mut depth_brace = 0i32;
    let mut last_comma = None;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ',' if depth_angle == 0 && depth_brace == 0 => last_comma = Some(i),
            _ => {}
        }
    }
    match last_comma {
        Some(pos) => &s[pos + 1..],
        None => s,
    }
}

// ─── Array Shape Parsing ────────────────────────────────────────────────────

use crate::types::ArrayShapeEntry;

/// Parse a PHPStan/Psalm array shape type string into its constituent
/// entries.
///
/// Handles both named and positional (implicit-key) entries, optional
/// keys (with `?` suffix), and nested types.
///
/// # Examples
///
/// - `"array{name: string, age: int}"` → two entries
/// - `"array{name: string, age?: int}"` → "age" is optional
/// - `"array{string, int}"` → positional keys "0", "1"
/// - `"array{user: User, items: list<Item>}"` → nested generics preserved
///
/// Returns `None` if the type is not an array shape.
pub fn parse_array_shape(type_str: &str) -> Option<Vec<ArrayShapeEntry>> {
    let s = type_str.strip_prefix('\\').unwrap_or(type_str);
    let s = s.strip_prefix('?').unwrap_or(s);

    // Must start with `array{` (case-insensitive base).
    let brace_pos = s.find('{')?;
    let base = &s[..brace_pos];
    if !base.eq_ignore_ascii_case("array") {
        return None;
    }

    // Extract the content between `{` and the matching `}`.
    let rest = &s[brace_pos + 1..];
    let close_pos = find_matching_brace_close(rest);
    let inner = rest[..close_pos].trim();

    if inner.is_empty() {
        return Some(vec![]);
    }

    let raw_entries = split_shape_entries(inner);
    let mut entries = Vec::with_capacity(raw_entries.len());
    let mut implicit_index: u32 = 0;

    for raw in raw_entries {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }

        // Try to split on `:` to find `key: type` or `key?: type`.
        // Must respect nesting and quoted strings so that `list<int>`
        // inside a value type doesn't get split, and colons inside
        // quoted keys like `"host:port"` are handled correctly.
        if let Some((key_part, value_part)) = split_shape_key_value(raw) {
            let key_trimmed = key_part.trim();
            let value_trimmed = value_part.trim();

            let (key, optional) = if let Some(k) = key_trimmed.strip_suffix('?') {
                (k.to_string(), true)
            } else {
                (key_trimmed.to_string(), false)
            };

            // Strip surrounding quotes from keys — PHPStan allows
            // `'foo'`, `"bar"`, and unquoted `baz` as key names.
            let key = strip_shape_key_quotes(&key);

            entries.push(ArrayShapeEntry {
                key,
                value_type: value_trimmed.to_string(),
                optional,
            });
        } else {
            // No `:` found — positional entry with implicit numeric key.
            entries.push(ArrayShapeEntry {
                key: implicit_index.to_string(),
                value_type: raw.to_string(),
                optional: false,
            });
            implicit_index += 1;
        }
    }

    Some(entries)
}

/// Strip surrounding single or double quotes from an array shape key.
///
/// PHPStan/Psalm allow array shape keys to be quoted when they contain
/// special characters (spaces, punctuation, etc.):
///   - `'po rt'` → `po rt`
///   - `"host"` → `host`
///   - `foo` → `foo` (unchanged)
fn strip_shape_key_quotes(key: &str) -> String {
    if ((key.starts_with('\'') && key.ends_with('\''))
        || (key.starts_with('"') && key.ends_with('"')))
        && key.len() >= 2
    {
        return key[1..key.len() - 1].to_string();
    }
    key.to_string()
}

/// Split array shape entries on commas at depth 0, respecting `<…>`,
/// `(…)`, and `{…}` nesting.
fn split_shape_entries(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        // Skip characters inside quoted strings so that commas inside
        // quoted array shape keys (e.g. `",host"`) don't split entries.
        if in_single_quote {
            if ch == '\'' && prev_char != '\\' {
                in_single_quote = false;
            }
            prev_char = ch;
            continue;
        }
        if in_double_quote {
            if ch == '"' && prev_char != '\\' {
                in_double_quote = false;
            }
            prev_char = ch;
            continue;
        }

        match ch {
            '\'' => in_single_quote = true,
            '"' => in_double_quote = true,
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ',' if depth_angle == 0 && depth_paren == 0 && depth_brace == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        prev_char = ch;
    }
    let last = &s[start..];
    if !last.trim().is_empty() {
        parts.push(last);
    }
    parts
}

/// Split a single array shape entry into key and value on the **first**
/// `:` at depth 0, outside of quoted strings.
///
/// Returns `Some((key_part, value_part))` if a `:` separator is found,
/// or `None` for positional entries.
///
/// Must respect `<…>`, `{…}` nesting and quoted strings so that colons
/// inside nested types or quoted keys (e.g. `"host:port"`) are not
/// mistaken for the key–value separator.
fn split_shape_key_value(s: &str) -> Option<(&str, &str)> {
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev_char = '\0';

    for (i, ch) in s.char_indices() {
        // Skip characters inside quoted strings so that `:` inside
        // quoted keys like `"host:port"` is not treated as a separator.
        if in_single_quote {
            if ch == '\'' && prev_char != '\\' {
                in_single_quote = false;
            }
            prev_char = ch;
            continue;
        }
        if in_double_quote {
            if ch == '"' && prev_char != '\\' {
                in_double_quote = false;
            }
            prev_char = ch;
            continue;
        }

        match ch {
            '\'' => in_single_quote = true,
            '"' => in_double_quote = true,
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            '{' => depth_brace += 1,
            '}' => depth_brace -= 1,
            ':' if depth_angle == 0 && depth_paren == 0 && depth_brace == 0 => {
                return Some((&s[..i], &s[i + 1..]));
            }
            _ => {}
        }
        prev_char = ch;
    }
    None
}

/// Look up the value type for a specific key in an array shape type string.
///
/// Given a type like `"array{name: string, user: User}"` and key `"user"`,
/// returns `Some("User")`.
///
/// Returns `None` if the type is not an array shape or the key is not found.
pub fn extract_array_shape_value_type(type_str: &str, key: &str) -> Option<String> {
    let entries = parse_array_shape(type_str)?;
    entries
        .into_iter()
        .find(|e| e.key == key)
        .map(|e| e.value_type)
}

// ─── Object Shape Parsing ───────────────────────────────────────────────────

/// Parse a PHPStan object shape type string into its constituent entries.
///
/// Object shapes describe an anonymous object with typed properties:
///
/// # Examples
///
/// - `"object{foo: int, bar: string}"` → two entries
/// - `"object{foo: int, bar?: string}"` → "bar" is optional
/// - `"object{'foo': int, \"bar\": string}"` → quoted property names
/// - `"object{foo: int, bar: string}&\stdClass"` → intersection ignored here
///
/// The returned entries reuse [`ArrayShapeEntry`] since the structure is
/// identical (key name, value type, optional flag).
///
/// Returns `None` if the type is not an object shape.
pub fn parse_object_shape(type_str: &str) -> Option<Vec<ArrayShapeEntry>> {
    let s = type_str.strip_prefix('\\').unwrap_or(type_str);
    let s = s.strip_prefix('?').unwrap_or(s);

    // Must start with `object{` (case-insensitive base).
    let brace_pos = s.find('{')?;
    let base = &s[..brace_pos];
    if !base.eq_ignore_ascii_case("object") {
        return None;
    }

    // Extract the content between `{` and the matching `}`.
    let rest = &s[brace_pos + 1..];
    let close_pos = find_matching_brace_close(rest);
    let inner = rest[..close_pos].trim();

    if inner.is_empty() {
        return Some(vec![]);
    }

    // Reuse the same splitting and key-value parsing as array shapes —
    // the syntax is identical (`key: Type`, `key?: Type`, quoted keys).
    let raw_entries = split_shape_entries(inner);
    let mut entries = Vec::with_capacity(raw_entries.len());

    for raw in raw_entries {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }

        if let Some((key_part, value_part)) = split_shape_key_value(raw) {
            let key_trimmed = key_part.trim();
            let value_trimmed = value_part.trim();

            let (key, optional) = if let Some(k) = key_trimmed.strip_suffix('?') {
                (k.to_string(), true)
            } else {
                (key_trimmed.to_string(), false)
            };

            let key = strip_shape_key_quotes(&key);

            entries.push(ArrayShapeEntry {
                key,
                value_type: value_trimmed.to_string(),
                optional,
            });
        }
        // Object shapes don't have positional entries — skip anything
        // without an explicit key.
    }

    Some(entries)
}

/// Check whether a type string is an object shape (`object{…}`).
///
/// Returns `true` for `"object{foo: int}"`, `"?object{bar: string}"`,
/// and `"\object{baz: bool}"`.  Returns `false` for bare `"object"`.
pub fn is_object_shape(type_str: &str) -> bool {
    let s = type_str.strip_prefix('\\').unwrap_or(type_str);
    let s = s.strip_prefix('?').unwrap_or(s);
    // Check for `object{` case-insensitively, but only when `{` immediately
    // follows the word `object` (no intervening whitespace).
    if let Some(brace_pos) = s.find('{') {
        let base = &s[..brace_pos];
        base.eq_ignore_ascii_case("object")
    } else {
        false
    }
}

/// Look up the value type for a specific property in an object shape.
///
/// Given a type like `"object{name: string, user: User}"` and key `"user"`,
/// returns `Some("User")`.
///
/// Returns `None` if the type is not an object shape or the property
/// is not found.
pub fn extract_object_shape_property_type(type_str: &str, prop: &str) -> Option<String> {
    let entries = parse_object_shape(type_str)?;
    entries
        .into_iter()
        .find(|e| e.key == prop)
        .map(|e| e.value_type)
}
