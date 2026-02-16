mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

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

// ── Inline @var docblock override tests ─────────────────────────────────────

/// When a variable is assigned from an unknown function but has an inline
/// `/** @var Type */` annotation, goto definition should resolve the member
/// via the annotated type.
#[tokio::test]
async fn test_goto_definition_inline_var_docblock_simple() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///varoverride.php").unwrap();
    let text = concat!(
        "<?php\n",                                  // 0
        "class Session {\n",                        // 1
        "    public function getId(): string {}\n", // 2
        "    public function flash(): void {}\n",   // 3
        "}\n",                                      // 4
        "class Controller {\n",                     // 5
        "    public function handle() {\n",         // 6
        "        /** @var Session */\n",            // 7
        "        $sess = mystery();\n",             // 8
        "        $sess->getId();\n",                // 9
        "    }\n",                                  // 10
        "}\n",                                      // 11
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

    // Click on "getId" on line 9: $sess->getId()
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 16,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $sess->getId() via @var Session annotation"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "getId is declared on line 2 in Session"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// When the `@var` annotation includes a variable name (`@var Type $var`),
/// goto definition should still resolve correctly.
#[tokio::test]
async fn test_goto_definition_inline_var_docblock_with_variable_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///varoverride_named.php").unwrap();
    let text = concat!(
        "<?php\n",                                // 0
        "class Logger {\n",                       // 1
        "    public function info(): void {}\n",  // 2
        "    public function error(): void {}\n", // 3
        "}\n",                                    // 4
        "class App {\n",                          // 5
        "    public function run() {\n",          // 6
        "        /** @var Logger $log */\n",      // 7
        "        $log = getLogger();\n",          // 8
        "        $log->error();\n",               // 9
        "    }\n",                                // 10
        "}\n",                                    // 11
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

    // Click on "error" on line 9: $log->error()
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
        "Should resolve $log->error() via @var Logger $log annotation"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 3,
                "error() is declared on line 3 in Logger"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// When the `@var` annotation names a *different* variable, the override
/// should NOT apply and goto definition should fail (no type info).
#[tokio::test]
async fn test_goto_definition_inline_var_docblock_wrong_variable_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///varoverride_wrong.php").unwrap();
    let text = concat!(
        "<?php\n",                               // 0
        "class Logger {\n",                      // 1
        "    public function info(): void {}\n", // 2
        "}\n",                                   // 3
        "class App {\n",                         // 4
        "    public function run() {\n",         // 5
        "        /** @var Logger $other */\n",   // 6
        "        $log = something();\n",         // 7
        "        $log->info();\n",               // 8
        "    }\n",                               // 9
        "}\n",                                   // 10
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

    // Click on "info" on line 8: $log->info()
    // The @var names $other, not $log, so no type resolution should occur.
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 8,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "Should NOT resolve $log->info() when @var names $other"
    );
}

/// When the native return type is a scalar (string), the `@var` override
/// should be blocked (same check as @return) and definition should fall
/// through to normal resolution (which won't find a class for `string`).
#[tokio::test]
async fn test_goto_definition_inline_var_docblock_blocked_by_scalar() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///varoverride_scalar.php").unwrap();
    let text = concat!(
        "<?php\n",                                    // 0
        "class Session {\n",                          // 1
        "    public function getId(): string {}\n",   // 2
        "}\n",                                        // 3
        "class App {\n",                              // 4
        "    public function getName(): string {}\n", // 5
        "    public function run() {\n",              // 6
        "        /** @var Session */\n",              // 7
        "        $s = $this->getName();\n",           // 8
        "        $s->getId();\n",                     // 9
        "    }\n",                                    // 10
        "}\n",                                        // 11
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

    // Click on "getId" on line 9: $s->getId()
    // getName() returns `string` — override should be blocked.
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 9,
                character: 13,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "Should NOT resolve $s->getId() when native type is scalar string"
    );
}

/// When the native return type is a class (non-scalar), the `@var` override
/// should be allowed and goto definition should resolve to the overridden type.
#[tokio::test]
async fn test_goto_definition_inline_var_docblock_override_allowed_for_object() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///varoverride_obj.php").unwrap();
    let text = concat!(
        "<?php\n",                                            // 0
        "class BaseService {\n",                              // 1
        "    public function base(): void {}\n",              // 2
        "}\n",                                                // 3
        "class Session extends BaseService {\n",              // 4
        "    public function getId(): string {}\n",           // 5
        "    public function flash(): void {}\n",             // 6
        "}\n",                                                // 7
        "class App {\n",                                      // 8
        "    public function getService(): BaseService {}\n", // 9
        "    public function run() {\n",                      // 10
        "        /** @var Session */\n",                      // 11
        "        $s = $this->getService();\n",                // 12
        "        $s->flash();\n",                             // 13
        "    }\n",                                            // 14
        "}\n",                                                // 15
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

    // Click on "flash" on line 13: $s->flash()
    // getService() returns BaseService but @var says Session — override allowed.
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 13,
                character: 13,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $s->flash() via @var Session override"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 6,
                "flash() is declared on line 6 in Session"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

// ─── Variable Go-To-Definition ─────────────────────────────────────────────

/// Clicking on `$typed` in `return $typed;` should jump to the assignment
/// on the previous line: `$typed = getUnknownValue();`.
#[tokio::test]
async fn test_goto_definition_variable_jumps_to_assignment() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_assign.php").unwrap();
    let text = concat!(
        "<?php\n",                           // 0
        "function demo(): mixed {\n",        // 1
        "    $typed = getUnknownValue();\n", // 2
        "    return $typed;\n",              // 3
        "}\n",                               // 4
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

    // Cursor on `$typed` in `return $typed;` (line 3, character 12 = on 't')
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 3,
                character: 12,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $typed to its assignment on the previous line"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 2, "$typed is assigned on line 2");
            assert_eq!(
                location.range.start.character, 4,
                "$typed starts at column 4"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// When the cursor is already on the definition line (the assignment),
/// go-to-definition should return None — the user is already there.
#[tokio::test]
async fn test_goto_definition_variable_on_definition_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_on_def.php").unwrap();
    let text = concat!(
        "<?php\n",                           // 0
        "function demo(): void {\n",         // 1
        "    $typed = getUnknownValue();\n", // 2
        "}\n",                               // 3
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

    // Cursor on `$typed` on line 2 (the assignment itself)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 2,
                character: 5,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_none(),
        "Should return None when cursor is already on the definition"
    );
}

/// Go-to-definition on a variable should jump to a function parameter
/// declaration when that is the most recent definition.
#[tokio::test]
async fn test_goto_definition_variable_jumps_to_parameter() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_param.php").unwrap();
    let text = concat!(
        "<?php\n",                                       // 0
        "class App {\n",                                 // 1
        "    public function handle(int $id): void {\n", // 2
        "        echo $id;\n",                           // 3
        "    }\n",                                       // 4
        "}\n",                                           // 5
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

    // Cursor on `$id` in `echo $id;` (line 3)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 3,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $id to the parameter declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 2,
                "$id is declared as a parameter on line 2"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Go-to-definition on a variable used after a foreach should jump to the
/// foreach `as $var` declaration.
#[tokio::test]
async fn test_goto_definition_variable_jumps_to_foreach() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_foreach.php").unwrap();
    let text = concat!(
        "<?php\n",                           // 0
        "function demo(): void {\n",         // 1
        "    $items = [1, 2, 3];\n",         // 2
        "    foreach ($items as $item) {\n", // 3
        "        echo $item;\n",             // 4
        "    }\n",                           // 5
        "}\n",                               // 6
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

    // Cursor on `$item` in `echo $item;` (line 4)
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
        "Should resolve $item to the foreach declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 3,
                "$item is declared in the foreach on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// When a variable is reassigned, go-to-definition should jump to the
/// most recent assignment before the cursor.
#[tokio::test]
async fn test_goto_definition_variable_jumps_to_most_recent_reassignment() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_reassign.php").unwrap();
    let text = concat!(
        "<?php\n",                   // 0
        "function demo(): void {\n", // 1
        "    $val = 1;\n",           // 2
        "    $val = 2;\n",           // 3
        "    $val = 3;\n",           // 4
        "    echo $val;\n",          // 5
        "}\n",                       // 6
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

    // Cursor on `$val` in `echo $val;` (line 5)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 10,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $val to the most recent assignment"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 4,
                "$val's most recent assignment is on line 4"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Go-to-definition on `$e` in a catch block should jump to the catch
/// declaration.
#[tokio::test]
async fn test_goto_definition_variable_jumps_to_catch() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_catch.php").unwrap();
    let text = concat!(
        "<?php\n",                          // 0
        "function demo(): void {\n",        // 1
        "    try {\n",                      // 2
        "        riskyOperation();\n",      // 3
        "    } catch (\\Exception $e) {\n", // 4
        "        echo $e;\n",               // 5
        "    }\n",                          // 6
        "}\n",                              // 7
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

    // Cursor on `$e` in `echo $e;` (line 5)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 5,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $e to the catch declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 4,
                "$e is declared in the catch on line 4"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Go-to-definition on a variable at the top level (outside any class)
/// should still resolve to its assignment.
#[tokio::test]
async fn test_goto_definition_variable_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_toplevel.php").unwrap();
    let text = concat!(
        "<?php\n",                       // 0
        "$typed = getUnknownValue();\n", // 1
        "return $typed;\n",              // 2
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

    // Cursor on `$typed` in `return $typed;` (line 2)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 2,
                character: 8,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve $typed to assignment on line 1"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(location.range.start.line, 1, "$typed is assigned on line 1");
            assert_eq!(
                location.range.start.character, 0,
                "$typed starts at column 0"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Clicking on `$val` in a foreach key-value `=> $val` should jump
/// to the foreach declaration.
#[tokio::test]
async fn test_goto_definition_variable_foreach_key_value() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_goto_foreach_kv.php").unwrap();
    let text = concat!(
        "<?php\n",                                // 0
        "function demo(): void {\n",              // 1
        "    $map = ['a' => 1];\n",               // 2
        "    foreach ($map as $key => $val) {\n", // 3
        "        echo $val;\n",                   // 4
        "    }\n",                                // 5
        "}\n",                                    // 6
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

    // Cursor on `$val` in `echo $val;` (line 4)
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
        "Should resolve $val to the foreach key-value declaration"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 3,
                "$val is declared in the foreach on line 3"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}
