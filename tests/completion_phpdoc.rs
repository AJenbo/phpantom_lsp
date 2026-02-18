mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

/// Helper: open a file and request completion at the given line/character.
async fn complete_at(
    backend: &phpantom_lsp::Backend,
    uri: &Url,
    text: &str,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    match backend.completion(completion_params).await.unwrap() {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        _ => vec![],
    }
}

/// Collect the filter_text values from completion items (always the raw tag name).
fn filter_texts(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect()
}

// ─── Basic trigger ──────────────────────────────────────────────────────────

/// Typing `@` inside a docblock should produce PHPDoc tag completions.
#[tokio::test]
async fn test_phpdoc_bare_at_triggers_completion() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_bare.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function foo(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    // foo() has no params, void return, and no throws — smart tags are
    // filtered out.  General tags like @deprecated should still appear.
    assert!(
        tags.contains(&"@deprecated"),
        "Should suggest @deprecated. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"@inheritdoc"),
        "Should suggest @inheritdoc. Got: {:?}",
        tags
    );
}

/// Typing `@par` should filter to tags matching the prefix.
#[tokio::test]
async fn test_phpdoc_partial_prefix_filters() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_filter.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @par\n",
        " */\n",
        "function greet(string $name): string {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 7).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"@param"),
        "Should suggest @param for prefix @par. Got: {:?}",
        tags
    );
    assert!(
        !tags.contains(&"@return"),
        "Should NOT suggest @return for prefix @par. Got: {:?}",
        tags
    );
}

// ─── Not triggered outside docblocks ────────────────────────────────────────

/// `@` in regular PHP code (e.g. error suppression) should NOT trigger
/// PHPDoc completion.
#[tokio::test]
async fn test_phpdoc_not_triggered_outside_docblock() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_outside.php").unwrap();
    let text = concat!("<?php\n", "@mkdir('/tmp/test');\n",);

    let items = complete_at(&backend, &uri, text, 1, 1).await;

    let phpdoc_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.filter_text
                .as_deref()
                .is_some_and(|ft| ft.starts_with('@'))
        })
        .collect();
    assert!(
        phpdoc_items.is_empty(),
        "Should NOT suggest PHPDoc tags outside a docblock. Got: {:?}",
        phpdoc_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// `@` inside a regular `/* ... */` comment should NOT trigger PHPDoc completion.
#[tokio::test]
async fn test_phpdoc_not_triggered_in_regular_comment() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_regular_comment.php").unwrap();
    let text = concat!("<?php\n", "/* @param string $x */\n",);

    let items = complete_at(&backend, &uri, text, 1, 4).await;

    let phpdoc_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.filter_text
                .as_deref()
                .is_some_and(|ft| ft.starts_with('@'))
        })
        .collect();
    assert!(
        phpdoc_items.is_empty(),
        "Should NOT suggest PHPDoc tags inside a regular comment. Got: {:?}",
        phpdoc_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

// ─── Context: function / method ─────────────────────────────────────────────

/// Docblock before a function should suggest function-related tags.
#[tokio::test]
async fn test_phpdoc_function_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_func.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function greet(string $name): string {\n",
        "    throw new InvalidArgumentException('bad');\n",
        "    return 'Hello ' . $name;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    // Function-specific smart tags (function has a param, non-void return, and a throw)
    assert!(tags.contains(&"@param"), "Should suggest @param");
    assert!(tags.contains(&"@return"), "Should suggest @return");
    assert!(tags.contains(&"@throws"), "Should suggest @throws");

    // General tags
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");
    assert!(tags.contains(&"@see"), "Should suggest @see");

    // PHPStan function tags
    assert!(
        tags.contains(&"@phpstan-assert"),
        "Should suggest @phpstan-assert"
    );

    // Should NOT include class-only tags
    assert!(
        !tags.contains(&"@property"),
        "Should NOT suggest @property in function context"
    );
    assert!(
        !tags.contains(&"@method"),
        "Should NOT suggest @method in function context"
    );
    assert!(
        !tags.contains(&"@mixin"),
        "Should NOT suggest @mixin in function context"
    );
}

/// Docblock before a method should also get function-related tags.
#[tokio::test]
async fn test_phpdoc_method_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Service {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public function handle(Request $request): Response {\n",
        "        throw new RuntimeException('fail');\n",
        "        return new Response();\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@param"), "Should suggest @param");
    assert!(tags.contains(&"@return"), "Should suggest @return");
    assert!(tags.contains(&"@throws"), "Should suggest @throws");
    assert!(tags.contains(&"@inheritdoc"), "Should suggest @inheritdoc");
}

/// Docblock before a static method should get function-related tags.
#[tokio::test]
async fn test_phpdoc_static_method_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_static_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public static function create(string $type): self {\n",
        "        return new self();\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@return"), "Should suggest @return");
    assert!(tags.contains(&"@param"), "Should suggest @param");
}

// ─── Context: class / interface / trait / enum ──────────────────────────────

/// Docblock before a class should suggest class-related tags.
#[tokio::test]
async fn test_phpdoc_class_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_class.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "class UserRepository {\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    // Class-specific tags
    assert!(tags.contains(&"@property"), "Should suggest @property");
    assert!(tags.contains(&"@method"), "Should suggest @method");
    assert!(tags.contains(&"@mixin"), "Should suggest @mixin");
    assert!(tags.contains(&"@template"), "Should suggest @template");
    assert!(tags.contains(&"@extends"), "Should suggest @extends");
    assert!(tags.contains(&"@implements"), "Should suggest @implements");

    // General tags
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");

    // Should NOT include function-only tags
    assert!(
        !tags.contains(&"@param"),
        "Should NOT suggest @param in class context"
    );
    assert!(
        !tags.contains(&"@return"),
        "Should NOT suggest @return in class context"
    );
    assert!(
        !tags.contains(&"@throws"),
        "Should NOT suggest @throws in class context"
    );
}

/// Docblock before an abstract class.
#[tokio::test]
async fn test_phpdoc_abstract_class_context() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_abstract.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "abstract class BaseService {\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@property"), "Should suggest @property");
    assert!(tags.contains(&"@method"), "Should suggest @method");
    assert!(
        !tags.contains(&"@param"),
        "Should NOT suggest @param in class context"
    );
}

/// Docblock before a final class.
#[tokio::test]
async fn test_phpdoc_final_class_context() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_final.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "final class Singleton {\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@property"), "Should suggest @property");
    assert!(tags.contains(&"@method"), "Should suggest @method");
}

/// Docblock before an interface should suggest class-related tags.
#[tokio::test]
async fn test_phpdoc_interface_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_iface.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "interface Cacheable {\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@method"), "Should suggest @method");
    assert!(tags.contains(&"@template"), "Should suggest @template");
    assert!(tags.contains(&"@extends"), "Should suggest @extends");
    assert!(
        !tags.contains(&"@param"),
        "Should NOT suggest @param in interface context"
    );
}

/// Docblock before a trait should suggest class-related tags.
#[tokio::test]
async fn test_phpdoc_trait_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_trait.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "trait Loggable {\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@property"), "Should suggest @property");
    assert!(tags.contains(&"@method"), "Should suggest @method");
    assert!(
        !tags.contains(&"@return"),
        "Should NOT suggest @return in trait context"
    );
}

/// Docblock before an enum should suggest class-related tags.
#[tokio::test]
async fn test_phpdoc_enum_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_enum.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "enum Status: string {\n",
        "    case Active = 'active';\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@method"), "Should suggest @method");
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");
}

// ─── Context: property ──────────────────────────────────────────────────────

/// Docblock before a property should suggest property-related tags.
#[tokio::test]
async fn test_phpdoc_property_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_prop.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public string $name;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@var"), "Should suggest @var");
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");
    // Should NOT include function or class tags
    assert!(
        !tags.contains(&"@param"),
        "Should NOT suggest @param in property context"
    );
    assert!(
        !tags.contains(&"@return"),
        "Should NOT suggest @return in property context"
    );
    assert!(
        !tags.contains(&"@method"),
        "Should NOT suggest @method in property context"
    );
    assert!(
        !tags.contains(&"@property"),
        "Should NOT suggest @property in property context"
    );
}

/// Docblock before a typed property with nullable type.
#[tokio::test]
async fn test_phpdoc_nullable_property_context() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_nullable_prop.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    protected ?string $apiKey = null;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@var"), "Should suggest @var");
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");
}

/// Docblock before a readonly property.
#[tokio::test]
async fn test_phpdoc_readonly_property_context() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_readonly_prop.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Entity {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public readonly int $id;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@var"), "Should suggest @var");
}

/// Docblock before a static property.
#[tokio::test]
async fn test_phpdoc_static_property_context() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_static_prop.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Registry {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    private static array $instances = [];\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@var"), "Should suggest @var");
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");
}

// ─── Context: constant ──────────────────────────────────────────────────────

/// Docblock before a class constant should suggest constant-related tags.
#[tokio::test]
async fn test_phpdoc_constant_context_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_const.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    const MAX_RETRIES = 3;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@var"), "Should suggest @var");
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");

    // Should NOT include function or class tags
    assert!(
        !tags.contains(&"@param"),
        "Should NOT suggest @param in constant context"
    );
    assert!(
        !tags.contains(&"@return"),
        "Should NOT suggest @return in constant context"
    );
    assert!(
        !tags.contains(&"@method"),
        "Should NOT suggest @method in constant context"
    );
}

/// Docblock before a constant with visibility.
#[tokio::test]
async fn test_phpdoc_visibility_constant_context() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_vis_const.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class HttpStatus {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public const OK = 200;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@var"), "Should suggest @var");
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");
}

// ─── PHPStan-specific tag filtering ─────────────────────────────────────────

/// Typing `@phpstan-` should suggest only PHPStan tags matching the prefix.
#[tokio::test]
async fn test_phpdoc_phpstan_prefix_filtering() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_phpstan.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @phpstan-\n",
        " */\n",
        "function check($value): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 12).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"@phpstan-assert"),
        "Should suggest @phpstan-assert"
    );
    assert!(
        tags.contains(&"@phpstan-assert-if-true"),
        "Should suggest @phpstan-assert-if-true"
    );
    assert!(
        tags.contains(&"@phpstan-assert-if-false"),
        "Should suggest @phpstan-assert-if-false"
    );

    // Regular tags should NOT match
    assert!(
        !tags.contains(&"@param"),
        "Should NOT suggest @param for prefix @phpstan-"
    );
    assert!(
        !tags.contains(&"@deprecated"),
        "Should NOT suggest @deprecated for prefix @phpstan-"
    );
}

/// PHPStan tags should be context-aware: function context should not include
/// class-only PHPStan tags.
#[tokio::test]
async fn test_phpdoc_phpstan_context_aware() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_phpstan_ctx.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @phpstan-\n",
        " */\n",
        "function transform(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 12).await;
    let tags = filter_texts(&items);

    // Function-context PHPStan tags
    assert!(
        tags.contains(&"@phpstan-assert"),
        "Should suggest @phpstan-assert in function context"
    );

    // Class-only PHPStan tags should NOT appear
    assert!(
        !tags.contains(&"@phpstan-require-extends"),
        "Should NOT suggest @phpstan-require-extends in function context"
    );
    assert!(
        !tags.contains(&"@phpstan-require-implements"),
        "Should NOT suggest @phpstan-require-implements in function context"
    );
}

/// PHPStan class tags in class context.
#[tokio::test]
async fn test_phpdoc_phpstan_class_context() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_phpstan_class.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @phpstan-\n",
        " */\n",
        "class GenericRepo {\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 12).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"@phpstan-type"),
        "Should suggest @phpstan-type in class context"
    );
    assert!(
        tags.contains(&"@phpstan-import-type"),
        "Should suggest @phpstan-import-type in class context"
    );
    assert!(
        tags.contains(&"@phpstan-require-extends"),
        "Should suggest @phpstan-require-extends in class context"
    );
    assert!(
        tags.contains(&"@phpstan-require-implements"),
        "Should suggest @phpstan-require-implements in class context"
    );

    // Function-only PHPStan tags should NOT appear
    assert!(
        !tags.contains(&"@phpstan-assert"),
        "Should NOT suggest @phpstan-assert in class context"
    );
}

// ─── Unknown context ────────────────────────────────────────────────────────

/// When the symbol after the docblock can't be determined, suggest all tags.
#[tokio::test]
async fn test_phpdoc_unknown_context_suggests_all() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_unknown.php").unwrap();
    let text = concat!("<?php\n", "/**\n", " * @\n", " */\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    // Unknown context: class tags and general tags should appear.
    // @param, @return, @throws are filtered because no function body
    // can be detected (no params, no return, no throws).
    assert!(tags.contains(&"@property"), "Should suggest @property");
    assert!(tags.contains(&"@method"), "Should suggest @method");
    assert!(tags.contains(&"@var"), "Should suggest @var");
    assert!(tags.contains(&"@deprecated"), "Should suggest @deprecated");
    assert!(tags.contains(&"@inheritdoc"), "Should suggest @inheritdoc");
}

// ─── Completion item details ────────────────────────────────────────────────

/// Completion items should have the KEYWORD kind.
#[tokio::test]
async fn test_phpdoc_items_have_keyword_kind() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_kind.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function foo(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    for item in &items {
        assert_eq!(
            item.kind,
            Some(CompletionItemKind::KEYWORD),
            "PHPDoc tag {:?} should use KEYWORD kind",
            item.label
        );
    }
}

/// Completion items should have a detail description.
#[tokio::test]
async fn test_phpdoc_items_have_detail() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_detail.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function foo(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    for item in &items {
        assert!(
            item.detail.is_some(),
            "PHPDoc tag {:?} should have a detail description",
            item.label
        );
        assert!(
            !item.detail.as_ref().unwrap().is_empty(),
            "PHPDoc tag {:?} should have a non-empty detail",
            item.label
        );
    }
}

/// Completion items should not be duplicated.
#[tokio::test]
async fn test_phpdoc_no_duplicates() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_dedup.php").unwrap();
    let text = concat!("<?php\n", "/**\n", " * @\n", " */\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);
    let unique: std::collections::HashSet<&&str> = tags.iter().collect();

    assert_eq!(
        tags.len(),
        unique.len(),
        "Should not have duplicate PHPDoc tags. Got: {:?}",
        tags
    );
}

// ─── Open (unclosed) docblock ───────────────────────────────────────────────

/// PHPDoc completion should work even when the docblock is not yet closed.
#[tokio::test]
async fn test_phpdoc_open_docblock() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_open.php").unwrap();
    let text = concat!("<?php\n", "/**\n", " * @\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;
    let tags = filter_texts(&items);

    // Should still produce completions even without closing */
    assert!(
        !tags.is_empty(),
        "Should suggest tags even in an unclosed docblock. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"@deprecated"),
        "Should suggest @deprecated. Got: {:?}",
        tags
    );
}

// ─── Multiple docblocks ─────────────────────────────────────────────────────

/// When there are multiple docblocks, only trigger for the one containing
/// the cursor.
#[tokio::test]
async fn test_phpdoc_multiple_docblocks() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_multi.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @param string $x\n",
        " */\n",
        "function first(): void {}\n",
        "\n",
        "/**\n",
        " * @\n",
        " */\n",
        "class MyClass {}\n",
    );

    // Cursor in second docblock — should get class context
    let items = complete_at(&backend, &uri, text, 7, 4).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"@property"),
        "Should suggest @property for class docblock"
    );
    assert!(
        tags.contains(&"@method"),
        "Should suggest @method for class docblock"
    );
    assert!(
        !tags.contains(&"@param"),
        "Should NOT suggest @param for class docblock"
    );
}

// ─── Case insensitivity ─────────────────────────────────────────────────────

/// Prefix matching should be case-insensitive.
#[tokio::test]
async fn test_phpdoc_case_insensitive_prefix() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_case.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @PAR\n",
        " */\n",
        "function greet(string $name): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 7).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"@param"),
        "Should match @param case-insensitively. Got: {:?}",
        tags
    );
}

// ─── Second tag on another line ─────────────────────────────────────────────

/// Adding a second tag to an existing docblock should still work.
#[tokio::test]
async fn test_phpdoc_second_tag_in_docblock() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_second.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @param string $name\n",
        " * @\n",
        " */\n",
        "function greet(string $name): string {\n",
        "    throw new RuntimeException('fail');\n",
        "    return 'Hello ' . $name;\n",
        "}\n",
    );

    // Cursor on the second `@` (line 3)
    let items = complete_at(&backend, &uri, text, 3, 4).await;
    let tags = filter_texts(&items);

    assert!(tags.contains(&"@return"), "Should suggest @return");
    assert!(tags.contains(&"@throws"), "Should suggest @throws");
}

// ─── @deprecated with reason ────────────────────────────────────────────────

/// @deprecated should be available in all contexts.
#[tokio::test]
async fn test_phpdoc_deprecated_in_all_contexts() {
    let backend = create_test_backend();

    // Function context
    let uri1 = Url::parse("file:///phpdoc_dep_func.php").unwrap();
    let text1 = concat!(
        "<?php\n",
        "/**\n",
        " * @dep\n",
        " */\n",
        "function old(): void {}\n",
    );
    let items1 = complete_at(&backend, &uri1, text1, 2, 7).await;
    assert!(
        items1
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@deprecated")),
        "Should suggest @deprecated in function context"
    );

    // Class context
    let uri2 = Url::parse("file:///phpdoc_dep_class.php").unwrap();
    let text2 = concat!(
        "<?php\n",
        "/**\n",
        " * @dep\n",
        " */\n",
        "class OldClass {}\n",
    );
    let items2 = complete_at(&backend, &uri2, text2, 2, 7).await;
    assert!(
        items2
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@deprecated")),
        "Should suggest @deprecated in class context"
    );

    // Property context
    let uri3 = Url::parse("file:///phpdoc_dep_prop.php").unwrap();
    let text3 = concat!(
        "<?php\n",
        "class Foo {\n",
        "    /**\n",
        "     * @dep\n",
        "     */\n",
        "    public string $old;\n",
        "}\n",
    );
    let items3 = complete_at(&backend, &uri3, text3, 3, 11).await;
    assert!(
        items3
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@deprecated")),
        "Should suggest @deprecated in property context"
    );

    // Constant context
    let uri4 = Url::parse("file:///phpdoc_dep_const.php").unwrap();
    let text4 = concat!(
        "<?php\n",
        "class Foo {\n",
        "    /**\n",
        "     * @dep\n",
        "     */\n",
        "    const OLD = 1;\n",
        "}\n",
    );
    let items4 = complete_at(&backend, &uri4, text4, 3, 11).await;
    assert!(
        items4
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@deprecated")),
        "Should suggest @deprecated in constant context"
    );
}

// ─── @template in class vs function ─────────────────────────────────────────

/// @template should appear in class context but not in property or constant context.
#[tokio::test]
async fn test_phpdoc_template_context_awareness() {
    let backend = create_test_backend();

    // Class context — should have @template
    let uri_class = Url::parse("file:///phpdoc_tmpl_class.php").unwrap();
    let text_class = concat!(
        "<?php\n",
        "/**\n",
        " * @templ\n",
        " */\n",
        "class Container {}\n",
    );
    let items_class = complete_at(&backend, &uri_class, text_class, 2, 9).await;
    assert!(
        items_class
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@template")),
        "Should suggest @template in class context"
    );

    // Property context — should NOT have @template
    let uri_prop = Url::parse("file:///phpdoc_tmpl_prop.php").unwrap();
    let text_prop = concat!(
        "<?php\n",
        "class Foo {\n",
        "    /**\n",
        "     * @templ\n",
        "     */\n",
        "    public string $name;\n",
        "}\n",
    );
    let items_prop = complete_at(&backend, &uri_prop, text_prop, 3, 12).await;
    assert!(
        !items_prop
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@template")),
        "Should NOT suggest @template in property context"
    );
}

// ─── @var availability ──────────────────────────────────────────────────────

/// @var should be available in property and constant contexts
/// but not in function/method or class contexts.
#[tokio::test]
async fn test_phpdoc_var_context_awareness() {
    let backend = create_test_backend();

    // Function context — should NOT have @var (use @param / @return instead)
    let uri_func = Url::parse("file:///phpdoc_var_func.php").unwrap();
    let text_func = concat!(
        "<?php\n",
        "/**\n",
        " * @va\n",
        " */\n",
        "function foo(): void {}\n",
    );
    let items_func = complete_at(&backend, &uri_func, text_func, 2, 6).await;
    assert!(
        !items_func
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@var")),
        "Should NOT suggest @var in function/method context"
    );

    // Property context
    let uri_prop = Url::parse("file:///phpdoc_var_prop.php").unwrap();
    let text_prop = concat!(
        "<?php\n",
        "class Foo {\n",
        "    /**\n",
        "     * @va\n",
        "     */\n",
        "    public $name;\n",
        "}\n",
    );
    let items_prop = complete_at(&backend, &uri_prop, text_prop, 3, 10).await;
    assert!(
        items_prop
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@var")),
        "Should suggest @var in property context"
    );

    // Class context — should NOT have @var
    let uri_class = Url::parse("file:///phpdoc_var_class.php").unwrap();
    let text_class = concat!("<?php\n", "/**\n", " * @va\n", " */\n", "class Foo {}\n",);
    let items_class = complete_at(&backend, &uri_class, text_class, 2, 6).await;
    assert!(
        !items_class
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@var")),
        "Should NOT suggest @var in class context"
    );
}

// ─── @inheritdoc only in function / method ──────────────────────────────────

/// @inheritdoc should only appear in function/method context.
#[tokio::test]
async fn test_phpdoc_inheritdoc_context() {
    let backend = create_test_backend();

    // Method context — should have @inheritdoc
    let uri_method = Url::parse("file:///phpdoc_inherit_method.php").unwrap();
    let text_method = concat!(
        "<?php\n",
        "class Child extends Base {\n",
        "    /**\n",
        "     * @inherit\n",
        "     */\n",
        "    public function doWork(): void {}\n",
        "}\n",
    );
    let items_method = complete_at(&backend, &uri_method, text_method, 3, 15).await;
    assert!(
        items_method
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@inheritdoc")),
        "Should suggest @inheritdoc in method context"
    );

    // Class context — should NOT have @inheritdoc
    let uri_class = Url::parse("file:///phpdoc_inherit_class.php").unwrap();
    let text_class = concat!(
        "<?php\n",
        "/**\n",
        " * @inherit\n",
        " */\n",
        "class Child extends Base {}\n",
    );
    let items_class = complete_at(&backend, &uri_class, text_class, 2, 11).await;
    assert!(
        !items_class
            .iter()
            .any(|i| i.filter_text.as_deref() == Some("@inheritdoc")),
        "Should NOT suggest @inheritdoc in class context"
    );
}

// ─── Property-related tags only in class context ────────────────────────────

/// @property should only appear in class context, not function context.
#[tokio::test]
async fn test_phpdoc_magic_property_tags_context() {
    let backend = create_test_backend();

    // Class context — should have @property
    let uri_class = Url::parse("file:///phpdoc_magic_class.php").unwrap();
    let text_class = concat!(
        "<?php\n",
        "/**\n",
        " * @property\n",
        " */\n",
        "class Magic {}\n",
    );
    let items_class = complete_at(&backend, &uri_class, text_class, 2, 12).await;
    let tags = filter_texts(&items_class);
    assert!(tags.contains(&"@property"), "Should suggest @property");

    // Function context — should NOT have @property
    let uri_func = Url::parse("file:///phpdoc_magic_func.php").unwrap();
    let text_func = concat!(
        "<?php\n",
        "/**\n",
        " * @property\n",
        " */\n",
        "function notAClass(): void {}\n",
    );
    let items_func = complete_at(&backend, &uri_func, text_func, 2, 12).await;
    let func_tags = filter_texts(&items_func);
    assert!(
        !func_tags.contains(&"@property"),
        "Should NOT suggest @property in function context. Got: {:?}",
        func_tags
    );
}

// ─── Display labels ─────────────────────────────────────────────────────────

/// Tags with a specific format should show a display label indicating usage.
#[tokio::test]
async fn test_phpdoc_display_labels_show_usage_format() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_display.php").unwrap();
    let text = concat!("<?php\n", "/**\n", " * @\n", " */\n", "class Foo {}\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    // Tags with formats should show the format in label
    let method_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@method"));
    assert!(method_item.is_some(), "Should have @method item");
    assert_eq!(
        method_item.unwrap().label,
        "@method ReturnType name()",
        "@method label should show usage pattern"
    );

    let template_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@template"));
    assert!(template_item.is_some(), "Should have @template item");
    assert_eq!(
        template_item.unwrap().label,
        "@template T",
        "@template label should show usage pattern"
    );

    // Tags without a special format should use the raw tag as label
    let deprecated_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@deprecated"));
    assert!(deprecated_item.is_some(), "Should have @deprecated item");
    assert_eq!(
        deprecated_item.unwrap().label,
        "@deprecated",
        "@deprecated should use tag as label"
    );
}

/// The insert_text for generic tags should be the raw tag name only,
/// not the display format.
#[tokio::test]
async fn test_phpdoc_insert_text_is_raw_tag() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_insert.php").unwrap();
    let text = concat!("<?php\n", "/**\n", " * @\n", " */\n", "class Foo {}\n",);

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let method_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@method"));
    assert!(method_item.is_some(), "Should have @method item");
    assert_eq!(
        method_item.unwrap().insert_text.as_deref(),
        Some("method"),
        "@method insert_text should be the raw tag without @"
    );

    let template_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@template"));
    assert!(template_item.is_some(), "Should have @template item");
    assert_eq!(
        template_item.unwrap().insert_text.as_deref(),
        Some("template"),
        "@template insert_text should be the raw tag without @"
    );
}

// ─── Smart pre-fill integration tests ───────────────────────────────────────

/// @param items should be pre-filled with parameter types and names
/// extracted from the function declaration.
#[tokio::test]
async fn test_phpdoc_smart_param_prefilled() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_param.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function greet(string $name, int $age): string {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

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

/// @return should be pre-filled with the return type hint.
#[tokio::test]
async fn test_phpdoc_smart_return_prefilled() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_return.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function getName(): string {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(return_item.is_some(), "Should have @return item");
    let r = return_item.unwrap();
    assert_eq!(r.label, "@return string");
    assert_eq!(r.insert_text.as_deref(), Some("return string"));
}

/// When a function has an explicit `: void` type hint, `@return` should
/// not be suggested at all — the type hint speaks for itself.
#[tokio::test]
async fn test_phpdoc_smart_return_void_generic() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_void.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function doStuff(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    // Explicit `: void` type hint → @return is not needed
    assert!(
        return_item.is_none(),
        "Should NOT suggest @return when `: void` type hint is present. Got: {:?}",
        return_item.map(|i| &i.label)
    );
}

/// When an explicit `: void` type hint is present, `@return` should not
/// be suggested even if the body contains return statements with values.
#[tokio::test]
async fn test_phpdoc_smart_return_void_with_return_value() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_void_returns.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function doStuff(): void {\n",
        "    return $this->something();\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(
        return_item.is_none(),
        "Should NOT suggest @return when `: void` type hint is present. Got: {:?}",
        return_item.map(|i| &i.label)
    );
}

/// When an explicit `: void` type hint is present, `@return` should not
/// be suggested even with bare `return;` statements.
#[tokio::test]
async fn test_phpdoc_smart_return_void_with_bare_return() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_void_bare.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function doStuff(): void {\n",
        "    if (true) {\n",
        "        return;\n",
        "    }\n",
        "    echo 'done';\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(
        return_item.is_none(),
        "Should NOT suggest @return when `: void` type hint is present. Got: {:?}",
        return_item.map(|i| &i.label)
    );
}

/// @var should be pre-filled with the property type hint.
#[tokio::test]
async fn test_phpdoc_smart_var_prefilled() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public string $name;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;

    let var_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@var"));
    assert!(var_item.is_some(), "Should have @var item");
    let v = var_item.unwrap();
    assert_eq!(v.label, "@var string");
    assert_eq!(v.insert_text.as_deref(), Some("var string"));
}

/// Smart @param should skip parameters that are already documented.
#[tokio::test]
async fn test_phpdoc_smart_param_skips_documented() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_skip.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @param string $name\n",
        " * @\n",
        " */\n",
        "function greet(string $name, int $age): string {}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 4).await;

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

/// @return should not be suggested when already documented.
#[tokio::test]
async fn test_phpdoc_smart_return_skipped_when_documented() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_ret_skip.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @return string\n",
        " * @\n",
        " */\n",
        "function getName(): string {}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 4).await;

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

/// @var should be pre-filled with nullable type.
#[tokio::test]
async fn test_phpdoc_smart_var_nullable() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_nullable.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    protected ?int $count = 0;\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;

    let var_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@var"));
    assert!(var_item.is_some(), "Should have @var item");
    assert_eq!(var_item.unwrap().label, "@var ?int");
}

/// @return should be pre-filled with nullable return type.
#[tokio::test]
async fn test_phpdoc_smart_return_nullable() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_ret_null.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function find(): ?User {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(return_item.is_some(), "Should have @return item");
    assert_eq!(return_item.unwrap().label, "@return ?User");
}

/// Smart @param for untyped parameters should still show parameter names.
#[tokio::test]
async fn test_phpdoc_smart_param_untyped() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_untyped.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function process($data, $options) {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 4).await;

    let param_items: Vec<_> = items
        .iter()
        .filter(|i| i.filter_text.as_deref() == Some("@param"))
        .collect();

    assert_eq!(param_items.len(), 2);
    assert_eq!(param_items[0].label, "@param $data");
    assert_eq!(param_items[1].label, "@param $options");
}

/// When all params are documented, fall back to generic @param.
#[tokio::test]
async fn test_phpdoc_smart_all_params_documented() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_smart_all_doc.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @param string $name\n",
        " * @\n",
        " */\n",
        "function greet(string $name): string {}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 4).await;

    let param_items: Vec<_> = items
        .iter()
        .filter(|i| i.filter_text.as_deref() == Some("@param"))
        .collect();

    // All params documented → @param is filtered out entirely
    assert!(
        param_items.is_empty(),
        "Should NOT suggest @param when all params are documented. Got: {:?}",
        param_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

// ─── Docblock @ prefix isolation ────────────────────────────────────────────
// When the cursor is inside a docblock on a word starting with `@`, ONLY
// PHPDoc tag suggestions should appear — never class names, constants, or
// functions that happen to match the text after `@`.

/// Typing `@potato` in a docblock should return an empty list, not class
/// names or constants that contain "potato".
#[tokio::test]
async fn test_phpdoc_no_class_completion_for_unknown_tag() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_potato.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class PotatoFactory {}\n",
        "define('WORLD_POTATO_CONSUMPTION', 42);\n",
        "/**\n",
        " * @potato\n",
        " */\n",
        "function cook(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 11).await;

    // No PHPDoc tags start with `@potato`, so the list should be empty.
    // Crucially, PotatoFactory and WORLD_POTATO_CONSUMPTION must NOT appear.
    assert!(
        items.is_empty(),
        "Typing @potato in a docblock should yield no completions, got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Typing `@throw` should suggest `@throws` (the matching PHPDoc tag),
/// not class names containing "throw".
#[tokio::test]
async fn test_phpdoc_partial_tag_suggests_matching_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_throw.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @throw\n",
        " */\n",
        "function risky(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 10).await;

    // Should contain the generic @throws fallback
    let throws_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@throws"));
    assert!(
        throws_item.is_some(),
        "Typing @throw should suggest @throws tag, got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );

    // Should NOT contain class items
    let class_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::CLASS))
        .collect();
    assert!(
        class_items.is_empty(),
        "No class items should appear in docblock @tag context, got: {:?}",
        class_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Typing `@re` should suggest `@return` (and other matching tags) but
/// never class names or constants.
#[tokio::test]
async fn test_phpdoc_partial_re_suggests_return_not_classes() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_re.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Renderer {}\n",
        "/**\n",
        " * @re\n",
        " */\n",
        "function render(): string { return ''; }\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 7).await;

    // Should contain @return
    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(
        return_item.is_some(),
        "Typing @re should suggest @return, got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );

    // Should NOT contain Renderer or any class
    let class_items: Vec<_> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::CLASS))
        .collect();
    assert!(
        class_items.is_empty(),
        "Class items like Renderer must not appear in docblock, got: {:?}",
        class_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Typing just `@` should show all applicable PHPDoc tags, not classes.
#[tokio::test]
async fn test_phpdoc_at_sign_only_shows_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_at.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SomeClass {}\n",
        "/**\n",
        " * @\n",
        " */\n",
        "function demo(): void {}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 4).await;

    // Should have PHPDoc tags
    assert!(
        !items.is_empty(),
        "Typing @ in docblock should suggest PHPDoc tags"
    );

    // Every item should be a KEYWORD (PHPDoc tag), not a CLASS or CONSTANT
    for item in &items {
        assert_eq!(
            item.kind,
            Some(CompletionItemKind::KEYWORD),
            "All items in docblock @ context should be KEYWORD, got {:?} for '{}'",
            item.kind,
            item.label
        );
    }
}

// ─── @return void with no type hint ─────────────────────────────────────────

/// When a function has no return type hint and an empty body, `@return void`
/// should be suggested (not the generic `@return Type`).
#[tokio::test]
async fn test_phpdoc_return_void_no_type_hint_empty_body() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_void_no_hint.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Demo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public function singleCatch() { }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(
        return_item.is_some(),
        "Should suggest @return void when no type hint and empty body"
    );
    assert_eq!(
        return_item.unwrap().label,
        "@return void",
        "Label should be @return void, not the generic fallback"
    );
    assert_eq!(
        return_item.unwrap().insert_text.as_deref(),
        Some("return void"),
    );
}

/// When a function has no return type hint but DOES return a value,
/// the generic `@return Type` fallback should appear instead of `@return void`.
#[tokio::test]
async fn test_phpdoc_return_no_type_hint_with_return_value() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_no_hint_ret.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Demo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public function getData() {\n",
        "        return ['key' => 'value'];\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    // Body has `return $value;` so it's not void — show generic fallback
    assert!(
        return_item.is_some(),
        "Should suggest @return when body has return with value"
    );
    assert_eq!(
        return_item.unwrap().label,
        "@return Type",
        "Should show generic @return Type, not @return void"
    );
}

/// When a function has no return type hint and only bare `return;` statements,
/// `@return void` should still be suggested.
#[tokio::test]
async fn test_phpdoc_return_void_no_hint_bare_return() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///phpdoc_no_hint_bare.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Demo {\n",
        "    /**\n",
        "     * @\n",
        "     */\n",
        "    public function process() {\n",
        "        if (true) {\n",
        "            return;\n",
        "        }\n",
        "        echo 'done';\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;

    let return_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("@return"));
    assert!(
        return_item.is_some(),
        "Should suggest @return void when body only has bare return;"
    );
    assert_eq!(return_item.unwrap().label, "@return void");
}
