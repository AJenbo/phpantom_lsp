mod common;

use common::create_test_backend;
use phpantom_lsp::{AccessKind, Backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Access Kind Detection ──────────────────────────────────────────────────

#[tokio::test]
async fn test_detect_access_kind_arrow() {
    assert_eq!(
        Backend::detect_access_kind(
            "$this->",
            Position {
                line: 0,
                character: 7
            }
        ),
        AccessKind::Arrow
    );
}

#[tokio::test]
async fn test_detect_access_kind_arrow_with_partial_identifier() {
    assert_eq!(
        Backend::detect_access_kind(
            "$this->get",
            Position {
                line: 0,
                character: 10
            }
        ),
        AccessKind::Arrow
    );
}

#[tokio::test]
async fn test_detect_access_kind_double_colon() {
    assert_eq!(
        Backend::detect_access_kind(
            "self::",
            Position {
                line: 0,
                character: 6
            }
        ),
        AccessKind::DoubleColon
    );
}

#[tokio::test]
async fn test_detect_access_kind_double_colon_with_partial_identifier() {
    assert_eq!(
        Backend::detect_access_kind(
            "Foo::cr",
            Position {
                line: 0,
                character: 7
            }
        ),
        AccessKind::DoubleColon
    );
}

#[tokio::test]
async fn test_detect_access_kind_other() {
    assert_eq!(
        Backend::detect_access_kind(
            "    $x = 1;",
            Position {
                line: 0,
                character: 4
            }
        ),
        AccessKind::Other
    );
}

#[tokio::test]
async fn test_detect_access_kind_multiline() {
    let content = "<?php\n$obj->meth";
    assert_eq!(
        Backend::detect_access_kind(
            content,
            Position {
                line: 1,
                character: 10
            }
        ),
        AccessKind::Arrow
    );
}

// ─── Arrow vs Double-Colon Filtering ────────────────────────────────────────

#[tokio::test]
async fn test_completion_arrow_shows_only_non_static() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///arrow.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public static function create(): self {}\n",
        "    public function run(): void {}\n",
        "    public static string $instance = '';\n",
        "    public int $count = 0;\n",
        "    const MAX = 10;\n",
        "    function helper() {\n",
        "        $this->\n",
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

    // Cursor right after `$this->` on line 8
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let property_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            // Should include non-static method `run` and `helper`
            assert!(
                method_names.contains(&"run"),
                "Arrow should include non-static method 'run'"
            );
            assert!(
                method_names.contains(&"helper"),
                "Arrow should include non-static method 'helper'"
            );
            // Should NOT include static method `create`
            assert!(
                !method_names.contains(&"create"),
                "Arrow should exclude static method 'create'"
            );

            // Should include non-static property `count`
            assert!(
                property_names.contains(&"count"),
                "Arrow should include non-static property 'count'"
            );
            // Should NOT include static property `instance`
            assert!(
                !property_names.contains(&"instance"),
                "Arrow should exclude static property 'instance'"
            );

            // Should NOT include constants
            assert!(
                constant_names.is_empty(),
                "Arrow should not include constants, got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_double_colon_shows_only_static_and_constants() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///dcolon.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public static function create(): self {}\n",
        "    public function run(): void {}\n",
        "    public static string $instance = '';\n",
        "    public int $count = 0;\n",
        "    const MAX = 10;\n",
        "    function helper() {\n",
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

    // Cursor right after `self::` on line 8
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
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let property_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            // Should include static method `create`
            assert!(
                method_names.contains(&"create"),
                "DoubleColon should include static method 'create'"
            );
            // Should NOT include non-static methods `run` and `helper`
            assert!(
                !method_names.contains(&"run"),
                "DoubleColon should exclude non-static method 'run'"
            );
            assert!(
                !method_names.contains(&"helper"),
                "DoubleColon should exclude non-static method 'helper'"
            );

            // Should include static property `$instance` (with $ prefix for :: access)
            assert!(
                property_names.contains(&"$instance"),
                "DoubleColon should include static property '$instance'"
            );
            // Should NOT include non-static property `count` (or `$count`)
            assert!(
                !property_names.contains(&"count") && !property_names.contains(&"$count"),
                "DoubleColon should exclude non-static property 'count'"
            );

            // Should include constant `MAX`
            assert!(
                constant_names.contains(&"MAX"),
                "DoubleColon should include constant 'MAX'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_arrow_with_partial_typed_identifier() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///partial.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Obj {\n",
        "    public static function staticMethod(): void {}\n",
        "    public function instanceMethod(): void {}\n",
        "    function test() {\n",
        "        $this->inst\n",
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

    // Cursor after `$this->inst` on line 5 — partial identifier typed after ->
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 19,
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
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Should include non-static method
            assert!(
                method_names.contains(&"instanceMethod"),
                "Should include instanceMethod"
            );
            assert!(method_names.contains(&"test"), "Should include test");
            // Should NOT include static method even with partial typing
            assert!(
                !method_names.contains(&"staticMethod"),
                "Should exclude staticMethod when using ->"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
