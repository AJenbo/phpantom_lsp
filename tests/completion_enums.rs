mod common;

use common::{create_psr4_workspace, create_test_backend};

use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Basic enum case completion via :: ──────────────────────────────────────

/// Test: Completing on `EnumName::` should show enum cases as constants.
#[tokio::test]
async fn test_completion_enum_cases_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_basic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "enum CustomerAvailabilityStatus: int\n",
        "{\n",
        "    case CUSTOMER_NOT_IN_AUDIENCE = -1;\n",
        "    case AVAILABLE_TO_CUSTOMER = 0;\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    public function test(): void {\n",
        "        CustomerAvailabilityStatus::\n",
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
                    character: 36,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"CUSTOMER_NOT_IN_AUDIENCE"),
                "Should include enum case 'CUSTOMER_NOT_IN_AUDIENCE', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"AVAILABLE_TO_CUSTOMER"),
                "Should include enum case 'AVAILABLE_TO_CUSTOMER', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Unit enum completion ───────────────────────────────────────────────────

/// Test: Completing on a unit enum (no backing type) should show cases.
#[tokio::test]
async fn test_completion_unit_enum_cases() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///unit_enum.php").unwrap();
    let text = concat!(
        "<?php\n",
        "enum Color\n",
        "{\n",
        "    case Red;\n",
        "    case Green;\n",
        "    case Blue;\n",
        "}\n",
        "\n",
        "class Painter {\n",
        "    public function test(): void {\n",
        "        Color::\n",
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

    assert!(result.is_some(), "Completion should return results");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"Red"),
                "Should include enum case 'Red', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Green"),
                "Should include enum case 'Green', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Blue"),
                "Should include enum case 'Blue', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Enum with methods ──────────────────────────────────────────────────────

/// Test: Completing on an enum via `::` should show cases (as constants) and
/// static methods.  Instance methods are only shown via `->` access.
#[tokio::test]
async fn test_completion_enum_cases_and_static_methods_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_methods.php").unwrap();
    let text = concat!(
        "<?php\n",
        "enum Suit: string\n",
        "{\n",
        "    case Hearts = 'H';\n",
        "    case Diamonds = 'D';\n",
        "    case Clubs = 'C';\n",
        "    case Spades = 'S';\n",
        "\n",
        "    public function color(): string\n",
        "    {\n",
        "        return 'red';\n",
        "    }\n",
        "\n",
        "    public static function fromSymbol(string $s): self\n",
        "    {\n",
        "        return self::Hearts;\n",
        "    }\n",
        "}\n",
        "\n",
        "class Game {\n",
        "    public function test(): void {\n",
        "        Suit::\n",
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
                    line: 21,
                    character: 14,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"Hearts"),
                "Should include enum case 'Hearts', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Spades"),
                "Should include enum case 'Spades', got: {:?}",
                constant_names
            );
            assert!(
                method_names.contains(&"fromSymbol"),
                "Should include static method 'fromSymbol', got: {:?}",
                method_names
            );
            // Instance methods should NOT appear via `::` access
            assert!(
                !method_names.contains(&"color"),
                "Should NOT include instance method 'color' via '::', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: Completing on `$this->` inside an enum method should show the
/// enum's own instance methods.
#[tokio::test]
async fn test_completion_enum_instance_methods_via_arrow() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_arrow.php").unwrap();
    let text = concat!(
        "<?php\n",                               // 0
        "enum Suit: string\n",                   // 1
        "{\n",                                   // 2
        "    case Hearts = 'H';\n",              // 3
        "    case Spades = 'S';\n",              // 4
        "\n",                                    // 5
        "    public function color(): string\n", // 6
        "    {\n",                               // 7
        "        return 'red';\n",               // 8
        "    }\n",                               // 9
        "\n",                                    // 10
        "    public function isRed(): bool\n",   // 11
        "    {\n",                               // 12
        "        return true;\n",                // 13
        "    }\n",                               // 14
        "\n",                                    // 15
        "    public function test(): void\n",    // 16
        "    {\n",                               // 17
        "        $this->\n",                     // 18
        "    }\n",                               // 19
        "}\n",                                   // 20
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
                method_names.contains(&"color"),
                "Should include instance method 'color' via '->', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"isRed"),
                "Should include instance method 'isRed' via '->', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Enum with real constants and cases ─────────────────────────────────────

/// Test: Enum with both `const` declarations and `case` declarations should
/// show all of them as constants in completion.
#[tokio::test]
async fn test_completion_enum_mixed_constants_and_cases() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_mixed.php").unwrap();
    let text = concat!(
        "<?php\n",
        "enum Status: int\n",
        "{\n",
        "    const DEFAULT_STATUS = 0;\n",
        "    case Active = 1;\n",
        "    case Inactive = 2;\n",
        "}\n",
        "\n",
        "class Handler {\n",
        "    public function test(): void {\n",
        "        Status::\n",
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
                    character: 16,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"DEFAULT_STATUS"),
                "Should include real constant 'DEFAULT_STATUS', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Active"),
                "Should include enum case 'Active', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Inactive"),
                "Should include enum case 'Inactive', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Goto definition: enum case ─────────────────────────────────────────────

/// Test: Clicking on `Status::Active` should jump to the `case Active` line.
#[tokio::test]
async fn test_goto_definition_enum_case_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                              // 0
        "enum Status: int\n",                   // 1
        "{\n",                                  // 2
        "    case Active = 1;\n",               // 3
        "    case Inactive = 2;\n",             // 4
        "}\n",                                  // 5
        "\n",                                   // 6
        "class Service {\n",                    // 7
        "    public function test(): void {\n", // 8
        "        $s = Status::Active;\n",       // 9
        "    }\n",                              // 10
        "}\n",                                  // 11
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

    // Click on "Active" in `Status::Active` on line 9
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve goto-definition for enum case"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 3,
                "case Active is declared on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Test: Goto-definition on a real `const` inside an enum still works.
#[tokio::test]
async fn test_goto_definition_enum_const_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_const_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                                // 0
        "enum Status: int\n",                     // 1
        "{\n",                                    // 2
        "    const DEFAULT_STATUS = 0;\n",        // 3
        "    case Active = 1;\n",                 // 4
        "    case Inactive = 2;\n",               // 5
        "}\n",                                    // 6
        "\n",                                     // 7
        "class Service {\n",                      // 8
        "    public function test(): void {\n",   // 9
        "        $d = Status::DEFAULT_STATUS;\n", // 10
        "    }\n",                                // 11
        "}\n",                                    // 12
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

    // Click on "DEFAULT_STATUS" in `Status::DEFAULT_STATUS` on line 10
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 10,
                character: 25,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve goto-definition for enum const"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 3,
                "const DEFAULT_STATUS is declared on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Test: Goto-definition on an enum method via `$this->` inside the enum
/// should jump to the method declaration.
#[tokio::test]
async fn test_goto_definition_enum_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_method_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                               // 0
        "enum Suit: string\n",                   // 1
        "{\n",                                   // 2
        "    case Hearts = 'H';\n",              // 3
        "    case Spades = 'S';\n",              // 4
        "\n",                                    // 5
        "    public function color(): string\n", // 6
        "    {\n",                               // 7
        "        return 'red';\n",               // 8
        "    }\n",                               // 9
        "\n",                                    // 10
        "    public function test(): void\n",    // 11
        "    {\n",                               // 12
        "        $this->color();\n",             // 13
        "    }\n",                               // 14
        "}\n",                                   // 15
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

    // Click on "color" in `$this->color()` on line 13
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 13,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve goto-definition for enum method"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 6,
                "method color() is declared on line 6"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Cross-file enum resolution (PSR-4) ─────────────────────────────────────

/// Test: Completing on an enum from another file via PSR-4 autoloading.
#[tokio::test]
async fn test_completion_enum_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\Enums\\": "src/Enums/"
            }
        }
    }"#;

    let enum_content = concat!(
        "<?php\n",
        "namespace App\\Enums;\n",
        "\n",
        "enum Priority: int\n",
        "{\n",
        "    case Low = 0;\n",
        "    case Medium = 1;\n",
        "    case High = 2;\n",
        "    case Critical = 3;\n",
        "\n",
        "    public static function fromValue(int $v): self\n",
        "    {\n",
        "        return self::Low;\n",
        "    }\n",
        "}\n",
    );

    let (backend, dir) =
        create_psr4_workspace(composer_json, &[("src/Enums/Priority.php", enum_content)]);

    let main_uri = Url::from_file_path(dir.path().join("main.php")).unwrap();
    let main_text = concat!(
        "<?php\n",
        "use App\\Enums\\Priority;\n",
        "\n",
        "class TaskService {\n",
        "    public function test(): void {\n",
        "        Priority::\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: main_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: main_text.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: main_uri },
                position: Position {
                    line: 5,
                    character: 18,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"Low"),
                "Should include enum case 'Low', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"High"),
                "Should include enum case 'High', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Critical"),
                "Should include enum case 'Critical', got: {:?}",
                constant_names
            );
            assert!(
                method_names.contains(&"fromValue"),
                "Should include static method 'fromValue', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: Goto-definition on an enum case from another file via PSR-4.
#[tokio::test]
async fn test_goto_definition_enum_case_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\Enums\\": "src/Enums/"
            }
        }
    }"#;

    let enum_content = concat!(
        "<?php\n",                 // 0
        "namespace App\\Enums;\n", // 1
        "\n",                      // 2
        "enum Direction\n",        // 3
        "{\n",                     // 4
        "    case North;\n",       // 5
        "    case South;\n",       // 6
        "    case East;\n",        // 7
        "    case West;\n",        // 8
        "}\n",                     // 9
    );

    let (backend, dir) =
        create_psr4_workspace(composer_json, &[("src/Enums/Direction.php", enum_content)]);

    let main_uri = Url::from_file_path(dir.path().join("main.php")).unwrap();
    let main_text = concat!(
        "<?php\n",
        "use App\\Enums\\Direction;\n",
        "\n",
        "class Navigator {\n",
        "    public function test(): void {\n",
        "        $d = Direction::North;\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: main_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: main_text.to_string(),
            },
        })
        .await;

    let enum_uri = Url::from_file_path(dir.path().join("src/Enums/Direction.php")).unwrap();

    // Click on "North" in `Direction::North` on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: main_uri.clone(),
            },
            position: Position {
                line: 5,
                character: 26,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve goto-definition for cross-file enum case"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, enum_uri, "Should jump to the enum file");
            assert_eq!(
                location.range.start.line, 5,
                "case North is declared on line 5"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Enum inside namespace (same file) ──────────────────────────────────────

/// Test: Completing on an enum defined inside a namespace in the same file.
#[tokio::test]
async fn test_completion_enum_in_namespace_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_ns.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Enums;\n",
        "\n",
        "enum Visibility\n",
        "{\n",
        "    case Published;\n",
        "    case Draft;\n",
        "    case Archived;\n",
        "}\n",
        "\n",
        "class ContentService {\n",
        "    public function test(): void {\n",
        "        Visibility::\n",
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
                    character: 21,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"Published"),
                "Should include 'Published', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Draft"),
                "Should include 'Draft', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Archived"),
                "Should include 'Archived', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Enum with trait use ────────────────────────────────────────────────────

/// Test: An enum that uses a trait should expose the trait's cases via `::`
/// and the trait's instance methods via `->`.
#[tokio::test]
async fn test_completion_enum_with_trait_cases_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_trait.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait HasDescription {\n",
        "    public function describe(): string { return ''; }\n",
        "}\n",
        "\n",
        "enum Size\n",
        "{\n",
        "    use HasDescription;\n",
        "\n",
        "    case Small;\n",
        "    case Medium;\n",
        "    case Large;\n",
        "}\n",
        "\n",
        "class Shop {\n",
        "    public function test(): void {\n",
        "        Size::\n",
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
                    line: 16,
                    character: 14,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"Small"),
                "Should include enum case 'Small', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Medium"),
                "Should include enum case 'Medium', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Large"),
                "Should include enum case 'Large', got: {:?}",
                constant_names
            );
            // Instance method `describe` from the trait should NOT appear
            // via `::` access — it's only accessible on instances via `->`.
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            assert!(
                !method_names.contains(&"describe"),
                "Instance method 'describe' should NOT appear via '::', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Enum implements interface ──────────────────────────────────────────────

/// Test: Enums implementing an interface should still parse correctly and
/// show their cases via `::`.
#[tokio::test]
async fn test_completion_enum_implements_interface() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_iface.php").unwrap();
    let text = concat!(
        "<?php\n",
        "interface HasLabel {\n",
        "    public function label(): string;\n",
        "}\n",
        "\n",
        "enum Fruit: string implements HasLabel\n",
        "{\n",
        "    case Apple = 'apple';\n",
        "    case Banana = 'banana';\n",
        "    case Cherry = 'cherry';\n",
        "\n",
        "    public function label(): string\n",
        "    {\n",
        "        return ucfirst($this->value);\n",
        "    }\n",
        "}\n",
        "\n",
        "class FruitStand {\n",
        "    public function test(): void {\n",
        "        Fruit::\n",
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
                    line: 19,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"Apple"),
                "Should include enum case 'Apple', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Cherry"),
                "Should include enum case 'Cherry', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Goto-definition for the enum name itself ───────────────────────────────

/// Test: Clicking on an enum name (e.g., `Status` in `Status::Active`)
/// should jump to the enum definition.
#[tokio::test]
async fn test_goto_definition_enum_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_name_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                              // 0
        "enum Status: int\n",                   // 1
        "{\n",                                  // 2
        "    case Active = 1;\n",               // 3
        "    case Inactive = 2;\n",             // 4
        "}\n",                                  // 5
        "\n",                                   // 6
        "class Service {\n",                    // 7
        "    public function test(): void {\n", // 8
        "        $s = Status::Active;\n",       // 9
        "    }\n",                              // 10
        "}\n",                                  // 11
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

    // Click on "Status" in `Status::Active` on line 9
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve goto-definition for enum name"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "enum Status is declared on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── self:: inside enum ─────────────────────────────────────────────────────

/// Test: Completing on `self::` inside an enum method should show the
/// enum's own cases and static methods.
#[tokio::test]
async fn test_completion_self_inside_enum() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///enum_self.php").unwrap();
    let text = concat!(
        "<?php\n",
        "enum Priority: int\n",
        "{\n",
        "    case Low = 0;\n",
        "    case Medium = 1;\n",
        "    case High = 2;\n",
        "\n",
        "    public function isUrgent(): bool\n",
        "    {\n",
        "        return $this === self::\n",
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
                    character: 31,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                constant_names.contains(&"Low"),
                "Should include enum case 'Low', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"Medium"),
                "Should include enum case 'Medium', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"High"),
                "Should include enum case 'High', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Implicit UnitEnum / BackedEnum interface inheritance ───────────────────

/// When a UnitEnum stub is available, a unit enum should inherit its methods
/// (e.g. `cases()`) via the implicit interface added to `used_traits`.
#[tokio::test]
async fn test_completion_unit_enum_inherits_cases_from_stub() {
    let backend = create_test_backend();

    // Open the UnitEnum stub so it lands in the ast_map.
    let stub_uri = Url::parse("file:///stubs/UnitEnum.php").unwrap();
    let unit_enum_stub = concat!(
        "<?php\n",
        "interface UnitEnum\n",
        "{\n",
        "    public static function cases(): array;\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: stub_uri,
                language_id: "php".to_string(),
                version: 1,
                text: unit_enum_stub.to_string(),
            },
        })
        .await;

    // Open a file containing a unit enum and a class that uses it.
    let uri = Url::parse("file:///enum_stub_unit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "enum Color\n",
        "{\n",
        "    case Red;\n",
        "    case Green;\n",
        "    case Blue;\n",
        "}\n",
        "\n",
        "class Palette {\n",
        "    public function test(): void {\n",
        "        Color::\n",
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

    assert!(result.is_some(), "Completion should return results");
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
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"cases"),
                "Unit enum should inherit 'cases()' from UnitEnum stub, got methods: {:?}",
                method_names
            );
            assert!(
                constant_names.contains(&"Red"),
                "Should still include enum case 'Red', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When a BackedEnum stub is available, a backed enum should inherit its
/// methods (e.g. `from()`, `tryFrom()`, `cases()`) via the implicit
/// interface added to `used_traits`.
#[tokio::test]
async fn test_completion_backed_enum_inherits_from_and_tryfrom_from_stub() {
    let backend = create_test_backend();

    // Open the BackedEnum stub so it lands in the ast_map.
    let stub_uri = Url::parse("file:///stubs/BackedEnum.php").unwrap();
    let backed_enum_stub = concat!(
        "<?php\n",
        "interface BackedEnum\n",
        "{\n",
        "    public static function from(int|string $value): static;\n",
        "    public static function tryFrom(int|string $value): ?static;\n",
        "    public static function cases(): array;\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: stub_uri,
                language_id: "php".to_string(),
                version: 1,
                text: backed_enum_stub.to_string(),
            },
        })
        .await;

    // Open a file containing a backed enum and a class that uses it.
    let uri = Url::parse("file:///enum_stub_backed.php").unwrap();
    let text = concat!(
        "<?php\n",
        "enum Priority: int\n",
        "{\n",
        "    case Low = 0;\n",
        "    case Medium = 1;\n",
        "    case High = 2;\n",
        "}\n",
        "\n",
        "class TaskService {\n",
        "    public function test(): void {\n",
        "        Priority::\n",
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
                    character: 18,
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
            let constant_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"from"),
                "Backed enum should inherit 'from()' from BackedEnum stub, got methods: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"tryFrom"),
                "Backed enum should inherit 'tryFrom()' from BackedEnum stub, got methods: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"cases"),
                "Backed enum should inherit 'cases()' from BackedEnum stub, got methods: {:?}",
                method_names
            );
            assert!(
                constant_names.contains(&"Low"),
                "Should still include enum case 'Low', got: {:?}",
                constant_names
            );
            assert!(
                constant_names.contains(&"High"),
                "Should still include enum case 'High', got: {:?}",
                constant_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Parser-level check: a unit enum should have `\UnitEnum` in used_traits,
/// and a backed enum should have `\BackedEnum`.
#[tokio::test]
async fn test_parser_enum_implicit_interface_in_used_traits() {
    let backend = create_test_backend();

    // Unit enum
    let unit_php = concat!(
        "<?php\n",
        "enum Direction\n",
        "{\n",
        "    case Up;\n",
        "    case Down;\n",
        "}\n",
    );
    let unit_classes = backend.parse_php(unit_php);
    assert_eq!(unit_classes.len(), 1);
    assert!(
        unit_classes[0]
            .used_traits
            .iter()
            .any(|t| t == "\\UnitEnum"),
        "Unit enum should have \\UnitEnum in used_traits, got: {:?}",
        unit_classes[0].used_traits
    );
    assert!(
        !unit_classes[0]
            .used_traits
            .iter()
            .any(|t| t == "\\BackedEnum"),
        "Unit enum should NOT have \\BackedEnum, got: {:?}",
        unit_classes[0].used_traits
    );

    // Backed enum (int)
    let backed_php = concat!(
        "<?php\n",
        "enum Status: int\n",
        "{\n",
        "    case Active = 1;\n",
        "    case Inactive = 0;\n",
        "}\n",
    );
    let backed_classes = backend.parse_php(backed_php);
    assert_eq!(backed_classes.len(), 1);
    assert!(
        backed_classes[0]
            .used_traits
            .iter()
            .any(|t| t == "\\BackedEnum"),
        "Backed enum should have \\BackedEnum in used_traits, got: {:?}",
        backed_classes[0].used_traits
    );
    assert!(
        !backed_classes[0]
            .used_traits
            .iter()
            .any(|t| t == "\\UnitEnum"),
        "Backed enum should NOT have \\UnitEnum, got: {:?}",
        backed_classes[0].used_traits
    );

    // Backed enum (string)
    let string_php = concat!(
        "<?php\n",
        "enum Suit: string\n",
        "{\n",
        "    case Hearts = 'H';\n",
        "}\n",
    );
    let string_classes = backend.parse_php(string_php);
    assert_eq!(string_classes.len(), 1);
    assert!(
        string_classes[0]
            .used_traits
            .iter()
            .any(|t| t == "\\BackedEnum"),
        "String-backed enum should have \\BackedEnum, got: {:?}",
        string_classes[0].used_traits
    );
}

/// An enum that also uses an explicit trait should have both the trait
/// and the implicit interface in `used_traits`.
#[tokio::test]
async fn test_parser_enum_with_trait_also_has_implicit_interface() {
    let backend = create_test_backend();
    let php = concat!(
        "<?php\n",
        "trait HasLabel {\n",
        "    public function label(): string { return 'label'; }\n",
        "}\n",
        "\n",
        "enum Status: int\n",
        "{\n",
        "    use HasLabel;\n",
        "\n",
        "    case Active = 1;\n",
        "    case Inactive = 0;\n",
        "}\n",
    );

    let classes = backend.parse_php(php);
    let enum_info = classes.iter().find(|c| c.name == "Status").unwrap();

    assert!(
        enum_info.used_traits.iter().any(|t| t == "HasLabel"),
        "Should include the explicit trait, got: {:?}",
        enum_info.used_traits
    );
    assert!(
        enum_info.used_traits.iter().any(|t| t == "\\BackedEnum"),
        "Should include implicit \\BackedEnum, got: {:?}",
        enum_info.used_traits
    );
}
