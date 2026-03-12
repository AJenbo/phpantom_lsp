//! Core PHPDoc tag extraction.
//!
//! This submodule handles extracting type information from PHPDoc comments
//! (`/** ... */`), specifically `@return`, `@var`, `@param`, `@mixin`,
//! `@deprecated`, and `@phpstan-assert` / `@psalm-assert` tags.
//!
//! It also provides:
//!   - [`should_override_type`]: compatibility check so that a docblock type
//!     only overrides a native type hint when the native hint is broad enough
//!     to be refined.
//!   - [`resolve_effective_type`]: pick the best type between docblock and
//!     native hints.
//!   - [`get_docblock_text_for_node`]: extract raw docblock text from an AST
//!     node's preceding trivia.
//!
//! Template/generics/type-alias tags live in [`super::templates`].
//! Virtual member tags (`@property`, `@method`) live in
//! [`super::virtual_members`].

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::types::{AssertionKind, TypeAssertion};

use super::types::{
    base_class_name, clean_type, is_scalar, normalize_nullable, split_type_token, strip_nullable,
};

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

/// Extract the deprecation message from a `@deprecated` PHPDoc tag.
///
/// Handles common formats:
///   - `@deprecated` → `Some("")`
///   - `@deprecated Some explanation text` → `Some("Some explanation text")`
///   - `@deprecated since 2.0` → `Some("since 2.0")`
///
/// Returns `None` when no `@deprecated` tag is present.
/// Returns `Some("")` when the tag is present but has no message.
/// Returns `Some("message")` when the tag includes explanatory text.
pub fn extract_deprecation_message(docblock: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if trimmed == "@deprecated" {
            return Some(String::new());
        }
        if let Some(rest) = trimmed.strip_prefix("@deprecated ") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("@deprecated\t") {
            return Some(rest.trim().to_string());
        }
    }

    None
}

/// Check whether a PHPDoc block contains an `@deprecated` tag.
///
/// Convenience wrapper around [`extract_deprecation_message`] for call
/// sites that only need a boolean check.
pub fn has_deprecated_tag(docblock: &str) -> bool {
    extract_deprecation_message(docblock).is_some()
}

/// Extract all `@mixin` tags from a class-level docblock.
///
/// PHPDoc `@mixin` tags declare that the annotated class exposes public
/// members from another class via magic methods (`__call`, `__get`, etc.).
/// The format is:
///
///   - `@mixin ClassName`
///   - `@mixin \Fully\Qualified\ClassName`
///
/// Returns a list of cleaned class name strings (leading `\` stripped).
pub fn extract_mixin_tags(docblock: &str) -> Vec<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        let rest = if let Some(r) = trimmed.strip_prefix("@mixin") {
            r
        } else {
            continue;
        };

        // The tag must be followed by whitespace.
        let rest = rest.trim_start();
        if rest.is_empty() {
            continue;
        }

        // The class name is the first whitespace-delimited token.
        let class_name = match rest.split_whitespace().next() {
            Some(name) => name,
            None => continue,
        };

        let cleaned = base_class_name(class_name);
        if !cleaned.is_empty() {
            results.push(cleaned);
        }
    }

    results
}

/// Extract `@phpstan-assert` / `@psalm-assert` type assertion annotations.
///
/// Supports all three variants:
///   - `@phpstan-assert Type $param`          → unconditional assertion
///   - `@phpstan-assert-if-true Type $param`  → assertion when return is true
///   - `@phpstan-assert-if-false Type $param` → assertion when return is false
///
/// Also supports the `@psalm-assert` equivalents and negated types
/// (`!Type`).
///
/// Returns a list of parsed assertions.  An empty list means no
/// assertion tags were found.
pub fn extract_type_assertions(docblock: &str) -> Vec<TypeAssertion> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();

    // The tags we recognise, longest-first so that `-if-true` / `-if-false`
    // are matched before the bare `@phpstan-assert`.
    const TAGS: &[(&str, AssertionKind)] = &[
        ("@phpstan-assert-if-true", AssertionKind::IfTrue),
        ("@phpstan-assert-if-false", AssertionKind::IfFalse),
        ("@phpstan-assert", AssertionKind::Always),
        ("@psalm-assert-if-true", AssertionKind::IfTrue),
        ("@psalm-assert-if-false", AssertionKind::IfFalse),
        ("@psalm-assert", AssertionKind::Always),
    ];

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        for &(tag, kind) in TAGS {
            if let Some(rest) = trimmed.strip_prefix(tag) {
                // The tag must be followed by whitespace.
                let rest = rest.trim_start();
                if rest.is_empty() {
                    break;
                }

                // Check for negation: `!Type $param`
                let (negated, rest) = if let Some(r) = rest.strip_prefix('!') {
                    (true, r.trim_start())
                } else {
                    (false, rest)
                };

                // Next token is the type, then the parameter name.
                let mut tokens = rest.split_whitespace();
                let type_str = match tokens.next() {
                    Some(t) => t,
                    None => break,
                };
                let param_str = match tokens.next() {
                    Some(p) if p.starts_with('$') => p,
                    _ => break,
                };

                results.push(TypeAssertion {
                    kind,
                    param_name: param_str.to_string(),
                    asserted_type: clean_type(type_str),
                    negated,
                });

                // Matched a tag — don't try shorter prefixes for this line.
                break;
            }
        }
    }

    results
}

/// Extract the type from a `@var` PHPDoc tag.
///
/// Used for property type annotations like:
///   - `/** @var Session */`
///   - `/** @var \App\Models\User */`
pub fn extract_var_type(docblock: &str) -> Option<String> {
    extract_tag_type(docblock, "@var")
}

/// Extract the type and optional variable name from a `@var` PHPDoc tag.
///
/// Handles both inline annotation formats:
///   - `/** @var TheType */`         → `Some(("TheType", None))`
///   - `/** @var TheType $var */`    → `Some(("TheType", Some("$var")))`
///
/// The variable name (if present) is returned **with** the `$` prefix so
/// callers can compare directly against AST variable names.
pub fn extract_var_type_with_name(docblock: &str) -> Option<(String, Option<String>)> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@var") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Extract the type token, respecting `<…>` nesting so that
            // generics like `Collection<int, User>` are treated as one unit.
            let (type_str, remainder) = split_type_token(rest);
            let cleaned_type = clean_type(type_str);
            if cleaned_type.is_empty() {
                return None;
            }

            // Check for an optional `$variable` name after the type.
            let var_name = remainder
                .split_whitespace()
                .next()
                .filter(|t| t.starts_with('$'))
                .map(|t| t.to_string());

            return Some((cleaned_type, var_name));
        }
    }
    None
}

/// Search backward in `content` from `stmt_start` for an inline `/** @var … */`
/// docblock comment and extract the type (and optional variable name).
///
/// Only considers a docblock that is separated from the statement by
/// whitespace alone — no intervening code.
///
/// Returns `(cleaned_type, optional_var_name)` or `None`.
pub fn find_inline_var_docblock(
    content: &str,
    stmt_start: usize,
) -> Option<(String, Option<String>)> {
    let before = content.get(..stmt_start)?;

    // Walk backward past whitespace / newlines.
    let trimmed = before.trim_end();
    if !trimmed.ends_with("*/") {
        return None;
    }

    // Find the matching `/**`.
    let block_end = trimmed.len();
    let open_pos = trimmed.rfind("/**")?;

    // Ensure nothing but whitespace between the start of the line and `/**`.
    let line_start = trimmed[..open_pos].rfind('\n').map_or(0, |p| p + 1);
    let prefix = &trimmed[line_start..open_pos];
    if !prefix.chars().all(|c| c.is_ascii_whitespace()) {
        return None;
    }

    let docblock = &trimmed[open_pos..block_end];
    extract_var_type_with_name(docblock)
}

/// Search backward through `content` (up to `before_offset`) for any
/// `/** @var RawType $var_name */` annotation and return the **raw**
/// (uncleaned) type string — including generic parameters like `<User>`.
///
/// This is used by foreach element-type resolution: when iterating over
/// a variable annotated as `list<User>`, we need the raw `list<User>`
/// string so that the generic value type (`User`) can be extracted.
///
/// Only matches annotations that explicitly name the variable
/// (e.g. `/** @var list<User> $users */`).
pub fn find_var_raw_type_in_source(
    content: &str,
    before_offset: usize,
    var_name: &str,
) -> Option<String> {
    let search_area = content.get(..before_offset)?;

    // Track brace depth so that annotations inside other function/method
    // bodies are not visible from the current scope.  When scanning
    // backward:
    //   `}` → entering a block above us → depth increases
    //   `{` → leaving that block        → depth decreases
    // Annotations found while `brace_depth > 0` belong to an inner
    // scope and must be skipped.  Once `min_depth` goes negative we
    // have exited our containing scope; if we then re-enter a block at
    // depth >= 0 we are inside a sibling scope (e.g. a different method
    // in the same class) and all further annotations are foreign.
    let mut brace_depth = 0i32;
    let mut min_depth = 0i32;
    let mut seen_sibling_scope = false;

    for line in search_area.lines().rev() {
        let trimmed = line.trim();

        // Count braces on non-docblock lines to track scope depth.
        // Docblock lines are skipped because they may contain `{` / `}`
        // in array shape type annotations (e.g. `array{key: string}`).
        let is_comment_line =
            trimmed.starts_with('*') || trimmed.starts_with("/*") || trimmed.starts_with("//");

        if !is_comment_line {
            let (opens, closes) = count_braces_on_line(trimmed);
            // Going backward: `}` means entering a block, `{` means leaving.
            brace_depth += closes;
            brace_depth -= opens;
        }

        min_depth = min_depth.min(brace_depth);

        // Once we have exited our containing scope (min_depth < 0) and
        // re-entered a block at depth >= 0, we are inside a sibling
        // scope (e.g. a different method in the same class).  From that
        // point on every annotation belongs to a foreign scope.
        if min_depth < 0 && brace_depth >= 0 {
            seen_sibling_scope = true;
        }
        if seen_sibling_scope {
            continue;
        }

        // Skip annotations that belong to a deeper (inner) scope.
        if brace_depth > 0 {
            continue;
        }

        // Quick reject: must mention both `@var` and the variable.
        if !trimmed.contains("@var") || !trimmed.contains(var_name) {
            continue;
        }

        // Strip docblock delimiters — handles single-line `/** @var … */`.
        let inner = trimmed
            .strip_prefix("/**")
            .unwrap_or(trimmed)
            .strip_suffix("*/")
            .unwrap_or(trimmed);
        let inner = inner.trim().trim_start_matches('*').trim();

        if let Some(rest) = inner.strip_prefix("@var") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Extract the full type token (respects `<…>` nesting).
            let (type_token, remainder) = split_type_token(rest);

            // The next token must be our variable name.
            if let Some(name) = remainder.split_whitespace().next()
                && name == var_name
            {
                return Some(type_token.to_string());
            }
        }
    }

    None
}

/// Extract the raw (uncleaned) type from a `@param` tag for a specific
/// parameter in a docblock string.
///
/// Given a docblock and a parameter name (with `$` prefix), returns the
/// raw type string including generic parameters.
///
/// Example:
///   docblock containing `@param list<User> $users` with var_name `"$users"`
///   → `Some("list<User>")`
pub fn extract_param_raw_type(docblock: &str, var_name: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Extract the full type token (respects `<…>` nesting).
            let (type_token, remainder) = split_type_token(rest);

            // The next token should be the parameter name.
            // Handle `...$name` (variadic) by stripping the leading `...`.
            if let Some(name) = remainder.split_whitespace().next() {
                let name = name.strip_prefix("...").unwrap_or(name);
                if name == var_name {
                    return Some(type_token.to_string());
                }
            }
        }
    }

    None
}

/// Extract all `@param-closure-this` declarations from a docblock.
///
/// The tag format is `@param-closure-this TypeName $paramName`, declaring
/// that `$this` inside a closure passed as `$paramName` resolves to
/// `TypeName`.  This is the static-analysis equivalent of runtime
/// `Closure::bindTo()` and is used heavily in Laravel (routing, macros,
/// testing).
///
/// Returns a list of `(type_name, param_name)` pairs.  The `param_name`
/// includes the `$` prefix.  The `type_name` is the raw type string
/// (e.g. `\Illuminate\Routing\Route`, `$this`, `static`).
pub fn extract_param_closure_this(docblock: &str) -> Vec<(String, String)> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@param-closure-this") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Extract the type token (respects `<…>` nesting).
            let (type_token, remainder) = split_type_token(rest);
            if type_token.is_empty() {
                continue;
            }

            // The next token should be the parameter name (`$paramName`).
            if let Some(name) = remainder.split_whitespace().next()
                && name.starts_with('$')
            {
                results.push((type_token.to_string(), name.to_string()));
            }
        }
    }

    results
}

/// Extract the human-readable description from a `@param` tag for a
/// specific parameter.
///
/// Given a docblock and a parameter name (with `$` prefix), returns the
/// description text that follows the type and `$name` on the `@param` line,
/// including any multi-line continuation (lines that don't start with `@`).
///
/// HTML tags like `<p>`, `</p>`, `<i>`, `</i>` are stripped.
///
/// Example:
///   `@param callable|null $callback Callback function to run for each element.`
///   with var_name `"$callback"` → `Some("Callback function to run for each element.")`
pub fn extract_param_description(docblock: &str, var_name: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let lines: Vec<&str> = inner.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                i += 1;
                continue;
            }

            // Skip the type token.
            let (_type_token, remainder) = split_type_token(rest);
            let remainder = remainder.trim_start();

            // Check if the next token is our parameter name.
            // Handle `...$name` (variadic) by stripping the leading `...`.
            let name_token = remainder.split_whitespace().next().unwrap_or("");
            let name_stripped = name_token.strip_prefix("...").unwrap_or(name_token);
            if name_stripped != var_name {
                i += 1;
                continue;
            }

            // Skip past the parameter name to get the description.
            let after_name = remainder.get(name_token.len()..).unwrap_or("").trim_start();

            let mut desc_parts: Vec<String> = Vec::new();
            if !after_name.is_empty() {
                desc_parts.push(strip_html_tags(after_name));
            }

            // Collect continuation lines (until next `@tag` or empty trimmed line).
            let mut j = i + 1;
            while j < lines.len() {
                let cont = lines[j].trim().trim_start_matches('*').trim();
                if cont.is_empty() || cont.starts_with('@') {
                    break;
                }
                desc_parts.push(strip_html_tags(cont));
                j += 1;
            }

            let desc = desc_parts.join(" ").trim().to_string();
            if desc.is_empty() {
                return None;
            }
            return Some(desc);
        }
        i += 1;
    }

    None
}

/// Extract the human-readable description from the `@return` tag in a
/// docblock.
///
/// Returns the text that follows the type on the `@return` line,
/// including any multi-line continuation (lines that don't start with `@`).
///
/// HTML tags like `<p>`, `</p>`, `<i>`, `</i>` are stripped.
///
/// Example:
///   `@return array an array containing all the elements`
///   → `Some("an array containing all the elements")`
pub fn extract_return_description(docblock: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let lines: Vec<&str> = inner.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@return") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                i += 1;
                continue;
            }

            // Skip PHPStan conditional return types.
            if rest.starts_with('(') {
                return None;
            }

            // Skip the type token.
            let (_type_token, remainder) = split_type_token(rest);
            let remainder = remainder.trim_start();

            let mut desc_parts: Vec<String> = Vec::new();
            if !remainder.is_empty() {
                desc_parts.push(strip_html_tags(remainder));
            }

            // Collect continuation lines.
            let mut j = i + 1;
            while j < lines.len() {
                let cont = lines[j].trim().trim_start_matches('*').trim();
                if cont.is_empty() || cont.starts_with('@') {
                    break;
                }
                desc_parts.push(strip_html_tags(cont));
                j += 1;
            }

            let desc = desc_parts.join(" ").trim().to_string();
            if desc.is_empty() {
                return None;
            }
            return Some(desc);
        }
        i += 1;
    }

    None
}

/// Extract the URL from a `@link` tag in a docblock.
///
/// Example:
///   `@link https://php.net/manual/en/function.array-map.php`
///   → `Some("https://php.net/manual/en/function.array-map.php")`
pub fn extract_link_url(docblock: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@link") {
            let rest = rest.trim_start();
            // Take the first whitespace-delimited token as the URL.
            if let Some(url) = rest.split_whitespace().next()
                && !url.is_empty()
            {
                return Some(url.to_string());
            }
        }
    }

    None
}

/// Strip common HTML tags from a docblock description string.
///
/// Removes `<p>`, `</p>`, `<i>`, `</i>`, `<b>`, `</b>`, `<br>`, `<br/>`,
/// `<br />`, `<li>`, `</li>`, `<ul>`, `</ul>`, `<ol>`, `</ol>`,
/// `<code>`, `</code>`, `<em>`, `</em>`, and `<strong>`, `</strong>`.
fn strip_html_tags(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '<' {
            // Find the closing `>`.
            if let Some(end) = s[i..].find('>') {
                let tag = &s[i..i + end + 1];
                let tag_lower = tag.to_ascii_lowercase();
                let is_html = tag_lower == "<p>"
                    || tag_lower == "</p>"
                    || tag_lower == "<i>"
                    || tag_lower == "</i>"
                    || tag_lower == "<b>"
                    || tag_lower == "</b>"
                    || tag_lower == "<br>"
                    || tag_lower == "<br/>"
                    || tag_lower == "<br />"
                    || tag_lower == "<li>"
                    || tag_lower == "</li>"
                    || tag_lower == "<ul>"
                    || tag_lower == "</ul>"
                    || tag_lower == "<ol>"
                    || tag_lower == "</ol>"
                    || tag_lower == "<code>"
                    || tag_lower == "</code>"
                    || tag_lower == "<em>"
                    || tag_lower == "</em>"
                    || tag_lower == "<strong>"
                    || tag_lower == "</strong>"
                    || tag_lower == "<span>"
                    || tag_lower == "</span>";
                if is_html {
                    // Skip past the closing `>`.
                    for _ in 0..end {
                        chars.next();
                    }
                    continue;
                }
            }
            result.push(c);
        } else {
            result.push(c);
        }
    }
    result
}

/// Search backward through `content` (up to `before_offset`) for any
/// `@var` or `@param` annotation that assigns a raw (uncleaned) type to
/// `$var_name`.
///
/// This combines the logic of [`find_var_raw_type_in_source`] (which looks
/// for `@var Type $var`) and a backward scan for `@param Type $var` in
/// method/function docblocks.
///
/// Returns the first matching raw type string (including generic parameters
/// like `list<User>`), or `None` if no annotation is found.
pub fn find_iterable_raw_type_in_source(
    content: &str,
    before_offset: usize,
    var_name: &str,
) -> Option<String> {
    let search_area = content.get(..before_offset)?;

    // Track brace depth so that annotations inside class/function bodies
    // are not visible from an outer scope.  When scanning backward:
    //   `}` → entering a block above us → depth increases
    //   `{` → leaving that block        → depth decreases
    // Annotations found while `brace_depth > 0` belong to an inner
    // scope and must be skipped.
    let mut brace_depth = 0i32;
    let mut min_depth = 0i32;
    let mut seen_sibling_scope = false;

    // Track the previous non-empty line we saw while scanning backward.
    // This lets us match `/** @var Type */` (no variable name) when the
    // *next* line is an assignment to our variable.
    let mut prev_non_empty_line: Option<&str> = None;

    for line in search_area.lines().rev() {
        let trimmed = line.trim();

        // Count braces on non-docblock lines to track scope depth.
        // Docblock lines are skipped because they may contain `{` / `}`
        // in array shape type annotations (e.g. `array{key: string}`).
        let is_comment_line =
            trimmed.starts_with('*') || trimmed.starts_with("/*") || trimmed.starts_with("//");

        if !is_comment_line {
            let (opens, closes) = count_braces_on_line(trimmed);
            // Going backward: `}` means entering a block, `{` means leaving.
            brace_depth += closes;
            brace_depth -= opens;
        }

        min_depth = min_depth.min(brace_depth);

        // Once we have exited our containing scope (min_depth < 0) and
        // re-entered a block at depth >= 0, we are inside a sibling
        // scope (e.g. a different method in the same class).  From that
        // point on every annotation belongs to a foreign scope.
        if min_depth < 0 && brace_depth >= 0 {
            seen_sibling_scope = true;
        }
        if seen_sibling_scope {
            if !trimmed.is_empty() {
                prev_non_empty_line = Some(trimmed);
            }
            continue;
        }

        // Skip annotations that belong to a deeper (inner) scope.
        if brace_depth > 0 {
            if !trimmed.is_empty() {
                prev_non_empty_line = Some(trimmed);
            }
            continue;
        }

        // ── Named annotation: line mentions the variable name ───────
        if trimmed.contains(var_name) {
            // Strip docblock delimiters — handles single-line `/** @var … */`
            // and multi-line `* @param …` lines.
            let inner = trimmed
                .strip_prefix("/**")
                .unwrap_or(trimmed)
                .strip_suffix("*/")
                .unwrap_or(trimmed);
            let inner = inner.trim().trim_start_matches('*').trim();

            // Try @var first, then @param.
            let rest = if let Some(r) = inner.strip_prefix("@var") {
                Some(r)
            } else {
                inner.strip_prefix("@param")
            };

            if let Some(rest) = rest {
                let rest = rest.trim_start();
                if !rest.is_empty() {
                    // Extract the full type token (respects `<…>` nesting).
                    let (type_token, remainder) = split_type_token(rest);

                    // The next token must be our variable name.
                    if let Some(name) = remainder.split_whitespace().next()
                        && name == var_name
                    {
                        return Some(type_token.to_string());
                    }
                }
            }
        }

        // ── No-variable-name annotation: `/** @var Type */` ─────────
        // When the annotation has no variable name, check whether the
        // line immediately following it assigns to our target variable.
        // This handles the common pattern:
        //   /** @var array<int, Customer> */
        //   $thing = [];
        //   $thing[0]->
        if is_comment_line
            && trimmed.contains("@var")
            && let Some(next_line) = prev_non_empty_line
            && next_line.contains(var_name)
        {
            // Verify the next line is an assignment to the variable
            // (e.g. `$thing = …;` or `$thing;`).
            let next_trimmed = next_line.trim();
            if next_trimmed.starts_with(var_name)
                && next_trimmed[var_name.len()..].trim_start().starts_with('=')
            {
                let inner = trimmed
                    .strip_prefix("/**")
                    .unwrap_or(trimmed)
                    .strip_suffix("*/")
                    .unwrap_or(trimmed);
                let inner = inner.trim().trim_start_matches('*').trim();

                if let Some(rest) = inner.strip_prefix("@var") {
                    let rest = rest.trim_start();
                    if !rest.is_empty() {
                        let (type_token, remainder) = split_type_token(rest);

                        // Only match when there is no variable name in
                        // the annotation (otherwise the named check above
                        // would have matched already).
                        let has_var_name = remainder
                            .split_whitespace()
                            .next()
                            .is_some_and(|t| t.starts_with('$'));
                        if !has_var_name {
                            return Some(type_token.to_string());
                        }
                    }
                }
            }
        }

        if !trimmed.is_empty() {
            prev_non_empty_line = Some(trimmed);
        }
    }

    None
}

/// Find the `@return` type annotation of the enclosing function or method.
///
/// Scans backward from `cursor_offset` through `content`, crossing the
/// opening `{` of the enclosing function body, to locate the docblock
/// that immediately precedes the function/method declaration.  If a
/// `@return` tag is found, its type string is returned.
///
/// This is used inside generator bodies to reverse-infer variable types
/// from the declared `@return Generator<TKey, TValue, TSend, TReturn>`.
///
/// Returns `None` when no enclosing function docblock or `@return` tag
/// can be found.
pub fn find_enclosing_return_type(content: &str, cursor_offset: usize) -> Option<String> {
    let search_area = content.get(..cursor_offset)?;

    // Walk backward, tracking brace depth.  We start inside a function
    // body (depth 0).  When we cross the opening `{` (depth goes to -1),
    // we have exited the function body and are in the function signature
    // region.  From there, look for the docblock above.
    let mut brace_depth = 0i32;

    // Find the byte offset of the opening `{` of the enclosing function.
    let mut func_open_brace: Option<usize> = None;
    for (i, ch) in search_area.char_indices().rev() {
        match ch {
            '}' => brace_depth += 1,
            '{' => {
                brace_depth -= 1;
                if brace_depth < 0 {
                    func_open_brace = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }

    let brace_pos = func_open_brace?;

    // The region before the `{` should contain the function signature
    // and (optionally) the docblock above it.
    let before_brace = content.get(..brace_pos)?;

    // Find the `*/` that ends the docblock.  It must appear in the
    // region before the opening brace.  We search for the last `*/`
    // before the `function` keyword.
    //
    // First, locate the `function` keyword so we know where the
    // signature starts.
    let mut sig_start = before_brace.len().saturating_sub(2000);
    // Adjust to a valid UTF-8 char boundary so we don't panic on
    // multi-byte characters (e.g. `─` in comment banners).
    while sig_start > 0 && !before_brace.is_char_boundary(sig_start) {
        sig_start -= 1;
    }
    let sig_region = &before_brace[sig_start..];
    let func_kw_rel = sig_region.rfind("function")?;
    let func_kw_pos = sig_start + func_kw_rel;

    // Everything before `function` (after trimming whitespace and
    // modifiers) should end with the docblock.
    let before_func = content.get(..func_kw_pos)?;

    // Scan backward over modifier keywords and whitespace.
    let trimmed = before_func.trim_end();
    let after_mods = strip_trailing_modifiers(trimmed);

    if !after_mods.ends_with("*/") {
        return None;
    }

    let open_pos = after_mods.rfind("/**")?;
    let docblock = &after_mods[open_pos..];

    extract_return_type(docblock)
}

/// Strip trailing PHP visibility/modifier keywords from a string.
///
/// Given a string like `"  /** ... */\n    public static"`, returns
/// `"  /** ... */"` (after stripping `static` and `public`).
///
/// Recognised modifiers: `public`, `protected`, `private`, `static`,
/// `abstract`, `final`.
fn strip_trailing_modifiers(s: &str) -> &str {
    const MODIFIERS: &[&str] = &[
        "public",
        "protected",
        "private",
        "static",
        "abstract",
        "final",
    ];

    let mut current = s;
    loop {
        let trimmed = current.trim_end();
        let mut found = false;
        for &modifier in MODIFIERS {
            if let Some(before) = trimmed.strip_suffix(modifier) {
                // Make sure the modifier is preceded by whitespace or
                // start of string (not part of a longer identifier).
                let before_trimmed = before.trim_end();
                if before.len() == before_trimmed.len() && !before.is_empty() {
                    // No whitespace before the modifier — it could be
                    // part of an identifier.  Skip.
                    continue;
                }
                current = before;
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
    }
    current.trim_end()
}

// ─── Type Override Logic ────────────────────────────────────────────────────

/// Decide whether a docblock type should override a native type hint.
///
/// Returns `true` when the docblock type is likely to carry more
/// information than the native hint (e.g. `Collection<int, User>` vs
/// bare `object`), and `false` when overriding would lose precision
/// (e.g. both are scalars).
pub fn should_override_type(docblock_type: &str, native_type: &str) -> bool {
    // If the docblock type is semantically equivalent to the native type
    // after normalizing nullable syntax (`?X` ↔ `X|null`), there is no
    // value in overriding — the docblock doesn't carry any extra
    // information.  For example `callable|null` vs `?callable`, or
    // `null|string` vs `?string`.
    if normalize_nullable(docblock_type) == normalize_nullable(native_type) {
        return false;
    }

    // If the docblock type is itself a scalar, there's no value in
    // overriding — it wouldn't help with class resolution anyway.
    // However, a scalar base with generic parameters (e.g.
    // `array<int, User>`, `iterable<string, Order>`) carries more
    // type information than the bare native hint and should be kept
    // so that downstream consumers (foreach element resolution, array
    // destructuring, etc.) can extract the generic type arguments.
    let clean_doc = strip_nullable(docblock_type);
    if is_scalar(clean_doc) && !clean_doc.contains('<') && !clean_doc.contains('{') {
        return false;
    }

    // Strip nullable wrapper from the native hint for analysis.
    let clean_native = strip_nullable(native_type);

    // `array`, `iterable`, `callable`, and `Closure` are broad types
    // that docblocks commonly refine (e.g. `array` → `list<User>`,
    // `iterable` → `Collection<int, Order>`,
    // `callable` → `callable(Task): void`).
    // Allow override for these even though they appear in SCALAR_TYPES
    // (or are simple class names in the case of `Closure`).
    let native_lower = clean_native.to_ascii_lowercase();
    if native_lower == "array" || native_lower == "iterable" || native_lower == "callable" {
        return true;
    }
    // `\Closure` / `Closure` is a class, not scalar, but docblocks
    // often refine it with a callable signature like `Closure(int): bool`.
    let native_base = clean_native.strip_prefix('\\').unwrap_or(clean_native);
    if native_base == "Closure" {
        return true;
    }

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

    // If the docblock type carries generic parameters (e.g.
    // `class-string<T>`, `non-empty-array<int>`), it refines the
    // native type even when the native type is scalar.
    if clean_doc.contains('<') || clean_doc.contains('{') {
        return true;
    }

    // Simple case: if the native type is a scalar, don't override.
    !is_scalar(clean_native)
}

// ─── Docblock Text Extraction ───────────────────────────────────────────────

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

// ─── Effective Type Resolution ──────────────────────────────────────────────

/// Pick the best available type between a native type hint and a docblock
/// annotation.
///
/// When both are present, the docblock type is used only if
/// [`should_override_type`] approves (i.e. the native hint is broad enough
/// to refine).  Malformed docblock types with unclosed brackets are
/// partially recovered or discarded.
pub fn resolve_effective_type(
    native_type: Option<&str>,
    docblock_type: Option<&str>,
) -> Option<String> {
    // When the docblock type has unclosed brackets (e.g. a multi-line
    // `@return` that couldn't be fully joined), treat it as broken and
    // attempt partial recovery.  If recovery yields nothing useful, fall
    // back to the native type so that resolution is never blocked by a
    // malformed PHPDoc annotation.
    let sanitised_doc = docblock_type.and_then(|doc| {
        if has_unclosed_brackets(doc) {
            let base = recover_base_type(doc);
            if base.is_empty() {
                None
            } else {
                Some(base.to_string())
            }
        } else {
            Some(doc.to_string())
        }
    });

    match (native_type, sanitised_doc.as_deref()) {
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

/// Count `{` and `}` characters on a line, skipping those inside string
/// literals.  Returns `(open_count, close_count)`.
fn count_braces_on_line(line: &str) -> (i32, i32) {
    let mut opens = 0i32;
    let mut closes = 0i32;
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut prev = '\0';

    for ch in line.chars() {
        if in_single_quote {
            if ch == '\'' && prev != '\\' {
                in_single_quote = false;
            }
            prev = ch;
            continue;
        }
        if in_double_quote {
            if ch == '"' && prev != '\\' {
                in_double_quote = false;
            }
            prev = ch;
            continue;
        }
        match ch {
            '\'' => in_single_quote = true,
            '"' => in_double_quote = true,
            '{' => opens += 1,
            '}' => closes += 1,
            _ => {}
        }
        prev = ch;
    }

    (opens, closes)
}

/// Generic tag extraction: find `@tag TypeName` and return the cleaned type.
///
/// **Skips** PHPStan conditional return types (those starting with `(`).
/// Use [`super::extract_conditional_return_type`] for those.
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

    let lines: Vec<&str> = inner.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        // Strip leading whitespace and the `*` gutter common in docblocks.
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix(tag) {
            // The tag must be followed by whitespace (or be exactly `@tag`
            // at end-of-line, which is invalid and we skip).
            let rest = rest.trim_start();
            if rest.is_empty() {
                i += 1;
                continue;
            }

            // PHPStan conditional return types start with `(` — skip them
            // here; they are handled by `extract_conditional_return_type`.
            if rest.starts_with('(') {
                return None;
            }

            // Extract the type token, respecting `<…>` nesting so that
            // generics like `Collection<int, User>` are treated as one unit.
            //
            // When the type spans multiple docblock lines (e.g.
            // `@return static<\n *   int,\n *   string\n * >`), the
            // single-line `split_type_token` will hit end-of-line with
            // unclosed brackets.  In that case, collect continuation
            // lines until brackets are balanced, then re-parse.
            let (type_str, _remainder) = split_type_token(rest);
            let needs_continuation = has_unclosed_brackets(type_str);

            if !needs_continuation {
                return Some(clean_type(type_str));
            }

            // ── Multi-line type: join continuation lines ────────
            let mut joined = rest.to_string();
            let mut j = i + 1;
            while j < lines.len() {
                let cont = lines[j].trim().trim_start_matches('*').trim();
                // Stop if we hit another tag or an empty line.
                if cont.starts_with('@') {
                    break;
                }
                joined.push(' ');
                joined.push_str(cont);
                // Check whether brackets are now balanced.
                if !has_unclosed_brackets(&joined) {
                    break;
                }
                j += 1;
            }

            let joined = normalize_bracket_whitespace(&joined);
            let (type_str, _) = split_type_token(&joined);
            let type_str = if has_unclosed_brackets(type_str) {
                // Brackets still unclosed — partially recover by
                // stripping the unclosed generic/brace suffix to get
                // the base type (e.g. `static<…broken` → `static`).
                recover_base_type(type_str)
            } else {
                type_str
            };

            if type_str.is_empty() {
                return None;
            }
            return Some(clean_type(type_str));
        }
        i += 1;
    }
    None
}

/// Collapse whitespace immediately after `<` or `{` and immediately
/// before `>` or `}` so that multi-line joined types like
/// `array< string, int >` become `array<string, int>`.
fn normalize_bracket_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        let c = chars[i];
        out.push(c);
        // After `<` or `{`, skip whitespace.
        if (c == '<' || c == '{') && i + 1 < len {
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            i = j;
            continue;
        }
        // Before `>` or `}`, trim trailing whitespace already in `out`.
        if (c == '>' || c == '}') && !out.is_empty() {
            // We already pushed c — remove it, trim trailing ws, re-push.
            out.pop();
            let trimmed_len = out.trim_end().len();
            out.truncate(trimmed_len);
            out.push(c);
        }
        i += 1;
    }
    out
}

/// Check whether a type string has unclosed `<…>` or `{…}` brackets.
fn has_unclosed_brackets(s: &str) -> bool {
    let mut angle: i32 = 0;
    let mut brace: i32 = 0;
    for c in s.chars() {
        match c {
            '<' => angle += 1,
            '>' if angle > 0 => angle -= 1,
            '{' => brace += 1,
            '}' if brace > 0 => brace -= 1,
            _ => {}
        }
    }
    angle != 0 || brace != 0
}

/// Attempt to recover a usable base type from a type string with unclosed
/// brackets.  Truncates at the first unclosed `<` or `{` and returns the
/// base portion (e.g. `static<…broken` → `static`,
/// `Collection<int, User` → `Collection`).  Returns an empty string if
/// nothing useful can be recovered.
fn recover_base_type(s: &str) -> &str {
    // Walk forward and find the position where the first `<` or `{`
    // opens without a corresponding close.
    let mut angle: i32 = 0;
    let mut brace: i32 = 0;
    let mut first_unclosed = None;
    for (i, c) in s.char_indices() {
        match c {
            '<' => {
                if angle == 0 && brace == 0 && first_unclosed.is_none() {
                    first_unclosed = Some(i);
                }
                angle += 1;
            }
            '>' if angle > 0 => {
                angle -= 1;
                if angle == 0 && brace == 0 {
                    first_unclosed = None;
                }
            }
            '{' => {
                if brace == 0 && angle == 0 && first_unclosed.is_none() {
                    first_unclosed = Some(i);
                }
                brace += 1;
            }
            '}' if brace > 0 => {
                brace -= 1;
                if brace == 0 && angle == 0 {
                    first_unclosed = None;
                }
            }
            _ => {}
        }
    }
    match first_unclosed {
        Some(pos) => {
            let base = s[..pos].trim();
            if base.is_empty() { "" } else { base }
        }
        None => s,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_deprecation_message ─────────────────────────────────

    #[test]
    fn bare_deprecated_tag() {
        let doc = "/** @deprecated */";
        assert_eq!(extract_deprecation_message(doc), Some(String::new()));
    }

    #[test]
    fn deprecated_tag_with_message() {
        let doc = "/** @deprecated Use collect() instead. */";
        assert_eq!(
            extract_deprecation_message(doc),
            Some("Use collect() instead.".to_string())
        );
    }

    #[test]
    fn deprecated_tag_with_version() {
        let doc = "/**\n * @deprecated since 2.0\n */";
        assert_eq!(
            extract_deprecation_message(doc),
            Some("since 2.0".to_string())
        );
    }

    #[test]
    fn deprecated_tag_with_tab_separator() {
        let doc = "/** @deprecated\tUse foo() */";
        assert_eq!(
            extract_deprecation_message(doc),
            Some("Use foo()".to_string())
        );
    }

    #[test]
    fn no_deprecated_tag() {
        let doc = "/** @return string */";
        assert_eq!(extract_deprecation_message(doc), None);
    }

    #[test]
    fn deprecated_bare_on_own_line() {
        let doc = "/**\n * @deprecated\n */";
        assert_eq!(extract_deprecation_message(doc), Some(String::new()));
    }

    #[test]
    fn deprecated_with_message_multiline_docblock() {
        let doc = "/**\n * Some description.\n * @deprecated Use newMethod() instead.\n * @return void\n */";
        assert_eq!(
            extract_deprecation_message(doc),
            Some("Use newMethod() instead.".to_string())
        );
    }

    #[test]
    fn has_deprecated_tag_returns_true() {
        let doc = "/** @deprecated Use foo() */";
        assert!(has_deprecated_tag(doc));
    }

    #[test]
    fn has_deprecated_tag_returns_false() {
        let doc = "/** @return string */";
        assert!(!has_deprecated_tag(doc));
    }
}
