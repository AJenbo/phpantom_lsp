//! Template, generics, and type alias tag extraction.
//!
//! This submodule handles `@template` (including `-covariant` /
//! `-contravariant` variants), `@extends` / `@implements` / `@use`
//! generic binding tags, `@phpstan-type` / `@psalm-type` local type
//! aliases, `@phpstan-import-type` / `@psalm-import-type` imported
//! aliases, and `class-string<T>` conditional return type synthesis.

use std::collections::HashMap;

use super::types::{split_generic_args, split_type_token};
use crate::types::{ConditionalReturnType, ParamCondition};

// ─── Template Parameters ────────────────────────────────────────────────────

/// Extract template parameter names from `@template` tags in a docblock.
///
/// Handles the common PHPStan / Psalm variants:
///   - `@template T`
///   - `@template TKey of array-key`
///   - `@template-covariant TValue`
///   - `@template-contravariant TValue`
///   - `@phpstan-template T`
///   - `@phpstan-template-covariant TValue`
///
/// Returns a list of template parameter names (e.g. `["T", "TKey"]`).
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

// ─── Template Parameter Bindings ────────────────────────────────────────────

/// Extract `@param` tags that bind a template parameter to a function
/// parameter.
///
/// Given a list of known `template_params` (e.g. `["T"]`), scans the
/// docblock for `@param T $varName` (or `@param ?T $varName`,
/// `@param T|null $varName`) and returns `(template_name, "$varName")`
/// pairs.
pub fn extract_template_param_bindings(
    docblock: &str,
    template_params: &[String],
) -> Vec<(String, String)> {
    if template_params.is_empty() {
        return Vec::new();
    }

    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut results = Vec::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Extract the full type token (respects `<…>` nesting).
            let (type_token, remainder) = split_type_token(rest);

            // The next token should be the parameter name (e.g. `$model`).
            let param_name = match remainder.split_whitespace().next() {
                Some(name) if name.starts_with('$') => name,
                _ => continue,
            };

            // Strip nullable prefix and `|null` suffix to get the core type.
            let core = type_token.strip_prefix('?').unwrap_or(type_token);
            // Handle `T|null` — split on `|` and check non-null parts.
            let matched_template = core
                .split('|')
                .map(str::trim)
                .filter(|part| *part != "null")
                .find(|part| template_params.iter().any(|t| t == part));

            if let Some(tpl_name) = matched_template {
                results.push((tpl_name.to_string(), param_name.to_string()));
            }
        }
    }

    results
}

// ─── Generics Tags (@extends, @implements, @use) ────────────────────────────

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
            let args: Vec<String> = split_generic_args(inner_generics)
                .into_iter()
                .map(|a| a.strip_prefix('\\').unwrap_or(a).to_string())
                .collect();
            if !args.is_empty() {
                results.push((base_name.to_string(), args));
            }
        }
        // No `<…>` means no generic args — skip.
    }

    results
}

// ─── Type Aliases ───────────────────────────────────────────────────────────

/// Extract all `@phpstan-type` / `@psalm-type` local type aliases and
/// `@phpstan-import-type` / `@psalm-import-type` imported aliases from a
/// docblock.
///
/// Returns a map from alias name to definition string.  For imported
/// aliases the definition has the form `"from:ClassName:OriginalName"` so
/// that the resolver can look up the alias in the source class.
pub fn extract_type_aliases(docblock: &str) -> HashMap<String, String> {
    let inner = docblock
        .trim()
        .strip_prefix("/**")
        .unwrap_or(docblock)
        .strip_suffix("*/")
        .unwrap_or(docblock);

    let mut aliases = HashMap::new();

    for line in inner.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();

        // ── Local type alias: @phpstan-type / @psalm-type ──
        if let Some(rest) = trimmed
            .strip_prefix("@phpstan-type")
            .or_else(|| trimmed.strip_prefix("@psalm-type"))
        {
            // Must not be `@phpstan-type-alias` or similar
            if rest.starts_with('-') {
                continue;
            }
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Split into alias name and definition.
            // Format: `AliasName = Definition` or `AliasName Definition`
            if let Some((name, def)) = parse_local_type_alias(rest)
                && !name.is_empty()
                && !def.is_empty()
            {
                aliases.insert(name.to_string(), def.to_string());
            }
            continue;
        }

        // ── Imported type alias: @phpstan-import-type / @psalm-import-type ──
        if let Some(rest) = trimmed
            .strip_prefix("@phpstan-import-type")
            .or_else(|| trimmed.strip_prefix("@psalm-import-type"))
        {
            let rest = rest.trim_start();
            if rest.is_empty() {
                continue;
            }

            // Format: `TypeName from ClassName` or `TypeName from ClassName as LocalAlias`
            if let Some((alias_name, definition)) = parse_import_type_alias(rest)
                && !alias_name.is_empty()
                && !definition.is_empty()
            {
                aliases.insert(alias_name, definition);
            }
        }
    }

    aliases
}

/// Parse a local `@phpstan-type` alias definition.
///
/// Accepts both `AliasName = Definition` and `AliasName Definition` forms.
/// The definition may contain complex types with `{…}`, `<…>`, `(…)` nesting.
///
/// Returns `(alias_name, definition)` or `None` if parsing fails.
fn parse_local_type_alias(rest: &str) -> Option<(&str, &str)> {
    // The alias name is the first word (identifier characters).
    let name_end = rest
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    let name = &rest[..name_end];
    if name.is_empty() {
        return None;
    }

    let after_name = rest[name_end..].trim_start();

    // Optional `=` separator
    let definition = after_name
        .strip_prefix('=')
        .unwrap_or(after_name)
        .trim_start();

    if definition.is_empty() {
        return None;
    }

    // The definition runs to the end of the line (docblock lines are
    // already split).  Trim trailing whitespace.
    let definition = definition.trim_end();

    Some((name, definition))
}

/// Parse an `@phpstan-import-type` alias.
///
/// Format: `TypeName from ClassName` or `TypeName from ClassName as LocalAlias`
///
/// Returns `(local_alias_name, "from:ClassName:OriginalName")` so the
/// resolver can look up the alias in the source class.
fn parse_import_type_alias(rest: &str) -> Option<(String, String)> {
    // Split: TypeName from ClassName [as LocalAlias]
    let parts: Vec<&str> = rest.split_whitespace().collect();

    // Minimum: TypeName from ClassName  (3 parts)
    if parts.len() < 3 || parts[1] != "from" {
        return None;
    }

    let original_name = parts[0];
    let source_class = parts[2];

    // Check for `as LocalAlias`
    let alias_name = if parts.len() >= 5 && parts[3] == "as" {
        parts[4].to_string()
    } else {
        original_name.to_string()
    };

    let definition = format!("from:{}:{}", source_class, original_name);

    Some((alias_name, definition))
}

// ─── Conditional Return Type Synthesis ──────────────────────────────────────

/// Synthesize a conditional return type from `@template` + `@param class-string<T>`
/// patterns.
///
/// When a method declares a template parameter (e.g. `@template TClass`)
/// whose return type is that template parameter, and a `@param` annotation
/// binds it via `class-string<TClass>`, the method effectively returns
/// an instance of whatever class name is passed as that argument.
///
/// This function detects that pattern and produces a
/// [`ConditionalReturnType`] so that the resolver can substitute the
/// concrete class at call sites.
///
/// Returns `None` if the pattern is not detected, or if
/// `has_existing_conditional` is true (an explicit conditional return type
/// in the docblock takes precedence).
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
