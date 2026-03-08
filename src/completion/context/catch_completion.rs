//! Smart catch clause exception completion.
//!
//! When the cursor is inside `catch (|)` or `catch (Partial|)`, this
//! module analyses the corresponding `try` block to find all exception
//! types that can be thrown, and suggests them as completions.
//!
//! Sources of thrown exception types (in priority order):
//!   1. `throw new ExceptionType(…)` statements in the try block
//!   2. Inline `/** @throws ExceptionType */` annotations in the try block
//!   3. Propagated `@throws` from methods called in the try block
//!   4. `throw $this->method()` / `throw self::method()` return types
//!
//! The inline `/** @throws */` annotation is an escape hatch that lets
//! developers document exceptions from dependencies that don't have
//! `@throws` tags themselves.
//!
//! Also provides a Throwable-filtered class completion variant for catch
//! clause fallback and `throw new` completion, which only suggests
//! exception classes from already-parsed sources and includes everything
//! else (classmap, stubs) unfiltered.

use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::*;
use crate::util::short_name;

use super::class_completion::{
    ClassItemCtx, ClassItemTexts, class_completion_texts, is_anonymous_class, matches_class_prefix,
};
use crate::completion::builder::analyze_use_block;
use crate::completion::source::comment_position::position_to_byte_offset;
use crate::completion::source::throws_analysis;

/// Information about the catch clause context at the cursor position.
#[derive(Debug)]
pub(crate) struct CatchContext {
    /// The partial class name the user has typed so far (may be empty).
    pub partial: String,
    /// Exception type names found in the corresponding try block.
    pub suggested_types: Vec<String>,
    /// Whether specific thrown types were discovered in the try block.
    /// When `false`, the caller should fall back to generic class
    /// completion instead of showing only `Throwable`.
    pub has_specific_types: bool,
}

/// Detect whether the cursor is inside a `catch (…)` clause's type
/// position, and if so, return a [`CatchContext`] with the try block's
/// thrown exception types.
///
/// Returns `None` if the cursor is not in a catch clause type position.
pub(crate) fn detect_catch_context(content: &str, position: Position) -> Option<CatchContext> {
    let byte_offset = position_to_byte_offset(content, position);
    let before_cursor = &content[..byte_offset.min(content.len())];

    // Walk backward from cursor to find the opening `(` of the catch clause,
    // collecting what's been typed so far.
    let (catch_paren_offset, partial, already_listed) = find_catch_paren(before_cursor)?;

    // From the `(` position, walk backward to find the `catch` keyword.
    let before_paren = &content[..catch_paren_offset];
    let trimmed = before_paren.trim_end();
    if !trimmed.ends_with("catch") {
        return None;
    }

    // Verify `catch` is a whole word
    let catch_end = trimmed.len();
    let catch_start = catch_end - 5;
    if catch_start > 0 {
        let prev_byte = trimmed.as_bytes()[catch_start - 1];
        if prev_byte.is_ascii_alphanumeric() || prev_byte == b'_' {
            return None;
        }
    }

    // Now find the matching try block by scanning backward from `catch`.
    let before_catch = trimmed[..catch_start].trim_end();

    // The text just before `catch` should be `}` (closing the try block
    // or a previous catch block).
    if !before_catch.ends_with('}') {
        return None;
    }

    // Find the try block: walk back through possible catch/finally blocks
    // to find the original `try {`.
    let try_body = find_try_block_body(content, before_catch)?;

    // Analyse the try block for thrown exception types.
    let mut suggested_types = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. Direct `throw new ExceptionType(…)` statements
    let throws = throws_analysis::find_throw_statements(&try_body);
    for throw in &throws {
        let short_name = throw
            .type_name
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(&throw.type_name);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 2. Inline `/** @throws ExceptionType */` annotations
    let inline_throws = throws_analysis::find_inline_throws_annotations(&try_body);
    for exc_type in &inline_throws {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 3. Propagated @throws from called methods
    let propagated = throws_analysis::find_propagated_throws(&try_body, content);
    let propagated: Vec<String> = propagated.iter().map(|t| t.type_name.clone()).collect();
    for exc_type in &propagated {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // 4. `throw $this->method()` / `throw self::method()` return types
    let throw_expr_types = throws_analysis::find_throw_expression_types(&try_body, content);
    let throw_expr_types: Vec<String> = throw_expr_types
        .iter()
        .map(|t| t.type_name.clone())
        .collect();
    for exc_type in &throw_expr_types {
        let short_name = exc_type
            .trim_start_matches('\\')
            .rsplit('\\')
            .next()
            .unwrap_or(exc_type);
        if !short_name.is_empty() && seen.insert(short_name.to_lowercase()) {
            suggested_types.push(short_name.to_string());
        }
    }

    // Track whether we found any specific thrown types before adding
    // the universal Throwable fallback.
    let has_specific_types = !suggested_types.is_empty();

    // Always offer \Throwable as a catch-all safety net
    if seen.insert("throwable".to_string()) {
        suggested_types.push("\\Throwable".to_string());
    }

    // Filter out types already listed in this catch clause
    let already_lower: Vec<String> = already_listed.iter().map(|s| s.to_lowercase()).collect();
    suggested_types.retain(|t| !already_lower.contains(&t.to_lowercase()));

    Some(CatchContext {
        partial,
        suggested_types,
        has_specific_types,
    })
}

/// Build LSP completion items from a [`CatchContext`].
///
/// Smart exception suggestions sort before any fallback items.
/// `\Throwable` is always offered but sorted last among the suggestions.
pub(crate) fn build_catch_completions(ctx: &CatchContext) -> Vec<CompletionItem> {
    let mut items = Vec::new();
    let partial_lower = ctx.partial.to_lowercase();

    for (idx, exc_type) in ctx.suggested_types.iter().enumerate() {
        let label = exc_type.trim_start_matches('\\');

        // Filter by the partial text the user has typed
        if !partial_lower.is_empty() && !label.to_lowercase().starts_with(&partial_lower) {
            continue;
        }

        // Sort \Throwable after specific exception types
        let sort_text = if exc_type.starts_with('\\') {
            format!("1_{:03}_{}", idx, label)
        } else {
            format!("0_{:03}_{}", idx, label)
        };

        items.push(CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::CLASS),
            detail: Some("Exception thrown in try block".to_string()),
            sort_text: Some(sort_text),
            filter_text: Some(label.to_string()),
            ..CompletionItem::default()
        });
    }

    items
}

// ─── Internal helpers ───────────────────────────────────────────────────────

/// Walk backward from the cursor position to find the `(` of a catch clause.
///
/// Returns `(paren_byte_offset, partial_typed, already_listed_types)` or
/// `None` if no suitable `(` is found.
fn find_catch_paren(before_cursor: &str) -> Option<(usize, String, Vec<String>)> {
    let bytes = before_cursor.as_bytes();
    let mut pos = bytes.len();
    let mut depth = 0i32;

    // Walk backward collecting the text inside the parentheses
    while pos > 0 {
        pos -= 1;
        match bytes[pos] {
            b')' => depth += 1,
            b'(' => {
                if depth == 0 {
                    // Found our opening paren
                    let inside = &before_cursor[pos + 1..];
                    let (partial, already_listed) = parse_catch_paren_content(inside);
                    return Some((pos, partial, already_listed));
                }
                depth -= 1;
            }
            // Stop at semicolons, opening braces — we've gone too far
            b';' | b'{' => return None,
            _ => {}
        }
    }

    None
}

/// Parse the content inside `catch (` up to the cursor.
///
/// For `catch (IOException | ` the partial is `""` and already_listed is
/// `["IOException"]`.
///
/// For `catch (IOEx` the partial is `"IOEx"` and already_listed is `[]`.
///
/// For `catch (IOException | Time` the partial is `"Time"` and
/// already_listed is `["IOException"]`.
fn parse_catch_paren_content(inside: &str) -> (String, Vec<String>) {
    let parts: Vec<&str> = inside.split('|').collect();
    let mut already_listed = Vec::new();

    if parts.len() <= 1 {
        // No `|` separator — everything is the partial
        let partial = inside.trim().trim_start_matches('\\').to_string();
        return (partial, already_listed);
    }

    // Everything except the last segment is an already-listed type
    for part in &parts[..parts.len() - 1] {
        let t = part.trim().trim_start_matches('\\');
        if !t.is_empty() {
            // Strip the variable name if present (shouldn't be before `|`, but be safe)
            let type_name = t.split_whitespace().next().unwrap_or(t);
            if !type_name.starts_with('$') {
                already_listed.push(type_name.to_string());
            }
        }
    }

    // The last segment is the partial
    let last = parts.last().unwrap_or(&"");
    let partial = last.trim().trim_start_matches('\\').to_string();

    (partial, already_listed)
}

/// Find the try block body by walking backward from the `}` that precedes
/// the `catch` keyword.
///
/// Handles chains like `try { … } catch (A $a) { … } catch (|)` by
/// walking back through previous catch (and finally) blocks to find the
/// original `try {`.
fn find_try_block_body(_content: &str, before_catch: &str) -> Option<String> {
    // `before_catch` ends with `}`. Find the matching `{`.
    let close_brace_offset = before_catch.len() - 1;

    // We need the absolute position in `content`. `before_catch` is a
    // prefix of `content` (after trimming), so we can use its length.
    // But actually, `before_catch` was derived by slicing `content`, so
    // we need to find where this `}` is in the full content.
    //
    // Walk backward to find the matching `{`.
    let block_open =
        crate::util::find_matching_backward(before_catch, close_brace_offset, b'{', b'}')?;

    // Now check what keyword precedes this block.
    let before_block = before_catch[..block_open].trim_end();

    // Check for `)` — this block was a catch block
    if before_block.ends_with(')') {
        // Skip the catch clause parentheses
        let close_paren = before_block.len() - 1;
        let open_paren =
            crate::util::find_matching_backward(before_block, close_paren, b'(', b')')?;
        let before_paren = before_block[..open_paren].trim_end();

        // Should be `catch`
        if before_paren.ends_with("catch") {
            let kw_start = before_paren.len() - 5;
            // Verify whole word
            if kw_start == 0
                || (!before_paren.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                    && before_paren.as_bytes()[kw_start - 1] != b'_')
            {
                // Must be preceded by `}` of the previous block
                let before_kw = before_paren[..kw_start].trim_end();
                if before_kw.ends_with('}') {
                    // Recurse to find the try block
                    return find_try_block_body(_content, before_kw);
                }
            }
        }
        return None;
    }

    // Check for `finally`
    if before_block.ends_with("finally") {
        let kw_start = before_block.len() - 7;
        if kw_start == 0
            || (!before_block.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                && before_block.as_bytes()[kw_start - 1] != b'_')
        {
            let before_kw = before_block[..kw_start].trim_end();
            if before_kw.ends_with('}') {
                return find_try_block_body(_content, before_kw);
            }
        }
        return None;
    }

    // Check for `try`
    if before_block.ends_with("try") {
        let kw_start = before_block.len() - 3;
        if kw_start == 0
            || (!before_block.as_bytes()[kw_start - 1].is_ascii_alphanumeric()
                && before_block.as_bytes()[kw_start - 1] != b'_')
        {
            // Found it! Extract the try block body.
            let body = &before_catch[block_open + 1..close_brace_offset];
            return Some(body.to_string());
        }
    }

    None
}

// ─── Throwable-filtered class completion ────────────────────────────────────

impl Backend {
    /// Check whether a class is a confirmed `\Throwable` descendant using
    /// only already-loaded data from the `ast_map`.
    ///
    /// Returns `true` only when the full parent chain can be walked to
    /// one of the three Throwable root types (`Throwable`, `Exception`,
    /// `Error`).  Returns `false` if the chain is broken (parent not
    /// loaded) or terminates at a non-Throwable class.
    ///
    /// This is a strict check: the caller should only include the class
    /// when this returns `true`.
    ///
    /// This never triggers disk I/O; it only consults `ast_map`.
    fn is_throwable_descendant(&self, class_name: &str, depth: u32) -> bool {
        if depth > 20 {
            return false; // prevent infinite loops
        }

        let normalized = class_name.strip_prefix('\\').unwrap_or(class_name);
        let short = short_name(normalized);

        // These three types form the root of PHP's exception hierarchy.
        if matches!(short, "Throwable" | "Exception" | "Error") {
            return true;
        }

        // Look up ClassInfo from ast_map (no disk I/O).
        match self.find_class_in_ast_map(class_name) {
            Some(ci) => match &ci.parent_class {
                Some(parent) => self.is_throwable_descendant(parent, depth + 1),
                None => false, // no parent, not a Throwable type
            },
            None => false, // class not loaded — can't confirm
        }
    }

    /// Check whether the class identified by `class_name` is a concrete,
    /// non-abstract `class` (i.e. `ClassLikeKind::Class` and not
    /// `is_abstract`) in the `ast_map`.
    ///
    /// Returns `false` for interfaces, traits, enums, abstract classes,
    /// and classes that are not currently loaded.  This never triggers
    /// disk I/O.
    fn is_concrete_class_in_ast_map(&self, class_name: &str) -> bool {
        self.find_class_in_ast_map(class_name)
            .is_some_and(|c| c.kind == ClassLikeKind::Class && !c.is_abstract)
    }

    /// Collect the FQN of every class that is currently loaded in the
    /// `ast_map`.  Used by `build_catch_class_name_completions` so that
    /// classmap / stub sources can skip classes we already evaluated.
    fn collect_loaded_fqns(&self) -> HashSet<String> {
        let mut loaded = HashSet::new();
        let amap = self.ast_map.read();
        let nmap = self.namespace_map.read();
        for (uri, classes) in amap.iter() {
            let file_ns = nmap.get(uri).and_then(|opt| opt.as_deref());
            for cls in classes {
                let fqn = if let Some(ns) = file_ns {
                    format!("{}\\{}", ns, cls.name)
                } else {
                    cls.name.clone()
                };
                loaded.insert(fqn);
            }
        }
        loaded
    }

    /// Build completion items for class names, filtered for Throwable
    /// descendants.  Used as the catch clause fallback when no specific
    /// `@throws` types were discovered in the try block, and for
    /// `throw new` completion.
    ///
    /// The logic follows this priority:
    ///
    /// 1. **Loaded concrete classes** (use-imports, same-namespace,
    ///    class_index): only classes (not interfaces/traits/enums) whose
    ///    parent chain is fully walkable to `\Throwable` / `\Exception`
    ///    / `\Error`.
    /// 2. **Classmap** entries (not yet parsed) whose short name ends
    ///    with `Exception` — filtered to exclude already-loaded FQNs.
    /// 3. **Stub** entries whose short name ends with `Exception` —
    ///    filtered to exclude already-loaded FQNs.
    /// 4. **Classmap** entries that do *not* end with `Exception`.
    /// 5. **Stub** entries that do *not* end with `Exception`.
    pub(crate) fn build_catch_class_name_completions(
        &self,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
        prefix: &str,
        content: &str,
        is_new: bool,
        position: Position,
    ) -> (Vec<CompletionItem>, bool) {
        let has_leading_backslash = prefix.starts_with('\\');
        let normalized = prefix.strip_prefix('\\').unwrap_or(prefix);
        let prefix_lower = normalized.to_lowercase();
        let is_fqn_prefix = has_leading_backslash || normalized.contains('\\');

        // When the user is typing a namespace-qualified reference,
        // provide an explicit replacement range so the editor replaces
        // the entire typed prefix (including namespace separators).
        let fqn_replace_range = if is_fqn_prefix {
            Some(Range {
                start: Position {
                    line: position.line,
                    character: position
                        .character
                        .saturating_sub(prefix.chars().count() as u32),
                },
                end: position,
            })
        } else {
            None
        };
        let mut seen_fqns: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        let ctx = ClassItemCtx {
            is_fqn_prefix,
            is_new,
            fqn_replace_range,
            file_use_map,
            use_block: analyze_use_block(content),
            file_namespace,
        };

        // Build the set of every FQN currently in the ast_map so that
        // classmap / stub sources can exclude already-evaluated classes.
        let loaded_fqns = self.collect_loaded_fqns();

        // ── 1a. Use-imported classes (must be concrete + Throwable) ─
        for (short_name, fqn) in file_use_map {
            if !matches_class_prefix(short_name, fqn, &prefix_lower, is_fqn_prefix) {
                continue;
            }
            if !seen_fqns.insert(fqn.clone()) {
                continue;
            }
            // Only concrete classes (not interfaces/traits/enums)
            if !self.is_concrete_class_in_ast_map(fqn) {
                continue;
            }
            // Strict check: only include if confirmed Throwable descendant
            if !self.is_throwable_descendant(fqn, 0) {
                continue;
            }
            let (label, base_name, filter, _use_import) = class_completion_texts(
                short_name,
                fqn,
                is_fqn_prefix,
                has_leading_backslash,
                file_namespace,
                &prefix_lower,
            );
            let (insert_text, insert_text_format) = if is_new {
                Self::build_new_insert(&base_name, None)
            } else {
                (base_name, None)
            };
            items.push(CompletionItem {
                label,
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(fqn.clone()),
                insert_text: Some(insert_text.clone()),
                insert_text_format,
                filter_text: Some(filter),
                sort_text: Some(format!("0_{}", short_name.to_lowercase())),
                text_edit: fqn_replace_range.map(|range| {
                    CompletionTextEdit::Edit(TextEdit {
                        range,
                        new_text: insert_text.clone(),
                    })
                }),
                ..CompletionItem::default()
            });
        }

        // ── 1b. Same-namespace classes (must be concrete + Throwable)
        // Collect candidates while holding the lock, then drop the lock
        // before calling `is_throwable_descendant` (which re-locks
        // `ast_map` internally — Rust's Mutex is not re-entrant).
        if let Some(ns) = file_namespace {
            let nmap = self.namespace_map.read();
            let same_ns_uris: Vec<String> = nmap
                .iter()
                .filter_map(|(uri, opt_ns)| {
                    if opt_ns.as_deref() == Some(ns.as_str()) {
                        Some(uri.clone())
                    } else {
                        None
                    }
                })
                .collect();
            drop(nmap);

            // Phase 1: collect candidate (name, fqn, deprecation_message)
            // tuples under the ast_map lock — only concrete classes.
            let mut candidates: Vec<(String, String, Option<String>)> = Vec::new();
            {
                let amap = self.ast_map.read();
                for uri in &same_ns_uris {
                    if let Some(classes) = amap.get(uri) {
                        for cls in classes {
                            if is_anonymous_class(&cls.name) {
                                continue;
                            }
                            if cls.kind != ClassLikeKind::Class || cls.is_abstract {
                                continue;
                            }
                            let cls_fqn = format!("{}\\{}", ns, cls.name);
                            if !matches_class_prefix(
                                &cls.name,
                                &cls_fqn,
                                &prefix_lower,
                                is_fqn_prefix,
                            ) {
                                continue;
                            }
                            let fqn = cls_fqn;
                            if !seen_fqns.insert(fqn.clone()) {
                                continue;
                            }
                            candidates.push((
                                cls.name.clone(),
                                fqn,
                                cls.deprecation_message.clone(),
                            ));
                        }
                    }
                }
            }
            // Phase 2: filter by Throwable ancestry without holding locks.
            for (name, fqn, deprecation_message) in candidates {
                if !self.is_throwable_descendant(&fqn, 0) {
                    continue;
                }
                let (label, base_name, filter, _use_import) = class_completion_texts(
                    &name,
                    &fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    file_namespace,
                    &prefix_lower,
                );
                let (insert_text, insert_text_format) = if is_new {
                    // Same-namespace classes are collected without
                    // ClassInfo, so we cannot check __construct here.
                    Self::build_new_insert(&base_name, None)
                } else {
                    (base_name, None)
                };
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn),
                    insert_text: Some(insert_text.clone()),
                    insert_text_format,
                    filter_text: Some(filter),
                    sort_text: Some(format!("1_{}", name.to_lowercase())),
                    deprecated: if deprecation_message.is_some() {
                        Some(true)
                    } else {
                        None
                    },
                    text_edit: fqn_replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: insert_text.clone(),
                        })
                    }),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 1c. class_index (must be concrete + Throwable) ──────────
        {
            let idx = self.class_index.read();
            for fqn in idx.keys() {
                let sn = short_name(fqn);
                if !matches_class_prefix(sn, fqn, &prefix_lower, is_fqn_prefix) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                if !self.is_concrete_class_in_ast_map(fqn) {
                    continue;
                }
                if !self.is_throwable_descendant(fqn, 0) {
                    continue;
                }
                let (label, base_name, filter, use_import) = class_completion_texts(
                    sn,
                    fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    file_namespace,
                    &prefix_lower,
                );
                let mut texts = ClassItemTexts {
                    label,
                    base_name,
                    filter,
                    use_import,
                };
                ctx.apply_import_fixups(&mut texts.base_name, &mut texts.use_import, false);
                items.push(ctx.build_item(
                    texts,
                    fqn,
                    format!("2_{}", sn.to_lowercase()),
                    |name| (format!("{name}()$0"), Some(InsertTextFormat::SNIPPET)),
                    false,
                ));
            }
        }

        // ── 2. Classmap — names ending with "Exception" ─────────────
        // ── 4. Classmap — names NOT ending with "Exception" ─────────
        // We collect both buckets in a single pass over the classmap and
        // assign different sort_text prefixes so "Exception" entries
        // appear first.
        {
            let cmap = self.classmap.read();
            for fqn in cmap.keys() {
                if loaded_fqns.contains(fqn) {
                    continue;
                }
                let sn = short_name(fqn);
                if !matches_class_prefix(sn, fqn, &prefix_lower, is_fqn_prefix) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                let prefix_num = if sn.ends_with("Exception") { "3" } else { "5" };
                let (label, base_name, filter, use_import) = class_completion_texts(
                    sn,
                    fqn,
                    is_fqn_prefix,
                    has_leading_backslash,
                    file_namespace,
                    &prefix_lower,
                );
                let mut texts = ClassItemTexts {
                    label,
                    base_name,
                    filter,
                    use_import,
                };
                ctx.apply_import_fixups(&mut texts.base_name, &mut texts.use_import, false);
                items.push(ctx.build_item(
                    texts,
                    fqn,
                    format!("{}_{}", prefix_num, sn.to_lowercase()),
                    |name| (format!("{name}()$0"), Some(InsertTextFormat::SNIPPET)),
                    false,
                ));
            }
        }

        // ── 3. Stubs — names ending with "Exception" ────────────────
        // ── 5. Stubs — names NOT ending with "Exception" ────────────
        for &name in self.stub_index.keys() {
            if loaded_fqns.contains(name) {
                continue;
            }
            let sn = short_name(name);
            if !matches_class_prefix(sn, name, &prefix_lower, is_fqn_prefix) {
                continue;
            }
            if !seen_fqns.insert(name.to_string()) {
                continue;
            }
            let prefix_num = if sn.ends_with("Exception") { "4" } else { "6" };
            let (label, base_name, filter, use_import) = class_completion_texts(
                sn,
                name,
                is_fqn_prefix,
                has_leading_backslash,
                file_namespace,
                &prefix_lower,
            );
            let mut texts = ClassItemTexts {
                label,
                base_name,
                filter,
                use_import,
            };
            ctx.apply_import_fixups(&mut texts.base_name, &mut texts.use_import, false);
            items.push(ctx.build_item(
                texts,
                name,
                format!("{}_{}", prefix_num, sn.to_lowercase()),
                |name| (format!("{name}()$0"), Some(InsertTextFormat::SNIPPET)),
                false,
            ));
        }

        let is_incomplete = items.len() > Self::MAX_CLASS_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CLASS_COMPLETIONS);
        }

        (items, is_incomplete)
    }
}

#[cfg(test)]
#[path = "catch_completion_tests.rs"]
mod tests;
