mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

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
            assert_eq!(
                method_items.len(),
                4,
                "Should have 4 method completions (3 original + test)"
            );

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
