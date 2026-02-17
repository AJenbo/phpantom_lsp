/// Member completion item building.
///
/// This module contains the logic for constructing LSP `CompletionItem`s from
/// resolved `ClassInfo`, filtered by the `AccessKind` (arrow, double-colon,
/// or parent double-colon).
///
/// Use-statement insertion helpers live in the sibling [`super::use_edit`]
/// module and are re-exported here for backward compatibility.
use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::Visibility;
use crate::types::*;

// Re-export use-statement helpers so existing `use crate::completion::builder::{…}`
// imports continue to work.
pub(crate) use super::use_edit::{build_use_edit, find_use_insert_position};

/// PHP magic methods that should not appear in completion results.
/// These are invoked implicitly by the language runtime rather than
/// called directly by user code.
const MAGIC_METHODS: &[&str] = &[
    "__construct",
    "__destruct",
    "__clone",
    "__get",
    "__set",
    "__isset",
    "__unset",
    "__call",
    "__callStatic",
    "__invoke",
    "__toString",
    "__sleep",
    "__wakeup",
    "__serialize",
    "__unserialize",
    "__set_state",
    "__debugInfo",
];

impl Backend {
    /// Check whether a method name is a PHP magic method that should be
    /// excluded from completion results.
    fn is_magic_method(name: &str) -> bool {
        MAGIC_METHODS.iter().any(|&m| m.eq_ignore_ascii_case(name))
    }

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

    /// Build completion items for a resolved class, filtered by access kind
    /// and visibility scope.
    ///
    /// - `Arrow` access: returns only non-static methods and properties.
    /// - `DoubleColon` access: returns only static methods, static properties, and constants.
    /// - `ParentDoubleColon` access: returns both static and non-static methods,
    ///   static properties, and constants — but excludes private members.
    /// - `Other` access: returns all members.
    ///
    /// Visibility filtering based on `current_class_name`:
    /// - `None` (top-level code): only **public** members are shown.
    /// - `Some(name)` where `name == target_class.name`: all members are shown
    ///   (same-class access, e.g. `$this->`).
    /// - `Some(name)` where `name != target_class.name`: **public** and
    ///   **protected** members are shown (the caller might be in a subclass).
    ///
    /// `is_self_or_ancestor` should be `true` when the cursor is inside the
    /// target class itself or inside a class that (transitively) extends the
    /// target.  When `true`, `__construct` is offered for `::` access
    /// (e.g. `self::__construct()`, `static::__construct()`,
    /// `parent::__construct()`, `ClassName::__construct()` from within a
    /// subclass).  When `false`, magic methods are suppressed entirely.
    pub(crate) fn build_completion_items(
        target_class: &ClassInfo,
        access_kind: AccessKind,
        current_class_name: Option<&str>,
        is_self_or_ancestor: bool,
    ) -> Vec<CompletionItem> {
        // Determine whether we are inside the same class as the target.
        let same_class = current_class_name.is_some_and(|name| name == target_class.name);
        // Inside *some* class (possibly a subclass) — show protected.
        let in_class = current_class_name.is_some();
        let mut items: Vec<CompletionItem> = Vec::new();

        // Methods — filtered by static / instance, excluding magic methods
        for method in &target_class.methods {
            // `__construct` is meaningful to call explicitly via `::` when
            // inside the same class or a subclass (e.g.
            // `parent::__construct(...)`, `self::__construct()`).
            // Outside of that relationship, magic methods are suppressed.
            let is_constructor = method.name.eq_ignore_ascii_case("__construct");
            if Self::is_magic_method(&method.name) {
                let allow = is_constructor
                    && is_self_or_ancestor
                    && matches!(
                        access_kind,
                        AccessKind::DoubleColon | AccessKind::ParentDoubleColon
                    );
                if !allow {
                    continue;
                }
            }

            // Visibility filtering:
            // - private: only visible from within the same class
            // - protected: visible from the same class or a subclass
            //   (we approximate by allowing when inside any class)
            if method.visibility == Visibility::Private && !same_class {
                continue;
            }
            if method.visibility == Visibility::Protected && !same_class && !in_class {
                continue;
            }

            let include = match access_kind {
                AccessKind::Arrow => !method.is_static,
                // External `ClassName::` shows only static methods, but
                // `__construct` is an exception — it's an instance method
                // that is routinely called via `ClassName::__construct()`
                // from within a subclass.
                AccessKind::DoubleColon => method.is_static || is_constructor,
                // `self::`, `static::`, and `parent::` show both static and
                // non-static methods (PHP allows calling instance methods
                // via `::` from within the class hierarchy).
                AccessKind::ParentDoubleColon => true,
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
                deprecated: if method.is_deprecated {
                    Some(true)
                } else {
                    None
                },
                ..CompletionItem::default()
            });
        }

        // Properties — filtered by static / instance
        for property in &target_class.properties {
            if property.visibility == Visibility::Private && !same_class {
                continue;
            }
            if property.visibility == Visibility::Protected && !same_class && !in_class {
                continue;
            }

            let include = match access_kind {
                AccessKind::Arrow => !property.is_static,
                AccessKind::DoubleColon | AccessKind::ParentDoubleColon => property.is_static,
                AccessKind::Other => true,
            };
            if !include {
                continue;
            }

            // Static properties accessed via `::` need the `$` prefix
            // (e.g. `self::$path`, `ClassName::$path`), while instance
            // properties via `->` use the bare name (e.g. `$this->path`).
            let display_name = if access_kind == AccessKind::DoubleColon
                || access_kind == AccessKind::ParentDoubleColon
            {
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
                deprecated: if property.is_deprecated {
                    Some(true)
                } else {
                    None
                },
                ..CompletionItem::default()
            });
        }

        // Constants — only for `::`, `parent::`, or unqualified access
        if access_kind == AccessKind::DoubleColon
            || access_kind == AccessKind::ParentDoubleColon
            || access_kind == AccessKind::Other
        {
            for constant in &target_class.constants {
                if constant.visibility == Visibility::Private && !same_class {
                    continue;
                }
                if constant.visibility == Visibility::Protected && !same_class && !in_class {
                    continue;
                }

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
                    deprecated: if constant.is_deprecated {
                        Some(true)
                    } else {
                        None
                    },
                    ..CompletionItem::default()
                });
            }
        }

        // `::class` keyword — returns the fully qualified class name as a string.
        // Available on any class, interface, or enum via `::` access.
        if access_kind == AccessKind::DoubleColon || access_kind == AccessKind::ParentDoubleColon {
            items.push(CompletionItem {
                label: "class".to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("class-string".to_string()),
                insert_text: Some("class".to_string()),
                filter_text: Some("class".to_string()),
                ..CompletionItem::default()
            });
        }

        // Sort all items alphabetically (case-insensitive) and assign
        // sort_text so the editor preserves this ordering.
        items.sort_by(|a, b| {
            a.filter_text
                .as_deref()
                .unwrap_or(&a.label)
                .to_lowercase()
                .cmp(&b.filter_text.as_deref().unwrap_or(&b.label).to_lowercase())
        });

        for (i, item) in items.iter_mut().enumerate() {
            item.sort_text = Some(format!("{:05}", i));
        }

        items
    }
}
