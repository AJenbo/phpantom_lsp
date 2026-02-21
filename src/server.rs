/// LSP server trait implementation.
///
/// This module contains the `impl LanguageServer for Backend` block,
/// which handles all LSP protocol messages (initialize, didOpen, didChange,
/// didClose, completion, etc.).
use std::collections::HashSet;
use std::path::PathBuf;

use tower_lsp::LanguageServer;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::request::{GotoImplementationParams, GotoImplementationResponse};
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::composer;

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
                        "@".to_string(),
                        "'".to_string(),
                        "\"".to_string(),
                        "[".to_string(),
                        " ".to_string(),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
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

            // Determine the vendor directory (needed for autoload files).
            let vendor_dir = {
                let composer_path = root.join("composer.json");
                if let Ok(content) = std::fs::read_to_string(&composer_path) {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                        json.get("config")
                            .and_then(|c| c.get("vendor-dir"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.trim_end_matches('/').to_string())
                            .unwrap_or_else(|| "vendor".to_string())
                    } else {
                        "vendor".to_string()
                    }
                } else {
                    "vendor".to_string()
                }
            };

            if let Ok(mut m) = self.psr4_mappings.lock() {
                *m = mappings;
            }

            // Parse autoload_classmap.php to get direct FQN → file path mappings.
            let classmap = composer::parse_autoload_classmap(&root, &vendor_dir);
            let classmap_count = classmap.len();
            if let Ok(mut cm) = self.classmap.lock() {
                *cm = classmap;
            }

            // Parse autoload_files.php to discover global symbols.
            // These files can contain any kind of PHP symbol (classes,
            // functions, define() constants, etc.).  Classes, traits,
            // interfaces, and enums can also be loaded via PSR-4 / classmap,
            // but functions and define() constants can *only* be discovered
            // through these files.
            //
            // We also follow `require_once` statements in those files to
            // discover additional files (used by packages like Trustly
            // that don't follow Composer conventions).
            let autoload_files = composer::parse_autoload_files(&root, &vendor_dir);
            let autoload_count = autoload_files.len();

            // Work queue + visited set for following require_once chains.
            let mut file_queue: Vec<PathBuf> = autoload_files;
            let mut visited: HashSet<PathBuf> = HashSet::new();

            while let Some(file_path) = file_queue.pop() {
                // Canonicalise to avoid revisiting the same file via
                // different relative paths.
                let canonical = file_path.canonicalize().unwrap_or(file_path);
                if !visited.insert(canonical.clone()) {
                    continue;
                }

                if let Ok(content) = std::fs::read_to_string(&canonical) {
                    let uri = format!("file://{}", canonical.display());

                    // Full AST parse: extracts classes, use statements,
                    // namespaces, standalone functions, and define()
                    // constants — all in a single pass.
                    self.update_ast(&uri, &content);

                    // Follow require_once statements to discover more files.
                    let require_paths = composer::extract_require_once_paths(&content);
                    if let Some(file_dir) = canonical.parent() {
                        for rel_path in require_paths {
                            let resolved = file_dir.join(&rel_path);
                            if resolved.is_file() {
                                file_queue.push(resolved);
                            }
                        }
                    }
                }
            }

            self.log(
                MessageType::INFO,
                format!(
                    "PHPantom initialized! Loaded {} PSR-4 mapping(s), {} classmap entries, {} autoload file(s)",
                    mapping_count, classmap_count, autoload_count
                ),
            )
            .await;
        } else {
            self.log(MessageType::INFO, "PHPantom initialized!".to_string())
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

        self.clear_file_maps(&uri);

        self.log(MessageType::INFO, format!("Closed file: {}", uri))
            .await;
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let content = if let Ok(files) = self.open_files.lock() {
            files.get(&uri).cloned()
        } else {
            None
        };

        if let Some(content) = content {
            // Wrap in catch_unwind so that a stack overflow (e.g. from
            // deep trait/inheritance resolution when the subject is a
            // call expression like `collect($x)->map(`) doesn't crash
            // the LSP server process.
            let uri_owned = uri.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.resolve_definition(&uri_owned, &content, position)
            }));

            match result {
                Ok(Some(location)) => {
                    return Ok(Some(GotoDefinitionResponse::Scalar(location)));
                }
                Ok(None) => {}
                Err(_) => {
                    log::error!(
                        "PHPantom: panic during goto_definition at {}:{}:{}",
                        uri,
                        position.line,
                        position.character
                    );
                }
            }
        }

        Ok(None)
    }

    async fn goto_implementation(
        &self,
        params: GotoImplementationParams,
    ) -> Result<Option<GotoImplementationResponse>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let content = if let Ok(files) = self.open_files.lock() {
            files.get(&uri).cloned()
        } else {
            None
        };

        if let Some(content) = content {
            let uri_owned = uri.clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.resolve_implementation(&uri_owned, &content, position)
            }));

            match result {
                Ok(Some(locations)) if locations.len() == 1 => {
                    return Ok(Some(GotoImplementationResponse::Scalar(
                        locations.into_iter().next().unwrap(),
                    )));
                }
                Ok(Some(locations)) if !locations.is_empty() => {
                    return Ok(Some(GotoImplementationResponse::Array(locations)));
                }
                Ok(_) => {}
                Err(_) => {
                    log::error!(
                        "PHPantom: panic during goto_implementation at {}:{}:{}",
                        uri,
                        position.line,
                        position.character
                    );
                }
            }
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.handle_completion(params).await
    }
}
