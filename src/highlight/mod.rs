//! Document highlighting (`textDocument/documentHighlight`).
//!
//! When the cursor lands on a symbol, returns all other occurrences of
//! that symbol in the current file so the editor can highlight them.
//! This module reuses the precomputed [`SymbolMap`] — no additional
//! parsing or AST walking is needed.
//!
//! Highlight kind assignment:
//! - Variable on an assignment LHS, parameter, foreach binding, or
//!   catch binding → `DocumentHighlightKind::Write`
//! - Everything else → `DocumentHighlightKind::Read`
//!
//! Scope rules:
//! - **Variables** are scoped to their enclosing function/method/closure.
//! - **Class names, member names, function names, constants** are
//!   file-global — all occurrences in the file are highlighted.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::symbol_map::{SymbolKind, SymbolMap, VarDefKind};
use crate::util::{offset_to_position, position_to_offset};

impl Backend {
    /// Collect document highlights for the symbol under the cursor.
    ///
    /// Returns `None` when the cursor is not on a navigable symbol.
    pub fn handle_document_highlight(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<Vec<DocumentHighlight>> {
        let offset = position_to_offset(content, position);

        // Look up the symbol span at the cursor (try exact, then one
        // byte back for end-of-token positions).
        let span = self.lookup_symbol_map(uri, offset).or_else(|| {
            if offset > 0 {
                self.lookup_symbol_map(uri, offset - 1)
            } else {
                None
            }
        })?;

        let maps = self.symbol_maps.read();
        let symbol_map = maps.get(uri)?;

        let highlights = match &span.kind {
            SymbolKind::Variable { name } => {
                // Check if this is actually a property declaration — if
                // so, highlight member accesses instead of local vars.
                if let Some(VarDefKind::Property) = symbol_map.var_def_kind_at(name, span.start) {
                    self.highlight_member_name(symbol_map, content, name)
                } else {
                    self.highlight_variable(symbol_map, content, name, span.start)
                }
            }
            SymbolKind::ClassReference { name, is_fqn } => {
                let ctx = self.file_context(uri);
                let fqn = if *is_fqn {
                    name.clone()
                } else {
                    Self::resolve_to_fqn(name, &ctx.use_map, &ctx.namespace)
                };
                self.highlight_class(symbol_map, content, &fqn, &ctx.use_map, &ctx.namespace)
            }
            SymbolKind::ClassDeclaration { name } => {
                let ctx = self.file_context(uri);
                let fqn = if let Some(ref ns) = ctx.namespace {
                    format!("{}\\{}", ns, name)
                } else {
                    name.clone()
                };
                self.highlight_class(symbol_map, content, &fqn, &ctx.use_map, &ctx.namespace)
            }
            SymbolKind::MemberAccess { member_name, .. } => {
                self.highlight_member_name(symbol_map, content, member_name)
            }
            SymbolKind::MemberDeclaration { name, .. } => {
                self.highlight_member_name(symbol_map, content, name)
            }
            SymbolKind::FunctionCall { name } => self.highlight_function(symbol_map, content, name),
            SymbolKind::ConstantReference { name } => {
                self.highlight_constant(symbol_map, content, name)
            }
            SymbolKind::SelfStaticParent { keyword } => {
                // `$this` is recorded as SelfStaticParent { keyword: "static" }.
                let source_text = content.get(span.start as usize..span.end as usize);
                if keyword == "static" && source_text.is_some_and(|s| s == "$this") {
                    self.highlight_this(symbol_map, content, span.start, uri)
                } else {
                    self.highlight_keyword(symbol_map, content, keyword, span.start, uri)
                }
            }
        };

        if highlights.is_empty() {
            None
        } else {
            Some(highlights)
        }
    }

    /// Highlight all occurrences of a variable within the same scope.
    fn highlight_variable(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        var_name: &str,
        cursor_offset: u32,
    ) -> Vec<DocumentHighlight> {
        let scope_start = symbol_map.find_enclosing_scope(cursor_offset);
        let mut highlights = Vec::new();
        let mut seen_offsets = std::collections::HashSet::new();

        // Collect from symbol spans.
        for span in &symbol_map.spans {
            if let SymbolKind::Variable { name } = &span.kind {
                if name != var_name {
                    continue;
                }
                let span_scope = symbol_map.find_enclosing_scope(span.start);
                if span_scope != scope_start {
                    continue;
                }
                seen_offsets.insert(span.start);

                let kind = if symbol_map.var_def_kind_at(name, span.start).is_some() {
                    DocumentHighlightKind::WRITE
                } else {
                    DocumentHighlightKind::READ
                };

                highlights.push(DocumentHighlight {
                    range: byte_range_to_lsp(content, span.start, span.end),
                    kind: Some(kind),
                });
            }
        }

        // Include var_def sites that may not have a matching Variable span
        // (e.g. parameters, foreach bindings).
        for def in &symbol_map.var_defs {
            if def.name == var_name
                && def.scope_start == scope_start
                && seen_offsets.insert(def.offset)
            {
                let end_offset = def.offset + 1 + def.name.len() as u32;
                highlights.push(DocumentHighlight {
                    range: byte_range_to_lsp(content, def.offset, end_offset),
                    kind: Some(DocumentHighlightKind::WRITE),
                });
            }
        }

        highlights.sort_by(cmp_highlight_range);
        highlights
    }

    /// Highlight all `$this` references within the same class body.
    fn highlight_this(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        cursor_offset: u32,
        uri: &str,
    ) -> Vec<DocumentHighlight> {
        let ctx_classes = self.ast_map.read().get(uri).cloned().unwrap_or_default();
        let current_class = crate::util::find_class_at_offset(&ctx_classes, cursor_offset);
        let (class_start, class_end) = match current_class {
            Some(cc) => (cc.start_offset, cc.end_offset),
            None => (0, u32::MAX),
        };

        let mut highlights = Vec::new();

        for span in &symbol_map.spans {
            if span.start < class_start || span.start > class_end {
                continue;
            }
            if let SymbolKind::SelfStaticParent { keyword } = &span.kind
                && keyword == "static"
            {
                let is_this = content
                    .get(span.start as usize..span.end as usize)
                    .is_some_and(|s| s == "$this");
                if is_this {
                    highlights.push(DocumentHighlight {
                        range: byte_range_to_lsp(content, span.start, span.end),
                        kind: Some(DocumentHighlightKind::READ),
                    });
                }
            }
        }

        highlights.sort_by(cmp_highlight_range);
        highlights
    }

    /// Highlight all occurrences of a class/interface/trait/enum name
    /// (by FQN) in the file.
    fn highlight_class(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        target_fqn: &str,
        use_map: &HashMap<String, String>,
        namespace: &Option<String>,
    ) -> Vec<DocumentHighlight> {
        let mut highlights = Vec::new();

        for span in &symbol_map.spans {
            let fqn = match &span.kind {
                SymbolKind::ClassReference { name, is_fqn } => {
                    if *is_fqn {
                        name.clone()
                    } else {
                        Self::resolve_to_fqn(name, use_map, namespace)
                    }
                }
                SymbolKind::ClassDeclaration { name } => {
                    if let Some(ns) = namespace {
                        format!("{}\\{}", ns, name)
                    } else {
                        name.clone()
                    }
                }
                _ => continue,
            };

            if fqn == target_fqn {
                highlights.push(DocumentHighlight {
                    range: byte_range_to_lsp(content, span.start, span.end),
                    kind: Some(DocumentHighlightKind::READ),
                });
            }
        }

        highlights.sort_by(cmp_highlight_range);
        highlights
    }

    /// Highlight all member accesses and declarations with the same name.
    ///
    /// This is a name-only match (no subject type resolution) which is
    /// acceptable for v1. It may produce false positives across unrelated
    /// classes in the same file, but that is a rare scenario.
    fn highlight_member_name(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        target_name: &str,
    ) -> Vec<DocumentHighlight> {
        let mut highlights = Vec::new();

        for span in &symbol_map.spans {
            match &span.kind {
                SymbolKind::MemberAccess { member_name, .. } if member_name == target_name => {
                    highlights.push(DocumentHighlight {
                        range: byte_range_to_lsp(content, span.start, span.end),
                        kind: Some(DocumentHighlightKind::READ),
                    });
                }
                SymbolKind::MemberDeclaration { name, .. } if name == target_name => {
                    highlights.push(DocumentHighlight {
                        range: byte_range_to_lsp(content, span.start, span.end),
                        kind: Some(DocumentHighlightKind::WRITE),
                    });
                }
                // Also match property declarations that appear as Variable spans.
                SymbolKind::Variable { name } if name == target_name => {
                    if symbol_map
                        .var_def_kind_at(name, span.start)
                        .is_some_and(|k| *k == VarDefKind::Property)
                    {
                        highlights.push(DocumentHighlight {
                            range: byte_range_to_lsp(content, span.start, span.end),
                            kind: Some(DocumentHighlightKind::WRITE),
                        });
                    }
                }
                _ => {}
            }
        }

        highlights.sort_by(cmp_highlight_range);
        highlights
    }

    /// Highlight all occurrences of a standalone function name.
    fn highlight_function(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        target_name: &str,
    ) -> Vec<DocumentHighlight> {
        let mut highlights = Vec::new();

        for span in &symbol_map.spans {
            if let SymbolKind::FunctionCall { name } = &span.kind
                && name == target_name
            {
                highlights.push(DocumentHighlight {
                    range: byte_range_to_lsp(content, span.start, span.end),
                    kind: Some(DocumentHighlightKind::READ),
                });
            }
        }

        highlights.sort_by(cmp_highlight_range);
        highlights
    }

    /// Highlight all occurrences of a constant name.
    fn highlight_constant(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        target_name: &str,
    ) -> Vec<DocumentHighlight> {
        let mut highlights = Vec::new();

        for span in &symbol_map.spans {
            if let SymbolKind::ConstantReference { name } = &span.kind
                && name == target_name
            {
                highlights.push(DocumentHighlight {
                    range: byte_range_to_lsp(content, span.start, span.end),
                    kind: Some(DocumentHighlightKind::READ),
                });
            }
        }

        highlights.sort_by(cmp_highlight_range);
        highlights
    }

    /// Highlight all occurrences of `self`, `static`, or `parent` within
    /// the same class body.
    fn highlight_keyword(
        &self,
        symbol_map: &SymbolMap,
        content: &str,
        target_keyword: &str,
        cursor_offset: u32,
        uri: &str,
    ) -> Vec<DocumentHighlight> {
        let ctx_classes = self.ast_map.read().get(uri).cloned().unwrap_or_default();
        let current_class = crate::util::find_class_at_offset(&ctx_classes, cursor_offset);
        let (class_start, class_end) = match current_class {
            Some(cc) => (cc.start_offset, cc.end_offset),
            None => (0, u32::MAX),
        };

        let mut highlights = Vec::new();

        for span in &symbol_map.spans {
            if span.start < class_start || span.start > class_end {
                continue;
            }
            if let SymbolKind::SelfStaticParent { keyword } = &span.kind
                && keyword == target_keyword
            {
                // Make sure we're not matching `$this` tokens when
                // the target is the `static` keyword.
                if target_keyword == "static" {
                    let is_this = content
                        .get(span.start as usize..span.end as usize)
                        .is_some_and(|s| s == "$this");
                    if is_this {
                        continue;
                    }
                }
                highlights.push(DocumentHighlight {
                    range: byte_range_to_lsp(content, span.start, span.end),
                    kind: Some(DocumentHighlightKind::READ),
                });
            }
        }

        highlights.sort_by(cmp_highlight_range);
        highlights
    }
}

/// Convert a byte offset range to an LSP `Range`.
fn byte_range_to_lsp(content: &str, start: u32, end: u32) -> Range {
    let start_pos = offset_to_position(content, start as usize);
    let end_pos = offset_to_position(content, end as usize);
    Range {
        start: start_pos,
        end: end_pos,
    }
}

/// Compare two document highlights by position for stable ordering.
fn cmp_highlight_range(a: &DocumentHighlight, b: &DocumentHighlight) -> std::cmp::Ordering {
    a.range
        .start
        .line
        .cmp(&b.range.start.line)
        .then(a.range.start.character.cmp(&b.range.start.character))
}
