/// Completion item building.
///
/// This module contains the logic for constructing LSP `CompletionItem`s from
/// resolved `ClassInfo`, filtered by the `AccessKind` (arrow, double-colon,
/// or parent double-colon), as well as class name completion when no member
/// access operator is present.
use std::collections::{HashMap, HashSet};

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::types::Visibility;
use crate::types::*;

/// Find the position where a new `use` statement should be inserted.
///
/// Scans the file content and returns a `Position` pointing to the
/// beginning of the line **after** the best insertion point:
///
///   1. After the last existing `use` statement (so the new import is
///      grouped with the others).
///   2. After the `namespace` declaration (if present but no `use`
///      statements exist yet).
///   3. After the `<?php` opening tag (fallback).
///
/// The returned position is always at column 0 of the target line, so
/// callers can insert `"use Foo\\Bar;\n"` directly.
pub(crate) fn find_use_insert_position(content: &str) -> Position {
    let mut last_use_line: Option<u32> = None;
    let mut namespace_line: Option<u32> = None;
    let mut php_open_line: Option<u32> = None;

    // Track brace depth so we can distinguish top-level `use` imports
    // from trait `use` statements inside class/enum/trait bodies.
    //
    // With semicolon-style namespaces (`namespace Foo;`), imports live
    // at depth 0 and class bodies are at depth 1.
    //
    // With brace-style namespaces (`namespace Foo { ... }`), imports
    // live at depth 1 and class bodies are at depth 2.
    //
    // We compute depth at the START of each line and track whether we
    // saw a brace-style namespace to set the right threshold.
    let mut brace_depth: u32 = 0;
    let mut uses_brace_namespace = false;

    for (i, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        // The depth at the start of this line (before counting its braces).
        let depth_at_start = brace_depth;

        // Update brace depth for the NEXT line.
        for ch in trimmed.chars() {
            match ch {
                '{' => brace_depth += 1,
                '}' => brace_depth = brace_depth.saturating_sub(1),
                _ => {}
            }
        }

        if trimmed.starts_with("<?php") && php_open_line.is_none() {
            php_open_line = Some(i as u32);
        }

        // Match `namespace Foo\Bar;` or `namespace Foo\Bar {`
        // but not `namespace\something` (which is a different construct).
        if trimmed.starts_with("namespace ") || trimmed.starts_with("namespace\t") {
            namespace_line = Some(i as u32);
            if trimmed.contains('{') {
                uses_brace_namespace = true;
            }
        }

        // The maximum brace depth at which `use` statements are still
        // namespace imports (not trait imports inside a class body).
        let max_import_depth = if uses_brace_namespace { 1 } else { 0 };

        // Match `use Foo\Bar;`, `use Foo\{Bar, Baz};`, etc.
        // Only at the import level — deeper means trait `use` inside a
        // class/enum/trait body.
        if depth_at_start <= max_import_depth
            && (trimmed.starts_with("use ") || trimmed.starts_with("use\t"))
            && !trimmed.starts_with("use (")
            && !trimmed.starts_with("use(")
        {
            last_use_line = Some(i as u32);
        }
    }

    // Insert after the last `use`, or after `namespace`, or after `<?php`.
    let target_line = last_use_line
        .or(namespace_line)
        .or(php_open_line)
        .unwrap_or(0);

    Position {
        line: target_line + 1,
        character: 0,
    }
}

/// Build an `additional_text_edits` entry that inserts a `use` statement
/// for the given fully-qualified class name.
///
/// Returns `None` when the FQN has no namespace separator (e.g. bare
/// `DateTime`), meaning no import is needed.
pub(crate) fn build_use_edit(fqn: &str, insert_pos: Position) -> Option<Vec<TextEdit>> {
    // No namespace → no import needed (e.g. `DateTime`, `Iterator`)
    if !fqn.contains('\\') {
        return None;
    }

    Some(vec![TextEdit {
        range: Range {
            start: insert_pos,
            end: insert_pos,
        },
        new_text: format!("use {};\n", fqn),
    }])
}

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

    // ─── Class name completion ──────────────────────────────────────────

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
                    additional_text_edits: build_use_edit(fqn, use_insert_pos),
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
                    additional_text_edits: build_use_edit(fqn, use_insert_pos),
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
                additional_text_edits: build_use_edit(name, use_insert_pos),
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
}
