mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── parent:: completion tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_completion_parent_double_colon_shows_static_and_instance() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///parent_basic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Animal {\n",
        "    public function breathe(): void {}\n",
        "    public static function kingdom(): string { return 'Animalia'; }\n",
        "}\n",
        "class Dog extends Animal {\n",
        "    function test() {\n",
        "        parent::\n",
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
                line: 7,
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
        "parent:: should return completion results"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // parent:: shows BOTH static and non-static methods
            assert!(
                method_names.contains(&"breathe"),
                "parent:: should include non-static 'breathe', got {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"kingdom"),
                "parent:: should include static 'kingdom', got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_parent_double_colon_excludes_private() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///parent_vis.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function pubMethod(): void {}\n",
        "    protected function protMethod(): void {}\n",
        "    private function privMethod(): void {}\n",
        "    public string $pubProp;\n",
        "    protected string $protProp;\n",
        "    private string $privProp;\n",
        "    public static string $pubStaticProp = '';\n",
        "    protected static string $protStaticProp = '';\n",
        "    private static string $privStaticProp = '';\n",
        "    public const PUB_CONST = 1;\n",
        "    protected const PROT_CONST = 2;\n",
        "    private const PRIV_CONST = 3;\n",
        "}\n",
        "class Child extends Base {\n",
        "    function test() {\n",
        "        parent::\n",
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
                line: 17,
                character: 16,
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
            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            // Methods: public and protected included, private excluded
            assert!(
                method_names.contains(&"pubMethod"),
                "Should include public method"
            );
            assert!(
                method_names.contains(&"protMethod"),
                "Should include protected method"
            );
            assert!(
                !method_names.contains(&"privMethod"),
                "Should NOT include private method"
            );

            // Properties: only static properties shown (parent:: uses :: syntax),
            // public and protected included, private excluded
            assert!(
                prop_names.contains(&"$pubStaticProp"),
                "Should include public static property"
            );
            assert!(
                prop_names.contains(&"$protStaticProp"),
                "Should include protected static property"
            );
            assert!(
                !prop_names.contains(&"$privStaticProp"),
                "Should NOT include private static property"
            );
            // Non-static properties should not appear via ::
            assert!(
                !prop_names.contains(&"pubProp"),
                "Should NOT include non-static property via ::"
            );
            assert!(
                !prop_names.contains(&"$pubProp"),
                "Should NOT include non-static property via ::"
            );

            // Constants: public and protected included, private excluded
            assert!(
                const_names.contains(&"PUB_CONST"),
                "Should include public constant"
            );
            assert!(
                const_names.contains(&"PROT_CONST"),
                "Should include protected constant"
            );
            assert!(
                !const_names.contains(&"PRIV_CONST"),
                "Should NOT include private constant"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_parent_double_colon_includes_constants() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///parent_const.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    const VERSION = '1.0';\n",
        "    const APP_NAME = 'MyApp';\n",
        "}\n",
        "class AppConfig extends Config {\n",
        "    function test() {\n",
        "        parent::\n",
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
                line: 7,
                character: 16,
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
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            assert!(
                const_names.contains(&"VERSION"),
                "Should include constant 'VERSION'"
            );
            assert!(
                const_names.contains(&"APP_NAME"),
                "Should include constant 'APP_NAME'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_parent_double_colon_cross_file_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/BaseService.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "class BaseService {\n",
                "    public function init(): void {}\n",
                "    public static function create(): self { return new self(); }\n",
                "    protected function configure(): void {}\n",
                "    private function internalSetup(): void {}\n",
                "    const SERVICE_VERSION = '2.0';\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\BaseService;\n",
        "class MyService extends BaseService {\n",
        "    function test() {\n",
        "        parent::\n",
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
        "parent:: should resolve cross-file via PSR-4"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            // Both static and instance methods
            assert!(
                method_names.contains(&"init"),
                "Should include non-static 'init'"
            );
            assert!(
                method_names.contains(&"create"),
                "Should include static 'create'"
            );
            assert!(
                method_names.contains(&"configure"),
                "Should include protected 'configure'"
            );
            assert!(
                !method_names.contains(&"internalSetup"),
                "Should NOT include private 'internalSetup'"
            );

            // Constants
            assert!(
                const_names.contains(&"SERVICE_VERSION"),
                "Should include constant"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_parent_double_colon_with_grandparent() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///parent_grandparent.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Grandparent {\n",
        "    public function ancestorMethod(): void {}\n",
        "    protected function ancestorProtected(): void {}\n",
        "    private function ancestorPrivate(): void {}\n",
        "}\n",
        "class ParentClass extends Grandparent {\n",
        "    public function parentMethod(): void {}\n",
        "}\n",
        "class ChildClass extends ParentClass {\n",
        "    function test() {\n",
        "        parent::\n",
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
                line: 11,
                character: 16,
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

            // ParentClass's own method
            assert!(
                method_names.contains(&"parentMethod"),
                "Should include parent's own 'parentMethod'"
            );

            // Grandparent's public and protected methods (inherited into ParentClass)
            assert!(
                method_names.contains(&"ancestorMethod"),
                "Should include grandparent's 'ancestorMethod'"
            );
            assert!(
                method_names.contains(&"ancestorProtected"),
                "Should include grandparent's protected 'ancestorProtected'"
            );

            // Grandparent's private should NOT appear
            assert!(
                !method_names.contains(&"ancestorPrivate"),
                "Should NOT include grandparent's private 'ancestorPrivate'"
            );

            // Child's own methods should NOT appear (parent:: is the parent, not self)
            assert!(
                !method_names.contains(&"test"),
                "Should NOT include child's own 'test'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_parent_double_colon_no_parent_falls_back() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///parent_none.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Standalone {\n",
        "    function test() {\n",
        "        parent::\n",
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
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    // Should fall back to the default PHPantomLSP item since there's no parent
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(items.len(), 1, "Should fall back to default completion");
            assert_eq!(items[0].label, "PHPantomLSP");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_parent_double_colon_magic_methods_excluded() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///parent_magic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function __construct() {}\n",
        "    public function __toString(): string { return ''; }\n",
        "    public function realMethod(): void {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    function test() {\n",
        "        parent::\n",
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
                character: 16,
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

            assert!(
                !method_names.contains(&"__construct"),
                "Magic methods should be filtered from parent::"
            );
            assert!(
                !method_names.contains(&"__toString"),
                "Magic methods should be filtered from parent::"
            );
            assert!(
                method_names.contains(&"realMethod"),
                "Non-magic method should appear via parent::"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
