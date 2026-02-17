/// LSP server trait implementation.
///
/// This module contains the `impl LanguageServer for Backend` block,
/// which handles all LSP protocol messages (initialize, didOpen, didChange,
/// didClose, completion, etc.).
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

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
                definition_provider: Some(OneOf::Left(true)),
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
                    "PHPantomLSP initialized! Loaded {} PSR-4 mapping(s), {} classmap entries, {} autoload file(s)",
                    mapping_count, classmap_count, autoload_count
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

        if let Some(content) = content
            && let Some(location) = self.resolve_definition(&uri, &content, position)
        {
            return Ok(Some(GotoDefinitionResponse::Scalar(location)));
        }

        Ok(None)
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

        if let Some(content) = content {
            let classes = classes.unwrap_or_default();

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

            // Try to extract a completion target (requires `->` or `::`)
            if let Some(target) = Self::extract_completion_target(&content, position) {
                let cursor_offset = Self::position_to_offset(&content, position);
                let current_class =
                    cursor_offset.and_then(|off| Self::find_class_at_offset(&classes, off));

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
                        // The name contains `\` — check if the first segment
                        // is a use-map alias (e.g. `OA\Endpoint` where
                        // `use Swagger\OpenAPI as OA;` maps `OA` →
                        // `Swagger\OpenAPI`).  Expand it to the FQN.
                        let first_segment = name.split('\\').next().unwrap_or(name);
                        if let Some(fqn_prefix) = file_use_map.get(first_segment) {
                            let rest = &name[first_segment.len()..];
                            let expanded = format!("{}{}", fqn_prefix, rest);
                            if let Some(cls) = self.find_or_load_class(&expanded) {
                                return Some(cls);
                            }
                        }
                        // Also try prefixing with the current namespace.
                        if let Some(ref ns) = file_namespace {
                            let ns_qualified = format!("{}\\{}", ns, name);
                            if let Some(cls) = self.find_or_load_class(&ns_qualified) {
                                return Some(cls);
                            }
                        }
                        name
                    };

                    self.find_or_load_class(resolved_name)
                };

                // Build a function_loader closure that looks up standalone
                // functions by name in the global_functions map and returns
                // their FunctionInfo (needed for return-type resolution on
                // call expressions like `app()->method()`).
                let function_loader = |name: &str| -> Option<crate::FunctionInfo> {
                    // Build candidate names to try: exact name, use-map
                    // resolved name, and namespace-qualified name.
                    let mut candidates: Vec<&str> = vec![name];

                    let use_resolved: Option<String> = file_use_map.get(name).cloned();
                    if let Some(ref fqn) = use_resolved {
                        candidates.push(fqn.as_str());
                    }

                    let ns_qualified: Option<String> = file_namespace
                        .as_ref()
                        .map(|ns| format!("{}\\{}", ns, name));
                    if let Some(ref nq) = ns_qualified {
                        candidates.push(nq.as_str());
                    }

                    // Unified lookup: checks global_functions first, then
                    // falls back to embedded PHP stubs (parsed lazily and
                    // cached for future lookups).
                    self.find_or_load_function(&candidates)
                };

                // `static::` in a final class is equivalent to `self::` but
                // suggests the class can be subclassed — which it can't.
                // Suppress suggestions to nudge the developer toward `self::`.
                let suppress =
                    target.subject == "static" && current_class.is_some_and(|cc| cc.is_final);

                let candidates = if suppress {
                    vec![]
                } else {
                    Self::resolve_target_classes(
                        &target.subject,
                        target.access_kind,
                        current_class,
                        &classes,
                        &content,
                        cursor_offset.unwrap_or(0),
                        &class_loader,
                        Some(&function_loader),
                    )
                };

                if !candidates.is_empty() {
                    // `parent::`, `self::`, and `static::` are syntactically
                    // `::` but semantically different from external static
                    // access: they show both static and instance members
                    // (PHP allows `self::nonStaticMethod()` etc. from an
                    // instance context).  `parent::` additionally excludes
                    // private members, which is handled by visibility
                    // filtering below.
                    let effective_access =
                        if matches!(target.subject.as_str(), "parent" | "self" | "static") {
                            AccessKind::ParentDoubleColon
                        } else {
                            target.access_kind
                        };

                    // Merge completion items from all candidate classes,
                    // deduplicating by label so ambiguous variables show
                    // the union of all possible members.
                    let mut all_items: Vec<CompletionItem> = Vec::new();
                    let current_class_name = current_class.map(|cc| cc.name.as_str());
                    for target_class in &candidates {
                        let merged =
                            Self::resolve_class_with_inheritance(target_class, &class_loader);

                        // Determine whether the cursor is inside the target
                        // class itself or inside a (transitive) subclass.
                        // This controls whether `__construct` is offered
                        // via `::` access.
                        let is_self_or_ancestor = if let Some(cc) = current_class {
                            if cc.name == target_class.name {
                                true
                            } else {
                                // Walk the parent chain of the current class
                                // to see if the target is an ancestor.
                                let mut ancestor_name = cc.parent_class.clone();
                                let mut found = false;
                                let mut depth = 0u32;
                                while let Some(ref name) = ancestor_name {
                                    depth += 1;
                                    if depth > 20 {
                                        break;
                                    }
                                    if *name == target_class.name {
                                        found = true;
                                        break;
                                    }
                                    ancestor_name =
                                        class_loader(name).and_then(|ci| ci.parent_class.clone());
                                }
                                found
                            }
                        } else {
                            false
                        };

                        let items = Self::build_completion_items(
                            &merged,
                            effective_access,
                            current_class_name,
                            is_self_or_ancestor,
                        );
                        for item in items {
                            if !all_items
                                .iter()
                                .any(|existing| existing.label == item.label)
                            {
                                all_items.push(item);
                            }
                        }
                    }
                    if !all_items.is_empty() {
                        return Ok(Some(CompletionResponse::Array(all_items)));
                    }
                }
            }

            // ── Class name + constant completion ────────────────────
            // When there is no `->` or `::` operator, check whether the
            // user is typing a class name or constant and offer
            // completions from all known sources (use-imports, same
            // namespace, stubs, classmap, class_index, global_defines,
            // stub_constant_index).
            if let Some(partial) = Self::extract_partial_class_name(&content, position) {
                let (class_items, class_incomplete) = self.build_class_name_completions(
                    &file_use_map,
                    &file_namespace,
                    &partial,
                    &content,
                );
                let (constant_items, const_incomplete) = self.build_constant_completions(&partial);

                if !class_items.is_empty() || !constant_items.is_empty() {
                    let mut items = class_items;
                    items.extend(constant_items);
                    return Ok(Some(CompletionResponse::List(CompletionList {
                        is_incomplete: class_incomplete || const_incomplete,
                        items,
                    })));
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
