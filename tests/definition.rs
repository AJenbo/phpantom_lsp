mod common;

use common::{create_psr4_workspace, create_test_backend};
use phpantom_lsp::Backend;
use std::collections::HashMap;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Word Extraction Tests ──────────────────────────────────────────────────

#[test]
fn test_extract_word_simple_class_name() {
    let content = "<?php\nclass Foo {}\n";
    // Cursor on "Foo"
    let pos = Position {
        line: 1,
        character: 7,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("Foo"));
}

#[test]
fn test_extract_word_fully_qualified_name() {
    let content = "<?php\nuse Illuminate\\Database\\Eloquent\\Model;\n";
    // Cursor somewhere inside the FQN
    let pos = Position {
        line: 1,
        character: 20,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(
        word.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Model")
    );
}

#[test]
fn test_extract_word_at_end_of_name() {
    let content = "<?php\nnew Exception();\n";
    // Cursor right after "Exception" (on the `(`)
    let pos = Position {
        line: 1,
        character: 13,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("Exception"));
}

#[test]
fn test_extract_word_class_reference() {
    let content = "<?php\n$x = OrderProductCollection::class;\n";
    // Cursor on "OrderProductCollection"
    let pos = Position {
        line: 1,
        character: 10,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("OrderProductCollection"));
}

#[test]
fn test_extract_word_type_hint() {
    let content = "<?php\npublic function order(): BelongsTo {}\n";
    // Cursor on "BelongsTo"
    let pos = Position {
        line: 1,
        character: 28,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("BelongsTo"));
}

#[test]
fn test_extract_word_on_whitespace_returns_none() {
    let content = "<?php\n   \n";
    let pos = Position {
        line: 1,
        character: 1,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert!(word.is_none());
}

#[test]
fn test_extract_word_leading_backslash_stripped() {
    let content = "<?php\nnew \\Exception();\n";
    // Cursor on "\\Exception"
    let pos = Position {
        line: 1,
        character: 6,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("Exception"));
}

#[test]
fn test_extract_word_past_end_of_file_returns_none() {
    let content = "<?php\n";
    let pos = Position {
        line: 10,
        character: 0,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert!(word.is_none());
}

#[test]
fn test_extract_word_parameter_type_hint() {
    let content = "<?php\npublic function run(IShoppingCart $cart): void {}\n";
    // Cursor on "IShoppingCart"
    let pos = Position {
        line: 1,
        character: 24,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("IShoppingCart"));
}

// ─── FQN Resolution Tests ──────────────────────────────────────────────────

#[test]
fn test_resolve_to_fqn_via_use_map() {
    let mut use_map = HashMap::new();
    use_map.insert(
        "BelongsTo".to_string(),
        "Illuminate\\Database\\Eloquent\\Relations\\BelongsTo".to_string(),
    );

    let fqn = Backend::resolve_to_fqn("BelongsTo", &use_map, &None);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Relations\\BelongsTo");
}

#[test]
fn test_resolve_to_fqn_via_namespace() {
    let use_map = HashMap::new();
    let namespace = Some("Luxplus\\Core\\Database\\Model\\Orders".to_string());

    let fqn = Backend::resolve_to_fqn("OrderProductCollection", &use_map, &namespace);
    assert_eq!(
        fqn,
        "Luxplus\\Core\\Database\\Model\\Orders\\OrderProductCollection"
    );
}

#[test]
fn test_resolve_to_fqn_already_qualified() {
    let use_map = HashMap::new();
    let fqn = Backend::resolve_to_fqn("Illuminate\\Database\\Eloquent\\Model", &use_map, &None);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Model");
}

#[test]
fn test_resolve_to_fqn_partial_qualified_with_use_map() {
    let mut use_map = HashMap::new();
    use_map.insert(
        "Eloquent".to_string(),
        "Illuminate\\Database\\Eloquent".to_string(),
    );

    let fqn = Backend::resolve_to_fqn("Eloquent\\Model", &use_map, &None);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Model");
}

#[test]
fn test_resolve_to_fqn_bare_name_no_context() {
    let use_map = HashMap::new();
    let fqn = Backend::resolve_to_fqn("Exception", &use_map, &None);
    assert_eq!(fqn, "Exception");
}

#[test]
fn test_resolve_to_fqn_use_map_takes_precedence_over_namespace() {
    let mut use_map = HashMap::new();
    use_map.insert(
        "HasFactory".to_string(),
        "Illuminate\\Database\\Eloquent\\Factories\\HasFactory".to_string(),
    );
    let namespace = Some("App\\Models".to_string());

    let fqn = Backend::resolve_to_fqn("HasFactory", &use_map, &namespace);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Factories\\HasFactory");
}

// ─── Find Definition Position Tests ─────────────────────────────────────────

#[test]
fn test_find_definition_position_class() {
    let content = "<?php\n\nclass Customer {\n    public function name() {}\n}\n";
    let pos = Backend::find_definition_position(content, "Customer");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_interface() {
    let content = "<?php\n\ninterface Loggable {\n    public function log(): void;\n}\n";
    let pos = Backend::find_definition_position(content, "Loggable");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_trait() {
    let content = "<?php\n\ntrait HasFactory {\n}\n";
    let pos = Backend::find_definition_position(content, "HasFactory");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_enum() {
    let content = "<?php\n\nenum LineItemType: string {\n    case Product = 'product';\n}\n";
    let pos = Backend::find_definition_position(content, "LineItemType");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_abstract_class() {
    let content = "<?php\n\nabstract class BaseModel {\n}\n";
    let pos = Backend::find_definition_position(content, "BaseModel");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    // "class" starts after "abstract "
    assert_eq!(pos.character, 9);
}

#[test]
fn test_find_definition_position_final_class() {
    let content = "<?php\n\nfinal class ImmutableValue {\n}\n";
    let pos = Backend::find_definition_position(content, "ImmutableValue");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 6);
}

#[test]
fn test_find_definition_position_not_found() {
    let content = "<?php\n\nclass Foo {}\n";
    let pos = Backend::find_definition_position(content, "Bar");
    assert!(pos.is_none());
}

#[test]
fn test_find_definition_position_no_partial_match() {
    let content = "<?php\n\nclass FooBar {}\n";
    // Should NOT match "Foo" inside "FooBar"
    let pos = Backend::find_definition_position(content, "Foo");
    assert!(pos.is_none());
}

#[test]
fn test_find_definition_position_with_namespace() {
    let content = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "\n",
        "class User {\n",
        "}\n",
    );
    let pos = Backend::find_definition_position(content, "User");
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 3);
}

// ─── Same-File Goto Definition Tests ────────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_same_file_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(): void {}\n",
        "}\n",
        "class Service {\n",
        "    public function run(Logger $logger): void {\n",
        "        $logger->info();\n",
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

    // Click on "Logger" in the parameter type hint on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 27,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve same-file class definition"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 1, "Logger is defined on line 1");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_same_file_interface() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "interface Cacheable {\n",
        "    public function getCacheKey(): string;\n",
        "}\n",
        "class Repository {\n",
        "    public function cache(Cacheable $item): void {}\n",
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

    // Click on "Cacheable" in the parameter type hint on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 30,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve same-file interface definition"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "Cacheable is defined on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Cross-File PSR-4 Goto Definition Tests ─────────────────────────────────

#[tokio::test]
async fn test_goto_definition_cross_file_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Logger.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class Logger {\n",
                "    public function info(string $msg): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///service.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Service {\n",
        "    public function run(Logger $logger): void {\n",
        "        $logger->info('hello');\n",
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

    // Click on "Logger" in the parameter type hint on line 4
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 27,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve cross-file PSR-4 class definition"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Logger.php"),
                "Should point to src/Logger.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 3,
                "Logger class defined on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_cross_file_with_use_statement() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/",
                    "App\\Contracts\\": "src/Contracts/"
                }
            }
        }"#,
        &[(
            "src/Contracts/Repository.php",
            concat!(
                "<?php\n",
                "namespace App\\Contracts;\n",
                "\n",
                "interface Repository {\n",
                "    public function find(int $id): mixed;\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///service.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Services;\n",
        "\n",
        "use App\\Contracts\\Repository;\n",
        "\n",
        "class UserService {\n",
        "    public function __construct(private Repository $repo) {}\n",
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

    // Click on "Repository" in the constructor parameter on line 6
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 47,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve class imported via use statement"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Contracts/Repository.php"),
                "Should point to Repository.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 3,
                "Repository interface defined on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_on_use_statement_name() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Models/User.php",
            concat!(
                "<?php\n",
                "namespace App\\Models;\n",
                "\n",
                "class User {\n",
                "    public string $name;\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///controller.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Controllers;\n",
        "\n",
        "use App\\Models\\User;\n",
        "\n",
        "class UserController {\n",
        "    public function show(User $user): void {}\n",
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

    // Click on the use statement FQN "App\\Models\\User" on line 3
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 3,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve goto-def on use statement FQN"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Models/User.php"),
                "Should point to User.php, got: {:?}",
                path
            );
            assert_eq!(location.range.start.line, 3, "User class defined on line 3");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_class_reference_via_namespace() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Enums/Status.php",
            concat!(
                "<?php\n",
                "namespace App\\Enums;\n",
                "\n",
                "enum Status: string {\n",
                "    case Active = 'active';\n",
                "    case Inactive = 'inactive';\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///model.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Enums;\n",
        "\n",
        "class Model {\n",
        "    protected $casts = [\n",
        "        'status' => Status::class,\n",
        "    ];\n",
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

    // Click on "Status" in Status::class on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve namespace-relative class reference"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Enums/Status.php"),
                "Should point to Status.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 3,
                "Status enum defined on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_return_type_hint() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Collection.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class Collection {\n",
                "    public function first(): mixed { return null; }\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///repo.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Repository {\n",
        "    public function getAll(): Collection {\n",
        "        return new Collection();\n",
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

    // Click on "Collection" in the return type on line 4
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 4,
                character: 33,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_some(), "Should resolve return type hint");

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Collection.php"),
                "Should point to Collection.php, got: {:?}",
                path
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Edge Cases ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_unresolvable_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\n$x = 42;\n";

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    // Click on a number — no class to resolve
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 5,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    // "42" gets extracted as a word but can't be resolved to any class
    assert!(result.is_none(), "Non-class symbol should return None");
}

#[tokio::test]
async fn test_goto_definition_whitespace_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = "<?php\n    \n";

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 2,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_none(), "Whitespace should return None");
}

#[tokio::test]
async fn test_goto_definition_vendor_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[
            (
                "vendor/composer/autoload_psr4.php",
                concat!(
                    "<?php\n",
                    "$vendorDir = dirname(__DIR__);\n",
                    "$baseDir = dirname($vendorDir);\n",
                    "\n",
                    "return array(\n",
                    "    'Monolog\\\\' => array($vendorDir . '/monolog/monolog/src/Monolog'),\n",
                    ");\n",
                ),
            ),
            (
                "vendor/monolog/monolog/src/Monolog/Logger.php",
                concat!(
                    "<?php\n",
                    "namespace Monolog;\n",
                    "\n",
                    "class Logger {\n",
                    "    public function info(string $msg): void {}\n",
                    "}\n",
                ),
            ),
        ],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "use Monolog\\Logger;\n",
        "\n",
        "class App {\n",
        "    public function boot(Logger $log): void {}\n",
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

    // Click on "Logger" in the parameter type hint on line 6
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 30,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve vendor class via PSR-4 autoload"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("vendor/monolog/monolog/src/Monolog/Logger.php"),
                "Should point to vendor Logger.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 3,
                "Logger class defined on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_trait() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Traits/Auditable.php",
            concat!(
                "<?php\n",
                "namespace App\\Traits;\n",
                "\n",
                "trait Auditable {\n",
                "    public function getAuditLog(): array { return []; }\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///model.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "\n",
        "use App\\Traits\\Auditable;\n",
        "\n",
        "class Order {\n",
        "    use Auditable;\n",
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

    // Click on "Auditable" in `use Auditable;` inside the class on line 6
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 10,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_some(), "Should resolve trait via use statement");

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Traits/Auditable.php"),
                "Should point to Auditable.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 3,
                "Auditable trait defined on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_extends_class() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/BaseModel.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "abstract class BaseModel {\n",
                "    public function save(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///user.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class User extends BaseModel {\n",
        "    public string $name;\n",
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

    // Click on "BaseModel" in the extends clause on line 3
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 3,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_some(), "Should resolve parent class in extends");

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/BaseModel.php"),
                "Should point to BaseModel.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 3,
                "BaseModel class defined on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: Class Constants ─────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_class_constant_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class MyClass {\n",
        "    const MY_CONST = 42;\n",
        "    const OTHER = 'hello';\n",
        "\n",
        "    public function foo(): int {\n",
        "        return self::MY_CONST;\n",
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

    // Click on "MY_CONST" in `self::MY_CONST` on line 6
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 6,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve self::MY_CONST to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "const MY_CONST is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_class_constant_via_classname() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Status {\n",
        "    const ACTIVE = 1;\n",
        "    const INACTIVE = 0;\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    public function check(): int {\n",
        "        return Status::ACTIVE;\n",
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

    // Click on "ACTIVE" in `Status::ACTIVE` on line 8
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 8,
                character: 24,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve Status::ACTIVE to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "const ACTIVE is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_class_constant_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Status.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class Status {\n",
                "    const PENDING = 'pending';\n",
                "    const APPROVED = 'approved';\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///service.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class OrderService {\n",
        "    public function getDefault(): string {\n",
        "        return Status::PENDING;\n",
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

    // Click on "PENDING" in `Status::PENDING` on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 25,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve cross-file Status::PENDING"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Status.php"),
                "Should point to Status.php, got: {:?}",
                path
            );
            assert_eq!(location.range.start.line, 4, "const PENDING is on line 4");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: Methods ─────────────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_method_via_this() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(string $msg): void {}\n",
        "\n",
        "    public function warn(string $msg): void {\n",
        "        $this->info($msg);\n",
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

    // Click on "info" in `$this->info(...)` on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $this->info to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "function info is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_static_method_via_classname() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public static function create(): self {\n",
        "        return new self();\n",
        "    }\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        Factory::create();\n",
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

    // Click on "create" in `Factory::create()` on line 9
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve Factory::create to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "function create is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_method_via_self() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Calculator {\n",
        "    public static function add(int $a, int $b): int {\n",
        "        return $a + $b;\n",
        "    }\n",
        "\n",
        "    public static function sum(array $nums): int {\n",
        "        return self::add($nums[0], $nums[1]);\n",
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

    // Click on "add" in `self::add(...)` on line 7
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 7,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve self::add to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "function add is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_method_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Logger.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class Logger {\n",
                "    public function info(string $msg): void {}\n",
                "    public function error(string $msg): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///service.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Service {\n",
        "    public function run(Logger $logger): void {\n",
        "        $logger->error('failed');\n",
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

    // Click on "error" in `$logger->error(...)` on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_some(), "Should resolve cross-file $logger->error");

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Logger.php"),
                "Should point to Logger.php, got: {:?}",
                path
            );
            assert_eq!(location.range.start.line, 5, "function error is on line 5");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: Properties ──────────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_property_via_this() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public int $age;\n",
        "\n",
        "    public function getName(): string {\n",
        "        return $this->name;\n",
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

    // Click on "name" in `$this->name` on line 6
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 6,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $this->name to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "$name property is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_property_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Config.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class Config {\n",
                "    public string $dbHost;\n",
                "    public int $dbPort;\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///service.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Service {\n",
        "    public function connect(Config $cfg): void {\n",
        "        $host = $cfg->dbHost;\n",
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

    // Click on "dbHost" in `$cfg->dbHost` on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 24,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_some(), "Should resolve cross-file $cfg->dbHost");

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Config.php"),
                "Should point to Config.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 4,
                "$dbHost property is on line 4"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: Inherited Members ───────────────────────────────────

#[tokio::test]
async fn test_goto_definition_inherited_method() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/BaseModel.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class BaseModel {\n",
                "    public function save(): void {}\n",
                "    public function delete(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///user.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class User extends BaseModel {\n",
        "    public string $name;\n",
        "\n",
        "    public function update(): void {\n",
        "        $this->save();\n",
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

    // Click on "save" in `$this->save()` on line 7
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve inherited $this->save() to parent class"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/BaseModel.php"),
                "Should point to BaseModel.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 4,
                "function save is on line 4 of BaseModel.php"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_inherited_constant_via_parent() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    const VERSION = '1.0';\n",
        "}\n",
        "\n",
        "class Child extends Base {\n",
        "    public function getVersion(): string {\n",
        "        return parent::VERSION;\n",
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

    // Click on "VERSION" in `parent::VERSION` on line 7
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 7,
                character: 25,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve parent::VERSION to Base class"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "const VERSION is declared on line 2 in Base"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: Variable Type Inference ─────────────────────────────

#[tokio::test]
async fn test_goto_definition_method_on_new_variable() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Mailer {\n",
        "    public function send(string $to): void {}\n",
        "    public function queue(string $to): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $mailer = new Mailer();\n",
        "        $mailer->send('user@example.com');\n",
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

    // Click on "send" in `$mailer->send(...)` on line 9
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $mailer->send via new Mailer() assignment"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "function send is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: Chained Access ──────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_chained_property_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Connection {\n",
        "    public function query(string $sql): void {}\n",
        "}\n",
        "\n",
        "class Database {\n",
        "    public Connection $conn;\n",
        "\n",
        "    public function run(): void {\n",
        "        $this->conn->query('SELECT 1');\n",
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

    // Click on "query" in `$this->conn->query(...)` on line 9
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $this->conn->query via chained property"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "function query is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: Promoted Properties ─────────────────────────────────

#[tokio::test]
async fn test_goto_definition_promoted_property() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function __construct(\n",
        "        private string $name,\n",
        "        private int $age,\n",
        "    ) {}\n",
        "\n",
        "    public function getName(): string {\n",
        "        return $this->name;\n",
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

    // Click on "name" in `$this->name` on line 8
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 8,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $this->name to promoted property"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 3, "promoted $name is on line 3");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: static:: keyword ────────────────────────────────────

#[tokio::test]
async fn test_goto_definition_constant_via_static() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    const MAX_RETRIES = 3;\n",
        "\n",
        "    public function getMax(): int {\n",
        "        return static::MAX_RETRIES;\n",
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

    // Click on "MAX_RETRIES" in `static::MAX_RETRIES` on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 24,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_some(), "Should resolve static::MAX_RETRIES");

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "const MAX_RETRIES is on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: use statement + cross-file ──────────────────────────

#[tokio::test]
async fn test_goto_definition_method_cross_file_with_use_statement() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Lib\\": "lib/"
                }
            }
        }"#,
        &[(
            "lib/Cache.php",
            concat!(
                "<?php\n",
                "namespace Lib;\n",
                "\n",
                "class Cache {\n",
                "    public function get(string $key): mixed {}\n",
                "    public function set(string $key, mixed $val): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "use Lib\\Cache;\n",
        "\n",
        "class Service {\n",
        "    public function load(Cache $cache): void {\n",
        "        $cache->get('key');\n",
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

    // Click on "get" in `$cache->get(...)` on line 7
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $cache->get via use statement"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("lib/Cache.php"),
                "Should point to Cache.php, got: {:?}",
                path
            );
            assert_eq!(location.range.start.line, 4, "function get is on line 4");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Member Definition: cursor on class name still resolves class ───────────

#[tokio::test]
async fn test_goto_definition_cursor_on_classname_before_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Status {\n",
        "    const ACTIVE = 1;\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    public function check(): int {\n",
        "        return Status::ACTIVE;\n",
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

    // Click on "Status" (the class name, left side of ::) on line 7
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 7,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Cursor on class name before :: should resolve to the class"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 1, "class Status is on line 1");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Goto Definition: new self() / new static() / new parent() ─────────────

#[tokio::test]
async fn test_goto_definition_new_self_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function create(): self {\n",
        "        return new self();\n",
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

    // Click on "self" in `new self()` on line 3
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 3,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new self()` to the enclosing class"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "class SessionManager is on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_new_static_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function create(): static {\n",
        "        return new static();\n",
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

    // Click on "static" in `new static()` on line 3
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 3,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new static()` to the enclosing class"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "class SessionManager is on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_new_parent_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Manager {\n",
        "    public function boot(): void {}\n",
        "}\n",
        "\n",
        "class SessionManager extends Manager {\n",
        "    protected function callCustomCreator(): void {\n",
        "        new parent();\n",
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

    // Click on "parent" in `new parent()` on line 7
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 7,
                character: 13,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new parent()` to the parent class Manager"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 1, "class Manager is on line 1");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_new_parent_cross_file_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Manager.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "\n",
                "class Manager {\n",
                "    public function boot(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class SessionManager extends Manager {\n",
        "    protected function callCustomCreator(): void {\n",
        "        new parent();\n",
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

    // Click on "parent" in `new parent()` on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 13,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new parent()` to cross-file Manager class via PSR-4"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/Manager.php"),
                "Should point to Manager.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 3,
                "class Manager is on line 3 in Manager.php"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_self_outside_class_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    // `self` used outside any class — should not resolve
    let text = concat!("<?php\n", "$x = new self();\n",);

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    // Click on "self" on line 1
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "`new self()` outside a class should not resolve"
    );
}

#[tokio::test]
async fn test_goto_definition_new_self_in_nested_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Factory {\n",
        "    public function build(): void {}\n",
        "\n",
        "    public static function create(): self {\n",
        "        $instance = new self();\n",
        "        return $instance;\n",
        "    }\n",
        "\n",
        "    public static function createStatic(): static {\n",
        "        $instance = new static();\n",
        "        return $instance;\n",
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

    // Click on "self" in `new self()` on line 7
    let params_self = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 7,
                character: 24,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params_self).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new self()` inside namespaced class"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 3, "class Factory is on line 3");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // Click on "static" in `new static()` on line 12
    let params_static = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 12,
                character: 24,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params_static).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new static()` inside namespaced class"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 3, "class Factory is on line 3");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Goto Definition: new ClassName()->method() ─────────────────────────────

#[tokio::test]
async fn test_goto_definition_new_classname_arrow_method_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function callCustomCreator(): void {}\n",
        "    public function boot(): void {\n",
        "        new SessionManager()->callCustomCreator();\n",
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

    // Click on "callCustomCreator" in `new SessionManager()->callCustomCreator()` on line 4
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 4,
                character: 32,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new SessionManager()->callCustomCreator()` to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "callCustomCreator is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_parenthesized_new_arrow_method_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SessionManager {\n",
        "    public function callCustomCreator(): void {}\n",
        "    public function boot(): void {\n",
        "        (new SessionManager())->callCustomCreator();\n",
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

    // Click on "callCustomCreator" in `(new SessionManager())->callCustomCreator()` on line 4
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 4,
                character: 33,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `(new SessionManager())->callCustomCreator()` to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "callCustomCreator is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_new_classname_arrow_method_cross_file() {
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
        "        new SessionManager()->callCustomCreator();\n",
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

    // Click on "callCustomCreator" on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 32,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new SessionManager()->callCustomCreator()` cross-file"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/SessionManager.php"),
                "Should point to SessionManager.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 4,
                "callCustomCreator is on line 4 in SessionManager.php"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_parenthesized_new_arrow_method_cross_file() {
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
        "        (new SessionManager())->callCustomCreator();\n",
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

    // Click on "callCustomCreator" on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 5,
                character: 33,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `(new SessionManager())->callCustomCreator()` cross-file"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            let path = location.uri.to_file_path().unwrap();
            assert!(
                path.ends_with("src/SessionManager.php"),
                "Should point to SessionManager.php, got: {:?}",
                path
            );
            assert_eq!(
                location.range.start.line, 4,
                "callCustomCreator is on line 4 in SessionManager.php"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_new_namespaced_classname_arrow_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Widget {\n",
        "    public function render(): void {}\n",
        "}\n",
        "\n",
        "class Page {\n",
        "    public function show(): void {\n",
        "        (new Widget())->render();\n",
        "        new Widget()->render();\n",
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

    // Click on "render" in `(new Widget())->render()` on line 9
    let params1 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 24,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params1).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `(new Widget())->render()` to render declaration"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 4, "render is on line 4");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // Click on "render" in `new Widget()->render()` on line 10
    let params2 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 10,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params2).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve `new Widget()->render()` to render declaration"
    );
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 4, "render is on line 4");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Ambiguous Variable Types ───────────────────────────────────────────────

/// When a variable is reassigned inside an `if` block, the variable could be
/// either type after the block.  Goto definition should still resolve the
/// member if *any* candidate type declares it.
///
/// ```php
/// $thing = new SessionManager();
/// if ($thing->callCustomCreator2()) {
///     $thing = new Manager();
/// }
/// $thing->callCustomCreator2(); // Manager doesn't have it, but SessionManager does
/// ```
#[tokio::test]
async fn test_goto_definition_ambiguous_variable_if_block() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                             // 0
        "class SessionManager {\n",                            // 1
        "    public function callCustomCreator2(): void {}\n", // 2
        "    public function start(): void {}\n",              // 3
        "}\n",                                                 // 4
        "\n",                                                  // 5
        "class Manager {\n",                                   // 6
        "    public function doWork(): void {}\n",             // 7
        "}\n",                                                 // 8
        "\n",                                                  // 9
        "class App {\n",                                       // 10
        "    public function run(): void {\n",                 // 11
        "        $thing = new SessionManager();\n",            // 12
        "        if ($thing->callCustomCreator2()) {\n",       // 13
        "            $thing = new Manager();\n",               // 14
        "        }\n",                                         // 15
        "        $thing->callCustomCreator2();\n",             // 16
        "    }\n",                                             // 17
        "}\n",                                                 // 18
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

    // Click on "callCustomCreator2" on line 16: $thing->callCustomCreator2()
    // After the if block, $thing could be SessionManager or Manager.
    // Manager doesn't have callCustomCreator2, but SessionManager does.
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 16,
                character: 20,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $thing->callCustomCreator2() via SessionManager even though Manager was assigned in if-block"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "callCustomCreator2 is declared on line 2 in SessionManager"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// When both candidate types share a method, the first candidate (original
/// assignment) should win.
#[tokio::test]
async fn test_goto_definition_ambiguous_variable_both_have_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                // 0
        "class Alpha {\n",                        // 1
        "    public function greet(): void {}\n", // 2
        "}\n",                                    // 3
        "\n",                                     // 4
        "class Beta {\n",                         // 5
        "    public function greet(): void {}\n", // 6
        "}\n",                                    // 7
        "\n",                                     // 8
        "class App {\n",                          // 9
        "    public function run(): void {\n",    // 10
        "        $obj = new Alpha();\n",          // 11
        "        if (true) {\n",                  // 12
        "            $obj = new Beta();\n",       // 13
        "        }\n",                            // 14
        "        $obj->greet();\n",               // 15
        "    }\n",                                // 16
        "}\n",                                    // 17
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

    // Click on "greet" on line 15
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 15,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $obj->greet() when both Alpha and Beta have greet()"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            // First candidate (Alpha) wins since it was the original assignment
            assert_eq!(
                location.range.start.line, 2,
                "greet() should resolve to Alpha (line 2) as the first candidate"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// An unconditional reassignment should replace previous candidates,
/// so only the final type is used.
#[tokio::test]
async fn test_goto_definition_unconditional_reassignment_replaces_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "class Foo {\n",                            // 1
        "    public function fooOnly(): void {}\n", // 2
        "}\n",                                      // 3
        "\n",                                       // 4
        "class Bar {\n",                            // 5
        "    public function barOnly(): void {}\n", // 6
        "}\n",                                      // 7
        "\n",                                       // 8
        "class App {\n",                            // 9
        "    public function run(): void {\n",      // 10
        "        $x = new Foo();\n",                // 11
        "        $x = new Bar();\n",                // 12
        "        $x->barOnly();\n",                 // 13
        "    }\n",                                  // 14
        "}\n",                                      // 15
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

    // Click on "barOnly" on line 13 — unconditional reassignment means $x is Bar
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 13,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $x->barOnly() to Bar::barOnly"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 6,
                "barOnly is declared on line 6 in Bar"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // fooOnly should NOT resolve since Foo was unconditionally replaced by Bar
    let params2 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 13,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result2 = backend.goto_definition(params2).await.unwrap();
    // The result should resolve to Bar, not Foo — we already checked above
    assert!(result2.is_some());
    match result2.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_ne!(
                location.range.start.line, 2,
                "fooOnly on line 2 (Foo) should NOT be reachable after unconditional reassignment"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Ambiguous variable across try/catch: reassignment in try block should
/// add a candidate, not replace the original.
#[tokio::test]
async fn test_goto_definition_ambiguous_variable_try_catch() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                         // 0
        "class Logger {\n",                                // 1
        "    public function log(string $msg): void {}\n", // 2
        "}\n",                                             // 3
        "\n",                                              // 4
        "class NullLogger {\n",                            // 5
        "    public function silence(): void {}\n",        // 6
        "}\n",                                             // 7
        "\n",                                              // 8
        "class App {\n",                                   // 9
        "    public function run(): void {\n",             // 10
        "        $logger = new Logger();\n",               // 11
        "        try {\n",                                 // 12
        "            $logger = new NullLogger();\n",       // 13
        "        } catch (\\Exception $e) {\n",            // 14
        "        }\n",                                     // 15
        "        $logger->log('hello');\n",                // 16
        "    }\n",                                         // 17
        "}\n",                                             // 18
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

    // Click on "log" on line 16: $logger->log('hello')
    // NullLogger doesn't have log(), but Logger does.
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 16,
                character: 20,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $logger->log() via Logger even though NullLogger was assigned in try block"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "log() is declared on line 2 in Logger"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Ambiguous variable with if/else: both branches reassign, original type
/// should still be available as a candidate.
#[tokio::test]
async fn test_goto_definition_ambiguous_variable_if_else_branches() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                // 0
        "class Writer {\n",                       // 1
        "    public function write(): void {}\n", // 2
        "}\n",                                    // 3
        "\n",                                     // 4
        "class Printer {\n",                      // 5
        "    public function print(): void {}\n", // 6
        "}\n",                                    // 7
        "\n",                                     // 8
        "class Sender {\n",                       // 9
        "    public function send(): void {}\n",  // 10
        "}\n",                                    // 11
        "\n",                                     // 12
        "class App {\n",                          // 13
        "    public function run(): void {\n",    // 14
        "        $out = new Writer();\n",         // 15
        "        if (true) {\n",                  // 16
        "            $out = new Printer();\n",    // 17
        "        } else {\n",                     // 18
        "            $out = new Sender();\n",     // 19
        "        }\n",                            // 20
        "        $out->write();\n",               // 21
        "    }\n",                                // 22
        "}\n",                                    // 23
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

    // Click on "write" on line 21 — only Writer has write()
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 21,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $out->write() via Writer even with if/else reassignments"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "write() is declared on line 2 in Writer"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Ambiguous variable across a loop: reassignment inside a while loop should
/// add a candidate.
#[tokio::test]
async fn test_goto_definition_ambiguous_variable_loop() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                   // 0
        "class Handler {\n",                         // 1
        "    public function handle(): void {}\n",   // 2
        "}\n",                                       // 3
        "\n",                                        // 4
        "class Fallback {\n",                        // 5
        "    public function fallback(): void {}\n", // 6
        "}\n",                                       // 7
        "\n",                                        // 8
        "class App {\n",                             // 9
        "    public function run(): void {\n",       // 10
        "        $h = new Handler();\n",             // 11
        "        while (true) {\n",                  // 12
        "            $h = new Fallback();\n",        // 13
        "        }\n",                               // 14
        "        $h->handle();\n",                   // 15
        "    }\n",                                   // 16
        "}\n",                                       // 17
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

    // Click on "handle" on line 15 — Fallback doesn't have handle(), Handler does
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 15,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $h->handle() via Handler even though Fallback was assigned in while loop"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "handle() is declared on line 2 in Handler"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Cross-file ambiguous variable: the reassigned class comes from PSR-4.
#[tokio::test]
async fn test_goto_definition_ambiguous_variable_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
        &[
            (
                "src/Cache.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Cache {\n",
                    "    public function get(string $key): mixed { return null; }\n",
                    "}\n",
                ),
            ),
            (
                "src/NullCache.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class NullCache {\n",
                    "    public function clear(): void {}\n",
                    "}\n",
                ),
            ),
        ],
    );

    let uri = Url::parse("file:///test_main.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Cache;\n",
        "use App\\NullCache;\n",
        "\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $store = new Cache();\n",
        "        if (getenv('DISABLE_CACHE')) {\n",
        "            $store = new NullCache();\n",
        "        }\n",
        "        $store->get('key');\n",
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

    // Click on "get" on line 10: $store->get('key')
    // NullCache doesn't have get(), but Cache does (cross-file via PSR-4)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 10,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $store->get() via Cache (PSR-4) even with NullCache in if-block"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            // Cache::get is declared on line 3 (0-indexed) of Cache.php
            assert_eq!(
                location.range.start.line, 3,
                "get() should be on line 3 of Cache.php"
            );
            let loc_path = location.uri.to_file_path().unwrap();
            assert!(
                loc_path.ends_with("src/Cache.php"),
                "Should resolve to Cache.php, got: {:?}",
                loc_path
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Union Return Type Resolution ───────────────────────────────────────────

/// When a function returns a union type (`B|C`), goto definition should
/// resolve the member if any part of the union declares it.
///
/// ```php
/// function a(): B|C { ... }
/// $a = a();
/// $a->onlyOnB(); // B has it, C doesn't — should still resolve
/// ```
#[tokio::test]
async fn test_goto_definition_union_return_type_function() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                                // 0
        "class B {\n",                                            // 1
        "    public function onlyOnB(): void {}\n",               // 2
        "}\n",                                                    // 3
        "\n",                                                     // 4
        "class C {\n",                                            // 5
        "    public function onlyOnC(): void {}\n",               // 6
        "}\n",                                                    // 7
        "\n",                                                     // 8
        "class App {\n",                                          // 9
        "    public function getBC(): B|C { return new B(); }\n", // 10
        "\n",                                                     // 11
        "    public function run(): void {\n",                    // 12
        "        $a = $this->getBC();\n",                         // 13
        "        $a->onlyOnB();\n",                               // 14
        "        $a->onlyOnC();\n",                               // 15
        "    }\n",                                                // 16
        "}\n",                                                    // 17
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

    // Click on "onlyOnB" on line 14 — B has it, C doesn't
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 14,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $a->onlyOnB() via union return type B|C"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "onlyOnB is declared on line 2 in class B"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // Click on "onlyOnC" on line 15 — C has it, B doesn't
    let params2 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 15,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result2 = backend.goto_definition(params2).await.unwrap();
    assert!(
        result2.is_some(),
        "Should resolve $a->onlyOnC() via union return type B|C"
    );

    match result2.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 6,
                "onlyOnC is declared on line 6 in class C"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Union return type via a standalone function assigned to a variable:
/// `$x = someFunc();` where `someFunc(): A|B`.
#[tokio::test]
async fn test_goto_definition_union_return_type_standalone_function() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                               // 0
        "class Dog {\n",                         // 1
        "    public function bark(): void {}\n", // 2
        "}\n",                                   // 3
        "\n",                                    // 4
        "class Cat {\n",                         // 5
        "    public function meow(): void {}\n", // 6
        "}\n",                                   // 7
        "\n",                                    // 8
        "function getAnimal(): Dog|Cat {\n",     // 9
        "    return new Dog();\n",               // 10
        "}\n",                                   // 11
        "\n",                                    // 12
        "class App {\n",                         // 13
        "    public function run(): void {\n",   // 14
        "        $pet = getAnimal();\n",         // 15
        "        $pet->bark();\n",               // 16
        "        $pet->meow();\n",               // 17
        "    }\n",                               // 18
        "}\n",                                   // 19
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

    // Register the standalone function in global_functions so the resolver
    // can look up its return type.
    {
        let mut fmap = backend.global_functions.lock().unwrap();
        fmap.insert(
            "getAnimal".to_string(),
            (
                uri.to_string(),
                phpantom_lsp::FunctionInfo {
                    name: "getAnimal".to_string(),
                    parameters: vec![],
                    return_type: Some("Dog|Cat".to_string()),
                    namespace: None,
                },
            ),
        );
    }

    // Click on "bark" on line 16 — Dog has it
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 16,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $pet->bark() via Dog|Cat union return type"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "bark is declared on line 2 in Dog"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // Click on "meow" on line 17 — Cat has it
    let params2 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 17,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result2 = backend.goto_definition(params2).await.unwrap();
    assert!(
        result2.is_some(),
        "Should resolve $pet->meow() via Dog|Cat union return type"
    );

    match result2.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 6,
                "meow is declared on line 6 in Cat"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Nullable union type (`?Foo` is equivalent to `Foo|null`): should still
/// resolve the class part.
#[tokio::test]
async fn test_goto_definition_nullable_union_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                                        // 0
        "class Formatter {\n",                                            // 1
        "    public function format(string $s): string { return $s; }\n", // 2
        "}\n",                                                            // 3
        "\n",                                                             // 4
        "class App {\n",                                                  // 5
        "    public function getFormatter(): ?Formatter {\n",             // 6
        "        return new Formatter();\n",                              // 7
        "    }\n",                                                        // 8
        "\n",                                                             // 9
        "    public function run(): void {\n",                            // 10
        "        $f = $this->getFormatter();\n",                          // 11
        "        $f->format('hello');\n",                                 // 12
        "    }\n",                                                        // 13
        "}\n",                                                            // 14
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

    // Click on "format" on line 12
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 12,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $f->format() via ?Formatter nullable return type"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "format is declared on line 2 in Formatter"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Union type on a property: `public A|B $prop;`
#[tokio::test]
async fn test_goto_definition_union_property_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                // 0
        "class Engine {\n",                       // 1
        "    public function start(): void {}\n", // 2
        "}\n",                                    // 3
        "\n",                                     // 4
        "class Motor {\n",                        // 5
        "    public function rev(): void {}\n",   // 6
        "}\n",                                    // 7
        "\n",                                     // 8
        "class Car {\n",                          // 9
        "    public Engine|Motor $powerUnit;\n",  // 10
        "\n",                                     // 11
        "    public function run(): void {\n",    // 12
        "        $this->powerUnit->start();\n",   // 13
        "        $this->powerUnit->rev();\n",     // 14
        "    }\n",                                // 15
        "}\n",                                    // 16
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

    // Click on "start" on line 13 — Engine has it
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 13,
                character: 30,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $this->powerUnit->start() via Engine|Motor union property type"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "start is declared on line 2 in Engine"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // Click on "rev" on line 14 — Motor has it
    let params2 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 14,
                character: 27,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result2 = backend.goto_definition(params2).await.unwrap();
    assert!(
        result2.is_some(),
        "Should resolve $this->powerUnit->rev() via Engine|Motor union property type"
    );

    match result2.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 6,
                "rev is declared on line 6 in Motor"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Union type in a parameter type hint: `function run(A|B $x) { $x->method(); }`
#[tokio::test]
async fn test_goto_definition_union_parameter_type_hint() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                                     // 0
        "class Reader {\n",                                            // 1
        "    public function read(): void {}\n",                       // 2
        "}\n",                                                         // 3
        "\n",                                                          // 4
        "class Stream {\n",                                            // 5
        "    public function consume(): void {}\n",                    // 6
        "}\n",                                                         // 7
        "\n",                                                          // 8
        "class App {\n",                                               // 9
        "    public function process(Reader|Stream $input): void {\n", // 10
        "        $input->read();\n",                                   // 11
        "        $input->consume();\n",                                // 12
        "    }\n",                                                     // 13
        "}\n",                                                         // 14
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

    // Click on "read" on line 11 — Reader has it
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 11,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $input->read() via Reader|Stream union param type"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "read is declared on line 2 in Reader"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // Click on "consume" on line 12 — Stream has it
    let params2 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 12,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result2 = backend.goto_definition(params2).await.unwrap();
    assert!(
        result2.is_some(),
        "Should resolve $input->consume() via Reader|Stream union param type"
    );

    match result2.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 6,
                "consume is declared on line 6 in Stream"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Union return type with scalar parts: `string|Foo` — the scalar `string`
/// should be ignored and `Foo` should resolve.
#[tokio::test]
async fn test_goto_definition_union_with_scalar_parts() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                                // 0
        "class Result {\n",                                       // 1
        "    public function unwrap(): mixed { return null; }\n", // 2
        "}\n",                                                    // 3
        "\n",                                                     // 4
        "class App {\n",                                          // 5
        "    public function fetch(): string|Result {\n",         // 6
        "        return new Result();\n",                         // 7
        "    }\n",                                                // 8
        "\n",                                                     // 9
        "    public function run(): void {\n",                    // 10
        "        $r = $this->fetch();\n",                         // 11
        "        $r->unwrap();\n",                                // 12
        "    }\n",                                                // 13
        "}\n",                                                    // 14
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

    // Click on "unwrap" on line 12 — `string` part is ignored, Result has it
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 12,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $r->unwrap() via string|Result, ignoring scalar part"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "unwrap is declared on line 2 in Result"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Cross-file union return type: parts of the union come from PSR-4.
#[tokio::test]
async fn test_goto_definition_union_return_type_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
        &[
            (
                "src/Encoder.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Encoder {\n",
                    "    public function encode(string $data): string { return $data; }\n",
                    "}\n",
                ),
            ),
            (
                "src/Decoder.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Decoder {\n",
                    "    public function decode(string $data): string { return $data; }\n",
                    "}\n",
                ),
            ),
        ],
    );

    let uri = Url::parse("file:///test_main.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Encoder;\n",
        "use App\\Decoder;\n",
        "\n",
        "class Codec {\n",
        "    public function getCodec(): Encoder|Decoder {\n",
        "        return new Encoder();\n",
        "    }\n",
        "\n",
        "    public function run(): void {\n",
        "        $c = $this->getCodec();\n",
        "        $c->encode('data');\n",
        "        $c->decode('data');\n",
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

    // Click on "encode" on line 11 — Encoder has it (cross-file)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 11,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $c->encode() via Encoder|Decoder union return type (cross-file)"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(
                location.range.start.line, 3,
                "encode is on line 3 of Encoder.php"
            );
            let loc_path = location.uri.to_file_path().unwrap();
            assert!(
                loc_path.ends_with("src/Encoder.php"),
                "Should resolve to Encoder.php, got: {:?}",
                loc_path
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // Click on "decode" on line 12 — Decoder has it (cross-file)
    let params2 = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 12,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result2 = backend.goto_definition(params2).await.unwrap();
    assert!(
        result2.is_some(),
        "Should resolve $c->decode() via Encoder|Decoder union return type (cross-file)"
    );

    match result2.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(
                location.range.start.line, 3,
                "decode is on line 3 of Decoder.php"
            );
            let loc_path = location.uri.to_file_path().unwrap();
            assert!(
                loc_path.ends_with("src/Decoder.php"),
                "Should resolve to Decoder.php, got: {:?}",
                loc_path
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}
