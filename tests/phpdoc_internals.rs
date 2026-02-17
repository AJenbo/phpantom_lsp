//! Tests for PHPDoc internal helpers.
//!
//! These tests were moved from the inline `#[cfg(test)] mod tests` block
//! in `src/completion/phpdoc.rs` to keep the project's convention of
//! placing tests in the `tests/` directory.

use phpantom_lsp::completion::phpdoc::*;
use tower_lsp::lsp_types::*;

// ── is_inside_docblock ──────────────────────────────────────────

#[test]
fn inside_open_docblock() {
    let content = "<?php\n/**\n * @\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert!(is_inside_docblock(content, pos));
}

#[test]
fn inside_closed_docblock() {
    let content = "<?php\n/**\n * @param string $x\n */\nfunction foo() {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert!(is_inside_docblock(content, pos));
}

#[test]
fn outside_docblock_after_close() {
    let content = "<?php\n/**\n * @param string $x\n */\nfunction foo() {}\n";
    let pos = Position {
        line: 4,
        character: 5,
    };
    assert!(!is_inside_docblock(content, pos));
}

#[test]
fn outside_docblock_before_open() {
    let content = "<?php\n\n/**\n * @param string $x\n */\n";
    let pos = Position {
        line: 1,
        character: 0,
    };
    assert!(!is_inside_docblock(content, pos));
}

#[test]
fn not_inside_regular_comment() {
    let content = "<?php\n/* regular comment @param */\n";
    let pos = Position {
        line: 1,
        character: 22,
    };
    assert!(!is_inside_docblock(content, pos));
}

#[test]
fn inside_multiline_docblock() {
    let content = "<?php\n/**\n * Some description.\n *\n * @\n */\n";
    let pos = Position {
        line: 4,
        character: 4,
    };
    assert!(is_inside_docblock(content, pos));
}

// ── extract_phpdoc_prefix ───────────────────────────────────────

#[test]
fn prefix_bare_at() {
    let content = "<?php\n/**\n * @\n */\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(extract_phpdoc_prefix(content, pos), Some("@".to_string()));
}

#[test]
fn prefix_partial_tag() {
    let content = "<?php\n/**\n * @par\n */\n";
    let pos = Position {
        line: 2,
        character: 7,
    };
    assert_eq!(
        extract_phpdoc_prefix(content, pos),
        Some("@par".to_string())
    );
}

#[test]
fn prefix_phpstan_tag() {
    let content = "<?php\n/**\n * @phpstan-a\n */\n";
    let pos = Position {
        line: 2,
        character: 14,
    };
    assert_eq!(
        extract_phpdoc_prefix(content, pos),
        Some("@phpstan-a".to_string())
    );
}

#[test]
fn prefix_full_tag() {
    let content = "<?php\n/**\n * @return\n */\n";
    let pos = Position {
        line: 2,
        character: 10,
    };
    assert_eq!(
        extract_phpdoc_prefix(content, pos),
        Some("@return".to_string())
    );
}

#[test]
fn no_prefix_outside_docblock() {
    let content = "<?php\n$email = 'user@example.com';\n";
    let pos = Position {
        line: 1,
        character: 25,
    };
    assert_eq!(extract_phpdoc_prefix(content, pos), None);
}

#[test]
fn no_prefix_no_at_sign() {
    let content = "<?php\n/**\n * Just a description\n */\n";
    let pos = Position {
        line: 2,
        character: 20,
    };
    assert_eq!(extract_phpdoc_prefix(content, pos), None);
}

#[test]
fn no_prefix_in_email_inside_docblock() {
    let content = "<?php\n/**\n * Contact user@example.com\n */\n";
    let pos = Position {
        line: 2,
        character: 25,
    };
    assert_eq!(extract_phpdoc_prefix(content, pos), None);
}

// ── detect_context ──────────────────────────────────────────────

#[test]
fn context_function() {
    let content = "<?php\n/**\n * @\n */\nfunction hello(): void {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(
        detect_context(content, pos),
        DocblockContext::FunctionOrMethod
    );
}

#[test]
fn context_method() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public function bar(): void {}\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(
        detect_context(content, pos),
        DocblockContext::FunctionOrMethod
    );
}

#[test]
fn context_static_method() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public static function bar(): void {}\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(
        detect_context(content, pos),
        DocblockContext::FunctionOrMethod
    );
}

#[test]
fn context_class() {
    let content = "<?php\n/**\n * @\n */\nclass MyClass {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
}

#[test]
fn context_abstract_class() {
    let content = "<?php\n/**\n * @\n */\nabstract class MyClass {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
}

#[test]
fn context_final_class() {
    let content = "<?php\n/**\n * @\n */\nfinal class MyClass {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
}

#[test]
fn context_interface() {
    let content = "<?php\n/**\n * @\n */\ninterface MyInterface {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
}

#[test]
fn context_trait() {
    let content = "<?php\n/**\n * @\n */\ntrait MyTrait {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
}

#[test]
fn context_enum() {
    let content = "<?php\n/**\n * @\n */\nenum Status {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::ClassLike);
}

#[test]
fn context_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public string $name;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::Property);
}

#[test]
fn context_typed_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    protected ?int $count = 0;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::Property);
}

#[test]
fn context_readonly_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public readonly string $name;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::Property);
}

#[test]
fn context_static_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    private static array $cache = [];\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::Property);
}

#[test]
fn context_constant() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    const MAX_SIZE = 100;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::Constant);
}

#[test]
fn context_visibility_constant() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public const VERSION = '1.0';\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::Constant);
}

#[test]
fn context_unknown_file_level() {
    let content = "<?php\n/**\n * @\n */\n\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    assert_eq!(detect_context(content, pos), DocblockContext::Unknown);
}

// ── extract_symbol_info ─────────────────────────────────────────

#[test]
fn symbol_info_function_params() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function greet(string $name, int $age): string {\n",
        "    return '';\n",
        "}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let info = extract_symbol_info(content, pos);

    assert_eq!(info.params.len(), 2);
    assert_eq!(
        info.params[0],
        (Some("string".to_string()), "$name".to_string())
    );
    assert_eq!(
        info.params[1],
        (Some("int".to_string()), "$age".to_string())
    );
    assert_eq!(info.return_type, Some("string".to_string()));
}

#[test]
fn symbol_info_method_no_type_hints() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public function bar($x, $y) {}\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let info = extract_symbol_info(content, pos);

    assert_eq!(info.params.len(), 2);
    assert_eq!(info.params[0], (None, "$x".to_string()));
    assert_eq!(info.params[1], (None, "$y".to_string()));
    assert_eq!(info.return_type, None);
}

#[test]
fn symbol_info_nullable_return() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function find(int $id): ?User {\n",
        "    return null;\n",
        "}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let info = extract_symbol_info(content, pos);
    assert_eq!(info.return_type, Some("?User".to_string()));
}

#[test]
fn symbol_info_property_type() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public string $name;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let info = extract_symbol_info(content, pos);
    assert_eq!(info.type_hint, Some("string".to_string()));
}

#[test]
fn symbol_info_nullable_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    protected ?int $count = 0;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let info = extract_symbol_info(content, pos);
    assert_eq!(info.type_hint, Some("?int".to_string()));
}

#[test]
fn symbol_info_readonly_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public readonly string $name;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let info = extract_symbol_info(content, pos);
    assert_eq!(info.type_hint, Some("string".to_string()));
}

#[test]
fn symbol_info_variadic_param() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function merge(array ...$arrays): array {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let info = extract_symbol_info(content, pos);
    assert_eq!(info.params.len(), 1);
    assert_eq!(
        info.params[0],
        (Some("array".to_string()), "$arrays".to_string())
    );
}

#[test]
fn symbol_info_reference_param() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function swap(int &$a, int &$b): void {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let info = extract_symbol_info(content, pos);
    assert_eq!(info.params.len(), 2);
    assert_eq!(info.params[0], (Some("int".to_string()), "$a".to_string()));
    assert_eq!(info.params[1], (Some("int".to_string()), "$b".to_string()));
}

#[test]
fn symbol_info_no_params() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function now(): DateTimeImmutable {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let info = extract_symbol_info(content, pos);
    assert!(info.params.is_empty());
    assert_eq!(info.return_type, Some("DateTimeImmutable".to_string()));
}

// ── find_existing_param_tags ─────────────────────────────────────

#[test]
fn finds_existing_param_tags() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @param string $name\n",
        " * @param int $age\n",
        " * @\n",
        " */\n",
        "function greet(string $name, int $age, bool $formal): string {}\n",
    );
    let pos = Position {
        line: 4,
        character: 4,
    };
    let existing = find_existing_param_tags(content, pos);
    assert_eq!(existing, vec!["$name", "$age"]);
}

#[test]
fn no_existing_param_tags() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function greet(string $name): string {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let existing = find_existing_param_tags(content, pos);
    assert!(existing.is_empty());
}

// ── build_phpdoc_completions ────────────────────────────────────

#[test]
fn completions_bare_at_function() {
    let content = "<?php\n/**\n * @\n */\nfunction foo(): void {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // Should suggest function tags (some may have pre-filled info)
    assert!(
        labels
            .iter()
            .any(|l| l.starts_with("@param") || l == &"@param Type $name"),
        "Should suggest @param. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("@return")),
        "Should suggest @return. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("@throws")),
        "Should suggest @throws. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l == &"@deprecated"),
        "Should suggest @deprecated"
    );
    assert!(
        labels.iter().any(|l| l.starts_with("@phpstan-assert")),
        "Should suggest @phpstan-assert"
    );

    // Should NOT suggest class-only tags
    assert!(
        !labels.iter().any(|l| l.starts_with("@property")),
        "Should NOT suggest @property in function context"
    );
    assert!(
        !labels.iter().any(|l| l.starts_with("@method")),
        "Should NOT suggest @method in function context"
    );
    assert!(
        !labels.iter().any(|l| l.starts_with("@mixin")),
        "Should NOT suggest @mixin in function context"
    );
}

#[test]
fn completions_bare_at_class() {
    let content = "<?php\n/**\n * @\n */\nclass Foo {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::ClassLike, pos);
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();

    assert!(
        filter_texts.contains(&"@property"),
        "Should suggest @property"
    );
    assert!(filter_texts.contains(&"@method"), "Should suggest @method");
    assert!(filter_texts.contains(&"@mixin"), "Should suggest @mixin");
    assert!(
        filter_texts.contains(&"@template"),
        "Should suggest @template"
    );
    assert!(
        filter_texts.contains(&"@deprecated"),
        "Should suggest @deprecated"
    );

    // Should NOT suggest function-only tags
    assert!(
        !filter_texts.contains(&"@param"),
        "Should NOT suggest @param in class context"
    );
    assert!(
        !filter_texts.contains(&"@return"),
        "Should NOT suggest @return in class context"
    );
    assert!(
        !filter_texts.contains(&"@throws"),
        "Should NOT suggest @throws in class context"
    );
}

#[test]
fn completions_bare_at_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public string $name;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::Property, pos);
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();

    assert!(filter_texts.contains(&"@var"), "Should suggest @var");
    assert!(
        filter_texts.contains(&"@deprecated"),
        "Should suggest @deprecated"
    );

    assert!(
        !filter_texts.contains(&"@param"),
        "Should NOT suggest @param in property context"
    );
    assert!(
        !filter_texts.contains(&"@return"),
        "Should NOT suggest @return in property context"
    );
    assert!(
        !filter_texts.contains(&"@method"),
        "Should NOT suggest @method in property context"
    );
}

#[test]
fn completions_bare_at_constant() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    const X = 1;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::Constant, pos);
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();

    assert!(filter_texts.contains(&"@var"), "Should suggest @var");
    assert!(
        filter_texts.contains(&"@deprecated"),
        "Should suggest @deprecated"
    );

    assert!(
        !filter_texts.contains(&"@param"),
        "Should NOT suggest @param in constant context"
    );
}

#[test]
fn completions_unknown_includes_all() {
    let content = "<?php\n/**\n * @\n */\n\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::Unknown, pos);
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();

    assert!(filter_texts.contains(&"@param"), "Should suggest @param");
    assert!(filter_texts.contains(&"@return"), "Should suggest @return");
    assert!(
        filter_texts.contains(&"@property"),
        "Should suggest @property"
    );
    assert!(filter_texts.contains(&"@method"), "Should suggest @method");
    assert!(filter_texts.contains(&"@var"), "Should suggest @var");
    assert!(
        filter_texts.contains(&"@deprecated"),
        "Should suggest @deprecated"
    );
}

#[test]
fn completions_filtered_by_prefix() {
    let content = "<?php\n/**\n * @par\n */\nfunction foo(): void {}\n";
    let pos = Position {
        line: 2,
        character: 7,
    };
    let items = build_phpdoc_completions(content, "@par", DocblockContext::FunctionOrMethod, pos);
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();

    assert!(filter_texts.contains(&"@param"), "Should suggest @param");
    assert!(
        !filter_texts.contains(&"@return"),
        "Should NOT suggest @return for prefix @par"
    );
}

#[test]
fn completions_phpstan_prefix() {
    let content = "<?php\n/**\n * @phpstan-a\n */\nfunction foo(): void {}\n";
    let pos = Position {
        line: 2,
        character: 14,
    };
    let items = build_phpdoc_completions(
        content,
        "@phpstan-a",
        DocblockContext::FunctionOrMethod,
        pos,
    );
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();

    assert!(
        filter_texts.contains(&"@phpstan-assert"),
        "Should suggest @phpstan-assert"
    );
    assert!(
        filter_texts.contains(&"@phpstan-assert-if-true"),
        "Should suggest @phpstan-assert-if-true"
    );
    assert!(
        filter_texts.contains(&"@phpstan-assert-if-false"),
        "Should suggest @phpstan-assert-if-false"
    );
    assert!(
        !filter_texts.contains(&"@phpstan-self-out"),
        "Should NOT suggest @phpstan-self-out for prefix @phpstan-a"
    );
}

#[test]
fn completions_case_insensitive() {
    let content = "<?php\n/**\n * @PAR\n */\nfunction foo(): void {}\n";
    let pos = Position {
        line: 2,
        character: 7,
    };
    let items = build_phpdoc_completions(content, "@PAR", DocblockContext::FunctionOrMethod, pos);
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();

    assert!(
        filter_texts.contains(&"@param"),
        "Should match case-insensitively"
    );
}

#[test]
fn completions_have_keyword_kind() {
    let content = "<?php\n/**\n * @\n */\nfunction foo(): void {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);
    for item in &items {
        assert_eq!(
            item.kind,
            Some(CompletionItemKind::KEYWORD),
            "PHPDoc tags should use KEYWORD kind"
        );
    }
}

#[test]
fn completions_no_duplicates() {
    let content = "<?php\n/**\n * @\n */\n\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::Unknown, pos);
    let filter_texts: Vec<&str> = items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect();
    let unique: std::collections::HashSet<&&str> = filter_texts.iter().collect();
    assert_eq!(
        filter_texts.len(),
        unique.len(),
        "Should not have duplicate tags. Got: {:?}",
        filter_texts
    );
}

// ── Smart pre-fill tests ────────────────────────────────────────

#[test]
fn smart_param_completions_per_parameter() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function greet(string $name, int $age): string {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let param_items: Vec<_> = items
        .iter()
        .filter(|i| i.filter_text.as_deref() == Some("@param"))
        .collect();

    // Should have one item per parameter
    assert_eq!(
        param_items.len(),
        2,
        "Should have one @param per parameter. Got: {:?}",
        param_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );

    assert_eq!(param_items[0].label, "@param string $name");
    assert_eq!(
        param_items[0].insert_text.as_deref(),
        Some("param string $name")
    );
    assert_eq!(param_items[1].label, "@param int $age");
    assert_eq!(
        param_items[1].insert_text.as_deref(),
        Some("param int $age")
    );
}

#[test]
fn smart_param_skips_already_documented() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @param string $name\n",
        " * @\n",
        " */\n",
        "function greet(string $name, int $age): string {}\n",
    );
    let pos = Position {
        line: 3,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let param_items: Vec<_> = items
        .iter()
        .filter(|i| i.filter_text.as_deref() == Some("@param"))
        .collect();

    // $name is already documented, only $age should appear
    assert_eq!(
        param_items.len(),
        1,
        "Should only suggest undocumented params. Got: {:?}",
        param_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
    assert_eq!(param_items[0].label, "@param int $age");
}

#[test]
fn smart_return_prefilled() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function getName(): string {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(
        return_item.is_some(),
        "Should have @return item. Got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
    let r = return_item.unwrap();
    assert_eq!(r.label, "@return string");
    assert_eq!(r.insert_text.as_deref(), Some("return string"));
}

#[test]
fn smart_return_void_uses_generic_label() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function doStuff(): void {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(return_item.is_some(), "Should have @return item");
    let r = return_item.unwrap();
    // void return → generic label (no point pre-filling @return void)
    assert_eq!(r.label, "@return Type");
    assert_eq!(r.insert_text.as_deref(), Some("return"));
}

#[test]
fn smart_return_skipped_when_already_documented() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @return string\n",
        " * @\n",
        " */\n",
        "function getName(): string {}\n",
    );
    let pos = Position {
        line: 3,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let return_items: Vec<_> = items
        .iter()
        .filter(|i| i.filter_text.as_deref() == Some("@return"))
        .collect();

    assert!(
        return_items.is_empty(),
        "Should NOT suggest @return when already documented. Got: {:?}",
        return_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

#[test]
fn smart_var_prefilled_for_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public string $name;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::Property, pos);

    let var_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@var"));
    assert!(var_item.is_some(), "Should have @var item");
    let v = var_item.unwrap();
    assert_eq!(v.label, "@var string");
    assert_eq!(v.insert_text.as_deref(), Some("var string"));
}

#[test]
fn smart_var_nullable_property() {
    let content = concat!(
        "<?php\nclass Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    protected ?int $count = 0;\n",
        "}\n",
    );
    let pos = Position {
        line: 3,
        character: 8,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::Property, pos);

    let var_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@var"));
    assert!(var_item.is_some(), "Should have @var item");
    assert_eq!(var_item.unwrap().label, "@var ?int");
}

#[test]
fn display_labels_for_generic_tags() {
    let content = "<?php\n/**\n * @\n */\nclass Foo {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::ClassLike, pos);

    let method_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@method"));
    assert!(method_item.is_some(), "Should have @method item");
    assert_eq!(
        method_item.unwrap().label,
        "@method ReturnType name()",
        "@method should show usage pattern as label"
    );

    let template_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@template"));
    assert!(template_item.is_some(), "Should have @template item");
    assert_eq!(
        template_item.unwrap().label,
        "@template T",
        "@template should show usage pattern as label"
    );
}

#[test]
fn display_labels_for_general_tags() {
    let content = "<?php\n/**\n * @\n */\nfunction foo(): void {}\n";
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let throws_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@throws"));
    assert!(throws_item.is_some(), "Should have @throws item");
    assert_eq!(throws_item.unwrap().label, "@throws ExceptionType");

    // Tags with no special format should use tag as label
    let deprecated_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@deprecated"));
    assert!(deprecated_item.is_some(), "Should have @deprecated item");
    assert_eq!(deprecated_item.unwrap().label, "@deprecated");
}

#[test]
fn smart_param_untyped_params() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function process($data, $options) {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let param_items: Vec<_> = items
        .iter()
        .filter(|i| i.filter_text.as_deref() == Some("@param"))
        .collect();

    assert_eq!(param_items.len(), 2);
    assert_eq!(param_items[0].label, "@param $data");
    assert_eq!(param_items[0].insert_text.as_deref(), Some("param $data"));
    assert_eq!(param_items[1].label, "@param $options");
}

#[test]
fn smart_return_nullable() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function find(): ?User {}\n",
    );
    let pos = Position {
        line: 2,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(return_item.is_some());
    assert_eq!(return_item.unwrap().label, "@return ?User");
}

#[test]
fn all_params_documented_falls_back_to_generic() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * @param string $name\n",
        " * @\n",
        " */\n",
        "function greet(string $name): string {}\n",
    );
    let pos = Position {
        line: 3,
        character: 4,
    };
    let items = build_phpdoc_completions(content, "@", DocblockContext::FunctionOrMethod, pos);

    let param_items: Vec<_> = items
        .iter()
        .filter(|i| i.filter_text.as_deref() == Some("@param"))
        .collect();

    // All params documented → falls back to generic @param
    assert_eq!(param_items.len(), 1);
    assert_eq!(param_items[0].label, "@param Type $name");
    assert_eq!(param_items[0].insert_text.as_deref(), Some("param"));
}
