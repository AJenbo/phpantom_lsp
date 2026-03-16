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
//! - Use-statement-aware class rename: when renaming a class, the
//!   `use` import FQN is updated (last segment only), aliases are
//!   preserved, and collisions with existing imports are resolved by
//!   introducing an alias.

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

        // Detect whether this is a class rename and resolve the FQN.
        let class_rename_fqn = self.resolve_class_rename_fqn(&span.kind, uri);

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

        // For class renames, delegate to the specialised handler that
        // understands `use` statements, aliases, and collisions.
        if let Some(ref fqn) = class_rename_fqn {
            return self.build_class_rename_edit(fqn, new_name, &locations);
        }

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

    /// Resolve the fully-qualified class name for a class rename.
    ///
    /// Returns `Some(fqn)` when the symbol being renamed is a class
    /// reference or class declaration, `None` otherwise.
    fn resolve_class_rename_fqn(&self, kind: &SymbolKind, uri: &str) -> Option<String> {
        match kind {
            SymbolKind::ClassReference { name, is_fqn } => {
                let ctx = self.file_context(uri);
                let fqn = if *is_fqn {
                    name.clone()
                } else {
                    Self::resolve_to_fqn(name, &ctx.use_map, &ctx.namespace)
                };
                Some(fqn.strip_prefix('\\').unwrap_or(&fqn).to_string())
            }
            SymbolKind::ClassDeclaration { name } => {
                let ctx = self.file_context(uri);
                let fqn = if let Some(ref ns) = ctx.namespace {
                    format!("{}\\{}", ns, name)
                } else {
                    name.clone()
                };
                Some(fqn)
            }
            _ => None,
        }
    }

    /// Build a `WorkspaceEdit` for a class rename that correctly handles
    /// `use` import statements, aliases, and import collisions.
    ///
    /// When renaming class `OldName` to `NewName`:
    ///
    /// - **`use Ns\OldName;`** becomes `use Ns\NewName;` and in-code
    ///   references `OldName` become `NewName`.
    /// - **`use Ns\OldName as Alias;`** becomes `use Ns\NewName as Alias;`
    ///   and in-code references `Alias` are left unchanged.
    /// - **Collision**: if the file already imports a different class with
    ///   the same short name as `NewName`, the renamed import gets an
    ///   alias (`use Ns\NewName as NewNameAlias;`) and in-code references
    ///   are updated to use that alias.
    fn build_class_rename_edit(
        &self,
        old_fqn: &str,
        new_short_name: &str,
        locations: &[Location],
    ) -> Option<WorkspaceEdit> {
        let old_fqn_normalized = old_fqn.strip_prefix('\\').unwrap_or(old_fqn);
        let old_short_name = crate::util::short_name(old_fqn_normalized);

        // Build the new FQN by replacing the last segment of the old FQN.
        let new_fqn = if let Some(ns_sep) = old_fqn_normalized.rfind('\\') {
            format!("{}\\{}", &old_fqn_normalized[..ns_sep], new_short_name)
        } else {
            new_short_name.to_string()
        };

        // Group locations by file URI for per-file processing.
        let mut locations_by_file: HashMap<String, Vec<&Location>> = HashMap::new();
        for loc in locations {
            locations_by_file
                .entry(loc.uri.to_string())
                .or_default()
                .push(loc);
        }

        let mut changes: HashMap<Url, Vec<TextEdit>> = HashMap::new();

        for (file_uri_str, file_locations) in &locations_by_file {
            let file_content = self.get_file_content(file_uri_str);
            let file_content = match file_content {
                Some(c) => c,
                None => continue,
            };

            // Get the file's use_map to understand import context.
            let file_use_map = self
                .use_map
                .read()
                .get(file_uri_str)
                .cloned()
                .unwrap_or_default();

            let parsed_uri = match Url::parse(file_uri_str) {
                Ok(u) => u,
                Err(_) => continue,
            };

            // Find the alias (if any) that imports the old FQN.
            let import_info = find_import_for_fqn(&file_use_map, old_fqn_normalized);

            // Determine whether the new short name would collide with
            // an existing import in this file.
            let has_collision = import_info.is_some()
                && new_short_name != old_short_name
                && has_import_collision(&file_use_map, old_fqn_normalized, new_short_name);

            // Decide what in-code references should be renamed to.
            // - If the import uses an explicit alias different from the old short
            //   name, in-code refs use the alias and should NOT change.
            // - If there's a collision, we introduce an alias and in-code refs
            //   must use that alias.
            // - Otherwise, in-code refs switch from old short name to new short name.
            let (skip_alias_refs, in_code_replacement) = match &import_info {
                Some(info) if info.alias != old_short_name => {
                    // Explicit alias: in-code refs use the alias, leave them alone.
                    (true, info.alias.clone())
                }
                Some(_) if has_collision => {
                    // Collision: introduce an alias for the renamed import.
                    let alias = pick_collision_alias(new_short_name, &file_use_map);
                    (false, alias)
                }
                _ => {
                    // Normal case: rename in-code refs to the new short name.
                    (false, new_short_name.to_string())
                }
            };

            // When the file has an import for the old class, find the
            // use-statement line range so we can (a) skip the FQN
            // reference that falls inside it (we replace the whole line
            // instead) and (b) generate a proper whole-line edit that
            // can add/remove aliases.
            let use_line_range = if import_info.is_some() {
                find_use_line_range(&file_content, old_fqn_normalized)
            } else {
                None
            };

            let mut file_edits: Vec<TextEdit> = Vec::new();

            for loc in file_locations {
                let start_off =
                    crate::util::position_to_byte_offset(&file_content, loc.range.start);
                let end_off = crate::util::position_to_byte_offset(&file_content, loc.range.end);
                let source_text = file_content
                    .get(start_off..end_off)
                    .unwrap_or("")
                    .to_string();

                // If this reference falls inside the use-statement line,
                // skip it — the whole-line edit below will handle it.
                if let Some(ref ul) = use_line_range
                    && ranges_overlap(loc.range, ul.range)
                {
                    continue;
                }

                if source_text.contains('\\') {
                    // This is an inline FQN reference (e.g. `\Ns\Foo`).
                    // Replace only the last segment.
                    let new_text = if let Some(ns_sep) = source_text.rfind('\\') {
                        format!("{}{}", &source_text[..=ns_sep], new_short_name)
                    } else {
                        new_short_name.to_string()
                    };
                    file_edits.push(TextEdit {
                        range: loc.range,
                        new_text,
                    });
                } else if skip_alias_refs && source_text == import_info.as_ref().unwrap().alias {
                    // This reference uses the alias.  The alias is being
                    // preserved, so skip this edit entirely.
                    continue;
                } else {
                    // Normal in-code reference (short name or declaration).
                    file_edits.push(TextEdit {
                        range: loc.range,
                        new_text: in_code_replacement.clone(),
                    });
                }
            }

            // Generate a whole-line replacement for the `use` statement.
            if let Some(ref info) = import_info
                && let Some(ref ul) = use_line_range
            {
                let new_line =
                    build_use_line(&new_fqn, info, has_collision, new_short_name, &file_use_map);
                file_edits.push(TextEdit {
                    range: ul.range,
                    new_text: new_line,
                });
            }

            if !file_edits.is_empty() {
                changes.entry(parsed_uri).or_default().extend(file_edits);
            }
        }

        if changes.is_empty() {
            return None;
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
            SymbolKind::FunctionCall { name, .. } => Some((name.clone(), range)),
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
        let vendor_prefixes = self.vendor_uri_prefixes.lock().clone();

        if vendor_prefixes.is_empty() {
            return false;
        }

        // Try to resolve the definition location.
        if let Some(loc) = self.resolve_definition(uri, content, position) {
            let def_uri = loc.uri.to_string();
            if vendor_prefixes
                .iter()
                .any(|p| def_uri.starts_with(p.as_str()))
            {
                return true;
            }
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

// ─── Import analysis helpers ────────────────────────────────────────────────

/// The line range of a `use` statement in a file.
struct UseLineRange {
    range: Range,
}

/// Information about how a class is imported in a file.
struct ImportInfo {
    /// The alias (short name) used in code.  For `use Ns\Foo;` this is
    /// `"Foo"`.  For `use Ns\Foo as Bar;` this is `"Bar"`.
    alias: String,
    /// Whether an explicit `as` alias was used.
    has_explicit_alias: bool,
}

/// Look up the import entry for a given FQN in a file's use_map.
///
/// The use_map is `alias → fqn`, so we need a reverse lookup.
fn find_import_for_fqn(use_map: &HashMap<String, String>, target_fqn: &str) -> Option<ImportInfo> {
    let target_normalized = target_fqn.strip_prefix('\\').unwrap_or(target_fqn);
    let target_short = crate::util::short_name(target_normalized);

    for (alias, fqn) in use_map {
        let fqn_normalized = fqn.strip_prefix('\\').unwrap_or(fqn);
        if fqn_normalized.eq_ignore_ascii_case(target_normalized) {
            let has_explicit_alias = !alias.eq_ignore_ascii_case(target_short);
            return Some(ImportInfo {
                alias: alias.clone(),
                has_explicit_alias,
            });
        }
    }
    None
}

/// Check whether importing `new_short_name` would collide with an
/// existing import in the file (other than the one being renamed).
fn has_import_collision(
    use_map: &HashMap<String, String>,
    old_fqn: &str,
    new_short_name: &str,
) -> bool {
    let old_normalized = old_fqn.strip_prefix('\\').unwrap_or(old_fqn);
    let new_lower = new_short_name.to_lowercase();

    for (alias, fqn) in use_map {
        let fqn_normalized = fqn.strip_prefix('\\').unwrap_or(fqn);
        // Skip the entry for the class being renamed.
        if fqn_normalized.eq_ignore_ascii_case(old_normalized) {
            continue;
        }
        if alias.to_lowercase() == new_lower {
            return true;
        }
    }
    false
}

/// Pick an alias name to avoid a collision.
///
/// Tries `"{name}Alias"` first, then `"{name}Alias2"`, etc.
fn pick_collision_alias(base_name: &str, use_map: &HashMap<String, String>) -> String {
    let candidate = format!("{}Alias", base_name);
    if !use_map.contains_key(&candidate) {
        return candidate;
    }
    for i in 2..100 {
        let candidate = format!("{}Alias{}", base_name, i);
        if !use_map.contains_key(&candidate) {
            return candidate;
        }
    }
    // Extremely unlikely fallback.
    format!("{}Alias99", base_name)
}

/// Check whether two LSP ranges overlap.
fn ranges_overlap(a: Range, b: Range) -> bool {
    !(a.end.line < b.start.line
        || (a.end.line == b.start.line && a.end.character <= b.start.character)
        || b.end.line < a.start.line
        || (b.end.line == a.start.line && b.end.character <= a.start.character))
}

/// Find the LSP range of the `use` statement line that imports `old_fqn`.
fn find_use_line_range(content: &str, old_fqn: &str) -> Option<UseLineRange> {
    let old_fqn_normalized = old_fqn.strip_prefix('\\').unwrap_or(old_fqn);

    for (line_idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("use ") {
            continue;
        }

        let rest = trimmed.strip_prefix("use ")?.trim();
        let rest = rest.strip_suffix(';').unwrap_or(rest).trim();

        let (fqn_part, _) = if let Some(as_pos) = rest.find(" as ") {
            (rest[..as_pos].trim(), Some(&rest[as_pos + 4..]))
        } else {
            (rest, None)
        };

        if !fqn_part.eq_ignore_ascii_case(old_fqn_normalized) {
            continue;
        }

        let line_start_byte: usize = content.lines().take(line_idx).map(|l| l.len() + 1).sum();
        let line_end_byte = line_start_byte + line.len();

        let start_pos = offset_to_position(content, line_start_byte);
        let end_pos = offset_to_position(content, line_end_byte);

        return Some(UseLineRange {
            range: Range {
                start: start_pos,
                end: end_pos,
            },
        });
    }

    None
}

/// Build the replacement text for a `use` statement line.
fn build_use_line(
    new_fqn: &str,
    import_info: &ImportInfo,
    has_collision: bool,
    new_short_name: &str,
    use_map: &HashMap<String, String>,
) -> String {
    if has_collision {
        let alias = pick_collision_alias(new_short_name, use_map);
        format!("use {} as {};", new_fqn, alias)
    } else if import_info.has_explicit_alias {
        format!("use {} as {};", new_fqn, import_info.alias)
    } else {
        format!("use {};", new_fqn)
    }
}
