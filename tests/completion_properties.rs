mod common;

use common::{create_psr4_workspace, create_test_backend};
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

#[tokio::test]
async fn test_completion_promoted_properties_appear_in_this() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///promoted.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCartService {\n",
        "    private IShoppingCart $regular;\n",
        "\n",
        "    public function __construct(\n",
        "        private IShoppingCart $promoted,\n",
        "    ) {}\n",
        "\n",
        "    public function doWork(): void {\n",
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
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                names.contains(&"regular"),
                "Should contain regular property 'regular', got: {:?}",
                names
            );
            assert!(
                names.contains(&"promoted"),
                "Should contain promoted property 'promoted', got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Visibility filtering ───────────────────────────────────────────────────

/// Private properties should NOT appear when accessing a variable from
/// outside the class (e.g. top-level code).
#[tokio::test]
async fn test_completion_private_property_hidden_outside_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///vis_private_hidden.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Container {\n",
        "    private array $bindings = [];\n",
        "    public function bind(string $k, object $v): void {}\n",
        "    public function getStatus(): int { return 0; }\n",
        "}\n",
        "$c = new Container();\n",
        "$c->\n",
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
                    line: 7,
                    character: 4,
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
            let names: Vec<&str> = items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                names.contains(&"bind"),
                "Should include public method 'bind', got: {:?}",
                names
            );
            assert!(
                names.contains(&"getStatus"),
                "Should include public method 'getStatus', got: {:?}",
                names
            );
            assert!(
                !names.contains(&"bindings"),
                "Should NOT include private property 'bindings', got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Private methods should NOT appear when accessing from outside the class.
#[tokio::test]
async fn test_completion_private_method_hidden_outside_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///vis_private_method_hidden.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Service {\n",
        "    private function internalHelper(): void {}\n",
        "    protected function onSetup(): void {}\n",
        "    public function run(): void {}\n",
        "}\n",
        "$svc = new Service();\n",
        "$svc->\n",
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
                    line: 7,
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
                method_names.contains(&"run"),
                "Should include public method 'run', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"internalHelper"),
                "Should NOT include private method 'internalHelper', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"onSetup"),
                "Should NOT include protected method 'onSetup' from top-level, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `$this->` inside the same class should show private and protected members.
#[tokio::test]
async fn test_completion_private_and_protected_visible_inside_own_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///vis_private_visible.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Vault {\n",
        "    private string $secret = 'x';\n",
        "    protected int $level = 1;\n",
        "    public string $name = 'vault';\n",
        "    private function decrypt(): string { return ''; }\n",
        "    protected function validate(): bool { return true; }\n",
        "    public function open(): void {\n",
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
                    line: 8,
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
            let names: Vec<&str> = items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                names.contains(&"secret"),
                "Should include private property 'secret' via $this->, got: {:?}",
                names
            );
            assert!(
                names.contains(&"level"),
                "Should include protected property 'level' via $this->, got: {:?}",
                names
            );
            assert!(
                names.contains(&"name"),
                "Should include public property 'name' via $this->, got: {:?}",
                names
            );
            assert!(
                names.contains(&"decrypt"),
                "Should include private method 'decrypt' via $this->, got: {:?}",
                names
            );
            assert!(
                names.contains(&"validate"),
                "Should include protected method 'validate' via $this->, got: {:?}",
                names
            );
            assert!(
                names.contains(&"open"),
                "Should include public method 'open' via $this->, got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// A variable of the same class type inside the class should also show
/// private/protected members (PHP allows same-class access).
#[tokio::test]
async fn test_completion_private_visible_on_same_class_variable() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///vis_same_class_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Node {\n",
        "    private int $id;\n",
        "    public string $label;\n",
        "    public function merge(Node $other): void {\n",
        "        $other->\n",
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
                    line: 5,
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
            let names: Vec<&str> = items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                names.contains(&"id"),
                "Should include private 'id' on same-class variable, got: {:?}",
                names
            );
            assert!(
                names.contains(&"label"),
                "Should include public 'label', got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Protected members should be visible when accessing from a different
/// class (the caller might be a subclass).
#[tokio::test]
async fn test_completion_protected_visible_from_different_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///vis_protected_subclass.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    private function secret(): void {}\n",
        "    protected function hook(): void {}\n",
        "    public function run(): void {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    public function doWork(): void {\n",
        "        $b = new Base();\n",
        "        $b->\n",
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
                method_names.contains(&"run"),
                "Should include public 'run', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"hook"),
                "Should include protected 'hook' from inside another class, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"secret"),
                "Should NOT include private 'secret' from different class, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Private constants should be hidden from outside the class via `::`.
#[tokio::test]
async fn test_completion_private_constant_hidden_outside_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///vis_const_hidden.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    private const SECRET_KEY = 'abc';\n",
        "    protected const INTERNAL_VER = 2;\n",
        "    public const VERSION = '1.0';\n",
        "    public static function create(): void {}\n",
        "}\n",
        "Config::\n",
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
                    line: 7,
                    character: 8,
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
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                const_names.contains(&"VERSION"),
                "Should include public constant 'VERSION', got: {:?}",
                const_names
            );
            assert!(
                !const_names.contains(&"SECRET_KEY"),
                "Should NOT include private constant 'SECRET_KEY', got: {:?}",
                const_names
            );
            assert!(
                !const_names.contains(&"INTERNAL_VER"),
                "Should NOT include protected constant 'INTERNAL_VER' from top-level, got: {:?}",
                const_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `self::` inside the class should show all constants including private.
#[tokio::test]
async fn test_completion_private_constant_visible_via_self() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///vis_const_self.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    private const SECRET_KEY = 'abc';\n",
        "    protected const INTERNAL_VER = 2;\n",
        "    public const VERSION = '1.0';\n",
        "    public function check(): void {\n",
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
                    line: 6,
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
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                const_names.contains(&"VERSION"),
                "Should include public constant 'VERSION', got: {:?}",
                const_names
            );
            assert!(
                const_names.contains(&"SECRET_KEY"),
                "Should include private constant 'SECRET_KEY' via self::, got: {:?}",
                const_names
            );
            assert!(
                const_names.contains(&"INTERNAL_VER"),
                "Should include protected constant 'INTERNAL_VER' via self::, got: {:?}",
                const_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Cross-file: private property of a PSR-4 class should be hidden from
/// top-level code.
#[tokio::test]
async fn test_completion_private_hidden_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[(
            "src/Repo.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "class Repo {\n",
                "    private array $cache = [];\n",
                "    protected string $table = '';\n",
                "    public function find(int $id): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///vis_cross_file.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Repo;\n",
        "$repo = new Repo();\n",
        "$repo->\n",
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
                    line: 3,
                    character: 7,
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
            let names: Vec<&str> = items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                names.contains(&"find"),
                "Should include public method 'find', got: {:?}",
                names
            );
            assert!(
                !names.contains(&"cache"),
                "Should NOT include private property 'cache', got: {:?}",
                names
            );
            assert!(
                !names.contains(&"table"),
                "Should NOT include protected property 'table' from top-level, got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_promoted_property_type_resolves_for_chaining() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///promoted_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(string $msg): void {}\n",
        "    public function error(string $msg): void {}\n",
        "}\n",
        "class Service {\n",
        "    public function __construct(\n",
        "        private Logger $logger,\n",
        "    ) {}\n",
        "\n",
        "    public function run(): void {\n",
        "        $this->logger->\n",
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

    // Cursor right after `$this->logger->` on line 11
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 11,
                character: 24,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should resolve promoted property type for chaining"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let names: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                names.iter().any(|n| n.starts_with("info(")),
                "Should contain Logger method 'info', got: {:?}",
                names
            );
            assert!(
                names.iter().any(|n| n.starts_with("error(")),
                "Should contain Logger method 'error', got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
