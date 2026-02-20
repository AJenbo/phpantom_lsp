//! Virtual member tag extraction (`@property`, `@method`).
//!
//! This submodule handles extracting magic property and method declarations
//! from class-level PHPDoc comments:
//!
//!   - `@property Type $name` / `@property-read` / `@property-write`
//!   - `@method ReturnType methodName(ParamType $param, ...)`
//!   - `@method static ReturnType methodName(...)`

use super::types::{clean_type, split_type_token};
use crate::types::{MethodInfo, ParameterInfo, Visibility};

// ─── @property Tags ─────────────────────────────────────────────────────────

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
/// name does **not** include the `$` prefix.
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

// ─── @method Tags ───────────────────────────────────────────────────────────

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
            template_params: Vec::new(),
            template_bindings: Vec::new(),
        });
    }

    results
}

// ─── Internal Helpers ───────────────────────────────────────────────────────

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
