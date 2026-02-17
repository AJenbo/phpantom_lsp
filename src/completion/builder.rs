/// Completion item building.
///
/// This module contains the logic for constructing LSP `CompletionItem`s from
/// resolved `ClassInfo`, filtered by the `AccessKind` (arrow, double-colon,
/// or parent double-colon), as well as class name completion when no member
/// access operator is present.
use std::collections::{HashMap, HashSet};

use bumpalo::Bump;
use mago_span::HasSpan;
use mago_syntax::ast::*;
use mago_syntax::parser::parse_file_content;
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
/// When the FQN has no namespace separator (e.g. `PDO`, `DateTime`),
/// an import is only needed if the current file declares a namespace —
/// otherwise we are already in the global namespace and no `use`
/// statement is required.  Returns `None` in that case.
pub(crate) fn build_use_edit(
    fqn: &str,
    insert_pos: Position,
    file_namespace: &Option<String>,
) -> Option<Vec<TextEdit>> {
    // No namespace separator → this is a global class (e.g. `PDO`, `DateTime`).
    // Only needs an import when the current file declares a namespace;
    // otherwise we're already in the global namespace.
    if !fqn.contains('\\') && file_namespace.is_none() {
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

    // ─── Variable name completion ────────────────────────────────────────

    /// Extract the partial variable name (including `$`) that the user
    /// is currently typing at the given cursor position.
    ///
    /// Walks backward from the cursor through alphanumeric characters and
    /// underscores, then checks for a preceding `$`.  Returns `None` if
    /// no `$` is found or the result is just `"$"` with no identifier
    /// characters.
    ///
    /// Examples:
    ///   - `$us|`  → `Some("$us")`
    ///   - `$_SE|` → `Some("$_SE")`
    ///   - `$|`    → `Some("$")`  (bare dollar — show all variables)
    ///   - `foo|`  → `None`
    pub fn extract_partial_variable_name(content: &str, position: Position) -> Option<String> {
        let lines: Vec<&str> = content.lines().collect();
        if position.line as usize >= lines.len() {
            return None;
        }

        let line = lines[position.line as usize];
        let chars: Vec<char> = line.chars().collect();
        let col = (position.character as usize).min(chars.len());

        // Walk backwards through identifier characters
        let mut i = col;
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }

        // Must be preceded by `$`
        if i == 0 || chars[i - 1] != '$' {
            return None;
        }
        // Include the `$`
        i -= 1;

        // If preceded by another `$` (e.g. `$$var` — variable variable),
        // skip for now.
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
        // Must be at least `$`
        if partial.is_empty() {
            return None;
        }

        Some(partial)
    }

    /// PHP superglobal variable names (always available in any scope).
    const SUPERGLOBALS: &'static [&'static str] = &[
        "$_GET",
        "$_POST",
        "$_REQUEST",
        "$_SESSION",
        "$_COOKIE",
        "$_SERVER",
        "$_FILES",
        "$_ENV",
        "$GLOBALS",
        "$argc",
        "$argv",
    ];

    /// Maximum number of variable completions to return.
    const MAX_VARIABLE_COMPLETIONS: usize = 100;

    /// Build completion items for variable names visible at the cursor.
    ///
    /// Uses the mago parser to walk the AST and collect variables from
    /// the correct scope (method body, function body, closure, or
    /// top-level code).  This ensures:
    ///   - Properties (`$this->name`) are NOT listed as variables.
    ///   - Method/function parameters only appear inside their body.
    ///   - `$this` only appears inside non-static methods.
    ///   - Variables from unrelated classes/methods are excluded.
    ///
    /// Additionally, PHP superglobals (`$_GET`, `$_POST`, …) are always
    /// offered.
    ///
    /// The prefix must include the `$` (e.g. `"$us"`).
    /// Returns `(items, is_incomplete)`.
    pub(crate) fn build_variable_completions(
        content: &str,
        prefix: &str,
        position: Position,
    ) -> (Vec<CompletionItem>, bool) {
        let prefix_lower = prefix.to_lowercase();
        let mut seen: HashSet<String> = HashSet::new();
        let mut items: Vec<CompletionItem> = Vec::new();

        let cursor_offset = Self::line_col_to_byte_offset(content, position).unwrap_or(0) as u32;

        // ── 1. AST-based scope-aware variable collection ────────────
        let scope_vars = collect_variables_in_scope(content, cursor_offset);

        for var_name in &scope_vars {
            if !var_name.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            if !seen.insert(var_name.clone()) {
                continue;
            }
            items.push(CompletionItem {
                label: var_name.clone(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("variable".to_string()),
                insert_text: Some(var_name.clone()),
                filter_text: Some(var_name.clone()),
                sort_text: Some(format!("0_{}", var_name.to_lowercase())),
                ..CompletionItem::default()
            });
        }

        // ── 2. PHP superglobals ─────────────────────────────────────
        for &name in Self::SUPERGLOBALS {
            if !name.to_lowercase().starts_with(&prefix_lower) {
                continue;
            }
            if !seen.insert(name.to_string()) {
                continue;
            }
            items.push(CompletionItem {
                label: name.to_string(),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("PHP superglobal".to_string()),
                insert_text: Some(name.to_string()),
                filter_text: Some(name.to_string()),
                sort_text: Some(format!("z_{}", name.to_lowercase())),
                deprecated: Some(true),
                ..CompletionItem::default()
            });
        }

        let is_incomplete = items.len() > Self::MAX_VARIABLE_COMPLETIONS;
        if is_incomplete {
            items.sort_by(|a, b| a.sort_text.cmp(&b.sort_text));
            items.truncate(Self::MAX_VARIABLE_COMPLETIONS);
        }

        (items, is_incomplete)
    }

    /// Convert a line/column `Position` to a byte offset within `content`.
    ///
    /// Returns `None` if the position is out of range.
    fn line_col_to_byte_offset(content: &str, position: Position) -> Option<usize> {
        let mut offset = 0usize;
        for (line_idx, line) in content.lines().enumerate() {
            if line_idx == position.line as usize {
                let col = (position.character as usize).min(line.len());
                return Some(offset + col);
            }
            // +1 for the newline character
            offset += line.len() + 1;
        }
        None
    }
}

// ─── Scope-aware variable collector ─────────────────────────────────────────

/// Collect all variable names visible at `cursor_offset` by parsing the
/// file and walking the AST to find the enclosing scope.
///
/// The returned set contains variable names including the `$` prefix
/// (e.g. `"$user"`, `"$this"`).
fn collect_variables_in_scope(content: &str, cursor_offset: u32) -> HashSet<String> {
    let arena = Bump::new();
    let file_id = mago_database::file::FileId::new("input.php");
    let program = parse_file_content(&arena, file_id, content);

    let mut vars = HashSet::new();
    find_scope_and_collect(program.statements.iter(), cursor_offset, &mut vars);
    vars
}

/// Walk top-level statements to find the scope enclosing the cursor,
/// then collect variables from that scope.
fn find_scope_and_collect<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    let stmts: Vec<&Statement> = statements.collect();

    // First pass: check if cursor is inside a class, function, or namespace.
    for &stmt in &stmts {
        match stmt {
            Statement::Class(class) => {
                let start = class.left_brace.start.offset;
                let end = class.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(class.members.iter(), cursor_offset, vars);
                    return;
                }
            }
            Statement::Interface(iface) => {
                let start = iface.left_brace.start.offset;
                let end = iface.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(iface.members.iter(), cursor_offset, vars);
                    return;
                }
            }
            Statement::Enum(enum_def) => {
                let start = enum_def.left_brace.start.offset;
                let end = enum_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(enum_def.members.iter(), cursor_offset, vars);
                    return;
                }
            }
            Statement::Trait(trait_def) => {
                let start = trait_def.left_brace.start.offset;
                let end = trait_def.right_brace.end.offset;
                if cursor_offset >= start && cursor_offset <= end {
                    collect_from_class_members(trait_def.members.iter(), cursor_offset, vars);
                    return;
                }
            }
            Statement::Function(func) => {
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                if cursor_offset >= body_start && cursor_offset <= body_end {
                    // Collect parameters
                    collect_from_params(&func.parameter_list, vars);
                    // Collect from body statements
                    collect_from_statements(func.body.statements.iter(), cursor_offset, vars);
                    return;
                }
            }
            Statement::Namespace(ns) => {
                let ns_span = ns.span();
                if cursor_offset >= ns_span.start.offset && cursor_offset <= ns_span.end.offset {
                    find_scope_and_collect(ns.statements().iter(), cursor_offset, vars);
                    return;
                }
            }
            _ => {}
        }
    }

    // Cursor is in top-level code — collect from all top-level statements.
    collect_from_statements(stmts.into_iter(), cursor_offset, vars);
}

/// Scan class-like members to find the method containing the cursor
/// and collect variables from that method's scope.
fn collect_from_class_members<'b>(
    members: impl Iterator<Item = &'b ClassLikeMember<'b>>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    for member in members {
        if let ClassLikeMember::Method(method) = member
            && let MethodBody::Concrete(block) = &method.body
        {
            let blk_start = block.left_brace.start.offset;
            let blk_end = block.right_brace.end.offset;
            if cursor_offset >= blk_start && cursor_offset <= blk_end {
                // Add $this only if the method is NOT static
                let is_static = method
                    .modifiers
                    .iter()
                    .any(|m| matches!(m, Modifier::Static(_)));
                if !is_static {
                    vars.insert("$this".to_string());
                }
                // Collect parameters (skip promoted properties —
                // they act as both params and properties, but as
                // variables they are still accessible in the body)
                collect_from_params(&method.parameter_list, vars);
                // Collect from body
                collect_from_statements(block.statements.iter(), cursor_offset, vars);
                return;
            }
        }
    }
    // Cursor is inside the class body but not inside any method body
    // (e.g. in a property declaration) — no variables are in scope.
}

/// Collect parameter names from a function/method/closure parameter list.
fn collect_from_params(params: &FunctionLikeParameterList, vars: &mut HashSet<String>) {
    for param in params.parameters.iter() {
        let name = param.variable.name.to_string();
        vars.insert(name);
    }
}

/// Walk statements within a scope collecting variable names.
///
/// This handles assignments, foreach, for, try/catch, closures,
/// global, static, and all control-flow structures.
///
/// Only variables defined **before** the cursor position are collected.
/// This prevents suggesting variables that haven't been defined yet
/// (e.g. a variable assigned on line 535 shouldn't appear when typing
/// on line 15).
fn collect_from_statements<'b>(
    statements: impl Iterator<Item = &'b Statement<'b>>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    for stmt in statements {
        // Skip statements that start after the cursor — variables
        // defined there haven't been introduced yet.
        let stmt_span = stmt.span();
        if stmt_span.start.offset > cursor_offset {
            continue;
        }

        match stmt {
            Statement::Expression(expr_stmt) => {
                collect_vars_from_expression(expr_stmt.expression, cursor_offset, vars);
            }
            Statement::Block(block) => {
                collect_from_statements(block.statements.iter(), cursor_offset, vars);
            }
            Statement::If(if_stmt) => match &if_stmt.body {
                IfBody::Statement(body) => {
                    // Collect from the condition (assignments in conditions)
                    collect_vars_from_expression(if_stmt.condition, cursor_offset, vars);
                    collect_from_statement(body.statement, cursor_offset, vars);
                    for else_if in body.else_if_clauses.iter() {
                        collect_vars_from_expression(else_if.condition, cursor_offset, vars);
                        collect_from_statement(else_if.statement, cursor_offset, vars);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        collect_from_statement(else_clause.statement, cursor_offset, vars);
                    }
                }
                IfBody::ColonDelimited(body) => {
                    collect_vars_from_expression(if_stmt.condition, cursor_offset, vars);
                    collect_from_statements(body.statements.iter(), cursor_offset, vars);
                    for else_if in body.else_if_clauses.iter() {
                        collect_vars_from_expression(else_if.condition, cursor_offset, vars);
                        collect_from_statements(else_if.statements.iter(), cursor_offset, vars);
                    }
                    if let Some(else_clause) = &body.else_clause {
                        collect_from_statements(else_clause.statements.iter(), cursor_offset, vars);
                    }
                }
            },
            Statement::Foreach(foreach) => {
                // Collect the key and value variables — they are defined
                // in the foreach header which starts before the cursor
                // (guaranteed by the span check above).
                if let Some(key_expr) = foreach.target.key() {
                    collect_var_name_from_expression(key_expr, vars);
                }
                collect_var_name_from_expression(foreach.target.value(), vars);
                // Recurse into body
                for inner in foreach.body.statements() {
                    collect_from_statement(inner, cursor_offset, vars);
                }
            }
            Statement::For(for_stmt) => {
                // Collect variables from initializations (e.g. `$i = 0`)
                for init_expr in for_stmt.initializations.iter() {
                    collect_vars_from_expression(init_expr, cursor_offset, vars);
                }
                match &for_stmt.body {
                    ForBody::Statement(inner) => {
                        collect_from_statement(inner, cursor_offset, vars);
                    }
                    ForBody::ColonDelimited(body) => {
                        collect_from_statements(body.statements.iter(), cursor_offset, vars);
                    }
                }
            }
            Statement::While(while_stmt) => match &while_stmt.body {
                WhileBody::Statement(inner) => {
                    collect_from_statement(inner, cursor_offset, vars);
                }
                WhileBody::ColonDelimited(body) => {
                    collect_from_statements(body.statements.iter(), cursor_offset, vars);
                }
            },
            Statement::DoWhile(dw) => {
                collect_from_statement(dw.statement, cursor_offset, vars);
            }
            Statement::Try(try_stmt) => {
                collect_from_statements(try_stmt.block.statements.iter(), cursor_offset, vars);
                for catch in try_stmt.catch_clauses.iter() {
                    // Only collect the catch variable if its clause starts
                    // before the cursor (i.e. the cursor is inside or after
                    // the catch block).
                    let catch_span = catch.span();
                    if catch_span.start.offset > cursor_offset {
                        continue;
                    }
                    if let Some(ref var) = catch.variable {
                        vars.insert(var.name.to_string());
                    }
                    collect_from_statements(catch.block.statements.iter(), cursor_offset, vars);
                }
                if let Some(finally) = &try_stmt.finally_clause {
                    let finally_span = finally.span();
                    if finally_span.start.offset <= cursor_offset {
                        collect_from_statements(
                            finally.block.statements.iter(),
                            cursor_offset,
                            vars,
                        );
                    }
                }
            }
            Statement::Global(global) => {
                // The span check above already ensures this statement is
                // before the cursor.
                for var in global.variables.iter() {
                    if let Variable::Direct(dv) = var {
                        vars.insert(dv.name.to_string());
                    }
                }
            }
            Statement::Static(static_stmt) => {
                for item in static_stmt.items.iter() {
                    vars.insert(item.variable().name.to_string());
                }
            }
            Statement::Return(ret) => {
                if let Some(expr) = ret.value {
                    collect_vars_from_expression(expr, cursor_offset, vars);
                }
            }
            Statement::Echo(echo) => {
                for expr in echo.values.iter() {
                    collect_vars_from_expression(expr, cursor_offset, vars);
                }
            }
            Statement::Switch(switch) => {
                collect_vars_from_expression(switch.expression, cursor_offset, vars);
                match &switch.body {
                    SwitchBody::BraceDelimited(body) => {
                        for case in body.cases.iter() {
                            collect_from_statements(case.statements().iter(), cursor_offset, vars);
                        }
                    }
                    SwitchBody::ColonDelimited(body) => {
                        for case in body.cases.iter() {
                            collect_from_statements(case.statements().iter(), cursor_offset, vars);
                        }
                    }
                }
            }
            // Skip class/function/namespace declarations (they have their
            // own scopes handled by find_scope_and_collect).
            Statement::Class(_)
            | Statement::Interface(_)
            | Statement::Trait(_)
            | Statement::Enum(_)
            | Statement::Function(_)
            | Statement::Namespace(_) => {}
            _ => {}
        }
    }
}

/// Helper: dispatch a single statement to `collect_from_statements`.
fn collect_from_statement<'b>(
    stmt: &'b Statement<'b>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    collect_from_statements(std::iter::once(stmt), cursor_offset, vars);
}

/// Extract variable names from an expression.
///
/// Handles assignments (`$x = ...`), closures/arrow-functions (enters
/// scope only if cursor is inside), and recursion into sub-expressions.
fn collect_vars_from_expression<'b>(
    expr: &'b Expression<'b>,
    cursor_offset: u32,
    vars: &mut HashSet<String>,
) {
    match expr {
        Expression::Assignment(assignment) => {
            // Collect the LHS variable name
            collect_var_name_from_expression(assignment.lhs, vars);
            // Also scan the RHS for nested assignments
            collect_vars_from_expression(assignment.rhs, cursor_offset, vars);
        }
        // If the cursor is inside a closure body, collect from that
        // closure's scope instead (closures have their own variable scope).
        Expression::Closure(closure) => {
            let body_start = closure.body.left_brace.start.offset;
            let body_end = closure.body.right_brace.end.offset;
            if cursor_offset >= body_start && cursor_offset <= body_end {
                // Closure introduces a new scope: parameters + use clause
                collect_from_params(&closure.parameter_list, vars);
                if let Some(ref use_clause) = closure.use_clause {
                    for use_var in use_clause.variables.iter() {
                        vars.insert(use_var.variable.name.to_string());
                    }
                }
                collect_from_statements(closure.body.statements.iter(), cursor_offset, vars);
            }
            // If cursor is outside this closure, don't collect its internals.
        }
        Expression::ArrowFunction(arrow) => {
            let span = arrow.span();
            if cursor_offset >= span.start.offset && cursor_offset <= span.end.offset {
                collect_from_params(&arrow.parameter_list, vars);
                collect_vars_from_expression(arrow.expression, cursor_offset, vars);
            }
        }
        // Don't recurse into sub-expressions that aren't scoping constructs
        // — we only care about assignment LHS variables, not every variable
        // reference (those are handled by the statement walker).
        _ => {}
    }
}

/// Extract a direct variable name from an expression (for assignment LHS,
/// foreach targets, etc.).  Only extracts `$name` from direct variables;
/// ignores property accesses, array accesses, etc.
fn collect_var_name_from_expression(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Variable(Variable::Direct(dv)) => {
            vars.insert(dv.name.to_string());
        }
        // `list($a, $b) = ...` or `[$a, $b] = ...`
        Expression::List(list) => {
            for element in list.elements.iter() {
                if let ArrayElement::KeyValue(kv) = element {
                    collect_var_name_from_expression(kv.value, vars);
                } else if let ArrayElement::Value(val) = element {
                    collect_var_name_from_expression(val.value, vars);
                }
            }
        }
        Expression::Array(arr) => {
            for element in arr.elements.iter() {
                if let ArrayElement::KeyValue(kv) = element {
                    collect_var_name_from_expression(kv.value, vars);
                } else if let ArrayElement::Value(val) = element {
                    collect_var_name_from_expression(val.value, vars);
                }
            }
        }
        _ => {}
    }
}

impl Backend {
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
