mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

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
    assert!(
        result.is_some(),
        "Completion should return results for $new = new self"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"build"),
                "Should include non-static 'build'"
            );
            assert!(
                !method_names.contains(&"create"),
                "Should exclude static 'create' via ->"
            );
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
    assert!(
        result.is_some(),
        "Completion should return results for $inst = new static"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"build"),
                "Should include non-static 'build'"
            );
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
    assert!(
        result.is_some(),
        "Completion should return results for $w = new Widget"
    );

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
    assert!(
        result.is_some(),
        "Completion should resolve $this->parent-> via self type hint"
    );

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
    assert!(
        result.is_some(),
        "Completion should resolve Registry:: to Registry class"
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

            // Only static method should appear for ::
            assert!(
                method_names.contains(&"instance"),
                "Should include static 'instance'"
            );
            assert!(
                !method_names.contains(&"get"),
                "Should exclude non-static 'get'"
            );
            assert!(
                constant_names.contains(&"VERSION"),
                "Should include constant"
            );
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
    assert!(
        result.is_some(),
        "Completion should resolve $other via parameter type hint"
    );

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
            assert!(
                method_names.contains(&"create"),
                "Should include static 'create'"
            );
            assert!(
                !method_names.contains(&"run"),
                "Should exclude non-static 'run'"
            );
            assert!(
                constant_names.contains(&"MAX"),
                "Should include constant 'MAX'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Completion: new ClassName()->  and  (new ClassName())-> ─────────────────

#[tokio::test]
async fn test_completion_new_classname_arrow() {
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function callCustomCreator(): void {}\n",
        "    public function boot(): void {}\n",
        "    public function run(): void {\n",
        "        new SessionManager()->\n",
        "    }\n",
        "}\n",
    );

    let backend = create_test_backend();
    let uri = Url::parse("file:///test.php").unwrap();
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 30,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for new SessionManager()->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("callCustomCreator")),
                "Should include callCustomCreator, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("boot")),
                "Should include boot, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_parenthesized_new_classname_arrow() {
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function callCustomCreator(): void {}\n",
        "    public function boot(): void {}\n",
        "    public function run(): void {\n",
        "        (new SessionManager())->\n",
        "    }\n",
        "}\n",
    );

    let backend = create_test_backend();
    let uri = Url::parse("file:///test.php").unwrap();
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 32,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for (new SessionManager())->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("callCustomCreator")),
                "Should include callCustomCreator, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("boot")),
                "Should include boot, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_new_classname_arrow_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/SessionManager.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class SessionManager {\n",
                "    public function callCustomCreator(): void {}\n",
                "    public function boot(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\SessionManager;\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        (new SessionManager())->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 32,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for (new SessionManager())-> cross-file"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("callCustomCreator")),
                "Should include callCustomCreator, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("boot")),
                "Should include boot, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Ambiguous Variable Completion Tests ────────────────────────────────────

/// When a variable is conditionally reassigned (if-block), completion should
/// offer the union of members from all candidate types.
#[tokio::test]
async fn test_completion_ambiguous_variable_if_block_union() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ambiguous.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function callCustomCreator2(): void {}\n",
        "    public function start(): void {}\n",
        "}\n",
        "\n",
        "class Manager {\n",
        "    public function doWork(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $thing = new SessionManager();\n",
        "        if ($thing->callCustomCreator2()) {\n",
        "            $thing = new Manager();\n",
        "        }\n",
        "        $thing->\n",
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

    // Cursor after `$thing->` on line 16
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 16,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for ambiguous $thing->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from SessionManager
            assert!(
                labels.iter().any(|l| l.starts_with("callCustomCreator2")),
                "Should include callCustomCreator2 from SessionManager, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("start")),
                "Should include start from SessionManager, got: {:?}",
                labels
            );
            // Should also include members from Manager
            assert!(
                labels.iter().any(|l| l.starts_with("doWork")),
                "Should include doWork from Manager, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Unconditional reassignment: only the final type's members should appear.
#[tokio::test]
async fn test_completion_unconditional_reassignment_only_final_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///unconditional.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function fooOnly(): void {}\n",
        "}\n",
        "\n",
        "class Bar {\n",
        "    public function barOnly(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $x = new Foo();\n",
        "        $x = new Bar();\n",
        "        $x->\n",
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

    // Cursor after `$x->` on line 13
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 13,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $x->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include Bar's method (the final unconditional assignment)
            assert!(
                labels.iter().any(|l| l.starts_with("barOnly")),
                "Should include barOnly from Bar, got: {:?}",
                labels
            );
            // Should NOT include Foo's method (unconditionally replaced)
            assert!(
                !labels.iter().any(|l| l.starts_with("fooOnly")),
                "Should NOT include fooOnly from Foo after unconditional reassignment, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ambiguous variable with if/else: completion shows union of all branches
/// plus the original type.
#[tokio::test]
async fn test_completion_ambiguous_variable_if_else_union() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ifelse.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Writer {\n",
        "    public function write(): void {}\n",
        "}\n",
        "\n",
        "class Printer {\n",
        "    public function print(): void {}\n",
        "}\n",
        "\n",
        "class Sender {\n",
        "    public function send(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $out = new Writer();\n",
        "        if (true) {\n",
        "            $out = new Printer();\n",
        "        } else {\n",
        "            $out = new Sender();\n",
        "        }\n",
        "        $out->\n",
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

    // Cursor after `$out->` on line 21
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 21,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for ambiguous $out->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from all three candidate types
            assert!(
                labels.iter().any(|l| l.starts_with("write")),
                "Should include write from Writer, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("print")),
                "Should include print from Printer, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("send")),
                "Should include send from Sender, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Union Type Completion Tests ────────────────────────────────────────────

/// When a method returns a union type (`B|C`), completion should offer
/// the union of members from all parts of the type.
#[tokio::test]
async fn test_completion_union_return_type_shows_all_members() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///union_completion.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "    public function fetch(): void {}\n",
        "}\n",
        "\n",
        "class Cat {\n",
        "    public function meow(): void {}\n",
        "    public function purr(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function getAnimal(): Dog|Cat { return new Dog(); }\n",
        "\n",
        "    public function run(): void {\n",
        "        $pet = $this->getAnimal();\n",
        "        $pet->\n",
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

    // Cursor after `$pet->` on line 16
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 16,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for union return type Dog|Cat"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from Dog
            assert!(
                labels.iter().any(|l| l.starts_with("bark")),
                "Should include bark from Dog, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("fetch")),
                "Should include fetch from Dog, got: {:?}",
                labels
            );
            // Should also include members from Cat
            assert!(
                labels.iter().any(|l| l.starts_with("meow")),
                "Should include meow from Cat, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("purr")),
                "Should include purr from Cat, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Union type on a parameter: completion shows members from all parts.
#[tokio::test]
async fn test_completion_union_parameter_type_shows_all_members() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///union_param.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Reader {\n",
        "    public function read(): void {}\n",
        "}\n",
        "\n",
        "class Stream {\n",
        "    public function consume(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function process(Reader|Stream $input): void {\n",
        "        $input->\n",
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

    // Cursor after `$input->` on line 11
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 11,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for union param type Reader|Stream"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("read")),
                "Should include read from Reader, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("consume")),
                "Should include consume from Stream, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Union Return + Conditional Reassignment ────────────────────────────────

/// When a variable is assigned from a function returning a union type (A|B)
/// and then conditionally reassigned to a new type (C), the resulting type
/// should be A|B|C — the union should grow, not be special-cased.
#[tokio::test]
async fn test_completion_union_return_plus_conditional_reassignment_grows_union() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///union_grow.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class A {\n",
        "    public function onlyOnA(): void {}\n",
        "}\n",
        "\n",
        "class B {\n",
        "    public function onlyOnB(): void {}\n",
        "}\n",
        "\n",
        "class C {\n",
        "    public function onlyOnC(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    /** @return A|B */\n",
        "    public function makeAOrB(): A|B { return new A(); }\n",
        "\n",
        "    public function run(): void {\n",
        "        $thing = $this->makeAOrB();\n",
        "        if (true) {\n",
        "            $thing = new C();\n",
        "        }\n",
        "        $thing->\n",
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

    // Cursor after `$thing->` on line 22
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 22,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $thing-> after union return + conditional reassignment"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from A (from makeAOrB union part)
            assert!(
                labels.iter().any(|l| l.starts_with("onlyOnA")),
                "Should include onlyOnA from A (union return), got: {:?}",
                labels
            );
            // Should include members from B (from makeAOrB union part)
            assert!(
                labels.iter().any(|l| l.starts_with("onlyOnB")),
                "Should include onlyOnB from B (union return), got: {:?}",
                labels
            );
            // Should include members from C (conditional reassignment)
            assert!(
                labels.iter().any(|l| l.starts_with("onlyOnC")),
                "Should include onlyOnC from C (conditional reassignment), got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
