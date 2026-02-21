/// Completion request orchestration.
///
/// This module contains the main `handle_completion` method that was
/// previously inlined in `server.rs`.  It coordinates the various
/// completion strategies (PHPDoc tags, named arguments, array shape keys,
/// member access, variable names, class/constant/function names) and
/// returns the first successful result.
///
/// Each strategy is extracted into a named private method:
/// - `complete_phpdoc_tag` — `@tag` completion inside docblocks
/// - `complete_docblock_type_or_variable` — type/variable after `@param`, `@return`, etc.
/// - `complete_type_hint` — type completion in parameter lists, return types, properties
/// - `try_named_arg_completion` — `name:` argument completion inside call parens
/// - `try_array_shape_completion` — `$arr['key']` completion from shape annotations
/// - `try_member_access_completion` — `->` and `::` member completion
/// - `try_variable_name_completion` — `$var` name completion
/// - `try_catch_completion` — exception type completion inside `catch()`
/// - `try_throw_new_completion` — Throwable-only completion after `throw new`
/// - `try_class_constant_function_completion` — bare class/constant/function names
///
/// Methods prefixed with `complete_` always short-circuit: the caller
/// unconditionally returns their result.  Methods prefixed with `try_`
/// return `Option<CompletionResponse>` where `None` means "not applicable,
/// try the next strategy."
///
/// Helper methods `patch_content_at_cursor` and `resolve_named_arg_params`
/// are also housed here because they are exclusively used by the
/// completion handler.
use std::panic;

use super::resolver::ResolutionCtx;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::FileContext;

/// PHP scalar and built-in types offered in docblock type positions.
///
/// These are prepended to class-name results so that typing `@param str`
/// suggests `string` alongside any user-defined classes starting with `str`.
const PHPDOC_SCALAR_TYPES: &[&str] = &[
    "string",
    "int",
    "float",
    "bool",
    "array",
    "object",
    "mixed",
    "void",
    "null",
    "callable",
    "iterable",
    "never",
    "self",
    "static",
    "true",
    "false",
    "class-string",
    "list",
    "non-empty-list",
    "non-empty-array",
    "positive-int",
    "negative-int",
    "non-negative-int",
    "non-positive-int",
    "non-empty-string",
    "numeric-string",
    "array-key",
    "scalar",
    "numeric",
];

impl Backend {
    /// Main completion handler — called by `LanguageServer::completion`.
    ///
    /// Tries each completion strategy in priority order and returns the
    /// first one that produces results.  Falls back to no completions
    /// when nothing matches.
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

        if let Some(content) = content {
            // Gather per-file context (classes, use-map, namespace) in one
            // call instead of three separate lock-and-unwrap blocks.
            let ctx = self.file_context(&uri);

            // ── Suppress completion inside non-doc comments ─────────
            if crate::completion::comment_position::is_inside_non_doc_comment(&content, position) {
                return Ok(None);
            }

            // ── PHPDoc tag completion ────────────────────────────────
            // Always short-circuits when an `@` prefix is detected
            // inside a docblock — even when the item list is empty.
            if let Some(prefix) =
                crate::completion::phpdoc::extract_phpdoc_prefix(&content, position)
            {
                return Ok(Some(
                    self.complete_phpdoc_tag(&content, &prefix, position, &ctx),
                ));
            }

            // ── Docblock type / variable completion ─────────────────
            // Always short-circuits when inside a docblock.
            if crate::completion::comment_position::is_inside_docblock(&content, position) {
                return Ok(self.complete_docblock_type_or_variable(&content, position, &ctx));
            }

            // ── Type hint completion in definitions ─────────────────
            // Always short-circuits when a type-hint position is detected.
            if let Some(th_ctx) = crate::completion::type_hint_completion::detect_type_hint_context(
                &content, position,
            ) {
                return Ok(self.complete_type_hint(&content, &th_ctx, &ctx));
            }

            // ── Named argument completion ───────────────────────────
            if let Some(response) = self.try_named_arg_completion(&content, position, &ctx) {
                return Ok(Some(response));
            }

            // ── Array shape key completion ───────────────────────────
            if let Some(response) = self.try_array_shape_completion(&content, position, &ctx) {
                return Ok(Some(response));
            }

            // ── Member access completion (-> or ::) ─────────────────
            if let Some(response) = self.try_member_access_completion(&content, position, &ctx) {
                return Ok(Some(response));
            }

            // ── Variable name completion ────────────────────────────
            if let Some(response) = Self::try_variable_name_completion(&content, position) {
                return Ok(Some(response));
            }

            // ── Smart catch clause completion ───────────────────────
            if let Some(response) = self.try_catch_completion(&content, position, &ctx) {
                return Ok(Some(response));
            }

            // ── `throw new` completion ──────────────────────────────
            if let Some(response) = self.try_throw_new_completion(&content, position, &ctx) {
                return Ok(Some(response));
            }

            // ── Class name + constant + function completion ─────────
            if let Some(response) =
                self.try_class_constant_function_completion(&content, position, &ctx)
            {
                return Ok(Some(response));
            }
        }

        // Nothing matched — return no completions.
        Ok(None)
    }

    // ─── Strategy: PHPDoc tag completion ─────────────────────────────────

    /// Build completions for `@tag` names inside a `/** … */` docblock.
    ///
    /// Called when [`crate::completion::phpdoc::extract_phpdoc_prefix`]
    /// detects that the cursor follows an `@` sign inside a docblock.
    /// Always returns a response (possibly with an empty item list) so
    /// that partial tags like `@potato` never fall through to
    /// class/constant/function completion.
    fn complete_phpdoc_tag(
        &self,
        content: &str,
        prefix: &str,
        position: Position,
        ctx: &FileContext,
    ) -> CompletionResponse {
        let context = crate::completion::phpdoc::detect_context(content, position);
        let items = crate::completion::phpdoc::build_phpdoc_completions(
            content,
            prefix,
            context,
            position,
            &ctx.use_map,
            &ctx.namespace,
        );
        CompletionResponse::Array(items)
    }

    // ─── Strategy: docblock type / variable completion ───────────────────

    /// Build completions at a type or variable position inside a docblock.
    ///
    /// When the cursor is inside a `/** … */` docblock at a recognised tag
    /// position (e.g. after `@param `, `@return `, `@throws `, `@var `),
    /// offer class-name or `$variable` completions as appropriate.  At all
    /// other docblock positions (descriptions, unknown tags) return `None`
    /// so that random words don't trigger class/variable suggestions.
    fn complete_docblock_type_or_variable(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        use crate::completion::phpdoc::{
            DocblockTypingContext, detect_docblock_typing_position, extract_symbol_info,
        };

        match detect_docblock_typing_position(content, position) {
            Some(DocblockTypingContext::Type { partial }) => {
                // Offer scalar / built-in types first, then class
                // / interface / enum names from the project.
                let partial_lower = partial.to_lowercase();
                let mut items: Vec<CompletionItem> = PHPDOC_SCALAR_TYPES
                    .iter()
                    .filter(|t| t.to_lowercase().starts_with(&partial_lower))
                    .enumerate()
                    .map(|(idx, t)| CompletionItem {
                        label: t.to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some("PHP built-in type".to_string()),
                        insert_text: Some(t.to_string()),
                        filter_text: Some(t.to_string()),
                        sort_text: Some(format!("0_scalar_{:03}", idx)),
                        ..CompletionItem::default()
                    })
                    .collect();

                let (class_items, class_incomplete) = self.build_class_name_completions(
                    &ctx.use_map,
                    &ctx.namespace,
                    &partial,
                    content,
                    false, // not a `new` context
                );
                items.extend(class_items);

                if items.is_empty() {
                    None
                } else {
                    Some(CompletionResponse::List(CompletionList {
                        is_incomplete: class_incomplete,
                        items,
                    }))
                }
            }
            Some(DocblockTypingContext::Variable { partial }) => {
                // Offer $parameter names from the function declaration.
                let sym = extract_symbol_info(content, position);
                let partial_lower = partial.to_lowercase();
                let items: Vec<CompletionItem> = sym
                    .params
                    .iter()
                    .filter(|(_, name)| {
                        partial_lower.is_empty() || name.to_lowercase().starts_with(&partial_lower)
                    })
                    .map(|(type_hint, name)| {
                        let detail = type_hint.as_deref().unwrap_or("mixed").to_string();
                        // Always use the full `$name` as insert_text
                        // — the LSP client replaces the typed prefix
                        // (whether `$`, `$na`, or empty) with whatever
                        // we provide, matching how regular variable
                        // completion works in variable_completion.rs.
                        CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(detail),
                            insert_text: Some(name.clone()),
                            filter_text: Some(name.clone()),
                            sort_text: Some(format!("0_{}", name.to_lowercase())),
                            ..CompletionItem::default()
                        }
                    })
                    .collect();
                if items.is_empty() {
                    None
                } else {
                    Some(CompletionResponse::Array(items))
                }
            }
            None => {
                // Description text or unrecognised position — no
                // completions.
                None
            }
        }
    }

    // ─── Strategy: type hint completion ──────────────────────────────────

    /// Build completions at a type-hint position inside a function/method
    /// parameter list, return type, or property declaration.
    ///
    /// Offers PHP native scalar types alongside class-name completions (but
    /// NOT constants or standalone functions, which are invalid in type
    /// positions).
    ///
    /// This check MUST run before named-argument detection so that typing
    /// inside a function *definition* like `function foo(Us|)` offers type
    /// completions rather than named-argument suggestions.
    fn complete_type_hint(
        &self,
        content: &str,
        th_ctx: &crate::completion::type_hint_completion::TypeHintContext,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        let partial_lower = th_ctx.partial.to_lowercase();
        let mut items: Vec<CompletionItem> =
            crate::completion::type_hint_completion::PHP_NATIVE_TYPES
                .iter()
                .filter(|t| t.to_lowercase().starts_with(&partial_lower))
                .enumerate()
                .map(|(idx, t)| CompletionItem {
                    label: t.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some("PHP built-in type".to_string()),
                    insert_text: Some(t.to_string()),
                    filter_text: Some(t.to_string()),
                    sort_text: Some(format!("0_{:03}", idx)),
                    ..CompletionItem::default()
                })
                .collect();

        let (class_items, class_incomplete) = self.build_class_name_completions(
            &ctx.use_map,
            &ctx.namespace,
            &th_ctx.partial,
            content,
            false, // not a `new` context
        );
        items.extend(class_items);

        if items.is_empty() {
            // Even when empty, the caller returns early so we don't fall
            // through to named-arg or class+constant+function completion.
            None
        } else {
            Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items,
            }))
        }
    }

    // ─── Strategy: named argument completion ─────────────────────────────

    /// Try to offer `name:` argument completions inside function/method
    /// call parentheses.
    ///
    /// Returns `None` when the cursor is not in a named-argument context
    /// or when no parameters could be resolved.
    fn try_named_arg_completion(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        let na_ctx = crate::completion::named_args::detect_named_arg_context(content, position)?;

        let mut params = self.resolve_named_arg_params(&na_ctx, content, position, ctx);

        // If resolution failed, the parser may have choked on
        // incomplete code (e.g. an unclosed `(`).  Patch the
        // content by inserting `);` at the cursor position so
        // the class body becomes syntactically valid, then
        // re-parse and retry resolution.
        if params.is_empty() {
            let patched = Self::patch_content_at_cursor(content, position);
            if patched != content {
                let patched_classes = self.parse_php(&patched);
                if !patched_classes.is_empty() {
                    let patched_ctx = FileContext {
                        classes: patched_classes,
                        use_map: ctx.use_map.clone(),
                        namespace: ctx.namespace.clone(),
                    };
                    params =
                        self.resolve_named_arg_params(&na_ctx, &patched, position, &patched_ctx);
                }
            }
        }

        if params.is_empty() {
            return None;
        }

        let items = crate::completion::named_args::build_named_arg_completions(&na_ctx, &params);
        if items.is_empty() {
            None
        } else {
            Some(CompletionResponse::Array(items))
        }
    }

    // ─── Strategy: array shape key completion ────────────────────────────

    /// Try to offer known array shape keys when the cursor is inside
    /// `$var['` or `$var["`.
    ///
    /// Returns `None` when the cursor is not in an array-key context or
    /// when no shape keys could be resolved.
    fn try_array_shape_completion(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        let ak_ctx = crate::completion::array_shape::detect_array_key_context(content, position)?;
        let items = self.build_array_key_completions(&ak_ctx, content, position, ctx);
        if items.is_empty() {
            None
        } else {
            Some(CompletionResponse::Array(items))
        }
    }

    // ─── Strategy: member access completion ──────────────────────────────

    /// Try to offer member completions after `->`, `?->`, or `::`.
    ///
    /// Resolves the subject to one or more `ClassInfo` values, merges
    /// inherited members, and builds completion items filtered by access
    /// kind and visibility.
    ///
    /// Returns `None` when there is no access operator before the cursor
    /// or when resolution produces no results.
    fn try_member_access_completion(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        let target = Self::extract_completion_target(content, position)?;

        let cursor_offset = Self::position_to_offset(content, position);
        let current_class = Self::find_class_at_offset(&ctx.classes, cursor_offset);

        let class_loader = self.class_loader(ctx);
        let function_loader = self.function_loader(ctx);

        // `static::` in a final class is equivalent to `self::` but
        // suggests the class can be subclassed — which it can't.
        // Suppress suggestions to nudge the developer toward `self::`.
        let suppress = target.subject == "static" && current_class.is_some_and(|cc| cc.is_final);

        // Wrap resolution + inheritance merging in catch_unwind so
        // that a stack overflow (e.g. from deep trait/inheritance
        // resolution when the subject is a call expression like
        // `collect($x)->`) doesn't crash the LSP server process.
        // The variable-resolution path already has its own
        // catch_unwind, but the direct call-expression path
        // (resolve_call_return_types → type_hint_to_classes →
        // class_loader → find_or_load_class → parse_php →
        // resolve_class_with_inheritance) does not.
        let member_items_result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
            let candidates = if suppress {
                vec![]
            } else {
                let rctx = ResolutionCtx {
                    current_class,
                    all_classes: &ctx.classes,
                    content,
                    cursor_offset,
                    class_loader: &class_loader,
                    function_loader: Some(&function_loader),
                };
                Self::resolve_target_classes(&target.subject, target.access_kind, &rctx)
            };

            if candidates.is_empty() {
                return vec![];
            }

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
                let merged = Self::resolve_class_with_inheritance(target_class, &class_loader);

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
                            let normalized = name.strip_prefix('\\').unwrap_or(name);
                            if normalized == target_class.name {
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
            all_items
        }));

        match member_items_result {
            Ok(all_items) if !all_items.is_empty() => Some(CompletionResponse::Array(all_items)),
            Err(_) => {
                log::error!(
                    "PHPantom: panic during member-access completion for '{}'",
                    target.subject
                );
                None
            }
            _ => None,
        }
    }

    // ─── Strategy: variable name completion ──────────────────────────────

    /// Try to offer `$variable` name completions.
    ///
    /// When the user is typing `$us`, `$_SE`, or just `$`, suggest
    /// variable names found in the current file plus PHP superglobals.
    ///
    /// Returns `None` when the cursor is not at a variable-name position
    /// or when no variables are found.
    fn try_variable_name_completion(
        content: &str,
        position: Position,
    ) -> Option<CompletionResponse> {
        let partial = Self::extract_partial_variable_name(content, position)?;
        let (var_items, var_incomplete) =
            Self::build_variable_completions(content, &partial, position);

        if var_items.is_empty() {
            None
        } else {
            Some(CompletionResponse::List(CompletionList {
                is_incomplete: var_incomplete,
                items: var_items,
            }))
        }
    }

    // ─── Strategy: catch clause completion ───────────────────────────────

    /// Try to offer exception type completions inside a `catch(…)` clause.
    ///
    /// Analyses the corresponding try block and suggests only the exception
    /// types that are thrown or documented there.  When no specific thrown
    /// types are found, falls back to Throwable-filtered class completion.
    ///
    /// Returns `None` when the cursor is not inside a catch clause or when
    /// no completions could be produced.
    fn try_catch_completion(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        let catch_ctx =
            crate::completion::catch_completion::detect_catch_context(content, position)?;

        let items = crate::completion::catch_completion::build_catch_completions(&catch_ctx);
        if catch_ctx.has_specific_types && !items.is_empty() {
            return Some(CompletionResponse::Array(items));
        }

        // No specific throws discovered — fall back to
        // Throwable-filtered class completion.  Already-parsed
        // classes are only offered when their parent chain
        // reaches \Throwable / \Exception / \Error.  Classmap
        // and stub classes are included unfiltered because
        // checking their ancestry would require on-demand parsing.
        //
        // Use the partial from the catch context rather than
        // `extract_partial_class_name` — the latter returns
        // `None` when the cursor sits right after `(` with
        // nothing typed, but the catch context already
        // captured the (possibly empty) partial correctly.
        let partial = if catch_ctx.partial.is_empty() {
            Self::extract_partial_class_name(content, position).unwrap_or_default()
        } else {
            catch_ctx.partial.clone()
        };
        let (class_items, class_incomplete) = self.build_catch_class_name_completions(
            &ctx.use_map,
            &ctx.namespace,
            &partial,
            content,
            false,
        );
        let mut all_items = items; // Throwable item (if matched)
        for ci in class_items {
            if !all_items.iter().any(|existing| existing.label == ci.label) {
                all_items.push(ci);
            }
        }
        if all_items.is_empty() {
            None
        } else {
            Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items: all_items,
            }))
        }
    }

    // ─── Strategy: throw new completion ──────────────────────────────────

    /// Try to offer Throwable-only class completions after `throw new`.
    ///
    /// Restricts to Throwable descendants only — no constants or functions.
    ///
    /// Returns `None` when the cursor is not in a `throw new` context or
    /// when no completions could be produced.
    fn try_throw_new_completion(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        let partial = Self::extract_partial_class_name(content, position)?;
        if !Self::is_throw_new_context(content, position) {
            return None;
        }
        let (class_items, class_incomplete) = self.build_catch_class_name_completions(
            &ctx.use_map,
            &ctx.namespace,
            &partial,
            content,
            true,
        );
        if class_items.is_empty() {
            None
        } else {
            Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items: class_items,
            }))
        }
    }

    // ─── Strategy: class / constant / function completion ────────────────

    /// Try to offer class name, constant, and function completions.
    ///
    /// When there is no `->` or `::` operator, check whether the user is
    /// typing a class name, constant, or function name and offer
    /// completions from all known sources (use-imports, same namespace,
    /// stubs, classmap, class_index, global_defines, stub_constant_index,
    /// global_functions, stub_function_index).
    ///
    /// Returns `None` when the cursor is not at an identifier position or
    /// when no completions could be produced.
    fn try_class_constant_function_completion(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        let partial = Self::extract_partial_class_name(content, position)?;
        let is_new = Self::is_new_context(content, position);
        let (class_items, class_incomplete) = self.build_class_name_completions(
            &ctx.use_map,
            &ctx.namespace,
            &partial,
            content,
            is_new,
        );

        // After `new`, only class names are valid — skip
        // constants and functions.
        if is_new {
            if class_items.is_empty() {
                return None;
            }
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items: class_items,
            }));
        }

        let (constant_items, const_incomplete) = self.build_constant_completions(&partial);
        let (function_items, func_incomplete) = self.build_function_completions(&partial);

        if class_items.is_empty() && constant_items.is_empty() && function_items.is_empty() {
            return None;
        }

        let mut items = class_items;
        items.extend(constant_items);
        items.extend(function_items);
        Some(CompletionResponse::List(CompletionList {
            is_incomplete: class_incomplete || const_incomplete || func_incomplete,
            items,
        }))
    }

    // ─── Shared helpers ─────────────────────────────────────────────────

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
        file_ctx: &FileContext,
    ) -> Vec<crate::ParameterInfo> {
        let expr = &ctx.call_expression;
        let class_loader = self.class_loader(file_ctx);
        let function_loader_cl = self.function_loader(file_ctx);

        // ── Constructor: `new ClassName` ─────────────────────────────
        if let Some(class_name) = expr.strip_prefix("new ") {
            let class_name = class_name.trim();
            if let Some(ci) = class_loader(class_name) {
                let merged = Self::resolve_class_with_inheritance(&ci, &class_loader);
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

            let owner_classes: Vec<crate::ClassInfo> = if subject == "$this"
                || subject == "self"
                || subject == "static"
            {
                let cursor_offset = Self::position_to_offset(content, position);
                let current_class = Self::find_class_at_offset(&file_ctx.classes, cursor_offset);
                current_class.cloned().into_iter().collect()
            } else if subject.starts_with('$') {
                // Variable — resolve via assignment scanning
                let cursor_offset = Self::position_to_offset(content, position);
                let current_class = Self::find_class_at_offset(&file_ctx.classes, cursor_offset);
                let rctx = ResolutionCtx {
                    current_class,
                    all_classes: &file_ctx.classes,
                    content,
                    cursor_offset,
                    class_loader: &class_loader,
                    function_loader: Some(&function_loader_cl),
                };
                Self::resolve_target_classes(subject, crate::AccessKind::Arrow, &rctx)
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

            let owner_class = if class_part == "self" || class_part == "static" {
                let cursor_offset = Self::position_to_offset(content, position);
                let current_class = Self::find_class_at_offset(&file_ctx.classes, cursor_offset);
                current_class.cloned()
            } else if class_part == "parent" {
                let cursor_offset = Self::position_to_offset(content, position);
                let current_class = Self::find_class_at_offset(&file_ctx.classes, cursor_offset);
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
        if let Some(func) = self.resolve_function_name(expr, &file_ctx.use_map, &file_ctx.namespace)
        {
            return func.parameters.clone();
        }

        vec![]
    }
}
