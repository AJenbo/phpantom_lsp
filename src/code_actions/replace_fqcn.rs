//! Replace FQCN with import code action.
//!
//! When the cursor is on a fully-qualified class name (leading `\`), offer
//! a code action that inserts a `use` statement and replaces the FQCN at
//! the call site with the short name.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::use_edit::{analyze_use_block, build_use_edit, use_import_conflicts};
use crate::symbol_map::SymbolKind;
use crate::util::{offset_to_position, position_to_byte_offset, short_name};

impl Backend {
    /// Collect "Replace FQCN with import" code actions when the cursor
    /// is on a fully-qualified class name.
    pub(crate) fn collect_replace_fqcn_actions(
        &self,
        uri: &str,
        content: &str,
        params: &CodeActionParams,
        out: &mut Vec<CodeActionOrCommand>,
    ) {
        let file_use_map: HashMap<String, String> = self.file_use_map(uri);
        let file_namespace: Option<String> = self.first_file_namespace(uri);

        let symbol_map = match self.symbol_maps.read().get(uri) {
            Some(sm) => sm.clone(),
            None => return,
        };

        let request_start = position_to_byte_offset(content, params.range.start);
        let request_end = position_to_byte_offset(content, params.range.end);

        for span in &symbol_map.spans {
            if span.start as usize >= request_end || span.end as usize <= request_start {
                continue;
            }

            let (ref_name, is_fqn) = match &span.kind {
                SymbolKind::ClassReference { name, is_fqn, .. } => (name.as_str(), *is_fqn),
                _ => continue,
            };

            // This action only applies to FQNs (leading `\` in source).
            if !is_fqn {
                continue;
            }

            // The stored name has no leading `\`, so it's already the FQN.
            let fqn = ref_name;
            let sn = short_name(fqn);

            // If the short name is already imported with the same FQN,
            // just offer to replace the inline FQN with the short name
            // (no new use statement needed).
            let already_imported = file_use_map.iter().any(|(alias, existing_fqn)| {
                alias.eq_ignore_ascii_case(sn) && existing_fqn.eq_ignore_ascii_case(fqn)
            });

            // If there's a conflict (different class with same short name
            // already imported), skip.
            if !already_imported && use_import_conflicts(fqn, &file_use_map) {
                continue;
            }

            let doc_uri: Url = match uri.parse() {
                Ok(u) => u,
                Err(_) => continue,
            };

            let mut edits: Vec<TextEdit> = Vec::new();

            // Add use statement if not already imported.
            if !already_imported {
                let use_block = analyze_use_block(content);
                if let Some(use_edits) = build_use_edit(fqn, &use_block, &file_namespace) {
                    edits.extend(use_edits);
                }
            }

            // Replace the FQN at the call site with the short name.
            // The span covers the name without the leading `\`, but in
            // source the leading `\` is present, so we need to include it.
            let replace_start = if span.start > 0 {
                // Check if the character before the span is `\`.
                let before = &content[..span.start as usize];
                if before.ends_with('\\') {
                    span.start as usize - 1
                } else {
                    span.start as usize
                }
            } else {
                span.start as usize
            };
            let replace_end = span.end as usize;

            let start_pos = offset_to_position(content, replace_start);
            let end_pos = offset_to_position(content, replace_end);

            edits.push(TextEdit {
                range: Range {
                    start: start_pos,
                    end: end_pos,
                },
                new_text: sn.to_string(),
            });

            let title = if already_imported {
                format!("Replace `\\{}` with short name `{}`", fqn, sn)
            } else {
                format!("Replace FQCN `\\{}` with import", fqn)
            };

            let mut changes = HashMap::new();
            changes.insert(doc_uri, edits);

            out.push(CodeActionOrCommand::CodeAction(CodeAction {
                title,
                kind: Some(CodeActionKind::REFACTOR),
                diagnostics: None,
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(false),
                disabled: None,
                data: None,
            }));

            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::*;

    fn code_action_titles(content: &str, cursor_offset: usize) -> Vec<String> {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        backend.update_ast(uri, content);

        let pos = crate::util::offset_to_position(content, cursor_offset);
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: pos,
                end: pos,
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };
        let mut actions = Vec::new();
        backend.collect_replace_fqcn_actions(uri, content, &params, &mut actions);
        actions
            .into_iter()
            .map(|a| match a {
                CodeActionOrCommand::CodeAction(ca) => ca.title,
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn offers_action_on_fqcn() {
        let src = "<?php\nnamespace App;\n\n\\Illuminate\\Support\\Str::plural('test');\n";
        // Cursor somewhere on the FQN (after the `\`)
        let offset = src.find("Illuminate\\Support\\Str").unwrap();
        let titles = code_action_titles(src, offset);
        assert_eq!(titles.len(), 1);
        assert!(titles[0].contains("Replace FQCN"));
        assert!(titles[0].contains("Illuminate\\Support\\Str"));
    }

    #[test]
    fn no_action_on_short_name() {
        let src =
            "<?php\nnamespace App;\n\nuse Illuminate\\Support\\Str;\n\nStr::plural('test');\n";
        let offset = src.find("Str::").unwrap();
        let titles = code_action_titles(src, offset);
        assert!(titles.is_empty());
    }

    #[test]
    fn reuses_existing_import() {
        let src = "<?php\nnamespace App;\n\nuse Illuminate\\Support\\Str;\n\n\\Illuminate\\Support\\Str::plural('test');\n";
        let offset = src.find("\\Illuminate\\Support\\Str::").unwrap() + 1;
        let titles = code_action_titles(src, offset);
        assert_eq!(titles.len(), 1);
        assert!(titles[0].contains("short name"));
    }

    #[test]
    fn skips_conflicting_import() {
        let src = "<?php\nnamespace App;\n\nuse Other\\Str;\n\n\\Illuminate\\Support\\Str::plural('test');\n";
        let offset = src.find("\\Illuminate\\Support\\Str::").unwrap() + 1;
        let titles = code_action_titles(src, offset);
        assert!(titles.is_empty());
    }
}
