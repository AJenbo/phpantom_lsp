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

use super::types::{base_class_name, clean_type, is_scalar, split_type_token, strip_nullable};

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

/// Check whether a PHPDoc block contains an `@deprecated` tag.
///
/// Handles common formats:
///   - `@deprecated`
///   - `@deprecated Some explanation text`
///   - `@deprecated since 2.0`
///
/// Returns `true` if the tag is present, `false` otherwise.
pub fn has_deprecated_tag(docblock: &str) -> bool {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if trimmed == "@deprecated"
            || trimmed.starts_with("@deprecated ")
            || trimmed.starts_with("@deprecated\t")
        {
            return true;
        }
    }

    false
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

    for line in search_area.lines().rev() {
        let trimmed = line.trim();

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
            if let Some(name) = remainder.split_whitespace().next()
                && name == var_name
            {
                return Some(type_token.to_string());
            }
        }
    }

    None
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

        // Skip annotations that belong to a deeper (inner) scope.
        if brace_depth > 0 {
            continue;
        }

        // Quick reject: must mention the variable name.
        if !trimmed.contains(var_name) {
            continue;
        }

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

// ─── Type Override Logic ────────────────────────────────────────────────────

/// Decide whether a docblock type should override a native type hint.
///
/// Returns `true` when the docblock type is likely to carry more
/// information than the native hint (e.g. `Collection<int, User>` vs
/// bare `object`), and `false` when overriding would lose precision
/// (e.g. both are scalars).
pub fn should_override_type(docblock_type: &str, native_type: &str) -> bool {
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

    // `array` and `iterable` are broad container types that docblocks
    // commonly refine (e.g. `array` → `list<User>`, `iterable` →
    // `Collection<int, Order>`).  Allow override for these even though
    // they appear in SCALAR_TYPES.
    let native_lower = clean_native.to_ascii_lowercase();
    if native_lower == "array" || native_lower == "iterable" {
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
