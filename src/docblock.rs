//! PHPDoc block parsing.
//!
//! This module extracts type information from PHPDoc comments (`/** ... */`),
//! specifically `@return`, `@var`, `@property`, and `@method` tags.  It also
//! provides a compatibility check so that a docblock type only overrides a
//! native type hint when the native hint is broad enough to be refined
//! (e.g. `object`, `mixed`, or another class name) and is *not* a concrete
//! scalar that could never be an object.
//!
//! Additionally, it supports PHPStan conditional return type annotations
//! such as:
//! ```text
//! @return ($abstract is class-string<TClass> ? TClass : mixed)
//! ```

use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::types::{ConditionalReturnType, MethodInfo, ParamCondition, ParameterInfo, Visibility};

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

        let cleaned = clean_type(class_name);
        if !cleaned.is_empty() {
            results.push(cleaned);
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
        // delimited tokens.  We take the first token as the type (for
        // `clean_type` purposes) and then scan forward until we find a
        // token starting with `$`.
        //
        // Format: @property Type $name  (or)  @property $name
        let mut parts = rest.split_whitespace();
        let first = match parts.next() {
            Some(t) => t,
            None => continue,
        };

        let (type_str, prop_name) = if first.starts_with('$') {
            // No explicit type: `@property $name`
            (None, first)
        } else {
            // Type is the first token; scan forward to find the `$name`.
            let mut found_name = None;
            for token in parts {
                if token.starts_with('$') {
                    found_name = Some(token);
                    break;
                }
            }
            match found_name {
                Some(name) => (Some(first), name),
                None => continue,
            }
        };

        let name = prop_name.strip_prefix('$').unwrap_or(prop_name);
        if name.is_empty() {
            continue;
        }

        let cleaned = type_str.map(clean_type);
        results.push((name.to_string(), cleaned.unwrap_or_default()));
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
        });
    }

    results
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
///
/// **Skips** PHPStan conditional return types (those starting with `(`).
/// Use [`extract_conditional_return_type`] for those.
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

            // The type is the first whitespace-delimited token.
            let type_str = rest.split_whitespace().next()?;

            return Some(clean_type(type_str));
        }
    }
    None
}

// ─── PHPStan Conditional Return Types ───────────────────────────────────────

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
pub fn extract_conditional_return_type(docblock: &str) -> Option<ConditionalReturnType> {
    // Collect the full @return content across multiple lines.
    let raw = extract_raw_return_content(docblock)?;
    let trimmed = raw.trim();
    if !trimmed.starts_with('(') {
        return None;
    }
    parse_conditional_expr(trimmed)
}

/// Extract the raw content after `@return` from a (possibly multi-line)
/// docblock, joining continuation lines.
///
/// For a conditional return type spanning multiple lines the content after
/// `@return` is concatenated (with a single space between lines) until the
/// parentheses are balanced or a new tag is encountered.
fn extract_raw_return_content(docblock: &str) -> Option<String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut collecting = false;
    let mut parts: Vec<String> = Vec::new();
    let mut paren_depth: i32 = 0;

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if !collecting {
            if let Some(rest) = trimmed.strip_prefix("@return") {
                let rest = rest.trim_start();
                if rest.is_empty() {
                    continue;
                }
                collecting = true;
                paren_depth += rest.chars().filter(|&c| c == '(').count() as i32;
                paren_depth -= rest.chars().filter(|&c| c == ')').count() as i32;
                parts.push(rest.to_string());
                if paren_depth <= 0 {
                    break;
                }
            }
        } else {
            // Stop if we hit another tag
            if trimmed.starts_with('@') {
                break;
            }
            if trimmed.is_empty() {
                continue;
            }
            paren_depth += trimmed.chars().filter(|&c| c == '(').count() as i32;
            paren_depth -= trimmed.chars().filter(|&c| c == ')').count() as i32;
            parts.push(trimmed.to_string());
            if paren_depth <= 0 {
                break;
            }
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(parts.join(" "))
}

/// Parse a conditional expression string into a [`ConditionalReturnType`].
///
/// Expected format (recursive):
/// ```text
/// ($paramName is ConditionType ? ThenType : ElseType)
/// ```
///
/// Where `ThenType` and `ElseType` can each be either a concrete type name
/// or another parenthesised conditional.
fn parse_conditional_expr(input: &str) -> Option<ConditionalReturnType> {
    let s = input.trim();

    // Must be wrapped in parens
    if !s.starts_with('(') || !s.ends_with(')') {
        // It's a concrete type
        let cleaned = clean_type(s);
        if cleaned.is_empty() {
            return None;
        }
        return Some(ConditionalReturnType::Concrete(cleaned));
    }

    // Strip outer parens
    let inner = &s[1..s.len() - 1];
    let inner = inner.trim();

    // Parse: $paramName is ConditionType ? ThenType : ElseType

    // 1. Extract $paramName
    let rest = inner.strip_prefix('$')?;
    let space_idx = rest.find(|c: char| c.is_whitespace())?;
    let param_name = rest[..space_idx].to_string();
    let rest = rest[space_idx..].trim_start();

    // 2. Expect "is"
    let rest = rest.strip_prefix("is")?.trim_start();

    // 3. Extract condition type (everything up to ` ? `)
    // We need to find ` ? ` but be careful about nested parens
    let question_pos = find_token_at_depth(rest, '?')?;
    let condition_str = rest[..question_pos].trim();
    let rest = rest[question_pos + 1..].trim_start();

    // 4. Parse condition
    let condition = parse_condition(condition_str);

    // 5. Parse then-type and else-type split by ` : `
    // We need to find `:` at depth 0
    let colon_pos = find_token_at_depth(rest, ':')?;
    let then_str = rest[..colon_pos].trim();
    let else_str = rest[colon_pos + 1..].trim();

    let then_type = parse_type_or_conditional(then_str)?;
    let else_type = parse_type_or_conditional(else_str)?;

    Some(ConditionalReturnType::Conditional {
        param_name,
        condition,
        then_type: Box::new(then_type),
        else_type: Box::new(else_type),
    })
}

/// Parse a string that is either a `(...)` conditional or a concrete type.
fn parse_type_or_conditional(s: &str) -> Option<ConditionalReturnType> {
    let s = s.trim();
    if s.starts_with('(') {
        parse_conditional_expr(s)
    } else {
        let cleaned = clean_type(s);
        if cleaned.is_empty() {
            return None;
        }
        Some(ConditionalReturnType::Concrete(cleaned))
    }
}

/// Find the position of `token` (e.g. `?` or `:`) at parenthesis depth 0.
///
/// Skips over `<…>` generics and `(…)` nested conditionals.
fn find_token_at_depth(s: &str, token: char) -> Option<usize> {
    let mut paren_depth = 0i32;
    let mut angle_depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            c if c == token && paren_depth == 0 && angle_depth == 0 => {
                return Some(i);
            }
            _ => {}
        }
    }
    None
}

/// Parse a condition string like `class-string<TClass>`, `null`, or `\Closure`.
fn parse_condition(s: &str) -> ParamCondition {
    let s = s.trim();
    if s.starts_with("class-string") {
        ParamCondition::ClassString
    } else if s.eq_ignore_ascii_case("null") {
        ParamCondition::IsNull
    } else {
        let cleaned = s.strip_prefix('\\').unwrap_or(s);
        ParamCondition::IsType(cleaned.to_string())
    }
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

    // ─── @method tag extraction ─────────────────────────────────────────

    #[test]
    fn method_tag_simple() {
        let doc = "/** @method MockInterface mock(string $abstract) */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "mock");
        assert_eq!(methods[0].return_type.as_deref(), Some("MockInterface"));
        assert!(!methods[0].is_static);
        assert_eq!(methods[0].parameters.len(), 1);
        assert_eq!(methods[0].parameters[0].name, "$abstract");
        assert_eq!(
            methods[0].parameters[0].type_hint.as_deref(),
            Some("string")
        );
        assert!(methods[0].parameters[0].is_required);
    }

    #[test]
    fn method_tag_static() {
        let doc = "/** @method static Decimal getAmountUntilBonusCashIsTriggered() */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "getAmountUntilBonusCashIsTriggered");
        assert_eq!(methods[0].return_type.as_deref(), Some("Decimal"));
        assert!(methods[0].is_static);
        assert!(methods[0].parameters.is_empty());
    }

    #[test]
    fn method_tag_no_return_type() {
        let doc = "/** @method assertDatabaseHas(string $table, array<string, mixed> $data, string $connection = null) */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "assertDatabaseHas");
        assert!(methods[0].return_type.is_none());
        assert_eq!(methods[0].parameters.len(), 3);
        assert_eq!(methods[0].parameters[0].name, "$table");
        assert_eq!(
            methods[0].parameters[0].type_hint.as_deref(),
            Some("string")
        );
        assert!(methods[0].parameters[0].is_required);
        assert_eq!(methods[0].parameters[1].name, "$data");
        assert_eq!(methods[0].parameters[1].type_hint.as_deref(), Some("array"));
        assert!(methods[0].parameters[1].is_required);
        assert_eq!(methods[0].parameters[2].name, "$connection");
        assert_eq!(
            methods[0].parameters[2].type_hint.as_deref(),
            Some("string")
        );
        assert!(!methods[0].parameters[2].is_required);
    }

    #[test]
    fn method_tag_fqn_return_type() {
        let doc = "/** @method \\Mockery\\MockInterface mock(string $abstract) */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(
            methods[0].return_type.as_deref(),
            Some("Mockery\\MockInterface")
        );
    }

    #[test]
    fn method_tag_callable_param() {
        let doc = "/** @method MockInterface mock(string $abstract, callable():mixed $mockDefinition = null) */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].parameters.len(), 2);
        assert_eq!(methods[0].parameters[1].name, "$mockDefinition");
        assert!(!methods[0].parameters[1].is_required);
    }

    #[test]
    fn method_tag_multiple() {
        let doc = concat!(
            "/**\n",
            " * @method \\Mockery\\MockInterface mock(string $abstract, callable():mixed $mockDefinition = null)\n",
            " * @method assertDatabaseHas(string $table, array<string, mixed> $data, string $connection = null)\n",
            " * @method assertDatabaseMissing(string $table, array<string, mixed> $data, string $connection = null)\n",
            " * @method static Decimal getAmountUntilBonusCashIsTriggered()\n",
            " */",
        );
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 4);
        assert_eq!(methods[0].name, "mock");
        assert!(!methods[0].is_static);
        assert_eq!(methods[1].name, "assertDatabaseHas");
        assert!(!methods[1].is_static);
        assert_eq!(methods[2].name, "assertDatabaseMissing");
        assert!(!methods[2].is_static);
        assert_eq!(methods[3].name, "getAmountUntilBonusCashIsTriggered");
        assert!(methods[3].is_static);
    }

    #[test]
    fn method_tag_no_params() {
        let doc = "/** @method string getName() */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].name, "getName");
        assert_eq!(methods[0].return_type.as_deref(), Some("string"));
        assert!(methods[0].parameters.is_empty());
    }

    #[test]
    fn method_tag_nullable_return() {
        let doc = "/** @method ?User findUser(int $id) */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].return_type.as_deref(), Some("?User"));
    }

    #[test]
    fn method_tag_none_when_missing() {
        let doc = "/** @property string $name */";
        let methods = extract_method_tags(doc);
        assert!(methods.is_empty());
    }

    #[test]
    fn method_tag_variadic_param() {
        let doc = "/** @method void addItems(string ...$items) */";
        let methods = extract_method_tags(doc);
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].parameters.len(), 1);
        assert!(methods[0].parameters[0].is_variadic);
        assert!(!methods[0].parameters[0].is_required);
    }

    // ─── @property tag extraction ───────────────────────────────────────

    #[test]
    fn property_tag_simple() {
        let doc = "/** @property Session $session */";
        let props = extract_property_tags(doc);
        assert_eq!(props, vec![("session".to_string(), "Session".to_string())]);
    }

    #[test]
    fn property_tag_nullable() {
        let doc = "/** @property ?int $count */";
        let props = extract_property_tags(doc);
        assert_eq!(props, vec![("count".to_string(), "?int".to_string())]);
    }

    #[test]
    fn property_tag_union_with_null() {
        let doc = "/** @property null|int $latest_id */";
        let props = extract_property_tags(doc);
        assert_eq!(props, vec![("latest_id".to_string(), "int".to_string())]);
    }

    #[test]
    fn property_tag_fqn() {
        let doc = "/** @property \\App\\Models\\User $user */";
        let props = extract_property_tags(doc);
        assert_eq!(
            props,
            vec![("user".to_string(), "App\\Models\\User".to_string())]
        );
    }

    #[test]
    fn property_tag_multiple() {
        let doc = concat!(
            "/**\n",
            " * @property null|int                    $latest_subscription_agreement_id\n",
            " * @property UserMobileVerificationState $mobile_verification_state\n",
            " */",
        );
        let props = extract_property_tags(doc);
        assert_eq!(props.len(), 2);
        assert_eq!(
            props[0],
            (
                "latest_subscription_agreement_id".to_string(),
                "int".to_string()
            )
        );
        assert_eq!(
            props[1],
            (
                "mobile_verification_state".to_string(),
                "UserMobileVerificationState".to_string()
            )
        );
    }

    #[test]
    fn property_tag_read_write_variants() {
        let doc = concat!(
            "/**\n",
            " * @property-read string $name\n",
            " * @property-write int $age\n",
            " */",
        );
        let props = extract_property_tags(doc);
        assert_eq!(props.len(), 2);
        assert_eq!(props[0], ("name".to_string(), "string".to_string()));
        assert_eq!(props[1], ("age".to_string(), "int".to_string()));
    }

    #[test]
    fn property_tag_no_type() {
        let doc = "/** @property $thing */";
        let props = extract_property_tags(doc);
        assert_eq!(props, vec![("thing".to_string(), "".to_string())]);
    }

    #[test]
    fn property_tag_generic_stripped() {
        let doc = "/** @property Collection<int, Model> $items */";
        let props = extract_property_tags(doc);
        assert_eq!(props, vec![("items".to_string(), "Collection".to_string())]);
    }

    #[test]
    fn property_tag_none_when_missing() {
        let doc = "/** @return Foo */";
        let props = extract_property_tags(doc);
        assert!(props.is_empty());
    }

    // ── extract_return_type (skips conditionals) ────────────────────────

    #[test]
    fn return_type_conditional_is_skipped() {
        let doc = concat!(
            "/**\n",
            " * @return ($abstract is class-string<TClass> ? TClass : mixed)\n",
            " */",
        );
        assert_eq!(extract_return_type(doc), None);
    }

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

    // ── extract_conditional_return_type ─────────────────────────────────

    #[test]
    fn conditional_simple_class_string() {
        let doc = concat!(
            "/**\n",
            " * @return ($abstract is class-string<TClass> ? TClass : mixed)\n",
            " */",
        );
        let result = extract_conditional_return_type(doc);
        assert!(result.is_some(), "Should parse a conditional return type");
        let cond = result.unwrap();
        match cond {
            ConditionalReturnType::Conditional {
                ref param_name,
                ref condition,
                ref then_type,
                ref else_type,
            } => {
                assert_eq!(param_name, "abstract");
                assert_eq!(*condition, ParamCondition::ClassString);
                assert_eq!(
                    **then_type,
                    ConditionalReturnType::Concrete("TClass".into())
                );
                assert_eq!(**else_type, ConditionalReturnType::Concrete("mixed".into()));
            }
            _ => panic!("Expected Conditional, got {:?}", cond),
        }
    }

    #[test]
    fn conditional_null_check() {
        let doc = concat!(
            "/**\n",
            " * @return ($guard is null ? \\Illuminate\\Contracts\\Auth\\Factory : \\Illuminate\\Contracts\\Auth\\StatefulGuard)\n",
            " */",
        );
        let result = extract_conditional_return_type(doc).unwrap();
        match result {
            ConditionalReturnType::Conditional {
                param_name,
                condition,
                then_type,
                else_type,
            } => {
                assert_eq!(param_name, "guard");
                assert_eq!(condition, ParamCondition::IsNull);
                assert_eq!(
                    *then_type,
                    ConditionalReturnType::Concrete("Illuminate\\Contracts\\Auth\\Factory".into())
                );
                assert_eq!(
                    *else_type,
                    ConditionalReturnType::Concrete(
                        "Illuminate\\Contracts\\Auth\\StatefulGuard".into()
                    )
                );
            }
            _ => panic!("Expected Conditional"),
        }
    }

    #[test]
    fn conditional_nested() {
        let doc = concat!(
            "/**\n",
            " * @return ($abstract is class-string<TClass> ? TClass : ($abstract is null ? \\Illuminate\\Foundation\\Application : mixed))\n",
            " */",
        );
        let result = extract_conditional_return_type(doc).unwrap();
        match result {
            ConditionalReturnType::Conditional {
                ref param_name,
                ref condition,
                ref then_type,
                ref else_type,
            } => {
                assert_eq!(param_name, "abstract");
                assert_eq!(*condition, ParamCondition::ClassString);
                assert_eq!(
                    **then_type,
                    ConditionalReturnType::Concrete("TClass".into())
                );
                // else_type should be another conditional
                match else_type.as_ref() {
                    ConditionalReturnType::Conditional {
                        param_name: inner_param,
                        condition: inner_cond,
                        then_type: inner_then,
                        else_type: inner_else,
                    } => {
                        assert_eq!(inner_param, "abstract");
                        assert_eq!(*inner_cond, ParamCondition::IsNull);
                        assert_eq!(
                            **inner_then,
                            ConditionalReturnType::Concrete(
                                "Illuminate\\Foundation\\Application".into()
                            )
                        );
                        assert_eq!(
                            **inner_else,
                            ConditionalReturnType::Concrete("mixed".into())
                        );
                    }
                    _ => panic!("Expected nested Conditional"),
                }
            }
            _ => panic!("Expected Conditional"),
        }
    }

    #[test]
    fn conditional_multiline() {
        let doc = concat!(
            "/**\n",
            " * Get the available container instance.\n",
            " *\n",
            " * @param  string|callable|null  $abstract\n",
            " * @return ($abstract is class-string<TClass>\n",
            " *     ? TClass\n",
            " *     : ($abstract is null\n",
            " *         ? \\Illuminate\\Foundation\\Application\n",
            " *         : mixed))\n",
            " */",
        );
        let result = extract_conditional_return_type(doc);
        assert!(result.is_some(), "Should parse multi-line conditional");
        match result.unwrap() {
            ConditionalReturnType::Conditional {
                param_name,
                condition,
                ..
            } => {
                assert_eq!(param_name, "abstract");
                assert_eq!(condition, ParamCondition::ClassString);
            }
            _ => panic!("Expected Conditional"),
        }
    }

    #[test]
    fn conditional_is_type() {
        let doc = concat!(
            "/**\n",
            " * @return ($job is \\Closure ? \\Illuminate\\Foundation\\Bus\\PendingClosureDispatch : \\Illuminate\\Foundation\\Bus\\PendingDispatch)\n",
            " */",
        );
        let result = extract_conditional_return_type(doc).unwrap();
        match result {
            ConditionalReturnType::Conditional {
                param_name,
                condition,
                then_type,
                else_type,
            } => {
                assert_eq!(param_name, "job");
                assert_eq!(condition, ParamCondition::IsType("Closure".into()));
                assert_eq!(
                    *then_type,
                    ConditionalReturnType::Concrete(
                        "Illuminate\\Foundation\\Bus\\PendingClosureDispatch".into()
                    )
                );
                assert_eq!(
                    *else_type,
                    ConditionalReturnType::Concrete(
                        "Illuminate\\Foundation\\Bus\\PendingDispatch".into()
                    )
                );
            }
            _ => panic!("Expected Conditional"),
        }
    }

    #[test]
    fn conditional_not_present() {
        let doc = "/** @return Application */";
        assert_eq!(extract_conditional_return_type(doc), None);
    }

    #[test]
    fn conditional_no_return_tag() {
        let doc = "/** Just a comment */";
        assert_eq!(extract_conditional_return_type(doc), None);
    }

    // ─── @mixin tag extraction ──────────────────────────────────────────────

    #[test]
    fn mixin_tag_simple() {
        let doc = concat!("/**\n", " * @mixin ShoppingCart\n", " */",);
        let mixins = extract_mixin_tags(doc);
        assert_eq!(mixins, vec!["ShoppingCart"]);
    }

    #[test]
    fn mixin_tag_fqn() {
        let doc = concat!("/**\n", " * @mixin \\App\\Models\\ShoppingCart\n", " */",);
        let mixins = extract_mixin_tags(doc);
        assert_eq!(mixins, vec!["App\\Models\\ShoppingCart"]);
    }

    #[test]
    fn mixin_tag_multiple() {
        let doc = concat!(
            "/**\n",
            " * @mixin ShoppingCart\n",
            " * @mixin Wishlist\n",
            " */",
        );
        let mixins = extract_mixin_tags(doc);
        assert_eq!(mixins, vec!["ShoppingCart", "Wishlist"]);
    }

    #[test]
    fn mixin_tag_none_when_missing() {
        let doc = "/** Just a comment */";
        let mixins = extract_mixin_tags(doc);
        assert!(mixins.is_empty());
    }

    #[test]
    fn mixin_tag_with_description() {
        let doc = concat!(
            "/**\n",
            " * @mixin ShoppingCart Some extra description\n",
            " */",
        );
        let mixins = extract_mixin_tags(doc);
        assert_eq!(mixins, vec!["ShoppingCart"]);
    }

    #[test]
    fn mixin_tag_generic_stripped() {
        let doc = concat!("/**\n", " * @mixin Collection<int, Model>\n", " */",);
        let mixins = extract_mixin_tags(doc);
        assert_eq!(mixins, vec!["Collection"]);
    }

    #[test]
    fn mixin_tag_mixed_with_other_tags() {
        let doc = concat!(
            "/**\n",
            " * @property string $name\n",
            " * @mixin ShoppingCart\n",
            " * @method int getId()\n",
            " */",
        );
        let mixins = extract_mixin_tags(doc);
        assert_eq!(mixins, vec!["ShoppingCart"]);
    }

    #[test]
    fn mixin_tag_empty_after_tag() {
        let doc = concat!("/**\n", " * @mixin\n", " */",);
        let mixins = extract_mixin_tags(doc);
        assert!(mixins.is_empty());
    }
}
