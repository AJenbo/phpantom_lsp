//! PHPDoc tag completion.
//!
//! When the user types `@` inside a `/** … */` docblock, this module
//! provides context-aware tag suggestions.  The tags offered depend on
//! what PHP symbol follows the docblock:
//!
//! - **Function / Method**: `@param`, `@return`, `@throws`, …
//! - **Class / Interface / Trait / Enum**: `@property`, `@method`, `@mixin`, `@template`, …
//! - **Property**: `@var`, `@deprecated`, …
//! - **Constant**: `@var`, `@deprecated`, …
//! - **Unknown / file-level**: all tags
//!
//! When the symbol following the docblock can be parsed, completions are
//! pre-filled with concrete type and parameter information extracted from
//! the declaration (e.g. `@param string $name` instead of bare `@param`).
//!
//! Smart `@throws` completion analyses the function/method body for
//! `throw new ExceptionType(…)` statements that are not caught by a
//! `try/catch` block enclosing them, and suggests documenting each
//! uncaught exception type.  When the exception class is not yet imported
//! via a `use` statement, an `additional_text_edits` entry is added to
//! insert the import automatically.
//!
//! Context detection and symbol info extraction live in the sibling
//! [`super::phpdoc_context`] module and are re-exported here for
//! backward compatibility.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::completion::throws_analysis;
use crate::completion::use_edit::{build_use_edit, find_use_insert_position};

// Re-export comment-position helpers so existing consumers (tests,
// handler, catch_completion) that import from `phpdoc::` keep working.
pub use super::comment_position::{
    is_inside_docblock, is_inside_non_doc_comment, position_to_byte_offset,
};

// Re-export all public items from `phpdoc_context` so that existing
// consumers (`handler.rs`, tests) that import from `phpdoc::` keep
// working without path changes.
pub use super::phpdoc_context::{
    DocblockContext, DocblockTypingContext, SymbolInfo, detect_context,
    detect_docblock_typing_position, extract_phpdoc_prefix, extract_symbol_info,
};

// ─── Existing-tag scanning ──────────────────────────────────────────────────

/// Collect the names of parameters already documented with `@param` tags
/// in the current docblock above the cursor.
pub fn find_existing_param_tags(content: &str, position: Position) -> Vec<String> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    // Find the opening `/**`
    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let docblock_so_far = &before_cursor[open_pos..];

    let mut existing = Vec::new();
    for line in docblock_so_far.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("@param") {
            let rest = rest.trim();
            // @param may have: Type $name desc  or just $name
            for word in rest.split_whitespace() {
                if word.starts_with('$') {
                    existing.push(word.to_string());
                    break;
                }
            }
        }
    }

    existing
}

/// Check whether `@return` is already documented in the current docblock.
fn has_existing_return_tag(content: &str, position: Position) -> bool {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return false,
    };

    let docblock_so_far = &before_cursor[open_pos..];
    docblock_so_far.lines().any(|line| {
        let trimmed = line.trim().trim_start_matches('*').trim();
        trimmed.starts_with("@return")
    })
}

/// Collect exception type names already documented with `@throws` tags
/// in the current docblock above the cursor.
///
/// Returns short type names as written in the docblock (e.g.
/// `"InvalidArgumentException"`, `"\\RuntimeException"`).
pub fn find_existing_throws_tags(content: &str, position: Position) -> Vec<String> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    let open_pos = match before_cursor.rfind("/**") {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    // Also look at the docblock text AFTER the cursor (the user may have
    // already documented some throws below the cursor line).
    let close_pos = content[open_pos..].find("*/").map(|p| open_pos + p + 2);
    let docblock = if let Some(end) = close_pos {
        &content[open_pos..end]
    } else {
        &content[open_pos..byte_offset.min(content.len())]
    };

    let mut existing = Vec::new();
    for line in docblock.lines() {
        let trimmed = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = trimmed.strip_prefix("@throws") {
            let rest = rest.trim();
            if let Some(type_name) = rest.split_whitespace().next() {
                let clean = type_name.trim_start_matches('\\');
                if !clean.is_empty() {
                    existing.push(clean.to_string());
                }
            }
        }
    }

    existing
}

// ─── Tag Definitions ────────────────────────────────────────────────────────

/// A PHPDoc / PHPStan tag definition with metadata for completion.
struct TagDef {
    /// The tag text including `@` (e.g. `"@param"`).
    tag: &'static str,
    /// Brief one-line description shown in the completion detail.
    detail: &'static str,
    /// Display label showing usage format (e.g. `"@param Type $name"`).
    /// `None` means use `tag` as the label.
    label: Option<&'static str>,
}

/// Strip the leading `@` from a tag string.
///
/// The user has already typed `@` (or `@par…`) in the buffer and the LSP
/// client only replaces the *word* portion after `@`.  If the insert text
/// still contains `@`, the result is a doubled `@@tag`.
fn strip_at(s: &str) -> &str {
    s.strip_prefix('@').unwrap_or(s)
}

// ── Function / Method tags ──────────────────────────────────────────────────

const FUNCTION_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@param",
        detail: "Document a function parameter",
        label: Some("@param Type $name"),
    },
    TagDef {
        tag: "@return",
        detail: "Document the return type",
        label: Some("@return Type"),
    },
    TagDef {
        tag: "@throws",
        detail: "Document a thrown exception",
        label: Some("@throws ExceptionType"),
    },
    TagDef {
        tag: "@inheritdoc",
        detail: "Inherit documentation from parent",
        label: None,
    },
];

// ── Class-like tags ─────────────────────────────────────────────────────────

const CLASS_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@property",
        detail: "Declare a magic property",
        label: Some("@property Type $name"),
    },
    TagDef {
        tag: "@method",
        detail: "Declare a magic method",
        label: Some("@method ReturnType name()"),
    },
    TagDef {
        tag: "@mixin",
        detail: "Declare a mixin class",
        label: Some("@mixin ClassName"),
    },
    TagDef {
        tag: "@template",
        detail: "Declare a generic type parameter",
        label: Some("@template T"),
    },
    TagDef {
        tag: "@extends",
        detail: "Specify generic parent class type",
        label: Some("@extends ClassName<Type>"),
    },
    TagDef {
        tag: "@implements",
        detail: "Specify generic interface type",
        label: Some("@implements InterfaceName<Type>"),
    },
    TagDef {
        tag: "@use",
        detail: "Specify generic trait type",
        label: Some("@use TraitName<Type>"),
    },
];

// ── Property tags ───────────────────────────────────────────────────────────

const PROPERTY_TAGS: &[TagDef] = &[TagDef {
    tag: "@var",
    detail: "Document the property type",
    label: Some("@var Type"),
}];

// ── Constant tags ───────────────────────────────────────────────────────────

const CONSTANT_TAGS: &[TagDef] = &[TagDef {
    tag: "@var",
    detail: "Document the constant type",
    label: Some("@var Type"),
}];

// ── General tags (available everywhere) ─────────────────────────────────────

const GENERAL_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@deprecated",
        detail: "Mark as deprecated",
        label: None,
    },
    TagDef {
        tag: "@see",
        detail: "Reference to related element",
        label: Some("@see ClassName::method()"),
    },
    TagDef {
        tag: "@since",
        detail: "Version when this was introduced",
        label: Some("@since 1.0.0"),
    },
    TagDef {
        tag: "@example",
        detail: "Reference to an example file",
        label: None,
    },
    TagDef {
        tag: "@link",
        detail: "URL to external documentation",
        label: Some("@link https://"),
    },
    TagDef {
        tag: "@internal",
        detail: "Mark as internal / not part of the public API",
        label: None,
    },
    TagDef {
        tag: "@todo",
        detail: "Document a to-do item",
        label: None,
    },
];

// ── Inline tags (inside code, not before a declaration) ─────────────────────
// Only tags that make sense as inline annotations: @var for type
// narrowing, @throws for exception hinting, plus a handful of general
// documentation tags.

const INLINE_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@var",
        detail: "Narrow the variable type",
        label: Some("@var Type"),
    },
    TagDef {
        tag: "@throws",
        detail: "Hint at an exception thrown by the next statement",
        label: Some("@throws ExceptionType"),
    },
    TagDef {
        tag: "@see",
        detail: "Reference to related element",
        label: Some("@see ClassName::method()"),
    },
    TagDef {
        tag: "@example",
        detail: "Reference to an example file",
        label: None,
    },
    TagDef {
        tag: "@link",
        detail: "URL to external documentation",
        label: Some("@link https://"),
    },
    TagDef {
        tag: "@todo",
        detail: "Document a to-do item",
        label: None,
    },
];

// ── PHPStan tags ────────────────────────────────────────────────────────────

const PHPSTAN_FUNCTION_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@phpstan-assert",
        detail: "PHPStan: assert parameter type after call",
        label: Some("@phpstan-assert Type $var"),
    },
    TagDef {
        tag: "@phpstan-assert-if-true",
        detail: "PHPStan: assert type when method returns true",
        label: Some("@phpstan-assert-if-true Type $var"),
    },
    TagDef {
        tag: "@phpstan-assert-if-false",
        detail: "PHPStan: assert type when method returns false",
        label: Some("@phpstan-assert-if-false Type $var"),
    },
    TagDef {
        tag: "@phpstan-self-out",
        detail: "PHPStan: narrow the type of $this after call",
        label: Some("@phpstan-self-out Type"),
    },
    TagDef {
        tag: "@phpstan-this-out",
        detail: "PHPStan: narrow the type of $this after call",
        label: Some("@phpstan-this-out Type"),
    },
    TagDef {
        tag: "@phpstan-ignore-next-line",
        detail: "PHPStan: suppress errors on the next line",
        label: None,
    },
    TagDef {
        tag: "@phpstan-type",
        detail: "PHPStan: define a local type alias",
        label: Some("@phpstan-type TypeName = Type"),
    },
    TagDef {
        tag: "@phpstan-import-type",
        detail: "PHPStan: import a type alias from another class",
        label: Some("@phpstan-import-type TypeName from ClassName"),
    },
];

const PHPSTAN_CLASS_TAGS: &[TagDef] = &[
    TagDef {
        tag: "@phpstan-type",
        detail: "PHPStan: define a local type alias",
        label: Some("@phpstan-type TypeName = Type"),
    },
    TagDef {
        tag: "@phpstan-import-type",
        detail: "PHPStan: import a type alias from another class",
        label: Some("@phpstan-import-type TypeName from ClassName"),
    },
    TagDef {
        tag: "@phpstan-require-extends",
        detail: "PHPStan: require extending a specific class",
        label: Some("@phpstan-require-extends ClassName"),
    },
    TagDef {
        tag: "@phpstan-require-implements",
        detail: "PHPStan: require implementing a specific interface",
        label: Some("@phpstan-require-implements InterfaceName"),
    },
];

const PHPSTAN_PROPERTY_TAGS: &[TagDef] = &[];

// ─── Completion Builder ─────────────────────────────────────────────────────

/// Build completion items for PHPDoc tags based on context.
///
/// `content` is the full file text (used to extract symbol info and
/// detect already-documented parameters).
/// `prefix` is the partial tag the user has typed (e.g. `"@par"`, `"@"`).
/// `context` indicates what PHP symbol follows the docblock.
/// `position` is the cursor position (used to scan the docblock and the
/// following declaration).
/// `use_map` maps short class names to FQNs from `use` statements.
/// `file_namespace` is the file's declared namespace (if any).
///
/// Returns the list of matching `CompletionItem`s.
pub fn build_phpdoc_completions(
    content: &str,
    prefix: &str,
    context: DocblockContext,
    position: Position,
    use_map: &HashMap<String, String>,
    file_namespace: &Option<String>,
) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut items = Vec::new();

    // Extract symbol info for smart pre-filling
    let sym = extract_symbol_info(content, position);

    // Collect all applicable tag lists based on context
    let tag_lists: Vec<&[TagDef]> = match context {
        DocblockContext::FunctionOrMethod => {
            vec![FUNCTION_TAGS, GENERAL_TAGS, PHPSTAN_FUNCTION_TAGS]
        }
        DocblockContext::ClassLike => vec![CLASS_TAGS, GENERAL_TAGS, PHPSTAN_CLASS_TAGS],
        DocblockContext::Property => vec![PROPERTY_TAGS, GENERAL_TAGS, PHPSTAN_PROPERTY_TAGS],
        DocblockContext::Constant => vec![CONSTANT_TAGS, GENERAL_TAGS],
        DocblockContext::Inline => vec![INLINE_TAGS],
        DocblockContext::Unknown => vec![
            FUNCTION_TAGS,
            CLASS_TAGS,
            PROPERTY_TAGS,
            GENERAL_TAGS,
            PHPSTAN_FUNCTION_TAGS,
            PHPSTAN_CLASS_TAGS,
            PHPSTAN_PROPERTY_TAGS,
        ],
    };

    for tags in tag_lists {
        for def in tags {
            if !def.tag.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            if !seen.insert(def.tag) {
                continue;
            }

            // ── Smart items for @throws ─────────────────────────────
            if def.tag == "@throws"
                && matches!(
                    context,
                    DocblockContext::FunctionOrMethod | DocblockContext::Unknown
                )
            {
                let uncaught = throws_analysis::find_uncaught_throw_types(content, position);
                let existing_throws = find_existing_throws_tags(content, position);

                // Filter out already-documented throws
                let missing: Vec<&String> = uncaught
                    .iter()
                    .filter(|t| {
                        !existing_throws
                            .iter()
                            .any(|e| e.eq_ignore_ascii_case(t.as_str()))
                    })
                    .collect();

                if !missing.is_empty() {
                    let use_insert_pos = find_use_insert_position(content);

                    for (idx, exc_type) in missing.iter().enumerate() {
                        let insert = format!("throws {}", exc_type);
                        let label = format!("@throws {}", exc_type);

                        // Build an auto-import edit if the exception type
                        // isn't already imported.
                        let additional_edits = throws_analysis::resolve_exception_fqn(
                            exc_type,
                            use_map,
                            file_namespace,
                        )
                        .filter(|fqn| !throws_analysis::has_use_import(content, fqn))
                        .and_then(|fqn| build_use_edit(&fqn, use_insert_pos, file_namespace));

                        items.push(CompletionItem {
                            label,
                            kind: Some(CompletionItemKind::KEYWORD),
                            detail: Some(def.detail.to_string()),
                            insert_text: Some(insert),
                            filter_text: Some(def.tag.to_string()),
                            sort_text: Some(format!("0a_{}_{:03}", def.tag.to_lowercase(), idx)),
                            additional_text_edits: additional_edits,
                            ..CompletionItem::default()
                        });
                    }
                    // Smart items emitted — skip the generic fallback.
                    continue;
                }

                if !uncaught.is_empty() {
                    // Uncaught exceptions exist but all are already
                    // documented — nothing more to add, skip generic.
                    continue;
                }
                // No uncaught exceptions detected at all — fall through
                // to the generic `@throws ExceptionType` fallback so the
                // user can manually document exceptions the detection
                // missed (e.g. from external calls or abstract methods).
            }

            // ── Smart items for @param ──────────────────────────────
            if def.tag == "@param" {
                // If the function has parameters, offer smart pre-filled
                // items for each undocumented one.  When ALL params are
                // already documented (or the function has none), skip
                // entirely — the generic fallback is not useful.
                if !sym.params.is_empty() {
                    let existing = find_existing_param_tags(content, position);
                    let mut param_idx = 0u32;

                    for (type_hint, name) in &sym.params {
                        // Skip params already documented
                        if existing.iter().any(|e| e == name) {
                            continue;
                        }

                        let (insert, label) = if let Some(th) = type_hint {
                            (
                                format!("param {} {}", th, name),
                                format!("@param {} {}", th, name),
                            )
                        } else {
                            (format!("param {}", name), format!("@param {}", name))
                        };

                        items.push(CompletionItem {
                            label,
                            kind: Some(CompletionItemKind::KEYWORD),
                            detail: Some(def.detail.to_string()),
                            insert_text: Some(insert),
                            filter_text: Some(def.tag.to_string()),
                            sort_text: Some(format!(
                                "0a_{}_{:03}",
                                def.tag.to_lowercase(),
                                param_idx
                            )),
                            ..CompletionItem::default()
                        });
                        param_idx += 1;
                    }
                }
                // Always skip the generic fallback for @param — either
                // we emitted smart items above, or all params are
                // documented / there are none.
                continue;
            }

            // ── Smart item for @return ──────────────────────────────
            if def.tag == "@return" {
                if has_existing_return_tag(content, position) {
                    continue;
                }
                if let Some(ref ret) = sym.return_type
                    && ret != "void"
                {
                    items.push(CompletionItem {
                        label: format!("@return {}", ret),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some(def.detail.to_string()),
                        insert_text: Some(format!("return {}", ret)),
                        filter_text: Some(def.tag.to_string()),
                        sort_text: Some(format!("0a_{}", def.tag.to_lowercase())),
                        ..CompletionItem::default()
                    });
                    // Smart item emitted — skip the generic fallback.
                    continue;
                }
                if sym.return_type.as_deref() == Some("void") {
                    // Explicit `: void` type hint — the hint speaks for
                    // itself, no need to suggest @return in PHPDoc.
                    continue;
                }
                if sym.return_type.is_none() {
                    // No return type hint — offer `@return void` when the
                    // function body contains no return-with-value statements.
                    // When `: void` is already declared, the type hint
                    // speaks for itself and we skip the suggestion.
                    let body = throws_analysis::extract_function_body(content, position);
                    let has_return = body.as_deref().is_some_and(|b| {
                        // Quick scan for a `return` keyword followed by a
                        // non-identifier char (avoid matching inside
                        // strings would be ideal, but a simple word-boundary
                        // check is good enough for this heuristic).
                        let bytes = b.as_bytes();
                        let len = bytes.len();
                        let mut pos = 0;
                        let mut found = false;
                        while pos + 6 <= len {
                            if &b[pos..pos + 6] == "return" {
                                let before_ok = pos == 0
                                    || !bytes[pos - 1].is_ascii_alphanumeric()
                                        && bytes[pos - 1] != b'_';
                                let after_ok = pos + 6 >= len
                                    || !bytes[pos + 6].is_ascii_alphanumeric()
                                        && bytes[pos + 6] != b'_';
                                if before_ok && after_ok {
                                    // Check it's not just `return;` (no value)
                                    let after = b[pos + 6..].trim_start();
                                    if !after.starts_with(';') {
                                        found = true;
                                        break;
                                    }
                                }
                            }
                            pos += 1;
                        }
                        found
                    });
                    if !has_return {
                        items.push(CompletionItem {
                            label: "@return void".to_string(),
                            kind: Some(CompletionItemKind::KEYWORD),
                            detail: Some(def.detail.to_string()),
                            insert_text: Some("return void".to_string()),
                            filter_text: Some(def.tag.to_string()),
                            sort_text: Some(format!("0a_{}", def.tag.to_lowercase())),
                            ..CompletionItem::default()
                        });
                        continue;
                    }
                    // Body has return-with-value statements — fall through
                    // to the generic `@return Type` fallback so the user
                    // can type the actual return type manually.
                }
                // Return type not detected — fall through to the generic
                // `@return Type` fallback so the user can type it manually.
            }

            // ── Smart item for @var on properties / constants ───────
            if def.tag == "@var"
                && matches!(
                    context,
                    DocblockContext::Property | DocblockContext::Constant
                )
                && let Some(ref th) = sym.type_hint
            {
                items.push(CompletionItem {
                    label: format!("@var {}", th),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some(def.detail.to_string()),
                    insert_text: Some(format!("var {}", th)),
                    filter_text: Some(def.tag.to_string()),
                    sort_text: Some(format!("0a_{}", def.tag.to_lowercase())),
                    ..CompletionItem::default()
                });
                continue;
            }

            // ── Generic fallback ────────────────────────────────────
            let display_label = def.label.unwrap_or(def.tag);

            // PHPStan tags sort after standard tags.
            let sort_prefix = if def.tag.starts_with("@phpstan") {
                "2"
            } else {
                "1"
            };

            items.push(CompletionItem {
                label: display_label.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some(def.detail.to_string()),
                insert_text: Some(strip_at(def.tag).to_string()),
                filter_text: Some(def.tag.to_string()),
                sort_text: Some(format!("{}_{}", sort_prefix, def.tag.to_lowercase())),
                ..CompletionItem::default()
            });
        }
    }

    items
}
