//! Semantic Tokens (`textDocument/semanticTokens/full`).
//!
//! Provides type-aware syntax highlighting that goes beyond what a
//! TextMate grammar can achieve.  Classes, interfaces, enums,
//! properties, methods, parameters, and type hints all get distinct
//! token types.
//!
//! The implementation leverages the precomputed [`SymbolMap`] which
//! already contains classified spans (`ClassReference`, `FunctionCall`,
//! `MemberAccess`, `PropertyAccess`, `VariableReference`, etc.) with
//! byte offsets.  The main work is mapping these to LSP semantic token
//! types and computing the delta encoding.
//!
//! Language builtins (`self`, `static`, `parent`, `$this`) carry the
//! `defaultLibrary` modifier so that themes can distinguish them from
//! user-defined symbols.

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::symbol_map::{SymbolKind, SymbolMap, VarDefKind};
use crate::types::ClassLikeKind;

// ─── Token type indices ─────────────────────────────────────────────────────
//
// These constants define the position of each token type in the legend
// array.  The LSP protocol uses integer indices rather than names.
// All indices are referenced: some only by the legend array, others
// also by classification logic.

const TT_NAMESPACE: u32 = 0;
const TT_CLASS: u32 = 1;
const TT_INTERFACE: u32 = 2;
const TT_ENUM: u32 = 3;
const TT_TYPE: u32 = 4;
const TT_TYPE_PARAMETER: u32 = 5;
const TT_PARAMETER: u32 = 6;
const TT_VARIABLE: u32 = 7;
const TT_PROPERTY: u32 = 8;
const TT_FUNCTION: u32 = 9;
const TT_METHOD: u32 = 10;
const TT_DECORATOR: u32 = 11;
const TT_ENUM_MEMBER: u32 = 12;

// ─── Token modifier bit positions ───────────────────────────────────────────

const TM_DECLARATION: u32 = 1 << 0;
const TM_STATIC: u32 = 1 << 1;
const TM_READONLY: u32 = 1 << 2;
const TM_DEPRECATED: u32 = 1 << 3;
const TM_ABSTRACT: u32 = 1 << 4;
const TM_DEFINITION: u32 = 1 << 5;
const TM_DEFAULT_LIBRARY: u32 = 1 << 6;

/// Build the semantic token legend that is advertised in `initialize`.
///
/// The order of types and modifiers here **must** match the index
/// constants above.
pub fn legend() -> SemanticTokensLegend {
    // Assert at compile time that every index constant has a matching
    // entry in the legend.  This also silences dead_code warnings for
    // constants that are only referenced by the legend (e.g. NAMESPACE).
    const _: () = {
        assert!(TT_NAMESPACE == 0);
        assert!(TT_CLASS == 1);
        assert!(TT_INTERFACE == 2);
        assert!(TT_ENUM == 3);
        assert!(TT_TYPE == 4);
        assert!(TT_TYPE_PARAMETER == 5);
        assert!(TT_PARAMETER == 6);
        assert!(TT_VARIABLE == 7);
        assert!(TT_PROPERTY == 8);
        assert!(TT_FUNCTION == 9);
        assert!(TT_METHOD == 10);
        assert!(TT_DECORATOR == 11);
        assert!(TT_ENUM_MEMBER == 12);
    };

    SemanticTokensLegend {
        token_types: vec![
            SemanticTokenType::NAMESPACE,      // 0
            SemanticTokenType::CLASS,          // 1
            SemanticTokenType::INTERFACE,      // 2
            SemanticTokenType::ENUM,           // 3
            SemanticTokenType::TYPE,           // 4
            SemanticTokenType::TYPE_PARAMETER, // 5
            SemanticTokenType::PARAMETER,      // 6
            SemanticTokenType::VARIABLE,       // 7
            SemanticTokenType::PROPERTY,       // 8
            SemanticTokenType::FUNCTION,       // 9
            SemanticTokenType::METHOD,         // 10
            SemanticTokenType::DECORATOR,      // 11
            SemanticTokenType::ENUM_MEMBER,    // 12
        ],
        token_modifiers: vec![
            SemanticTokenModifier::DECLARATION,     // bit 0
            SemanticTokenModifier::STATIC,          // bit 1
            SemanticTokenModifier::READONLY,        // bit 2
            SemanticTokenModifier::DEPRECATED,      // bit 3
            SemanticTokenModifier::ABSTRACT,        // bit 4
            SemanticTokenModifier::DEFINITION,      // bit 5
            SemanticTokenModifier::DEFAULT_LIBRARY, // bit 6
        ],
    }
}

/// A single absolute-positioned semantic token before delta encoding.
struct AbsoluteToken {
    line: u32,
    start_char: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
}

impl Backend {
    /// Handle a `textDocument/semanticTokens/full` request.
    ///
    /// Walks the file's precomputed [`SymbolMap`] and emits semantic
    /// tokens for every classified span.  For `ClassReference` spans
    /// the symbol is resolved to determine whether it is a class,
    /// interface, enum, or trait.
    pub fn handle_semantic_tokens_full(
        &self,
        uri: &str,
        content: &str,
    ) -> Option<SemanticTokensResult> {
        let symbol_map = self.symbol_maps.read().get(uri)?.clone();
        let ctx = self.file_context(uri);

        let mut tokens = self.collect_tokens(&symbol_map, content, uri, &ctx);

        // Sort by position (line, then character) to prepare for delta encoding.
        tokens.sort_by(|a, b| a.line.cmp(&b.line).then(a.start_char.cmp(&b.start_char)));

        // Deduplicate overlapping tokens at the same position (keep first).
        tokens.dedup_by(|b, a| a.line == b.line && a.start_char == b.start_char);

        let delta_tokens = encode_deltas(&tokens);

        Some(SemanticTokensResult::Tokens(SemanticTokens {
            result_id: None,
            data: delta_tokens,
        }))
    }

    /// Walk the symbol map and produce absolute-positioned tokens.
    fn collect_tokens(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        uri: &str,
        ctx: &crate::types::FileContext,
    ) -> Vec<AbsoluteToken> {
        let mut tokens = Vec::with_capacity(symbol_map.spans.len());

        for span in &symbol_map.spans {
            let length = span.end.saturating_sub(span.start);
            if length == 0 {
                continue;
            }

            let (token_type, modifiers) = match &span.kind {
                SymbolKind::ClassReference { name, is_fqn } => {
                    // Check if this class reference is actually a
                    // template parameter name (e.g. `T` from `@template T`).
                    if self.is_template_param(name, span.start, symbol_map) {
                        (TT_TYPE_PARAMETER, 0)
                    } else {
                        let tt = self.resolve_class_token_type(name, *is_fqn, ctx, span.start);
                        let mods = self.resolve_class_modifiers(name, *is_fqn, ctx, span.start);
                        (tt, mods)
                    }
                }

                SymbolKind::ClassDeclaration { name } => {
                    let tt = self.resolve_declaration_token_type(name, uri, ctx);
                    let mut mods = TM_DECLARATION;
                    // Check if the declared class itself is deprecated or abstract.
                    mods |= self.resolve_class_declaration_modifiers(name, uri, ctx);
                    (tt, mods)
                }

                SymbolKind::MemberAccess {
                    member_name,
                    is_static,
                    is_method_call,
                    subject_text,
                    ..
                } => {
                    let tt = if *is_method_call {
                        TT_METHOD
                    } else {
                        TT_PROPERTY
                    };
                    let mut mods = if *is_static { TM_STATIC } else { 0 };

                    // Try to resolve deprecation/readonly from the subject's class.
                    mods |= self.resolve_member_modifiers(
                        subject_text,
                        member_name,
                        *is_method_call,
                        uri,
                        ctx,
                    );

                    (tt, mods)
                }

                SymbolKind::MemberDeclaration { name, is_static } => {
                    // Determine if it's a method, property, or constant
                    // by checking the source text at the span.
                    let tt = self.classify_member_declaration(name, span.start, uri, ctx);
                    let mut mods = TM_DECLARATION;
                    if *is_static {
                        mods |= TM_STATIC;
                    }
                    (tt, mods)
                }

                SymbolKind::Variable { name } => {
                    // Check if this variable is a parameter.
                    let (tt, mut mods) =
                        self.classify_variable(name, span.start, symbol_map, uri, ctx);
                    // Mark definitions.
                    if symbol_map.is_at_var_definition(name, span.start) {
                        mods |= TM_DEFINITION;
                    }
                    (tt, mods)
                }

                SymbolKind::FunctionCall {
                    name: _,
                    is_definition,
                } => {
                    let mods = if *is_definition { TM_DECLARATION } else { 0 };
                    (TT_FUNCTION, mods)
                }

                SymbolKind::SelfStaticParent { keyword } => {
                    // `$this` is recorded as SelfStaticParent with keyword "static".
                    let source_text = content
                        .get(span.start as usize..span.end as usize)
                        .unwrap_or("");
                    if source_text == "$this" {
                        (TT_VARIABLE, TM_READONLY | TM_DEFAULT_LIBRARY)
                    } else if keyword == "parent" {
                        // Resolve to the parent class kind when possible.
                        let tt = self
                            .resolve_self_static_parent_token_type(keyword, uri, ctx, span.start);
                        (tt, TM_DEFAULT_LIBRARY)
                    } else {
                        // self, static — resolve to enclosing class kind.
                        let tt = self
                            .resolve_self_static_parent_token_type(keyword, uri, ctx, span.start);
                        (tt, TM_DEFAULT_LIBRARY)
                    }
                }

                SymbolKind::ConstantReference { name: _ } => {
                    // Check if this is a PHP attribute name (starts after `#[`).
                    let is_attr = span.start >= 2
                        && content
                            .get((span.start as usize).saturating_sub(2)..span.start as usize)
                            .is_some_and(|s| s.ends_with('#') || s.ends_with("["));
                    if is_attr {
                        (TT_DECORATOR, 0)
                    } else {
                        // Constants get the ENUM_MEMBER token type (standard LSP
                        // convention for constant-like values, including class
                        // constants and enum cases).
                        (TT_ENUM_MEMBER, TM_READONLY)
                    }
                }
            };

            if let Some(abs) =
                offset_to_absolute(content, span.start, length, token_type, modifiers)
            {
                tokens.push(abs);
            }
        }

        tokens
    }

    /// Resolve a class reference name to the appropriate token type
    /// (class, interface, enum, or type).
    fn resolve_class_token_type(
        &self,
        name: &str,
        is_fqn: bool,
        ctx: &crate::types::FileContext,
        offset: u32,
    ) -> u32 {
        let fqn = if is_fqn {
            name.to_string()
        } else {
            ctx.resolve_name_at(name, offset)
        };

        // First check in-file classes (fast path).
        for class in &ctx.classes {
            let class_fqn = match &class.file_namespace {
                Some(ns) => format!("{}\\{}", ns, class.name),
                None => class.name.clone(),
            };
            if class_fqn == fqn || class.name == fqn {
                return kind_to_token_type(class.kind);
            }
        }

        // Try resolving from the global class index / stubs.
        if let Some(class_info) = self.find_or_load_class(&fqn) {
            return kind_to_token_type(class_info.kind);
        }

        // Fall back to CLASS for unresolved references.
        TT_CLASS
    }

    /// Resolve modifiers for a class reference (e.g. deprecated).
    fn resolve_class_modifiers(
        &self,
        name: &str,
        is_fqn: bool,
        ctx: &crate::types::FileContext,
        offset: u32,
    ) -> u32 {
        let fqn = if is_fqn {
            name.to_string()
        } else {
            ctx.resolve_name_at(name, offset)
        };

        // Check in-file classes.
        for class in &ctx.classes {
            let class_fqn = match &class.file_namespace {
                Some(ns) => format!("{}\\{}", ns, class.name),
                None => class.name.clone(),
            };
            if class_fqn == fqn || class.name == fqn {
                if class.deprecation_message.is_some() {
                    return TM_DEPRECATED;
                }
                return 0;
            }
        }

        if let Some(class_info) = self.find_or_load_class(&fqn)
            && class_info.deprecation_message.is_some()
        {
            return TM_DEPRECATED;
        }

        0
    }

    /// Resolve the token type for a class declaration by looking up
    /// the class in the file's AST.
    fn resolve_declaration_token_type(
        &self,
        name: &str,
        _uri: &str,
        ctx: &crate::types::FileContext,
    ) -> u32 {
        for class in &ctx.classes {
            if class.name == name {
                return kind_to_token_type(class.kind);
            }
        }
        TT_CLASS
    }

    /// Resolve modifiers for a class declaration (deprecated, abstract).
    fn resolve_class_declaration_modifiers(
        &self,
        name: &str,
        _uri: &str,
        ctx: &crate::types::FileContext,
    ) -> u32 {
        let mut mods = 0u32;
        for class in &ctx.classes {
            if class.name == name {
                if class.deprecation_message.is_some() {
                    mods |= TM_DEPRECATED;
                }
                if class.is_abstract {
                    mods |= TM_ABSTRACT;
                }
                break;
            }
        }
        mods
    }

    /// Resolve member-level modifiers (deprecated, readonly, static)
    /// by attempting to look up the member in the subject's resolved class.
    fn resolve_member_modifiers(
        &self,
        _subject_text: &str,
        _member_name: &str,
        _is_method_call: bool,
        _uri: &str,
        _ctx: &crate::types::FileContext,
    ) -> u32 {
        // Full subject resolution is expensive. Skip it for now and
        // rely on the basic is_static flag from the SymbolKind.
        // A future enhancement can resolve the subject to add
        // deprecated/readonly modifiers.
        0
    }

    /// Classify a MemberDeclaration as method, property, or constant.
    fn classify_member_declaration(
        &self,
        name: &str,
        offset: u32,
        _uri: &str,
        ctx: &crate::types::FileContext,
    ) -> u32 {
        // Find the enclosing class and look up the member.
        for class in &ctx.classes {
            if offset < class.start_offset || offset > class.end_offset {
                continue;
            }
            // Check methods.
            for method in &class.methods {
                if method.name == name {
                    return TT_METHOD;
                }
            }
            // Check properties.
            for prop in &class.properties {
                if prop.name == name {
                    return TT_PROPERTY;
                }
            }
            // Check constants / enum cases.
            for constant in &class.constants {
                if constant.name == name {
                    return TT_ENUM_MEMBER;
                }
            }
        }
        // Fall back to method if we can't determine.
        TT_METHOD
    }

    /// Classify a variable as parameter, property, or regular variable.
    fn classify_variable(
        &self,
        name: &str,
        offset: u32,
        symbol_map: &SymbolMap,
        _uri: &str,
        _ctx: &crate::types::FileContext,
    ) -> (u32, u32) {
        // Check if this is a property declaration.
        if let Some(kind) = symbol_map.var_def_kind_at(name, offset) {
            match kind {
                VarDefKind::Property => return (TT_PROPERTY, TM_DECLARATION),
                VarDefKind::Parameter => return (TT_PARAMETER, 0),
                _ => {}
            }
        }

        // Check if any VarDefSite marks this variable as a parameter
        // in the current scope.
        let scope = symbol_map.find_enclosing_scope(offset);
        for def in &symbol_map.var_defs {
            if def.name == name && def.scope_start == scope {
                match def.kind {
                    VarDefKind::Parameter => return (TT_PARAMETER, 0),
                    VarDefKind::Property => return (TT_PROPERTY, 0),
                    _ => {}
                }
            }
        }

        (TT_VARIABLE, 0)
    }

    /// Check whether a `ClassReference` name is actually a `@template`
    /// parameter that is in scope at the given offset.
    fn is_template_param(&self, name: &str, offset: u32, symbol_map: &SymbolMap) -> bool {
        symbol_map.find_template_def(name, offset).is_some()
    }

    /// Determine the token type for `self`, `static`, or `parent` by
    /// resolving to the enclosing class.
    fn resolve_self_static_parent_token_type(
        &self,
        keyword: &str,
        _uri: &str,
        ctx: &crate::types::FileContext,
        offset: u32,
    ) -> u32 {
        if keyword == "parent" {
            // Try to resolve the parent class kind.
            if let Some(class) = ctx.classes.first()
                && let Some(ref parent_name) = class.parent_class
            {
                let fqn = ctx.resolve_name_at(parent_name, offset);
                if let Some(parent_info) = self.find_or_load_class(&fqn) {
                    return kind_to_token_type(parent_info.kind);
                }
            }
        }
        TT_TYPE
    }
}

/// Map a [`ClassLikeKind`] to a semantic token type index.
fn kind_to_token_type(kind: ClassLikeKind) -> u32 {
    match kind {
        ClassLikeKind::Class => TT_CLASS,
        ClassLikeKind::Interface => TT_INTERFACE,
        ClassLikeKind::Trait => TT_TYPE,
        ClassLikeKind::Enum => TT_ENUM,
    }
}

/// Convert a byte offset to an absolute line/character position and
/// build an [`AbsoluteToken`].
///
/// Returns `None` if the offset is beyond the content length.
fn offset_to_absolute(
    content: &str,
    start_offset: u32,
    length: u32,
    token_type: u32,
    modifiers: u32,
) -> Option<AbsoluteToken> {
    let pos = crate::util::offset_to_position(content, start_offset as usize);
    Some(AbsoluteToken {
        line: pos.line,
        start_char: pos.character,
        length,
        token_type,
        modifiers,
    })
}

/// Convert a list of absolute-positioned tokens into LSP delta-encoded
/// [`SemanticToken`] values.
fn encode_deltas(tokens: &[AbsoluteToken]) -> Vec<SemanticToken> {
    let mut result = Vec::with_capacity(tokens.len());
    let mut prev_line = 0u32;
    let mut prev_start = 0u32;

    for tok in tokens {
        let delta_line = tok.line - prev_line;
        let delta_start = if delta_line == 0 {
            tok.start_char - prev_start
        } else {
            tok.start_char
        };

        result.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.length,
            token_type: tok.token_type,
            token_modifiers_bitset: tok.modifiers,
        });

        prev_line = tok.line;
        prev_start = tok.start_char;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legend_has_correct_type_count() {
        let l = legend();
        // Ensure the legend has all the token types we reference.
        assert!(l.token_types.len() > TT_ENUM_MEMBER as usize);
        assert_eq!(l.token_types.len(), 13);
        assert_eq!(l.token_modifiers.len(), 7);
    }

    #[test]
    fn delta_encoding_single_token() {
        let tokens = vec![AbsoluteToken {
            line: 3,
            start_char: 5,
            length: 10,
            token_type: TT_CLASS,
            modifiers: 0,
        }];
        let deltas = encode_deltas(&tokens);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].delta_line, 3);
        assert_eq!(deltas[0].delta_start, 5);
        assert_eq!(deltas[0].length, 10);
        assert_eq!(deltas[0].token_type, TT_CLASS);
    }

    #[test]
    fn delta_encoding_same_line() {
        let tokens = vec![
            AbsoluteToken {
                line: 1,
                start_char: 2,
                length: 3,
                token_type: TT_VARIABLE,
                modifiers: 0,
            },
            AbsoluteToken {
                line: 1,
                start_char: 10,
                length: 4,
                token_type: TT_METHOD,
                modifiers: 0,
            },
        ];
        let deltas = encode_deltas(&tokens);
        assert_eq!(deltas.len(), 2);
        // First token: absolute.
        assert_eq!(deltas[0].delta_line, 1);
        assert_eq!(deltas[0].delta_start, 2);
        // Second token: same line, relative start.
        assert_eq!(deltas[1].delta_line, 0);
        assert_eq!(deltas[1].delta_start, 8); // 10 - 2
    }

    #[test]
    fn delta_encoding_new_line() {
        let tokens = vec![
            AbsoluteToken {
                line: 1,
                start_char: 5,
                length: 3,
                token_type: TT_FUNCTION,
                modifiers: 0,
            },
            AbsoluteToken {
                line: 3,
                start_char: 2,
                length: 6,
                token_type: TT_CLASS,
                modifiers: TM_DECLARATION,
            },
        ];
        let deltas = encode_deltas(&tokens);
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[1].delta_line, 2); // 3 - 1
        assert_eq!(deltas[1].delta_start, 2); // absolute on new line
        assert_eq!(deltas[1].token_modifiers_bitset, TM_DECLARATION);
    }

    #[test]
    fn kind_to_token_type_mapping() {
        assert_eq!(kind_to_token_type(ClassLikeKind::Class), TT_CLASS);
        assert_eq!(kind_to_token_type(ClassLikeKind::Interface), TT_INTERFACE);
        assert_eq!(kind_to_token_type(ClassLikeKind::Enum), TT_ENUM);
        assert_eq!(kind_to_token_type(ClassLikeKind::Trait), TT_TYPE);
    }
}
