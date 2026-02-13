/// LSP server trait implementation.
///
/// This module contains the `impl LanguageServer for Backend` block,
/// which handles all LSP protocol messages (initialize, didOpen, didChange,
/// didClose, completion, etc.).
use std::collections::HashMap;

use tower_lsp::LanguageServer;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;
use crate::types::AccessKind;

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract and store the workspace root path
        let workspace_root = params
            .root_uri
            .as_ref()
            .and_then(|uri| uri.to_file_path().ok());

        if let Some(root) = workspace_root
            && let Ok(mut wr) = self.workspace_root.lock()
        {
            *wr = Some(root);
        }

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                completion_provider: Some(CompletionOptions {
                    resolve_provider: Some(false),
                    trigger_characters: Some(vec![
                        "$".to_string(),
                        ">".to_string(),
                        ":".to_string(),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: self.name.clone(),
                version: Some(self.version.clone()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        // Parse composer.json for PSR-4 mappings if we have a workspace root
        let workspace_root = self
            .workspace_root
            .lock()
            .ok()
            .and_then(|guard| guard.clone());

        if let Some(root) = workspace_root {
            let mappings = composer::parse_composer_json(&root);
            let mapping_count = mappings.len();

            if let Ok(mut m) = self.psr4_mappings.lock() {
                *m = mappings;
            }

            self.log(
                MessageType::INFO,
                format!(
                    "PHPantomLSP initialized! Loaded {} PSR-4 mapping(s)",
                    mapping_count
                ),
            )
            .await;
        } else {
            self.log(MessageType::INFO, "PHPantomLSP initialized!".to_string())
                .await;
        }
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let uri = doc.uri.to_string();
        let text = doc.text;

        // Store file content
        if let Ok(mut files) = self.open_files.lock() {
            files.insert(uri.clone(), text.clone());
        }

        // Parse and update AST map, use map, and namespace map
        self.update_ast(&uri, &text);

        self.log(MessageType::INFO, format!("Opened file: {}", uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Some(change) = params.content_changes.first() {
            let text = &change.text;

            // Update stored content
            if let Ok(mut files) = self.open_files.lock() {
                files.insert(uri.clone(), text.clone());
            }

            // Re-parse and update AST map, use map, and namespace map
            self.update_ast(&uri, text);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Ok(mut files) = self.open_files.lock() {
            files.remove(&uri);
        }

        if let Ok(mut map) = self.ast_map.lock() {
            map.remove(&uri);
        }

        if let Ok(mut map) = self.use_map.lock() {
            map.remove(&uri);
        }

        if let Ok(mut map) = self.namespace_map.lock() {
            map.remove(&uri);
        }

        self.log(MessageType::INFO, format!("Closed file: {}", uri))
            .await;
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;

        // Get file content for offset calculation
        let content = if let Ok(files) = self.open_files.lock() {
            files.get(&uri).cloned()
        } else {
            None
        };

        // Get classes from ast_map for the current file
        let classes = if let Ok(map) = self.ast_map.lock() {
            map.get(&uri).cloned()
        } else {
            None
        };

        if let (Some(content), Some(classes)) = (content, classes) {
            // Try to extract a completion target (requires `->` or `::`)
            if let Some(target) = Self::extract_completion_target(&content, position) {
                let cursor_offset = Self::position_to_offset(&content, position);
                let current_class =
                    cursor_offset.and_then(|off| Self::find_class_at_offset(&classes, off));

                // Gather the current file's `use` statement mappings and namespace
                // so the class_loader can resolve short names like `Resource` to
                // their fully-qualified equivalents like `Klarna\Rest\Resource`.
                let file_use_map: HashMap<String, String> = if let Ok(map) = self.use_map.lock() {
                    map.get(&uri).cloned().unwrap_or_default()
                } else {
                    HashMap::new()
                };

                let file_namespace: Option<String> = if let Ok(map) = self.namespace_map.lock() {
                    map.get(&uri).cloned().flatten()
                } else {
                    None
                };

                // Build the class_loader closure that provides cross-file /
                // PSR-4 resolution.  This captures `&self`, the current file's
                // use-statement mappings, and the current namespace so it can:
                //   1. Resolve short names via `use` statements
                //   2. Try the current namespace as a prefix
                //   3. Search the full ast_map
                //   4. Load files on demand via PSR-4
                let class_loader = |name: &str| -> Option<crate::ClassInfo> {
                    // If the name has no namespace separator, it might be a
                    // short name imported via `use`.  Resolve it first.
                    let resolved_name = if !name.contains('\\') {
                        if let Some(fqn) = file_use_map.get(name) {
                            fqn.as_str()
                        } else if let Some(ref ns) = file_namespace {
                            // Not in use map — try current namespace
                            // (e.g. bare `Sibling` inside `namespace Foo\Bar;`
                            //  becomes `Foo\Bar\Sibling`)
                            // We build a temporary owned string and leak it into
                            // a short-lived search, so use a two-phase approach:
                            // first try the namespace-qualified name, then fall
                            // back to the bare name.
                            let ns_qualified = format!("{}\\{}", ns, name);
                            if let Some(cls) = self.find_or_load_class(&ns_qualified) {
                                return Some(cls);
                            }
                            name
                        } else {
                            name
                        }
                    } else {
                        name
                    };

                    self.find_or_load_class(resolved_name)
                };

                let resolved = Self::resolve_target_class(
                    &target.subject,
                    target.access_kind,
                    current_class,
                    &classes,
                    &content,
                    cursor_offset.unwrap_or(0),
                    &class_loader,
                );

                if let Some(target_class) = resolved {
                    let merged = Self::resolve_class_with_inheritance(&target_class, &class_loader);
                    // `parent::` is syntactically `::` but semantically
                    // different: it shows both static and instance members
                    // while excluding private ones.
                    let effective_access = if target.subject == "parent" {
                        AccessKind::ParentDoubleColon
                    } else {
                        target.access_kind
                    };
                    let items = Self::build_completion_items(&merged, effective_access);
                    if !items.is_empty() {
                        return Ok(Some(CompletionResponse::Array(items)));
                    }
                }
            }
        }

        // Fallback: return the default PHPantomLSP completion item
        Ok(Some(CompletionResponse::Array(vec![CompletionItem {
            label: "PHPantomLSP".to_string(),
            kind: Some(CompletionItemKind::TEXT),
            detail: Some("PHPantomLSP completion".to_string()),
            insert_text: Some("PHPantomLSP".to_string()),
            ..CompletionItem::default()
        }])))
    }
}
