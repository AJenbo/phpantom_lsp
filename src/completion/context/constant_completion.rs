/// Global constant name completions.
///
/// This module builds completion items for standalone constants
/// (`define()` constants and built-in PHP constants from stubs).
use std::collections::HashSet;

use tower_lsp::lsp_types::*;

use crate::Backend;

impl Backend {
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

    /// Build completion items for global constants matching `prefix`.
    pub(crate) fn build_constant_completions(&self, prefix: &str) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // ── 1. User-defined constants (from parsed files) ───────────
        {
            let dmap = self.global_defines.read();
            for (name, _) in dmap.iter() {
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
}
