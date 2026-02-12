use tower_lsp::LanguageServer;
/// LSP server trait implementation.
///
/// This module contains the `impl LanguageServer for Backend` block,
/// which handles all LSP protocol messages (initialize, didOpen, didChange,
/// didClose, completion, etc.).
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::Backend;

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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
        self.log(MessageType::INFO, "PHPantomLSP initialized!".to_string())
            .await;
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

        // Parse and update AST map
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

            // Re-parse and update AST map
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

        // Get classes from ast_map
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

                let resolved = Self::resolve_target_class(
                    &target.subject,
                    target.access_kind,
                    current_class,
                    &classes,
                    &content,
                    cursor_offset.unwrap_or(0),
                );

                if let Some(target_class) = resolved {
                    let items = Self::build_completion_items(target_class, target.access_kind);
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
