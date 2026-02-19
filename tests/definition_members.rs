mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

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

// ─── Property vs Method Disambiguation ──────────────────────────────────────

/// When a class has both a property `$id` and a method `id()`, goto-definition
/// on `$user->id` (no parentheses) should navigate to the *property*, not the
/// method.
#[tokio::test]
async fn test_goto_definition_property_preferred_over_method_without_parens() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    //                                           line
    let text = concat!(
        "<?php\n",                              // 0
        "class User {\n",                       // 1
        "    public int $id;\n",                // 2  ← property declaration
        "    public string $name;\n",           // 3
        "\n",                                   // 4
        "    public function id(): int {\n",    // 5  ← method declaration
        "        return $this->id;\n",          // 6
        "    }\n",                              // 7
        "\n",                                   // 8
        "    public function test(): void {\n", // 9
        "        $user = new User();\n",        // 10
        "        $val = $user->id;\n",          // 11 ← property access (no parens)
        "        $val2 = $user->id();\n",       // 12 ← method call (with parens)
        "    }\n",                              // 13
        "}\n",                                  // 14
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

    // ── Case 1: `$user->id` (no parens) → should go to the $id property on line 2
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 11,
                character: 24, // on "id" in `$user->id`
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $user->id to the property declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "$user->id (no parens) should go to the $id property on line 2, not the id() method on line 5"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // ── Case 2: `$user->id()` (with parens) → should go to the id() method on line 5
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 12,
                character: 25, // on "id" in `$user->id()`
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $user->id() to the method declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 5,
                "$user->id() (with parens) should go to the id() method on line 5, not the $id property on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Same disambiguation but via `$this->` inside the class itself.
#[tokio::test]
async fn test_goto_definition_this_property_vs_method_disambiguation() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                 // 0
        "class Order {\n",                         // 1
        "    public float $total;\n",              // 2  ← property
        "\n",                                      // 3
        "    public function total(): float {\n",  // 4  ← method
        "        return $this->total;\n",          // 5  ← property access
        "    }\n",                                 // 6
        "\n",                                      // 7
        "    public function display(): void {\n", // 8
        "        echo $this->total;\n",            // 9  ← property access
        "        echo $this->total();\n",          // 10 ← method call
        "    }\n",                                 // 11
        "}\n",                                     // 12
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

    // `$this->total` on line 9 (no parens) → property on line 2
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 23, // on "total" in `$this->total`
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(
                location.range.start.line, 2,
                "$this->total (no parens) should go to the $total property on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }

    // `$this->total()` on line 10 (with parens) → method on line 4
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 10,
                character: 23, // on "total" in `$this->total()`
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(
                location.range.start.line, 4,
                "$this->total() (with parens) should go to the total() method on line 4"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ── @method tag: method name matches a type keyword ─────────────────────────

#[tokio::test]
async fn test_goto_definition_method_tag_name_matches_type_keyword() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_string.php").unwrap();
    let text = concat!(
        "<?php\n", // 0
        "/**\n",   // 1
        " * @method static string string(string $key, \\Closure|string|null $default = null)\n", // 2
        " */\n",                      // 3
        "class Config {\n",           // 4
        "}\n",                        // 5
        "\n",                         // 6
        "Config::string('hello');\n", // 7
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

    // Click on "string" in `Config::string('hello')` on line 7, character 8
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 7,
                character: 10,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve Config::string() to the @method tag declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "@method string string(...) is declared on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_goto_definition_method_on_static_call_with_nested_call_arg() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///test_nested_arg.php").unwrap();
    let text = concat!(
        "<?php\n",                                                                     // 0
        "class Country {}\n",                                                          // 1
        "\n",                                                                          // 2
        "class SettingsProvider {\n",                                                  // 3
        "    public function get(string $key): string { return ''; }\n",               // 4
        "}\n",                                                                         // 5
        "\n",                                                                          // 6
        "class Environment {\n",                                                       // 7
        "    public static function get(Country $env): self { return new self(); }\n", // 8
        "    public function settings(): SettingsProvider { return new SettingsProvider(); }\n", // 9
        "}\n",                                                                       // 10
        "\n",                                                                        // 11
        "class CurrentEnvironment {\n",                                              // 12
        "    public static function country(): Country { return new Country(); }\n", // 13
        "    public static function settings(): SettingsProvider {\n",               // 14
        "        return Environment::get(self::country())->settings();\n",           // 15
        "    }\n",                                                                   // 16
        "}\n",                                                                       // 17
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

    // Click on "settings" in `Environment::get(self::country())->settings()` on line 15
    // "settings" starts at character 50
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 15,
                character: 52,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve ->settings() after Environment::get(self::country()) to its declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 9,
                "Environment::settings() is declared on line 9"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Regression: member access must not fall through to standalone function ──

/// When the cursor is on `map` in `collect($x)->map(`, go-to-definition
/// must NOT resolve `map` as a standalone function.  If the owning class
/// can be determined, it should jump to `Collection::map()`.  If it
/// cannot (e.g. `collect` isn't indexed), the result should be `None` —
/// never a fallback to a global `map()` function.
///
/// This is the general pattern: any word on the right side of `->`,
/// `?->`, or `::` is a *member name*, not a standalone symbol.
#[tokio::test]
async fn test_goto_definition_member_does_not_fallthrough_to_function() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///fallthrough.php").unwrap();
    // `map` exists both as a method on Collection AND as a standalone
    // function.  Go-to-definition on the `->map` call must resolve to
    // the method, not the function.
    let text = concat!(
        "<?php\n",
        "class Collection {\n",
        "    public function map(callable $cb): static {}\n",
        "    public function values(): static {}\n",
        "}\n",
        "\n",
        "/** @return Collection */\n",
        "function collect($v): Collection { return new Collection(); }\n",
        "\n",
        "/** Standalone map function — must NOT be the target. */\n",
        "function map(array $arr, callable $cb): array { return []; }\n",
        "\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $result = collect([])->map(fn ($x) => $x);\n",
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

    // Click on `map` in `collect([])->map(` on line 14
    // Line 14: "        $result = collect([])->map(fn ($x) => $x);\n"
    //                                         ^ cursor on 'm' of 'map'
    let line_text = "        $result = collect([])->map(fn ($x) => $x);";
    let map_col = line_text.find("->map(").unwrap() + 2; // position of 'm'

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 14,
                character: map_col as u32,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    match result {
        Some(GotoDefinitionResponse::Scalar(location)) => {
            // Must point to Collection::map on line 2, NOT the standalone
            // `function map(...)` on line 10.
            assert_eq!(
                location.range.start.line, 2,
                "Expected Collection::map on line 2, got line {}",
                location.range.start.line,
            );
        }
        None => {
            // Acceptable: subject resolution couldn't find Collection.
            // The important thing is that we did NOT jump to the
            // standalone `map()` function on line 10.
        }
        other => panic!(
            "Expected Scalar location pointing to Collection::map or None, got: {:?}",
            other
        ),
    }
}

/// When the owning class truly can't be resolved (unknown function in the
/// chain), go-to-definition on the member should return None — not jump
/// to a standalone function with the same name.
#[tokio::test]
async fn test_goto_definition_unresolvable_member_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///unresolvable.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** Standalone values function — must NOT be the target. */\n",
        "function values(): array { return []; }\n",
        "\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        unknown_function()->values();\n",
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

    // Click on `values` in `unknown_function()->values()` on line 6
    let line_text = "        unknown_function()->values();";
    let val_col = line_text.find("->values(").unwrap() + 2; // position of 'v'

    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 6,
                character: val_col as u32,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    // Must be None — we must NOT fall through to the standalone `values()`.
    assert!(
        result.is_none(),
        "Expected None when owning class is unresolvable, but got: {:?}. \
         This means the member name fell through to standalone function lookup.",
        result,
    );
}
