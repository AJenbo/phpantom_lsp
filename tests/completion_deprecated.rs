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

// ─── Deprecated method ──────────────────────────────────────────────────────

/// A method marked `@deprecated` in its PHPDoc should have
/// `deprecated: Some(true)` in its completion item.
#[tokio::test]
async fn test_deprecated_method_is_marked() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Mailer {\n",
        "    /**\n",
        "     * @deprecated Use sendAsync() instead.\n",
        "     */\n",
        "    public function sendLegacy(): void {}\n",
        "\n",
        "    public function sendAsync(): void {}\n",
        "\n",
        "    public function run(): void {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    // Cursor after `$this->` on line 10
    let items = complete_at(&backend, &uri, text, 10, 15).await;

    let legacy = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("sendLegacy"));
    assert!(legacy.is_some(), "Should find sendLegacy in completions");
    assert_eq!(
        legacy.unwrap().deprecated,
        Some(true),
        "sendLegacy should be marked deprecated"
    );

    let async_method = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("sendAsync"));
    assert!(
        async_method.is_some(),
        "Should find sendAsync in completions"
    );
    assert_ne!(
        async_method.unwrap().deprecated,
        Some(true),
        "sendAsync should NOT be marked deprecated"
    );
}

// ─── Non-deprecated method ──────────────────────────────────────────────────

/// A method without `@deprecated` should NOT be marked deprecated.
#[tokio::test]
async fn test_non_deprecated_method_is_not_marked() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///non_deprecated_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Service {\n",
        "    /**\n",
        "     * @return void\n",
        "     */\n",
        "    public function doWork(): void {}\n",
        "\n",
        "    public function run(): void {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 8, 15).await;

    let work = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("doWork"));
    assert!(work.is_some(), "Should find doWork in completions");
    assert_ne!(
        work.unwrap().deprecated,
        Some(true),
        "doWork should NOT be marked deprecated"
    );
}

// ─── Deprecated property ────────────────────────────────────────────────────

/// A property marked `@deprecated` in its PHPDoc should have
/// `deprecated: Some(true)` in its completion item.
#[tokio::test]
async fn test_deprecated_property_is_marked() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_property.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    /**\n",
        "     * @deprecated Use getDebugMode() instead.\n",
        "     */\n",
        "    public bool $debug = false;\n",
        "\n",
        "    public string $name = 'app';\n",
        "\n",
        "    public function test(): void {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 10, 15).await;

    let debug = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("debug"));
    assert!(debug.is_some(), "Should find debug in completions");
    assert_eq!(
        debug.unwrap().deprecated,
        Some(true),
        "debug property should be marked deprecated"
    );

    let name = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("name"));
    assert!(name.is_some(), "Should find name in completions");
    assert_ne!(
        name.unwrap().deprecated,
        Some(true),
        "name property should NOT be marked deprecated"
    );
}

// ─── Deprecated constant ────────────────────────────────────────────────────

/// A class constant marked `@deprecated` in its PHPDoc should have
/// `deprecated: Some(true)` in its completion item.
#[tokio::test]
async fn test_deprecated_constant_is_marked() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_constant.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class HttpStatus {\n",
        "    /**\n",
        "     * @deprecated Use OK instead.\n",
        "     */\n",
        "    const SUCCESS = 200;\n",
        "\n",
        "    const OK = 200;\n",
        "}\n",
        "$x = HttpStatus::\n",
    );

    // Cursor after `HttpStatus::` on line 9
    let items = complete_at(&backend, &uri, text, 9, 17).await;

    let success = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("SUCCESS"));
    assert!(success.is_some(), "Should find SUCCESS in completions");
    assert_eq!(
        success.unwrap().deprecated,
        Some(true),
        "SUCCESS constant should be marked deprecated"
    );

    let ok = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("OK"));
    assert!(ok.is_some(), "Should find OK in completions");
    assert_ne!(
        ok.unwrap().deprecated,
        Some(true),
        "OK constant should NOT be marked deprecated"
    );
}

// ─── Deprecated function ────────────────────────────────────────────────────

/// A standalone function marked `@deprecated` in its PHPDoc should have
/// `deprecated: Some(true)` in its completion item.
#[tokio::test]
async fn test_deprecated_function_is_marked() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_function.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @deprecated Use newHelper() instead.\n",
        " */\n",
        "function oldHelper(): void {}\n",
        "\n",
        "function newHelper(): void {}\n",
        "\n",
        "Helper\n",
    );

    // Cursor at end of `Helper` on line 8 — matches both oldHelper and newHelper
    let items = complete_at(&backend, &uri, text, 8, 6).await;

    let old = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("oldHelper"));
    assert!(
        old.is_some(),
        "Should find oldHelper in completions. Got: {:?}",
        items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(
        old.unwrap().deprecated,
        Some(true),
        "oldHelper should be marked deprecated"
    );

    let new = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("newHelper"));
    assert!(
        new.is_some(),
        "Should find newHelper in completions. Got: {:?}",
        items.iter().map(|i| i.label.as_str()).collect::<Vec<_>>()
    );
    assert_ne!(
        new.unwrap().deprecated,
        Some(true),
        "newHelper should NOT be marked deprecated"
    );
}

// ─── Deprecated method with bare @deprecated ────────────────────────────────

/// A method with a bare `@deprecated` tag (no description) should still
/// be marked deprecated.
#[tokio::test]
async fn test_deprecated_method_bare_tag() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_bare.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    /**\n",
        "     * @deprecated\n",
        "     */\n",
        "    public function oldMethod(): void {}\n",
        "\n",
        "    public function test(): void {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 8, 15).await;

    let old = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("oldMethod"));
    assert!(old.is_some(), "Should find oldMethod in completions");
    assert_eq!(
        old.unwrap().deprecated,
        Some(true),
        "oldMethod should be marked deprecated even with bare @deprecated tag"
    );
}

// ─── Multiple deprecated members ────────────────────────────────────────────

/// When a class has multiple deprecated and non-deprecated members,
/// only the deprecated ones should be marked.
#[tokio::test]
async fn test_multiple_deprecated_and_non_deprecated_members() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multi_deprecated.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Api {\n",
        "    /**\n",
        "     * @deprecated Use fetchV2() instead.\n",
        "     */\n",
        "    public function fetchV1(): array { return []; }\n",
        "\n",
        "    public function fetchV2(): array { return []; }\n",
        "\n",
        "    /**\n",
        "     * @deprecated\n",
        "     */\n",
        "    public string $legacyUrl = '';\n",
        "\n",
        "    public string $baseUrl = '';\n",
        "\n",
        "    /**\n",
        "     * @deprecated Use VERSION_2 instead.\n",
        "     */\n",
        "    const VERSION_1 = 1;\n",
        "\n",
        "    const VERSION_2 = 2;\n",
        "}\n",
        "class Client {\n",
        "    public function run(Api $api): void {\n",
        "        $api->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 25, 14).await;

    // Deprecated items
    let fetch_v1 = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("fetchV1"));
    assert!(fetch_v1.is_some(), "Should find fetchV1");
    assert_eq!(
        fetch_v1.unwrap().deprecated,
        Some(true),
        "fetchV1 should be deprecated"
    );

    let legacy_url = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("legacyUrl"));
    assert!(legacy_url.is_some(), "Should find legacyUrl");
    assert_eq!(
        legacy_url.unwrap().deprecated,
        Some(true),
        "legacyUrl should be deprecated"
    );

    // Non-deprecated items
    let fetch_v2 = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("fetchV2"));
    assert!(fetch_v2.is_some(), "Should find fetchV2");
    assert_ne!(
        fetch_v2.unwrap().deprecated,
        Some(true),
        "fetchV2 should NOT be deprecated"
    );

    let base_url = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("baseUrl"));
    assert!(base_url.is_some(), "Should find baseUrl");
    assert_ne!(
        base_url.unwrap().deprecated,
        Some(true),
        "baseUrl should NOT be deprecated"
    );
}

/// Constant completion via `::` should also respect deprecated flags.
#[tokio::test]
async fn test_deprecated_constant_via_double_colon() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_const_dc.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Api {\n",
        "    /**\n",
        "     * @deprecated Use VERSION_2 instead.\n",
        "     */\n",
        "    const VERSION_1 = 1;\n",
        "\n",
        "    const VERSION_2 = 2;\n",
        "}\n",
        "$v = Api::\n",
    );

    let items = complete_at(&backend, &uri, text, 9, 10).await;

    let v1 = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("VERSION_1"));
    assert!(v1.is_some(), "Should find VERSION_1");
    assert_eq!(
        v1.unwrap().deprecated,
        Some(true),
        "VERSION_1 should be deprecated"
    );

    let v2 = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("VERSION_2"));
    assert!(v2.is_some(), "Should find VERSION_2");
    assert_ne!(
        v2.unwrap().deprecated,
        Some(true),
        "VERSION_2 should NOT be deprecated"
    );
}

// ─── Deprecated with other docblock tags ─────────────────────────────────────

/// `@deprecated` mixed with `@param`, `@return`, etc. should still be detected.
#[tokio::test]
async fn test_deprecated_mixed_with_other_tags() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_mixed.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Processor {\n",
        "    /**\n",
        "     * Process the given data.\n",
        "     *\n",
        "     * @param array $data The data to process.\n",
        "     * @deprecated since 3.0, use processV2() instead.\n",
        "     * @return bool\n",
        "     */\n",
        "    public function process(array $data): bool { return true; }\n",
        "\n",
        "    public function test(): void {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 12, 15).await;

    let process = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("process"));
    assert!(process.is_some(), "Should find process in completions");
    assert_eq!(
        process.unwrap().deprecated,
        Some(true),
        "process should be marked deprecated even with mixed tags"
    );
}

// ─── Static deprecated method ───────────────────────────────────────────────

/// Deprecated static methods should be marked when accessed via `::`.
#[tokio::test]
async fn test_deprecated_static_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    /**\n",
        "     * @deprecated Use createNew() instead.\n",
        "     */\n",
        "    public static function createLegacy(): self { return new self(); }\n",
        "\n",
        "    public static function createNew(): self { return new self(); }\n",
        "}\n",
        "Factory::\n",
    );

    let items = complete_at(&backend, &uri, text, 9, 10).await;

    let legacy = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("createLegacy"));
    assert!(legacy.is_some(), "Should find createLegacy in completions");
    assert_eq!(
        legacy.unwrap().deprecated,
        Some(true),
        "createLegacy should be marked deprecated"
    );

    let new_method = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("createNew"));
    assert!(new_method.is_some(), "Should find createNew in completions");
    assert_ne!(
        new_method.unwrap().deprecated,
        Some(true),
        "createNew should NOT be marked deprecated"
    );
}

// ─── Deprecated static property ─────────────────────────────────────────────

/// Deprecated static properties should be marked when accessed via `::`.
#[tokio::test]
async fn test_deprecated_static_property() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_static_prop.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Settings {\n",
        "    /**\n",
        "     * @deprecated Use $newDefault instead.\n",
        "     */\n",
        "    public static string $oldDefault = 'legacy';\n",
        "\n",
        "    public static string $newDefault = 'modern';\n",
        "}\n",
        "Settings::\n",
    );

    let items = complete_at(&backend, &uri, text, 9, 11).await;

    let old = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("$oldDefault"));
    assert!(
        old.is_some(),
        "Should find $oldDefault in completions. Got: {:?}",
        items
            .iter()
            .map(|i| (&i.label, &i.filter_text))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        old.unwrap().deprecated,
        Some(true),
        "$oldDefault should be marked deprecated"
    );

    let new = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("$newDefault"));
    assert!(new.is_some(), "Should find $newDefault in completions");
    assert_ne!(
        new.unwrap().deprecated,
        Some(true),
        "$newDefault should NOT be marked deprecated"
    );
}

// ─── No false positive on similar words ─────────────────────────────────────

/// A docblock mentioning "deprecated" in prose (not as a tag) should NOT
/// cause the member to be flagged.
#[tokio::test]
async fn test_deprecated_word_in_prose_not_flagged() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///deprecated_prose.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Docs {\n",
        "    /**\n",
        "     * This method replaces the deprecated v1 API.\n",
        "     */\n",
        "    public function replaceOld(): void {}\n",
        "\n",
        "    public function test(): void {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 8, 15).await;

    let method = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("replaceOld"));
    assert!(method.is_some(), "Should find replaceOld in completions");
    assert_ne!(
        method.unwrap().deprecated,
        Some(true),
        "replaceOld should NOT be marked deprecated — 'deprecated' in prose is not a tag"
    );
}
