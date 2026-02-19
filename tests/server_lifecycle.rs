mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

#[tokio::test]
async fn test_initialize_server_info() {
    let backend = create_test_backend();
    let params = InitializeParams::default();
    let result = backend.initialize(params).await.unwrap();

    let server_info = result.server_info.expect("server_info should be present");
    assert_eq!(server_info.name, "PHPantom");
    assert_eq!(server_info.version, Some("0.2.0".to_string()));
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
    let classes = backend.get_classes_for_uri(uri.as_ref());
    assert!(
        classes.is_some(),
        "AST map should have an entry after did_open"
    );
    assert_eq!(classes.unwrap().len(), 1);
}

#[tokio::test]
async fn test_completion_returns_none_when_nothing_matches() {
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
    assert!(
        result.is_none(),
        "Completion should return None when nothing matches"
    );
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

    let classes = backend.get_classes_for_uri(uri.as_ref()).unwrap();
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
    let classes = backend.get_classes_for_uri(uri.as_ref()).unwrap();
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

    assert!(backend.get_classes_for_uri(uri.as_ref()).is_some());

    // Close the file
    let close_params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
    };
    backend.did_close(close_params).await;

    // AST map entry should be removed after close
    assert!(
        backend.get_classes_for_uri(uri.as_ref()).is_none(),
        "After close, AST map should not have an entry"
    );
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
        .get_classes_for_uri(uri.as_ref())
        .expect("ast_map should have entry for URI");
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "User");
    assert_eq!(classes[0].methods.len(), 2);

    let method_names: Vec<&str> = classes[0].methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"login"));
    assert!(method_names.contains(&"logout"));
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

    let classes = backend.get_classes_for_uri(uri.as_ref()).unwrap();
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
    let classes = backend.get_classes_for_uri(uri.as_ref()).unwrap();
    assert_eq!(classes[0].methods.len(), 2);
    let method_names: Vec<&str> = classes[0].methods.iter().map(|m| m.name.as_str()).collect();
    assert!(method_names.contains(&"first"));
    assert!(method_names.contains(&"second"));

    // Verify the AST was re-parsed and has both methods
    let classes = backend.get_classes_for_uri(uri.as_ref()).unwrap();
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
    assert!(backend.get_classes_for_uri(uri.as_ref()).is_some());

    // Close
    let close_params = DidCloseTextDocumentParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
    };
    backend.did_close(close_params).await;

    // Verify ast_map entry was removed
    assert!(
        backend.get_classes_for_uri(uri.as_ref()).is_none(),
        "ast_map should be cleaned up after did_close"
    );
}
