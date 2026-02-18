/// Completion request orchestration.
///
/// This module contains the main `handle_completion` method that was
/// previously inlined in `server.rs`.  It coordinates the various
/// completion strategies (PHPDoc tags, named arguments, array shape keys,
/// member access, variable names, class/constant/function names) and
/// returns the first successful result.
///
/// Helper methods `patch_content_at_cursor` and `resolve_named_arg_params`
/// are also housed here because they are exclusively used by the
/// completion handler.
use std::collections::HashMap;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::Backend;

impl Backend {
    /// Main completion handler — called by `LanguageServer::completion`.
    ///
    /// Tries each completion strategy in priority order and returns the
    /// first one that produces results.  Falls back to a default
    /// `PHPantomLSP` completion item when nothing else matches.
    pub(crate) async fn handle_completion(
        &self,
        params: CompletionParams,
    ) -> Result<Option<CompletionResponse>> {
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
            // These are loaded early because PHPDoc `@throws` completion
            // needs them for auto-import edits.
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

            // ── PHPDoc tag completion ────────────────────────────────
            // When the user types `@` inside a `/** … */` docblock,
            // offer context-aware PHPDoc / PHPStan tag suggestions.
            if let Some(prefix) =
                crate::completion::phpdoc::extract_phpdoc_prefix(&content, position)
            {
                let context = crate::completion::phpdoc::detect_context(&content, position);
                let items = crate::completion::phpdoc::build_phpdoc_completions(
                    &content,
                    &prefix,
                    context,
                    position,
                    &file_use_map,
                    &file_namespace,
                );
                if !items.is_empty() {
                    return Ok(Some(CompletionResponse::Array(items)));
                }
            }

            // ── Named argument completion ───────────────────────────
            // When the cursor is inside the parentheses of a function or
            // method call, offer parameter names as `name:` completions.
            if let Some(na_ctx) =
                crate::completion::named_args::detect_named_arg_context(&content, position)
            {
                let mut params = self.resolve_named_arg_params(
                    &na_ctx,
                    &content,
                    position,
                    &classes,
                    &file_use_map,
                    &file_namespace,
                );

                // If resolution failed, the parser may have choked on
                // incomplete code (e.g. an unclosed `(`).  Patch the
                // content by inserting `);` at the cursor position so
                // the class body becomes syntactically valid, then
                // re-parse and retry resolution.
                if params.is_empty() {
                    let patched = Self::patch_content_at_cursor(&content, position);
                    if patched != content {
                        let patched_classes = self.parse_php(&patched);
                        if !patched_classes.is_empty() {
                            params = self.resolve_named_arg_params(
                                &na_ctx,
                                &patched,
                                position,
                                &patched_classes,
                                &file_use_map,
                                &file_namespace,
                            );
                        }
                    }
                }

                if !params.is_empty() {
                    let items = crate::completion::named_args::build_named_arg_completions(
                        &na_ctx, &params,
                    );
                    if !items.is_empty() {
                        return Ok(Some(CompletionResponse::Array(items)));
                    }
                }
            }

            // ── Array shape key completion ───────────────────────────
            // When the cursor is inside `$var['` or `$var["`, offer
            // known array shape keys from the variable's type annotation.
            if let Some(ak_ctx) =
                crate::completion::array_shape::detect_array_key_context(&content, position)
            {
                let items = self.build_array_key_completions(
                    &ak_ctx,
                    &content,
                    position,
                    &classes,
                    &file_use_map,
                    &file_namespace,
                );
                if !items.is_empty() {
                    return Ok(Some(CompletionResponse::Array(items)));
                }
            }

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
                    self.resolve_class_name(name, &classes, &file_use_map, &file_namespace)
                };

                let function_loader = |name: &str| -> Option<crate::FunctionInfo> {
                    self.resolve_function_name(name, &file_use_map, &file_namespace)
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
                            crate::AccessKind::ParentDoubleColon
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

            // ── Variable name completion ────────────────────────────
            // When the user is typing `$us`, `$_SE`, or just `$`,
            // suggest variable names found in the current file plus
            // PHP superglobals.
            if let Some(partial) = Self::extract_partial_variable_name(&content, position) {
                let (var_items, var_incomplete) =
                    Self::build_variable_completions(&content, &partial, position);

                if !var_items.is_empty() {
                    return Ok(Some(CompletionResponse::List(CompletionList {
                        is_incomplete: var_incomplete,
                        items: var_items,
                    })));
                }
            }

            // ── Class name + constant + function completion ─────────
            // When there is no `->` or `::` operator, check whether the
            // user is typing a class name, constant, or function name
            // and offer completions from all known sources (use-imports,
            // same namespace, stubs, classmap, class_index,
            // global_defines, stub_constant_index, global_functions,
            // stub_function_index).
            if let Some(partial) = Self::extract_partial_class_name(&content, position) {
                let (class_items, class_incomplete) = self.build_class_name_completions(
                    &file_use_map,
                    &file_namespace,
                    &partial,
                    &content,
                );
                let (constant_items, const_incomplete) = self.build_constant_completions(&partial);
                let (function_items, func_incomplete) = self.build_function_completions(&partial);

                if !class_items.is_empty()
                    || !constant_items.is_empty()
                    || !function_items.is_empty()
                {
                    let mut items = class_items;
                    items.extend(constant_items);
                    items.extend(function_items);
                    return Ok(Some(CompletionResponse::List(CompletionList {
                        is_incomplete: class_incomplete || const_incomplete || func_incomplete,
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

    /// Insert `);` at the given cursor position in `content`.
    ///
    /// This produces a patched version of the source that the parser can
    /// handle when the user is in the middle of typing a function call
    /// (e.g. `$this->greet(|` where the closing `)` hasn't been typed
    /// yet).  Closing the call expression lets the parser recover the
    /// surrounding class/function structure.
    fn patch_content_at_cursor(content: &str, position: Position) -> String {
        let line_idx = position.line as usize;
        let col = position.character as usize;
        let mut result = String::with_capacity(content.len() + 2);

        for (i, line) in content.lines().enumerate() {
            if i == line_idx {
                // Insert `);` at the cursor column
                let byte_col = line
                    .char_indices()
                    .nth(col)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
                result.push_str(&line[..byte_col]);
                result.push_str(");");
                result.push_str(&line[byte_col..]);
            } else {
                result.push_str(line);
            }
            result.push('\n');
        }

        // Remove the trailing newline we may have added if the original
        // content did not end with one.
        if !content.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }

        result
    }

    /// Resolve the parameter list for a named-argument completion context.
    ///
    /// Examines the `call_expression` in the context and looks up the
    /// corresponding function or method to extract its parameters.
    fn resolve_named_arg_params(
        &self,
        ctx: &crate::completion::named_args::NamedArgContext,
        content: &str,
        position: Position,
        classes: &[crate::ClassInfo],
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
    ) -> Vec<crate::ParameterInfo> {
        let expr = &ctx.call_expression;

        // ── Constructor: `new ClassName` ─────────────────────────────
        if let Some(class_name) = expr.strip_prefix("new ") {
            let class_name = class_name.trim();
            if let Some(ci) =
                self.resolve_class_name(class_name, classes, file_use_map, file_namespace)
            {
                let merged = Self::resolve_class_with_inheritance(&ci, &|name| {
                    self.resolve_class_name(name, classes, file_use_map, file_namespace)
                });
                if let Some(ctor) = merged.methods.iter().find(|m| m.name == "__construct") {
                    return ctor.parameters.clone();
                }
            }
            return vec![];
        }

        // ── Instance method: `$subject->method` ─────────────────────
        if let Some(pos) = expr.rfind("->") {
            let subject = &expr[..pos];
            let method_name = &expr[pos + 2..];
            let class_loader = |name: &str| -> Option<crate::ClassInfo> {
                self.resolve_class_name(name, classes, file_use_map, file_namespace)
            };

            let owner_classes: Vec<crate::ClassInfo> =
                if subject == "$this" || subject == "self" || subject == "static" {
                    let cursor_offset = Self::position_to_offset(content, position);
                    let current_class =
                        cursor_offset.and_then(|off| Self::find_class_at_offset(classes, off));
                    current_class.cloned().into_iter().collect()
                } else if subject.starts_with('$') {
                    // Variable — resolve via assignment scanning
                    let cursor_offset = Self::position_to_offset(content, position).unwrap_or(0);
                    let current_class = Self::find_class_at_offset(classes, cursor_offset);
                    let function_loader = |name: &str| -> Option<crate::FunctionInfo> {
                        self.resolve_function_name(name, file_use_map, file_namespace)
                    };
                    Self::resolve_target_classes(
                        subject,
                        crate::AccessKind::Arrow,
                        current_class,
                        classes,
                        content,
                        cursor_offset,
                        &class_loader,
                        Some(&function_loader),
                    )
                } else {
                    vec![]
                };

            for owner in &owner_classes {
                let merged = Self::resolve_class_with_inheritance(owner, &class_loader);
                if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
                    return method.parameters.clone();
                }
            }
            return vec![];
        }

        // ── Static method: `ClassName::method` ──────────────────────
        if let Some(pos) = expr.rfind("::") {
            let class_part = &expr[..pos];
            let method_name = &expr[pos + 2..];
            let class_loader = |name: &str| -> Option<crate::ClassInfo> {
                self.resolve_class_name(name, classes, file_use_map, file_namespace)
            };

            let owner_class = if class_part == "self" || class_part == "static" {
                let cursor_offset = Self::position_to_offset(content, position);
                let current_class =
                    cursor_offset.and_then(|off| Self::find_class_at_offset(classes, off));
                current_class.cloned()
            } else if class_part == "parent" {
                let cursor_offset = Self::position_to_offset(content, position);
                let current_class =
                    cursor_offset.and_then(|off| Self::find_class_at_offset(classes, off));
                current_class
                    .and_then(|cc| cc.parent_class.as_ref())
                    .and_then(|p| class_loader(p))
            } else {
                class_loader(class_part)
            };

            if let Some(ref owner) = owner_class {
                let merged = Self::resolve_class_with_inheritance(owner, &class_loader);
                if let Some(method) = merged.methods.iter().find(|m| m.name == method_name) {
                    return method.parameters.clone();
                }
            }
            return vec![];
        }

        // ── Standalone function: `functionName` ─────────────────────
        if let Some(func) = self.resolve_function_name(expr, file_use_map, file_namespace) {
            return func.parameters.clone();
        }

        vec![]
    }
}
