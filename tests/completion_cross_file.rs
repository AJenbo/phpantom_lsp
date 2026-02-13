mod common;

use common::create_psr4_workspace;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Cross-file / PSR-4 resolution tests ────────────────────────────────────

#[tokio::test]
async fn test_cross_file_double_colon_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Service.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Service {\n",
                "    public static function create(): self { return new self(); }\n",
                "    public function run(): void {}\n",
                "    const VERSION = '1.0';\n",
                "}\n",
            ),
        )],
    );

    // The "current" file references Acme\Service via ::
    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class App {\n",
        "    function boot() {\n",
        "        Acme\\Service::\n",
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

    // Cursor right after `Acme\Service::` on line 3
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 3,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve Acme\\Service::"
    );

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
            // :: shows static method + constants
            assert!(
                method_names.contains(&"create"),
                "Should include static 'create', got {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"run"),
                "Should exclude non-static 'run'"
            );
            assert!(
                constant_names.contains(&"VERSION"),
                "Should include constant 'VERSION', got {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_new_variable_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Widget.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Widget {\n",
                "    public function render(): string { return ''; }\n",
                "    public string $title;\n",
                "}\n",
            ),
        )],
    );

    // The "current" file creates a new Acme\Widget and calls ->
    let uri = Url::parse("file:///page.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Page {\n",
        "    function show() {\n",
        "        $w = new Acme\\Widget();\n",
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

    // Cursor right after `$w->` on line 4
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve $w-> to Acme\\Widget members"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                method_names.contains(&"render"),
                "Should include 'render', got {:?}",
                method_names
            );
            assert!(
                prop_names.contains(&"title"),
                "Should include property 'title', got {:?}",
                prop_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_param_type_hint_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Logger.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Logger {\n",
                "    public function info(string $msg): void {}\n",
                "    public function error(string $msg): void {}\n",
                "}\n",
            ),
        )],
    );

    // The "current" file has a method with an Acme\Logger parameter
    let uri = Url::parse("file:///handler.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Handler {\n",
        "    function handle(Acme\\Logger $log) {\n",
        "        $log->\n",
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

    // Cursor right after `$log->` on line 3
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 3,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve $log-> via param type hint"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"info"),
                "Should include 'info', got {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"error"),
                "Should include 'error', got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_caches_parsed_class() {
    // Verify that after the first completion triggers PSR-4 loading,
    // subsequent completions for the same class don't need to re-read
    // the file (it's cached in ast_map).
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Cache.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Cache {\n",
                "    public static function get(string $key): mixed { return null; }\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///controller.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Controller {\n",
        "    function index() {\n",
        "        Acme\\Cache::\n",
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
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 3,
                character: 20,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    // First call — triggers PSR-4 file loading
    let result1 = backend.completion(completion_params.clone()).await.unwrap();
    assert!(result1.is_some(), "First call should resolve");

    // Second call — should use cached ast_map entry
    let result2 = backend.completion(completion_params).await.unwrap();
    assert!(result2.is_some(), "Second call should also resolve");

    match result2.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"get"),
                "Cached result should still include 'get', got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_no_psr4_mapping_falls_back() {
    // When there's no PSR-4 mapping for a class, we should get the fallback
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[], // no files on disk
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class App {\n",
        "    function boot() {\n",
        "        Unknown\\Thing::\n",
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
                line: 3,
                character: 24,
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
async fn test_cross_file_nested_namespace_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Vendor\\Package\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Sub/Helper.php",
            concat!(
                "<?php\n",
                "namespace Vendor\\Package\\Sub;\n",
                "class Helper {\n",
                "    public static function format(): string { return ''; }\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Test {\n",
        "    function run() {\n",
        "        Vendor\\Package\\Sub\\Helper::\n",
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
                line: 3,
                character: 36,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should resolve deeply nested namespace");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"format"),
                "Should include 'format', got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Use-statement and namespace-relative resolution tests ──────────────────

#[tokio::test]
async fn test_cross_file_use_statement_new_variable() {
    // Simulates the exact scenario from the bug report:
    //   use Klarna\Rest\Resource;
    //   $e = new Resource();
    //   $e->   ← should resolve Resource via use statement → PSR-4
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Klarna\\": "src/Klarna/"
                }
            }
        }"#,
        &[(
            "src/Klarna/Rest/Resource.php",
            concat!(
                "<?php\n",
                "namespace Klarna\\Rest;\n",
                "class Resource {\n",
                "    public function request(string $method): self { return $this; }\n",
                "    public string $url;\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///order.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Klarna\\Rest\\Checkout;\n",
        "\n",
        "use Klarna\\Rest\\Resource;\n",
        "\n",
        "class Order {\n",
        "    public function create() {\n",
        "        $e = new Resource();\n",
        "        $e->\n",
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

    // Cursor right after `$e->` on line 8
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve $e-> via use statement + PSR-4"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                method_names.contains(&"request"),
                "Should include 'request', got {:?}",
                method_names
            );
            assert!(
                prop_names.contains(&"url"),
                "Should include property 'url', got {:?}",
                prop_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_use_statement_double_colon() {
    // `use Acme\Factory;` then `Factory::` should resolve via use statement
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Factory.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Factory {\n",
                "    public static function build(): self { return new self(); }\n",
                "    const VERSION = 1;\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "use Acme\\Factory;\n",
        "\n",
        "class App {\n",
        "    function boot() {\n",
        "        Factory::\n",
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

    // Cursor right after `Factory::` on line 7
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
        "Completion should resolve Factory:: via use statement"
    );

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
            assert!(
                method_names.contains(&"build"),
                "Should include static 'build', got {:?}",
                method_names
            );
            assert!(
                constant_names.contains(&"VERSION"),
                "Should include constant 'VERSION', got {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_use_statement_aliased() {
    // `use Acme\Service as Svc;` then `$s = new Svc(); $s->`
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Service.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Service {\n",
                "    public function execute(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///runner.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use Acme\\Service as Svc;\n",
        "\n",
        "class Runner {\n",
        "    function run() {\n",
        "        $s = new Svc();\n",
        "        $s->\n",
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

    // Cursor right after `$s->` on line 6
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve aliased $s-> via use statement"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"execute"),
                "Should include 'execute', got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_use_statement_param_type_hint() {
    // Parameter typed with a short name imported via `use`
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Mailer.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Mailer {\n",
                "    public function send(string $to): bool { return true; }\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///notify.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "use Acme\\Mailer;\n",
        "\n",
        "class Notifier {\n",
        "    function notify(Mailer $m) {\n",
        "        $m->\n",
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

    // Cursor right after `$m->` on line 7
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve $m-> via use-statement + param type hint"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"send"),
                "Should include 'send', got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_cross_file_namespace_relative_resolution() {
    // Class referenced without a `use` statement, resolved relative to
    // the current namespace:  inside `namespace Acme;`, bare `Sibling`
    // resolves to `Acme\Sibling`.
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Sibling.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Sibling {\n",
                "    public function greet(): string { return 'hi'; }\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///main.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Acme;\n",
        "\n",
        "class Main {\n",
        "    function run() {\n",
        "        $s = new Sibling();\n",
        "        $s->\n",
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

    // Cursor right after `$s->` on line 6
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve $s-> via namespace-relative lookup"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"greet"),
                "Should include 'greet', got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
