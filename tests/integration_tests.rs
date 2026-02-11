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
