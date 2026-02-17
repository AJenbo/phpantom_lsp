/// Use-statement insertion helpers.
///
/// This module provides reusable helpers for computing where to insert a
/// `use` statement in a PHP file and for building the corresponding LSP
/// `TextEdit`.  These are shared by class-name completion, and will be
/// needed by future features such as auto-import on hover, code actions,
/// and refactoring.
use tower_lsp::lsp_types::*;

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
