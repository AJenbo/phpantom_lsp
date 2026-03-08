/// Namespace declaration completions.
///
/// This module builds completion items for `namespace` declarations,
/// suggesting namespace names that fall under known PSR-4 prefixes.
use std::collections::HashSet;

use tower_lsp::lsp_types::*;

use crate::Backend;

impl Backend {
    // ─── Namespace declaration completion ───────────────────────────

    /// Maximum number of namespace suggestions to return.
    const MAX_NAMESPACE_COMPLETIONS: usize = 100;

    /// Build completion items for a `namespace` declaration.
    ///
    /// Only namespaces that fall under a known PSR-4 prefix are
    /// suggested.  The sources are:
    ///   1. PSR-4 mapping prefixes themselves (exploded to every level)
    ///   2. Namespace portions of FQNs from `namespace_map`,
    ///      `class_index`, `classmap`, and `ast_map` — but only when
    ///      they start with a PSR-4 prefix.
    ///
    /// Every accepted namespace is exploded to each intermediate level
    /// (e.g. `A\B\C` also inserts `A\B` and `A`).
    ///
    /// Returns `(items, is_incomplete)`.
    pub(crate) fn build_namespace_completions(
        &self,
        prefix: &str,
        position: Position,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.to_lowercase();
        let mut namespaces: HashSet<String> = HashSet::new();

        // Collect the project's own PSR-4 prefixes (without trailing
        // `\`) so we can gate which cache entries are eligible.
        let psr4_prefixes: Vec<String> = {
            let mappings = self.psr4_mappings.read();
            mappings
                .iter()
                .map(|m| m.prefix.trim_end_matches('\\').to_string())
                .filter(|p| !p.is_empty())
                .collect()
        };

        // Helper: insert a namespace and all its parent namespaces.
        fn insert_with_parents(ns: &str, set: &mut HashSet<String>) {
            if ns.is_empty() {
                return;
            }
            set.insert(ns.to_string());
            let mut parts: Vec<&str> = ns.split('\\').collect();
            while parts.len() > 1 {
                parts.pop();
                set.insert(parts.join("\\"));
            }
        }

        /// Check whether `ns` falls under one of the PSR-4 prefixes.
        fn under_psr4(ns: &str, prefixes: &[String]) -> bool {
            prefixes
                .iter()
                .any(|p| ns == p || ns.starts_with(&format!("{}\\", p)))
        }

        // Helper: insert ns (and parents) only if under a PSR-4 prefix.
        fn insert_if_under_psr4(ns: &str, set: &mut HashSet<String>, prefixes: &[String]) {
            if under_psr4(ns, prefixes) {
                insert_with_parents(ns, set);
            }
        }

        // ── 1. PSR-4 prefixes (always included, exploded) ───────────
        for p in &psr4_prefixes {
            insert_with_parents(p, &mut namespaces);
        }

        // ── 2. namespace_map (already-opened files) ─────────────────
        {
            let nmap = self.namespace_map.read();
            for ns in nmap.values().flatten() {
                insert_if_under_psr4(ns, &mut namespaces, &psr4_prefixes);
            }
        }

        // ── 3. ast_map namespace portions ───────────────────────────
        {
            let amap = self.ast_map.read();
            let nmap = self.namespace_map.read();
            for (uri, classes) in amap.iter() {
                let file_ns = nmap.get(uri).and_then(|opt| opt.as_deref());
                if let Some(ns) = file_ns {
                    for cls in classes {
                        let fqn = format!("{}\\{}", ns, cls.name);
                        if let Some(ns_end) = fqn.rfind('\\') {
                            insert_if_under_psr4(&fqn[..ns_end], &mut namespaces, &psr4_prefixes);
                        }
                    }
                }
            }
        }

        // ── 4. class_index + classmap namespace portions ────────────
        {
            let idx = self.class_index.read();
            for fqn in idx.keys() {
                if let Some(ns_end) = fqn.rfind('\\') {
                    insert_if_under_psr4(&fqn[..ns_end], &mut namespaces, &psr4_prefixes);
                }
            }
        }
        {
            let cmap = self.classmap.read();
            for fqn in cmap.keys() {
                if let Some(ns_end) = fqn.rfind('\\') {
                    insert_if_under_psr4(&fqn[..ns_end], &mut namespaces, &psr4_prefixes);
                }
            }
        }

        // When the typed prefix contains a backslash the editor may
        // only replace the segment after the last `\`.  Provide an
        // explicit replacement range covering the entire typed prefix
        // so that picking `Tests\Feature\Domain` after typing
        // `Tests\Feature\D` replaces the whole thing instead of
        // inserting a duplicate prefix.
        let replace_range = if prefix.contains('\\') {
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

        // ── Filter and build items ──────────────────────────────────
        let mut items: Vec<CompletionItem> = namespaces
            .into_iter()
            .filter(|ns| ns.to_lowercase().contains(&prefix_lower))
            .map(|ns| {
                let sn = ns.rsplit('\\').next().unwrap_or(&ns);
                CompletionItem {
                    label: ns.clone(),
                    kind: Some(CompletionItemKind::MODULE),
                    insert_text: Some(ns.clone()),
                    filter_text: Some(ns.clone()),
                    sort_text: Some(format!("0_{}", sn.to_lowercase())),
                    text_edit: replace_range.map(|range| {
                        CompletionTextEdit::Edit(TextEdit {
                            range,
                            new_text: ns,
                        })
                    }),
                    ..CompletionItem::default()
                }
            })
            .collect();

        let is_incomplete = items.len() > Self::MAX_NAMESPACE_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_NAMESPACE_COMPLETIONS);
        }

        (items, is_incomplete)
    }
}
