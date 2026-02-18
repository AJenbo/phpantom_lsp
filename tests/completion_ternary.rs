mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

/// Ternary expression with `new` instantiations: both branches contribute
/// a possible type, so the variable should show completions from both.
#[tokio::test]
async fn test_completion_ternary_new_instantiations() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_new.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Mailer {\n",
        "    public function send(): void {}\n",
        "    public function queue(): void {}\n",
        "}\n",
        "\n",
        "class NullMailer {\n",
        "    public function send(): void {}\n",
        "    public function discard(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(bool $useReal): void {\n",
        "        $mailer = $useReal ? new Mailer() : new NullMailer();\n",
        "        $mailer->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 14,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $mailer-> from ternary"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("send")),
                "Should include send (shared method), got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("queue")),
                "Should include queue from Mailer, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("discard")),
                "Should include discard from NullMailer, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Null-coalescing expression: `$var = $a ?? new Fallback()` should
/// resolve the variable to the union of both sides.
#[tokio::test]
async fn test_completion_null_coalescing_new_instantiation() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///coalesce_new.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class PrimaryCache {\n",
        "    public function get(): void {}\n",
        "    public function warmUp(): void {}\n",
        "}\n",
        "\n",
        "class FallbackCache {\n",
        "    public function get(): void {}\n",
        "    public function fallbackOnly(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    /** @var PrimaryCache|null */\n",
        "    private ?PrimaryCache $primary;\n",
        "\n",
        "    public function run(): void {\n",
        "        $cache = $this->primary ?? new FallbackCache();\n",
        "        $cache->\n",
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

    let params = CompletionParams {
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

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $cache-> from null-coalescing"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // FallbackCache is the RHS of `??`
            assert!(
                labels.iter().any(|l| l.starts_with("fallbackOnly")),
                "Should include fallbackOnly from FallbackCache, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("get")),
                "Should include get (shared method), got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Short ternary (`$a ?: $b`): when `then` is None, the condition
/// itself and the else branch both contribute types.
#[tokio::test]
async fn test_completion_short_ternary() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///short_ternary.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ConfigA {\n",
        "    public function load(): void {}\n",
        "}\n",
        "\n",
        "class ConfigB {\n",
        "    public function save(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $a = new ConfigA();\n",
        "        $cfg = $a ?: new ConfigB();\n",
        "        $cfg->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 13,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $cfg-> from short ternary"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("save")),
                "Should include save from ConfigB (else branch), got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary with method calls: `$cond ? $this->makeA() : $this->makeB()`
/// where each method returns a different type.
#[tokio::test]
async fn test_completion_ternary_method_calls() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Redis {\n",
        "    public function redisGet(): void {}\n",
        "}\n",
        "\n",
        "class Memcached {\n",
        "    public function memGet(): void {}\n",
        "}\n",
        "\n",
        "class CacheFactory {\n",
        "    /** @return Redis */\n",
        "    public function createRedis(): Redis { return new Redis(); }\n",
        "    /** @return Memcached */\n",
        "    public function createMemcached(): Memcached { return new Memcached(); }\n",
        "\n",
        "    public function make(bool $useRedis): void {\n",
        "        $driver = $useRedis ? $this->createRedis() : $this->createMemcached();\n",
        "        $driver->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 17,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $driver-> from ternary with method calls"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("redisGet")),
                "Should include redisGet from Redis, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("memGet")),
                "Should include memGet from Memcached, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary with static method calls.
#[tokio::test]
async fn test_completion_ternary_static_calls() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class FileLogger {\n",
        "    public function rotate(): void {}\n",
        "    /** @return static */\n",
        "    public static function create(): static { return new static(); }\n",
        "}\n",
        "\n",
        "class SyslogLogger {\n",
        "    public function facility(): void {}\n",
        "    /** @return static */\n",
        "    public static function create(): static { return new static(); }\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function boot(bool $useSyslog): void {\n",
        "        $logger = $useSyslog ? SyslogLogger::create() : FileLogger::create();\n",
        "        $logger->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 16,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $logger-> from ternary with static calls"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("rotate")),
                "Should include rotate from FileLogger, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("facility")),
                "Should include facility from SyslogLogger, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary at the top level (outside a class).
#[tokio::test]
async fn test_completion_ternary_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_top.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Alpha {\n",
        "    public function alphaMethod(): void {}\n",
        "}\n",
        "\n",
        "class Beta {\n",
        "    public function betaMethod(): void {}\n",
        "}\n",
        "\n",
        "$x = true ? new Alpha() : new Beta();\n",
        "$x->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 10,
                character: 4,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for top-level $x-> from ternary"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("alphaMethod")),
                "Should include alphaMethod from Alpha, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("betaMethod")),
                "Should include betaMethod from Beta, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Null-coalescing at the top level (outside a class).
#[tokio::test]
async fn test_completion_null_coalescing_top_level() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///coalesce_top.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function fooMethod(): void {}\n",
        "}\n",
        "\n",
        "class Bar {\n",
        "    public function barMethod(): void {}\n",
        "}\n",
        "\n",
        "$a = new Foo();\n",
        "$b = $a ?? new Bar();\n",
        "$b->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 11,
                character: 4,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for top-level $b-> from null-coalescing"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // RHS new Bar() should contribute
            assert!(
                labels.iter().any(|l| l.starts_with("barMethod")),
                "Should include barMethod from Bar, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary followed by an unconditional reassignment: the ternary types
/// should be replaced by the final assignment.
#[tokio::test]
async fn test_completion_ternary_overridden_by_reassignment() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_override.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function fooMethod(): void {}\n",
        "}\n",
        "\n",
        "class Bar {\n",
        "    public function barMethod(): void {}\n",
        "}\n",
        "\n",
        "class Baz {\n",
        "    public function bazMethod(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(bool $c): void {\n",
        "        $obj = $c ? new Foo() : new Bar();\n",
        "        $obj = new Baz();\n",
        "        $obj->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 17,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $obj->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("bazMethod")),
                "Should include bazMethod from Baz (final assignment), got: {:?}",
                labels
            );
            assert!(
                !labels.iter().any(|l| l.starts_with("fooMethod")),
                "Should NOT include fooMethod from Foo (overridden), got: {:?}",
                labels
            );
            assert!(
                !labels.iter().any(|l| l.starts_with("barMethod")),
                "Should NOT include barMethod from Bar (overridden), got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary with property access in one branch and instantiation in the other.
#[tokio::test]
async fn test_completion_ternary_mixed_property_and_new() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_mixed.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Connection {\n",
        "    public function query(): void {}\n",
        "}\n",
        "\n",
        "class FakeConnection {\n",
        "    public function fake(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    /** @var Connection */\n",
        "    private Connection $db;\n",
        "\n",
        "    public function run(bool $testing): void {\n",
        "        $conn = $testing ? new FakeConnection() : $this->db;\n",
        "        $conn->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 15,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $conn-> from mixed ternary"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("fake")),
                "Should include fake from FakeConnection, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("query")),
                "Should include query from Connection, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Null-coalescing with both sides being method calls.
#[tokio::test]
async fn test_completion_null_coalescing_method_calls() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///coalesce_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Printer {\n",
        "    public function print(): void {}\n",
        "}\n",
        "\n",
        "class Scanner {\n",
        "    public function scan(): void {}\n",
        "}\n",
        "\n",
        "class Factory {\n",
        "    /** @return Printer */\n",
        "    public function makePrinter(): Printer { return new Printer(); }\n",
        "    /** @return Scanner */\n",
        "    public function makeScanner(): Scanner { return new Scanner(); }\n",
        "\n",
        "    public function resolve(): void {\n",
        "        $device = $this->makePrinter() ?? $this->makeScanner();\n",
        "        $device->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 17,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $device-> from null-coalescing method calls"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("print")),
                "Should include print from Printer, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("scan")),
                "Should include scan from Scanner, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary inside a conditional block: the ternary types should be treated
/// as conditional (appended to, not replacing).
#[tokio::test]
async fn test_completion_ternary_inside_if_block() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_in_if.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class DefaultService {\n",
        "    public function defaultOp(): void {}\n",
        "}\n",
        "\n",
        "class ServiceA {\n",
        "    public function opA(): void {}\n",
        "}\n",
        "\n",
        "class ServiceB {\n",
        "    public function opB(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(bool $cond, bool $flag): void {\n",
        "        $svc = new DefaultService();\n",
        "        if ($cond) {\n",
        "            $svc = $flag ? new ServiceA() : new ServiceB();\n",
        "        }\n",
        "        $svc->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 19,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $svc-> with ternary inside if"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("defaultOp")),
                "Should include defaultOp from DefaultService, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("opA")),
                "Should include opA from ServiceA, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("opB")),
                "Should include opB from ServiceB, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Cross-file ternary expression: arm types from other files should
/// resolve via PSR-4.
#[tokio::test]
async fn test_completion_ternary_cross_file() {
    use common::create_psr4_workspace;

    let composer = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let handler_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "use App\\Drivers\\FileDriver;\n",
        "use App\\Drivers\\DbDriver;\n",
        "\n",
        "class Handler {\n",
        "    public function handle(bool $useFile): void {\n",
        "        $driver = $useFile ? new FileDriver() : new DbDriver();\n",
        "        $driver->\n",
        "    }\n",
        "}\n",
    );

    let file_driver_php = concat!(
        "<?php\n",
        "namespace App\\Drivers;\n",
        "\n",
        "class FileDriver {\n",
        "    public function readFile(): void {}\n",
        "}\n",
    );

    let db_driver_php = concat!(
        "<?php\n",
        "namespace App\\Drivers;\n",
        "\n",
        "class DbDriver {\n",
        "    public function queryDb(): void {}\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer,
        &[
            ("src/Handler.php", handler_php),
            ("src/Drivers/FileDriver.php", file_driver_php),
            ("src/Drivers/DbDriver.php", db_driver_php),
        ],
    );

    let uri = Url::parse("file:///src/Handler.php").unwrap();
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: handler_php.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 9,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for cross-file ternary"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("readFile")),
                "Should include readFile from FileDriver, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("queryDb")),
                "Should include queryDb from DbDriver, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Null-coalescing with both sides being `new` instantiations.
#[tokio::test]
async fn test_completion_null_coalescing_both_new() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///coalesce_both_new.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ErrorHandler {\n",
        "    public function handleError(): void {}\n",
        "}\n",
        "\n",
        "class ExceptionHandler {\n",
        "    public function handleException(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $handler = new ErrorHandler() ?? new ExceptionHandler();\n",
        "        $handler->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 12,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $handler-> from null-coalescing"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("handleError")),
                "Should include handleError from ErrorHandler, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("handleException")),
                "Should include handleException from ExceptionHandler, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary combined with match inside one of the branches: the match arm
/// types should be collected alongside the ternary branch.
#[tokio::test]
async fn test_completion_ternary_with_match_in_branch() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_match.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SimpleHandler {\n",
        "    public function handle(): void {}\n",
        "}\n",
        "\n",
        "class ComplexHandlerA {\n",
        "    public function handleA(): void {}\n",
        "}\n",
        "\n",
        "class ComplexHandlerB {\n",
        "    public function handleB(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(bool $simple, int $mode): void {\n",
        "        $handler = $simple\n",
        "            ? new SimpleHandler()\n",
        "            : match ($mode) {\n",
        "                1 => new ComplexHandlerA(),\n",
        "                2 => new ComplexHandlerB(),\n",
        "            };\n",
        "        $handler->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 21,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $handler-> from ternary with match"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("handle")),
                "Should include handle from SimpleHandler, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("handleA")),
                "Should include handleA from ComplexHandlerA, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("handleB")),
                "Should include handleB from ComplexHandlerB, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Match expression with a ternary inside one of its arms: the ternary
/// branch types should contribute to the match arm results.
#[tokio::test]
async fn test_completion_match_with_ternary_in_arm() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///match_ternary.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ExporterA {\n",
        "    public function exportA(): void {}\n",
        "}\n",
        "\n",
        "class ExporterB {\n",
        "    public function exportB(): void {}\n",
        "}\n",
        "\n",
        "class ExporterC {\n",
        "    public function exportC(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(string $format, bool $compress): void {\n",
        "        $exporter = match ($format) {\n",
        "            'csv'  => $compress ? new ExporterA() : new ExporterB(),\n",
        "            'json' => new ExporterC(),\n",
        "        };\n",
        "        $exporter->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 19,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $exporter-> from match with ternary in arm"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("exportA")),
                "Should include exportA from ExporterA (ternary then), got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("exportB")),
                "Should include exportB from ExporterB (ternary else), got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("exportC")),
                "Should include exportC from ExporterC (match arm), got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Null-coalescing inside an if block: types should accumulate with
/// the unconditional assignment before it.
#[tokio::test]
async fn test_completion_null_coalescing_inside_if_block() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///coalesce_in_if.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class BaseService {\n",
        "    public function baseOp(): void {}\n",
        "}\n",
        "\n",
        "class AlternateService {\n",
        "    public function altOp(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    /** @var BaseService|null */\n",
        "    private ?BaseService $base;\n",
        "\n",
        "    public function run(bool $cond): void {\n",
        "        $svc = new BaseService();\n",
        "        if ($cond) {\n",
        "            $svc = $this->base ?? new AlternateService();\n",
        "        }\n",
        "        $svc->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 18,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $svc-> with null-coalescing inside if"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("baseOp")),
                "Should include baseOp from BaseService, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("altOp")),
                "Should include altOp from AlternateService, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Chained null-coalescing: `$a ?? $b ?? new C()` should resolve all
/// branches that contribute class types.
#[tokio::test]
async fn test_completion_chained_null_coalescing() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///coalesce_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class First {\n",
        "    public function firstOp(): void {}\n",
        "}\n",
        "\n",
        "class Second {\n",
        "    public function secondOp(): void {}\n",
        "}\n",
        "\n",
        "class Third {\n",
        "    public function thirdOp(): void {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $a = new First();\n",
        "        $b = new Second();\n",
        "        $result = $a ?? $b ?? new Third();\n",
        "        $result->\n",
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 18,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $result-> from chained null-coalescing"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // The RHS of `??` is `new Third()` which resolves directly.
            assert!(
                labels.iter().any(|l| l.starts_with("thirdOp")),
                "Should include thirdOp from Third, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
