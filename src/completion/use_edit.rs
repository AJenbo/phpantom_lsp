/// Use-statement insertion helpers.
///
/// This module provides reusable helpers for computing where to insert a
/// `use` statement in a PHP file and for building the corresponding LSP
/// `TextEdit`.  These are shared by class-name completion, and will be
/// needed by future features such as auto-import on hover, code actions,
/// and refactoring.
use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::util::short_name;

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

/// Check whether importing the given FQN would create a conflict with an
/// existing `use` statement in the file.
///
/// Two kinds of conflict are detected (both case-insensitive):
///
/// 1. **Short-name collision.** The short name of the FQN (the part after
///    the last `\`) matches an alias that already points to a different
///    class.  For example, `use Cassandra\Exception;` blocks importing
///    `App\Exception` because both resolve to the alias `Exception`.
///
/// 2. **Leading-segment collision.** The first namespace segment of the
///    FQN matches an existing alias.  For example, `use Stringable as pq;`
///    blocks importing `pq\Exception` because writing `pq\Exception` in
///    code would resolve `pq` through the alias, not through the
///    namespace.
pub(crate) fn use_import_conflicts(fqn: &str, file_use_map: &HashMap<String, String>) -> bool {
    let sn = short_name(fqn);
    // The first namespace segment (e.g. `pq` in `pq\Exception`).
    // For single-segment FQNs this equals the short name, so the
    // leading-segment check is redundant with the short-name check and
    // we skip it to avoid a false positive against the class's own
    // import.
    let first_segment = fqn.split('\\').next().unwrap_or(fqn);
    let has_namespace = fqn.contains('\\');

    for (alias, existing_fqn) in file_use_map {
        // 1. Short-name collision.
        if alias.eq_ignore_ascii_case(sn) && !existing_fqn.eq_ignore_ascii_case(fqn) {
            return true;
        }
        // 2. Leading-segment collision (only for multi-segment FQNs).
        if has_namespace && alias.eq_ignore_ascii_case(first_segment) {
            return true;
        }
    }
    false
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── use_import_conflicts ────────────────────────────────────────

    #[test]
    fn conflict_when_short_name_taken_by_different_fqn() {
        let mut use_map = HashMap::new();
        use_map.insert("Exception".to_string(), "Cassandra\\Exception".to_string());

        assert!(use_import_conflicts("App\\Exception", &use_map));
    }

    #[test]
    fn no_conflict_when_same_fqn() {
        let mut use_map = HashMap::new();
        use_map.insert("Exception".to_string(), "App\\Exception".to_string());

        assert!(!use_import_conflicts("App\\Exception", &use_map));
    }

    #[test]
    fn no_conflict_when_different_short_name() {
        let mut use_map = HashMap::new();
        use_map.insert("Exception".to_string(), "Cassandra\\Exception".to_string());

        assert!(!use_import_conflicts("App\\Collection", &use_map));
    }

    #[test]
    fn conflict_is_case_insensitive() {
        let mut use_map = HashMap::new();
        use_map.insert("exception".to_string(), "Cassandra\\Exception".to_string());

        assert!(use_import_conflicts("App\\Exception", &use_map));
    }

    #[test]
    fn no_conflict_when_use_map_empty() {
        let use_map = HashMap::new();

        assert!(!use_import_conflicts("App\\Exception", &use_map));
    }

    #[test]
    fn conflict_with_global_class_fqn() {
        // File has `use Cassandra\Exception;`, importing the global `Exception`
        // (no namespace) should conflict.
        let mut use_map = HashMap::new();
        use_map.insert("Exception".to_string(), "Cassandra\\Exception".to_string());

        assert!(use_import_conflicts("Exception", &use_map));
    }

    #[test]
    fn no_conflict_same_fqn_case_insensitive() {
        let mut use_map = HashMap::new();
        use_map.insert("Exception".to_string(), "app\\exception".to_string());

        assert!(!use_import_conflicts("App\\Exception", &use_map));
    }

    // ── Leading-segment collision ───────────────────────────────────

    #[test]
    fn conflict_when_first_segment_matches_alias() {
        // `use Stringable as pq;` — importing `pq\Exception` would be
        // confusing because `pq\Exception` in code resolves through the
        // alias to `Stringable\Exception`.
        let mut use_map = HashMap::new();
        use_map.insert("pq".to_string(), "Stringable".to_string());

        assert!(use_import_conflicts("pq\\Exception", &use_map));
    }

    #[test]
    fn conflict_when_first_segment_matches_alias_case_insensitive() {
        let mut use_map = HashMap::new();
        use_map.insert("PQ".to_string(), "Stringable".to_string());

        assert!(use_import_conflicts("pq\\Exception", &use_map));
    }

    #[test]
    fn no_leading_segment_conflict_for_single_segment_fqn() {
        // `use Stringable;` should not conflict with importing global
        // class `Stringable` — single-segment FQNs skip the leading-
        // segment check to avoid a false positive.
        let mut use_map = HashMap::new();
        use_map.insert("Stringable".to_string(), "Stringable".to_string());

        assert!(!use_import_conflicts("Stringable", &use_map));
    }

    #[test]
    fn leading_segment_conflict_with_deep_namespace() {
        // `use Something as App;` blocks `App\Models\User` because `App`
        // in code would resolve through the alias.
        let mut use_map = HashMap::new();
        use_map.insert("App".to_string(), "Something".to_string());

        assert!(use_import_conflicts("App\\Models\\User", &use_map));
    }

    #[test]
    fn no_leading_segment_conflict_when_no_alias_matches() {
        let mut use_map = HashMap::new();
        use_map.insert("Exception".to_string(), "Cassandra\\Exception".to_string());

        // First segment is `App`, alias is `Exception` — no match.
        assert!(!use_import_conflicts("App\\Collection", &use_map));
    }

    // ── find_use_insert_position ────────────────────────────────────

    #[test]
    fn insert_after_last_use_statement() {
        let content = "<?php\nnamespace App;\nuse Foo\\Bar;\nuse Baz\\Qux;\n\nclass X {}\n";
        let pos = find_use_insert_position(content);
        assert_eq!(
            pos,
            Position {
                line: 4,
                character: 0
            }
        );
    }

    #[test]
    fn insert_after_namespace_when_no_use() {
        let content = "<?php\nnamespace App;\n\nclass X {}\n";
        let pos = find_use_insert_position(content);
        assert_eq!(
            pos,
            Position {
                line: 2,
                character: 0
            }
        );
    }

    #[test]
    fn insert_after_php_open_tag_when_no_namespace() {
        let content = "<?php\n\nclass X {}\n";
        let pos = find_use_insert_position(content);
        assert_eq!(
            pos,
            Position {
                line: 1,
                character: 0
            }
        );
    }

    #[test]
    fn trait_use_inside_class_not_treated_as_import() {
        let content = "<?php\nnamespace App;\nuse Foo\\Bar;\n\nclass X {\n    use SomeTrait;\n}\n";
        let pos = find_use_insert_position(content);
        // Should insert after `use Foo\Bar;` (line 2), not after `use SomeTrait;`
        assert_eq!(
            pos,
            Position {
                line: 3,
                character: 0
            }
        );
    }
}
