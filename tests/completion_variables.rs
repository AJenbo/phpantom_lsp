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

// ─── PHPStan Conditional Return Type Tests ──────────────────────────────────

/// When a function has a PHPStan conditional return type like
/// `@return ($abstract is class-string<TClass> ? TClass : mixed)`
/// and is called with `A::class`, completion should resolve to class A.
#[tokio::test]
async fn test_completion_conditional_return_class_string() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///conditional.php").unwrap();
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
        "/**\n",
        " * @return ($abstract is class-string<TClass> ? TClass : ($abstract is null ? \\App : mixed))\n",
        " */\n",
        "function app($abstract = null, array $parameters = []) {}\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        $obj = app(A::class);\n",
        "        $obj->\n",
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

    // Cursor after `$obj->` on line 17
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 17,
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
        "Completion should return results for $obj-> after app(A::class)"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from A (resolved via class-string<T>)
            assert!(
                labels.iter().any(|l| l.starts_with("onlyOnA")),
                "Should include onlyOnA from A (resolved via class-string conditional), got: {:?}",
                labels
            );
            // Should NOT include members from B
            assert!(
                !labels.iter().any(|l| l.starts_with("onlyOnB")),
                "Should NOT include onlyOnB from B, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When a function has a PHPStan conditional return type and is called
/// without arguments, it should resolve to the null-default branch.
/// e.g. `app()` → Application
#[tokio::test]
async fn test_completion_conditional_return_null_default() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///conditional_null.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Application {\n",
        "    public function version(): string {}\n",
        "    public function boot(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @return ($abstract is class-string<TClass> ? TClass : ($abstract is null ? Application : mixed))\n",
        " */\n",
        "function app($abstract = null, array $parameters = []) {}\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        $a = app();\n",
        "        $a->\n",
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

    // Cursor after `$a->` on line 14
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 14,
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
        "Completion should return results for $a-> after app()"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("version")),
                "Should include version from Application (null-default branch), got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("boot")),
                "Should include boot from Application (null-default branch), got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When a function has `@return ($guard is null ? Factory : StatefulGuard)`
/// and is called with a non-null argument like `auth('web')`, completion
/// should resolve to the else branch (StatefulGuard).
#[tokio::test]
async fn test_completion_conditional_return_non_null_argument() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///conditional_auth.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public function guard(): void {}\n",
        "}\n",
        "\n",
        "class StatefulGuard {\n",
        "    public function login(): void {}\n",
        "    public function logout(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @return ($guard is null ? Factory : StatefulGuard)\n",
        " */\n",
        "function auth($guard = null) {}\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        $g = auth('web');\n",
        "        $g->\n",
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

    // Cursor after `$g->` on line 18
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 18,
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
        "Completion should return results for $g-> after auth('web')"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from StatefulGuard (non-null arg → else branch)
            assert!(
                labels.iter().any(|l| l.starts_with("login")),
                "Should include login from StatefulGuard, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("logout")),
                "Should include logout from StatefulGuard, got: {:?}",
                labels
            );
            // Should NOT include members from Factory
            assert!(
                !labels.iter().any(|l| l.starts_with("guard")),
                "Should NOT include guard from Factory, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When `app(A::class)->` is used inline (without assigning to a variable),
/// completion should resolve the conditional return type using the text
/// arguments and offer members of `A`.
#[tokio::test]
async fn test_completion_inline_conditional_return_class_string() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_conditional.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function callCustomCreator2(): void {}\n",
        "    public function driver(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @return ($abstract is class-string<TClass> ? TClass : ($abstract is null ? \\App : mixed))\n",
        " */\n",
        "function app($abstract = null, array $parameters = []) {}\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        app(SessionManager::class)->\n",
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

    // Cursor after `app(SessionManager::class)->` on line 13
    // 8 spaces + "app(SessionManager::class)->" = 8 + 28 = 36
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 13,
                character: 36,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for app(SessionManager::class)->"
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
                labels.iter().any(|l| l.starts_with("driver")),
                "Should include driver from SessionManager, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When `auth('web')->` is used inline (without assigning to a variable),
/// the non-null argument should resolve to the else branch of an `is null`
/// conditional return type.
#[tokio::test]
async fn test_completion_inline_conditional_return_non_null_argument() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_auth.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public function guard(): void {}\n",
        "}\n",
        "\n",
        "class StatefulGuard {\n",
        "    public function login(): void {}\n",
        "    public function logout(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @return ($guard is null ? Factory : StatefulGuard)\n",
        " */\n",
        "function auth($guard = null) {}\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        auth('web')->\n",
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

    // Cursor after `auth('web')->` on line 17
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 17,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for auth('web')->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from StatefulGuard (non-null arg → else branch)
            assert!(
                labels.iter().any(|l| l.starts_with("login")),
                "Should include login from StatefulGuard, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("logout")),
                "Should include logout from StatefulGuard, got: {:?}",
                labels
            );
            // Should NOT include members from Factory
            assert!(
                !labels.iter().any(|l| l.starts_with("guard")),
                "Should NOT include guard from Factory, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When `app()->` is used inline with no arguments, the null-default
/// branch should be taken, just as when assigned to a variable.
#[tokio::test]
async fn test_completion_inline_conditional_return_null_default() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_null.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Application {\n",
        "    public function version(): string {}\n",
        "    public function boot(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @return ($abstract is class-string<TClass> ? TClass : ($abstract is null ? Application : mixed))\n",
        " */\n",
        "function app($abstract = null, array $parameters = []) {}\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        app()->\n",
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

    // Cursor after `app()->` on line 13
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 13,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for app()->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("version")),
                "Should include version from Application (null-default branch), got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("boot")),
                "Should include boot from Application (null-default branch), got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When a **method** has a PHPStan conditional return type (e.g.
/// `Application::make`), chaining through it should resolve the type
/// correctly.  For example:
///
/// ```php
/// app()->make(CurrentCart::class)->save();
/// ```
///
/// `app()` returns `Application`, `make(CurrentCart::class)` should
/// resolve via the conditional `@return` to `CurrentCart`, and then
/// `->save()` (or `->` completion) should offer `CurrentCart` members.
#[tokio::test]
async fn test_completion_method_conditional_return_class_string_chain() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_conditional.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class CurrentCart {\n",
        "    public function save(): void {}\n",
        "    public function getTotal(): float {}\n",
        "}\n",
        "\n",
        "class Application {\n",
        "    /**\n",
        "     * @template TClass of object\n",
        "     * @param  string|class-string<TClass>  $abstract\n",
        "     * @return ($abstract is class-string<TClass> ? TClass : mixed)\n",
        "     */\n",
        "    public function make($abstract, array $parameters = []) {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @return Application\n",
        " */\n",
        "function app() {}\n",
        "\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        app()->make(CurrentCart::class)->\n",
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

    // Cursor after `app()->make(CurrentCart::class)->` on line 22
    // 8 spaces + "app()->make(CurrentCart::class)->" = 8 + 33 = 41
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 22,
                character: 41,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for app()->make(CurrentCart::class)->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should include members from CurrentCart (resolved via method conditional return)
            assert!(
                labels.iter().any(|l| l.starts_with("save")),
                "Should include save from CurrentCart (resolved via method class-string conditional), got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("getTotal")),
                "Should include getTotal from CurrentCart, got: {:?}",
                labels
            );
            // Should NOT include members from Application
            assert!(
                !labels.iter().any(|l| l.starts_with("make")),
                "Should NOT include make from Application, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ── Inline @var docblock override tests ─────────────────────────────────────

#[tokio::test]
async fn test_completion_inline_var_docblock_simple() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inlinevar.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Session {\n",
        "    public function getId(): string {}\n",
        "    public function flash(): void {}\n",
        "}\n",
        "class Controller {\n",
        "    public function handle() {\n",
        "        /** @var Session */\n",
        "        $sess = mystery();\n",
        "        $sess->\n",
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
                line: 9,
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
        "Completion should return results for @var Session"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"getId"),
                "Should include getId from Session, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"flash"),
                "Should include flash from Session, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inline_var_docblock_with_variable_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inlinevar_named.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(): void {}\n",
        "    public function error(): void {}\n",
        "}\n",
        "class App {\n",
        "    public function run() {\n",
        "        /** @var Logger $log */\n",
        "        $log = getLogger();\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 9,
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
        "Completion should return results for @var Logger $log"
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
                "Should include info from Logger, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"error"),
                "Should include error from Logger, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inline_var_docblock_wrong_variable_name_ignored() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inlinevar_wrong.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(): void {}\n",
        "}\n",
        "class App {\n",
        "    public function run() {\n",
        "        /** @var Logger $other */\n",
        "        $log = something();\n",
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
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            // The @var annotation names $other, not $log — so it should NOT
            // apply and $log should fall back to the default placeholder.
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                !method_names.contains(&"info"),
                "Should NOT include Logger::info when @var names a different variable, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inline_var_docblock_override_blocked_by_scalar() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inlinevar_scalar.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Session {\n",
        "    public function getId(): string {}\n",
        "}\n",
        "class App {\n",
        "    public function getName(): string {}\n",
        "    public function run() {\n",
        "        /** @var Session */\n",
        "        $s = $this->getName();\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 9,
                character: 12,
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
            // getName() returns `string` — the @var Session override should
            // be blocked because string is a scalar.
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                !method_names.contains(&"getId"),
                "Should NOT include Session::getId when native type is scalar string, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inline_var_docblock_override_allowed_for_object() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inlinevar_obj.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class BaseService {\n",
        "    public function base(): void {}\n",
        "}\n",
        "class Session extends BaseService {\n",
        "    public function getId(): string {}\n",
        "    public function flash(): void {}\n",
        "}\n",
        "class App {\n",
        "    public function getService(): BaseService {}\n",
        "    public function run() {\n",
        "        /** @var Session */\n",
        "        $s = $this->getService();\n",
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

    let completion_params = CompletionParams {
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

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results when @var overrides a class type"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"getId"),
                "Should include getId from Session (override allowed), got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"flash"),
                "Should include flash from Session (override allowed), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inline_var_docblock_unconditional_reassignment() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inlinevar_reassign.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class First {\n",
        "    public function one(): void {}\n",
        "}\n",
        "class Second {\n",
        "    public function two(): void {}\n",
        "}\n",
        "class App {\n",
        "    public function run() {\n",
        "        $x = new First();\n",
        "        /** @var Second */\n",
        "        $x = transform();\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 12,
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
        "Completion should return results for reassigned @var"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"two"),
                "Should include two from Second (latest assignment), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"one"),
                "Should NOT include one from First (overwritten), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── instanceof narrowing ───────────────────────────────────────────────────

/// When the cursor is inside `if ($var instanceof Foo) { … }`, only
/// members of `Foo` should be suggested — not the full union.
#[tokio::test]
async fn test_completion_instanceof_narrows_to_single_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///instanceof_basic.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "class Animal {\n",                         // 1
        "    public function breathe(): void {}\n", // 2
        "}\n",                                      // 3
        "class Dog extends Animal {\n",             // 4
        "    public function bark(): void {}\n",    // 5
        "}\n",                                      // 6
        "class Cat extends Animal {\n",             // 7
        "    public function purr(): void {}\n",    // 8
        "}\n",                                      // 9
        "class Svc {\n",                            // 10
        "    public function test(): void {\n",     // 11
        "        if (rand(0,1)) {\n",               // 12
        "            $pet = new Dog();\n",          // 13
        "        } else {\n",                       // 14
        "            $pet = new Cat();\n",          // 15
        "        }\n",                              // 16
        "        if ($pet instanceof Dog) {\n",     // 17
        "            $pet->\n",                     // 18
        "        }\n",                              // 19
        "    }\n",                                  // 20
        "}\n",                                      // 21
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 18,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's own method 'bark', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"breathe"),
                "Should include inherited method 'breathe', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's method 'purr', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// After the instanceof block, the full union should be restored.
#[tokio::test]
async fn test_completion_instanceof_no_narrowing_outside_block() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///instanceof_outside.php").unwrap();
    let text = concat!(
        "<?php\n",                                      // 0
        "class Alpha {\n",                              // 1
        "    public function alphaMethod(): void {}\n", // 2
        "}\n",                                          // 3
        "class Beta {\n",                               // 4
        "    public function betaMethod(): void {}\n",  // 5
        "}\n",                                          // 6
        "class Svc {\n",                                // 7
        "    public function test(): void {\n",         // 8
        "        if (rand(0,1)) {\n",                   // 9
        "            $obj = new Alpha();\n",            // 10
        "        } else {\n",                           // 11
        "            $obj = new Beta();\n",             // 12
        "        }\n",                                  // 13
        "        if ($obj instanceof Alpha) {\n",       // 14
        "            // narrowed here\n",               // 15
        "        }\n",                                  // 16
        "        $obj->\n",                             // 17
        "    }\n",                                      // 18
        "}\n",                                          // 19
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 17,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Outside the instanceof block, both types should be available
            assert!(
                method_names.contains(&"alphaMethod"),
                "Should still include 'alphaMethod' after instanceof block, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"betaMethod"),
                "Should still include 'betaMethod' after instanceof block, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `elseif ($var instanceof OtherClass)` should narrow to OtherClass.
#[tokio::test]
async fn test_completion_instanceof_elseif_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///instanceof_elseif.php").unwrap();
    let text = concat!(
        "<?php\n",                                    // 0
        "class Foo {\n",                              // 1
        "    public function fooMethod(): void {}\n", // 2
        "}\n",                                        // 3
        "class Bar {\n",                              // 4
        "    public function barMethod(): void {}\n", // 5
        "}\n",                                        // 6
        "class Baz {\n",                              // 7
        "    public function bazMethod(): void {}\n", // 8
        "}\n",                                        // 9
        "class Svc {\n",                              // 10
        "    public function test(): void {\n",       // 11
        "        if (rand(0,1)) {\n",                 // 12
        "            $x = new Foo();\n",              // 13
        "        } elseif (rand(0,1)) {\n",           // 14
        "            $x = new Bar();\n",              // 15
        "        } else {\n",                         // 16
        "            $x = new Baz();\n",              // 17
        "        }\n",                                // 18
        "        if ($x instanceof Foo) {\n",         // 19
        "            // cursor NOT here\n",           // 20
        "        } elseif ($x instanceof Bar) {\n",   // 21
        "            $x->\n",                         // 22
        "        }\n",                                // 23
        "    }\n",                                    // 24
        "}\n",                                        // 25
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

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"barMethod"),
                "elseif instanceof Bar should narrow to Bar, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"fooMethod"),
                "Should NOT include Foo methods inside Bar elseif, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bazMethod"),
                "Should NOT include Baz methods inside Bar elseif, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// instanceof with a different variable name should NOT narrow our variable.
#[tokio::test]
async fn test_completion_instanceof_different_variable_no_effect() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///instanceof_other_var.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "class TypeA {\n",                          // 1
        "    public function aMethod(): void {}\n", // 2
        "}\n",                                      // 3
        "class TypeB {\n",                          // 4
        "    public function bMethod(): void {}\n", // 5
        "}\n",                                      // 6
        "class Svc {\n",                            // 7
        "    public function test(): void {\n",     // 8
        "        if (rand(0,1)) {\n",               // 9
        "            $obj = new TypeA();\n",        // 10
        "        } else {\n",                       // 11
        "            $obj = new TypeB();\n",        // 12
        "        }\n",                              // 13
        "        $other = new TypeA();\n",          // 14
        "        if ($other instanceof TypeA) {\n", // 15
        "            $obj->\n",                     // 16
        "        }\n",                              // 17
        "    }\n",                                  // 18
        "}\n",                                      // 19
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 16,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // $obj should NOT be narrowed because the instanceof checks $other
            assert!(
                method_names.contains(&"aMethod"),
                "Should include aMethod (union not narrowed), got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"bMethod"),
                "Should include bMethod (union not narrowed), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// instanceof narrowing in top-level code (outside any class).
#[tokio::test]
async fn test_completion_instanceof_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///instanceof_toplevel.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Container {\n",                              // 1
        "    public function bind(): void {}\n",            // 2
        "}\n",                                              // 3
        "class AdminUser {\n",                              // 4
        "    public function grantPermission(): void {}\n", // 5
        "}\n",                                              // 6
        "\n",                                               // 7
        "if (rand(0, 1)) {\n",                              // 8
        "    $ambiguous = new Container();\n",              // 9
        "} else {\n",                                       // 10
        "    $ambiguous = new AdminUser();\n",              // 11
        "}\n",                                              // 12
        "if ($ambiguous instanceof AdminUser) {\n",         // 13
        "    $ambiguous->\n",                               // 14
        "}\n",                                              // 15
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"grantPermission"),
                "Should include AdminUser's 'grantPermission', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bind"),
                "Should NOT include Container's 'bind', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// instanceof narrowing should resolve the class via cross-file PSR-4.
#[tokio::test]
async fn test_completion_instanceof_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[
            (
                "src/Vehicle.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Vehicle {\n",
                    "    public function drive(): void {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Truck.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Truck extends Vehicle {\n",
                    "    public function haul(): void {}\n",
                    "}\n",
                ),
            ),
        ],
    );

    let uri = Url::parse("file:///main.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Vehicle;\n",
        "use App\\Truck;\n",
        "class Dispatch {\n",
        "    public function run(Vehicle $v): void {\n",
        "        if ($v instanceof Truck) {\n",
        "            $v->\n",
        "        }\n",
        "    }\n",
        "}\n",
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

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"haul"),
                "Should include Truck's own method 'haul', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"drive"),
                "Should include inherited method 'drive', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Parenthesised instanceof condition should also narrow.
#[tokio::test]
async fn test_completion_instanceof_parenthesised_condition() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///instanceof_parens.php").unwrap();
    let text = concat!(
        "<?php\n",                                    // 0
        "class One {\n",                              // 1
        "    public function oneMethod(): void {}\n", // 2
        "}\n",                                        // 3
        "class Two {\n",                              // 4
        "    public function twoMethod(): void {}\n", // 5
        "}\n",                                        // 6
        "class Svc {\n",                              // 7
        "    public function test(): void {\n",       // 8
        "        if (rand(0,1)) {\n",                 // 9
        "            $val = new One();\n",            // 10
        "        } else {\n",                         // 11
        "            $val = new Two();\n",            // 12
        "        }\n",                                // 13
        "        if (($val instanceof Two)) {\n",     // 14
        "            $val->\n",                       // 15
        "        }\n",                                // 16
        "    }\n",                                    // 17
        "}\n",                                        // 18
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"twoMethod"),
                "Parenthesised instanceof should narrow to Two, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"oneMethod"),
                "Should NOT include One's methods, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_top_level_chained_method_on_variable() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///toplevel_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string { return ''; }\n",
        "    public function getEmail(): string { return ''; }\n",
        "    public function getProfile(): UserProfile {\n",
        "        return new UserProfile();\n",
        "    }\n",
        "}\n",
        "\n",
        "class UserProfile {\n",
        "    public string $bio = '';\n",
        "    public function getUser(): User {\n",
        "        return new User();\n",
        "    }\n",
        "    public function getDisplayName(): string {\n",
        "        return '';\n",
        "    }\n",
        "}\n",
        "\n",
        "$profile = new UserProfile();\n",
        "$profile->getUser()->\n",
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
                line: 20,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $profile->getUser()->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"getName"),
                "Should include 'getName' from User via chain, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getEmail"),
                "Should include 'getEmail' from User via chain, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getProfile"),
                "Should include 'getProfile' from User via chain, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_chained_method_on_variable_inside_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///chain_in_class.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string { return ''; }\n",
        "    public function getEmail(): string { return ''; }\n",
        "}\n",
        "\n",
        "class UserProfile {\n",
        "    public function getUser(): User {\n",
        "        return new User();\n",
        "    }\n",
        "    public function test(): void {\n",
        "        $p = new UserProfile();\n",
        "        $p->getUser()->\n",
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
                line: 12,
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
        "Completion should return results for $p->getUser()-> inside a method"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"getName"),
                "Should include 'getName' from User via chain, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getEmail"),
                "Should include 'getEmail' from User via chain, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_top_level_variable_new_classname() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///toplevel.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $email;\n",
        "    public function getName(): string { return ''; }\n",
        "    public function getEmail(): string { return ''; }\n",
        "}\n",
        "\n",
        "$user = new User();\n",
        "$user->\n",
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
                line: 8,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for top-level $user = new User()"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"getName"),
                "Should include 'getName', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getEmail"),
                "Should include 'getEmail', got: {:?}",
                method_names
            );

            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                prop_names.contains(&"email"),
                "Should include property 'email', got: {:?}",
                prop_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_top_level_variable_from_function_call() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///toplevel_func.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Order {\n",
        "    public function getTotal(): float { return 0.0; }\n",
        "    public function getStatus(): string { return ''; }\n",
        "}\n",
        "\n",
        "function createOrder(): Order {\n",
        "    return new Order();\n",
        "}\n",
        "\n",
        "$order = createOrder();\n",
        "$order->\n",
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
                line: 11,
                character: 8,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for top-level $order = createOrder()"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                method_names.contains(&"getTotal"),
                "Should include 'getTotal', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getStatus"),
                "Should include 'getStatus', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── assert($var instanceof …) narrowing ────────────────────────────────────

/// When `assert($var instanceof Foo)` appears before the cursor,
/// only members of `Foo` should be suggested.
#[tokio::test]
async fn test_completion_assert_instanceof_narrows_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_instanceof_basic.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "class Animal {\n",                         // 1
        "    public function breathe(): void {}\n", // 2
        "}\n",                                      // 3
        "class Dog extends Animal {\n",             // 4
        "    public function bark(): void {}\n",    // 5
        "}\n",                                      // 6
        "class Cat extends Animal {\n",             // 7
        "    public function purr(): void {}\n",    // 8
        "}\n",                                      // 9
        "class Svc {\n",                            // 10
        "    public function test(): void {\n",     // 11
        "        if (rand(0,1)) {\n",               // 12
        "            $pet = new Dog();\n",          // 13
        "        } else {\n",                       // 14
        "            $pet = new Cat();\n",          // 15
        "        }\n",                              // 16
        "        assert($pet instanceof Dog);\n",   // 17
        "        $pet->\n",                         // 18
        "    }\n",                                  // 19
        "}\n",                                      // 20
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 18,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's own method 'bark', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"breathe"),
                "Should include inherited method 'breathe', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's method 'purr', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `assert()` narrowing should work in top-level code (outside a class).
#[tokio::test]
async fn test_completion_assert_instanceof_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_instanceof_toplevel.php").unwrap();
    let text = concat!(
        "<?php\n",                                      // 0
        "class Alpha {\n",                              // 1
        "    public function alphaMethod(): void {}\n", // 2
        "}\n",                                          // 3
        "class Beta {\n",                               // 4
        "    public function betaMethod(): void {}\n",  // 5
        "}\n",                                          // 6
        "if (rand(0,1)) {\n",                           // 7
        "    $obj = new Alpha();\n",                    // 8
        "} else {\n",                                   // 9
        "    $obj = new Beta();\n",                     // 10
        "}\n",                                          // 11
        "assert($obj instanceof Alpha);\n",             // 12
        "$obj->\n",                                     // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 13,
                    character: 6,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"alphaMethod"),
                "Should include 'alphaMethod', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"betaMethod"),
                "Should NOT include 'betaMethod' after assert, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `assert()` narrowing should work on parameters with type hints.
#[tokio::test]
async fn test_completion_assert_instanceof_narrows_parameter() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_instanceof_param.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Base {\n",                                   // 1
        "    public function baseMethod(): void {}\n",      // 2
        "}\n",                                              // 3
        "class Child extends Base {\n",                     // 4
        "    public function childMethod(): void {}\n",     // 5
        "}\n",                                              // 6
        "class Handler {\n",                                // 7
        "    public function handle(Base $item): void {\n", // 8
        "        assert($item instanceof Child);\n",        // 9
        "        $item->\n",                                // 10
        "    }\n",                                          // 11
        "}\n",                                              // 12
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"childMethod"),
                "Should include Child's 'childMethod', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"baseMethod"),
                "Should include inherited 'baseMethod', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `assert()` with a different variable should not affect the target variable.
#[tokio::test]
async fn test_completion_assert_instanceof_different_variable_no_effect() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_instanceof_diffvar.php").unwrap();
    let text = concat!(
        "<?php\n",                                    // 0
        "class Foo {\n",                              // 1
        "    public function fooMethod(): void {}\n", // 2
        "}\n",                                        // 3
        "class Bar {\n",                              // 4
        "    public function barMethod(): void {}\n", // 5
        "}\n",                                        // 6
        "class Svc {\n",                              // 7
        "    public function run(): void {\n",        // 8
        "        $a = new Foo();\n",                  // 9
        "        $b = new Bar();\n",                  // 10
        "        assert($b instanceof Foo);\n",       // 11
        "        $a->\n",                             // 12
        "    }\n",                                    // 13
        "}\n",                                        // 14
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // $a is still Foo — the assert on $b should not affect $a
            assert!(
                method_names.contains(&"fooMethod"),
                "Should include 'fooMethod' for $a, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"barMethod"),
                "Should NOT include 'barMethod' for $a, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `assert()` narrowing should work with cross-file PSR-4 class resolution.
#[tokio::test]
async fn test_completion_assert_instanceof_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[(
            "src/Models/User.php",
            concat!(
                "<?php\n",
                "namespace App\\Models;\n",
                "class User {\n",
                "    public function addRoles(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///assert_instanceof_cross.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Models\\User;\n",
        "class Base {\n",
        "    public function baseMethod(): void {}\n",
        "}\n",
        "function getUnknownValue(int $i): Base { return new Base(); }\n",
        "$asserted = getUnknownValue(1);\n",
        "assert($asserted instanceof User);\n",
        "$asserted->\n",
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

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addRoles"),
                "Should include User's 'addRoles' via cross-file resolution, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"baseMethod"),
                "Should NOT include Base's 'baseMethod' after assert narrowing, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `assert()` with parenthesised inner expression should also narrow.
#[tokio::test]
async fn test_completion_assert_instanceof_parenthesised() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_instanceof_parens.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "class X {\n",                              // 1
        "    public function xMethod(): void {}\n", // 2
        "}\n",                                      // 3
        "class Y {\n",                              // 4
        "    public function yMethod(): void {}\n", // 5
        "}\n",                                      // 6
        "class Svc {\n",                            // 7
        "    public function run(): void {\n",      // 8
        "        if (rand(0,1)) {\n",               // 9
        "            $v = new X();\n",              // 10
        "        } else {\n",                       // 11
        "            $v = new Y();\n",              // 12
        "        }\n",                              // 13
        "        assert(($v instanceof X));\n",     // 14
        "        $v->\n",                           // 15
        "    }\n",                                  // 16
        "}\n",                                      // 17
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"xMethod"),
                "Should include X's 'xMethod', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"yMethod"),
                "Should NOT include Y's 'yMethod' after assert narrowing, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── negated instanceof narrowing ───────────────────────────────────────────

/// `assert(!$var instanceof ClassName)` should *exclude* ClassName from
/// the candidate set (PHP precedence: `instanceof` binds tighter than `!`,
/// so `!$x instanceof Foo` ≡ `!($x instanceof Foo)`).
#[tokio::test]
async fn test_completion_assert_negated_instanceof_excludes_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_neg_instanceof.php").unwrap();
    let text = concat!(
        "<?php\n",                                      // 0
        "class Alpha {\n",                              // 1
        "    public function alphaMethod(): void {}\n", // 2
        "}\n",                                          // 3
        "class Beta {\n",                               // 4
        "    public function betaMethod(): void {}\n",  // 5
        "}\n",                                          // 6
        "class Svc {\n",                                // 7
        "    public function run(): void {\n",          // 8
        "        if (rand(0,1)) {\n",                   // 9
        "            $obj = new Alpha();\n",            // 10
        "        } else {\n",                           // 11
        "            $obj = new Beta();\n",             // 12
        "        }\n",                                  // 13
        "        assert(!$obj instanceof Alpha);\n",    // 14
        "        $obj->\n",                             // 15
        "    }\n",                                      // 16
        "}\n",                                          // 17
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"betaMethod"),
                "Should include Beta's 'betaMethod' after excluding Alpha, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"alphaMethod"),
                "Should NOT include Alpha's 'alphaMethod' after negated assert, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `assert(!($var instanceof ClassName))` with explicit parens should
/// also exclude ClassName.
#[tokio::test]
async fn test_completion_assert_negated_instanceof_parenthesised_excludes_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_neg_instanceof_parens.php").unwrap();
    let text = concat!(
        "<?php\n",                                    // 0
        "class Foo {\n",                              // 1
        "    public function fooMethod(): void {}\n", // 2
        "}\n",                                        // 3
        "class Bar {\n",                              // 4
        "    public function barMethod(): void {}\n", // 5
        "}\n",                                        // 6
        "class Svc {\n",                              // 7
        "    public function run(): void {\n",        // 8
        "        if (rand(0,1)) {\n",                 // 9
        "            $x = new Foo();\n",              // 10
        "        } else {\n",                         // 11
        "            $x = new Bar();\n",              // 12
        "        }\n",                                // 13
        "        assert(!($x instanceof Foo));\n",    // 14
        "        $x->\n",                             // 15
        "    }\n",                                    // 16
        "}\n",                                        // 17
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"barMethod"),
                "Should include Bar's 'barMethod', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"fooMethod"),
                "Should NOT include Foo's 'fooMethod' after negated assert, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if (!($var instanceof ClassName)) { … }` should exclude ClassName
/// inside the if body.
#[tokio::test]
async fn test_completion_if_negated_instanceof_excludes_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///if_neg_instanceof.php").unwrap();
    let text = concat!(
        "<?php\n",                                 // 0
        "class Cat {\n",                           // 1
        "    public function purr(): void {}\n",   // 2
        "}\n",                                     // 3
        "class Dog {\n",                           // 4
        "    public function bark(): void {}\n",   // 5
        "}\n",                                     // 6
        "class Svc {\n",                           // 7
        "    public function run(): void {\n",     // 8
        "        if (rand(0,1)) {\n",              // 9
        "            $pet = new Cat();\n",         // 10
        "        } else {\n",                      // 11
        "            $pet = new Dog();\n",         // 12
        "        }\n",                             // 13
        "        if (!($pet instanceof Cat)) {\n", // 14
        "            $pet->\n",                    // 15
        "        }\n",                             // 16
        "    }\n",                                 // 17
        "}\n",                                     // 18
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' inside negated instanceof block, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' inside negated instanceof block, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if (!$var instanceof ClassName)` without explicit inner parens should
/// also exclude (PHP precedence: `instanceof` > `!`).
#[tokio::test]
async fn test_completion_if_negated_instanceof_no_inner_parens() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///if_neg_instanceof_no_parens.php").unwrap();
    let text = concat!(
        "<?php\n",                                   // 0
        "class A1 {\n",                              // 1
        "    public function a1Method(): void {}\n", // 2
        "}\n",                                       // 3
        "class B1 {\n",                              // 4
        "    public function b1Method(): void {}\n", // 5
        "}\n",                                       // 6
        "class Svc {\n",                             // 7
        "    public function run(): void {\n",       // 8
        "        if (rand(0,1)) {\n",                // 9
        "            $v = new A1();\n",              // 10
        "        } else {\n",                        // 11
        "            $v = new B1();\n",              // 12
        "        }\n",                               // 13
        "        if (!$v instanceof A1) {\n",        // 14
        "            $v->\n",                        // 15
        "        }\n",                               // 16
        "    }\n",                                   // 17
        "}\n",                                       // 18
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"b1Method"),
                "Should include B1's 'b1Method', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"a1Method"),
                "Should NOT include A1's 'a1Method' after negated instanceof, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if ($var instanceof ClassName) { … } else { … }` — inside the else
/// branch the variable is NOT ClassName, so exclude it.
#[tokio::test]
async fn test_completion_if_instanceof_else_excludes_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///if_instanceof_else.php").unwrap();
    let text = concat!(
        "<?php\n",                                     // 0
        "class Red {\n",                               // 1
        "    public function redMethod(): void {}\n",  // 2
        "}\n",                                         // 3
        "class Blue {\n",                              // 4
        "    public function blueMethod(): void {}\n", // 5
        "}\n",                                         // 6
        "class Svc {\n",                               // 7
        "    public function run(): void {\n",         // 8
        "        if (rand(0,1)) {\n",                  // 9
        "            $c = new Red();\n",               // 10
        "        } else {\n",                          // 11
        "            $c = new Blue();\n",              // 12
        "        }\n",                                 // 13
        "        if ($c instanceof Red) {\n",          // 14
        "            // narrowed to Red here\n",       // 15
        "        } else {\n",                          // 16
        "            $c->\n",                          // 17
        "        }\n",                                 // 18
        "    }\n",                                     // 19
        "}\n",                                         // 20
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 17,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"blueMethod"),
                "Should include Blue's 'blueMethod' in else branch, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"redMethod"),
                "Should NOT include Red's 'redMethod' in else branch, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if ($var instanceof ClassName) { … } else { … }` — the then branch
/// should still narrow positively (regression check).
#[tokio::test]
async fn test_completion_if_instanceof_then_still_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///if_instanceof_then_check.php").unwrap();
    let text = concat!(
        "<?php\n",                                     // 0
        "class Red {\n",                               // 1
        "    public function redMethod(): void {}\n",  // 2
        "}\n",                                         // 3
        "class Blue {\n",                              // 4
        "    public function blueMethod(): void {}\n", // 5
        "}\n",                                         // 6
        "class Svc {\n",                               // 7
        "    public function run(): void {\n",         // 8
        "        if (rand(0,1)) {\n",                  // 9
        "            $c = new Red();\n",               // 10
        "        } else {\n",                          // 11
        "            $c = new Blue();\n",              // 12
        "        }\n",                                 // 13
        "        if ($c instanceof Red) {\n",          // 14
        "            $c->\n",                          // 15
        "        } else {\n",                          // 16
        "            // excluded here\n",              // 17
        "        }\n",                                 // 18
        "    }\n",                                     // 19
        "}\n",                                         // 20
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"redMethod"),
                "Should include Red's 'redMethod' in then branch, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"blueMethod"),
                "Should NOT include Blue's 'blueMethod' in then branch, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if ($param instanceof ClassName) { … } else { … }` — the else branch
/// should exclude ClassName when the variable comes from a parameter.
#[tokio::test]
async fn test_completion_if_instanceof_else_excludes_with_parameter() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///if_instanceof_else_param.php").unwrap();
    let text = concat!(
        "<?php\n",                                        // 0
        "class Sun {\n",                                  // 1
        "    public function shine(): void {}\n",         // 2
        "}\n",                                            // 3
        "class Moon {\n",                                 // 4
        "    public function glow(): void {}\n",          // 5
        "}\n",                                            // 6
        "class Svc {\n",                                  // 7
        "    public function run(Sun|Moon $s): void {\n", // 8
        "        if ($s instanceof Sun) {\n",             // 9
        "            // narrowed to Sun\n",               // 10
        "        } else {\n",                             // 11
        "            $s->\n",                             // 12
        "        }\n",                                    // 13
        "    }\n",                                        // 14
        "}\n",                                            // 15
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"glow"),
                "Should include Moon's 'glow' in else branch, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"shine"),
                "Should NOT include Sun's 'shine' in else branch, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Top-level `if ($var instanceof …) {} else {}` should also narrow in
/// the else branch.
#[tokio::test]
async fn test_completion_if_instanceof_else_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///if_instanceof_else_toplevel.php").unwrap();
    let text = concat!(
        "<?php\n",                               // 0
        "class Up {\n",                          // 1
        "    public function rise(): void {}\n", // 2
        "}\n",                                   // 3
        "class Down {\n",                        // 4
        "    public function fall(): void {}\n", // 5
        "}\n",                                   // 6
        "if (rand(0,1)) {\n",                    // 7
        "    $dir = new Up();\n",                // 8
        "} else {\n",                            // 9
        "    $dir = new Down();\n",              // 10
        "}\n",                                   // 11
        "if ($dir instanceof Up) {\n",           // 12
        "    // narrowed to Up\n",               // 13
        "} else {\n",                            // 14
        "    $dir->\n",                          // 15
        "}\n",                                   // 16
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 10,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"fall"),
                "Should include Down's 'fall' in else branch, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"rise"),
                "Should NOT include Up's 'rise' in else branch, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── while-loop instanceof narrowing ────────────────────────────────────────

/// When the cursor is inside `while ($var instanceof Foo) { … }`, only
/// members of `Foo` should be suggested.
#[tokio::test]
async fn test_completion_while_instanceof_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///while_instanceof.php").unwrap();
    let text = concat!(
        "<?php\n",                                             // 0
        "class Node {\n",                                      // 1
        "    public function next(): ?Node {}\n",              // 2
        "    public function getValue(): string {}\n",         // 3
        "}\n",                                                 // 4
        "class Leaf {\n",                                      // 5
        "    public function leafOnly(): void {}\n",           // 6
        "}\n",                                                 // 7
        "class Svc {\n",                                       // 8
        "    public function walk(Node|Leaf $item): void {\n", // 9
        "        while ($item instanceof Node) {\n",           // 10
        "            $item->\n",                               // 11
        "        }\n",                                         // 12
        "    }\n",                                             // 13
        "}\n",                                                 // 14
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 11,
                    character: 19,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"next"),
                "Should include Node's 'next' inside while body, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getValue"),
                "Should include Node's 'getValue' inside while body, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"leafOnly"),
                "Should NOT include Leaf's 'leafOnly' inside while body, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Negated instanceof in a while condition should exclude the class.
#[tokio::test]
async fn test_completion_while_negated_instanceof_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///while_neg_instanceof.php").unwrap();
    let text = concat!(
        "<?php\n",                                           // 0
        "class Alpha {\n",                                   // 1
        "    public function alphaMethod(): void {}\n",      // 2
        "}\n",                                               // 3
        "class Beta {\n",                                    // 4
        "    public function betaMethod(): void {}\n",       // 5
        "}\n",                                               // 6
        "class Svc {\n",                                     // 7
        "    public function test(Alpha|Beta $x): void {\n", // 8
        "        while (!($x instanceof Alpha)) {\n",        // 9
        "            $x->\n",                                // 10
        "        }\n",                                       // 11
        "    }\n",                                           // 12
        "}\n",                                               // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"betaMethod"),
                "Should include Beta's 'betaMethod' when Alpha is excluded, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"alphaMethod"),
                "Should NOT include Alpha's 'alphaMethod' when Alpha is excluded, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── is_a() narrowing ──────────────────────────────────────────────────────

/// `if (is_a($var, Foo::class))` should narrow like instanceof.
#[tokio::test]
async fn test_completion_is_a_narrows_like_instanceof() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///is_a_narrow.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Dog {\n",                                    // 1
        "    public function bark(): void {}\n",            // 2
        "}\n",                                              // 3
        "class Cat {\n",                                    // 4
        "    public function purr(): void {}\n",            // 5
        "}\n",                                              // 6
        "class Svc {\n",                                    // 7
        "    public function test(Dog|Cat $pet): void {\n", // 8
        "        if (is_a($pet, Dog::class)) {\n",          // 9
        "            $pet->\n",                             // 10
        "        }\n",                                      // 11
        "    }\n",                                          // 12
        "}\n",                                              // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' inside is_a block, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' inside is_a block, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if (!is_a($var, Foo::class))` should exclude Foo (negated is_a).
#[tokio::test]
async fn test_completion_negated_is_a_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///is_a_neg.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Dog {\n",                                    // 1
        "    public function bark(): void {}\n",            // 2
        "}\n",                                              // 3
        "class Cat {\n",                                    // 4
        "    public function purr(): void {}\n",            // 5
        "}\n",                                              // 6
        "class Svc {\n",                                    // 7
        "    public function test(Dog|Cat $pet): void {\n", // 8
        "        if (!is_a($pet, Dog::class)) {\n",         // 9
        "            $pet->\n",                             // 10
        "        }\n",                                      // 11
        "    }\n",                                          // 12
        "}\n",                                              // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"purr"),
                "Should include Cat's 'purr' when Dog is excluded, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bark"),
                "Should NOT include Dog's 'bark' when Dog is excluded, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `is_a()` else branch should invert narrowing (exclude the matched class).
#[tokio::test]
async fn test_completion_is_a_else_branch_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///is_a_else.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Dog {\n",                                    // 1
        "    public function bark(): void {}\n",            // 2
        "}\n",                                              // 3
        "class Cat {\n",                                    // 4
        "    public function purr(): void {}\n",            // 5
        "}\n",                                              // 6
        "class Svc {\n",                                    // 7
        "    public function test(Dog|Cat $pet): void {\n", // 8
        "        if (is_a($pet, Dog::class)) {\n",          // 9
        "            // dog branch\n",                      // 10
        "        } else {\n",                               // 11
        "            $pet->\n",                             // 12
        "        }\n",                                      // 13
        "    }\n",                                          // 14
        "}\n",                                              // 15
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"purr"),
                "Should include Cat's 'purr' in else branch (Dog excluded), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bark"),
                "Should NOT include Dog's 'bark' in else branch, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── get_class() / $var::class narrowing ────────────────────────────────────

/// `if (get_class($var) === Foo::class)` should narrow to exactly Foo.
#[tokio::test]
async fn test_completion_get_class_identical_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///get_class_narrow.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser extends User {\n",                      // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $u): void {\n", // 8
        "        if (get_class($u) === User::class) {\n",        // 9
        "            $u->\n",                                    // 10
        "        }\n",                                           // 11
        "    }\n",                                               // 12
        "}\n",                                                   // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' when get_class matches User, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' when get_class matches User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if ($var::class === Foo::class)` should narrow to exactly Foo.
#[tokio::test]
async fn test_completion_var_class_constant_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_class_narrow.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser extends User {\n",                      // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $u): void {\n", // 8
        "        if ($u::class === User::class) {\n",            // 9
        "            $u->\n",                                    // 10
        "        }\n",                                           // 11
        "    }\n",                                               // 12
        "}\n",                                                   // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' when $u::class === User::class, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' when $u::class === User::class, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `if (get_class($var) !== Foo::class)` should exclude Foo (negated identity).
#[tokio::test]
async fn test_completion_get_class_not_identical_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///get_class_neg.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $u): void {\n", // 8
        "        if (get_class($u) !== User::class) {\n",        // 9
        "            $u->\n",                                    // 10
        "        }\n",                                           // 11
        "    }\n",                                               // 12
        "}\n",                                                   // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addRoles"),
                "Should include AdminUser's 'addRoles' when User is excluded, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getName"),
                "Should NOT include User's 'getName' when User is excluded, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `get_class($var) === Foo::class` else branch should exclude Foo.
#[tokio::test]
async fn test_completion_get_class_else_branch_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///get_class_else.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $u): void {\n", // 8
        "        if (get_class($u) === User::class) {\n",        // 9
        "            // user branch\n",                          // 10
        "        } else {\n",                                    // 11
        "            $u->\n",                                    // 12
        "        }\n",                                           // 13
        "    }\n",                                               // 14
        "}\n",                                                   // 15
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addRoles"),
                "Should include AdminUser's 'addRoles' in else branch, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getName"),
                "Should NOT include User's 'getName' in else branch (get_class matched), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Reversed order: `Foo::class === get_class($var)` should also narrow.
#[tokio::test]
async fn test_completion_get_class_reversed_order_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///get_class_reversed.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $u): void {\n", // 8
        "        if (User::class === get_class($u)) {\n",        // 9
        "            $u->\n",                                    // 10
        "        }\n",                                           // 11
        "    }\n",                                               // 12
        "}\n",                                                   // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' with reversed order, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' with reversed order, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── match(true) instanceof narrowing ───────────────────────────────────────

/// Inside a `match (true) { $var instanceof Foo => … }` arm body,
/// the variable should be narrowed to Foo.
#[tokio::test]
async fn test_completion_match_true_instanceof_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///match_true_narrow.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $v): void {\n", // 8
        "        $result = match (true) {\n",                    // 9
        "            $v instanceof AdminUser => $v->\n",         // 10
        "            default => null,\n",                        // 11
        "        };\n",                                          // 12
        "    }\n",                                               // 13
        "}\n",                                                   // 14
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 49,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addRoles"),
                "Should include AdminUser's 'addRoles' in match arm, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getName"),
                "Should NOT include User's 'getName' in match arm, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `match (true)` with is_a() in arm condition should also narrow.
#[tokio::test]
async fn test_completion_match_true_is_a_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///match_true_is_a.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Dog {\n",                                    // 1
        "    public function bark(): void {}\n",            // 2
        "}\n",                                              // 3
        "class Cat {\n",                                    // 4
        "    public function purr(): void {}\n",            // 5
        "}\n",                                              // 6
        "class Svc {\n",                                    // 7
        "    public function test(Dog|Cat $pet): void {\n", // 8
        "        $result = match (true) {\n",               // 9
        "            is_a($pet, Cat::class) => $pet->\n",   // 10
        "            default => null,\n",                   // 11
        "        };\n",                                     // 12
        "    }\n",                                          // 13
        "}\n",                                              // 14
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 50,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"purr"),
                "Should include Cat's 'purr' in match arm with is_a, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bark"),
                "Should NOT include Dog's 'bark' in match arm with is_a, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `assert(is_a($var, Foo::class))` should narrow unconditionally.
#[tokio::test]
async fn test_completion_assert_is_a_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_is_a.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Dog {\n",                                    // 1
        "    public function bark(): void {}\n",            // 2
        "}\n",                                              // 3
        "class Cat {\n",                                    // 4
        "    public function purr(): void {}\n",            // 5
        "}\n",                                              // 6
        "class Svc {\n",                                    // 7
        "    public function test(Dog|Cat $pet): void {\n", // 8
        "        assert(is_a($pet, Dog::class));\n",        // 9
        "        $pet->\n",                                 // 10
        "    }\n",                                          // 11
        "}\n",                                              // 12
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after assert(is_a()), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after assert(is_a()), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `while` narrowing with `is_a()` should also work.
#[tokio::test]
async fn test_completion_while_is_a_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///while_is_a.php").unwrap();
    let text = concat!(
        "<?php\n",                                             // 0
        "class Node {\n",                                      // 1
        "    public function next(): ?Node {}\n",              // 2
        "    public function getValue(): string {}\n",         // 3
        "}\n",                                                 // 4
        "class Leaf {\n",                                      // 5
        "    public function leafOnly(): void {}\n",           // 6
        "}\n",                                                 // 7
        "class Svc {\n",                                       // 8
        "    public function walk(Node|Leaf $item): void {\n", // 9
        "        while (is_a($item, Node::class)) {\n",        // 10
        "            $item->\n",                               // 11
        "        }\n",                                         // 12
        "    }\n",                                             // 13
        "}\n",                                                 // 14
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 11,
                    character: 19,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"next"),
                "Should include Node's 'next' inside while body with is_a, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"leafOnly"),
                "Should NOT include Leaf's 'leafOnly' inside while body with is_a, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `$var::class === Foo::class` else branch should exclude Foo.
#[tokio::test]
async fn test_completion_var_class_constant_else_branch_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_class_else.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $u): void {\n", // 8
        "        if ($u::class === User::class) {\n",            // 9
        "            // user branch\n",                          // 10
        "        } else {\n",                                    // 11
        "            $u->\n",                                    // 12
        "        }\n",                                           // 13
        "    }\n",                                               // 14
        "}\n",                                                   // 15
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addRoles"),
                "Should include AdminUser's 'addRoles' in else branch, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getName"),
                "Should NOT include User's 'getName' in else branch ($u::class matched), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `get_class()` narrowing with `==` (loose equality) should also work.
#[tokio::test]
async fn test_completion_get_class_loose_equality_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///get_class_eq.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "class Svc {\n",                                         // 7
        "    public function test(User|AdminUser $u): void {\n", // 8
        "        if (get_class($u) == User::class) {\n",         // 9
        "            $u->\n",                                    // 10
        "        }\n",                                           // 11
        "    }\n",                                               // 12
        "}\n",                                                   // 13
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' with loose == comparison, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' with loose == comparison, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── @phpstan-assert / @psalm-assert narrowing ──────────────────────────────

/// `@phpstan-assert User $value` on a standalone function call should narrow
/// the variable unconditionally after the call.
#[tokio::test]
async fn test_completion_phpstan_assert_narrows_unconditionally() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///phpstan_assert_basic.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert User $value\n",                      // 8
        " */\n",                                                 // 9
        "function assertUser($value): void {}\n",                // 10
        "class Svc {\n",                                         // 11
        "    public function test(User|AdminUser $v): void {\n", // 12
        "        assertUser($v);\n",                             // 13
        "        $v->\n",                                        // 14
        "    }\n",                                               // 15
        "}\n",                                                   // 16
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' after assertUser(), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' after assertUser(), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Negated `@phpstan-assert !User $value` should exclude User.
#[tokio::test]
async fn test_completion_phpstan_assert_negated_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///phpstan_assert_neg.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert !User $value\n",                     // 8
        " */\n",                                                 // 9
        "function assertNotUser($value): void {}\n",             // 10
        "class Svc {\n",                                         // 11
        "    public function test(User|AdminUser $v): void {\n", // 12
        "        assertNotUser($v);\n",                          // 13
        "        $v->\n",                                        // 14
        "    }\n",                                               // 15
        "}\n",                                                   // 16
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addRoles"),
                "Should include AdminUser's 'addRoles' after assertNotUser(), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getName"),
                "Should NOT include User's 'getName' after assertNotUser(), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `@psalm-assert` should work identically to `@phpstan-assert`.
#[tokio::test]
async fn test_completion_psalm_assert_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///psalm_assert.php").unwrap();
    let text = concat!(
        "<?php\n",                                        // 0
        "class Dog {\n",                                  // 1
        "    public function bark(): void {}\n",          // 2
        "}\n",                                            // 3
        "class Cat {\n",                                  // 4
        "    public function purr(): void {}\n",          // 5
        "}\n",                                            // 6
        "/**\n",                                          // 7
        " * @psalm-assert Dog $animal\n",                 // 8
        " */\n",                                          // 9
        "function assertDog($animal): void {}\n",         // 10
        "class Svc {\n",                                  // 11
        "    public function test(Dog|Cat $a): void {\n", // 12
        "        assertDog($a);\n",                       // 13
        "        $a->\n",                                 // 14
        "    }\n",                                        // 15
        "}\n",                                            // 16
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after assertDog() with @psalm-assert, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after assertDog() with @psalm-assert, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── @phpstan-assert-if-true narrowing ──────────────────────────────────────

/// `@phpstan-assert-if-true User $value` should narrow inside the if-body
/// when the function is used as a condition.
#[tokio::test]
async fn test_completion_phpstan_assert_if_true_narrows_in_then() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_if_true_then.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert-if-true User $value\n",              // 8
        " */\n",                                                 // 9
        "function isUser($value): bool {}\n",                    // 10
        "class Svc {\n",                                         // 11
        "    public function test(User|AdminUser $v): void {\n", // 12
        "        if (isUser($v)) {\n",                           // 13
        "            $v->\n",                                    // 14
        "        }\n",                                           // 15
        "    }\n",                                               // 16
        "}\n",                                                   // 17
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' in if-body with assert-if-true, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' in if-body with assert-if-true, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `@phpstan-assert-if-true User $value` in the else-body means the function
/// returned false, so $value is NOT User → exclude User from the union.
#[tokio::test]
async fn test_completion_phpstan_assert_if_true_excludes_in_else() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_if_true_else.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert-if-true User $value\n",              // 8
        " */\n",                                                 // 9
        "function isUser($value): bool {}\n",                    // 10
        "class Svc {\n",                                         // 11
        "    public function test(User|AdminUser $v): void {\n", // 12
        "        if (isUser($v)) {\n",                           // 13
        "            // then branch\n",                          // 14
        "        } else {\n",                                    // 15
        "            $v->\n",                                    // 16
        "        }\n",                                           // 17
        "    }\n",                                               // 18
        "}\n",                                                   // 19
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

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // In the else branch, function returned false → User is excluded
            // from User|AdminUser, leaving only AdminUser.
            assert!(
                method_names.contains(&"addRoles"),
                "Should include AdminUser's 'addRoles' in else branch (User excluded), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getName"),
                "Should NOT include User's 'getName' in else branch (User excluded), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Negated condition `if (!isUser($v))` with `@phpstan-assert-if-true`
/// should NOT narrow in the then-body (function returned false).
/// But the else-body should narrow (function returned true).
#[tokio::test]
async fn test_completion_phpstan_assert_if_true_negated_narrows_in_else() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_if_true_neg.php").unwrap();
    let text = concat!(
        "<?php\n",                                                // 0
        "class User {\n",                                         // 1
        "    public function getName(): string {}\n",             // 2
        "}\n",                                                    // 3
        "class AdminUser {\n",                                    // 4
        "    public function addRoles(string $r): void {}\n",     // 5
        "}\n",                                                    // 6
        "/**\n",                                                  // 7
        " * @phpstan-assert-if-true User $value\n",               // 8
        " */\n",                                                  // 9
        "function isUser($value): bool {}\n",                     // 10
        "class Svc {\n",                                          // 11
        "    public function test(User|AdminUser $v): void {\n",  // 12
        "        if (!isUser($v)) {\n",                           // 13
        "            // negated then: function returned false\n", // 14
        "        } else {\n",                                     // 15
        "            $v->\n",                                     // 16
        "        }\n",                                            // 17
        "    }\n",                                                // 18
        "}\n",                                                    // 19
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

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // `!isUser($v)` in then means func returned false.
            // In else, func returned true → IfTrue applies → narrow to User.
            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' in else of negated assert-if-true, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' in else of negated assert-if-true, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── @phpstan-assert-if-false narrowing ─────────────────────────────────────

/// `@phpstan-assert-if-false User $value` should narrow in the else-body
/// (function returned false → assertion holds).
#[tokio::test]
async fn test_completion_phpstan_assert_if_false_narrows_in_else() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_if_false_else.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert-if-false User $value\n",             // 8
        " */\n",                                                 // 9
        "function isNotUser($value): bool {}\n",                 // 10
        "class Svc {\n",                                         // 11
        "    public function test(User|AdminUser $v): void {\n", // 12
        "        if (isNotUser($v)) {\n",                        // 13
        "            // then branch: function returned true\n",  // 14
        "        } else {\n",                                    // 15
        "            $v->\n",                                    // 16
        "        }\n",                                           // 17
        "    }\n",                                               // 18
        "}\n",                                                   // 19
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

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Else body: function returned false → IfFalse assertion applies
            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' in else with assert-if-false, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' in else with assert-if-false, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `@phpstan-assert-if-false User $value` in the then-body means the function
/// returned true, so $value is NOT User → exclude User from the union.
#[tokio::test]
async fn test_completion_phpstan_assert_if_false_excludes_in_then() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_if_false_then.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert-if-false User $value\n",             // 8
        " */\n",                                                 // 9
        "function isNotUser($value): bool {}\n",                 // 10
        "class Svc {\n",                                         // 11
        "    public function test(User|AdminUser $v): void {\n", // 12
        "        if (isNotUser($v)) {\n",                        // 13
        "            $v->\n",                                    // 14
        "        }\n",                                           // 15
        "    }\n",                                               // 16
        "}\n",                                                   // 17
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Then body: function returned true → User is excluded
            // from User|AdminUser, leaving only AdminUser.
            assert!(
                method_names.contains(&"addRoles"),
                "Should include AdminUser's 'addRoles' in then branch (User excluded), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getName"),
                "Should NOT include User's 'getName' in then branch (User excluded), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `@phpstan-assert-if-true` in a while-loop condition should narrow
/// inside the loop body.
#[tokio::test]
async fn test_completion_phpstan_assert_if_true_in_while() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_if_true_while.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Node {\n",                                   // 1
        "    public function next(): ?Node {}\n",           // 2
        "    public function getValue(): string {}\n",      // 3
        "}\n",                                              // 4
        "class Leaf {\n",                                   // 5
        "    public function leafOnly(): void {}\n",        // 6
        "}\n",                                              // 7
        "/**\n",                                            // 8
        " * @phpstan-assert-if-true Node $item\n",          // 9
        " */\n",                                            // 10
        "function isNode($item): bool {}\n",                // 11
        "class Svc {\n",                                    // 12
        "    public function walk(Node|Leaf $n): void {\n", // 13
        "        while (isNode($n)) {\n",                   // 14
        "            $n->\n",                               // 15
        "        }\n",                                      // 16
        "    }\n",                                          // 17
        "}\n",                                              // 18
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getValue"),
                "Should include Node's 'getValue' in while body with assert-if-true, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"leafOnly"),
                "Should NOT include Leaf's 'leafOnly' in while body with assert-if-true, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `@phpstan-assert` on the second parameter should narrow the correct variable.
#[tokio::test]
async fn test_completion_phpstan_assert_second_parameter() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///phpstan_assert_second.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert User $obj\n",                        // 8
        " */\n",                                                 // 9
        "function assertType(string $class, $obj): void {}\n",   // 10
        "class Svc {\n",                                         // 11
        "    public function test(User|AdminUser $v): void {\n", // 12
        "        assertType(User::class, $v);\n",                // 13
        "        $v->\n",                                        // 14
        "    }\n",                                               // 15
        "}\n",                                                   // 16
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should include User's 'getName' (assert on second param), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"addRoles"),
                "Should NOT include AdminUser's 'addRoles' (assert on second param), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `@phpstan-assert-if-true` + `@phpstan-assert-if-false` on the same
/// function: each applies in the correct branch.
#[tokio::test]
async fn test_completion_phpstan_assert_if_true_and_false_combined() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///assert_combined.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class User {\n",                                        // 1
        "    public function getName(): string {}\n",            // 2
        "}\n",                                                   // 3
        "class AdminUser {\n",                                   // 4
        "    public function addRoles(string $r): void {}\n",    // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert-if-true User $value\n",              // 8
        " * @phpstan-assert-if-false AdminUser $value\n",        // 9
        " */\n",                                                 // 10
        "function isUser($value): bool {}\n",                    // 11
        "class Svc {\n",                                         // 12
        "    public function test(User|AdminUser $v): void {\n", // 13
        "        if (isUser($v)) {\n",                           // 14
        "            $v->\n",                                    // 15
        "        } else {\n",                                    // 16
        "            $v->\n",                                    // 17
        "        }\n",                                           // 18
        "    }\n",                                               // 19
        "}\n",                                                   // 20
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

    // Test then-body: IfTrue → User
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 15,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions for then-body");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Then-body should include User's 'getName' (assert-if-true), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }

    // Test else-body: IfFalse → AdminUser
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 17,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions for else-body");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addRoles"),
                "Else-body should include AdminUser's 'addRoles' (assert-if-false), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Top-level code (outside class) should also support `@phpstan-assert`.
#[tokio::test]
async fn test_completion_phpstan_assert_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///phpstan_assert_top.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "class Dog {\n",                                         // 1
        "    public function bark(): void {}\n",                 // 2
        "}\n",                                                   // 3
        "class Cat {\n",                                         // 4
        "    public function purr(): void {}\n",                 // 5
        "}\n",                                                   // 6
        "/**\n",                                                 // 7
        " * @phpstan-assert Dog $val\n",                         // 8
        " */\n",                                                 // 9
        "function assertDog($val): void {}\n",                   // 10
        "\n",                                                    // 11
        "function getAnimal(): Dog|Cat { return new Dog(); }\n", // 12
        "$animal = getAnimal();\n",                              // 13
        "assertDog($animal);\n",                                 // 14
        "$animal->\n",                                           // 15
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

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 10,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' at top level after assertDog(), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' at top level after assertDog(), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
