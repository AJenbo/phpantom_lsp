mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Same-File Constant Go-to-Definition ────────────────────────────────────

/// Clicking on a constant name used in an expression should jump to the
/// `define('CONSTANT_NAME', ...)` call in the same file.
#[tokio::test]
async fn test_goto_definition_constant_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///constants.php").unwrap();
    let text = concat!(
        "<?php\n",                           // 0
        "define('APP_VERSION', '1.0.0');\n", // 1
        "define('APP_NAME', 'PHPantom');\n", // 2
        "\n",                                // 3
        "echo APP_VERSION;\n",               // 4
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

    // Click on "APP_VERSION" on line 4
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 4,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve constant APP_VERSION to its define() call"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "APP_VERSION is defined on line 1"
            );
            assert_eq!(
                location.range.start.character, 0,
                "define() starts at column 0"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Clicking on a different constant in the same file should jump to the
/// correct `define()` call.
#[tokio::test]
async fn test_goto_definition_constant_same_file_second_constant() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///constants2.php").unwrap();
    let text = concat!(
        "<?php\n",                           // 0
        "define('APP_VERSION', '1.0.0');\n", // 1
        "define('APP_NAME', 'PHPantom');\n", // 2
        "\n",                                // 3
        "echo APP_NAME;\n",                  // 4
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

    // Click on "APP_NAME" on line 4
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 4,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve constant APP_NAME to its define() call"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "APP_NAME is defined on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Constants defined with double-quoted strings should also be resolved.
#[tokio::test]
async fn test_goto_definition_constant_double_quoted() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_dq.php").unwrap();
    let text = concat!(
        "<?php\n",                             // 0
        "define(\"DB_HOST\", 'localhost');\n", // 1
        "\n",                                  // 2
        "echo DB_HOST;\n",                     // 3
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

    // Click on "DB_HOST" on line 3
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 3,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve double-quoted define constant"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 1, "DB_HOST is defined on line 1");
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// When `define()` is indented (e.g. inside a class method or if block),
/// the position should point at the correct column.
#[tokio::test]
async fn test_goto_definition_constant_indented_define() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_indent.php").unwrap();
    let text = concat!(
        "<?php\n",                           // 0
        "if (!defined('DEBUG_MODE')) {\n",   // 1
        "    define('DEBUG_MODE', true);\n", // 2
        "}\n",                               // 3
        "\n",                                // 4
        "echo DEBUG_MODE;\n",                // 5
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

    // Click on "DEBUG_MODE" on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(result.is_some(), "Should resolve indented define constant");

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "DEBUG_MODE is defined on line 2"
            );
            assert_eq!(
                location.range.start.character, 4,
                "define() is indented by 4 spaces"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Cross-File Constant Go-to-Definition ───────────────────────────────────

/// Clicking on a constant defined in another file (via define()) should
/// jump to that file's define() call.
#[tokio::test]
async fn test_goto_definition_constant_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[
            (
                "src/constants.php",
                concat!(
                    "<?php\n",                          // 0
                    "define('MAX_RETRIES', 3);\n",      // 1
                    "define('DEFAULT_TIMEOUT', 30);\n", // 2
                ),
            ),
            (
                "src/Service.php",
                concat!(
                    "<?php\n",                                   // 0
                    "namespace App;\n",                          // 1
                    "class Service {\n",                         // 2
                    "    public function getTimeout(): int {\n", // 3
                    "        return DEFAULT_TIMEOUT;\n",         // 4
                    "    }\n",                                   // 5
                    "}\n",                                       // 6
                ),
            ),
        ],
    );

    // Open the constants file first so define() calls are registered
    let constants_path = _dir.path().join("src/constants.php");
    let constants_uri = Url::from_file_path(&constants_path).unwrap();
    let constants_text = std::fs::read_to_string(&constants_path).unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: constants_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: constants_text,
            },
        })
        .await;

    // Open the service file
    let service_path = _dir.path().join("src/Service.php");
    let service_uri = Url::from_file_path(&service_path).unwrap();
    let service_text = std::fs::read_to_string(&service_path).unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: service_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: service_text,
            },
        })
        .await;

    // Click on "DEFAULT_TIMEOUT" on line 4 in Service.php
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: service_uri.clone(),
            },
            position: Position {
                line: 4,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve DEFAULT_TIMEOUT to its define() in constants.php"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, constants_uri, "Should jump to constants.php");
            assert_eq!(
                location.range.start.line, 2,
                "DEFAULT_TIMEOUT is defined on line 2 of constants.php"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Cross-file: jumping to the first constant in a multi-constant file.
#[tokio::test]
async fn test_goto_definition_constant_cross_file_first_constant() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[
            (
                "src/constants.php",
                concat!(
                    "<?php\n",                          // 0
                    "define('MAX_RETRIES', 3);\n",      // 1
                    "define('DEFAULT_TIMEOUT', 30);\n", // 2
                ),
            ),
            (
                "src/Worker.php",
                concat!(
                    "<?php\n",                                           // 0
                    "namespace App;\n",                                  // 1
                    "class Worker {\n",                                  // 2
                    "    public function run(): void {\n",               // 3
                    "        for ($i = 0; $i < MAX_RETRIES; $i++) {}\n", // 4
                    "    }\n",                                           // 5
                    "}\n",                                               // 6
                ),
            ),
        ],
    );

    // Open the constants file first
    let constants_path = _dir.path().join("src/constants.php");
    let constants_uri = Url::from_file_path(&constants_path).unwrap();
    let constants_text = std::fs::read_to_string(&constants_path).unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: constants_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: constants_text,
            },
        })
        .await;

    // Open the worker file
    let worker_path = _dir.path().join("src/Worker.php");
    let worker_uri = Url::from_file_path(&worker_path).unwrap();
    let worker_text = std::fs::read_to_string(&worker_path).unwrap();

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: worker_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: worker_text,
            },
        })
        .await;

    // Click on "MAX_RETRIES" on line 4 in Worker.php
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: worker_uri.clone(),
            },
            position: Position {
                line: 4,
                character: 35,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve MAX_RETRIES to its define() in constants.php"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, constants_uri, "Should jump to constants.php");
            assert_eq!(
                location.range.start.line, 1,
                "MAX_RETRIES is defined on line 1 of constants.php"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Constants used in various contexts ─────────────────────────────────────

/// Constants used in function arguments should be resolvable.
#[tokio::test]
async fn test_goto_definition_constant_in_function_arg() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_arg.php").unwrap();
    let text = concat!(
        "<?php\n",                                     // 0
        "define('LOG_LEVEL', 'info');\n",              // 1
        "\n",                                          // 2
        "function setLevel(string $level): void {}\n", // 3
        "setLevel(LOG_LEVEL);\n",                      // 4
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

    // Click on "LOG_LEVEL" on line 4, inside the function argument
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 4,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve LOG_LEVEL used as function argument"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "LOG_LEVEL is defined on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Constants used in class method bodies should be resolvable.
#[tokio::test]
async fn test_goto_definition_constant_inside_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_class.php").unwrap();
    let text = concat!(
        "<?php\n",                                      // 0
        "define('BASE_URL', 'https://example.com');\n", // 1
        "\n",                                           // 2
        "class ApiClient {\n",                          // 3
        "    public function getUrl(): string {\n",     // 4
        "        return BASE_URL . '/api';\n",          // 5
        "    }\n",                                      // 6
        "}\n",                                          // 7
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

    // Click on "BASE_URL" on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve BASE_URL used inside a class method"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "BASE_URL is defined on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// An unknown constant (not in global_defines or stubs) should return None.
#[tokio::test]
async fn test_goto_definition_constant_unknown_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_unknown.php").unwrap();
    let text = concat!(
        "<?php\n",                  // 0
        "echo UNKNOWN_CONSTANT;\n", // 1
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

    // Click on "UNKNOWN_CONSTANT" on line 1
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 1,
                character: 10,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "Unknown constant should not resolve to any definition"
    );
}

/// Constants used in array index positions should be resolvable.
#[tokio::test]
async fn test_goto_definition_constant_in_array_index() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_array.php").unwrap();
    let text = concat!(
        "<?php\n",                        // 0
        "define('KEY_NAME', 'name');\n",  // 1
        "\n",                             // 2
        "$data = ['name' => 'Alice'];\n", // 3
        "echo $data[KEY_NAME];\n",        // 4
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

    // Click on "KEY_NAME" on line 4, inside the array brackets
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 4,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve KEY_NAME used as array index"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "KEY_NAME is defined on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Multiple define() calls in a file — each constant should resolve to
/// its own define() call, not the first one.
#[tokio::test]
async fn test_goto_definition_constant_multiple_defines() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///multi_const.php").unwrap();
    let text = concat!(
        "<?php\n",                // 0
        "define('FIRST', 1);\n",  // 1
        "define('SECOND', 2);\n", // 2
        "define('THIRD', 3);\n",  // 3
        "\n",                     // 4
        "echo THIRD;\n",          // 5
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

    // Click on "THIRD" on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve THIRD to its specific define() call"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 3,
                "THIRD is defined on line 3, not line 1 or 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Constants with spaces after the opening paren: `define( 'NAME', value )`
/// should still be found.
#[tokio::test]
async fn test_goto_definition_constant_spaces_in_define() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_spaces.php").unwrap();
    let text = concat!(
        "<?php\n",                         // 0
        "define( 'SPACED_CONST', 42 );\n", // 1
        "\n",                              // 2
        "echo SPACED_CONST;\n",            // 3
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

    // Click on "SPACED_CONST" on line 3
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 3,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve define() with spaces after paren"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "SPACED_CONST is defined on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Constant used in comparison / switch ───────────────────────────────────

/// Constant used on the right side of a comparison should be resolvable.
#[tokio::test]
async fn test_goto_definition_constant_in_comparison() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///const_cmp.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "define('STATUS_ACTIVE', 1);\n",            // 1
        "define('STATUS_INACTIVE', 0);\n",          // 2
        "\n",                                       // 3
        "function isActive(int $status): bool {\n", // 4
        "    return $status === STATUS_ACTIVE;\n",  // 5
        "}\n",                                      // 6
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

    // Click on "STATUS_ACTIVE" on line 5
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
        "Should resolve STATUS_ACTIVE used in comparison"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 1,
                "STATUS_ACTIVE is defined on line 1"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}
