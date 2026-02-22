/// Use-statement insertion helpers.
///
/// This module provides reusable helpers for computing where to insert a
/// `use` statement in a PHP file and for building the corresponding LSP
/// `TextEdit`.  These are shared by class-name completion, and will be
/// needed by future features such as auto-import on hover, code actions,
/// and refactoring.
///
/// New `use` statements are inserted at the alphabetically correct
/// position among the existing imports so the use block stays sorted.
use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::util::short_name;

/// Information about a file's existing `use` block, used to compute
/// the correct alphabetical insertion position for new imports.
#[derive(Debug, Clone)]
pub(crate) struct UseBlockInfo {
    /// Each existing top-level `use` import: `(line_number, sort_key)`.
    /// `sort_key` is the lowercased FQN extracted from the statement,
    /// used for case-insensitive alphabetical comparison.
    /// Entries are in file order (sorted by line number).
    existing: Vec<(u32, String)>,
    /// The line to insert at when there are no existing `use` statements.
    /// Points after the `namespace` declaration, or after `<?php`.
    fallback_line: u32,
}

impl UseBlockInfo {
    /// Compute the insertion `Position` for a new `use` statement that
    /// imports the given FQN, maintaining alphabetical order among the
    /// existing imports.
    ///
    /// If there are no existing imports, returns the fallback position
    /// (after `namespace` or `<?php`).
    pub(crate) fn insert_position_for(&self, fqn: &str) -> Position {
        let key = fqn.to_lowercase();

        if self.existing.is_empty() {
            return Position {
                line: self.fallback_line,
                character: 0,
            };
        }

        // Find the first existing use whose sort key is alphabetically
        // after the new FQN — the new statement goes right before it.
        for (line, existing_key) in &self.existing {
            if *existing_key > key {
                return Position {
                    line: *line,
                    character: 0,
                };
            }
        }

        // New FQN sorts after all existing imports — append after the last one.
        let last_line = self.existing.last().expect("non-empty checked above").0;
        Position {
            line: last_line + 1,
            character: 0,
        }
    }
}

/// Extract the sort key (lowercased FQN) from a `use` statement line.
///
/// Handles the common forms:
///   - `use Foo\Bar;` → `foo\bar`
///   - `use Foo\Bar as Alias;` → `foo\bar`
///   - `use function Foo\bar;` → `function foo\bar` (preserves keyword prefix for grouping)
///   - `use const Foo\BAR;` → `const foo\bar`
///   - `use Foo\{Bar, Baz};` → `foo\`
///
/// Returns `None` if the line does not look like a use statement.
fn extract_use_sort_key(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed
        .strip_prefix("use ")
        .or_else(|| trimmed.strip_prefix("use\t"))?;

    // Skip `use (` / `use(` — those are closures, not imports.
    if rest.starts_with('(') {
        return None;
    }

    // Preserve `function`/`const` prefix so they sort into their own
    // group naturally (all `const …` together, all `function …` together).
    let (prefix, fqn_part) = if let Some(r) = rest.strip_prefix("function ") {
        ("function ", r)
    } else if let Some(r) = rest.strip_prefix("const ") {
        ("const ", r)
    } else {
        ("", rest)
    };

    // Extract the FQN: everything up to `;`, ` as `, or `{`.
    let fqn = fqn_part
        .split(';')
        .next()
        .unwrap_or(fqn_part)
        .split(" as ")
        .next()
        .unwrap_or(fqn_part)
        .split('{')
        .next()
        .unwrap_or(fqn_part)
        .trim()
        .trim_start_matches('\\');

    Some(format!("{}{}", prefix, fqn).to_lowercase())
}

/// Analyse the file content and return a [`UseBlockInfo`] describing the
/// existing `use` block.
///
/// This replaces the older `find_use_insert_position` — instead of a
/// single append-at-bottom position, callers get a structure that
/// supports alphabetical insertion via
/// [`UseBlockInfo::insert_position_for`].
///
/// The scanning logic distinguishes top-level `use` imports from trait
/// `use` statements inside class/enum/trait bodies by tracking brace
/// depth.
pub(crate) fn analyze_use_block(content: &str) -> UseBlockInfo {
    let mut existing: Vec<(u32, String)> = Vec::new();
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
            && let Some(sort_key) = extract_use_sort_key(trimmed)
        {
            existing.push((i as u32, sort_key));
        }
    }

    // Fallback: insert after `namespace`, or after `<?php`.
    let fallback_line = namespace_line.or(php_open_line).map(|l| l + 1).unwrap_or(0);

    UseBlockInfo {
        existing,
        fallback_line,
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
/// for the given fully-qualified class name at the alphabetically correct
/// position in the file's existing use block.
///
/// When the FQN has no namespace separator (e.g. `PDO`, `DateTime`),
/// an import is only needed if the current file declares a namespace —
/// otherwise we are already in the global namespace and no `use`
/// statement is required.  Returns `None` in that case.
pub(crate) fn build_use_edit(
    fqn: &str,
    use_block: &UseBlockInfo,
    file_namespace: &Option<String>,
) -> Option<Vec<TextEdit>> {
    // No namespace separator → this is a global class (e.g. `PDO`, `DateTime`).
    // Only needs an import when the current file declares a namespace;
    // otherwise we're already in the global namespace.
    if !fqn.contains('\\') && file_namespace.is_none() {
        return None;
    }

    let insert_pos = use_block.insert_position_for(fqn);

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

    /// Backward-compatible helper for tests: returns the position **after**
    /// the last existing `use` statement (or the appropriate fallback).
    fn find_use_insert_position(content: &str) -> Position {
        let info = analyze_use_block(content);
        if info.existing.is_empty() {
            Position {
                line: info.fallback_line,
                character: 0,
            }
        } else {
            let last_line = info.existing.last().expect("non-empty checked above").0;
            Position {
                line: last_line + 1,
                character: 0,
            }
        }
    }

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

    // ── extract_use_sort_key ────────────────────────────────────────

    #[test]
    fn sort_key_simple_use() {
        assert_eq!(
            extract_use_sort_key("use Foo\\Bar;"),
            Some("foo\\bar".to_string())
        );
    }

    #[test]
    fn sort_key_with_alias() {
        assert_eq!(
            extract_use_sort_key("use Foo\\Bar as Baz;"),
            Some("foo\\bar".to_string())
        );
    }

    #[test]
    fn sort_key_grouped_use() {
        assert_eq!(
            extract_use_sort_key("use Foo\\{Bar, Baz};"),
            Some("foo\\".to_string())
        );
    }

    #[test]
    fn sort_key_function_use() {
        assert_eq!(
            extract_use_sort_key("use function Foo\\bar;"),
            Some("function foo\\bar".to_string())
        );
    }

    #[test]
    fn sort_key_const_use() {
        assert_eq!(
            extract_use_sort_key("use const Foo\\BAR;"),
            Some("const foo\\bar".to_string())
        );
    }

    #[test]
    fn sort_key_leading_backslash_stripped() {
        assert_eq!(
            extract_use_sort_key("use \\Foo\\Bar;"),
            Some("foo\\bar".to_string())
        );
    }

    #[test]
    fn sort_key_not_a_use_statement() {
        assert_eq!(extract_use_sort_key("class Foo {}"), None);
    }

    #[test]
    fn sort_key_closure_use_ignored() {
        assert_eq!(extract_use_sort_key("use ($var)"), None);
    }

    // ── analyze_use_block ───────────────────────────────────────────

    #[test]
    fn collects_existing_uses_with_sort_keys() {
        let content = "<?php\nnamespace App;\nuse Foo\\Bar;\nuse Baz\\Qux;\n\nclass X {}\n";
        let info = analyze_use_block(content);
        assert_eq!(info.existing.len(), 2);
        assert_eq!(info.existing[0], (2, "foo\\bar".to_string()));
        assert_eq!(info.existing[1], (3, "baz\\qux".to_string()));
    }

    #[test]
    fn fallback_after_namespace_when_no_use() {
        let content = "<?php\nnamespace App;\n\nclass X {}\n";
        let info = analyze_use_block(content);
        assert!(info.existing.is_empty());
        assert_eq!(info.fallback_line, 2);
    }

    #[test]
    fn fallback_after_php_open_tag_when_no_namespace() {
        let content = "<?php\n\nclass X {}\n";
        let info = analyze_use_block(content);
        assert!(info.existing.is_empty());
        assert_eq!(info.fallback_line, 1);
    }

    #[test]
    fn trait_use_inside_class_not_collected() {
        let content = "<?php\nnamespace App;\nuse Foo\\Bar;\n\nclass X {\n    use SomeTrait;\n}\n";
        let info = analyze_use_block(content);
        // Only the top-level `use Foo\Bar;` should be collected.
        assert_eq!(info.existing.len(), 1);
        assert_eq!(info.existing[0], (2, "foo\\bar".to_string()));
    }

    // ── UseBlockInfo::insert_position_for ───────────────────────────

    #[test]
    fn insert_alphabetically_before_first() {
        // Existing: App\Zoo (line 2). Inserting App\Alpha should go before it.
        let info = UseBlockInfo {
            existing: vec![(2, "app\\zoo".to_string())],
            fallback_line: 1,
        };
        assert_eq!(
            info.insert_position_for("App\\Alpha"),
            Position {
                line: 2,
                character: 0,
            }
        );
    }

    #[test]
    fn insert_alphabetically_after_last() {
        // Existing: App\Alpha (line 2). Inserting App\Zoo should go after it.
        let info = UseBlockInfo {
            existing: vec![(2, "app\\alpha".to_string())],
            fallback_line: 1,
        };
        assert_eq!(
            info.insert_position_for("App\\Zoo"),
            Position {
                line: 3,
                character: 0,
            }
        );
    }

    #[test]
    fn insert_alphabetically_in_the_middle() {
        // Existing: App\Alpha (line 2), App\Zoo (line 3).
        // Inserting App\Middle should go between them.
        let info = UseBlockInfo {
            existing: vec![(2, "app\\alpha".to_string()), (3, "app\\zoo".to_string())],
            fallback_line: 1,
        };
        assert_eq!(
            info.insert_position_for("App\\Middle"),
            Position {
                line: 3,
                character: 0,
            }
        );
    }

    #[test]
    fn insert_uses_fallback_when_no_existing() {
        let info = UseBlockInfo {
            existing: vec![],
            fallback_line: 2,
        };
        assert_eq!(
            info.insert_position_for("App\\Foo"),
            Position {
                line: 2,
                character: 0,
            }
        );
    }

    #[test]
    fn insert_case_insensitive_comparison() {
        // Existing: app\alpha (line 2), app\zoo (line 3).
        // Inserting App\Middle (mixed case) should still land between them.
        let info = UseBlockInfo {
            existing: vec![(2, "app\\alpha".to_string()), (3, "app\\zoo".to_string())],
            fallback_line: 1,
        };
        assert_eq!(
            info.insert_position_for("APP\\MIDDLE"),
            Position {
                line: 3,
                character: 0,
            }
        );
    }

    #[test]
    fn insert_among_three_existing() {
        // Existing: A (line 2), C (line 3), E (line 4).
        // Inserting D should go before E (line 4).
        let info = UseBlockInfo {
            existing: vec![
                (2, "a\\a".to_string()),
                (3, "c\\c".to_string()),
                (4, "e\\e".to_string()),
            ],
            fallback_line: 1,
        };
        assert_eq!(
            info.insert_position_for("D\\D"),
            Position {
                line: 4,
                character: 0,
            }
        );
    }

    // ── find_use_insert_position (backward compat) ──────────────────

    #[test]
    fn compat_insert_after_last_use_statement() {
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
    fn compat_insert_after_namespace_when_no_use() {
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
    fn compat_insert_after_php_open_tag_when_no_namespace() {
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
    fn compat_trait_use_inside_class_not_treated_as_import() {
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

    // ── build_use_edit (alphabetical) ───────────────────────────────

    #[test]
    fn build_edit_inserts_at_correct_alpha_position() {
        let info = UseBlockInfo {
            existing: vec![(2, "app\\alpha".to_string()), (3, "app\\zoo".to_string())],
            fallback_line: 1,
        };
        let edits = build_use_edit("App\\Middle", &info, &Some("App".to_string()))
            .expect("should produce edit");
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "use App\\Middle;\n");
        assert_eq!(
            edits[0].range.start,
            Position {
                line: 3,
                character: 0
            }
        );
    }

    #[test]
    fn build_edit_skips_global_class_without_namespace() {
        let info = UseBlockInfo {
            existing: vec![],
            fallback_line: 1,
        };
        assert!(build_use_edit("PDO", &info, &None).is_none());
    }

    #[test]
    fn build_edit_includes_global_class_with_namespace() {
        let info = UseBlockInfo {
            existing: vec![],
            fallback_line: 2,
        };
        let edits =
            build_use_edit("PDO", &info, &Some("App".to_string())).expect("should produce edit");
        assert_eq!(edits[0].new_text, "use PDO;\n");
        assert_eq!(
            edits[0].range.start,
            Position {
                line: 2,
                character: 0
            }
        );
    }

    // ── End-to-end: analyze_use_block + build_use_edit ──────────────

    #[test]
    fn end_to_end_insert_before_existing_alphabetically() {
        let content = concat!(
            "<?php\n",
            "namespace App;\n",
            "use Exception;\n",
            "use Stringable;\n",
            "\n",
            "class X {}\n",
        );
        let info = analyze_use_block(content);
        let edits = build_use_edit("Cassandra\\DefaultCluster", &info, &Some("App".to_string()))
            .expect("should produce edit");

        assert_eq!(edits[0].new_text, "use Cassandra\\DefaultCluster;\n");
        // `Cassandra\DefaultCluster` < `Exception`, so insert before line 2.
        assert_eq!(
            edits[0].range.start,
            Position {
                line: 2,
                character: 0,
            }
        );
    }

    #[test]
    fn end_to_end_insert_after_all_existing() {
        let content = concat!(
            "<?php\n",
            "namespace App;\n",
            "use App\\Alpha;\n",
            "use App\\Beta;\n",
            "\n",
            "class X {}\n",
        );
        let info = analyze_use_block(content);
        let edits = build_use_edit("App\\Zeta", &info, &Some("App".to_string()))
            .expect("should produce edit");

        assert_eq!(edits[0].new_text, "use App\\Zeta;\n");
        // `App\Zeta` > `App\Beta`, so insert after line 3 → line 4.
        assert_eq!(
            edits[0].range.start,
            Position {
                line: 4,
                character: 0,
            }
        );
    }

    #[test]
    fn end_to_end_insert_between_existing() {
        let content = concat!(
            "<?php\n",
            "namespace App;\n",
            "use App\\Alpha;\n",
            "use App\\Zeta;\n",
            "\n",
            "class X {}\n",
        );
        let info = analyze_use_block(content);
        let edits = build_use_edit("App\\Middle", &info, &Some("App".to_string()))
            .expect("should produce edit");

        assert_eq!(edits[0].new_text, "use App\\Middle;\n");
        // `App\Middle` > `App\Alpha` but < `App\Zeta`, so insert at line 3.
        assert_eq!(
            edits[0].range.start,
            Position {
                line: 3,
                character: 0,
            }
        );
    }
}
