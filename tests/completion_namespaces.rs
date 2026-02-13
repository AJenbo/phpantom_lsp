mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

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
