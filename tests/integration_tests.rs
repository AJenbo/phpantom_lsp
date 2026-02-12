use phpantom_lsp::Backend;
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
    assert_eq!(server_info.version.unwrap(), "0.1.0");
}

#[tokio::test]
async fn test_initialize_capabilities() {
    let backend = create_test_backend();
    let params = InitializeParams::default();

    let result = backend.initialize(params).await.unwrap();

    // Check that hover is supported
    assert!(result.capabilities.hover_provider.is_some());

    // Check that completion is supported
    assert!(result.capabilities.completion_provider.is_some());

    // Check text document sync
    assert!(result.capabilities.text_document_sync.is_some());
}

#[tokio::test]
async fn test_did_open_stores_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\nfunction PHPantom() {}\n".to_string();

    let params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.clone(),
        },
    };

    backend.did_open(params).await;

    // Test that hovering works after opening
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 10, // Position on "PHPantom"
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let hover_result = backend.hover(hover_params).await.unwrap();
    assert!(hover_result.is_some());
}

#[tokio::test]
async fn test_hover_on_phpantom() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\n// PHPantom is great\n".to_string();

    // Open the document
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Hover over "PHPantom"
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 5, // Middle of "PHPantom"
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let hover_result = backend.hover(hover_params).await.unwrap();
    assert!(hover_result.is_some());

    let hover = hover_result.unwrap();
    match hover.contents {
        HoverContents::Scalar(MarkedString::String(s)) => {
            assert_eq!(s, "Welcome to PHPantomLSP!");
        }
        _ => panic!("Expected scalar string hover contents"),
    }
}

#[tokio::test]
async fn test_hover_on_other_word_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\n$variable = 123;\n".to_string();

    // Open the document
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Hover over "variable"
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 3, // On "variable"
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let hover_result = backend.hover(hover_params).await.unwrap();
    assert!(hover_result.is_none());
}

#[tokio::test]
async fn test_completion_returns_phpantomlsp() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\n$x = \n".to_string();

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
                character: 5,
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
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].label, "PHPantomLSP");
            assert_eq!(items[0].kind, Some(CompletionItemKind::TEXT));
            assert_eq!(items[0].detail.as_deref(), Some("PHPantomLSP completion"));
            assert_eq!(items[0].insert_text.as_deref(), Some("PHPantomLSP"));
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

    // Open with initial content
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: "<?php\n$old = 1;\n".to_string(),
        },
    };
    backend.did_open(open_params).await;

    // Change content to include PHPantom
    let change_params = DidChangeTextDocumentParams {
        text_document: VersionedTextDocumentIdentifier {
            uri: uri.clone(),
            version: 2,
        },
        content_changes: vec![TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "<?php\n// PHPantom\n".to_string(),
        }],
    };
    backend.did_change(change_params).await;

    // Hover should now work on PHPantom
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 5,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    let hover_result = backend.hover(hover_params).await.unwrap();
    assert!(hover_result.is_some());
}

#[tokio::test]
async fn test_did_close_removes_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\n// PHPantom\n".to_string();

    // Open the document
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Verify hover works
    let hover_params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 1,
                character: 5,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let hover_result = backend.hover(hover_params.clone()).await.unwrap();
    assert!(hover_result.is_some());

    // Close the document
    let close_params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
    };
    backend.did_close(close_params).await;

    // Verify hover no longer works (file not in memory)
    let hover_result_after = backend.hover(hover_params).await.unwrap();
    assert!(hover_result_after.is_none());
}

// ─── Class Method Completion Tests ──────────────────────────────────────────

#[tokio::test]
async fn test_parse_php_extracts_class_and_methods() {
    let backend = create_test_backend();
    let php = "<?php\nclass User {\n    function login() {}\n    function logout() {}\n}\n";

    let classes = backend.parse_php(php);
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "User");
    assert_eq!(classes[0].methods, vec!["login", "logout"]);
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
    assert!(classes[0].methods.contains(&"login".to_string()));
    assert!(classes[0].methods.contains(&"logout".to_string()));
}

#[tokio::test]
async fn test_completion_inside_class_returns_methods() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///user.php").unwrap();
    let text =
        "<?php\nclass User {\n    function login() {}\n    function logout() {}\n}\n".to_string();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    // Position cursor inside the class body (line 2, inside the class braces)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 2,
                character: 4,
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
            assert_eq!(items.len(), 2, "Should return 2 method completions");

            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"login"), "Should contain 'login'");
            assert!(labels.contains(&"logout"), "Should contain 'logout'");

            // Check all items have the correct kind
            for item in &items {
                assert_eq!(
                    item.kind,
                    Some(CompletionItemKind::METHOD),
                    "Kind should be Method"
                );
                assert_eq!(
                    item.detail.as_deref(),
                    Some("Class: User"),
                    "Detail should show the class name"
                );
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

    // Position cursor outside the class body (line 6, which is `$x = 1;`)
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

    // Should fall back to the default PHPantomLSP completion
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(items.len(), 1, "Fallback should return 1 item");
            assert_eq!(items[0].label, "PHPantomLSP");
            assert_eq!(items[0].kind, Some(CompletionItemKind::TEXT));
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

    // Cursor inside the second class (Bar) body — line 6 is `    function handleRequest() {}`
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 4,
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
            assert_eq!(items.len(), 1, "Bar has only one method");
            assert_eq!(items[0].label, "handleRequest");
            assert_eq!(
                items[0].detail.as_deref(),
                Some("Class: Bar"),
                "Detail should reference Bar, not Foo"
            );
            assert_eq!(items[0].kind, Some(CompletionItemKind::METHOD));
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
    assert_eq!(classes[0].methods[0], "first");

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
    assert!(classes[0].methods.contains(&"first".to_string()));
    assert!(classes[0].methods.contains(&"second".to_string()));

    // Request completion inside the class body
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 3,
                character: 4,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(items.len(), 2);
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"first"));
            assert!(labels.contains(&"second"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
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

    // Empty class has no methods, so should fall back to PHPantomLSP
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
    assert_eq!(classes[0].methods, vec!["handle"]);
}

#[tokio::test]
async fn test_parse_php_no_classes_returns_empty() {
    let backend = create_test_backend();
    let php = "<?php\nfunction foo() {}\n$x = 1;\n";

    let classes = backend.parse_php(php);
    assert!(classes.is_empty(), "No classes should be found");
}

#[tokio::test]
async fn test_completion_method_insert_text() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///insert.php").unwrap();
    let text = "<?php\nclass Widget {\n    function render() {}\n    function update() {}\n}\n"
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 2,
                character: 4,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            for item in &items {
                assert_eq!(
                    item.insert_text.as_deref(),
                    Some(item.label.as_str()),
                    "insert_text should match label"
                );
            }
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
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
    assert_eq!(classes[0].methods, vec!["login", "logout"]);
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
    assert_eq!(classes[0].methods, vec!["handle"]);
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

    // Cursor inside the class body (line 4, inside the method area)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 4,
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
            assert_eq!(items.len(), 2, "Should return 2 method completions");

            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"login"), "Should contain 'login'");
            assert!(labels.contains(&"logout"), "Should contain 'logout'");

            for item in &items {
                assert_eq!(item.kind, Some(CompletionItemKind::METHOD));
                assert_eq!(item.detail.as_deref(), Some("Class: User"));
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
    assert_eq!(classes[0].methods, vec!["doA"]);
    assert_eq!(classes[1].name, "B");
    assert_eq!(classes[1].methods, vec!["doB"]);
}
