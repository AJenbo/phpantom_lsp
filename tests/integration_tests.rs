use phpantom_lsp::{AccessKind, Backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

fn create_test_backend() -> Backend {
    Backend::new_test()
}

#[tokio::test]
async fn test_initialize_server_info() {
    let backend = create_test_backend();
    let params = InitializeParams::default();
    let result = backend.initialize(params).await.unwrap();

    let server_info = result.server_info.expect("server_info should be present");
    assert_eq!(server_info.name, "PHPantomLSP");
    assert_eq!(server_info.version, Some("0.1.0".to_string()));
}

#[tokio::test]
async fn test_initialize_capabilities() {
    let backend = create_test_backend();
    let params = InitializeParams::default();
    let result = backend.initialize(params).await.unwrap();

    let caps = result.capabilities;
    assert!(
        caps.completion_provider.is_some(),
        "Completion provider should be enabled"
    );
}

#[tokio::test]
async fn test_did_open_stores_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\nclass Stored { function m() {} }\n".to_string();

    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.clone(),
        },
    };

    backend.did_open(params).await;

    // Verify the file was stored by checking the AST map has an entry
    let classes = backend.get_classes_for_uri(&uri.to_string());
    assert!(
        classes.is_some(),
        "AST map should have an entry after did_open"
    );
    assert_eq!(classes.unwrap().len(), 1);
}

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
async fn test_shutdown() {
    let backend = create_test_backend();
    let result = backend.shutdown().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_did_change_updates_content() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let initial_text = "<?php\nclass A { function first() {} }\n".to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: initial_text,
        },
    };
    backend.did_open(open_params).await;

    let classes = backend.get_classes_for_uri(&uri.to_string()).unwrap();
    assert_eq!(classes[0].methods.len(), 1);

    // Change the content to add a second method
    let change_params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.clone(),
            version: 2,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "<?php\nclass A { function first() {} function second() {} }\n".to_string(),
        }],
    };
    backend.did_change(change_params).await;

    // Verify content was updated by checking the re-parsed AST
    let classes = backend.get_classes_for_uri(&uri.to_string()).unwrap();
    assert_eq!(
        classes[0].methods.len(),
        2,
        "After change, class should have 2 methods"
    );
}

#[tokio::test]
async fn test_did_close_removes_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\nclass Z { function z() {} }\n".to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    assert!(backend.get_classes_for_uri(&uri.to_string()).is_some());

    // Close the file
    let close_params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
    };
    backend.did_close(close_params).await;

    // AST map entry should be removed after close
    assert!(
        backend.get_classes_for_uri(&uri.to_string()).is_none(),
        "After close, AST map should not have an entry"
    );
}

// ─── AST Parsing Tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_extracts_class_and_methods() {
    let backend = create_test_backend();
    let php = "<?php\nclass User {\n    function login() {}\n    function logout() {}\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "User");
    assert_eq!(classes[0].methods.len(), 2);
    assert_eq!(classes[0].methods[0].name, "login");
    assert_eq!(classes[0].methods[1].name, "logout");
}

#[tokio::test]
async fn test_did_open_populates_ast_map() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///user.php").unwrap();
    let text =
        "<?php\nclass User {\n    function login() {}\n    function logout() {}\n}\n".to_string();

    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(params).await;

    let classes = backend
        .get_classes_for_uri(&uri.to_string())
        .expect("ast_map should have entry for URI");
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "User");
    assert_eq!(classes[0].methods.len(), 2);

    let method_names: Vec<&str> = classes[0].methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"login"));
    assert!(method_names.contains(&"logout"));
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
async fn test_did_change_reparses_ast() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///changing.php").unwrap();

    // Open with initial content: one class with one method
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: "<?php\nclass A {\n    function first() {}\n}\n".to_string(),
        },
    };
    backend.did_open(open_params).await;

    let classes = backend.get_classes_for_uri(&uri.to_string()).unwrap();
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "first");

    // Change the file: add a second method
    let change_params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.clone(),
            version: 2,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "<?php\nclass A {\n    function first() {}\n    function second() {}\n}\n"
                .to_string(),
        }],
    };
    backend.did_change(change_params).await;

    // Verify the AST was re-parsed
    let classes = backend.get_classes_for_uri(&uri.to_string()).unwrap();
    assert_eq!(classes[0].methods.len(), 2);
    let method_names: Vec<&str> = classes[0].methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"first"));
    assert!(method_names.contains(&"second"));

    // Verify the AST was re-parsed and has both methods
    let classes = backend.get_classes_for_uri(&uri.to_string()).unwrap();
    assert_eq!(classes[0].methods.len(), 2);
    let method_names: Vec<&str> = classes[0].methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"first"));
    assert!(method_names.contains(&"second"));
}

#[tokio::test]
async fn test_did_close_cleans_up_ast_map() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///cleanup.php").unwrap();
    let text = "<?php\nclass X {\n    function y() {}\n}\n".to_string();

    // Open
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Verify ast_map is populated
    assert!(backend.get_classes_for_uri(&uri.to_string()).is_some());

    // Close
    let close_params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
    };
    backend.did_close(close_params).await;

    // Verify ast_map entry was removed
    assert!(
        backend.get_classes_for_uri(&uri.to_string()).is_none(),
        "ast_map should be cleaned up after did_close"
    );
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
async fn test_parse_php_ignores_standalone_functions() {
    let backend = create_test_backend();
    let php = "<?php\nfunction standalone() {}\nclass Service {\n    function handle() {}\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(
        classes.len(),
        1,
        "Only class declarations should be extracted"
    );
    assert_eq!(classes[0].name, "Service");
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "handle");
}

#[tokio::test]
async fn test_parse_php_no_classes_returns_empty() {
    let backend = create_test_backend();
    let php = "<?php\nfunction foo() {}\n$x = 1;\n";

    let classes = backend.parse_php(php);
    assert!(classes.is_empty(), "No classes should be found");
}

// ─── Method Insert Text with Parameters ─────────────────────────────────────

#[tokio::test]
async fn test_completion_method_insert_text_no_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///insert.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Widget {\n",
        "    function render() {}\n",
        "    function update() {}\n",
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
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();
            for item in &method_items {
                let insert = item.insert_text.as_deref().unwrap();
                let filter = item.filter_text.as_deref().unwrap();
                // insert_text should be just the method name
                assert_eq!(insert, filter);
                // label should be the full signature, e.g. "render()"
                assert!(
                    item.label.starts_with(filter),
                    "Label '{}' should start with method name '{}'",
                    item.label,
                    filter
                );
            }
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_method_insert_text_with_required_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///params.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Editor {\n",
        "    function updateText(string $text, $frogs = false): void {}\n",
        "    function setTitle(string $title): void {}\n",
        "    function reset() {}\n",
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

    // Cursor right after `$this->` on line 6
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();
            assert_eq!(method_items.len(), 4, "Should have 4 method completions (3 original + test)");

            // Find specific methods by filter_text (method name)
            let update_text = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("updateText"))
                .expect("Should have updateText");
            assert_eq!(
                update_text.insert_text.as_deref(),
                Some("updateText"),
                "insert_text should be just the method name"
            );

            let set_title = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("setTitle"))
                .expect("Should have setTitle");
            assert_eq!(
                set_title.insert_text.as_deref(),
                Some("setTitle"),
                "insert_text should be just the method name"
            );

            let reset = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("reset"))
                .expect("Should have reset");
            assert_eq!(
                reset.insert_text.as_deref(),
                Some("reset"),
                "insert_text should be just the method name"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_method_insert_text_multiple_required_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///multi_params.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Calculator {\n",
        "    function add(int $a, int $b): int {}\n",
        "    function addWithLabel(int $a, int $b, string $label = 'sum'): int {}\n",
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
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();

            let add = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("add"))
                .expect("Should have add");
            assert_eq!(
                add.insert_text.as_deref(),
                Some("add"),
                "insert_text should be just the method name"
            );

            let add_with_label = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("addWithLabel"))
                .expect("Should have addWithLabel");
            assert_eq!(
                add_with_label.insert_text.as_deref(),
                Some("addWithLabel"),
                "insert_text should be just the method name"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_method_insert_text_variadic_param() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///variadic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    function log(string $message, ...$context): void {}\n",
        "    function logAll(...$messages): void {}\n",
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
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();

            let log = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("log"))
                .expect("Should have log");
            assert_eq!(
                log.insert_text.as_deref(),
                Some("log"),
                "insert_text should be just the method name"
            );

            let log_all = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("logAll"))
                .expect("Should have logAll");
            assert_eq!(
                log_all.insert_text.as_deref(),
                Some("logAll"),
                "insert_text should be just the method name"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_method_all_optional_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///optional.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    function setup($debug = false, $verbose = false): void {}\n",
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

    // Cursor right after `$this->` on line 4
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let setup = items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("setup"))
                .expect("Should have setup");
            assert_eq!(
                setup.insert_text.as_deref(),
                Some("setup"),
                "insert_text should be just the method name"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Property Completion Tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_extracts_properties() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public int $age;\n",
        "    private $secret;\n",
        "    function login() {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(
        classes[0].properties.len(),
        3,
        "Should extract 3 properties"
    );

    let prop_names: Vec<&str> = classes[0]
        .properties
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(prop_names.contains(&"name"), "Should contain 'name'");
    assert!(prop_names.contains(&"age"), "Should contain 'age'");
    assert!(prop_names.contains(&"secret"), "Should contain 'secret'");

    // Verify type hints
    let name_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "name")
        .unwrap();
    assert_eq!(
        name_prop.type_hint.as_deref(),
        Some("string"),
        "name property should have string type hint"
    );

    let age_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "age")
        .unwrap();
    assert_eq!(
        age_prop.type_hint.as_deref(),
        Some("int"),
        "age property should have int type hint"
    );

    let secret_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "secret")
        .unwrap();
    assert_eq!(
        secret_prop.type_hint, None,
        "secret property should have no type hint"
    );
}

#[tokio::test]
async fn test_completion_includes_properties() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///props.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public int $age;\n",
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

    // Cursor right after `$this->` on line 7
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
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
            // Should have 3 methods (login, logout, test) + 2 properties = 5 items
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();
            let property_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .collect();

            assert_eq!(method_items.len(), 3, "Should have 3 methods");
            assert_eq!(property_items.len(), 2, "Should have 2 properties");

            let prop_labels: Vec<&str> = property_items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                prop_labels.contains(&"name"),
                "Should contain property 'name'"
            );
            assert!(
                prop_labels.contains(&"age"),
                "Should contain property 'age'"
            );

            // Check property insert_text is the property name (no $)
            let name_item = property_items.iter().find(|i| i.label == "name").unwrap();
            assert_eq!(
                name_item.insert_text.as_deref(),
                Some("name"),
                "Property insert_text should be 'name' without $"
            );

            // Check property detail includes type hint
            let name_detail = name_item.detail.as_deref().unwrap();
            assert!(
                name_detail.contains("string"),
                "Property detail '{}' should include type hint 'string'",
                name_detail
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_property_without_type_hint() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///untyped.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Bag {\n",
        "    public $stuff;\n",
        "    function get() {\n",
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

    // Cursor right after `$this->` on line 4
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let property_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .collect();

            assert_eq!(property_items.len(), 1);
            assert_eq!(property_items[0].label, "stuff");

            let detail = property_items[0].detail.as_deref().unwrap();
            assert_eq!(
                detail, "Class: Bag",
                "Untyped property detail should just show class name"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_parse_php_extracts_static_properties() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Counter {\n",
        "    public static int $count = 0;\n",
        "    public string $label;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].properties.len(), 2);

    let count_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "count")
        .expect("Should have count property");
    assert!(count_prop.is_static, "count should be static");

    let label_prop = classes[0]
        .properties
        .iter()
        .find(|p| p.name == "label")
        .expect("Should have label property");
    assert!(!label_prop.is_static, "label should not be static");
}

#[tokio::test]
async fn test_parse_php_extracts_method_return_type() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Greeter {\n",
        "    function greet(string $name): string {}\n",
        "    function doStuff() {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].methods.len(), 2);

    let greet = &classes[0].methods[0];
    assert_eq!(greet.name, "greet");
    assert_eq!(
        greet.return_type.as_deref(),
        Some("string"),
        "greet should have return type 'string'"
    );
    assert_eq!(greet.parameters.len(), 1);
    assert_eq!(greet.parameters[0].name, "$name");
    assert!(greet.parameters[0].is_required);
    assert_eq!(greet.parameters[0].type_hint.as_deref(), Some("string"));

    let do_stuff = &classes[0].methods[1];
    assert_eq!(do_stuff.name, "doStuff");
    assert_eq!(
        do_stuff.return_type, None,
        "doStuff should have no return type"
    );
}

#[tokio::test]
async fn test_parse_php_method_parameter_info() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Service {\n",
        "    function process(string $input, int $count, ?string $label = null, ...$extras): bool {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);

    let method = &classes[0].methods[0];
    assert_eq!(method.name, "process");
    assert_eq!(method.parameters.len(), 4);

    let input = &method.parameters[0];
    assert_eq!(input.name, "$input");
    assert!(input.is_required);
    assert_eq!(input.type_hint.as_deref(), Some("string"));
    assert!(!input.is_variadic);

    let count = &method.parameters[1];
    assert_eq!(count.name, "$count");
    assert!(count.is_required);
    assert_eq!(count.type_hint.as_deref(), Some("int"));

    let label = &method.parameters[2];
    assert_eq!(label.name, "$label");
    assert!(
        !label.is_required,
        "$label has a default value, should not be required"
    );
    assert_eq!(label.type_hint.as_deref(), Some("?string"));

    let extras = &method.parameters[3];
    assert_eq!(extras.name, "$extras");
    assert!(
        !extras.is_required,
        "variadic params should not be required"
    );
    assert!(extras.is_variadic);
}

#[tokio::test]
async fn test_completion_method_detail_shows_signature() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///detail.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Editor {\n",
        "    function updateText(string $text, $frogs = false): void {}\n",
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

    // Cursor right after `$this->` on line 4
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let update = items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("updateText"))
                .expect("Should have updateText");

            // Label should show the full signature
            assert_eq!(
                update.label, "updateText(string $text, $frogs = ...): void",
                "Label should be the full method signature"
            );

            // Detail should show the class name
            let detail = update.detail.as_deref().unwrap();
            assert!(
                detail.contains("Editor"),
                "Detail '{}' should reference class Editor",
                detail
            );

            // insert_text should be just the method name
            assert_eq!(
                update.insert_text.as_deref(),
                Some("updateText"),
                "insert_text should be just the method name"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_class_with_only_properties() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///onlyprops.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Data {\n",
        "    public string $name;\n",
        "    public int $value;\n",
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
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let property_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .collect();
            // Class has 2 properties + test method, but we check properties
            assert_eq!(property_items.len(), 2, "Should return 2 property completions");
            let labels: Vec<&str> = property_items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"name"));
            assert!(labels.contains(&"value"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_parse_php_property_with_default_value() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Settings {\n",
        "    public bool $debug = false;\n",
        "    public string $title = 'default';\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].properties.len(), 2);

    let prop_names: Vec<&str> = classes[0]
        .properties
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    assert!(prop_names.contains(&"debug"));
    assert!(prop_names.contains(&"title"));
}

// ─── Namespace Support Tests ────────────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_class_inside_implicit_namespace() {
    let backend = create_test_backend();
    let php = "<?php\nnamespace Demo;\n\nclass User {\n    function login() {}\n    function logout() {}\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(
        classes.len(),
        1,
        "Should find class inside implicit namespace"
    );
    assert_eq!(classes[0].name, "User");
    assert_eq!(classes[0].methods.len(), 2);
    assert_eq!(classes[0].methods[0].name, "login");
    assert_eq!(classes[0].methods[1].name, "logout");
}

#[tokio::test]
async fn test_parse_php_class_inside_brace_delimited_namespace() {
    let backend = create_test_backend();
    let php =
        "<?php\nnamespace Demo {\n    class Service {\n        function handle() {}\n    }\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(
        classes.len(),
        1,
        "Should find class inside brace-delimited namespace"
    );
    assert_eq!(classes[0].name, "Service");
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "handle");
}

#[tokio::test]
async fn test_completion_inside_namespaced_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///namespaced.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "\n",
        "class User {\n",
        "    public function login() {}\n",
        "    public function logout() {}\n",
        "    public function test() {\n",
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

    // Cursor right after `$this->` on line 7
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for namespaced class"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
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

            for item in &method_items {
                assert_eq!(item.kind, Some(CompletionItemKind::METHOD));
            }
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_parse_php_multiple_classes_in_brace_delimited_namespaces() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "namespace Foo {\n",
        "    class A {\n",
        "        function doA() {}\n",
        "    }\n",
        "}\n",
        "namespace Bar {\n",
        "    class B {\n",
        "        function doB() {}\n",
        "    }\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 2, "Should find classes in both namespaces");
    assert_eq!(classes[0].name, "A");
    assert_eq!(classes[0].methods.len(), 1);
    assert_eq!(classes[0].methods[0].name, "doA");
    assert_eq!(classes[1].name, "B");
    assert_eq!(classes[1].methods.len(), 1);
    assert_eq!(classes[1].methods[0].name, "doB");
}

// ─── Namespaced class with properties ───────────────────────────────────────

#[tokio::test]
async fn test_completion_namespaced_class_with_properties_and_methods() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ns_full.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Entity;\n",
        "\n",
        "class Product {\n",
        "    public string $name;\n",
        "    public float $price;\n",
        "    public function getName(): string {}\n",
        "    public function setPrice(float $price): void {}\n",
        "    public function test() {\n",
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

    // Cursor right after `$this->` on line 9
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 9,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();
            let property_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .collect();

            assert_eq!(method_items.len(), 3, "Should have 3 methods");
            assert_eq!(property_items.len(), 2, "Should have 2 properties");

            // Check method insert texts
            let get_name = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("getName"))
                .unwrap();
            assert_eq!(get_name.insert_text.as_deref(), Some("getName"));
            assert_eq!(get_name.label, "getName(): string");

            let set_price = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("setPrice"))
                .unwrap();
            assert_eq!(set_price.insert_text.as_deref(), Some("setPrice"));
            assert_eq!(set_price.label, "setPrice(float $price): void");

            // Check property labels
            let prop_labels: Vec<&str> = property_items.iter().map(|i| i.label.as_str()).collect();
            assert!(prop_labels.contains(&"name"));
            assert!(prop_labels.contains(&"price"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Edge case: static method info ──────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_static_method() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public static function create(string $type): self {}\n",
        "    public function build(): void {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].methods.len(), 2);

    let create = &classes[0].methods[0];
    assert_eq!(create.name, "create");
    assert!(create.is_static, "create should be static");
    assert_eq!(create.parameters.len(), 1);
    assert_eq!(create.parameters[0].name, "$type");

    let build = &classes[0].methods[1];
    assert_eq!(build.name, "build");
    assert!(!build.is_static, "build should not be static");
}

#[tokio::test]
async fn test_parse_php_extracts_constants() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Config {\n",
        "    const VERSION = '1.0';\n",
        "    const int MAX_RETRIES = 3;\n",
        "    public string $name;\n",
        "    public function getName(): string {}\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].constants.len(), 2);

    let version = &classes[0].constants[0];
    assert_eq!(version.name, "VERSION");
    assert!(version.type_hint.is_none(), "VERSION has no type hint");

    let max_retries = &classes[0].constants[1];
    assert_eq!(max_retries.name, "MAX_RETRIES");
    assert_eq!(
        max_retries.type_hint.as_deref(),
        Some("int"),
        "MAX_RETRIES should have int type hint"
    );
}

#[tokio::test]
async fn test_parse_php_extracts_multiple_constants_in_one_declaration() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "class Status {\n",
        "    const ACTIVE = 1, INACTIVE = 0;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].constants.len(), 2);
    assert_eq!(classes[0].constants[0].name, "ACTIVE");
    assert_eq!(classes[0].constants[1].name, "INACTIVE");
}

#[tokio::test]
async fn test_detect_access_kind_arrow() {
    assert_eq!(
        Backend::detect_access_kind(
            "$this->",
            Position {
                line: 0,
                character: 7
            }
        ),
        AccessKind::Arrow
    );
}

#[tokio::test]
async fn test_detect_access_kind_arrow_with_partial_identifier() {
    assert_eq!(
        Backend::detect_access_kind(
            "$this->get",
            Position {
                line: 0,
                character: 10
            }
        ),
        AccessKind::Arrow
    );
}

#[tokio::test]
async fn test_detect_access_kind_double_colon() {
    assert_eq!(
        Backend::detect_access_kind(
            "self::",
            Position {
                line: 0,
                character: 6
            }
        ),
        AccessKind::DoubleColon
    );
}

#[tokio::test]
async fn test_detect_access_kind_double_colon_with_partial_identifier() {
    assert_eq!(
        Backend::detect_access_kind(
            "Foo::cr",
            Position {
                line: 0,
                character: 7
            }
        ),
        AccessKind::DoubleColon
    );
}

#[tokio::test]
async fn test_detect_access_kind_other() {
    assert_eq!(
        Backend::detect_access_kind(
            "    $x = 1;",
            Position {
                line: 0,
                character: 4
            }
        ),
        AccessKind::Other
    );
}

#[tokio::test]
async fn test_detect_access_kind_multiline() {
    let content = "<?php\n$obj->meth";
    assert_eq!(
        Backend::detect_access_kind(
            content,
            Position {
                line: 1,
                character: 10
            }
        ),
        AccessKind::Arrow
    );
}

#[tokio::test]
async fn test_completion_arrow_shows_only_non_static() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///arrow.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public static function create(): self {}\n",
        "    public function run(): void {}\n",
        "    public static string $instance = '';\n",
        "    public int $count = 0;\n",
        "    const MAX = 10;\n",
        "    function helper() {\n",
        "        $this->\n",
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

    // Cursor right after `$this->` on line 8
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
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let property_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            // Should include non-static method `run` and `helper`
            assert!(
                method_names.contains(&"run"),
                "Arrow should include non-static method 'run'"
            );
            assert!(
                method_names.contains(&"helper"),
                "Arrow should include non-static method 'helper'"
            );
            // Should NOT include static method `create`
            assert!(
                !method_names.contains(&"create"),
                "Arrow should exclude static method 'create'"
            );

            // Should include non-static property `count`
            assert!(
                property_names.contains(&"count"),
                "Arrow should include non-static property 'count'"
            );
            // Should NOT include static property `instance`
            assert!(
                !property_names.contains(&"instance"),
                "Arrow should exclude static property 'instance'"
            );

            // Should NOT include constants
            assert!(
                constant_names.is_empty(),
                "Arrow should not include constants, got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_double_colon_shows_only_static_and_constants() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///dcolon.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public static function create(): self {}\n",
        "    public function run(): void {}\n",
        "    public static string $instance = '';\n",
        "    public int $count = 0;\n",
        "    const MAX = 10;\n",
        "    function helper() {\n",
        "        self::\n",
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

    // Cursor right after `self::` on line 8
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 14,
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
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let property_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            // Should include static method `create`
            assert!(
                method_names.contains(&"create"),
                "DoubleColon should include static method 'create'"
            );
            // Should NOT include non-static methods `run` and `helper`
            assert!(
                !method_names.contains(&"run"),
                "DoubleColon should exclude non-static method 'run'"
            );
            assert!(
                !method_names.contains(&"helper"),
                "DoubleColon should exclude non-static method 'helper'"
            );

            // Should include static property `instance`
            assert!(
                property_names.contains(&"instance"),
                "DoubleColon should include static property 'instance'"
            );
            // Should NOT include non-static property `count`
            assert!(
                !property_names.contains(&"count"),
                "DoubleColon should exclude non-static property 'count'"
            );

            // Should include constant `MAX`
            assert!(
                constant_names.contains(&"MAX"),
                "DoubleColon should include constant 'MAX'"
            );
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

#[tokio::test]
async fn test_completion_new_self_variable() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///newself.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public function build(): void {}\n",
        "    public static function create(): self {\n",
        "        $new = new self();\n",
        "        $new->\n",
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

    // Cursor right after `$new->` on line 5
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results for $new = new self");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(method_names.contains(&"build"), "Should include non-static 'build'");
            assert!(!method_names.contains(&"create"), "Should exclude static 'create' via ->");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_new_static_variable() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///newstatic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public function build(): void {}\n",
        "    public static function create(): static {\n",
        "        $inst = new static();\n",
        "        $inst->\n",
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
    assert!(result.is_some(), "Completion should return results for $inst = new static");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(method_names.contains(&"build"), "Should include non-static 'build'");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_new_classname_variable() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///newclass.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Widget {\n",
        "    public function render(): void {}\n",
        "    public function test() {\n",
        "        $w = new Widget();\n",
        "        $w->\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results for $w = new Widget");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(method_names.contains(&"render"), "Should include 'render'");
            assert!(method_names.contains(&"test"), "Should include 'test'");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_unknown_variable_shows_fallback() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///unknown.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function run(): void {}\n",
        "    public function test() {\n",
        "        $unknown->\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 18,
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
            assert_eq!(items.len(), 1, "Unknown variable should fall back");
            assert_eq!(items[0].label, "PHPantomLSP");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_property_chain_self_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Node {\n",
        "    public self $parent;\n",
        "    public function value(): int {}\n",
        "    public function test() {\n",
        "        $this->parent->\n",
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

    // Cursor right after `$this->parent->` on line 5
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should resolve $this->parent-> via self type hint");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(method_names.contains(&"value"), "Should include 'value'");
            assert!(method_names.contains(&"test"), "Should include 'test'");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_classname_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///classdcolon.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Registry {\n",
        "    public static function instance(): self {}\n",
        "    public function get(): void {}\n",
        "    const VERSION = 1;\n",
        "    function test() {\n",
        "        Registry::\n",
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

    // Cursor right after `Registry::` on line 6
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should resolve Registry:: to Registry class");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            // Only static method should appear for ::
            assert!(method_names.contains(&"instance"), "Should include static 'instance'");
            assert!(!method_names.contains(&"get"), "Should exclude non-static 'get'");
            assert!(constant_names.contains(&"VERSION"), "Should include constant");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_param_type_hint_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///paramhint.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Processor {\n",
        "    public function run(): void {}\n",
        "    public function handle(self $other) {\n",
        "        $other->\n",
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

    // Cursor right after `$other->` on line 4
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should resolve $other via parameter type hint");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(method_names.contains(&"run"), "Should include 'run'");
            assert!(method_names.contains(&"handle"), "Should include 'handle'");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_constant_detail_with_type_hint() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///consttype.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Cfg {\n",
        "    const string LABEL = 'hello';\n",
        "    const COUNT = 5;\n",
        "    function f() {\n",
        "        self::\n",
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

    // Cursor after `self::` on line 5
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 14,
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
            let constants: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .collect();
            assert_eq!(constants.len(), 2, "Should have 2 constants");

            let label_const = constants.iter().find(|c| c.label == "LABEL").unwrap();
            assert!(
                label_const.detail.as_ref().unwrap().contains("string"),
                "LABEL detail should mention type hint 'string', got: {}",
                label_const.detail.as_ref().unwrap()
            );

            let count_const = constants.iter().find(|c| c.label == "COUNT").unwrap();
            assert!(
                !count_const.detail.as_ref().unwrap().contains("—"),
                "COUNT detail should not have type hint separator"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_static_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///staticdc.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public static function create(): static {}\n",
        "    public function run(): void {}\n",
        "    const MAX = 10;\n",
        "    function test() {\n",
        "        static::\n",
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

    // Cursor right after `static::` on line 6
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should resolve static::");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();
            // Only static method for ::
            assert!(method_names.contains(&"create"), "Should include static 'create'");
            assert!(!method_names.contains(&"run"), "Should exclude non-static 'run'");
            assert!(constant_names.contains(&"MAX"), "Should include constant 'MAX'");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_arrow_with_partial_typed_identifier() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///partial.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Obj {\n",
        "    public static function staticMethod(): void {}\n",
        "    public function instanceMethod(): void {}\n",
        "    function test() {\n",
        "        $this->inst\n",
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

    // Cursor after `$this->inst` on line 5 — partial identifier typed after ->
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 19,
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
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Should include non-static method
            assert!(
                method_names.contains(&"instanceMethod"),
                "Should include instanceMethod"
            );
            assert!(method_names.contains(&"test"), "Should include test");
            // Should NOT include static method even with partial typing
            assert!(
                !method_names.contains(&"staticMethod"),
                "Should exclude staticMethod when using ->"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
