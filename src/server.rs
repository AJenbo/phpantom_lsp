/// LSP server trait implementation.
///
/// This module contains the `impl LanguageServer for Backend` block,
/// which handles all LSP protocol messages (initialize, didOpen, didChange,
/// didClose, completion, etc.).
///
/// **Diagnostic debouncing.** `did_open` publishes diagnostics immediately
/// (the user just opened the file, they want to see issues right away).
/// `did_change` debounces: each keystroke bumps a per-file version counter
/// and sleeps for 200 ms.  If another edit arrives before the timer fires,
/// the version counter won't match and the stale handler skips publishing.
/// tower-lsp runs each notification handler as an independent async task,
/// so the sleep only blocks that handler, not the server.
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;

use tower_lsp::LanguageServer;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::request::{GotoImplementationParams, GotoImplementationResponse};
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::classmap_scanner;
use crate::composer;
use crate::config::IndexingStrategy;

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        // Extract and store the workspace root path
        let workspace_root = params
            .root_uri
            .as_ref()
            .and_then(|uri| uri.to_file_path().ok());

        if let Some(root) = workspace_root {
            *self.workspace_root.write() = Some(root);
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
                code_action_provider: Some(CodeActionProviderCapability::Options(
                    CodeActionOptions {
                        code_action_kinds: Some(vec![
                            CodeActionKind::QUICKFIX,
                            CodeActionKind::new("source.organizeImports"),
                        ]),
                        work_done_progress_options: WorkDoneProgressOptions {
                            work_done_progress: None,
                        },
                        resolve_provider: None,
                    },
                )),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                })),
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
        let workspace_root = self.workspace_root.read().clone();

        if let Some(root) = workspace_root {
            // ── Load project configuration ──────────────────────────────
            // Read `.phpantom.toml` before anything else so that settings
            // (e.g. PHP version override, diagnostic toggles) are active
            // from the very first file load.
            match crate::config::load_config(&root) {
                Ok(cfg) => {
                    *self.config.lock() = cfg;
                }
                Err(e) => {
                    self.log(
                        MessageType::WARNING,
                        format!("Failed to load .phpantom.toml: {}", e),
                    )
                    .await;
                }
            }

            // Detect the target PHP version.  The config file override
            // takes precedence; otherwise fall back to composer.json.
            let php_version = self
                .config()
                .php
                .version
                .as_deref()
                .and_then(crate::types::PhpVersion::from_composer_constraint)
                .unwrap_or_else(|| composer::detect_php_version(&root).unwrap_or_default());
            self.set_php_version(php_version);

            let (mappings, vendor_dir) = composer::parse_composer_json(&root);
            let mapping_count = mappings.len();

            // Cache the vendor dir name so cross-file scans can skip it
            // without re-reading composer.json on every request.
            *self.vendor_dir_name.lock() = vendor_dir.clone();

            // Store the vendor URI prefix so diagnostics can skip vendor files.
            let vendor_path = root.join(&vendor_dir);
            if let Ok(canonical) = vendor_path.canonicalize() {
                let prefix = format!("file://{}/", canonical.display());
                *self.vendor_uri_prefix.lock() = prefix;
            } else {
                // Vendor dir doesn't exist yet — store the non-canonical path
                // so files opened from that location are still skipped.
                let prefix = format!("file://{}/", vendor_path.display());
                *self.vendor_uri_prefix.lock() = prefix;
            }

            *self.psr4_mappings.write() = mappings;

            // ── Build the classmap ──────────────────────────────────────
            //
            // The classmap is a HashMap<String, PathBuf> mapping FQNs to
            // file paths.  Depending on the indexing strategy, we either
            // use Composer's generated classmap, build one ourselves, or
            // both (Composer classmap + self-scan fallback).
            let strategy = self.config().indexing.strategy;
            let has_composer_json = root.join("composer.json").is_file();

            let (classmap, classmap_source) = match strategy {
                IndexingStrategy::None => {
                    // Use only Composer's classmap, no self-scan fallback.
                    let cm = composer::parse_autoload_classmap(&root, &vendor_dir);
                    let source = if cm.is_empty() {
                        "none"
                    } else {
                        "composer classmap"
                    };
                    (cm, source)
                }
                IndexingStrategy::SelfScan | IndexingStrategy::Full => {
                    // Always self-scan, ignore Composer's classmap.
                    let cm = self.build_self_classmap(&root, &vendor_dir, has_composer_json);
                    (cm, "self-scan")
                }
                IndexingStrategy::Composer => {
                    // Default: try Composer's classmap first, fall back to
                    // self-scan when it is missing or incomplete.
                    let composer_cm = composer::parse_autoload_classmap(&root, &vendor_dir);
                    if !composer_cm.is_empty() {
                        // Composer classmap exists and has entries — check
                        // if it covers the project's own PSR-4 namespaces.
                        let incomplete = self.is_classmap_incomplete(&root, &composer_cm);
                        if incomplete {
                            // Merge: keep Composer's entries (they cover
                            // vendor code) and self-scan user source dirs
                            // to fill in the gaps.
                            let mut cm = composer_cm;
                            let self_cm =
                                self.build_self_classmap(&root, &vendor_dir, has_composer_json);
                            for (fqcn, path) in self_cm {
                                cm.entry(fqcn).or_insert(path);
                            }
                            (cm, "composer classmap + self-scan")
                        } else {
                            (composer_cm, "composer classmap")
                        }
                    } else if has_composer_json {
                        // Classmap file is missing — self-scan.
                        self.log(
                            MessageType::INFO,
                            "PHPantom: No Composer classmap found. Building class index. Run `composer dump-autoload -o` for faster startup.".to_string(),
                        ).await;
                        let cm = self.build_self_classmap(&root, &vendor_dir, has_composer_json);
                        (cm, "self-scan")
                    } else {
                        // No composer.json at all — workspace fallback.
                        self.log(
                            MessageType::INFO,
                            "PHPantom: No composer.json found. Scanning workspace for PHP classes."
                                .to_string(),
                        )
                        .await;
                        let cm = self.build_self_classmap(&root, &vendor_dir, has_composer_json);
                        (cm, "self-scan (workspace)")
                    }
                }
            };

            let classmap_count = classmap.len();
            *self.classmap.write() = classmap;

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
                    "PHPantom initialized! PHP {}, {} PSR-4 mapping(s), {} classmap entries ({}), {} autoload file(s)",
                    php_version, mapping_count, classmap_count, classmap_source, autoload_count
                ),
            )
            .await;
        } else {
            self.log(MessageType::INFO, "PHPantom initialized!".to_string())
                .await;
        }

        // Spawn the background diagnostic worker. We build a shallow
        // clone of `self` that shares every `Arc`-wrapped field (maps,
        // caches, the diagnostic notify/pending slot) so the worker
        // sees all mutations the real Backend makes.  Non-Arc fields
        // (php_version, vendor_uri_prefix, vendor_dir_name) are
        // snapshotted — they are only written during init (above) and
        // never change afterwards.
        let worker_backend = self.clone_for_diagnostic_worker();
        tokio::spawn(async move {
            worker_backend.diagnostic_worker().await;
        });
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        let uri = doc.uri.to_string();
        let text = doc.text;

        // Store file content
        self.open_files.write().insert(uri.clone(), text.clone());

        // Parse and update AST map, use map, and namespace map
        self.update_ast(&uri, &text);

        // Schedule diagnostics asynchronously so that the first-open
        // response is not blocked by lazy stub parsing (which can take
        // tens of seconds when many class references trigger cache-miss
        // parses).  This matches the did_change path.
        self.schedule_diagnostics(uri.clone());

        self.log(MessageType::INFO, format!("Opened file: {}", uri))
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        if let Some(change) = params.content_changes.first() {
            let text = &change.text;

            // Update stored content
            self.open_files.write().insert(uri.clone(), text.clone());

            // Re-parse and update AST map, use map, and namespace map
            self.update_ast(&uri, text);

            // Schedule diagnostics in a background task with debouncing.
            // This returns immediately so that completion, hover, and
            // signature help are never blocked by diagnostic computation.
            self.schedule_diagnostics(uri);
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri.to_string();

        self.open_files.write().remove(&uri);

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

    async fn code_action(&self, params: CodeActionParams) -> Result<Option<CodeActionResponse>> {
        let uri = params.text_document.uri.to_string();

        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            let actions = crate::util::catch_panic_unwind_safe("code_action", &uri, None, || {
                self.handle_code_action(&uri, &content, &params)
            });

            if let Some(actions) = actions
                && !actions.is_empty()
            {
                return Ok(Some(actions));
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

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let uri = params.text_document.uri.to_string();
        let position = params.position;

        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            let result = crate::util::catch_panic_unwind_safe(
                "prepare_rename",
                &uri,
                Some(position),
                || self.handle_prepare_rename(&uri, &content, position),
            );

            if let Some(response) = result {
                return Ok(response);
            }
        }

        Ok(None)
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let uri = params.text_document_position.text_document.uri.to_string();
        let position = params.text_document_position.position;
        let new_name = &params.new_name;

        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            let new_name = new_name.to_string();
            let result =
                crate::util::catch_panic_unwind_safe("rename", &uri, Some(position), || {
                    self.handle_rename(&uri, &content, position, &new_name)
                });

            if let Some(edit) = result {
                return Ok(edit);
            }
        }

        Ok(None)
    }
}

// ─── Self-scan helpers ──────────────────────────────────────────────────────

impl Backend {
    /// Build a classmap by self-scanning the project.
    ///
    /// When `composer.json` exists, scans the directories declared in
    /// `autoload.psr-4`, `autoload-dev.psr-4`, `autoload.classmap`, and
    /// `autoload-dev.classmap`, plus vendor packages from `installed.json`.
    ///
    /// When no `composer.json` exists, falls back to scanning the entire
    /// workspace root (excluding hidden directories and vendor).
    fn build_self_classmap(
        &self,
        workspace_root: &std::path::Path,
        vendor_dir: &str,
        has_composer_json: bool,
    ) -> HashMap<String, PathBuf> {
        if !has_composer_json {
            // No composer.json — walk everything under workspace root.
            return classmap_scanner::scan_workspace_fallback(workspace_root, vendor_dir);
        }

        // Read composer.json to extract autoload directories.
        let composer_path = workspace_root.join("composer.json");
        let json = match std::fs::read_to_string(&composer_path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        {
            Some(j) => j,
            None => {
                return classmap_scanner::scan_workspace_fallback(workspace_root, vendor_dir);
            }
        };

        let mut psr4_dirs: Vec<(String, PathBuf)> = Vec::new();
        let mut classmap_dirs: Vec<PathBuf> = Vec::new();

        // Extract from both "autoload" and "autoload-dev" sections.
        for section_key in &["autoload", "autoload-dev"] {
            if let Some(section) = json.get(section_key) {
                // PSR-4 entries
                if let Some(psr4) = section.get("psr-4").and_then(|p| p.as_object()) {
                    for (prefix, paths) in psr4 {
                        let normalised = if prefix.is_empty() {
                            String::new()
                        } else if prefix.ends_with('\\') {
                            prefix.clone()
                        } else {
                            format!("{prefix}\\")
                        };
                        for dir_str in json_value_to_strings(paths) {
                            let dir = workspace_root.join(&dir_str);
                            psr4_dirs.push((normalised.clone(), dir));
                        }
                    }
                }

                // Classmap entries
                if let Some(cm) = section.get("classmap").and_then(|c| c.as_array()) {
                    for entry in cm {
                        if let Some(dir_str) = entry.as_str() {
                            classmap_dirs.push(workspace_root.join(dir_str));
                        }
                    }
                }
            }
        }

        // Scan user source directories.
        let mut classmap =
            classmap_scanner::scan_psr4_directories(&psr4_dirs, &classmap_dirs, vendor_dir);

        // Scan vendor packages from installed.json.
        let vendor_cm = classmap_scanner::scan_vendor_packages(workspace_root, vendor_dir);
        for (fqcn, path) in vendor_cm {
            classmap.entry(fqcn).or_insert(path);
        }

        classmap
    }

    /// Check whether the Composer classmap is incomplete.
    ///
    /// Reads the PSR-4 namespace prefixes from the project's own
    /// `composer.json` and checks whether the classmap contains at
    /// least one entry for each prefix.  If a prefix is entirely
    /// absent, the classmap is considered incomplete (likely from a
    /// non-optimized `composer dump-autoload` that only covers vendor
    /// code).
    fn is_classmap_incomplete(
        &self,
        workspace_root: &std::path::Path,
        classmap: &HashMap<String, PathBuf>,
    ) -> bool {
        let composer_path = workspace_root.join("composer.json");
        let json = match std::fs::read_to_string(&composer_path)
            .ok()
            .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        {
            Some(j) => j,
            None => return false,
        };

        // Collect PSR-4 namespace prefixes from the project's own autoload.
        let mut user_prefixes: Vec<String> = Vec::new();
        for section_key in &["autoload", "autoload-dev"] {
            if let Some(psr4) = json
                .get(section_key)
                .and_then(|s| s.get("psr-4"))
                .and_then(|p| p.as_object())
            {
                for prefix in psr4.keys() {
                    if !prefix.is_empty() {
                        let normalised = if prefix.ends_with('\\') {
                            prefix.clone()
                        } else {
                            format!("{prefix}\\")
                        };
                        user_prefixes.push(normalised);
                    }
                }
            }
        }

        if user_prefixes.is_empty() {
            // No PSR-4 prefixes to check — can't determine completeness.
            return false;
        }

        // Check that at least one classmap entry exists for each prefix.
        for prefix in &user_prefixes {
            let has_entry = classmap
                .keys()
                .any(|fqcn| fqcn.starts_with(prefix.as_str()));
            if !has_entry {
                // Check that the PSR-4 source directory actually contains
                // PHP files.  If it does, the classmap is incomplete.
                if let Some(psr4_obj) = json
                    .get("autoload")
                    .and_then(|s| s.get("psr-4"))
                    .and_then(|p| p.as_object())
                    .into_iter()
                    .chain(
                        json.get("autoload-dev")
                            .and_then(|s| s.get("psr-4"))
                            .and_then(|p| p.as_object()),
                    )
                    .next()
                {
                    let raw_prefix = prefix.trim_end_matches('\\');
                    if let Some(paths) = psr4_obj
                        .get(raw_prefix)
                        .or_else(|| psr4_obj.get(prefix.as_str()))
                    {
                        for dir_str in json_value_to_strings(paths) {
                            let dir = workspace_root.join(&dir_str);
                            if dir.is_dir() {
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }
}

/// Extract string values from a JSON value that is either a single
/// string or an array of strings.
fn json_value_to_strings(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::String(s) => vec![s.clone()],
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}
