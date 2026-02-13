mod common;

use common::create_test_backend;
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
