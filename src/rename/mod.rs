//! Rename (`textDocument/rename`) and prepare-rename support.
//!
//! When the user triggers a rename on a symbol, the LSP first calls
//! `prepareRename` to validate that the symbol is renameable and to
//! return the range + current name of the symbol.  If the user
//! confirms, `rename` is called with the new name, and we produce a
//! `WorkspaceEdit` that replaces every occurrence across the workspace.
//!
//! The heavy lifting (finding all references) is delegated to the
//! existing `find_references` infrastructure.  This module adds:
//!
//! - Vendor rejection: symbols defined under the vendor directory
//!   cannot be renamed.
//! - Non-renameable symbol rejection: keywords like `self`, `static`,
//!   `parent`, and `$this` cannot be renamed.
//! - Property name fixup: `$this->foo` references need the edit to
//!   replace only `foo`, not the `$` prefix.  Static properties
//!   (`self::$prop`) include the `$` in the source but the rename
//!   should replace the whole `$prop` token consistently.

mod tests;

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::symbol_map::SymbolKind;
use crate::util::{offset_to_position, position_to_offset};

/// Symbols that cannot be renamed.
const NON_RENAMEABLE_KEYWORDS: &[&str] = &["self", "static", "parent"];

impl Backend {
    /// Handle `textDocument/prepareRename`.
    ///
    /// Validates that the symbol under the cursor is renameable and
    /// returns its range and current name.  Returns `None` (which the
    /// LSP layer translates to an error) when the symbol cannot be
    /// renamed.
    pub(crate) fn handle_prepare_rename(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<PrepareRenameResponse> {
        let offset = position_to_offset(content, position);

        let span = self.lookup_symbol_map(uri, offset).or_else(|| {
            if offset > 0 {
                self.lookup_symbol_map(uri, offset - 1)
            } else {
                None
            }
        })?;

        // Reject non-renameable symbols.
        if let SymbolKind::SelfStaticParent { keyword } = &span.kind {
            // `$this` is recorded as SelfStaticParent { keyword: "static" }.
            let source_text = content.get(span.start as usize..span.end as usize)?;
            if source_text == "$this" || NON_RENAMEABLE_KEYWORDS.contains(&keyword.as_str()) {
                return None;
            }
        }

        // Extract the symbol name and validate it's something we can rename.
        let (name, range) =
            self.renameable_symbol_info(uri, content, &span.kind, span.start, span.end)?;

        // Reject vendor symbols: if the definition lives under the
        // vendor directory the user shouldn't rename it.
        if self.is_vendor_symbol(uri, content, position) {
            return None;
        }

        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range,
            placeholder: name,
        })
    }

    /// Handle `textDocument/rename`.
    ///
    /// Produces a `WorkspaceEdit` that renames every occurrence of the
    /// symbol under the cursor to `new_name`.
    pub(crate) fn handle_rename(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        new_name: &str,
    ) -> Option<WorkspaceEdit> {
        let offset = position_to_offset(content, position);

        let span = self.lookup_symbol_map(uri, offset).or_else(|| {
            if offset > 0 {
                self.lookup_symbol_map(uri, offset - 1)
            } else {
                None
            }
        })?;

        // Reject non-renameable symbols (same logic as prepare_rename).
        if let SymbolKind::SelfStaticParent { keyword } = &span.kind {
            let source_text = content.get(span.start as usize..span.end as usize)?;
            if source_text == "$this" || NON_RENAMEABLE_KEYWORDS.contains(&keyword.as_str()) {
                return None;
            }
        }

        // Reject vendor symbols.
        if self.is_vendor_symbol(uri, content, position) {
            return None;
        }

        // Find all references (including the declaration).
        let locations = self.find_references(uri, content, position, true)?;

        if locations.is_empty() {
            return None;
        }

        // Determine whether this is a property rename.  Properties are
        // special because the `$` prefix is part of the declaration but
        // usage sites via `->` or `?->` don't include it.
        let is_property = self.is_property_rename(&span.kind, uri, &span);
        let is_variable = matches!(&span.kind, SymbolKind::Variable { .. }) && !is_property;

        // Build the workspace edit.  Group text edits by document URI.
        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

        for location in &locations {
            let loc_uri_str = location.uri.to_string();

            // For each reference location, we need the file content to
            // inspect what text is at that range.
            let loc_content = if loc_uri_str == uri {
                Some(content.to_string())
            } else {
                self.get_file_content(&loc_uri_str)
            };

            let edit_text = if is_variable {
                // Variables: the reference range includes the `$`, so
                // the new name should also include it.
                if new_name.starts_with('$') {
                    new_name.to_string()
                } else {
                    format!("${}", new_name)
                }
            } else if is_property {
                // Properties: the reference may or may not include `$`.
                // Check the actual source text at the location to decide.
                let has_dollar = loc_content.as_ref().is_some_and(|c| {
                    let start_off = crate::util::position_to_byte_offset(c, location.range.start);
                    c.as_bytes().get(start_off) == Some(&b'$')
                });
                let bare_name = new_name.strip_prefix('$').unwrap_or(new_name);
                if has_dollar {
                    format!("${}", bare_name)
                } else {
                    bare_name.to_string()
                }
            } else {
                new_name.to_string()
            };

            let text_edit = TextEdit {
                range: location.range,
                new_text: edit_text,
            };

            changes
                .entry(location.uri.clone())
                .or_default()
                .push(text_edit);
        }

        Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        })
    }

    /// Extract the renameable symbol name and its source range.
    ///
    /// Returns `None` for symbols that cannot be renamed.
    fn renameable_symbol_info(
        &self,
        _uri: &str,
        content: &str,
        kind: &SymbolKind,
        start: u32,
        end: u32,
    ) -> Option<(String, Range)> {
        let range = Range {
            start: offset_to_position(content, start as usize),
            end: offset_to_position(content, end as usize),
        };

        match kind {
            SymbolKind::Variable { name } => {
                // Include the `$` prefix in the range — the span already does.
                Some((format!("${}", name), range))
            }
            SymbolKind::ClassReference { name, .. } => Some((name.clone(), range)),
            SymbolKind::ClassDeclaration { name } => Some((name.clone(), range)),
            SymbolKind::MemberAccess { member_name, .. } => Some((member_name.clone(), range)),
            SymbolKind::MemberDeclaration { name, .. } => Some((name.clone(), range)),
            SymbolKind::FunctionCall { name } => Some((name.clone(), range)),
            SymbolKind::ConstantReference { name } => Some((name.clone(), range)),
            SymbolKind::SelfStaticParent { .. } => None,
        }
    }

    /// Check whether the symbol under the cursor is defined in a vendor
    /// file.
    ///
    /// We check this by resolving the definition location.  If the
    /// definition URI starts with the vendor prefix, the rename is
    /// rejected.
    fn is_vendor_symbol(&self, uri: &str, content: &str, position: Position) -> bool {
        let vendor_prefix = self.vendor_uri_prefix.lock().clone();

        if vendor_prefix.is_empty() {
            return false;
        }

        // Try to resolve the definition location.
        if let Some(loc) = self.resolve_definition(uri, content, position)
            && loc.uri.to_string().starts_with(&vendor_prefix)
        {
            return true;
        }

        false
    }

    /// Determine whether this rename targets a property (as opposed to
    /// a local variable or other symbol kind).
    fn is_property_rename(
        &self,
        kind: &SymbolKind,
        uri: &str,
        span: &crate::symbol_map::SymbolSpan,
    ) -> bool {
        match kind {
            SymbolKind::MemberAccess { is_method_call, .. } => !is_method_call,
            SymbolKind::MemberDeclaration { .. } => {
                // A MemberDeclaration is a property if it is NOT a method
                // and NOT a class constant.  We check the ast_map to see
                // whether the offset matches a method or constant name.
                let is_method = self
                    .get_classes_for_uri(uri)
                    .iter()
                    .flat_map(|classes| classes.iter())
                    .flat_map(|c| c.methods.iter())
                    .any(|m| m.name_offset != 0 && m.name_offset == span.start);
                let is_constant = self
                    .get_classes_for_uri(uri)
                    .iter()
                    .flat_map(|classes| classes.iter())
                    .flat_map(|c| c.constants.iter())
                    .any(|con| con.name_offset != 0 && con.name_offset == span.start);
                !is_method && !is_constant
            }
            SymbolKind::Variable { name } => {
                // Variable spans can represent property declarations.
                self.lookup_var_def_kind_at(uri, name, span.start)
                    .is_some_and(|k| k == crate::symbol_map::VarDefKind::Property)
            }
            _ => false,
        }
    }
}
