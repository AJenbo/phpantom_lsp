mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

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
            assert_eq!(
                property_items.len(),
                2,
                "Should return 2 property completions"
            );
            let labels: Vec<&str> = property_items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"name"));
            assert!(labels.contains(&"value"));
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
