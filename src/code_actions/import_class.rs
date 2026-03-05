//! Import class code action.
//!
//! When the cursor is on an unresolved class name (a `ClassReference` in
//! the symbol map that cannot be resolved via use-map, namespace, or
//! local classes), offer code actions to add a `use` statement for each
//! matching class found in the class index, classmap, and stubs.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::use_edit::{analyze_use_block, build_use_edit, use_import_conflicts};
use crate::diagnostics::unknown_classes::UNKNOWN_CLASS_CODE;

use crate::symbol_map::SymbolKind;
use crate::util::short_name;

impl Backend {
    /// Collect "Import class" code actions for the cursor position.
    ///
    /// For each unresolved `ClassReference` that overlaps with the
    /// request range, search the class index, classmap, and stubs for
    /// classes whose short name matches, and offer a code action per
    /// candidate.
    pub(crate) fn collect_import_class_actions(
        &self,
        uri: &str,
        content: &str,
        params: &CodeActionParams,
        out: &mut Vec<CodeActionOrCommand>,
    ) {
        // ── Gather file context ─────────────────────────────────────────
        let file_use_map: HashMap<String, String> = match self.use_map.lock() {
            Ok(m) => m.get(uri).cloned().unwrap_or_default(),
            Err(_) => return,
        };

        let file_namespace: Option<String> = self
            .namespace_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .flatten();

        let symbol_map = match self.symbol_maps.lock() {
            Ok(m) => match m.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            },
            Err(_) => return,
        };

        let local_classes = self
            .ast_map
            .lock()
            .ok()
            .and_then(|m| m.get(uri).cloned())
            .unwrap_or_default();

        // Convert LSP range to byte offsets for comparison with symbol spans.
        let request_start = position_to_offset(content, params.range.start);
        let request_end = position_to_offset(content, params.range.end);
        let (request_start, request_end) = match (request_start, request_end) {
            (Some(s), Some(e)) => (s, e),
            _ => return,
        };

        // ── Find ClassReference spans overlapping the request range ─────
        for span in &symbol_map.spans {
            // Check overlap: span overlaps the request range if
            // span.start < request_end && span.end > request_start
            if span.start as usize >= request_end || span.end as usize <= request_start {
                continue;
            }

            let (ref_name, is_fqn) = match &span.kind {
                SymbolKind::ClassReference { name, is_fqn } => (name.as_str(), *is_fqn),
                _ => continue,
            };

            // Skip already-qualified names — they don't need importing.
            if is_fqn || ref_name.contains('\\') {
                continue;
            }

            // Skip if the name is already imported via use-map.
            if file_use_map.contains_key(ref_name) {
                continue;
            }

            // Skip if it resolves as a local class (same file).
            if local_classes.iter().any(|c| c.name == ref_name) {
                continue;
            }

            // Skip if it resolves via same-namespace lookup.
            if let Some(ns) = &file_namespace {
                let ns_qualified = format!("{}\\{}", ns, ref_name);
                if self.find_or_load_class(&ns_qualified).is_some() {
                    continue;
                }
            }

            // Skip if the unqualified name resolves in global scope
            // (and the file has no namespace, so no import needed).
            if file_namespace.is_none() && self.find_or_load_class(ref_name).is_some() {
                continue;
            }

            // ── Name is unresolved — find import candidates ─────────────
            let candidates = self.find_import_candidates(ref_name);

            if candidates.is_empty() {
                continue;
            }

            let use_block = analyze_use_block(content);
            let doc_uri: Url = match uri.parse() {
                Ok(u) => u,
                Err(_) => continue,
            };

            // Find any unknown_class diagnostics from the request context
            // that overlap this span so we can attach them to the code
            // action.  This lets editors show the import action as a
            // quick-fix for the diagnostic.
            let matching_diagnostics: Vec<Diagnostic> = params
                .context
                .diagnostics
                .iter()
                .filter(|d| {
                    matches!(
                        &d.code,
                        Some(NumberOrString::String(code)) if code == UNKNOWN_CLASS_CODE
                    )
                })
                .cloned()
                .collect();

            for fqn in &candidates {
                // Skip candidates that would conflict with an existing
                // import (e.g. a different class with the same short name
                // is already imported).
                if use_import_conflicts(fqn, &file_use_map) {
                    continue;
                }

                let edits = match build_use_edit(fqn, &use_block, &file_namespace) {
                    Some(e) => e,
                    // No edit needed (global class, no namespace) — skip.
                    None => continue,
                };

                let title = format!("Import `{}`", fqn);

                let mut changes = HashMap::new();
                changes.insert(doc_uri.clone(), edits);

                out.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title,
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: if matching_diagnostics.is_empty() {
                        None
                    } else {
                        Some(matching_diagnostics.clone())
                    },
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    command: None,
                    is_preferred: if candidates.len() == 1 {
                        Some(true)
                    } else {
                        None
                    },
                    disabled: None,
                    data: None,
                }));
            }

            // Only process the first unresolved reference at the cursor.
            // Multiple overlapping references at the exact same position
            // are unlikely, and processing one keeps the action list tidy.
            break;
        }

        // ── Also check MemberAccess spans for unresolved static subjects ─
        // e.g. `Foo::bar()` where `Foo` is not imported — the symbol map
        // records this as a MemberAccess with subject_text "Foo", not a
        // ClassReference.  We handle this by looking for static member
        // accesses whose subject is an unresolved short name.
        self.collect_import_from_static_access(
            uri,
            content,
            params,
            request_start,
            request_end,
            &file_use_map,
            &file_namespace,
            &local_classes,
            &symbol_map,
            out,
        );
    }

    /// Check static member access subjects for unresolved class names.
    #[allow(clippy::too_many_arguments)]
    fn collect_import_from_static_access(
        &self,
        uri: &str,
        content: &str,
        _params: &CodeActionParams,
        request_start: usize,
        request_end: usize,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
        local_classes: &[crate::types::ClassInfo],
        symbol_map: &crate::symbol_map::SymbolMap,
        out: &mut Vec<CodeActionOrCommand>,
    ) {
        for span in &symbol_map.spans {
            if span.start as usize >= request_end || span.end as usize <= request_start {
                continue;
            }

            let subject = match &span.kind {
                SymbolKind::MemberAccess {
                    subject_text,
                    is_static: true,
                    ..
                } => subject_text.as_str(),
                _ => continue,
            };

            // Only handle simple unqualified names (not $this, self, parent, etc.)
            if subject.starts_with('$')
                || subject.contains('\\')
                || subject.eq_ignore_ascii_case("self")
                || subject.eq_ignore_ascii_case("static")
                || subject.eq_ignore_ascii_case("parent")
            {
                continue;
            }

            // Already imported?
            if file_use_map.contains_key(subject) {
                continue;
            }

            // Local class?
            if local_classes.iter().any(|c| c.name == subject) {
                continue;
            }

            // Resolves via namespace?
            if let Some(ns) = file_namespace {
                let ns_qualified = format!("{}\\{}", ns, subject);
                if self.find_or_load_class(&ns_qualified).is_some() {
                    continue;
                }
            }

            if file_namespace.is_none() && self.find_or_load_class(subject).is_some() {
                continue;
            }

            let candidates = self.find_import_candidates(subject);
            if candidates.is_empty() {
                continue;
            }

            // The span covers the whole `Foo::bar` expression. We only
            // want the subject part for the diagnostic range, but for
            // the code action the span range is fine.
            let use_block = analyze_use_block(content);
            let doc_uri: Url = match uri.parse() {
                Ok(u) => u,
                Err(_) => continue,
            };

            for fqn in &candidates {
                if use_import_conflicts(fqn, file_use_map) {
                    continue;
                }

                let edits = match build_use_edit(fqn, &use_block, file_namespace) {
                    Some(e) => e,
                    None => continue,
                };

                let title = format!("Import `{}`", fqn);

                let mut changes = HashMap::new();
                changes.insert(doc_uri.clone(), edits);

                out.push(CodeActionOrCommand::CodeAction(CodeAction {
                    title,
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: None,
                    edit: Some(WorkspaceEdit {
                        changes: Some(changes),
                        document_changes: None,
                        change_annotations: None,
                    }),
                    command: None,
                    is_preferred: if candidates.len() == 1 {
                        Some(true)
                    } else {
                        None
                    },
                    disabled: None,
                    data: None,
                }));
            }

            break;
        }
    }

    /// Search all known class sources for classes whose short name matches
    /// `name` (case-insensitive).
    ///
    /// Returns a deduplicated, sorted list of fully-qualified class names.
    fn find_import_candidates(&self, name: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        let name_lower = name.to_lowercase();

        // ── 1. class_index ──────────────────────────────────────────────
        if let Ok(idx) = self.class_index.lock() {
            for fqn in idx.keys() {
                if short_name(fqn).to_lowercase() == name_lower {
                    candidates.push(fqn.clone());
                }
            }
        }

        // ── 2. Composer classmap ────────────────────────────────────────
        if let Ok(cmap) = self.classmap.lock() {
            for fqn in cmap.keys() {
                if short_name(fqn).to_lowercase() == name_lower
                    && !candidates
                        .iter()
                        .any(|c: &String| c.eq_ignore_ascii_case(fqn))
                {
                    candidates.push(fqn.clone());
                }
            }
        }

        // ── 3. ast_map (already-parsed files) ───────────────────────────
        if let Ok(amap) = self.ast_map.lock()
            && let Ok(nmap) = self.namespace_map.lock()
        {
            for (file_uri, classes) in amap.iter() {
                let ns = nmap.get(file_uri).and_then(|o| o.as_deref());
                for cls in classes {
                    if cls.name.to_lowercase() == name_lower {
                        let fqn = match ns {
                            Some(ns) => format!("{}\\{}", ns, cls.name),
                            None => cls.name.clone(),
                        };
                        if !candidates
                            .iter()
                            .any(|c: &String| c.eq_ignore_ascii_case(&fqn))
                        {
                            candidates.push(fqn);
                        }
                    }
                }
            }
        }

        // ── 4. Stubs (built-in PHP classes) ─────────────────────────────
        // Stubs are global-namespace classes, so the FQN is the short name.
        // Only add if the file has a namespace (otherwise no import needed).
        for &stub_name in self.stub_index.keys() {
            if short_name(stub_name).to_lowercase() == name_lower
                && !candidates
                    .iter()
                    .any(|c: &String| c.eq_ignore_ascii_case(stub_name))
            {
                candidates.push(stub_name.to_string());
            }
        }

        candidates.sort();
        candidates.dedup();
        candidates
    }
}

/// Convert an LSP `Position` (0-based line/character) to a byte offset
/// in `content`.
///
/// Returns `None` if the position is beyond the end of the content.
fn position_to_offset(content: &str, position: Position) -> Option<usize> {
    let mut line: u32 = 0;
    let mut col: u32 = 0;
    for (i, ch) in content.char_indices() {
        if line == position.line && col == position.character {
            return Some(i);
        }
        if ch == '\n' {
            if line == position.line {
                // Position is past the end of this line — clamp to newline.
                return Some(i);
            }
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    // Position at end of content.
    if line == position.line && col == position.character {
        Some(content.len())
    } else if line == position.line {
        // Character past end of last line — clamp to end.
        Some(content.len())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── position_to_offset tests ────────────────────────────────────────

    #[test]
    fn position_to_offset_start() {
        let content = "hello\nworld\n";
        assert_eq!(position_to_offset(content, Position::new(0, 0)), Some(0));
    }

    #[test]
    fn position_to_offset_mid_line() {
        let content = "hello\nworld\n";
        assert_eq!(position_to_offset(content, Position::new(0, 3)), Some(3));
    }

    #[test]
    fn position_to_offset_second_line() {
        let content = "hello\nworld\n";
        assert_eq!(position_to_offset(content, Position::new(1, 0)), Some(6));
    }

    #[test]
    fn position_to_offset_end_of_content() {
        let content = "hi";
        assert_eq!(position_to_offset(content, Position::new(0, 2)), Some(2));
    }

    // ── find_import_candidates smoke test ───────────────────────────────

    #[test]
    fn find_candidates_from_classmap() {
        let backend = crate::Backend::new_test();
        // Populate classmap with a known class.
        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert(
                "App\\Models\\User".to_string(),
                "/fake/path/User.php".into(),
            );
            cmap.insert(
                "App\\Http\\Request".to_string(),
                "/fake/path/Request.php".into(),
            );
        }

        let candidates = backend.find_import_candidates("User");
        assert!(candidates.contains(&"App\\Models\\User".to_string()));
        assert!(!candidates.contains(&"App\\Http\\Request".to_string()));
    }

    #[test]
    fn find_candidates_case_insensitive() {
        let backend = crate::Backend::new_test();
        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert(
                "Vendor\\Obscure\\ZYGOMORPHIC".to_string(),
                "/fake/path.php".into(),
            );
        }

        let candidates = backend.find_import_candidates("Zygomorphic");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0], "Vendor\\Obscure\\ZYGOMORPHIC");
    }

    #[test]
    fn find_candidates_deduplicates() {
        let backend = crate::Backend::new_test();
        // Add the same FQN to both class_index and classmap.
        if let Ok(mut idx) = backend.class_index.lock() {
            idx.insert("App\\Foo".to_string(), "file:///foo.php".to_string());
        }
        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert("App\\Foo".to_string(), "/foo.php".into());
        }

        let candidates = backend.find_import_candidates("Foo");
        let count = candidates.iter().filter(|c| *c == "App\\Foo").count();
        assert_eq!(count, 1, "should not have duplicates");
    }

    // ── Integration-style test with code action collection ──────────────

    #[test]
    fn import_action_offered_for_unresolved_class() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew Request();\n";

        // Parse the file so the symbol map is populated.
        backend.update_ast(uri, content);

        // Add a candidate to the classmap.
        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert(
                "Illuminate\\Http\\Request".to_string(),
                "/vendor/laravel/framework/src/Illuminate/Http/Request.php".into(),
            );
        }

        // Build a request range covering "Request" on line 3.
        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(3, 4),
                end: Position::new(3, 11),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = backend.handle_code_action(uri, content, &params);
        assert!(
            actions.iter().any(|a| {
                if let CodeActionOrCommand::CodeAction(ca) = a {
                    ca.title.contains("Illuminate\\Http\\Request")
                } else {
                    false
                }
            }),
            "expected an import action for Illuminate\\Http\\Request, got: {:?}",
            actions
                .iter()
                .map(|a| match a {
                    CodeActionOrCommand::CodeAction(ca) => ca.title.clone(),
                    CodeActionOrCommand::Command(c) => c.title.clone(),
                })
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn no_import_action_when_already_imported() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nuse Illuminate\\Http\\Request;\n\nnew Request();\n";

        backend.update_ast(uri, content);

        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert(
                "Illuminate\\Http\\Request".to_string(),
                "/vendor/laravel/framework/src/Illuminate/Http/Request.php".into(),
            );
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(5, 4),
                end: Position::new(5, 11),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = backend.handle_code_action(uri, content, &params);
        // No import actions should be offered — Request is already imported.
        let import_actions: Vec<_> = actions
            .iter()
            .filter(|a| match a {
                CodeActionOrCommand::CodeAction(ca) => ca.title.starts_with("Import"),
                _ => false,
            })
            .collect();
        assert!(
            import_actions.is_empty(),
            "should not offer import when already imported, got: {:?}",
            import_actions
        );
    }

    #[test]
    fn no_import_action_for_fqn_reference() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew \\Illuminate\\Http\\Request();\n";

        backend.update_ast(uri, content);

        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert(
                "Illuminate\\Http\\Request".to_string(),
                "/vendor/laravel/framework/src/Illuminate/Http/Request.php".into(),
            );
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(3, 5),
                end: Position::new(3, 35),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let import_actions: Vec<_> = actions
            .iter()
            .filter(|a| match a {
                CodeActionOrCommand::CodeAction(ca) => ca.title.starts_with("Import"),
                _ => false,
            })
            .collect();
        assert!(
            import_actions.is_empty(),
            "should not offer import for FQN reference"
        );
    }

    #[test]
    fn import_action_inserts_use_statement() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        let content = "<?php\nnamespace App;\n\nnew Request();\n";

        backend.update_ast(uri, content);

        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert(
                "Illuminate\\Http\\Request".to_string(),
                "/vendor/laravel/framework/src/Illuminate/Http/Request.php".into(),
            );
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(3, 4),
                end: Position::new(3, 11),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = backend.handle_code_action(uri, content, &params);
        let action = actions
            .iter()
            .find_map(|a| match a {
                CodeActionOrCommand::CodeAction(ca)
                    if ca.title.contains("Illuminate\\Http\\Request") =>
                {
                    Some(ca)
                }
                _ => None,
            })
            .expect("expected import action");

        // Verify the edit inserts a use statement.
        let edit = action.edit.as_ref().expect("expected workspace edit");
        let changes = edit.changes.as_ref().expect("expected changes");
        let file_edits = changes
            .get(&uri.parse::<Url>().unwrap())
            .expect("expected edits for the file");
        assert_eq!(file_edits.len(), 1);
        assert_eq!(file_edits[0].new_text, "use Illuminate\\Http\\Request;\n");
    }

    #[test]
    fn import_skips_conflict_with_existing_import() {
        let backend = crate::Backend::new_test();
        let uri = "file:///test.php";
        // Already importing a *different* Request class.
        let content = "<?php\nnamespace App;\n\nuse Symfony\\Component\\HttpFoundation\\Request;\n\nnew Request();\n";

        backend.update_ast(uri, content);

        if let Ok(mut cmap) = backend.classmap.lock() {
            cmap.insert(
                "Illuminate\\Http\\Request".to_string(),
                "/vendor/laravel/framework/src/Illuminate/Http/Request.php".into(),
            );
        }

        let params = CodeActionParams {
            text_document: TextDocumentIdentifier {
                uri: uri.parse().unwrap(),
            },
            range: Range {
                start: Position::new(5, 4),
                end: Position::new(5, 11),
            },
            context: CodeActionContext {
                diagnostics: vec![],
                only: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        let actions = backend.handle_code_action(uri, content, &params);
        // Should not offer importing Illuminate\Http\Request because
        // Symfony's Request is already imported with the same short name.
        let import_actions: Vec<_> = actions
            .iter()
            .filter(|a| match a {
                CodeActionOrCommand::CodeAction(ca) => {
                    ca.title.contains("Illuminate\\Http\\Request")
                }
                _ => false,
            })
            .collect();
        assert!(
            import_actions.is_empty(),
            "should not offer conflicting import"
        );
    }
}
