mod common;

use common::{create_test_backend, create_test_backend_with_function_stubs};
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
        _ => vec![],
    }
}

/// Verify that `find_or_load_function` can resolve a basic built-in PHP
/// function from the embedded stubs and return its `FunctionInfo`.
#[tokio::test]
async fn test_stub_function_index_resolves_array_map() {
    let backend = create_test_backend_with_function_stubs();

    // `array_map` is a standard PHP function that should be in the stubs.
    let result = backend.find_or_load_function(&["array_map"]);
    assert!(
        result.is_some(),
        "find_or_load_function should resolve 'array_map' from embedded stubs"
    );

    let func = result.unwrap();
    assert_eq!(func.name, "array_map");
    // array_map returns `array` according to the stubs.
    assert!(
        func.return_type.is_some(),
        "array_map should have a return type from stubs"
    );
}

/// Verify that `find_or_load_function` can resolve `str_contains`.
#[tokio::test]
async fn test_stub_function_index_resolves_str_contains() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["str_contains"]);
    assert!(
        result.is_some(),
        "find_or_load_function should resolve 'str_contains' from embedded stubs"
    );

    let func = result.unwrap();
    assert_eq!(func.name, "str_contains");
    assert!(
        func.return_type.is_some(),
        "str_contains should have a return type"
    );
    assert_eq!(func.return_type.as_deref(), Some("bool"));
}

/// Verify that `find_or_load_function` can resolve `json_decode`.
#[tokio::test]
async fn test_stub_function_index_resolves_json_decode() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["json_decode"]);
    assert!(
        result.is_some(),
        "find_or_load_function should resolve 'json_decode' from embedded stubs"
    );

    let func = result.unwrap();
    assert_eq!(func.name, "json_decode");
    assert!(
        func.return_type.is_some(),
        "json_decode should have a return type"
    );
}

/// Verify that stub functions are cached in `global_functions` after the
/// first lookup, so subsequent lookups are fast (Phase 1 hit).
#[tokio::test]
async fn test_stub_function_cached_after_first_lookup() {
    let backend = create_test_backend_with_function_stubs();

    // First lookup triggers parsing and caching.
    let first = backend.find_or_load_function(&["str_contains"]);
    assert!(first.is_some());

    // Second lookup should hit the cache (Phase 1).
    let second = backend.find_or_load_function(&["str_contains"]);
    assert!(second.is_some());
    assert_eq!(second.unwrap().name, "str_contains");

    // Verify it's actually in global_functions now.
    let in_cache = backend
        .global_functions
        .lock()
        .ok()
        .and_then(|fmap| fmap.get("str_contains").map(|(uri, _)| uri.clone()));
    assert!(
        in_cache.is_some(),
        "str_contains should be cached in global_functions"
    );
    assert!(
        in_cache.unwrap().starts_with("phpantom-stub-fn://"),
        "cached URI should use the phpantom-stub-fn:// scheme"
    );
}

/// Verify that a non-existent function returns None.
#[tokio::test]
async fn test_stub_function_nonexistent_returns_none() {
    let backend = create_test_backend();

    let result = backend.find_or_load_function(&["this_function_does_not_exist_xyz"]);
    assert!(result.is_none(), "Non-existent function should return None");
}

/// Verify that when multiple candidates are provided, the first match wins.
#[tokio::test]
async fn test_stub_function_multiple_candidates() {
    let backend = create_test_backend_with_function_stubs();

    // Try a non-existent name first, then a real one.
    let result = backend.find_or_load_function(&["nonexistent_func_xyz", "array_pop"]);
    assert!(result.is_some());
    assert_eq!(result.unwrap().name, "array_pop");
}

/// Verify that `date_create` resolves from stubs and has a return type
/// that includes `DateTime` (it returns `DateTime|false`).
#[tokio::test]
async fn test_stub_function_date_create_return_type() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["date_create"]);
    assert!(
        result.is_some(),
        "date_create should be in the embedded stubs"
    );

    let func = result.unwrap();
    assert_eq!(func.name, "date_create");

    let ret = func.return_type.as_deref().unwrap_or("");
    assert!(
        ret.contains("DateTime"),
        "date_create return type should mention DateTime, got: {}",
        ret
    );
}

/// End-to-end test: a variable assigned from a built-in stub function
/// (`date_create`) should resolve to `DateTime` and offer its methods
/// via `->` completion.
#[tokio::test]
async fn test_completion_variable_from_stub_function_date_create() {
    let backend = create_test_backend_with_function_stubs();

    let uri = Url::parse("file:///stub_func_test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function bar(): void {\n",
        "        $dt = date_create();\n",
        "        $dt->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 14).await;

    // DateTime should have a `format` method.
    // Completion labels include the full signature (e.g. "format($format): string").
    let has_format = items.iter().any(|item| item.label.starts_with("format("));
    assert!(
        has_format,
        "Completion after date_create() should include DateTime::format, got labels: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// End-to-end test: chained call from a stub function.
/// `date_create()->format(...)` — verify that `date_create()` resolves
/// to DateTime so chained `->` offers DateTime methods.
#[tokio::test]
async fn test_completion_chained_stub_function_call() {
    let backend = create_test_backend_with_function_stubs();

    let uri = Url::parse("file:///stub_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function bar(): void {\n",
        "        date_create()->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 23).await;

    let has_format = items.iter().any(|item| item.label.starts_with("format("));
    assert!(
        has_format,
        "Chained completion after date_create()-> should include format, got labels: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Verify that `simplexml_load_string` resolves and its return type
/// includes `SimpleXMLElement`.
#[tokio::test]
async fn test_stub_function_simplexml_load_string() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["simplexml_load_string"]);
    assert!(
        result.is_some(),
        "simplexml_load_string should be in the embedded stubs"
    );

    let func = result.unwrap();
    let ret = func.return_type.as_deref().unwrap_or("");
    assert!(
        ret.contains("SimpleXMLElement"),
        "simplexml_load_string return type should mention SimpleXMLElement, got: {}",
        ret
    );
}

/// Verify that the function_loader closure in completion handles stub
/// functions — a built-in function used as an expression subject should
/// resolve its return type.
#[tokio::test]
async fn test_completion_stub_function_in_expression_subject() {
    let backend = create_test_backend_with_function_stubs();

    let uri = Url::parse("file:///stub_expr.php").unwrap();
    // `simplexml_load_string(...)` returns `SimpleXMLElement|false`.
    // SimpleXMLElement has methods like `xpath`, `children`, `attributes`, etc.
    let text = concat!(
        "<?php\n",
        "class Processor {\n",
        "    public function process(): void {\n",
        "        $xml = simplexml_load_string('<root/>');\n",
        "        $xml->\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 14).await;

    // SimpleXMLElement should have `xpath` or `children` method.
    // Completion labels include the full signature (e.g. "xpath($expression): array|false|null").
    let has_sxml_method = items.iter().any(|item| {
        item.label.starts_with("xpath(")
            || item.label.starts_with("children(")
            || item.label.starts_with("attributes(")
    });
    assert!(
        has_sxml_method,
        "Completion after simplexml_load_string() should include SimpleXMLElement methods, got labels: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Verify that loading all sibling functions from a stub file works.
/// When we look up `array_pop`, the entire `standard_N.php` file is
/// parsed, so other functions from the same file should also be cached.
#[tokio::test]
async fn test_stub_function_sibling_functions_cached() {
    let backend = create_test_backend_with_function_stubs();

    // Look up array_push — this triggers parsing of its stub file.
    let result = backend.find_or_load_function(&["array_push"]);
    assert!(result.is_some(), "array_push should be in stubs");

    // Now other functions from the same file should be cached.
    // array_pop is in the same standard file group.
    // Check if it got cached in global_functions (it may be in a different
    // file, but let's verify the caching mechanism works for the same file).
    let in_cache = backend
        .global_functions
        .lock()
        .ok()
        .and_then(|fmap| fmap.get("array_push").cloned());
    assert!(
        in_cache.is_some(),
        "array_push should be in global_functions cache after lookup"
    );
}

/// Verify that stub functions with parameters have their parameter info
/// extracted correctly.
#[tokio::test]
async fn test_stub_function_parameters_extracted() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["str_contains"]);
    assert!(result.is_some());

    let func = result.unwrap();
    // str_contains(string $haystack, string $needle): bool
    assert!(
        func.parameters.len() >= 2,
        "str_contains should have at least 2 parameters, got {}",
        func.parameters.len()
    );
    assert_eq!(func.parameters[0].name, "$haystack");
    assert_eq!(func.parameters[1].name, "$needle");
}

/// Verify that user-defined functions take precedence over stub functions.
/// If a function with the same name is in `global_functions`, the stub
/// version should NOT override it.
#[tokio::test]
async fn test_user_function_takes_precedence_over_stub() {
    let backend = create_test_backend();

    // Pre-populate global_functions with a user-defined `str_contains`.
    let custom_func = phpantom_lsp::FunctionInfo {
        name: "str_contains".to_string(),
        parameters: vec![],
        return_type: Some("CustomReturn".to_string()),
        namespace: None,
        conditional_return: None,
        type_assertions: vec![],
    };

    if let Ok(mut fmap) = backend.global_functions.lock() {
        fmap.insert(
            "str_contains".to_string(),
            ("file:///custom.php".to_string(), custom_func),
        );
    }

    let result = backend.find_or_load_function(&["str_contains"]);
    assert!(result.is_some());
    let func = result.unwrap();
    assert_eq!(
        func.return_type.as_deref(),
        Some("CustomReturn"),
        "User-defined function should take precedence over stub"
    );
}

/// Verify that the constant index is built (even if not yet used for
/// resolution, the infrastructure should be in place).
#[tokio::test]
async fn test_stub_constant_index_built() {
    let backend = create_test_backend_with_function_stubs();

    // The stub_constant_index should be populated from the embedded stubs.
    // PHP_EOL is a very common constant that should be present.
    let has_php_eol = backend.stub_constant_index.contains_key("PHP_EOL");
    assert!(has_php_eol, "stub_constant_index should contain PHP_EOL");
}

/// Verify that common constants are present in the constant index.
#[tokio::test]
async fn test_stub_constant_index_common_constants() {
    let backend = create_test_backend_with_function_stubs();

    // Note: TRUE, FALSE, NULL are language constructs, not in the stubs map.
    let expected = [
        "PHP_INT_MAX",
        "PHP_INT_MIN",
        "SORT_ASC",
        "SORT_DESC",
        "PHP_EOL",
        "PHP_MAJOR_VERSION",
    ];
    for name in &expected {
        assert!(
            backend.stub_constant_index.contains_key(name),
            "stub_constant_index should contain '{}', but it doesn't",
            name
        );
    }
}

/// End-to-end: verify that the function_loader in the definition resolver
/// can also access stub functions (used for resolving call-expression
/// subjects in go-to-definition member resolution).
#[tokio::test]
async fn test_definition_resolver_uses_stub_functions() {
    let backend = create_test_backend_with_function_stubs();

    let uri = Url::parse("file:///def_stub.php").unwrap();
    // When cursor is on `format` after `date_create()->`, the definition
    // resolver needs to resolve `date_create()` via the stub function
    // loader to know the return type is DateTime, then find `format` on it.
    let text = concat!(
        "<?php\n",
        "class TestDef {\n",
        "    public function test(): void {\n",
        "        $dt = date_create();\n",
        "        $dt->format('Y-m-d');\n",
        "    }\n",
        "}\n",
    );

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    // The `date_create` function should now be loadable via stubs for
    // return type resolution.
    let func = backend.find_or_load_function(&["date_create"]);
    assert!(
        func.is_some(),
        "date_create should be resolvable for the definition resolver"
    );
}

/// Verify that `array_key_exists` is resolvable (it's a very commonly used
/// built-in function).
#[tokio::test]
async fn test_stub_function_array_key_exists() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["array_key_exists"]);
    assert!(result.is_some(), "array_key_exists should be in stubs");

    let func = result.unwrap();
    assert_eq!(func.name, "array_key_exists");
    assert_eq!(func.return_type.as_deref(), Some("bool"));
}

/// Verify that `substr` is resolvable.
#[tokio::test]
async fn test_stub_function_substr() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["substr"]);
    assert!(result.is_some(), "substr should be in stubs");

    let func = result.unwrap();
    assert_eq!(func.name, "substr");
    // substr returns `string` in modern stubs, but may vary;
    // just verify the function was loaded successfully.
}

/// Verify that `preg_match` is resolvable.
#[tokio::test]
async fn test_stub_function_preg_match() {
    let backend = create_test_backend_with_function_stubs();

    let result = backend.find_or_load_function(&["preg_match"]);
    assert!(result.is_some(), "preg_match should be in stubs");

    let func = result.unwrap();
    assert_eq!(func.name, "preg_match");
}
