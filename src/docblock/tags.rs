//! PHPDoc tag extraction.
//!
//! This submodule handles extracting type information from PHPDoc comments
//! (`/** ... */`), specifically `@return`, `@var`, `@property`, `@method`,
//! `@mixin`, `@deprecated`, and `@phpstan-assert` / `@psalm-assert` tags.
//!
//! It also provides a compatibility check ([`should_override_type`]) so that
//! a docblock type only overrides a native type hint when the native hint is
//! broad enough to be refined (e.g. `object`, `mixed`, or another class name)
//! and is *not* a concrete scalar that could never be an object.

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::types::{AssertionKind, MethodInfo, ParameterInfo, TypeAssertion, Visibility};

use crate::types::{ConditionalReturnType, ParamCondition};

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

    for line in search_area.lines().rev() {
        let trimmed = line.trim();

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

/// Extract all `@property` tags from a class-level docblock.
///
/// PHPDoc `@property` tags declare magic properties that are accessible via
/// `__get` / `__set`.  The format is:
///
///   - `@property Type $name`
///   - `@property null|Type $name`
///   - `@property ?Type $name`
///   - `@property-read Type $name`
///   - `@property-write Type $name`
///
/// Returns a list of `(property_name, cleaned_type)` pairs.  The property
/// name is returned **without** the `$` prefix.
pub fn extract_property_tags(docblock: &str) -> Vec<(String, String)> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        // Match @property, @property-read, and @property-write
        let rest = if let Some(r) = trimmed.strip_prefix("@property-read") {
            r
        } else if let Some(r) = trimmed.strip_prefix("@property-write") {
            r
        } else if let Some(r) = trimmed.strip_prefix("@property") {
            r
        } else {
            continue;
        };

        // The tag must be followed by whitespace.
        let rest = rest.trim_start();
        if rest.is_empty() {
            continue;
        }

        // The type may be a compound like `null|int`, `?Foo`, or a generic
        // like `Collection<int, Model>` that spans multiple whitespace-
        // delimited tokens.  We use `split_type_token` to extract the full
        // type (respecting `<…>` nesting) and then scan the remainder for
        // the `$name`.
        //
        // Format: @property Type $name  (or)  @property $name
        if rest.starts_with('$') {
            // No explicit type: `@property $name`
            let prop_name = rest.split_whitespace().next().unwrap_or(rest);
            let name = prop_name.strip_prefix('$').unwrap_or(prop_name);
            if name.is_empty() {
                continue;
            }
            results.push((name.to_string(), String::new()));
            continue;
        }

        // Extract the type token, respecting `<…>` nesting so that
        // generics like `Collection<int, Model>` are treated as one unit.
        let (type_token, remainder) = split_type_token(rest);

        // Find the `$name` in the remainder.
        let prop_name = match remainder.split_whitespace().find(|t| t.starts_with('$')) {
            Some(name) => name,
            None => continue,
        };

        let name = prop_name.strip_prefix('$').unwrap_or(prop_name);
        if name.is_empty() {
            continue;
        }

        let cleaned = clean_type(type_token);
        results.push((name.to_string(), cleaned));
    }

    results
}

/// Extract all `@method` tags from a class-level docblock.
///
/// PHPDoc `@method` tags declare magic methods that are accessible via
/// `__call` / `__callStatic`.  The format is:
///
///   - `@method ReturnType methodName(ParamType $param, ...)`
///   - `@method static ReturnType methodName(ParamType $param, ...)`
///   - `@method methodName(ParamType $param, ...)`  (no return type)
///
/// Returns a list of `MethodInfo` structs.  Parameters are parsed with
/// type hints and default-value detection where possible.
pub fn extract_method_tags(docblock: &str) -> Vec<MethodInfo> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        let rest = match trimmed.strip_prefix("@method") {
            Some(r) => r,
            None => continue,
        };

        // The tag must be followed by whitespace.
        let rest = rest.trim_start();
        if rest.is_empty() {
            continue;
        }

        // Check for optional `static` keyword.
        let (is_static, rest) = if let Some(after_static) = rest.strip_prefix("static") {
            // "static" must be followed by whitespace or `(` to avoid
            // matching a method literally named "staticFoo".
            if after_static.is_empty() {
                continue;
            }
            let next_char = after_static.chars().next().unwrap();
            if next_char.is_whitespace() || next_char == '(' {
                (true, after_static.trim_start())
            } else {
                (false, rest)
            }
        } else {
            (false, rest)
        };

        // Find the opening parenthesis — the method name is the token
        // immediately before it.
        let paren_pos = match rest.find('(') {
            Some(p) => p,
            None => continue,
        };

        let before_paren = &rest[..paren_pos];
        let after_paren = &rest[paren_pos + 1..]; // after '('

        // Split `before_paren` into optional return type + method name.
        // The method name is the last whitespace-delimited token.
        let before_paren = before_paren.trim();
        if before_paren.is_empty() {
            continue;
        }

        let (return_type_raw, method_name) =
            if let Some(last_space) = before_paren.rfind(|c: char| c.is_whitespace()) {
                let ret = before_paren[..last_space].trim();
                let name = before_paren[last_space..].trim();
                (Some(ret), name)
            } else {
                // Only one token — that's the method name, no return type.
                (None, before_paren)
            };

        if method_name.is_empty() {
            continue;
        }

        let return_type = return_type_raw.map(clean_type);
        let return_type = match return_type {
            Some(ref s) if s.is_empty() => None,
            other => other,
        };

        // Parse parameters from the content between `(` and `)`.
        let params_str = if let Some(close_paren) = after_paren.rfind(')') {
            after_paren[..close_paren].trim()
        } else {
            after_paren.trim()
        };

        let parameters = if params_str.is_empty() {
            Vec::new()
        } else {
            parse_method_tag_params(params_str)
        };

        results.push(MethodInfo {
            name: method_name.to_string(),
            parameters,
            return_type,
            is_static,
            visibility: Visibility::Public,
            conditional_return: None,
            is_deprecated: false,
        });
    }

    results
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

// ─── Generics / Template Support ────────────────────────────────────────────

/// Extract template parameter names from `@template` tags in a docblock.
///
/// Handles the common PHPStan / Psalm variants:
///   - `@template T`
///   - `@template TKey of array-key`
///   - `@template-covariant TValue`
///   - `@template-contravariant TValue`
///   - `@phpstan-template T`
///
/// Returns the parameter names in declaration order (e.g. `["TKey", "TValue"]`).
pub fn extract_template_params(docblock: &str) -> Vec<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        // Match all @template variants:
        //   @template, @template-covariant, @template-contravariant,
        //   @phpstan-template, @phpstan-template-covariant, etc.
        let rest = if let Some(r) = trimmed.strip_prefix("@phpstan-template") {
            r
        } else if let Some(r) = trimmed.strip_prefix("@template") {
            r
        } else {
            continue;
        };

        // After stripping the tag prefix, we may have a variance suffix
        // like `-covariant` or `-contravariant` still attached.
        let rest = if let Some(r) = rest.strip_prefix("-covariant") {
            r
        } else if let Some(r) = rest.strip_prefix("-contravariant") {
            r
        } else {
            rest
        };

        // Must be followed by whitespace.
        let rest = rest.trim_start();
        if rest.is_empty() {
            continue;
        }

        // The template parameter name is the first whitespace-delimited token.
        if let Some(name) = rest.split_whitespace().next() {
            // Sanity: template names are identifiers (start with a letter or _).
            if name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            {
                results.push(name.to_string());
            }
        }
    }

    results
}

/// Extract generic type arguments from `@extends`, `@implements`, or `@use`
/// tags (and their `@phpstan-` prefixed variants) in a docblock.
///
/// The `tag` parameter should be one of `"@extends"`, `"@implements"`, or
/// `"@use"`.
///
/// For example, given `@extends Collection<int, Language>`, returns
/// `[("Collection", ["int", "Language"])]`.
///
/// Handles:
///   - `@extends Collection<int, Language>`
///   - `@phpstan-extends Collection<int, Language>`
///   - `@implements ArrayAccess<string, User>`
///   - Nested generics: `@extends Base<array<int, string>, User>`
pub fn extract_generics_tag(docblock: &str, tag: &str) -> Vec<(String, Vec<String>)> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    // Build the tag variants we accept.  For `@extends` we also accept
    // `@phpstan-extends`.
    let bare_tag = tag.strip_prefix('@').unwrap_or(tag);
    let phpstan_tag = format!("@phpstan-{bare_tag}");

    let mut results = Vec::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        let rest = if let Some(r) = trimmed.strip_prefix(&phpstan_tag) {
            r
        } else if let Some(r) = trimmed.strip_prefix(tag) {
            r
        } else {
            continue;
        };

        // Must be followed by whitespace.
        let rest = rest.trim_start();
        if rest.is_empty() {
            continue;
        }

        // Extract the full type token (e.g. `Collection<int, Language>`),
        // respecting `<…>` nesting.
        let (type_token, _remainder) = split_type_token(rest);

        // Split into base class name and generic arguments.
        if let Some(angle_pos) = type_token.find('<') {
            let base_name = type_token[..angle_pos].trim();
            let base_name = base_name.strip_prefix('\\').unwrap_or(base_name);
            if base_name.is_empty() {
                continue;
            }

            // Extract the inner generic arguments (between `<` and `>`).
            let inner_generics = &type_token[angle_pos + 1..];
            let inner_generics = inner_generics
                .strip_suffix('>')
                .unwrap_or(inner_generics)
                .trim();

            if inner_generics.is_empty() {
                continue;
            }

            // Split on commas respecting nesting.
            let args = split_generic_args(inner_generics);
            if !args.is_empty() {
                results.push((base_name.to_string(), args));
            }
        }
        // No `<…>` means no generic args — skip.
    }

    results
}

/// Attempt to synthesize a `ConditionalReturnType` from method-level
/// `@template` annotations.
///
/// When a method (or standalone function) declares `@template T`,
/// `@param class-string<T> $class`, and `@return T`, this creates a
/// conditional return type that resolves `T` from the call-site argument.
///
/// For example, given:
/// ```text
/// @template T
/// @param class-string<T> $class
/// @return T
/// ```
/// Calling `find(User::class)` will resolve the return type to `User`.
///
/// Returns `None` when:
///   - No template params are provided
///   - The return type does not reference a template param
///   - No `@param class-string<T>` annotation is found for the template param
///   - An existing conditional return type is already present (pass `true`
///     for `has_existing_conditional` to skip synthesis)
pub fn synthesize_template_conditional(
    docblock: &str,
    template_params: &[String],
    return_type: Option<&str>,
    has_existing_conditional: bool,
) -> Option<ConditionalReturnType> {
    // Don't override an existing conditional return type.
    if has_existing_conditional {
        return None;
    }

    if template_params.is_empty() {
        return None;
    }

    let ret = return_type?;

    // Strip nullable prefix so that `?T` matches template param `T`.
    let stripped = ret.strip_prefix('?').unwrap_or(ret);

    // Check if the (stripped) return type is one of the template params.
    if !template_params.iter().any(|t| t == stripped) {
        return None;
    }

    // Find a `@param class-string<T> $paramName` annotation for this
    // template param, and extract the parameter name (without `$`).
    let param_name = find_class_string_param_name(docblock, stripped)?;

    Some(ConditionalReturnType::Conditional {
        param_name,
        condition: ParamCondition::ClassString,
        // `then_type` is unused for ClassString — the resolver extracts
        // the class name directly from the argument (e.g. `User::class`
        // → `"User"`).
        then_type: Box::new(ConditionalReturnType::Concrete("mixed".into())),
        // `else_type` is used when the argument is not a `::class`
        // literal — `mixed` will produce `None` from resolution, which
        // lets the caller fall back to the plain return type.
        else_type: Box::new(ConditionalReturnType::Concrete("mixed".into())),
    })
}

/// Search a docblock for a `@param class-string<T> $paramName` annotation
/// where `T` matches the given `template_name`.
///
/// Returns the parameter name **without** the `$` prefix, or `None` if no
/// matching annotation is found.
///
/// Handles common type variants:
///   - `class-string<T>`
///   - `?class-string<T>` (nullable)
///   - `class-string<T>|null` (union with null)
fn find_class_string_param_name(docblock: &str, template_name: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let pattern = format!("class-string<{}>", template_name);

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Extract the full type token (respects `<…>` nesting).
            let (type_token, remainder) = split_type_token(rest);

            // Check if the type token contains `class-string<T>`.
            // We strip `?` prefix and check for the pattern.
            let check = type_token.strip_prefix('?').unwrap_or(type_token);
            // Also handle `class-string<T>|null` — split on `|` and
            // check each part.
            let matches = check.split('|').any(|part| part.trim() == pattern);

            if !matches {
                continue;
            }

            // The next token after the type should be `$paramName`.
            // However, `split_type_token` splits at the closing `>`,
            // so if the type is `class-string<T>|null`, the remainder
            // will be `|null $class`.  Skip any union continuation
            // (`|part`) before looking for the `$` variable name.
            let mut search = remainder;
            while let Some(rest) = search.strip_prefix('|') {
                // Skip `|unionPart` — the next whitespace-delimited
                // token is the union type, not the variable name.
                let rest = rest.trim_start();
                let (_, after) = split_type_token(rest);
                search = after;
            }
            if let Some(var_name) = search.split_whitespace().next()
                && let Some(name) = var_name.strip_prefix('$')
            {
                return Some(name.to_string());
            }
        }
    }

    None
}

/// Split a comma-separated generic argument list, respecting `<…>` and `(…)`
/// nesting.  Returns cleaned argument strings.
///
/// - `"int, Language"` → `["int", "Language"]`
/// - `"int, array<string, mixed>"` → `["int", "array<string, mixed>"]`
fn split_generic_args(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            ',' if depth_angle == 0 && depth_paren == 0 => {
                let arg = s[start..i].trim();
                if !arg.is_empty() {
                    let arg = arg.strip_prefix('\\').unwrap_or(arg);
                    parts.push(arg.to_string());
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    // Push the last segment.
    let last = s[start..].trim();
    if !last.is_empty() {
        let last = last.strip_prefix('\\').unwrap_or(last);
        parts.push(last.to_string());
    }
    parts
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

            // PHPStan conditional return types start with `(` — skip them
            // here; they are handled by `extract_conditional_return_type`.
            if rest.starts_with('(') {
                return None;
            }

            // Extract the type token, respecting `<…>` nesting so that
            // generics like `Collection<int, User>` are treated as one unit.
            let (type_str, _remainder) = split_type_token(rest);

            return Some(clean_type(type_str));
        }
    }
    None
}

/// Parse the parameter list from a `@method` tag.
///
/// Handles formats like:
///   - `string $abstract, callable():mixed $mockDefinition = null`
///   - `array<string, mixed> $data, string $connection = null`
///
/// Splits on commas while respecting `<>` and `()` nesting.
fn parse_method_tag_params(params_str: &str) -> Vec<ParameterInfo> {
    let parts = split_params(params_str);
    let mut result = Vec::new();

    for part in &parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        // Check for default value: ` = ...` after the variable name.
        // We look for the last `$` to find the variable name, then check
        // if `=` follows.
        let has_default = part.contains('=');

        // Check for variadic `...`
        let is_variadic = part.contains("...");

        // Find the parameter name (token starting with `$`).
        // Scan tokens right-to-left to find the `$name` token (it may be
        // followed by `= default`).
        let dollar_pos = part.rfind('$');
        let (type_hint, param_name) = if let Some(dp) = dollar_pos {
            let name_and_rest = &part[dp..];
            // The name ends at whitespace, `=`, `)`, or end of string.
            let name_end = name_and_rest
                .find(|c: char| c.is_whitespace() || c == '=' || c == ')')
                .unwrap_or(name_and_rest.len());
            let name = &name_and_rest[..name_end];

            let before = part[..dp].trim().trim_end_matches("...");
            let type_str = if before.is_empty() {
                None
            } else {
                Some(clean_type(before))
            };
            let type_str = match type_str {
                Some(ref s) if s.is_empty() => None,
                other => other,
            };

            (type_str, name.to_string())
        } else {
            // No `$` found — treat the whole thing as a name-less param.
            // This is unusual but we handle it gracefully.
            continue;
        };

        let is_required = !has_default && !is_variadic;

        result.push(ParameterInfo {
            name: param_name,
            is_required,
            type_hint,
            is_variadic,
            is_reference: false,
        });
    }

    result
}

/// Split a parameter string on commas while respecting `<>` and `()`
/// nesting so that `array<string, mixed>` is not split.
fn split_params(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth_angle = 0i32;
    let mut depth_paren = 0i32;
    let mut start = 0;

    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle -= 1,
            '(' => depth_paren += 1,
            ')' => depth_paren -= 1,
            ',' if depth_angle == 0 && depth_paren == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    // Push the last segment.
    parts.push(&s[start..]);
    parts
}
