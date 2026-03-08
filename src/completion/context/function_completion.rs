/// Standalone function name completions.
///
/// This module builds completion items for global and namespaced
/// functions (both user-defined and built-in PHP stubs).
use std::collections::HashSet;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::*;
use crate::util::short_name;

use crate::completion::builder::{analyze_use_block, build_callable_snippet};
use crate::completion::use_edit::build_use_function_edit;

impl Backend {
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

    /// Build completion items for standalone functions matching `prefix`.
    ///
    /// When `for_use_import` is `true` the items are tailored for a
    /// `use function` statement: the insert text is the FQN (so that
    /// `use function FQN;` is produced) and no parentheses are appended.
    ///
    /// When `for_use_import` is `false`, namespaced functions get an
    /// `additional_text_edits` entry that inserts `use function FQN;`
    /// at the correct position, mirroring how class auto-import works.
    /// The `content` and `file_namespace` parameters are required for
    /// this auto-import; pass `None` / empty when not needed.
    pub(crate) fn build_function_completions(
        &self,
        prefix: &str,
        for_use_import: bool,
        content: Option<&str>,
        file_namespace: &Option<String>,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.strip_prefix('\\').unwrap_or(prefix).to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        // Pre-compute use-block info for auto-import insertion.
        let use_block = content.map(analyze_use_block);

        // ── 1. User-defined functions (from parsed files) ───────────
        {
            let fmap = self.global_functions.read();
            for (key, (_uri, info)) in fmap.iter() {
                // Match against both the FQN (key) and the short name so
                // that typing either finds the function.
                if !key.to_lowercase().contains(&prefix_lower)
                    && !info.name.to_lowercase().contains(&prefix_lower)
                {
                    continue;
                }
                // Deduplicate on the map key (FQN for namespaced
                // functions, bare name for global ones).  User-defined
                // functions run first, so they shadow same-named stubs.
                if !seen.insert(key.clone()) {
                    continue;
                }

                let is_namespaced = info.namespace.is_some();
                let fqn = key.clone();

                if for_use_import {
                    // `use function` context: insert the FQN so the
                    // resulting statement reads `use function FQN;`.
                    let label = if is_namespaced {
                        fqn.clone()
                    } else {
                        Self::build_function_label(info)
                    };
                    let detail = if is_namespaced {
                        Some(Self::build_function_label(info))
                    } else {
                        Some("function".to_string())
                    };
                    items.push(CompletionItem {
                        label,
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail,
                        insert_text: Some(fqn.clone()),
                        filter_text: Some(fqn.clone()),
                        sort_text: Some(format!("4_{}", fqn.to_lowercase())),
                        deprecated: if info.deprecation_message.is_some() {
                            Some(true)
                        } else {
                            None
                        },
                        ..CompletionItem::default()
                    });
                } else {
                    // Inline context: insert the short name (with snippet
                    // placeholders) and auto-import the FQN.
                    let label = Self::build_function_label(info);
                    let detail = if let Some(ref ns) = info.namespace {
                        format!("function ({})", ns)
                    } else {
                        "function".to_string()
                    };
                    // No import needed when the function lives in the
                    // same namespace as the current file.
                    let same_ns = file_namespace
                        .as_ref()
                        .zip(info.namespace.as_ref())
                        .is_some_and(|(file_ns, func_ns)| file_ns.eq_ignore_ascii_case(func_ns));
                    let additional_text_edits = if is_namespaced && !same_ns {
                        use_block
                            .as_ref()
                            .and_then(|ub| build_use_function_edit(&fqn, ub))
                    } else {
                        None
                    };
                    items.push(CompletionItem {
                        label,
                        kind: Some(CompletionItemKind::FUNCTION),
                        detail: Some(detail),
                        insert_text: Some(build_callable_snippet(&info.name, &info.parameters)),
                        insert_text_format: Some(InsertTextFormat::SNIPPET),
                        filter_text: Some(info.name.clone()),
                        sort_text: Some(format!("4_{}", info.name.to_lowercase())),
                        deprecated: if info.deprecation_message.is_some() {
                            Some(true)
                        } else {
                            None
                        },
                        additional_text_edits,
                        ..CompletionItem::default()
                    });
                }
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

            let is_namespaced = name.contains('\\');
            let sn = if is_namespaced {
                short_name(name)
            } else {
                name
            };

            if for_use_import {
                items.push(CompletionItem {
                    label: name.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some("PHP function".to_string()),
                    insert_text: Some(name.to_string()),
                    filter_text: Some(name.to_string()),
                    sort_text: Some(format!("5_{}", name.to_lowercase())),
                    ..CompletionItem::default()
                });
            } else {
                let detail = if is_namespaced {
                    let ns = &name[..name.rfind('\\').unwrap()];
                    format!("PHP function ({})", ns)
                } else {
                    "PHP function".to_string()
                };
                let additional_text_edits = if is_namespaced {
                    use_block
                        .as_ref()
                        .and_then(|ub| build_use_function_edit(name, ub))
                } else {
                    None
                };
                items.push(CompletionItem {
                    label: sn.to_string(),
                    kind: Some(CompletionItemKind::FUNCTION),
                    detail: Some(detail),
                    insert_text: Some(format!("{sn}()$0")),
                    insert_text_format: Some(InsertTextFormat::SNIPPET),
                    filter_text: Some(sn.to_string()),
                    sort_text: Some(format!("5_{}", sn.to_lowercase())),
                    additional_text_edits,
                    ..CompletionItem::default()
                });
            }
        }

        let is_incomplete = items.len() > Self::MAX_FUNCTION_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_FUNCTION_COMPLETIONS);
        }

        (items, is_incomplete)
    }
}
