/// Class name, constant, and function completions.
///
/// This module handles building completion items for bare identifiers
/// (class names, global constants, and standalone functions) when no
/// member-access operator (`->` or `::`) is present.
use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::*;

use super::builder::{build_use_edit, find_use_insert_position};

impl Backend {
    /// Extract the partial identifier (class name fragment) that the user
    /// is currently typing at the given cursor position.
    ///
    /// Walks backward from the cursor through alphanumeric characters,
    /// underscores, and backslashes (namespace separators).  Returns
    /// `None` if the resulting text starts with `$` (variable context)
    /// or is empty.
    pub fn extract_partial_class_name(content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backwards through identifier characters (including `\`)
        let mut i = col;
        while i > 0
            && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '\\')
        {
            i -= 1;
        }

        if i == col {
            // Nothing typed — no partial identifier
            return None;
        }

        // If preceded by `$`, this is a variable, not a class name
        if i > 0 && chars[i - 1] == '$' {
            return None;
        }

        // If preceded by `->` or `::`, member completion handles this
        if i >= 2 && chars[i - 2] == '-' && chars[i - 1] == '>' {
            return None;
        }
        if i >= 2 && chars[i - 2] == ':' && chars[i - 1] == ':' {
            return None;
        }

        let partial: String = chars[i..col].iter().collect();
        if partial.is_empty() {
            return None;
        }

        Some(partial)
    }

    /// Build completion items for class names from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Classes imported via `use` statements in the current file
    ///   2. Classes in the same namespace (from the ast_map)
    ///   3. Classes from the class_index (discovered during parsing)
    ///   4. Classes from the Composer classmap (`autoload_classmap.php`)
    ///   5. Built-in PHP classes from embedded stubs
    ///
    /// Each item uses the short class name as `label` and the
    /// fully-qualified name as `detail`.  Items are deduplicated by FQN.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching classes exceeds [`MAX_CLASS_COMPLETIONS`], the result is
    /// truncated and `is_incomplete` is `true`, signalling the client to
    /// re-request as the user types more characters.
    const MAX_CLASS_COMPLETIONS: usize = 100;

    pub(crate) fn build_class_name_completions(
        &self,
        file_use_map: &HashMap<String, String>,
        file_namespace: &Option<String>,
        prefix: &str,
        content: &str,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.to_lowercase();
        let mut seen_fqns: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // Pre-compute the insertion position for `use` statements.
        // Only items from sources 3–5 (not already imported, not same
        // namespace) will carry an `additional_text_edits` entry.
        let use_insert_pos = find_use_insert_position(content);

        // ── 1. Use-imported classes (highest priority) ──────────────
        for (short_name, fqn) in file_use_map {
            if !short_name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen_fqns.insert(fqn.clone()) {
                continue;
            }
            items.push(CompletionItem {
                label: short_name.clone(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(fqn.clone()),
                insert_text: Some(short_name.clone()),
                filter_text: Some(short_name.clone()),
                sort_text: Some(format!("0_{}", short_name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        // ── 2. Same-namespace classes (from ast_map) ────────────────
        if let Some(ns) = file_namespace
            && let Ok(nmap) = self.namespace_map.lock()
        {
            // Find all URIs that share the same namespace
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

            if let Ok(amap) = self.ast_map.lock() {
                for uri in &same_ns_uris {
                    if let Some(classes) = amap.get(uri) {
                        for cls in classes {
                            if !cls.name.to_lowercase().contains(&prefix_lower) {
                                continue;
                            }
                            let fqn = format!("{}\\{}", ns, cls.name);
                            if !seen_fqns.insert(fqn.clone()) {
                                continue;
                            }
                            items.push(CompletionItem {
                                label: cls.name.clone(),
                                kind: Some(CompletionItemKind::CLASS),
                                detail: Some(fqn),
                                insert_text: Some(cls.name.clone()),
                                filter_text: Some(cls.name.clone()),
                                sort_text: Some(format!("1_{}", cls.name.to_lowercase())),
                                deprecated: if cls.is_deprecated { Some(true) } else { None },
                                ..CompletionItem::default()
                            });
                        }
                    }
                }
            }
        }

        // ── 3. class_index (discovered / interacted-with classes) ───
        if let Ok(idx) = self.class_index.lock() {
            for fqn in idx.keys() {
                let short_name = fqn.rsplit('\\').next().unwrap_or(fqn);
                if !short_name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                items.push(CompletionItem {
                    label: short_name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(short_name.to_string()),
                    filter_text: Some(short_name.to_string()),
                    sort_text: Some(format!("2_{}", short_name.to_lowercase())),
                    additional_text_edits: build_use_edit(fqn, use_insert_pos, file_namespace),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 4. Composer classmap (all autoloaded classes) ───────────
        if let Ok(cmap) = self.classmap.lock() {
            for fqn in cmap.keys() {
                let short_name = fqn.rsplit('\\').next().unwrap_or(fqn);
                if !short_name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen_fqns.insert(fqn.clone()) {
                    continue;
                }
                items.push(CompletionItem {
                    label: short_name.to_string(),
                    kind: Some(CompletionItemKind::CLASS),
                    detail: Some(fqn.clone()),
                    insert_text: Some(short_name.to_string()),
                    filter_text: Some(short_name.to_string()),
                    sort_text: Some(format!("3_{}", short_name.to_lowercase())),
                    additional_text_edits: build_use_edit(fqn, use_insert_pos, file_namespace),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 5. Built-in PHP classes from stubs (lowest priority) ────
        for &name in self.stub_index.keys() {
            let short_name = name.rsplit('\\').next().unwrap_or(name);
            if !short_name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen_fqns.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: short_name.to_string(),
                kind: Some(CompletionItemKind::CLASS),
                detail: Some(name.to_string()),
                insert_text: Some(short_name.to_string()),
                filter_text: Some(short_name.to_string()),
                sort_text: Some(format!("4_{}", short_name.to_lowercase())),
                additional_text_edits: build_use_edit(name, use_insert_pos, file_namespace),
                ..CompletionItem::default()
            });
        }

        // Cap the result set so the client isn't overwhelmed.
        // Sort by sort_text first so that higher-priority items
        // (use-imports, same-namespace, user project classes) survive
        // the truncation ahead of lower-priority SPL stubs.
        let is_incomplete = items.len() > Self::MAX_CLASS_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CLASS_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Constant name completion ───────────────────────────────────

    /// Build completion items for standalone constants (`define()` constants)
    /// from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Constants discovered from parsed files (`global_defines`)
    ///   2. Built-in PHP constants from embedded stubs (`stub_constant_index`)
    ///
    /// Each item uses the constant name as `label` and the source as `detail`.
    /// Items are deduplicated by name.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching constants exceeds [`MAX_CONSTANT_COMPLETIONS`], the result
    /// is truncated and `is_incomplete` is `true`.
    const MAX_CONSTANT_COMPLETIONS: usize = 100;

    pub(crate) fn build_constant_completions(&self, prefix: &str) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // ── 1. User-defined constants (from parsed files) ───────────
        if let Ok(dmap) = self.global_defines.lock() {
            for (name, _uri) in dmap.iter() {
                if !name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                if !seen.insert(name.clone()) {
                    continue;
                }
                items.push(CompletionItem {
                    label: name.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some("define constant".to_string()),
                    insert_text: Some(name.clone()),
                    filter_text: Some(name.clone()),
                    sort_text: Some(format!("5_{}", name.to_lowercase())),
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Built-in PHP constants from stubs ────────────────────
        for &name in self.stub_constant_index.keys() {
            if !name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: Some("PHP constant".to_string()),
                insert_text: Some(name.to_string()),
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("6_{}", name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_CONSTANT_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_CONSTANT_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    // ─── Function name completion ───────────────────────────────────

    /// Build a label showing the full function signature.
    ///
    /// Example: `array_map(callable|null $callback, array $array, array ...$arrays): array`
    pub(crate) fn build_function_label(func: &FunctionInfo) -> String {
        let params: Vec<String> = func
            .parameters
            .iter()
            .map(|p| {
                let mut parts = Vec::new();
                if let Some(ref th) = p.type_hint {
                    parts.push(th.clone());
                }
                if p.is_reference {
                    parts.push(format!("&{}", p.name));
                } else if p.is_variadic {
                    parts.push(format!("...{}", p.name));
                } else {
                    parts.push(p.name.clone());
                }
                let param_str = parts.join(" ");
                if !p.is_required && !p.is_variadic {
                    format!("{} = ...", param_str)
                } else {
                    param_str
                }
            })
            .collect();

        let ret = func
            .return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();

        format!("{}({}){}", func.name, params.join(", "), ret)
    }

    /// Build completion items for standalone functions from all known sources.
    ///
    /// Sources (in priority order):
    ///   1. Functions discovered from parsed files (`global_functions`)
    ///   2. Built-in PHP functions from embedded stubs (`stub_function_index`)
    ///
    /// For user-defined functions (source 1), the full signature is shown in
    /// the label because we already have a parsed `FunctionInfo`.  For stub
    /// functions (source 2), only the function name is shown to avoid the
    /// cost of parsing every matching stub at completion time.
    ///
    /// Returns `(items, is_incomplete)`.  When the total number of
    /// matching functions exceeds [`MAX_FUNCTION_COMPLETIONS`], the result
    /// is truncated and `is_incomplete` is `true`.
    const MAX_FUNCTION_COMPLETIONS: usize = 100;

    pub(crate) fn build_function_completions(&self, prefix: &str) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // ── 1. User-defined functions (from parsed files) ───────────
        if let Ok(fmap) = self.global_functions.lock() {
            for (name, (_uri, info)) in fmap.iter() {
                if !name.to_lowercase().contains(&prefix_lower) {
                    continue;
                }
                // Use the short name for deduplication — if a user-defined
                // function shadows a built-in, the user version wins.
                if !seen.insert(info.name.clone()) {
                    continue;
                }
                let label = Self::build_function_label(info);
                items.push(CompletionItem {
                    label,
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("function".to_string()),
                    insert_text: Some(info.name.clone()),
                    filter_text: Some(info.name.clone()),
                    sort_text: Some(format!("4_{}", info.name.to_lowercase())),
                    deprecated: if info.is_deprecated { Some(true) } else { None },
                    ..CompletionItem::default()
                });
            }
        }

        // ── 2. Built-in PHP functions from stubs ────────────────────
        for &name in self.stub_function_index.keys() {
            if !name.to_lowercase().contains(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::FUNCTION),
                detail: Some("PHP function".to_string()),
                insert_text: Some(name.to_string()),
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("5_{}", name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_FUNCTION_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_FUNCTION_COMPLETIONS);
        }

        (items, is_incomplete)
    }
}
