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
            offset_encoding: None,
            capabilities: ServerCapabilities {
                signature_help_provider: Some(SignatureHelpOptions {
                    trigger_characters: Some(vec!["(".to_string(), ",".to_string()]),
                    retrigger_characters: Some(vec![",".to_string(), ")".to_string()]),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                }),
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
                        "\\".to_string(),
                    ]),
                    all_commit_characters: None,
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    completion_item: None,
                }),
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                definition_provider: Some(OneOf::Left(true)),
                implementation_provider: Some(ImplementationProviderCapability::Simple(true)),
                references_provider: Some(OneOf::Left(true)),
                document_highlight_provider: Some(OneOf::Left(true)),
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
            // Detect the target PHP version from composer.json before
            // any stub parsing happens, so that version-aware filtering
            // is active from the very first file load.
            let php_version = composer::detect_php_version(&root).unwrap_or_default();
            self.set_php_version(php_version);

            let (mappings, vendor_dir) = composer::parse_composer_json(&root);
            let mapping_count = mappings.len();

            // Cache the vendor dir name so cross-file scans can skip it
            // without re-reading composer.json on every request.
            if let Ok(mut vdn) = self.vendor_dir_name.lock() {
                *vdn = vendor_dir.clone();
            }

            // Store the vendor URI prefix so diagnostics can skip vendor files.
            let vendor_path = root.join(&vendor_dir);
            if let Ok(canonical) = vendor_path.canonicalize() {
                let prefix = format!("file://{}/", canonical.display());
                if let Ok(mut vp) = self.vendor_uri_prefix.lock() {
                    *vp = prefix;
                }
            } else {
                // Vendor dir doesn't exist yet — store the non-canonical path
                // so files opened from that location are still skipped.
                let prefix = format!("file://{}/", vendor_path.display());
                if let Ok(mut vp) = self.vendor_uri_prefix.lock() {
                    *vp = prefix;
                }
            }

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
                    "PHPantom initialized! PHP {}, {} PSR-4 mapping(s), {} classmap entries, {} autoload file(s)",
                    php_version, mapping_count, classmap_count, autoload_count
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

        // Publish diagnostics (deprecated usage, unused imports, etc.)
        self.publish_diagnostics_for_file(&uri, &text).await;

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

            // Re-publish diagnostics after re-parse
            self.publish_diagnostics_for_file(&uri, text).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Ok(mut files) = self.open_files.lock() {
            files.remove(&uri);
        }

        self.clear_file_maps(&uri);

        // Clear diagnostics so stale warnings don't linger after the file is closed
        self.clear_diagnostics_for_file(&uri).await;

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

        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            let result = crate::util::catch_panic_unwind_safe(
                "goto_definition",
                &uri,
                Some(position),
                || self.resolve_definition(&uri, &content, position),
            );

            if let Some(Some(location)) = result {
                return Ok(Some(GotoDefinitionResponse::Scalar(location)));
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

        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            let result = crate::util::catch_panic_unwind_safe(
                "goto_implementation",
                &uri,
                Some(position),
                || self.resolve_implementation(&uri, &content, position),
            );

            if let Some(Some(locations)) = result {
                if locations.len() == 1 {
                    return Ok(Some(GotoImplementationResponse::Scalar(
                        locations.into_iter().next().unwrap(),
                    )));
                }
                if !locations.is_empty() {
                    return Ok(Some(GotoImplementationResponse::Array(locations)));
                }
            }
        }

        Ok(None)
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let content = self.get_file_content(&uri);

        if let Some(content) = content
            && let Some(hover) =
                crate::util::catch_panic_unwind_safe("hover", &uri, Some(position), || {
                    self.handle_hover(&uri, &content, position)
                })
        {
            return Ok(hover);
        }

        Ok(None)
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        self.handle_completion(params).await
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let include_declaration = params.context.include_declaration;

        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            let result =
                crate::util::catch_panic_unwind_safe("references", &uri, Some(position), || {
                    self.find_references(&uri, &content, position, include_declaration)
                });

            if let Some(locations) = result {
                return Ok(locations);
            }
        }

        Ok(None)
    }

    async fn signature_help(&self, params: SignatureHelpParams) -> Result<Option<SignatureHelp>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let content = self.get_file_content(&uri);

        if let Some(content) = content
            && let Some(sig_help) =
                crate::util::catch_panic_unwind_safe("signature_help", &uri, Some(position), || {
                    self.handle_signature_help(&uri, &content, position)
                })
        {
            return Ok(sig_help);
        }

        Ok(None)
    }

    async fn document_highlight(
        &self,
        params: DocumentHighlightParams,
    ) -> Result<Option<Vec<DocumentHighlight>>> {
        let uri = params
            .text_document_position_params
            .text_document
            .uri
            .to_string();
        let position = params.text_document_position_params.position;

        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            let result = crate::util::catch_panic_unwind_safe(
                "document_highlight",
                &uri,
                Some(position),
                || self.handle_document_highlight(&uri, &content, position),
            );

            if let Some(highlights) = result {
                return Ok(highlights);
            }
        }

        Ok(None)
    }
}
