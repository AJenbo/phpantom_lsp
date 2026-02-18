mod common;

use common::{create_psr4_workspace, create_test_backend};
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
            assert_eq!(get_name.insert_text.as_deref(), Some("getName()$0"));
            assert_eq!(get_name.insert_text_format, Some(InsertTextFormat::SNIPPET));
            assert_eq!(get_name.label, "getName(): string");

            let set_price = method_items
                .iter()
                .find(|i| i.filter_text.as_deref() == Some("setPrice"))
                .unwrap();
            assert_eq!(
                set_price.insert_text.as_deref(),
                Some("setPrice(${1:\\$price})$0")
            );
            assert_eq!(
                set_price.insert_text_format,
                Some(InsertTextFormat::SNIPPET)
            );
            assert_eq!(set_price.label, "setPrice(float $price): void");

            // Check property labels
            let prop_labels: Vec<&str> = property_items.iter().map(|i| i.label.as_str()).collect();
            assert!(prop_labels.contains(&"name"));
            assert!(prop_labels.contains(&"price"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Use-As Alias Resolution ────────────────────────────────────────────────

/// `use Swagger\OpenAPI as OA;` followed by `new OA\Endpoint()` should
/// resolve `OA\Endpoint` to `Swagger\OpenAPI\Endpoint` and offer its
/// members for completion.
#[tokio::test]
async fn test_completion_use_as_alias_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///use_alias.php").unwrap();
    let text = concat!(
        "<?php\n",                                                  // 0
        "namespace Swagger\\OpenAPI;\n",                            // 1
        "\n",                                                       // 2
        "class Endpoint {\n",                                       // 3
        "    public function getPath(): string { return ''; }\n",   // 4
        "    public function getMethod(): string { return ''; }\n", // 5
        "}\n",                                                      // 6
    );

    let consumer_uri = Url::parse("file:///consumer.php").unwrap();
    let consumer_text = concat!(
        "<?php\n",                                   // 0
        "use Swagger\\OpenAPI as OA;\n",             // 1
        "\n",                                        // 2
        "class App {\n",                             // 3
        "    public function run(): void {\n",       // 4
        "        $endpoint = new OA\\Endpoint();\n", // 5
        "        $endpoint->\n",                     // 6
        "    }\n",                                   // 7
        "}\n",                                       // 8
    );

    // Open both files so the class is in the AST map
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: consumer_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: consumer_text.to_string(),
            },
        })
        .await;

    // Cursor after `$endpoint->` on line 6
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: consumer_uri },
                position: Position {
                    line: 6,
                    character: 20,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completions for OA\\Endpoint resolved via use-as alias"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("getPath")),
                "Should include getPath from Endpoint, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("getMethod")),
                "Should include getMethod from Endpoint, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Cross-file PSR-4: `use App\Services as Svc;` alias resolving to a class
/// loaded from a PSR-4 mapped file.
#[tokio::test]
async fn test_completion_use_as_alias_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\Services\\": "src/Services/"
            }
        }
    }"#;

    let service_content = concat!(
        "<?php\n",
        "namespace App\\Services;\n",
        "\n",
        "class PaymentGateway {\n",
        "    public function charge(int $amount): bool { return true; }\n",
        "    public function refund(int $amount): bool { return true; }\n",
        "}\n",
    );

    let consumer_content = concat!(
        "<?php\n",                            // 0
        "use App\\Services as Svc;\n",        // 1
        "\n",                                 // 2
        "$gw = new Svc\\PaymentGateway();\n", // 3
        "$gw->\n",                            // 4
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[("src/Services/PaymentGateway.php", service_content)],
    );

    let uri = Url::parse("file:///consumer_psr4.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: consumer_content.to_string(),
            },
        })
        .await;

    // Cursor after `$gw->` on line 4
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 4,
                    character: 5,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completions for Svc\\PaymentGateway resolved via use-as alias + PSR-4"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("charge")),
                "Should include charge() from PaymentGateway, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("refund")),
                "Should include refund() from PaymentGateway, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `use Foo\Bar as FB;` with a class alias (not namespace alias).
/// `$x = new FB();` should resolve to `Foo\Bar` and offer its members.
#[tokio::test]
async fn test_completion_use_as_class_alias() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///class_alias_def.php").unwrap();
    let text = concat!(
        "<?php\n",                                                  // 0
        "namespace Foo;\n",                                         // 1
        "\n",                                                       // 2
        "class Bar {\n",                                            // 3
        "    public function doWork(): void {}\n",                  // 4
        "    public function getStatus(): string { return ''; }\n", // 5
        "}\n",                                                      // 6
    );

    let consumer_uri = Url::parse("file:///class_alias_use.php").unwrap();
    let consumer_text = concat!(
        "<?php\n",               // 0
        "use Foo\\Bar as FB;\n", // 1
        "\n",                    // 2
        "$x = new FB();\n",      // 3
        "$x->\n",                // 4
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: consumer_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: consumer_text.to_string(),
            },
        })
        .await;

    // Cursor after `$x->` on line 4
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: consumer_uri },
                position: Position {
                    line: 4,
                    character: 4,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completions for FB resolved via use-as class alias"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("doWork")),
                "Should include doWork from Bar, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("getStatus")),
                "Should include getStatus from Bar, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Go-to-definition on a method accessed via an aliased namespace should
/// resolve correctly.
#[tokio::test]
async fn test_goto_definition_use_as_alias() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///alias_goto_def.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "namespace Vendor\\Lib;\n",                 // 1
        "\n",                                       // 2
        "class Client {\n",                         // 3
        "    public function request(): void {}\n", // 4
        "}\n",                                      // 5
    );

    let consumer_uri = Url::parse("file:///alias_goto_consumer.php").unwrap();
    let consumer_text = concat!(
        "<?php\n",                  // 0
        "use Vendor\\Lib as VL;\n", // 1
        "\n",                       // 2
        "$c = new VL\\Client();\n", // 3
        "$c->request();\n",         // 4
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: consumer_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: consumer_text.to_string(),
            },
        })
        .await;

    // Cursor on `request` in `$c->request();` (line 4)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: consumer_uri.clone(),
            },
            position: Position {
                line: 4,
                character: 5,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $c->request() when $c is VL\\Client via use-as alias"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 4,
                "request() is declared on line 4 in Client"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}
