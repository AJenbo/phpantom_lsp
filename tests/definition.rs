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
