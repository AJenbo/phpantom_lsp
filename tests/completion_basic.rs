mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Interface Completion Tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_completion_interface_type_hint_resolves_methods() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///iface.php").unwrap();
    let text = concat!(
        "<?php\n",
        "interface Loggable {\n",
        "    public function log(string $message): void;\n",
        "    public function getLogLevel(): int;\n",
        "}\n",
        "class Service {\n",
        "    public function run(Loggable $logger): void {\n",
        "        $logger->\n",
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

    // Cursor right after `$logger->` on line 7
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for interface-typed parameter"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                names.iter().any(|n| n.starts_with("log(")),
                "Should contain interface method 'log', got: {:?}",
                names
            );
            assert!(
                names.iter().any(|n| n.starts_with("getLogLevel(")),
                "Should contain interface method 'getLogLevel', got: {:?}",
                names
            );
        }
        _ => panic!("Expected Array response"),
    }
}

#[tokio::test]
async fn test_completion_interface_constant_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///iface_const.php").unwrap();
    let text = concat!(
        "<?php\n",
        "interface HasStatus {\n",
        "    const STATUS_ACTIVE = 1;\n",
        "    const STATUS_INACTIVE = 0;\n",
        "    public function getStatus(): int;\n",
        "}\n",
        "class Foo {\n",
        "    public function bar(): void {\n",
        "        HasStatus::\n",
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

    // Cursor right after `HasStatus::` on line 8
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for interface constant access"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                names.contains(&"STATUS_ACTIVE"),
                "Should contain constant 'STATUS_ACTIVE', got: {:?}",
                names
            );
            assert!(
                names.contains(&"STATUS_INACTIVE"),
                "Should contain constant 'STATUS_INACTIVE', got: {:?}",
                names
            );
        }
        _ => panic!("Expected Array response"),
    }
}

// ─── Basic Completion Tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_returns_phpantomlsp() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\n$x = 1;\n".to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 0,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert!(!items.is_empty(), "Should have at least one item");
            assert_eq!(items[0].label, "PHPantomLSP");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inside_class_returns_methods() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///user.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    function login() {}\n",
        "    function logout() {}\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    )
    .to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Cursor right after `$this->` on line 5
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            // Should have 3 non-static methods (login, logout, test)
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();
            assert_eq!(method_items.len(), 3, "Should return 3 method completions");

            let filter_texts: Vec<&str> = method_items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(filter_texts.contains(&"login"), "Should contain 'login'");
            assert!(filter_texts.contains(&"logout"), "Should contain 'logout'");

            // Check labels show full signature
            for item in &method_items {
                let label = &item.label;
                assert!(
                    label.contains("(") && label.contains(")"),
                    "Label '{}' should contain full signature with parens",
                    label
                );
            }

            // Check insert_text is just the method name
            for item in &method_items {
                let insert = item.insert_text.as_deref().unwrap();
                let filter = item.filter_text.as_deref().unwrap();
                assert_eq!(insert, filter, "insert_text should be just the method name");
            }
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_outside_class_returns_fallback() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///user.php").unwrap();
    let text = "<?php\n\nclass User {\n    function login() {}\n}\n\n$x = 1;\n".to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Cursor outside the class (line 6: $x = 1;)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 0,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(items.len(), 1, "Should fall back to default item");
            assert_eq!(items[0].label, "PHPantomLSP");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_with_multiple_classes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///multi.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    function doFoo() {}\n",
        "    function doBar() {}\n",
        "}\n",
        "class Bar {\n",
        "    function handleRequest() {}\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    )
    .to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Verify two classes were parsed
    let classes = backend
        .get_classes_for_uri(&uri.to_string())
        .expect("ast_map should have entry");
    assert_eq!(classes.len(), 2);

    // Cursor right after `$this->` on line 8 inside Bar
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();
            // Bar has handleRequest and test — both non-static
            assert_eq!(method_items.len(), 2, "Bar has two methods");
            let names: Vec<&str> = method_items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(names.contains(&"handleRequest"));
            assert!(names.contains(&"test"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_empty_class_falls_back() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///empty.php").unwrap();
    let text = "<?php\nclass Empty {\n}\n".to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Cursor inside the empty class body
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    // Empty class has no methods or properties, so should fall back to PHPantomLSP
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].label, "PHPantomLSP");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_no_access_operator_shows_fallback() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///all.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public static function create(): self {}\n",
        "    public function run(): void {}\n",
        "    public static string $instance = '';\n",
        "    public int $count = 0;\n",
        "    const MAX = 10;\n",
        "    \n",
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

    // Cursor on blank line 7 inside the class body (no `->` or `::`)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 4,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return fallback");

    // Without `->` or `::`, no class members should be suggested — only fallback
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(items.len(), 1, "Should only have fallback item");
            assert_eq!(items[0].label, "PHPantomLSP");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
