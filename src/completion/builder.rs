/// Completion item building.
///
/// This module contains the logic for constructing LSP `CompletionItem`s from
/// resolved `ClassInfo`, filtered by the `AccessKind` (arrow vs double-colon).
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::*;

impl Backend {
    /// Build the label showing the full method signature.
    ///
    /// Example: `regularCode(string $text, $frogs = false): string`
    pub(crate) fn build_method_label(method: &MethodInfo) -> String {
        let params: Vec<String> = method
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

        let ret = method
            .return_type
            .as_ref()
            .map(|r| format!(": {}", r))
            .unwrap_or_default();

        format!("{}({}){}", method.name, params.join(", "), ret)
    }

    /// Build completion items for a resolved class, filtered by access kind.
    ///
    /// - `Arrow` access: returns only non-static methods and properties.
    /// - `DoubleColon` access: returns only static methods, static properties, and constants.
    /// - `Other` access: returns all members.
    pub(crate) fn build_completion_items(
        target_class: &ClassInfo,
        access_kind: AccessKind,
    ) -> Vec<CompletionItem> {
        let mut items: Vec<CompletionItem> = Vec::new();

        // Methods — filtered by static / instance
        for method in &target_class.methods {
            let include = match access_kind {
                AccessKind::Arrow => !method.is_static,
                AccessKind::DoubleColon => method.is_static,
                AccessKind::Other => true,
            };
            if !include {
                continue;
            }

            let label = Self::build_method_label(method);
            items.push(CompletionItem {
                label,
                kind: Some(CompletionItemKind::METHOD),
                detail: Some(format!("Class: {}", target_class.name)),
                insert_text: Some(method.name.clone()),
                filter_text: Some(method.name.clone()),
                ..CompletionItem::default()
            });
        }

        // Properties — filtered by static / instance
        for property in &target_class.properties {
            let include = match access_kind {
                AccessKind::Arrow => !property.is_static,
                AccessKind::DoubleColon => property.is_static,
                AccessKind::Other => true,
            };
            if !include {
                continue;
            }

            // Static properties accessed via `::` need the `$` prefix
            // (e.g. `self::$path`), while instance properties via `->`
            // use the bare name (e.g. `$this->path`).
            let display_name = if access_kind == AccessKind::DoubleColon {
                format!("${}", property.name)
            } else {
                property.name.clone()
            };

            let detail = if let Some(ref th) = property.type_hint {
                format!("Class: {} — {}", target_class.name, th)
            } else {
                format!("Class: {}", target_class.name)
            };

            items.push(CompletionItem {
                label: display_name.clone(),
                kind: Some(CompletionItemKind::PROPERTY),
                detail: Some(detail),
                insert_text: Some(display_name.clone()),
                filter_text: Some(display_name),
                ..CompletionItem::default()
            });
        }

        // Constants — only for `::` or unqualified access
        if access_kind == AccessKind::DoubleColon || access_kind == AccessKind::Other {
            for constant in &target_class.constants {
                let detail = if let Some(ref th) = constant.type_hint {
                    format!("Class: {} — {}", target_class.name, th)
                } else {
                    format!("Class: {}", target_class.name)
                };

                items.push(CompletionItem {
                    label: constant.name.clone(),
                    kind: Some(CompletionItemKind::CONSTANT),
                    detail: Some(detail),
                    insert_text: Some(constant.name.clone()),
                    filter_text: Some(constant.name.clone()),
                    ..CompletionItem::default()
                });
            }
        }

        items
    }
}
