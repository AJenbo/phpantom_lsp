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
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::resolver::ResolutionCtx;

use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::class_completion::{
    ClassCompletionParams, ClassNameContext, detect_class_name_context, is_class_declaration_name,
};
use crate::completion::named_args::{NamedArgContext, parse_existing_args};
use crate::docblock::types::PHPDOC_TYPE_KEYWORDS;
use crate::symbol_map::SymbolKind;
use crate::types::{CompletionTarget, FileContext};
use crate::util::{find_class_at_offset, position_to_byte_offset, position_to_offset};

/// Check whether a `(` immediately follows the cursor position (past any
/// partial identifier the user has already typed).
///
/// When the user is renaming an existing call — `$obj->oldName|()`,
/// `functionNa|()`, `new ClassNa|()` — the opening paren is already
/// present and inserting a snippet with its own `()` would produce
/// double parentheses like `method()()`.
fn paren_follows_cursor(content: &str, position: Position) -> bool {
    let byte_off = position_to_byte_offset(content, position);
    let rest = &content[byte_off..];
    // Skip past any partial identifier the user has typed
    // (ASCII letters, digits, underscore, backslash for namespaced names).
    let after_ident =
        rest.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '\\');
    after_ident.starts_with('(')
}

/// Downgrade callable snippet items to plain-name insertions.
///
/// When `(` already follows the cursor, snippets that insert their own
/// parentheses would produce duplicates.  This strips the snippet
/// format and replaces the insert text with just the name from
/// `filter_text`.
///
/// Applies to methods, functions, and class names (for `new` / `throw new`).
fn strip_snippet_parens(items: Vec<CompletionItem>) -> Vec<CompletionItem> {
    items
        .into_iter()
        .map(|mut item| {
            if item.insert_text_format == Some(InsertTextFormat::SNIPPET)
                && matches!(
                    item.kind,
                    Some(CompletionItemKind::METHOD)
                        | Some(CompletionItemKind::FUNCTION)
                        | Some(CompletionItemKind::CLASS)
                )
            {
                // Replace the snippet with just the name
                // (the filter_text already holds it).
                if let Some(ref name) = item.filter_text {
                    item.insert_text = Some(name.clone());
                }
                // Also clear any text_edit that carries the snippet text.
                if let Some(CompletionTextEdit::Edit(ref mut te)) = item.text_edit
                    && let Some(ref name) = item.filter_text
                {
                    te.new_text = name.clone();
                }
                item.insert_text_format = None;
            }
            item
        })
        .collect()
}

/// Filter out completion items for classes defined in the current file.
///
/// When writing a `use` statement it makes no sense to import a class
/// from the file you are already in.  The `detail` field of each item
/// carries the FQN, which is matched against the FQNs of classes in the
/// file's `ctx.classes` (from the ast_map).
fn filter_current_file_classes(
    items: Vec<CompletionItem>,
    ctx: &FileContext,
) -> Vec<CompletionItem> {
    if ctx.classes.is_empty() {
        return items;
    }
    let current_fqns: HashSet<String> = ctx
        .classes
        .iter()
        .map(|cls| {
            if let Some(ref ns) = ctx.namespace {
                format!("{}\\{}", ns, cls.name)
            } else {
                cls.name.clone()
            }
        })
        .collect();
    items
        .into_iter()
        .filter(|item| {
            item.detail
                .as_ref()
                .is_none_or(|d| !current_fqns.contains(d))
        })
        .collect()
}

/// Filter out completion items for functions defined in the current file.
///
/// Collects the map keys (FQNs) of functions whose URI matches the
/// current file and removes any completion item whose `insert_text`
/// matches one of those FQNs.  This works for both use-import items
/// (where `insert_text` is the FQN) and inline items (where
/// `insert_text` is a snippet starting with the short name, which
/// equals the FQN for global functions).
fn filter_current_file_functions(
    items: Vec<CompletionItem>,
    current_uri: &str,
    backend: &Backend,
) -> Vec<CompletionItem> {
    let current_funcs: HashSet<String> = {
        let fmap = backend.global_functions().read();
        fmap.iter()
            .filter(|(_, (uri, _))| uri == current_uri)
            .map(|(key, _)| key.clone())
            .collect()
    };
    if current_funcs.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| {
            item.insert_text
                .as_ref()
                .is_none_or(|it| !current_funcs.contains(it))
        })
        .collect()
}

/// Filter out completion items for constants defined in the current file.
fn filter_current_file_constants(
    items: Vec<CompletionItem>,
    current_uri: &str,
    backend: &Backend,
) -> Vec<CompletionItem> {
    let current_consts: HashSet<String> = {
        let dmap = backend.global_defines().read();
        dmap.iter()
            .filter(|(_, info)| info.file_uri.as_str() == current_uri)
            .map(|(name, _)| name.clone())
            .collect()
    };
    if current_consts.is_empty() {
        return items;
    }
    items
        .into_iter()
        .filter(|item| {
            item.filter_text
                .as_ref()
                .is_none_or(|ft| !current_consts.contains(ft))
        })
        .collect()
}

/// Append a semicolon to the `insert_text` of each completion item.
///
/// Used for `use`, `use function`, and `use const` completions so that
/// accepting a suggestion produces a complete statement (e.g. `use Foo\Bar;`).
fn append_semicolon_to_insert_text(items: Vec<CompletionItem>) -> Vec<CompletionItem> {
    items
        .into_iter()
        .map(|mut item| {
            // Namespace segment items (MODULE kind) represent
            // intermediate namespace paths the user can drill into.
            // They should not receive a trailing semicolon because
            // the user will continue typing after selecting one
            // (e.g. `use App\Models\` → pick a class next).
            if item.kind == Some(CompletionItemKind::MODULE) {
                return item;
            }
            if let Some(ref mut text) = item.insert_text
                && !text.ends_with(';')
            {
                text.push(';');
            }
            if let Some(CompletionTextEdit::Edit(ref mut edit)) = item.text_edit
                && !edit.new_text.ends_with(';')
            {
                edit.new_text.push(';');
            }
            item
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclarationHeaderKind {
    Class,
    Interface,
    Enum,
}

/// Return the start index of the current statement-ish segment ending at
/// `offset` (exclusive), using `{`, `}`, `;`, and newlines as hard boundaries.
fn statement_segment_start(chars: &[char], offset: usize) -> usize {
    for i in (0..offset).rev() {
        if matches!(chars[i], '{' | '}' | ';' | '\n' | '\r') {
            return i + 1;
        }
    }
    0
}

/// Collect lowercase ASCII identifier-like words (`[A-Za-z0-9_]+`) from
/// `chars[start..end]`.
fn collect_ascii_words(chars: &[char], start: usize, end: usize) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut i = start;
    while i < end {
        if chars[i].is_ascii_alphanumeric() || chars[i] == '_' {
            let j = i;
            i += 1;
            while i < end && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            words.push(chars[j..i].iter().collect::<String>().to_ascii_lowercase());
            continue;
        }
        i += 1;
    }
    words
}

/// Compute brace depth before `position` using a lightweight text scan.
///
/// This is used for top-level keyword gating (`namespace`) and does not try
/// to fully parse strings/comments.
fn brace_depth_before(content: &str, position: Position) -> i32 {
    let off = position_to_byte_offset(content, position);
    let mut depth = 0_i32;
    for &b in content.as_bytes().iter().take(off.min(content.len())) {
        match b {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
    }
    depth.max(0)
}

/// Detect whether the cursor is inside a class/interface/enum declaration
/// header (before the opening `{`).
fn declaration_header_kind(content: &str, position: Position) -> Option<DeclarationHeaderKind> {
    let chars: Vec<char> = content.chars().collect();
    let offset = crate::completion::named_args::position_to_char_offset(&chars, position)?;

    let start = statement_segment_start(&chars, offset);
    let words = collect_ascii_words(&chars, start, offset);

    if words.is_empty() {
        return None;
    }

    let mut decl_idx_kind: Option<(usize, DeclarationHeaderKind)> = None;
    for (idx, w) in words.iter().enumerate() {
        let kind = match w.as_str() {
            "class" => Some(DeclarationHeaderKind::Class),
            "interface" => Some(DeclarationHeaderKind::Interface),
            "enum" => Some(DeclarationHeaderKind::Enum),
            _ => None,
        };
        if let Some(kind) = kind {
            decl_idx_kind = Some((idx, kind));
            break;
        }
    }
    let (decl_idx, kind) = decl_idx_kind?;

    // Only declaration modifiers may appear before the declaration keyword.
    let valid_prefix = words[..decl_idx]
        .iter()
        .all(|w| matches!(w.as_str(), "abstract" | "final" | "readonly"));
    if !valid_prefix {
        return None;
    }

    // Need at least the declaration name token after `class|interface|enum`.
    let name_token = words.get(decl_idx + 1)?;

    if name_token.is_empty() {
        return None;
    }

    // Guard out anonymous classes (`new class`).
    if kind == DeclarationHeaderKind::Class
        && decl_idx > 0
        && words.get(decl_idx - 1).is_some_and(|w| w == "new")
    {
        return None;
    }

    Some(kind)
}

/// Whether the cursor is right after a class-member modifier chain
/// followed by whitespace, e.g. `public ` or `private static `.
fn is_after_member_modifier_chain(content: &str, position: Position) -> bool {
    let chars: Vec<char> = content.chars().collect();
    let Some(offset) = crate::completion::named_args::position_to_char_offset(&chars, position)
    else {
        return false;
    };
    if offset == 0 || !chars[offset - 1].is_ascii_whitespace() {
        return false;
    }

    let start = statement_segment_start(&chars, offset);
    let words = collect_ascii_words(&chars, start, offset);

    if words.is_empty() {
        return false;
    }

    words.iter().all(|w| {
        matches!(
            w.as_str(),
            "public" | "protected" | "private" | "static" | "abstract" | "final" | "readonly"
        )
    })
}

/// Detect `enum Name: <partial>` backed-type position and return `<partial>`.
///
/// Examples that return `Some`:
/// - `enum Role: |`
/// - `enum Role: st|`
///
/// Returns `None` outside enum declaration headers.
fn enum_backing_type_partial(content: &str, position: Position) -> Option<String> {
    let chars: Vec<char> = content.chars().collect();
    let offset = crate::completion::named_args::position_to_char_offset(&chars, position)?;

    // Walk backward through the currently typed token.
    let mut partial_start = offset;
    while partial_start > 0
        && (chars[partial_start - 1].is_ascii_alphanumeric() || chars[partial_start - 1] == '_')
    {
        partial_start -= 1;
    }

    // Reject obvious non-keyword contexts.
    if partial_start > 0 && chars[partial_start - 1] == '$' {
        return None;
    }
    if partial_start >= 2 && chars[partial_start - 2] == '-' && chars[partial_start - 1] == '>' {
        return None;
    }
    if partial_start >= 2 && chars[partial_start - 2] == ':' && chars[partial_start - 1] == ':' {
        return None;
    }

    let partial: String = chars[partial_start..offset].iter().collect();

    // Back up over whitespace before the partial and require `:`.
    let mut i = partial_start;
    while i > 0 && chars[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == 0 || chars[i - 1] != ':' {
        return None;
    }

    // Only valid inside `enum` declaration headers.
    if !matches!(
        declaration_header_kind(content, position),
        Some(DeclarationHeaderKind::Enum)
    ) {
        return None;
    }

    Some(partial)
}

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
        let content = self.get_file_content(&uri);

        if let Some(content) = content {
            // Gather per-file context (classes, use-map, namespace) in one
            // call instead of three separate lock-and-unwrap blocks.
            let ctx = self.file_context(&uri);

            // ── Suppress completion inside non-doc comments ─────────
            if crate::completion::comment_position::is_inside_non_doc_comment(&content, position) {
                return Ok(None);
            }

            // ── PHPDoc block generation on `/**` ────────────────────
            // When the user types `/**` above a declaration, generate
            // a complete docblock skeleton as a single snippet item.
            // Must run before the docblock-interior checks below.
            {
                let class_loader = self.class_loader(&ctx);
                let function_loader = self.function_loader(&ctx);
                if let Some(response) = crate::completion::phpdoc::generation::try_generate_docblock(
                    &content,
                    position,
                    &ctx.use_map,
                    &ctx.namespace,
                    &class_loader,
                    Some(&function_loader),
                ) {
                    return Ok(Some(response));
                }
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
                return Ok(self.complete_docblock_type_or_variable(&content, position, &ctx, &uri));
            }

            // ── Type hint completion in definitions ─────────────────
            // Always short-circuits when a type-hint position is detected.
            if let Some(th_ctx) = crate::completion::type_hint_completion::detect_type_hint_context(
                &content, position,
            ) {
                return Ok(self.complete_type_hint(&content, &th_ctx, &ctx, position, &uri));
            }

            // ── Named argument completion ───────────────────────────
            if let Some(response) = self.try_named_arg_completion(&uri, &content, position, &ctx) {
                return Ok(Some(response));
            }

            // ── String context detection ────────────────────────────
            // Classify once and use throughout the remaining pipeline.
            let string_ctx =
                crate::completion::comment_position::classify_string_context(&content, position);
            use crate::completion::comment_position::StringContext;

            // ── Array shape key completion ───────────────────────────
            // Runs before `InStringLiteral` suppression because in
            // normal code `$arr['` puts the scanner inside a
            // single-quoted string, yet array shape completion is
            // designed to work there.  Skip only in simple
            // interpolation: `"$arr['key']"` does NOT perform array
            // access in PHP (only `"{$arr['key']}"` does).
            if !matches!(string_ctx, StringContext::SimpleInterpolation)
                && let Some(response) = self.try_array_shape_completion(&content, position, &ctx)
            {
                return Ok(Some(response));
            }

            if matches!(string_ctx, StringContext::InStringLiteral) {
                return Ok(None);
            }

            // ── Member access completion (-> or ::) ─────────────────
            if let Some(response) =
                self.try_member_access_completion(&uri, &content, position, &ctx)
            {
                // In simple interpolation (`"$var->"`), PHP only allows
                // property access — method calls and constants are
                // syntax errors.  Filter to properties only.
                if matches!(string_ctx, StringContext::SimpleInterpolation) {
                    let filtered = match response {
                        CompletionResponse::Array(items) => items
                            .into_iter()
                            .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                            .collect(),
                        CompletionResponse::List(list) => list
                            .items
                            .into_iter()
                            .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                            .collect(),
                    };
                    return Ok(Some(CompletionResponse::Array(filtered)));
                }
                return Ok(Some(response));
            }

            // ── Variable name completion ────────────────────────────
            // Placed before the interpolation guard so that `"$`
            // and `"{$` both offer variable suggestions.
            if let Some(response) = Self::try_variable_name_completion(&content, position) {
                return Ok(Some(response));
            }

            // Inside any interpolation context the only useful
            // completions are variable names and member access (handled
            // above).  Suppress the remaining completion strategies so
            // class names, catch clauses, etc. don't leak into strings.
            if matches!(
                string_ctx,
                StringContext::SimpleInterpolation | StringContext::BraceInterpolation
            ) {
                return Ok(None);
            }

            // ── Smart catch clause completion ───────────────────────
            if let Some(response) = self.try_catch_completion(&content, position, &ctx, &uri) {
                return Ok(Some(response));
            }

            // ── `throw new` completion ──────────────────────────────
            if let Some(response) = self.try_throw_new_completion(&content, position, &ctx, &uri) {
                return Ok(Some(response));
            }

            // ── Class declaration name completion ───────────────────
            // When declaring a new class/interface/trait/enum, suggest
            // the filename (without extension) as the class name.
            if let Some(response) = self.try_class_declaration_completion(&uri, &content, position)
            {
                return Ok(Some(response));
            }

            // ── Class name + constant + function completion ─────────
            if let Some(response) =
                self.try_class_constant_function_completion(&content, position, &ctx, &uri)
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
        let class_loader = self.class_loader(ctx);
        let function_loader = self.function_loader(ctx);

        // For inline variable assignments, try to infer the type from
        // the assignment RHS so that @var can be pre-filled.
        let inferred_var_type =
            if matches!(context, crate::completion::phpdoc::DocblockContext::Inline) {
                let sym = crate::completion::phpdoc::extract_symbol_info(content, position);
                crate::completion::phpdoc::generation::infer_inline_variable_type(
                    &sym,
                    content,
                    position,
                    &ctx.classes,
                    &class_loader,
                    Some(&function_loader as &dyn Fn(&str) -> Option<crate::types::FunctionInfo>),
                )
            } else {
                None
            };

        let smart = crate::completion::phpdoc::SmartContext {
            inferred_inline_var_type: inferred_var_type.as_deref(),
            class_loader: Some(&class_loader),
            function_loader: Some(&function_loader),
        };
        let items = crate::completion::phpdoc::build_phpdoc_completions(
            content,
            prefix,
            context,
            position,
            &ctx.use_map,
            &ctx.namespace,
            &smart,
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
        uri: &str,
    ) -> Option<CompletionResponse> {
        use crate::completion::phpdoc::{
            DocblockTypingContext, detect_docblock_typing_position, extract_symbol_info,
        };

        match detect_docblock_typing_position(content, position) {
            Some(DocblockTypingContext::Type { partial, tag }) => {
                // For @throws, use Throwable-filtered completion with
                // the same ordering as `throw new` so that exception
                // classes appear at the top.
                if tag == "throws" {
                    let (class_items, class_incomplete) = self.build_catch_class_name_completions(
                        ctx, &partial, content, false, position, uri,
                    );
                    return if class_items.is_empty() {
                        None
                    } else {
                        Some(CompletionResponse::List(CompletionList {
                            is_incomplete: class_incomplete,
                            items: class_items,
                        }))
                    };
                }

                // Offer scalar / built-in types first, then class
                // / interface / enum names from the project.
                let partial_lower = partial.to_lowercase();
                let mut items: Vec<CompletionItem> = PHPDOC_TYPE_KEYWORDS
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

                let (class_items, class_incomplete) =
                    self.build_class_name_completions(ClassCompletionParams {
                        file_use_map: &ctx.use_map,
                        file_namespace: &ctx.namespace,
                        prefix: &partial,
                        content,
                        context: ClassNameContext::TypeHint,
                        position,
                        affinity_table_override: None,
                        uri,
                    });
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

                // Compute an explicit replacement range covering the typed
                // `$…` prefix.  Using `text_edit` with a range prevents
                // the double-dollar problem in editors (Helix, Neovim) that
                // don't treat `$` as a word character — the same fix that
                // was applied to regular variable completion.
                let prefix_char_len = partial.chars().count() as u32;
                let replace_range = Range {
                    start: Position {
                        line: position.line,
                        character: position.character.saturating_sub(prefix_char_len),
                    },
                    end: position,
                };

                let items: Vec<CompletionItem> = sym
                    .params
                    .iter()
                    .filter(|(_, name)| {
                        partial_lower.is_empty() || name.to_lowercase().starts_with(&partial_lower)
                    })
                    .map(|(type_hint, name)| {
                        let detail = type_hint.as_deref().unwrap_or("mixed").to_string();
                        CompletionItem {
                            label: name.clone(),
                            kind: Some(CompletionItemKind::VARIABLE),
                            detail: Some(detail),
                            text_edit: Some(CompletionTextEdit::Edit(TextEdit {
                                range: replace_range,
                                new_text: name.clone(),
                            })),
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
        position: Position,
        uri: &str,
    ) -> Option<CompletionResponse> {
        let partial_lower = th_ctx.partial.to_lowercase();
        let space_prefix = if th_ctx.needs_space_prefix { " " } else { "" };
        let mut items: Vec<CompletionItem> =
            crate::completion::type_hint_completion::PHP_NATIVE_TYPES
                .iter()
                .filter(|t| t.to_lowercase().starts_with(&partial_lower))
                .enumerate()
                .map(|(idx, t)| CompletionItem {
                    label: t.to_string(),
                    kind: Some(CompletionItemKind::KEYWORD),
                    detail: Some("PHP built-in type".to_string()),
                    insert_text: Some(format!("{}{}", space_prefix, t)),
                    filter_text: Some(t.to_string()),
                    sort_text: Some(format!("0_{:03}", idx)),
                    ..CompletionItem::default()
                })
                .collect();

        let (class_items, class_incomplete) =
            self.build_class_name_completions(ClassCompletionParams {
                file_use_map: &ctx.use_map,
                file_namespace: &ctx.namespace,
                prefix: &th_ctx.partial,
                content,
                context: ClassNameContext::TypeHint,
                position,
                affinity_table_override: None,
                uri,
            });

        // When a leading space is needed (return type after `:` with no
        // space), prefix the insert text of each class-name item so that
        // the result is `: ClassName` rather than `:ClassName`.
        if th_ctx.needs_space_prefix {
            for mut item in class_items {
                if let Some(ref txt) = item.insert_text {
                    item.insert_text = Some(format!(" {}", txt));
                }
                if let Some(CompletionTextEdit::Edit(ref te)) = item.text_edit {
                    item.text_edit = Some(CompletionTextEdit::Edit(TextEdit {
                        range: te.range,
                        new_text: format!(" {}", te.new_text),
                    }));
                }
                items.push(item);
            }
        } else {
            items.extend(class_items);
        }

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
        uri: &str,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        // ── Primary path: AST-based detection via symbol map ────────
        // The symbol map's `CallSite` data handles chains, nesting,
        // and strings correctly.  Fall back to text scanning when the
        // AST has no hit (typically because the parser couldn't recover
        // from incomplete code).
        let na_ctx = self
            .detect_named_arg_from_symbol_map(uri, content, position)
            .or_else(|| {
                crate::completion::named_args::detect_named_arg_context(content, position)
            })?;

        let mut params = self.resolve_named_arg_params(&na_ctx, content, position, ctx);

        // If resolution failed, the parser may have choked on
        // incomplete code (e.g. an unclosed `(`).  Patch the
        // content by inserting `);` at the cursor position so
        // the class body becomes syntactically valid, then
        // re-parse and retry resolution.
        if params.is_empty() {
            let patched = Self::patch_content_at_cursor(content, position);
            if patched != content {
                let patched_classes: Vec<Arc<crate::types::ClassInfo>> =
                    self.parse_php(&patched).into_iter().map(Arc::new).collect();
                if !patched_classes.is_empty() {
                    let patched_ctx = FileContext {
                        classes: patched_classes,
                        use_map: ctx.use_map.clone(),
                        namespace: ctx.namespace.clone(),
                        resolved_names: ctx.resolved_names.clone(),
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

    /// Detect a named-argument context using precomputed [`CallSite`] data
    /// from the symbol map.
    ///
    /// Returns `None` when the symbol map has no enclosing call site at the
    /// cursor (e.g. the parser couldn't recover from incomplete code) or
    /// when the cursor is in a position that should not trigger named-arg
    /// completion (preceded by `$`, `->`, or `::`).
    fn detect_named_arg_from_symbol_map(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<NamedArgContext> {
        let symbol_map = self.symbol_maps.read().get(uri).cloned()?;

        let cursor_byte_offset = position_to_offset(content, position);
        let cs = symbol_map.find_enclosing_call_site(cursor_byte_offset)?;

        // ── Check eligibility at cursor ─────────────────────────────
        // Walk backward from cursor through identifier chars to find the
        // start of the current "word" in the raw source text.
        let bytes = content.as_bytes();
        let mut word_start = cursor_byte_offset as usize;
        while word_start > 0 && {
            let b = bytes[word_start - 1];
            b.is_ascii_alphanumeric() || b == b'_'
        } {
            word_start -= 1;
        }

        // If preceded by `$`, this is a variable, not a named arg.
        if word_start > 0 && bytes[word_start - 1] == b'$' {
            return None;
        }
        // If preceded by `->` or `::`, member completion handles this.
        if word_start >= 2 && bytes[word_start - 2] == b'-' && bytes[word_start - 1] == b'>' {
            return None;
        }
        if word_start >= 2 && bytes[word_start - 2] == b':' && bytes[word_start - 1] == b':' {
            return None;
        }

        let prefix = content
            .get(word_start..cursor_byte_offset as usize)
            .unwrap_or("")
            .to_string();

        // ── Parse arguments between `(` and cursor ──────────────────
        let args_text = content
            .get(cs.args_start as usize..word_start)
            .unwrap_or("");
        let (existing_named, positional_count) = parse_existing_args(args_text);

        Some(NamedArgContext {
            call_expression: cs.call_expression.clone(),
            existing_named_args: existing_named,
            positional_count,
            prefix,
        })
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
        uri: &str,
        content: &str,
        position: Position,
        ctx: &FileContext,
    ) -> Option<CompletionResponse> {
        // ── Primary path: AST-based detection via symbol map ────────
        // The symbol map's `MemberAccess` correctly handles `(new Foo)->`,
        // call-result chains, array access chains, and null-safe chains.
        // Fall back to text scanning when the symbol map has no hit.
        let target = self
            .extract_completion_target_from_symbol_map(uri, content, position)
            .or_else(|| super::target::extract_completion_target(content, position))?;

        let cursor_offset = position_to_offset(content, position);
        let current_class = find_class_at_offset(&ctx.classes, cursor_offset);

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
        // (resolve_call_return_types_expr → type_hint_to_classes →
        // class_loader → find_or_load_class → parse_php →
        // resolve_class_with_inheritance) does not.
        let member_items = crate::util::catch_panic_unwind_safe(
            "member-access completion",
            uri,
            Some(position),
            || {
                let candidates = if suppress {
                    vec![]
                } else {
                    let rctx = ResolutionCtx {
                        current_class,
                        all_classes: &ctx.classes,
                        content,
                        cursor_offset,
                        class_loader: &class_loader,
                        resolved_class_cache: Some(&self.resolved_class_cache),
                        function_loader: Some(&function_loader),
                    };
                    let mut resolved = super::resolver::resolve_target_classes(
                        &target.subject,
                        target.access_kind,
                        &rctx,
                    );

                    // ── Incomplete-expression retry ─────────────────
                    // When the cursor sits right after `->` (or `?->`)
                    // at the end of an expression with no trailing
                    // semicolon (e.g. inside an arrow function body the
                    // user is still typing), the PHP parser may fail to
                    // produce the enclosing statement.  Patch the
                    // content by appending a dummy identifier +
                    // semicolon so the parser can recover.
                    if resolved.is_empty() && target.subject.starts_with('$') {
                        let patched = Self::patch_incomplete_member_access(content, position);
                        if patched != content {
                            let patched_classes: Vec<Arc<crate::types::ClassInfo>> =
                                self.parse_php(&patched).into_iter().map(Arc::new).collect();
                            let patched_offset = position_to_offset(&patched, position);
                            let patched_current =
                                find_class_at_offset(&patched_classes, patched_offset);
                            let patched_rctx = ResolutionCtx {
                                current_class: patched_current,
                                all_classes: &patched_classes,
                                content: &patched,
                                cursor_offset: patched_offset,
                                class_loader: &class_loader,
                                resolved_class_cache: Some(&self.resolved_class_cache),
                                function_loader: Some(&function_loader),
                            };
                            resolved = super::resolver::resolve_target_classes(
                                &target.subject,
                                target.access_kind,
                                &patched_rctx,
                            );
                        }
                    }

                    resolved
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
                // filtering in `build_completion_items`.
                let effective_access =
                    if matches!(target.subject.as_str(), "parent" | "self" | "static") {
                        crate::AccessKind::ParentDoubleColon
                    } else {
                        target.access_kind
                    };

                super::builder::build_union_completion_items(
                    &candidates,
                    effective_access,
                    current_class,
                    &class_loader,
                    &self.resolved_class_cache,
                    uri,
                )
            },
        );

        match member_items {
            Some(all_items) if !all_items.is_empty() => {
                // ── B15: suppress snippet parentheses when `(` already follows ──
                let items = if paren_follows_cursor(content, position) {
                    strip_snippet_parens(all_items)
                } else {
                    all_items
                };
                Some(CompletionResponse::Array(items))
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
        uri: &str,
    ) -> Option<CompletionResponse> {
        let catch_ctx =
            crate::completion::catch_completion::detect_catch_context(content, position)?;

        let items = crate::completion::catch_completion::build_catch_completions(
            &catch_ctx,
            &ctx.use_map,
            &ctx.namespace,
        );
        if catch_ctx.has_specific_types && !items.is_empty() {
            // These items don't carry snippets, but guard for consistency.
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
        let (class_items, class_incomplete) =
            self.build_catch_class_name_completions(ctx, &partial, content, false, position, uri);
        let mut all_items = items; // Throwable item (if matched)
        for ci in class_items {
            if !all_items.iter().any(|existing| existing.label == ci.label) {
                all_items.push(ci);
            }
        }
        if all_items.is_empty() {
            None
        } else {
            let items = if paren_follows_cursor(content, position) {
                strip_snippet_parens(all_items)
            } else {
                all_items
            };
            Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items,
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
        uri: &str,
    ) -> Option<CompletionResponse> {
        let partial = Self::extract_partial_class_name(content, position)?;
        if !Self::is_throw_new_context(content, position) {
            return None;
        }
        let (class_items, class_incomplete) =
            self.build_catch_class_name_completions(ctx, &partial, content, true, position, uri);
        if class_items.is_empty() {
            None
        } else {
            let items = if paren_follows_cursor(content, position) {
                strip_snippet_parens(class_items)
            } else {
                class_items
            };
            Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items,
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
    /// Suggest the filename (without `.php` extension) as the class name
    /// when the cursor is inside a class/interface/trait/enum declaration.
    ///
    /// Returns a single completion item so the user can quickly name the
    /// class to match the file, following PSR-4 conventions.
    fn try_class_declaration_completion(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<CompletionResponse> {
        if !is_class_declaration_name(content, position) {
            return None;
        }

        let name = Self::filename_class_name(uri)?;

        let item = CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Match filename".to_string()),
            insert_text: Some(name),
            ..CompletionItem::default()
        };

        Some(CompletionResponse::Array(vec![item]))
    }

    /// Extract the filename without extension from a `file://` URI.
    ///
    /// For example, `file:///home/user/Test.php` returns `Some("Test")`.
    fn filename_class_name(uri: &str) -> Option<String> {
        let url = Url::parse(uri).ok()?;
        let file_path = url.to_file_path().ok()?;
        let stem = file_path.file_stem()?;
        let name = stem.to_string_lossy();
        if name.is_empty() {
            return None;
        }
        Some(name.into_owned())
    }

    fn try_class_constant_function_completion(
        &self,
        content: &str,
        position: Position,
        ctx: &FileContext,
        current_uri: &str,
    ) -> Option<CompletionResponse> {
        if let Some(partial) = enum_backing_type_partial(content, position) {
            let items =
                crate::completion::keyword_completion::build_backed_enum_type_completions(&partial);
            if items.is_empty() {
                return None;
            }
            return Some(CompletionResponse::Array(items));
        }

        let class_ctx = detect_class_name_context(content, position);
        let keyword_ctx = self.keyword_context_for_position(current_uri, content, position, ctx);
        let partial = match Self::extract_partial_class_name(content, position) {
            Some(p) => p,
            None => {
                // Allow keyword completion on empty prefix inside class-like
                // bodies (e.g. after typing `public `).
                if keyword_ctx.after_member_modifier_chain {
                    let items = crate::completion::keyword_completion::build_keyword_completions(
                        "",
                        class_ctx,
                        keyword_ctx,
                    );
                    if items.is_empty() {
                        return None;
                    }
                    return Some(CompletionResponse::Array(items));
                }
                return None;
            }
        };

        // ── `use function` → only functions ─────────────────────────
        if matches!(class_ctx, ClassNameContext::UseFunction) {
            let (function_items, func_incomplete) = self.build_function_completions(
                &partial,
                true,
                Some(content),
                &ctx.namespace,
                current_uri,
            );
            // Filter out functions defined in the current file.
            let function_items = filter_current_file_functions(function_items, current_uri, self);
            let items = append_semicolon_to_insert_text(function_items);
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: func_incomplete,
                items,
            }));
        }

        // ── `use const` → only constants ────────────────────────────
        if matches!(class_ctx, ClassNameContext::UseConst) {
            let (constant_items, const_incomplete) =
                self.build_constant_completions(&partial, current_uri, position);
            // Filter out constants defined in the current file.
            let constant_items = filter_current_file_constants(constant_items, current_uri, self);
            let items = append_semicolon_to_insert_text(constant_items);
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: const_incomplete,
                items,
            }));
        }

        // ── `namespace` declaration → only namespace names ──────────
        if matches!(class_ctx, ClassNameContext::NamespaceDeclaration) {
            let (ns_items, ns_incomplete) = self.build_namespace_completions(&partial, position);
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: ns_incomplete,
                items: ns_items,
            }));
        }

        // For `use` imports, pass an empty use_map: the file's own
        // use_map contains the half-typed line (e.g. `use c` → "c")
        // which would appear as a bogus completion item.  Existing
        // imports are irrelevant when writing a new use statement.
        let (use_map_for_completion, affinity_override) =
            if matches!(class_ctx, ClassNameContext::UseImport) {
                // Pass an empty use_map so the half-typed `use` line
                // doesn't appear as a bogus completion item, but build
                // the affinity table from the *real* use-map so that
                // tier-2 candidates are still ranked by namespace affinity.
                let table = crate::completion::class_completion::build_affinity_table(
                    &ctx.use_map,
                    &ctx.namespace,
                );
                (&HashMap::new() as &HashMap<String, String>, Some(table))
            } else {
                (&ctx.use_map, None)
            };

        let (class_items, class_incomplete) =
            self.build_class_name_completions(ClassCompletionParams {
                file_use_map: use_map_for_completion,
                file_namespace: &ctx.namespace,
                prefix: &partial,
                content,
                context: class_ctx,
                position,
                affinity_table_override: affinity_override,
                uri: current_uri,
            });

        // ── `use` (class import) → classes + keyword hints ──────────
        if matches!(class_ctx, ClassNameContext::UseImport) {
            // Filter out classes defined in the current file.
            let class_items = filter_current_file_classes(class_items, ctx);
            // Filter out classes that are already imported via `use`.
            let already_imported: std::collections::HashSet<&str> =
                ctx.use_map.values().map(|v| v.as_str()).collect();
            let class_items: Vec<CompletionItem> = class_items
                .into_iter()
                .filter(|item| {
                    item.detail
                        .as_deref()
                        .is_none_or(|fqn| !already_imported.contains(fqn))
                })
                .collect();
            let mut items = append_semicolon_to_insert_text(class_items);
            // Inject `function` / `const` keyword suggestions when the
            // partial is a case-sensitive prefix of the keyword.  This
            // lets the user type `use f` → select "function" → continue
            // with a function name.
            if "function".starts_with(&partial) {
                items.insert(
                    0,
                    CompletionItem {
                        label: "function".to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some("use function import".to_string()),
                        insert_text: Some("function ".to_string()),
                        filter_text: Some("function".to_string()),
                        sort_text: Some("0_!function".to_string()),
                        ..CompletionItem::default()
                    },
                );
            }
            if "const".starts_with(&partial) {
                items.insert(
                    0,
                    CompletionItem {
                        label: "const".to_string(),
                        kind: Some(CompletionItemKind::KEYWORD),
                        detail: Some("use const import".to_string()),
                        insert_text: Some("const ".to_string()),
                        filter_text: Some("const".to_string()),
                        sort_text: Some("0_!const".to_string()),
                        ..CompletionItem::default()
                    },
                );
            }
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items,
            }));
        }

        // In restricted contexts (new, extends, implements, use,
        // instanceof), only class names are valid — skip constants
        // and functions.
        if class_ctx.is_class_only() {
            if class_items.is_empty() {
                return None;
            }
            let items = if paren_follows_cursor(content, position) {
                strip_snippet_parens(class_items)
            } else {
                class_items
            };
            return Some(CompletionResponse::List(CompletionList {
                is_incomplete: class_incomplete,
                items,
            }));
        }

        let keyword_items = crate::completion::keyword_completion::build_keyword_completions(
            &partial,
            class_ctx,
            keyword_ctx,
        );
        let (constant_items, const_incomplete) =
            self.build_constant_completions(&partial, current_uri, position);
        let (function_items, func_incomplete) = self.build_function_completions(
            &partial,
            false,
            Some(content),
            &ctx.namespace,
            current_uri,
        );

        if class_items.is_empty()
            && keyword_items.is_empty()
            && constant_items.is_empty()
            && function_items.is_empty()
        {
            return None;
        }

        let mut items = keyword_items;
        items.extend(class_items);
        items.extend(constant_items);
        items.extend(function_items);

        // Strip snippet parentheses when `(` already follows the cursor
        // (e.g. `array_map|()` or `new ClassName|()`).
        let items = if paren_follows_cursor(content, position) {
            strip_snippet_parens(items)
        } else {
            items
        };

        Some(CompletionResponse::List(CompletionList {
            is_incomplete: class_incomplete || const_incomplete || func_incomplete,
            items,
        }))
    }

    /// Build contextual keyword flags from the precomputed symbol map.
    fn keyword_context_for_position(
        &self,
        uri: &str,
        content: &str,
        position: Position,
        file_ctx: &FileContext,
    ) -> crate::completion::keyword_completion::KeywordContext {
        let cursor_offset = position_to_offset(content, position);
        let decl_kind = declaration_header_kind(content, position);
        let maps = self.symbol_maps.read();
        let map = maps.get(uri);
        let in_function_like = map.is_some_and(|m| m.is_inside_function_like_scope(cursor_offset));
        let in_breakable = map.is_some_and(|m| m.is_inside_breakable_scope(cursor_offset));
        let in_loop = map.is_some_and(|m| m.is_inside_loop_scope(cursor_offset));
        let in_switch = map.is_some_and(|m| m.is_inside_switch_scope(cursor_offset));
        let class_at_cursor = find_class_at_offset(&file_ctx.classes, cursor_offset);
        let in_class_like = class_at_cursor.is_some();
        let class_body_kind = if in_function_like {
            None
        } else {
            class_at_cursor.map(|c| c.kind)
        };
        let in_top_level =
            !in_function_like && !in_class_like && brace_depth_before(content, position) == 0;
        let after_member_modifier_chain =
            class_body_kind.is_some() && is_after_member_modifier_chain(content, position);

        crate::completion::keyword_completion::KeywordContext {
            in_function_like,
            in_breakable,
            in_loop,
            in_switch,
            in_top_level,
            in_extends_declaration_header: decl_kind.is_some(),
            in_implements_declaration_header: matches!(
                decl_kind,
                Some(DeclarationHeaderKind::Class | DeclarationHeaderKind::Enum)
            ),
            class_body_kind,
            after_member_modifier_chain,
        }
    }

    // ─── Shared helpers ─────────────────────────────────────────────────

    /// Insert `);` at the given cursor position in `content`.
    ///
    /// This produces a patched version of the source that the parser can
    /// handle when the user is in the middle of typing a function call
    /// (e.g. `$this->greet(|` where the closing `)` hasn't been typed
    /// yet).  Closing the call expression lets the parser recover the
    /// surrounding class/function structure.
    /// Patch incomplete member-access expressions for parser recovery.
    ///
    /// When the cursor is right after `->` or `?->` and the line has no
    /// semicolon, the PHP parser may fail to recognise the enclosing
    /// statement (e.g. an arrow function body).  This inserts a dummy
    /// identifier and semicolon (`_x;`) at the cursor so the parser can
    /// recover the surrounding structure.
    fn patch_incomplete_member_access(content: &str, position: Position) -> String {
        let line_idx = position.line as usize;
        let col = position.character as usize;
        let mut result = String::with_capacity(content.len() + 4);

        for (i, line) in content.lines().enumerate() {
            if i == line_idx {
                let byte_col = line
                    .char_indices()
                    .nth(col)
                    .map(|(idx, _)| idx)
                    .unwrap_or(line.len());
                // Only patch when the cursor is right after `->` or
                // `?->` with nothing meaningful following it.
                let before = &line[..byte_col];
                let after = line[byte_col..].trim();
                if (before.ends_with("->") || before.ends_with("?->")) && after.is_empty() {
                    result.push_str(before);
                    result.push_str("_x;");
                    result.push_str(&line[byte_col..]);
                } else {
                    result.push_str(line);
                }
            } else {
                result.push_str(line);
            }
            result.push('\n');
        }

        if !content.ends_with('\n') && result.ends_with('\n') {
            result.pop();
        }

        result
    }

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
    ///
    /// Delegates to the shared [`Backend::resolve_callable_target`] and
    /// extracts just the parameters from the result.
    fn resolve_named_arg_params(
        &self,
        ctx: &crate::completion::named_args::NamedArgContext,
        content: &str,
        position: Position,
        file_ctx: &FileContext,
    ) -> Vec<crate::types::ParameterInfo> {
        self.resolve_callable_target(&ctx.call_expression, content, position, file_ctx)
            .map(|r| r.parameters)
            .unwrap_or_default()
    }

    /// Extract a [`CompletionTarget`] from the symbol map's precomputed
    /// `MemberAccess` data.
    ///
    /// Returns `None` when the symbol map has no `MemberAccess` at or
    /// just before the cursor (e.g. the AST is broken at the cursor
    /// position).  The caller should fall back to text-based extraction.
    fn extract_completion_target_from_symbol_map(
        &self,
        uri: &str,
        content: &str,
        position: Position,
    ) -> Option<CompletionTarget> {
        let maps = self.symbol_maps.read();
        let map = maps.get(uri)?;
        let cursor_offset = position_to_offset(content, position);

        // The cursor may be at the end of a partially-typed member name
        // (e.g. `$obj->get|`), so the MemberAccess span may end before
        // the cursor.  Walk backward through identifier characters from
        // the cursor to find where the member name starts, then look up
        // the span that starts at or contains the access operator.
        let bytes = content.as_bytes();
        let mut search_offset = cursor_offset as usize;
        while search_offset > 0 && {
            let b = bytes[search_offset - 1];
            b.is_ascii_alphanumeric() || b == b'_'
        } {
            search_offset -= 1;
        }

        // Check for `->` or `?->` before the member name start
        let has_arrow = search_offset >= 2
            && bytes[search_offset - 2] == b'-'
            && bytes[search_offset - 1] == b'>';
        let has_nullsafe_arrow = search_offset >= 3
            && bytes[search_offset - 3] == b'?'
            && bytes[search_offset - 2] == b'-'
            && bytes[search_offset - 1] == b'>';
        let has_double_colon = search_offset >= 2
            && bytes[search_offset - 2] == b':'
            && bytes[search_offset - 1] == b':';

        if !has_arrow && !has_nullsafe_arrow && !has_double_colon {
            return None;
        }

        // Look up the operator position in the symbol map.  For `->` the
        // span covers the subject + operator + member, so the operator
        // byte is within the span.  We look up a byte inside the
        // operator to find the MemberAccess span.
        let operator_offset = if has_nullsafe_arrow {
            (search_offset - 3) as u32
        } else {
            (search_offset - 2) as u32
        };

        if let Some(span) = map.lookup(operator_offset)
            && let SymbolKind::MemberAccess {
                subject_text,
                is_static,
                ..
            } = &span.kind
        {
            let access_kind = if *is_static {
                crate::AccessKind::DoubleColon
            } else {
                crate::AccessKind::Arrow
            };
            return Some(CompletionTarget {
                access_kind,
                subject: subject_text.clone(),
            });
        }

        None
    }
}
