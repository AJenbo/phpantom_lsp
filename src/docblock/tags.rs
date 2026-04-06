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

use mago_docblock::document::TagKind;
use mago_span::HasSpan;
use mago_syntax::ast::*;

use crate::symbol_map::docblock::get_docblock_text_with_offset;
use crate::types::{AssertionKind, PhpVersion, TypeAssertion};
use crate::util::strip_fqn_prefix;

use super::parser::{DocblockInfo, collapse_newlines, parse_docblock_for_tags};
use super::types::split_type_token;
use crate::php_type::PhpType;

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
    extract_type_via_mago(docblock, &[TagKind::PhpstanReturn, TagKind::Return])
}

/// Like [`extract_return_type`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_return_type_from_info(info: &DocblockInfo) -> Option<String> {
    extract_type_via_mago_from_info(info, &[TagKind::PhpstanReturn, TagKind::Return])
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
    extract_deprecation_message_from_info(&parse_docblock_for_tags(docblock)?)
}

/// Like [`extract_deprecation_message`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_deprecation_message_from_info(info: &DocblockInfo) -> Option<String> {
    let tag = info.first_tag_by_kind(TagKind::Deprecated)?;
    Some(tag.description.trim().to_owned())
}

/// Check whether a PHPDoc block contains an `@deprecated` tag.
///
/// Convenience wrapper around [`extract_deprecation_message`] for call
/// sites that only need a boolean check.
pub fn has_deprecated_tag(docblock: &str) -> bool {
    extract_deprecation_message(docblock).is_some()
}

/// Like [`has_deprecated_tag`], but operates on a pre-parsed [`DocblockInfo`].
pub fn has_deprecated_tag_from_info(info: &DocblockInfo) -> bool {
    extract_deprecation_message_from_info(info).is_some()
}

/// Extract the PHP version from a `@removed` PHPDoc tag.
///
/// Handles the format `@removed X.Y` where `X.Y` is a PHP version
/// (e.g. `7.0`, `8.0`).
///
/// Returns `None` when no `@removed` tag is present or the version
/// cannot be parsed.
pub fn extract_removed_version(docblock: &str) -> Option<PhpVersion> {
    let info = parse_docblock_for_tags(docblock)?;
    // `@removed` is not a standard PHPDoc tag, so mago-docblock classifies
    // it as `TagKind::Other`.  We match by name instead.
    let tag = info.tags.iter().find(|t| t.name == "removed")?;
    let desc = tag.description.trim();
    if desc.is_empty() {
        return None;
    }
    PhpVersion::from_composer_constraint(desc)
}

/// Extract all `@see` references from a PHPDoc block.
///
/// Returns the raw text after each `@see` tag, which may be:
///   - A symbol reference: `ClassName`, `ClassName::method()`,
///     `ClassName::$property`, `functionName()`
///   - A URL: `https://example.com/docs`
///   - A doc reference: `doc://getting-started/index`
///
/// The full text after `@see` (including any trailing description) is
/// returned as-is, so `@see MyClass::foo() Use this instead` yields
/// `"MyClass::foo() Use this instead"`.
///
/// This is used alongside [`extract_deprecation_message`] to enrich
/// deprecated diagnostics with pointers to replacement APIs.
pub fn extract_see_references(docblock: &str) -> Vec<String> {
    let Some(info) = parse_docblock_for_tags(docblock) else {
        return Vec::new();
    };

    extract_see_references_from_info(&info)
}

/// Like [`extract_see_references`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_see_references_from_info(info: &DocblockInfo) -> Vec<String> {
    info.tags_by_kind(TagKind::See)
        .map(|tag| tag.description.trim().to_owned())
        .filter(|desc| !desc.is_empty())
        .collect()
}

/// Extract the deprecation message from a `@deprecated` PHPDoc tag,
/// enriched with any `@see` references from the same docblock.
///
/// Behaves like [`extract_deprecation_message`] but appends `@see`
/// references (if present) to the returned message.  This gives
/// diagnostic consumers a single string that includes both the
/// deprecation reason and pointers to replacement APIs.
///
/// Format examples:
///   - `@deprecated` alone → `Some("")`
///   - `@deprecated` + `@see NewClass` → `Some("See: NewClass")`
///   - `@deprecated Use new API` + `@see NewClass::method()` →
///     `Some("Use new API (see: NewClass::method())")`
///   - `@deprecated Use new API` + two `@see` tags →
///     `Some("Use new API (see: NewClass::method(), OtherFunc())")`
pub fn extract_deprecation_with_see(docblock: &str) -> Option<String> {
    let info = parse_docblock_for_tags(docblock)?;
    extract_deprecation_with_see_from_info(&info)
}

/// Like [`extract_deprecation_with_see`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_deprecation_with_see_from_info(info: &DocblockInfo) -> Option<String> {
    let base_msg = extract_deprecation_message_from_info(info)?;
    let see_refs = extract_see_references_from_info(info);

    if see_refs.is_empty() {
        return Some(base_msg);
    }

    let see_list = see_refs.join(", ");

    if base_msg.is_empty() {
        Some(format!("See: {}", see_list))
    } else {
        Some(format!("{} (see: {})", base_msg, see_list))
    }
}

/// Extract all `@mixin` tags from a class-level docblock.
///
/// PHPDoc `@mixin` tags declare that the annotated class exposes public
/// members from another class via magic methods (`__call`, `__get`, etc.).
/// The format is:
///
///   - `@mixin ClassName`
///   - `@mixin \Fully\Qualified\ClassName`
///   - `@mixin ClassName<TypeArg1, TypeArg2>`
///
/// Returns a list of `(base_class_name, generic_args)` tuples.  The base
/// class name has its leading `\` and generic parameters stripped.  The
/// `generic_args` vector is empty when the tag has no `<…>` suffix.
pub fn extract_mixin_tags(docblock: &str) -> Vec<(String, Vec<String>)> {
    let Some(info) = parse_docblock_for_tags(docblock) else {
        return Vec::new();
    };

    extract_mixin_tags_from_info(&info)
}

/// Like [`extract_mixin_tags`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_mixin_tags_from_info(info: &DocblockInfo) -> Vec<(String, Vec<String>)> {
    let mut results = Vec::new();

    for tag in info.tags_by_kind(TagKind::Mixin) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Extract the full type token (respects `<…>` nesting so that
        // generics like `Builder<TRelatedModel>` are treated as one unit).
        let (type_token, _remainder) = split_type_token(desc);

        // Parse the type token into a structured PhpType and extract
        // the base class name and optional generic arguments.
        let parsed = PhpType::parse(type_token);
        let (base, generic_args) = match &parsed {
            PhpType::Generic(name, args) => {
                let arg_strs: Vec<String> = args
                    .iter()
                    .map(|a| {
                        let s = a.to_string();
                        strip_fqn_prefix(&s).to_string()
                    })
                    .collect();
                (name.clone(), arg_strs)
            }
            PhpType::Named(name) => (name.clone(), vec![]),
            PhpType::Nullable(inner) => match inner.as_ref() {
                PhpType::Named(name) => (name.clone(), vec![]),
                PhpType::Generic(name, args) => {
                    let arg_strs: Vec<String> = args
                        .iter()
                        .map(|a| {
                            let s = a.to_string();
                            strip_fqn_prefix(&s).to_string()
                        })
                        .collect();
                    (name.clone(), arg_strs)
                }
                _ => continue,
            },
            _ => continue,
        };

        if !base.is_empty() {
            results.push((base, generic_args));
        }
    }

    results
}

/// Extract all `@throws` tags from a method-level docblock.
///
/// PHPDoc `@throws` tags declare which exceptions a method may throw.
/// The format is:
///
///   - `@throws ExceptionType`
///   - `@throws \Fully\Qualified\ExceptionType`
///   - `@throws ExceptionType Some description text`
///
/// Returns a list of cleaned type name strings (leading `\` stripped).
pub fn extract_throws_tags(docblock: &str) -> Vec<String> {
    let Some(info) = parse_docblock_for_tags(docblock) else {
        return Vec::new();
    };

    extract_throws_tags_from_info(&info)
}

/// Like [`extract_throws_tags`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_throws_tags_from_info(info: &DocblockInfo) -> Vec<String> {
    let mut results = Vec::new();

    for tag in info.tags_by_kind(TagKind::Throws) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // The type name is the first whitespace-delimited token.
        let type_name = match desc.split_whitespace().next() {
            Some(name) => name,
            None => continue,
        };

        let cleaned = type_name.trim_start_matches('\\');
        if !cleaned.is_empty() {
            results.push(cleaned.to_string());
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
    let Some(info) = parse_docblock_for_tags(docblock) else {
        return Vec::new();
    };

    extract_type_assertions_from_info(&info)
}

/// Like [`extract_type_assertions`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_type_assertions_from_info(info: &DocblockInfo) -> Vec<TypeAssertion> {
    /// Map a `TagKind` to the corresponding `AssertionKind`.
    const fn assertion_kind_for(kind: TagKind) -> AssertionKind {
        match kind {
            TagKind::PhpstanAssertIfTrue | TagKind::PsalmAssertIfTrue | TagKind::AssertIfTrue => {
                AssertionKind::IfTrue
            }
            TagKind::PhpstanAssertIfFalse
            | TagKind::PsalmAssertIfFalse
            | TagKind::AssertIfFalse => AssertionKind::IfFalse,
            // TagKind::Assert, PhpstanAssert, PsalmAssert, and anything else
            _ => AssertionKind::Always,
        }
    }

    const ASSERT_KINDS: &[TagKind] = &[
        TagKind::PhpstanAssertIfTrue,
        TagKind::PhpstanAssertIfFalse,
        TagKind::PhpstanAssert,
        TagKind::PsalmAssertIfTrue,
        TagKind::PsalmAssertIfFalse,
        TagKind::PsalmAssert,
        TagKind::AssertIfTrue,
        TagKind::AssertIfFalse,
        TagKind::Assert,
    ];

    let mut results = Vec::new();

    for tag in info.tags_by_kinds(ASSERT_KINDS) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Check for negation: `!Type $param`
        let (negated, rest) = if let Some(r) = desc.strip_prefix('!') {
            (true, r.trim_start())
        } else {
            (false, desc)
        };

        // Next token is the type, then the parameter name.
        let mut tokens = rest.split_whitespace();
        let type_str = match tokens.next() {
            Some(t) => t,
            None => continue,
        };
        let param_str = match tokens.next() {
            Some(p) if p.starts_with('$') => p,
            _ => continue,
        };

        results.push(TypeAssertion {
            kind: assertion_kind_for(tag.kind),
            param_name: param_str.to_string(),
            asserted_type: PhpType::parse(type_str.trim_end_matches(['.', ','])),
            negated,
        });
    }

    results
}

/// Extract the type from a `@var` PHPDoc tag.
///
/// Used for property type annotations like:
///   - `/** @var Session */`
///   - `/** @var \App\Models\User */`
pub fn extract_var_type(docblock: &str) -> Option<String> {
    extract_type_via_mago(docblock, &[TagKind::PhpstanVar, TagKind::Var])
}

/// Like [`extract_var_type`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_var_type_from_info(info: &DocblockInfo) -> Option<String> {
    extract_type_via_mago_from_info(info, &[TagKind::PhpstanVar, TagKind::Var])
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
    extract_var_type_with_name_from_info(&parse_docblock_for_tags(docblock)?)
}

/// Like [`extract_var_type_with_name`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_var_type_with_name_from_info(
    info: &DocblockInfo,
) -> Option<(String, Option<String>)> {
    for tag in info.tags_by_kinds(&[TagKind::PhpstanVar, TagKind::Var]) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Extract the type token, respecting `<…>` nesting so that
        // generics like `Collection<int, User>` are treated as one unit.
        let (type_str, remainder) = split_type_token(desc);
        let cleaned_type = type_str.trim_end_matches(['.', ',']).to_string();
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
        // re-entered a block close to that level, we are inside a
        // sibling scope (e.g. a different method in the same class).
        // From that point on every annotation belongs to a foreign
        // scope.  The threshold is `min_depth + 1` rather than `>= 0`
        // because the cursor may be inside a nested block (foreach,
        // if, etc.) whose extra depth prevents brace_depth from ever
        // reaching 0 when traversing sibling classes.
        if min_depth < 0 && brace_depth > min_depth {
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
    extract_param_raw_type_from_info(&parse_docblock_for_tags(docblock)?, var_name)
}

/// Like [`extract_param_raw_type`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_param_raw_type_from_info(info: &DocblockInfo, var_name: &str) -> Option<String> {
    for tag in info.tags_by_kinds(&[TagKind::PhpstanParam, TagKind::Param]) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Extract the full type token (respects `<…>` nesting).
        let (type_token, remainder) = split_type_token(desc);

        // The next token should be the parameter name.
        // Handle `...$name` (variadic) by stripping the leading `...`.
        if let Some(name) = remainder.split_whitespace().next() {
            let name = name.strip_prefix("...").unwrap_or(name);
            if name == var_name {
                return Some(type_token.to_string());
            }
        }
    }

    None
}

/// Extract all `@param` tags from a docblock as `(name, type)` pairs.
///
/// Returns a list where each entry is `(param_name, type_string)`.
/// The `param_name` includes the `$` prefix.  Variadic `...$name`
/// parameters are returned with the `$name` only (the `...` is stripped).
///
/// This is used to discover extra `@param` tags that document parameters
/// not present in the native function signature (e.g. parameters accessed
/// via `func_get_args()`).
pub fn extract_all_param_tags(docblock: &str) -> Vec<(String, String)> {
    let Some(info) = parse_docblock_for_tags(docblock) else {
        return Vec::new();
    };

    extract_all_param_tags_from_info(&info)
}

/// Like [`extract_all_param_tags`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_all_param_tags_from_info(info: &DocblockInfo) -> Vec<(String, String)> {
    let mut results = Vec::new();

    // Only match `@param` and `@phpstan-param`, not compound tags like
    // `@param-closure-this` (those have their own TagKind).
    for tag in info.tags_by_kinds(&[TagKind::PhpstanParam, TagKind::Param]) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Extract the full type token (respects `<…>` nesting).
        let (type_token, remainder) = split_type_token(desc);

        // The next token should be the parameter name.
        // Handle `...$name` (variadic) by stripping the leading `...`.
        if let Some(name) = remainder.split_whitespace().next() {
            let name = name.strip_prefix("...").unwrap_or(name);
            if name.starts_with('$') {
                results.push((name.to_string(), type_token.to_string()));
            }
        }
    }

    results
}

/// Extract `@param` type strings in order of appearance, including
/// tags that omit the parameter name.
///
/// Returns a list of `(Option<param_name>, type_string)` pairs in
/// docblock order.  When a `@param` tag has no `$name` token (common
/// in phpstorm-stubs, e.g. `@param callable(TValue, TKey): bool`),
/// the first element is `None`.  Callers can match these entries to
/// native parameters by position.
///
/// This is used as a positional fallback when name-based matching
/// via [`extract_param_raw_type_from_info`] fails to find a docblock
/// type for a parameter.
pub fn extract_param_types_positional_from_info(
    info: &DocblockInfo,
) -> Vec<(Option<String>, String)> {
    let mut results = Vec::new();

    for tag in info.tags_by_kinds(&[TagKind::PhpstanParam, TagKind::Param]) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        let (type_token, remainder) = split_type_token(desc);

        let param_name = remainder.split_whitespace().next().and_then(|name| {
            let name = name.strip_prefix("...").unwrap_or(name);
            if name.starts_with('$') {
                Some(name.to_string())
            } else {
                None
            }
        });

        results.push((param_name, type_token.to_string()));
    }

    results
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
    let Some(info) = parse_docblock_for_tags(docblock) else {
        return Vec::new();
    };

    extract_param_closure_this_from_info(&info)
}

/// Like [`extract_param_closure_this`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_param_closure_this_from_info(info: &DocblockInfo) -> Vec<(String, String)> {
    let mut results = Vec::new();

    for tag in info.tags_by_kind(TagKind::ParamClosureThis) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Extract the type token (respects `<…>` nesting).
        let (type_token, remainder) = split_type_token(desc);
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
    extract_param_description_from_info(&parse_docblock_for_tags(docblock)?, var_name)
}

/// Like [`extract_param_description`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_param_description_from_info(info: &DocblockInfo, var_name: &str) -> Option<String> {
    for tag in info.tags_by_kinds(&[TagKind::PhpstanParam, TagKind::Param]) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Skip the type token.
        let (_type_token, remainder) = split_type_token(desc);
        let remainder = remainder.trim_start();

        // Check if the next token is our parameter name.
        // Handle `...$name` (variadic) by stripping the leading `...`.
        let name_token = remainder.split_whitespace().next().unwrap_or("");
        let name_stripped = name_token.strip_prefix("...").unwrap_or(name_token);
        if name_stripped != var_name {
            continue;
        }

        // Skip past the parameter name to get the description.
        let after_name = remainder.get(name_token.len()..).unwrap_or("").trim_start();

        // mago-docblock joins multi-line tag descriptions with `\n`.
        // The old code joined continuation lines with spaces, so
        // normalise newlines to spaces to preserve existing behaviour.
        let normalised = collapse_newlines(after_name);
        let cleaned = strip_html_tags(&normalised);
        let desc = cleaned.trim().to_string();
        if desc.is_empty() {
            return None;
        }
        return Some(desc);
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
    extract_return_description_from_info(&parse_docblock_for_tags(docblock)?)
}

/// Like [`extract_return_description`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_return_description_from_info(info: &DocblockInfo) -> Option<String> {
    for tag in info.tags_by_kinds(&[TagKind::PhpstanReturn, TagKind::Return]) {
        let desc = tag.description.trim();
        if desc.is_empty() {
            continue;
        }

        // Skip PHPStan conditional return types.
        if desc.starts_with('(') {
            return None;
        }

        // Skip the type token.
        let (_type_token, remainder) = split_type_token(desc);
        let remainder = remainder.trim_start();

        // mago-docblock joins multi-line tag descriptions with `\n`.
        // The old code joined continuation lines with spaces, so
        // normalise newlines to spaces to preserve existing behaviour.
        let normalised = collapse_newlines(remainder);
        let cleaned = strip_html_tags(&normalised);
        let result = cleaned.trim().to_string();
        if result.is_empty() {
            return None;
        }
        return Some(result);
    }

    None
}

/// Extract the URL from a `@link` tag in a docblock.
///
/// Example:
///   `@link https://php.net/manual/en/function.array-map.php`
///   → `Some("https://php.net/manual/en/function.array-map.php")`
pub fn extract_link_urls(docblock: &str) -> Vec<String> {
    let Some(info) = parse_docblock_for_tags(docblock) else {
        return Vec::new();
    };

    extract_link_urls_from_info(&info)
}

/// Like [`extract_link_urls`], but operates on a pre-parsed [`DocblockInfo`].
pub fn extract_link_urls_from_info(info: &DocblockInfo) -> Vec<String> {
    let mut urls = Vec::new();

    for tag in info.tags_by_kind(TagKind::Link) {
        let desc = tag.description.trim();
        // Take the first whitespace-delimited token as the URL.
        if let Some(url) = desc.split_whitespace().next()
            && !url.is_empty()
        {
            urls.push(url.to_string());
        }
    }

    urls
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
        // re-entered a block close to that level, we are inside a
        // sibling scope (e.g. a different method in the same class).
        // From that point on every annotation belongs to a foreign
        // scope.
        //
        // The threshold is `min_depth + 1` rather than `>= 0` because
        // the cursor may be inside a nested block (foreach, if, etc.)
        // that adds extra depth.  When starting inside a foreach in a
        // class method, min_depth reaches -3 (foreach { + method { +
        // class {), so a sibling method body at depth -1 would never
        // reach 0.  Using `min_depth + 1` catches the first rise back
        // toward our exit point.
        if min_depth < 0 && brace_depth > min_depth {
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
    let after_mods = crate::util::strip_trailing_modifiers(trimmed);

    if !after_mods.ends_with("*/") {
        return None;
    }

    let open_pos = after_mods.rfind("/**")?;
    let docblock = &after_mods[open_pos..];

    extract_return_type(docblock)
}

// ─── Type Override Logic ────────────────────────────────────────────────────

/// Decide whether a docblock type should override a native type hint.
///
/// Returns `true` when the docblock type is likely to carry more
/// information than the native hint (e.g. `Collection<int, User>` vs
/// bare `object`), and `false` when overriding would lose precision
/// (e.g. both are scalars).
pub fn should_override_type(docblock_type: &str, native_type: &str) -> bool {
    let doc_parsed = PhpType::parse(docblock_type);
    let native_parsed = PhpType::parse(native_type);
    should_override_type_typed(&doc_parsed, &native_parsed)
}

/// Typed variant of [`should_override_type`] that accepts pre-parsed
/// [`PhpType`] values, avoiding redundant parse round-trips when callers
/// already hold parsed types.
pub fn should_override_type_typed(docblock_type: &PhpType, native_type: &PhpType) -> bool {
    // If the docblock type is semantically equivalent to the native type
    // (handles `?X` ↔ `X|null`, reordered unions, FQN vs short names),
    // there is no value in overriding — the docblock doesn't carry any
    // extra information.
    if docblock_type.equivalent(native_type) {
        return false;
    }

    // Unwrap nullable wrappers for further analysis.  `?Foo` → `Foo`,
    // `Foo|null` → `Foo`.  For non-nullable types, use as-is.
    let doc_inner = unwrap_nullable(docblock_type);
    let native_inner = unwrap_nullable(native_type);

    // If the docblock type is a bare, unparameterised primitive scalar
    // (`int`, `string`, `bool`, etc.), there's no value in overriding.
    // We intentionally exclude:
    //  - PHPDoc pseudo-types (`non-empty-string`, `class-string`,
    //    `positive-int`) — these are valid refinements.
    //  - Parameterised types (`array<int>`, `int<0, max>`) — these
    //    carry type information the native hint doesn't have.
    //  - Shapes, callables, slices — these also carry extra info.
    if is_bare_primitive_scalar(doc_inner) {
        return false;
    }

    // Produce a lowercased base name for the native type's inner part
    // (stripping nullable).  This is used for broad-type and refinement
    // checks below.
    let native_inner_str = native_inner.to_string();
    let native_lower = native_inner_str.to_ascii_lowercase();

    // `array`, `iterable`, `callable`, and `Closure` are broad types
    // that docblocks commonly refine (e.g. `array` → `list<User>`,
    // `iterable` → `Collection<int, Order>`,
    // `callable` → `callable(Task): void`).
    if matches!(
        native_lower.as_str(),
        "array" | "iterable" | "callable" | "closure" | "\\closure"
    ) {
        return true;
    }

    // If the native type is a union or intersection, check each component.
    // If ALL parts are scalar, the docblock can't override.
    // If ANY part is non-scalar, it's plausible to refine.
    match native_inner {
        PhpType::Union(members) | PhpType::Intersection(members) => {
            return members.iter().any(|m| !m.is_scalar());
        }
        _ => {}
    }

    // If the native type is a narrow scalar (not a broad container
    // handled above), only allow override when the docblock type is a
    // *compatible refinement*.  For example `string` → `class-string<Foo>`
    // is valid, but `string` → `array<int>` is not.
    if native_inner.is_scalar() {
        return is_compatible_refinement_typed(doc_inner, &native_lower);
    }

    // If the docblock type carries generic parameters or shape braces,
    // it is refining the class with extra type info — allow it.
    if has_parameterisation(doc_inner) {
        return true;
    }

    // PHPDoc pseudo-types like `class-string`, `non-empty-string`,
    // `positive-int`, `literal-string`, etc. refine their native
    // scalar counterparts.  These contain hyphens which never appear
    // in native PHP types.
    if extract_base_name_lower(doc_inner).contains('-') {
        return true;
    }

    // Native type is a non-scalar class — docblock can always refine.
    true
}

/// Unwrap nullable wrappers from a `PhpType`.
///
/// `Nullable(X)` → `X`.  For non-nullable types, returns the type
/// unchanged.  Note: `Union([X, Named("null")])` is NOT unwrapped
/// here — the caller should use `non_null_type()` if needed.
fn unwrap_nullable(ty: &PhpType) -> &PhpType {
    match ty {
        PhpType::Nullable(inner) => inner.as_ref(),
        _ => ty,
    }
}

/// Check whether a `PhpType` has generic parameters or shape braces.
fn has_parameterisation(ty: &PhpType) -> bool {
    matches!(
        ty,
        PhpType::Generic(_, _) | PhpType::ArrayShape(_) | PhpType::ObjectShape(_)
    )
}

/// Check whether a `PhpType` is a bare, unparameterised primitive scalar.
///
/// Returns `true` for simple type names like `int`, `string`, `bool`,
/// `void`, `null`, `array`, `callable`, `iterable`, `resource` (and
/// aliases like `integer`, `double`, `boolean`).
///
/// Returns `false` for:
/// - PHPDoc pseudo-types (`non-empty-string`, `class-string`, `positive-int`)
/// - Parameterised types (`array<int>`, `int<0, max>`, `list<User>`)
/// - Shapes, callables with signatures, slices (`Foo[]`)
/// - Class names, unions, intersections, etc.
fn is_bare_primitive_scalar(ty: &PhpType) -> bool {
    matches!(ty, PhpType::Named(s) if is_bare_primitive_name(s))
}

/// Whether a type name is one of the basic PHP primitive / built-in names.
///
/// This is intentionally narrower than `PhpType::is_scalar()` — it
/// excludes `mixed`, `object`, `self`, `static`, `parent`, `$this`,
/// and all PHPDoc pseudo-types like `class-string`, `non-empty-string`.
fn is_bare_primitive_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "int"
            | "integer"
            | "float"
            | "double"
            | "string"
            | "bool"
            | "boolean"
            | "void"
            | "never"
            | "null"
            | "false"
            | "true"
            | "array"
            | "callable"
            | "iterable"
            | "resource"
    )
}

/// Check whether a docblock type is a compatible refinement of a native
/// type.  Both parameters should be stripped of nullable wrappers before
/// calling.  `native_lower` must already be lowercased.
///
/// A refinement is compatible when the docblock's base type narrows the
/// native type without changing its fundamental kind.  For example:
/// - `string` → `class-string<Foo>` (compatible: refines string)
/// - `string` → `non-empty-string` (compatible: refines string)
/// - `int` → `positive-int` (compatible: refines int)
/// - `array` → `list<User>` (compatible: refines array)
/// - `object` → `callable-object` (compatible: refines object)
/// - `string` → `array<int>` (incompatible: completely different type)
/// - `int` → `Collection<User>` (incompatible: completely different type)
///
/// This is the single source of truth for refinement compatibility and
/// is used by both `should_override_type` and the update-docblock
/// contradiction checker.
///
/// Accepts a pre-parsed [`PhpType`] to avoid a parse-stringify-reparse
/// round-trip.  Extracts the outermost type name from the docblock type,
/// stripping generic parameters, shape braces, and callable signatures.
/// Unlike `base_name()` this includes scalar names (`array`, `int`, ...)
/// which are needed for the refinement checks.
pub(crate) fn is_compatible_refinement_typed(doc_type: &PhpType, native_lower: &str) -> bool {
    let doc_base = extract_base_name_lower(doc_type);

    match native_lower {
        // `string` is refined by `class-string`, `non-empty-string`,
        // `literal-string`, `numeric-string`, `callable-string`,
        // `lowercase-string`, `truthy-string` etc.
        "string" => doc_base.contains("string"),
        // `int` / `integer` is refined by `positive-int`, `negative-int`,
        // `non-negative-int`, `non-positive-int`, `int-mask`, `int-mask-of`,
        // `int` (with range syntax like `int<0, max>`).
        "int" | "integer" => doc_base.contains("int"),
        // `float` / `double` can be refined by `non-negative-float` etc.
        "float" | "double" => doc_base.contains("float") || doc_base.contains("double"),
        // `bool` / `boolean` can be refined by `true` or `false` (already
        // handled as scalars earlier, but include for completeness).
        "bool" | "boolean" => {
            doc_base == "true" || doc_base == "false" || doc_base.contains("bool")
        }
        // `array` is refined by `list`, `non-empty-array`, `non-empty-list`,
        // `associative-array`, `callable-array`, `array<…>`, `array{…}`.
        "array" => {
            doc_base.contains("array") || doc_base.contains("list") || doc_base == "iterable"
        }
        // `iterable` is refined by `array`, `list`, or any Collection-like.
        // Since any class implementing Traversable/Iterator could be a valid
        // refinement, allow all non-scalar docblock types.
        "iterable" => true,
        // `callable` / `Closure` are broad — any callable signature refines them.
        "callable" => true,
        "closure" => true,
        // `object` is refined by any class name, `callable-object`,
        // or an object shape like `object{name: string, age: int}`.
        "object" => !doc_type.is_scalar() || matches!(doc_type, PhpType::ObjectShape(_)),
        // `mixed` can be refined by anything.
        "mixed" => true,
        // `resource` is refined by `closed-resource`, `open-resource`.
        "resource" => doc_base.contains("resource"),
        // `self`, `static`, `parent`, `$this` — these are late-bound
        // type references that any concrete class name refines.
        "self" | "static" | "parent" | "$this" => !doc_type.is_scalar(),
        // `void`, `never`, `null`, `true`, `false` — these are so narrow
        // that docblock refinement is never meaningful.
        "void" | "never" | "null" | "true" | "false" => false,
        // For any other type, be conservative — don't override.
        _ => false,
    }
}

/// Extract the outermost type name from a `PhpType` as a lowercased string.
///
/// Strips generic parameters, shape braces, callable signatures, and
/// nullable wrappers.  Returns the base identifier lowercased (e.g.
/// `Generic("Collection", _)` → `"collection"`, `Named("int")` → `"int"`).
///
/// For types that don't have a simple base name (e.g. unions, literals),
/// falls back to the full `Display` output lowercased.
fn extract_base_name_lower(ty: &PhpType) -> String {
    match ty {
        PhpType::Named(name) => name.to_ascii_lowercase(),
        PhpType::Generic(name, _) => name.to_ascii_lowercase(),
        PhpType::Nullable(inner) => extract_base_name_lower(inner),
        _ => ty.to_string().to_ascii_lowercase(),
    }
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
    get_docblock_text_with_offset(trivia, content, node).map(|(text, _)| text)
}

/// Locate the docblock for an AST node and return it as a parsed
/// [`DocblockInfo`].
///
/// This combines [`get_docblock_text_for_node`] and
/// [`parse_docblock_for_tags`] into a single call, eliminating
/// redundant re-parsing when multiple tags need to be extracted from
/// the same docblock.
pub fn get_docblock_info_for_node(
    trivia: &[Trivia<'_>],
    content: &str,
    node: &impl HasSpan,
) -> Option<DocblockInfo> {
    let text = get_docblock_text_for_node(trivia, content, node)?;
    parse_docblock_for_tags(text)
}

// ─── Effective Type Resolution ──────────────────────────────────────────────

/// Pick the best available type between a native type hint and a docblock
/// annotation, returning a parsed [`PhpType`].
///
/// When both are present, the docblock type is used only if
/// [`should_override_type`] approves (i.e. the native hint is broad enough
/// to refine).  Malformed docblock types with unclosed brackets are
/// partially recovered or discarded.
///
/// Callers that need a string representation can use
/// [`PhpType::to_string()`] on the result.
pub fn resolve_effective_type(
    native_type: Option<&str>,
    docblock_type: Option<&str>,
) -> Option<PhpType> {
    // When the docblock type has unclosed brackets (e.g. a multi-line
    // `@return` that couldn't be fully joined), treat it as broken and
    // attempt partial recovery.  If recovery yields nothing useful, fall
    // back to the native type so that resolution is never blocked by a
    // malformed PHPDoc annotation.
    let sanitised_doc = docblock_type.and_then(|doc| {
        if crate::util::has_unclosed_delimiters(doc) {
            let base = recover_base_type(doc);
            if base.is_empty() {
                None
            } else {
                Some(PhpType::parse(base))
            }
        } else {
            Some(PhpType::parse(doc))
        }
    });

    let native_parsed = native_type.map(PhpType::parse);
    resolve_effective_type_typed(native_parsed.as_ref(), sanitised_doc.as_ref())
}

/// Parse a raw docblock type string into a [`PhpType`], applying
/// unclosed-bracket recovery when necessary.
///
/// Raw docblock strings from `extract_return_type`, `extract_var_type`,
/// `extract_param_raw_type`, etc. may contain malformed type expressions
/// (e.g. `"static<"`, `"Collection<int"`) when multi-line annotations
/// couldn't be fully joined.  This helper recovers the base type in such
/// cases, matching the sanitisation logic in [`resolve_effective_type`].
///
/// Returns `None` when the string is completely unrecoverable (e.g.
/// `"<garbage"` with no base type).
pub fn sanitise_and_parse_docblock_type(raw: &str) -> Option<PhpType> {
    if crate::util::has_unclosed_delimiters(raw) {
        let base = recover_base_type(raw);
        if base.is_empty() {
            None
        } else {
            Some(PhpType::parse(base))
        }
    } else {
        Some(PhpType::parse(raw))
    }
}

/// Typed variant of [`resolve_effective_type`] that accepts pre-parsed
/// [`PhpType`] values, avoiding redundant parse-stringify round-trips when
/// callers already hold parsed types.
///
/// Unlike the string-based version, this does not perform unclosed-bracket
/// recovery since pre-parsed types are already well-formed.
pub fn resolve_effective_type_typed(
    native_type: Option<&PhpType>,
    docblock_type: Option<&PhpType>,
) -> Option<PhpType> {
    match (native_type, docblock_type) {
        // Docblock provided, no native hint → use docblock.
        (None, Some(doc)) => Some(doc.clone()),
        // Both present → override only if compatible.
        (Some(native), Some(doc)) => {
            if should_override_type_typed(doc, native) {
                Some(doc.clone())
            } else {
                Some(native.clone())
            }
        }
        // Native only → keep it.
        (Some(native), None) => Some(native.clone()),
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
/// Shared implementation for tag-type extraction via the mago-docblock parser.
///
/// Searches the parsed docblock for the first tag matching any of the given
/// `kinds` (tried in order, so vendor-prefixed kinds like `PhpstanReturn`
/// should come before the generic `Return` to give them priority).
///
/// The tag's `description` field already contains the joined, multi-line
/// content after the tag name.  We extract the type portion using
/// `split_type_token`, stripping trailing punctuation.
///
/// Skips PHPStan conditional return types (descriptions starting with `(`).
fn extract_type_via_mago(docblock: &str, kinds: &[TagKind]) -> Option<String> {
    extract_type_via_mago_from_info(&parse_docblock_for_tags(docblock)?, kinds)
}

/// Like [`extract_type_via_mago`], but operates on a pre-parsed [`DocblockInfo`].
fn extract_type_via_mago_from_info(info: &DocblockInfo, kinds: &[TagKind]) -> Option<String> {
    // Try each kind in priority order; return on the first match.
    for &kind in kinds {
        for tag in info.tags_by_kind(kind) {
            let desc = tag.description.trim();
            if desc.is_empty() {
                continue;
            }

            // PHPStan conditional return types start with `(` — skip them
            // here; they are handled by `extract_conditional_return_type`.
            if desc.starts_with('(') {
                return None;
            }

            // mago-docblock joins multi-line tag descriptions with `\n`.
            // Normalise newlines (and surrounding whitespace from
            // indentation) into a single space so that `split_type_token`
            // sees the same single-line input the old line-by-line scanner
            // produced after trimming and joining continuation lines.
            let normalised = collapse_newlines(desc);
            let (type_str, _remainder) = split_type_token(&normalised);
            if type_str.is_empty() {
                continue;
            }

            return Some(type_str.trim_end_matches(['.', ',']).to_string());
        }
    }

    None
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

    // ── extract_see_references ──────────────────────────────────────

    #[test]
    fn see_references_empty_when_no_see_tag() {
        let doc = "/** @deprecated Use foo() */";
        assert!(extract_see_references(doc).is_empty());
    }

    #[test]
    fn see_references_single_class() {
        let doc = "/**\n * @deprecated\n * @see NewClass\n */";
        assert_eq!(extract_see_references(doc), vec!["NewClass"]);
    }

    #[test]
    fn see_references_method() {
        let doc = "/**\n * @deprecated\n * @see MyClass::newMethod()\n */";
        assert_eq!(extract_see_references(doc), vec!["MyClass::newMethod()"]);
    }

    #[test]
    fn see_references_property() {
        let doc = "/**\n * @deprecated\n * @see MyClass::$items\n */";
        assert_eq!(extract_see_references(doc), vec!["MyClass::$items"]);
    }

    #[test]
    fn see_references_function() {
        let doc = "/**\n * @deprecated\n * @see number_of()\n */";
        assert_eq!(extract_see_references(doc), vec!["number_of()"]);
    }

    #[test]
    fn see_references_url() {
        let doc = "/**\n * @see https://example.com/docs\n */";
        assert_eq!(
            extract_see_references(doc),
            vec!["https://example.com/docs"]
        );
    }

    #[test]
    fn see_references_with_description() {
        let doc = "/**\n * @see MyClass::setItems() To set the items.\n */";
        assert_eq!(
            extract_see_references(doc),
            vec!["MyClass::setItems() To set the items."]
        );
    }

    #[test]
    fn see_references_multiple() {
        let doc = "/**\n * @deprecated\n * @see number_of() Alias.\n * @see MyClass::$items For the property.\n * @see MyClass::setItems() To set items.\n */";
        let refs = extract_see_references(doc);
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0], "number_of() Alias.");
        assert_eq!(refs[1], "MyClass::$items For the property.");
        assert_eq!(refs[2], "MyClass::setItems() To set items.");
    }

    #[test]
    fn see_references_with_tab_separator() {
        let doc = "/**\n * @see\tMyClass\n */";
        assert_eq!(extract_see_references(doc), vec!["MyClass"]);
    }

    #[test]
    fn see_references_bare_see_tag_ignored() {
        // A bare @see with no reference text should not produce an entry.
        let doc = "/**\n * @see\n */";
        assert!(extract_see_references(doc).is_empty());
    }

    // ── extract_deprecation_with_see ────────────────────────────────

    #[test]
    fn deprecation_with_see_no_deprecated_tag() {
        let doc = "/**\n * @see NewClass\n * @return string\n */";
        assert_eq!(extract_deprecation_with_see(doc), None);
    }

    #[test]
    fn deprecation_with_see_no_see_tags() {
        let doc = "/** @deprecated Use foo() instead */";
        assert_eq!(
            extract_deprecation_with_see(doc),
            Some("Use foo() instead".to_string())
        );
    }

    #[test]
    fn deprecation_with_see_bare_deprecated_plus_see() {
        let doc = "/**\n * @deprecated\n * @see NewClass\n */";
        assert_eq!(
            extract_deprecation_with_see(doc),
            Some("See: NewClass".to_string())
        );
    }

    #[test]
    fn deprecation_with_see_message_plus_see() {
        let doc = "/**\n * @deprecated Use the new API.\n * @see NewClass::newMethod()\n */";
        assert_eq!(
            extract_deprecation_with_see(doc),
            Some("Use the new API. (see: NewClass::newMethod())".to_string())
        );
    }

    #[test]
    fn deprecation_with_see_message_plus_multiple_see() {
        let doc =
            "/**\n * @deprecated Old approach.\n * @see NewClass::foo()\n * @see OtherFunc()\n */";
        assert_eq!(
            extract_deprecation_with_see(doc),
            Some("Old approach. (see: NewClass::foo(), OtherFunc())".to_string())
        );
    }

    #[test]
    fn deprecation_with_see_bare_deprecated_plus_multiple_see() {
        let doc =
            "/**\n * @deprecated\n * @see NewClass\n * @see https://example.com/migration\n */";
        assert_eq!(
            extract_deprecation_with_see(doc),
            Some("See: NewClass, https://example.com/migration".to_string())
        );
    }

    #[test]
    fn deprecation_with_see_url_reference() {
        let doc =
            "/**\n * @deprecated\n * @see https://example.com/my/bar Documentation of Foo.\n */";
        assert_eq!(
            extract_deprecation_with_see(doc),
            Some("See: https://example.com/my/bar Documentation of Foo.".to_string())
        );
    }

    #[test]
    fn deprecation_with_see_doc_protocol_reference() {
        let doc = "/**\n * @deprecated\n * @see doc://getting-started/index Getting started.\n */";
        assert_eq!(
            extract_deprecation_with_see(doc),
            Some("See: doc://getting-started/index Getting started.".to_string())
        );
    }

    #[test]
    fn deprecation_with_see_realistic_phpdoc() {
        let doc = r#"/**
 * Count the items.
 *
 * @see number_of()                 Alias.
 * @see MyClass::$items             For the property whose items are counted.
 * @see MyClass::setItems()         To set the items for this collection.
 * @see https://example.com/my/bar  Documentation of Foo.
 *
 * @deprecated Use number_of() instead.
 * @return int Indicates the number of items.
 */"#;
        let result = extract_deprecation_with_see(doc).unwrap();
        assert!(result.starts_with("Use number_of() instead."));
        assert!(result.contains("number_of()"));
        assert!(result.contains("MyClass::$items"));
        assert!(result.contains("MyClass::setItems()"));
        assert!(result.contains("https://example.com/my/bar"));
    }

    // ── extract_removed_version ─────────────────────────────────────

    #[test]
    fn removed_tag_seven_zero() {
        let doc = "/** @removed 7.0 */";
        let version = extract_removed_version(doc).unwrap();
        assert_eq!(version.major, 7);
        assert_eq!(version.minor, 0);
    }

    #[test]
    fn removed_tag_eight_zero() {
        let doc = "/**\n * @removed 8.0\n */";
        let version = extract_removed_version(doc).unwrap();
        assert_eq!(version.major, 8);
        assert_eq!(version.minor, 0);
    }

    #[test]
    fn no_removed_tag() {
        let doc = "/** @return string */";
        assert_eq!(extract_removed_version(doc), None);
    }

    #[test]
    fn other_tags_but_no_removed() {
        let doc = "/**\n * @deprecated Use foo() instead.\n * @see NewClass\n * @return int\n */";
        assert_eq!(extract_removed_version(doc), None);
    }

    // ── find_var_raw_type_in_source — scope isolation ───────────────

    #[test]
    fn var_docblock_does_not_leak_across_sibling_methods() {
        // A `@var` in one class method must not be visible in another.
        let src = concat!(
            "<?php\n",
            "class A {\n",
            "    public function first(): void {\n",
            "        /** @var object{title: string} $item */\n",
            "        $item = foo();\n",
            "    }\n",
            "}\n",
            "class B {\n",
            "    public function second(): void {\n",
            "        $item->\n", // cursor here
            "    }\n",
            "}\n",
        );
        let cursor = src.find("$item->").unwrap();
        let result = find_var_raw_type_in_source(src, cursor, "$item");
        assert_eq!(
            result, None,
            "@var from A::first() must not leak into B::second()"
        );
    }

    #[test]
    fn var_docblock_does_not_leak_across_sibling_methods_same_class() {
        let src = concat!(
            "<?php\n",
            "class Demo {\n",
            "    public function first(): void {\n",
            "        /** @var Pen $item */\n",
            "        $item = foo();\n",
            "    }\n",
            "    public function second(): void {\n",
            "        $item->\n",
            "    }\n",
            "}\n",
        );
        let cursor = src.find("$item->").unwrap();
        let result = find_var_raw_type_in_source(src, cursor, "$item");
        assert_eq!(
            result, None,
            "@var from first() must not leak into second()"
        );
    }

    #[test]
    fn var_docblock_does_not_leak_when_cursor_inside_nested_block() {
        // The original bug: when the cursor is inside a foreach (or if,
        // while, etc.), the extra nesting depth prevented the sibling
        // scope detection from firing, allowing @var from a method in a
        // completely different class to leak through.
        let src = concat!(
            "<?php\n",
            "class ObjectShapeDemo {\n",
            "    public function demo(): void {\n",
            "        /** @var object{title: string, score: float} $item */\n",
            "        $item = getUnknownValue();\n",
            "    }\n",
            "}\n",
            "class Other {\n",
            "    public function demo(): void {\n",
            "        foreach ($things as $item) {\n",
            "            $item->\n",
            "        }\n",
            "    }\n",
            "}\n",
        );
        let cursor = src.find("$item->").unwrap();
        let result = find_var_raw_type_in_source(src, cursor, "$item");
        assert_eq!(
            result, None,
            "@var from ObjectShapeDemo must not leak into Other when cursor is inside foreach"
        );
    }

    #[test]
    fn var_docblock_found_in_own_scope() {
        let src = concat!(
            "<?php\n",
            "class Demo {\n",
            "    public function demo(): void {\n",
            "        /** @var Pen $item */\n",
            "        $item = foo();\n",
            "        $item->\n",
            "    }\n",
            "}\n",
        );
        let cursor = src.find("$item->").unwrap();
        let result = find_var_raw_type_in_source(src, cursor, "$item");
        assert_eq!(result, Some("Pen".to_string()));
    }

    #[test]
    fn var_docblock_found_inside_nested_block_in_own_scope() {
        let src = concat!(
            "<?php\n",
            "class Demo {\n",
            "    public function demo(): void {\n",
            "        /** @var Pen $item */\n",
            "        $item = foo();\n",
            "        if (true) {\n",
            "            $item->\n",
            "        }\n",
            "    }\n",
            "}\n",
        );
        let cursor = src.find("$item->").unwrap();
        let result = find_var_raw_type_in_source(src, cursor, "$item");
        assert_eq!(result, Some("Pen".to_string()));
    }

    // ── find_iterable_raw_type_in_source — scope isolation ──────────

    #[test]
    fn iterable_docblock_does_not_leak_across_sibling_classes_nested() {
        // Same bug scenario for find_iterable_raw_type_in_source:
        // @var in a sibling class method leaks when cursor is nested.
        let src = concat!(
            "<?php\n",
            "class A {\n",
            "    public function demo(): void {\n",
            "        /** @var object{title: string} $item */\n",
            "        $item = foo();\n",
            "    }\n",
            "}\n",
            "class B {\n",
            "    public function demo(): void {\n",
            "        foreach ($things as $x) {\n",
            "            $item->\n",
            "        }\n",
            "    }\n",
            "}\n",
        );
        let cursor = src.find("$item->").unwrap();
        let result = find_iterable_raw_type_in_source(src, cursor, "$item");
        assert_eq!(
            result, None,
            "@var from A::demo() must not leak into B::demo() foreach body"
        );
    }

    #[test]
    fn iterable_param_found_in_own_method_from_nested_block() {
        // @param in the enclosing method's docblock must still be found
        // even when the cursor is inside a nested block.
        let src = concat!(
            "<?php\n",
            "class Demo {\n",
            "    /**\n",
            "     * @param list<Pen> $items\n",
            "     */\n",
            "    public function demo(array $items): void {\n",
            "        foreach ($items as $x) {\n",
            "            // cursor\n",
            "        }\n",
            "    }\n",
            "}\n",
        );
        let cursor = src.find("// cursor").unwrap();
        let result = find_iterable_raw_type_in_source(src, cursor, "$items");
        assert_eq!(result, Some("list<Pen>".to_string()));
    }

    #[test]
    fn iterable_var_found_in_own_scope_nested() {
        let src = concat!(
            "<?php\n",
            "class Demo {\n",
            "    public function demo(): void {\n",
            "        /** @var list<Pen> $items */\n",
            "        $items = foo();\n",
            "        foreach ($items as $x) {\n",
            "            // cursor\n",
            "        }\n",
            "    }\n",
            "}\n",
        );
        let cursor = src.find("// cursor").unwrap();
        let result = find_iterable_raw_type_in_source(src, cursor, "$items");
        assert_eq!(result, Some("list<Pen>".to_string()));
    }
}
