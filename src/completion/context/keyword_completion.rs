/// PHP keyword completions for non-member expression/statement contexts.
use tower_lsp::lsp_types::*;

use crate::completion::class_completion::ClassNameContext;
use crate::types::ClassLikeKind;

/// Cursor context used to gate context-sensitive PHP keywords.
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeywordContext {
    /// Cursor is inside a function-like body (function, method, closure, arrow fn).
    pub in_function_like: bool,
    /// Cursor is inside a breakable construct (`for`/`foreach`/`while`/`do`/`switch`).
    pub in_breakable: bool,
    /// Cursor is inside a loop construct (`for`/`foreach`/`while`/`do`).
    pub in_loop: bool,
    /// Cursor is inside a `switch` body.
    pub in_switch: bool,
    /// Cursor is at top-level (outside classes/functions and brace-nested blocks).
    pub in_top_level: bool,
    /// Cursor is in a class/interface/enum declaration header where `extends` is valid.
    pub in_extends_declaration_header: bool,
    /// Cursor is in a class/enum declaration header where `implements` is valid.
    pub in_implements_declaration_header: bool,
    /// Cursor is in a class-like body (outside method/function scope).
    pub class_body_kind: Option<ClassLikeKind>,
    /// Cursor is right after a class-member modifier chain followed by
    /// whitespace (e.g. `public `, `private static `).
    pub after_member_modifier_chain: bool,
}

/// Core PHP keywords that can be completed in generic code contexts.
///
/// This intentionally excludes type keywords handled by type-hint/docblock
/// completion paths (`int`, `string`, etc.).
const PHP_KEYWORDS: &[&str] = &[
    "abstract",
    "as",
    "break",
    "case",
    "catch",
    "class",
    "clone",
    "const",
    "continue",
    "declare",
    "default",
    "do",
    "echo",
    "else",
    "elseif",
    "enum",
    "extends",
    "final",
    "finally",
    "fn",
    "for",
    "foreach",
    "function",
    "global",
    "goto",
    "if",
    "implements",
    "include",
    "include_once",
    "instanceof",
    "interface",
    "match",
    "namespace",
    "new",
    "print",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "require_once",
    "return",
    "static",
    "switch",
    "throw",
    "trait",
    "try",
    "unset",
    "use",
    "while",
    "yield",
];

/// Scalar types allowed as enum backing types in `enum Name: …`.
const BACKED_ENUM_TYPES: &[&str] = &["string", "int"];

/// Build keyword completion items for the typed `prefix`.
///
/// Keywords are only shown in unrestricted contexts (`Any`) to avoid
/// leaking into class-only positions such as `new`, `extends`, `implements`,
/// and import/type contexts.
pub(crate) fn build_keyword_completions(
    prefix: &str,
    class_ctx: ClassNameContext,
    ctx: KeywordContext,
) -> Vec<CompletionItem> {
    if !matches!(class_ctx, ClassNameContext::Any) {
        return Vec::new();
    }

    let prefix_lower = prefix.to_lowercase();
    PHP_KEYWORDS
        .iter()
        .enumerate()
        .filter(|(_, keyword)| keyword.starts_with(&prefix_lower))
        .filter(|(_, keyword)| keyword_allowed(keyword, ctx))
        .map(|(idx, keyword)| CompletionItem {
            label: (*keyword).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("PHP keyword".to_string()),
            insert_text: Some((*keyword).to_string()),
            filter_text: Some((*keyword).to_string()),
            sort_text: Some(format!("3_{idx:03}_{keyword}")),
            ..CompletionItem::default()
        })
        .collect()
}

/// Build completion items for enum backing types (`string`, `int`).
pub(crate) fn build_backed_enum_type_completions(prefix: &str) -> Vec<CompletionItem> {
    let prefix_lower = prefix.to_ascii_lowercase();
    BACKED_ENUM_TYPES
        .iter()
        .enumerate()
        .filter(|(_, ty)| ty.starts_with(&prefix_lower))
        .map(|(idx, ty)| CompletionItem {
            label: (*ty).to_string(),
            kind: Some(CompletionItemKind::KEYWORD),
            detail: Some("Enum backing type".to_string()),
            insert_text: Some((*ty).to_string()),
            filter_text: Some((*ty).to_string()),
            sort_text: Some(format!("0_enum_backed_{idx:03}_{ty}")),
            ..CompletionItem::default()
        })
        .collect()
}

fn keyword_allowed(keyword: &&str, ctx: KeywordContext) -> bool {
    if let Some(kind) = ctx.class_body_kind {
        return keyword_allowed_in_class_body(keyword, kind);
    }

    match *keyword {
        "return" | "yield" => ctx.in_function_like,
        "break" => ctx.in_breakable,
        "continue" => ctx.in_loop,
        "case" | "default" => ctx.in_switch,
        "namespace" => ctx.in_top_level,
        "extends" => ctx.in_extends_declaration_header,
        "implements" => ctx.in_implements_declaration_header,
        _ => true,
    }
}

fn keyword_allowed_in_class_body(keyword: &&str, kind: ClassLikeKind) -> bool {
    match kind {
        ClassLikeKind::Class | ClassLikeKind::Trait => matches!(
            *keyword,
            "public"
                | "protected"
                | "private"
                | "static"
                | "final"
                | "abstract"
                | "readonly"
                | "function"
                | "const"
                | "use"
        ),
        ClassLikeKind::Interface => matches!(*keyword, "public" | "function" | "const"),
        ClassLikeKind::Enum => matches!(
            *keyword,
            "public" | "protected" | "private" | "static" | "function" | "const" | "use" | "case"
        ),
    }
}
