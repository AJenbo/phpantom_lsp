mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Basic trait usage (same file) ──────────────────────────────────────────

#[tokio::test]
async fn test_completion_trait_methods_available_on_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_basic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Greetable {\n",
        "    public function greet(): string { return 'hi'; }\n",
        "    protected function farewell(): string { return 'bye'; }\n",
        "}\n",
        "class Person {\n",
        "    use Greetable;\n",
        "    public function name(): string { return 'Alice'; }\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 9,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Completion should return results");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"greet"),
                "Should include trait method 'greet', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"farewell"),
                "Should include trait protected method 'farewell', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"name"),
                "Should include own method 'name', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait properties ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_trait_properties_available_on_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_props.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait HasTimestamps {\n",
        "    public string $created_at;\n",
        "    protected string $updated_at;\n",
        "    private string $internal_ts;\n",
        "}\n",
        "class Post {\n",
        "    use HasTimestamps;\n",
        "    public string $title;\n",
        "    function test() {\n",
        "        $this->\n",
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

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                prop_names.contains(&"title"),
                "Should include own property 'title', got: {:?}",
                prop_names
            );
            assert!(
                prop_names.contains(&"created_at"),
                "Should include trait property 'created_at', got: {:?}",
                prop_names
            );
            assert!(
                prop_names.contains(&"updated_at"),
                "Should include trait protected property 'updated_at', got: {:?}",
                prop_names
            );
            // Private trait members ARE included (trait is copy-paste semantics)
            assert!(
                prop_names.contains(&"internal_ts"),
                "Should include trait private property 'internal_ts', got: {:?}",
                prop_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait constants ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_trait_constants_available_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_const.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait HasVersion {\n",
        "    public const VERSION = '1.0';\n",
        "}\n",
        "class App {\n",
        "    use HasVersion;\n",
        "    public const NAME = 'MyApp';\n",
        "    function test() {\n",
        "        self::\n",
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
                    line: 8,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                const_names.contains(&"NAME"),
                "Should include own constant 'NAME', got: {:?}",
                const_names
            );
            assert!(
                const_names.contains(&"VERSION"),
                "Should include trait constant 'VERSION', got: {:?}",
                const_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Multiple traits ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_multiple_traits_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///multi_traits.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Loggable {\n",
        "    public function log(): void {}\n",
        "}\n",
        "trait Cacheable {\n",
        "    public function cache(): void {}\n",
        "}\n",
        "class Service {\n",
        "    use Loggable, Cacheable;\n",
        "    public function run(): void {}\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 11,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"log"),
                "Should include Loggable::log, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"cache"),
                "Should include Cacheable::cache, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"run"),
                "Should include own method 'run', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait composed from other traits ───────────────────────────────────────

#[tokio::test]
async fn test_completion_nested_trait_composition() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///nested_traits.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Hello {\n",
        "    public function sayHello(): string { return 'Hello'; }\n",
        "}\n",
        "trait World {\n",
        "    public function sayWorld(): string { return 'World'; }\n",
        "}\n",
        "trait HelloWorld {\n",
        "    use Hello, World;\n",
        "}\n",
        "class Greeter {\n",
        "    use HelloWorld;\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 13,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"sayHello"),
                "Should include Hello::sayHello via nested trait, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"sayWorld"),
                "Should include World::sayWorld via nested trait, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Class method overrides trait method ────────────────────────────────────

#[tokio::test]
async fn test_completion_class_method_overrides_trait_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_override.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Renderable {\n",
        "    public function render(): string { return 'trait'; }\n",
        "    public function format(): string { return 'format'; }\n",
        "}\n",
        "class View {\n",
        "    use Renderable;\n",
        "    public function render(): string { return 'class'; }\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 9,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();

            let render_items: Vec<&&CompletionItem> = method_items
                .iter()
                .filter(|i| i.filter_text.as_deref() == Some("render"))
                .collect();

            // There should be exactly one 'render' — the class's version wins
            assert_eq!(
                render_items.len(),
                1,
                "Should have exactly one 'render' method (class override), got: {}",
                render_items.len()
            );

            // Trait-only method should still be present
            let format_items: Vec<&&CompletionItem> = method_items
                .iter()
                .filter(|i| i.filter_text.as_deref() == Some("format"))
                .collect();
            assert_eq!(
                format_items.len(),
                1,
                "Should include trait-only method 'format'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait + parent class inheritance (PHP precedence) ───────────────────────

#[tokio::test]
async fn test_completion_trait_overrides_parent_class_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_vs_parent.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function hello(): string { return 'base'; }\n",
        "    public function baseOnly(): void {}\n",
        "}\n",
        "trait SayWorld {\n",
        "    public function hello(): string { return 'trait'; }\n",
        "    public function traitOnly(): void {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    use SayWorld;\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 12,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Trait method should be present (overrides parent)
            assert!(
                method_names.contains(&"hello"),
                "Should include 'hello' (from trait), got: {:?}",
                method_names
            );
            // Trait-only method
            assert!(
                method_names.contains(&"traitOnly"),
                "Should include trait-only method, got: {:?}",
                method_names
            );
            // Parent-only method
            assert!(
                method_names.contains(&"baseOnly"),
                "Should include parent-only method, got: {:?}",
                method_names
            );
            // Own method
            assert!(
                method_names.contains(&"test"),
                "Should include own method 'test', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Cross-file trait usage with PSR-4 ──────────────────────────────────────

#[tokio::test]
async fn test_completion_trait_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let trait_php = concat!(
        "<?php\n",
        "namespace App\\Traits;\n",
        "trait Auditable {\n",
        "    public function getAuditLog(): array { return []; }\n",
        "    public function setAuditor(string $name): void {}\n",
        "}\n",
    );

    let class_php = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "use App\\Traits\\Auditable;\n",
        "class User {\n",
        "    use Auditable;\n",
        "    public string $name;\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Traits/Auditable.php", trait_php),
            ("src/Models/User.php", class_php),
        ],
    );

    let uri = Url::parse("file:///test_cross_trait.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: class_php.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getAuditLog"),
                "Should include cross-file trait method 'getAuditLog', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"setAuditor"),
                "Should include cross-file trait method 'setAuditor', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Cross-file nested trait composition with PSR-4 ─────────────────────────

#[tokio::test]
async fn test_completion_nested_trait_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let trait_a = concat!(
        "<?php\n",
        "namespace App\\Traits;\n",
        "trait Timestamps {\n",
        "    public function getCreatedAt(): string { return ''; }\n",
        "}\n",
    );

    let trait_b = concat!(
        "<?php\n",
        "namespace App\\Traits;\n",
        "trait SoftDeletes {\n",
        "    public function trashed(): bool { return false; }\n",
        "}\n",
    );

    let composed_trait = concat!(
        "<?php\n",
        "namespace App\\Traits;\n",
        "use App\\Traits\\Timestamps;\n",
        "use App\\Traits\\SoftDeletes;\n",
        "trait ModelBehavior {\n",
        "    use Timestamps, SoftDeletes;\n",
        "    public function save(): bool { return true; }\n",
        "}\n",
    );

    let model_php = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "use App\\Traits\\ModelBehavior;\n",
        "class Post {\n",
        "    use ModelBehavior;\n",
        "    public string $title;\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Traits/Timestamps.php", trait_a),
            ("src/Traits/SoftDeletes.php", trait_b),
            ("src/Traits/ModelBehavior.php", composed_trait),
            ("src/Models/Post.php", model_php),
        ],
    );

    let uri = Url::parse("file:///test_nested_trait.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: model_php.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
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
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"save"),
                "Should include ModelBehavior::save, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getCreatedAt"),
                "Should include nested Timestamps::getCreatedAt, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"trashed"),
                "Should include nested SoftDeletes::trashed, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait static members ───────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_trait_static_methods_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait HasFactory {\n",
        "    public static function factory(): self { return new static(); }\n",
        "}\n",
        "class User {\n",
        "    use HasFactory;\n",
        "    public static function query(): string { return ''; }\n",
        "    function test() {\n",
        "        self::\n",
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
                    line: 8,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"factory"),
                "Should include trait static method 'factory', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"query"),
                "Should include own static method 'query', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Variable typed as class that uses trait ────────────────────────────────

#[tokio::test]
async fn test_completion_variable_of_class_with_trait() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Printable {\n",
        "    public function print(): void {}\n",
        "}\n",
        "class Document {\n",
        "    use Printable;\n",
        "    public function getTitle(): string { return ''; }\n",
        "}\n",
        "class Consumer {\n",
        "    public function test() {\n",
        "        $doc = new Document();\n",
        "        $doc->\n",
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
                    line: 11,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"print"),
                "Should include trait method 'print' on variable, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTitle"),
                "Should include own method 'getTitle' on variable, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait in parent class is inherited by child ────────────────────────────

#[tokio::test]
async fn test_completion_child_inherits_parent_trait_members() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///parent_trait.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Serializable {\n",
        "    public function serialize(): string { return ''; }\n",
        "}\n",
        "class BaseModel {\n",
        "    use Serializable;\n",
        "    public function getId(): int { return 0; }\n",
        "}\n",
        "class User extends BaseModel {\n",
        "    public function getEmail(): string { return ''; }\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 11,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Own method
            assert!(
                method_names.contains(&"getEmail"),
                "Should include own method 'getEmail', got: {:?}",
                method_names
            );
            // Parent method
            assert!(
                method_names.contains(&"getId"),
                "Should include parent method 'getId', got: {:?}",
                method_names
            );
            // Trait method from parent's trait
            assert!(
                method_names.contains(&"serialize"),
                "Should include trait method 'serialize' from parent, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait used in both class and parent (no duplicates) ────────────────────

#[tokio::test]
async fn test_completion_no_duplicate_members_from_trait() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_no_dup.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Loggable {\n",
        "    public function log(): void {}\n",
        "}\n",
        "class Base {\n",
        "    use Loggable;\n",
        "}\n",
        "class Child extends Base {\n",
        "    use Loggable;\n",
        "    function test() {\n",
        "        $this->\n",
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

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let log_count = items
                .iter()
                .filter(|i| {
                    i.kind == Some(CompletionItemKind::METHOD)
                        && i.filter_text.as_deref() == Some("log")
                })
                .count();

            assert_eq!(
                log_count, 1,
                "Should have exactly one 'log' method (no duplicates), got: {}",
                log_count
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait with method that returns typed object (chaining) ─────────────────

#[tokio::test]
async fn test_completion_trait_method_return_type_chain() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Builder {\n",
        "    public function build(): string { return ''; }\n",
        "}\n",
        "trait HasBuilder {\n",
        "    public function getBuilder(): Builder { return new Builder(); }\n",
        "}\n",
        "class Service {\n",
        "    use HasBuilder;\n",
        "    function test() {\n",
        "        $this->getBuilder()->\n",
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
                    line: 10,
                    character: 30,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"build"),
                "Should resolve trait method return type and chain to Builder::build, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait parsed correctly (parser test) ───────────────────────────────────

#[tokio::test]
async fn test_parser_extracts_trait_info() {
    let backend = create_test_backend();

    let text = concat!(
        "<?php\n",
        "trait MyTrait {\n",
        "    public function traitMethod(): void {}\n",
        "    public string $traitProp;\n",
        "}\n",
        "class MyClass {\n",
        "    use MyTrait;\n",
        "    public function classMethod(): void {}\n",
        "}\n",
    );

    let classes = backend.parse_php(text);

    // Should have both the trait and the class
    assert_eq!(classes.len(), 2, "Should have trait + class");

    let trait_info = classes.iter().find(|c| c.name == "MyTrait").unwrap();
    assert_eq!(trait_info.methods.len(), 1);
    assert_eq!(trait_info.methods[0].name, "traitMethod");
    assert_eq!(trait_info.properties.len(), 1);
    assert_eq!(trait_info.properties[0].name, "traitProp");
    assert!(trait_info.parent_class.is_none());

    let class_info = classes.iter().find(|c| c.name == "MyClass").unwrap();
    assert_eq!(class_info.methods.len(), 1);
    assert_eq!(class_info.methods[0].name, "classMethod");
    assert_eq!(class_info.used_traits.len(), 1);
    assert_eq!(class_info.used_traits[0], "MyTrait");
}

// ─── Trait with namespace parsed correctly ──────────────────────────────────

#[tokio::test]
async fn test_parser_resolves_trait_names_with_use_statements() {
    let backend = create_test_backend();

    let uri = "file:///parse_ns_trait.php";
    let text = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "use App\\Traits\\Auditable;\n",
        "class User {\n",
        "    use Auditable;\n",
        "}\n",
    );

    // update_ast resolves trait names via use-map and namespace
    backend.update_ast(uri, text);

    let classes = backend
        .get_classes_for_uri(uri)
        .expect("Should have AST entries");

    let user = classes.iter().find(|c| c.name == "User").unwrap();
    assert_eq!(user.used_traits.len(), 1);
    // The trait name should be resolved to FQN via the `use` statement
    assert_eq!(
        user.used_traits[0], "App\\Traits\\Auditable",
        "Trait name should be resolved to FQN, got: {}",
        user.used_traits[0]
    );
}

// ─── Go-to-definition for trait method ──────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_trait_method_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                                 // 0
        "trait Greetable {\n",                     // 1
        "    public function greet(): string {\n", // 2
        "        return 'hello';\n",               // 3
        "    }\n",                                 // 4
        "}\n",                                     // 5
        "class Person {\n",                        // 6
        "    use Greetable;\n",                    // 7
        "    function test() {\n",                 // 8
        "        $this->greet();\n",               // 9
        "    }\n",                                 // 10
        "}\n",                                     // 11
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
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 9,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should resolve definition for trait method"
    );
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        assert_eq!(location.uri, uri);
        // Should point to the `greet` method definition in the trait (line 2)
        assert_eq!(
            location.range.start.line, 2,
            "Should jump to trait method definition line"
        );
    } else {
        panic!("Expected GotoDefinitionResponse::Scalar");
    }
}

// ─── Go-to-definition for trait method cross-file ───────────────────────────

#[tokio::test]
async fn test_goto_definition_trait_method_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let trait_php = concat!(
        "<?php\n",
        "namespace App\\Traits;\n",
        "trait Loggable {\n",
        "    public function logMessage(): void {}\n",
        "}\n",
    );

    let class_php = concat!(
        "<?php\n",
        "namespace App\\Services;\n",
        "use App\\Traits\\Loggable;\n",
        "class Worker {\n",
        "    use Loggable;\n",
        "    function run() {\n",
        "        $this->logMessage();\n",
        "    }\n",
        "}\n",
    );

    let (backend, dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Traits/Loggable.php", trait_php),
            ("src/Services/Worker.php", class_php),
        ],
    );

    let uri = Url::parse("file:///test_trait_goto.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: class_php.to_string(),
            },
        })
        .await;

    let result = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 6,
                    character: 20,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should resolve definition for cross-file trait method"
    );
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        let trait_path = dir.path().join("src/Traits/Loggable.php");
        let expected_uri = Url::from_file_path(&trait_path).unwrap();
        assert_eq!(location.uri, expected_uri, "Should point to the trait file");
        // Method definition should be on line 3
        assert_eq!(
            location.range.start.line, 3,
            "Should jump to trait method definition line"
        );
    } else {
        panic!("Expected GotoDefinitionResponse::Scalar");
    }
}

// ─── Multiple use statements (separate lines) ──────────────────────────────

#[tokio::test]
async fn test_completion_separate_use_statements_for_traits() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///separate_use.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait A {\n",
        "    public function fromA(): void {}\n",
        "}\n",
        "trait B {\n",
        "    public function fromB(): void {}\n",
        "}\n",
        "class MyClass {\n",
        "    use A;\n",
        "    use B;\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 11,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"fromA"),
                "Should include A::fromA, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"fromB"),
                "Should include B::fromB, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait + interface + parent class combined ──────────────────────────────

#[tokio::test]
async fn test_completion_trait_with_interface_and_parent() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///combined.php").unwrap();
    let text = concat!(
        "<?php\n",
        "interface Printable {\n",
        "    public function print(): void;\n",
        "}\n",
        "trait Loggable {\n",
        "    public function log(): void {}\n",
        "}\n",
        "class Base {\n",
        "    public function baseMethod(): void {}\n",
        "}\n",
        "class Document extends Base implements Printable {\n",
        "    use Loggable;\n",
        "    public function print(): void {}\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 14,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"print"),
                "Should include own method 'print', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"log"),
                "Should include trait method 'log', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"baseMethod"),
                "Should include parent method 'baseMethod', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Param type hint with trait-using class ─────────────────────────────────

#[tokio::test]
async fn test_completion_param_type_hint_class_with_trait() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///param_trait.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Taggable {\n",
        "    public function addTag(string $tag): void {}\n",
        "    public function getTags(): array { return []; }\n",
        "}\n",
        "class Article {\n",
        "    use Taggable;\n",
        "    public function getTitle(): string { return ''; }\n",
        "}\n",
        "class Processor {\n",
        "    public function process(Article $article) {\n",
        "        $article->\n",
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
                    line: 11,
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"addTag"),
                "Should include trait method 'addTag' on param variable, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTags"),
                "Should include trait method 'getTags' on param variable, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTitle"),
                "Should include class method 'getTitle' on param variable, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Class with trait and child class ───────────────────────────────────────

#[tokio::test]
async fn test_completion_grandchild_inherits_trait_from_grandparent() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///grandchild_trait.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Identifiable {\n",
        "    public function getId(): int { return 0; }\n",
        "}\n",
        "class BaseModel {\n",
        "    use Identifiable;\n",
        "}\n",
        "class User extends BaseModel {\n",
        "    public function getName(): string { return ''; }\n",
        "}\n",
        "class Admin extends User {\n",
        "    public function getRole(): string { return ''; }\n",
        "    function test() {\n",
        "        $this->\n",
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
                    line: 13,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getRole"),
                "Should include own method 'getRole', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getName"),
                "Should include parent method 'getName', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getId"),
                "Should include grandparent trait method 'getId', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait definition itself is indexable ────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_trait_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///goto_trait_name.php").unwrap();
    let text = concat!(
        "<?php\n",                              // 0
        "trait Fooable {\n",                    // 1
        "    public function foo(): void {}\n", // 2
        "}\n",                                  // 3
        "class Bar {\n",                        // 4
        "    use Fooable;\n",                   // 5
        "}\n",                                  // 6
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

    // Click on "Fooable" in the use statement (line 5)
    let result = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position {
                    line: 5,
                    character: 10,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should resolve goto-definition for trait name"
    );
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        assert_eq!(location.uri, uri);
        // Should point to the trait declaration on line 1
        assert_eq!(
            location.range.start.line, 1,
            "Should jump to trait declaration line"
        );
    } else {
        panic!("Expected GotoDefinitionResponse::Scalar");
    }
}

// ─── Trait with docblock return types ───────────────────────────────────────

#[tokio::test]
async fn test_completion_trait_method_with_docblock_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_docblock.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Result {\n",
        "    public function isOk(): bool { return true; }\n",
        "}\n",
        "trait HasResult {\n",
        "    /** @return Result */\n",
        "    public function getResult() { return new Result(); }\n",
        "}\n",
        "class Handler {\n",
        "    use HasResult;\n",
        "    function test() {\n",
        "        $this->getResult()->\n",
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
                    line: 11,
                    character: 28,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"isOk"),
                "Should chain through trait method docblock return type to Result::isOk, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
