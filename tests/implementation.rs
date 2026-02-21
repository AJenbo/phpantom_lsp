mod common;

use common::{create_psr4_workspace, create_test_backend};
use phpantom_lsp::Backend;
use phpantom_lsp::composer::parse_autoload_classmap;
use std::fs;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::request::{GotoImplementationParams, GotoImplementationResponse};
use tower_lsp::lsp_types::*;

// ─── Helpers ────────────────────────────────────────────────────────────────

async fn implementation_at(
    backend: &phpantom_lsp::Backend,
    uri: &Url,
    line: u32,
    character: u32,
) -> Vec<Location> {
    let params = GotoImplementationParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    match backend.goto_implementation(params).await.unwrap() {
        Some(GotoImplementationResponse::Scalar(loc)) => vec![loc],
        Some(GotoImplementationResponse::Array(locs)) => locs,
        Some(GotoImplementationResponse::Link(links)) => links
            .into_iter()
            .map(|l| Location {
                uri: l.target_uri,
                range: l.target_selection_range,
            })
            .collect(),
        None => vec![],
    }
}

async fn open(backend: &phpantom_lsp::Backend, uri: &Url, text: &str) {
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
}

// ─── Interface name → implementing classes ──────────────────────────────────

/// Cursor on an interface name → jumps to all classes that implement it.
#[tokio::test]
async fn test_implementation_interface_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_iface.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "interface Renderable {\n",                              // 1
        "    public function render(): string;\n",               // 2
        "}\n",                                                   // 3
        "class HtmlView implements Renderable {\n",              // 4
        "    public function render(): string { return ''; }\n", // 5
        "}\n",                                                   // 6
        "class JsonView implements Renderable {\n",              // 7
        "    public function render(): string { return ''; }\n", // 8
        "}\n",                                                   // 9
        "class PlainClass {\n",                                  // 10
        "    public function render(): string { return ''; }\n", // 11
        "}\n",                                                   // 12
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

    // Cursor on "Renderable" on line 1 (the interface declaration)
    let locations = implementation_at(&backend, &uri, 1, 12).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementors of Renderable, got {}",
        locations.len()
    );

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();
    assert!(
        lines.contains(&4),
        "Should include HtmlView (line 4), got lines: {:?}",
        lines
    );
    assert!(
        lines.contains(&7),
        "Should include JsonView (line 7), got lines: {:?}",
        lines
    );
    // PlainClass does NOT implement Renderable, so it should NOT be included.
    assert!(
        !lines.contains(&10),
        "Should NOT include PlainClass (line 10), got lines: {:?}",
        lines
    );
}

// ─── Abstract class name → extending classes ────────────────────────────────

/// Cursor on an abstract class name → jumps to concrete subclasses.
#[tokio::test]
async fn test_implementation_abstract_class_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_abstract.php").unwrap();
    let text = concat!(
        "<?php\n",                                              // 0
        "abstract class Shape {\n",                             // 1
        "    abstract public function area(): float;\n",        // 2
        "}\n",                                                  // 3
        "class Circle extends Shape {\n",                       // 4
        "    public function area(): float { return 3.14; }\n", // 5
        "}\n",                                                  // 6
        "class Square extends Shape {\n",                       // 7
        "    public function area(): float { return 1.0; }\n",  // 8
        "}\n",                                                  // 9
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

    // Cursor on "Shape" on line 1
    let locations = implementation_at(&backend, &uri, 1, 18).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 subclasses of Shape, got {}",
        locations.len()
    );

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();
    assert!(
        lines.contains(&4),
        "Should include Circle (line 4), got lines: {:?}",
        lines
    );
    assert!(
        lines.contains(&7),
        "Should include Square (line 7), got lines: {:?}",
        lines
    );
}

// ─── Method call on interface → concrete method implementations ─────────────

/// Cursor on a method call where the variable is typed as an interface →
/// jumps to the concrete method implementations.
#[tokio::test]
async fn test_implementation_method_on_interface() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_method.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "interface Renderable {\n",                              // 1
        "    public function render(): string;\n",               // 2
        "}\n",                                                   // 3
        "class HtmlView implements Renderable {\n",              // 4
        "    public function render(): string { return ''; }\n", // 5
        "}\n",                                                   // 6
        "class JsonView implements Renderable {\n",              // 7
        "    public function render(): string { return ''; }\n", // 8
        "}\n",                                                   // 9
        "class Service {\n",                                     // 10
        "    public function handle(Renderable $view) {\n",      // 11
        "        $view->render();\n",                            // 12
        "    }\n",                                               // 13
        "}\n",                                                   // 14
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

    // Cursor on "render" on line 12 (`$view->render()`)
    let locations = implementation_at(&backend, &uri, 12, 16).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementations of render(), got {}",
        locations.len()
    );

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();
    assert!(
        lines.contains(&5),
        "Should include HtmlView::render() (line 5), got lines: {:?}",
        lines
    );
    assert!(
        lines.contains(&8),
        "Should include JsonView::render() (line 8), got lines: {:?}",
        lines
    );
}

// ─── Method call on abstract class → concrete method implementations ────────

/// Cursor on a method call where the variable is typed as an abstract class →
/// jumps to the concrete overrides.
#[tokio::test]
async fn test_implementation_method_on_abstract_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_abstract_method.php").unwrap();
    let text = concat!(
        "<?php\n",                                              // 0
        "abstract class Shape {\n",                             // 1
        "    abstract public function area(): float;\n",        // 2
        "}\n",                                                  // 3
        "class Circle extends Shape {\n",                       // 4
        "    public function area(): float { return 3.14; }\n", // 5
        "}\n",                                                  // 6
        "class Square extends Shape {\n",                       // 7
        "    public function area(): float { return 1.0; }\n",  // 8
        "}\n",                                                  // 9
        "function calc(Shape $s) {\n",                          // 10
        "    $s->area();\n",                                    // 11
        "}\n",                                                  // 12
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

    // Cursor on "area" on line 11 (`$s->area()`)
    let locations = implementation_at(&backend, &uri, 11, 10).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementations of area(), got {}",
        locations.len()
    );

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();
    assert!(
        lines.contains(&5),
        "Should include Circle::area() (line 5), got lines: {:?}",
        lines
    );
    assert!(
        lines.contains(&8),
        "Should include Square::area() (line 8), got lines: {:?}",
        lines
    );
}

// ─── Non-interface / non-abstract returns None ──────────────────────────────

/// Cursor on a concrete class name → should NOT return implementations.
#[tokio::test]
async fn test_implementation_concrete_class_returns_none() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_concrete.php").unwrap();
    let text = concat!(
        "<?php\n",                      // 0
        "class User {\n",               // 1
        "    public string $name;\n",   // 2
        "}\n",                          // 3
        "class Admin extends User {\n", // 4
        "    public string $role;\n",   // 5
        "}\n",                          // 6
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

    // Cursor on "User" on line 1 — User is NOT abstract, so no implementations.
    let locations = implementation_at(&backend, &uri, 1, 7).await;
    assert!(
        locations.is_empty(),
        "Concrete class should not return implementations, got {:?}",
        locations
    );
}

// ─── Transitive implements (class extends class that implements iface) ──────

/// A class that extends another class which implements the interface should
/// be included as an implementor.
#[tokio::test]
async fn test_implementation_transitive_via_parent() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_transitive.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "interface Renderable {\n",                              // 1
        "    public function render(): string;\n",               // 2
        "}\n",                                                   // 3
        "class BaseView implements Renderable {\n",              // 4
        "    public function render(): string { return ''; }\n", // 5
        "}\n",                                                   // 6
        "class AdminView extends BaseView {\n",                  // 7
        "    public function render(): string { return ''; }\n", // 8
        "}\n",                                                   // 9
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

    // Cursor on "Renderable" on line 1
    let locations = implementation_at(&backend, &uri, 1, 12).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementors (direct + transitive), got {}",
        locations.len()
    );

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();
    assert!(
        lines.contains(&4),
        "Should include BaseView (line 4), got lines: {:?}",
        lines
    );
    assert!(
        lines.contains(&7),
        "Should include AdminView (line 7, transitive), got lines: {:?}",
        lines
    );
}

// ─── Multiple interfaces ────────────────────────────────────────────────────

/// A class implementing multiple interfaces should appear when querying
/// any of the interfaces.
#[tokio::test]
async fn test_implementation_multiple_interfaces() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_multi.php").unwrap();
    let text = concat!(
        "<?php\n",                                                  // 0
        "interface Serializable {\n",                               // 1
        "    public function serialize(): string;\n",               // 2
        "}\n",                                                      // 3
        "interface Printable {\n",                                  // 4
        "    public function print(): void;\n",                     // 5
        "}\n",                                                      // 6
        "class Report implements Serializable, Printable {\n",      // 7
        "    public function serialize(): string { return ''; }\n", // 8
        "    public function print(): void {}\n",                   // 9
        "}\n",                                                      // 10
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

    // Cursor on "Serializable" on line 1
    let locs_serial = implementation_at(&backend, &uri, 1, 12).await;
    assert!(
        !locs_serial.is_empty(),
        "Serializable should have at least one implementor"
    );
    let serial_lines: Vec<u32> = locs_serial.iter().map(|l| l.range.start.line).collect();
    assert!(
        serial_lines.contains(&7),
        "Report should implement Serializable, got lines: {:?}",
        serial_lines
    );

    // Cursor on "Printable" on line 4
    let locs_print = implementation_at(&backend, &uri, 4, 12).await;
    assert!(
        !locs_print.is_empty(),
        "Printable should have at least one implementor"
    );
    let print_lines: Vec<u32> = locs_print.iter().map(|l| l.range.start.line).collect();
    assert!(
        print_lines.contains(&7),
        "Report should implement Printable, got lines: {:?}",
        print_lines
    );
}

// ─── Enum implements interface ──────────────────────────────────────────────

/// Enums can implement interfaces — they should show up as implementors.
#[tokio::test]
async fn test_implementation_enum_implements_interface() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_enum.php").unwrap();
    let text = concat!(
        "<?php\n",                                    // 0
        "interface HasLabel {\n",                     // 1
        "    public function label(): string;\n",     // 2
        "}\n",                                        // 3
        "enum Color: string implements HasLabel {\n", // 4
        "    case Red = 'red';\n",                    // 5
        "    public function label(): string {\n",    // 6
        "        return $this->value;\n",             // 7
        "    }\n",                                    // 8
        "}\n",                                        // 9
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

    // Cursor on "HasLabel" on line 1
    let locations = implementation_at(&backend, &uri, 1, 12).await;

    assert!(
        !locations.is_empty(),
        "HasLabel should have at least one implementor (Color enum)"
    );

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();
    assert!(
        lines.contains(&4),
        "Should include Color enum (line 4), got lines: {:?}",
        lines
    );
}

// ─── Cross-file PSR-4 implementation ────────────────────────────────────────

/// Go-to-implementation works across files via PSR-4 autoloading.
#[tokio::test]
async fn test_implementation_cross_file_psr4() {
    let composer = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let interface_php = concat!(
        "<?php\n",
        "namespace App\\Contracts;\n",
        "interface Logger {\n",
        "    public function log(string $msg): void;\n",
        "}\n",
    );

    let file_logger_php = concat!(
        "<?php\n",
        "namespace App\\Logging;\n",
        "use App\\Contracts\\Logger;\n",
        "class FileLogger implements Logger {\n",
        "    public function log(string $msg): void {}\n",
        "}\n",
    );

    let db_logger_php = concat!(
        "<?php\n",
        "namespace App\\Logging;\n",
        "use App\\Contracts\\Logger;\n",
        "class DbLogger implements Logger {\n",
        "    public function log(string $msg): void {}\n",
        "}\n",
    );

    let service_php = concat!(
        "<?php\n",
        "namespace App\\Services;\n",
        "use App\\Contracts\\Logger;\n",
        "class AppService {\n",
        "    public function run(Logger $logger) {\n",
        "        $logger->log('hello');\n",
        "    }\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer,
        &[
            ("src/Contracts/Logger.php", interface_php),
            ("src/Logging/FileLogger.php", file_logger_php),
            ("src/Logging/DbLogger.php", db_logger_php),
            ("src/Services/AppService.php", service_php),
        ],
    );

    // Open all files so they are in the ast_map.
    let iface_uri = Url::parse("file:///logger_iface.php").unwrap();
    open(&backend, &iface_uri, interface_php).await;

    let file_logger_uri = Url::parse("file:///file_logger.php").unwrap();
    open(&backend, &file_logger_uri, file_logger_php).await;

    let db_logger_uri = Url::parse("file:///db_logger.php").unwrap();
    open(&backend, &db_logger_uri, db_logger_php).await;

    let service_uri = Url::parse("file:///service.php").unwrap();
    open(&backend, &service_uri, service_php).await;

    // Cursor on "Logger" on line 2 of interface file (interface Logger)
    let locations = implementation_at(&backend, &iface_uri, 2, 12).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementors of Logger across files, got {}",
        locations.len()
    );
}

// ─── Method on interface across files ───────────────────────────────────────

/// Method implementations across files — cursor on `$logger->log()`.
#[tokio::test]
async fn test_implementation_method_cross_file() {
    let composer = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let interface_php = concat!(
        "<?php\n",
        "namespace App\\Contracts;\n",
        "interface Formatter {\n",
        "    public function format(string $data): string;\n",
        "}\n",
    );

    let html_formatter_php = concat!(
        "<?php\n",
        "namespace App\\Formatters;\n",
        "use App\\Contracts\\Formatter;\n",
        "class HtmlFormatter implements Formatter {\n",
        "    public function format(string $data): string { return $data; }\n",
        "}\n",
    );

    let json_formatter_php = concat!(
        "<?php\n",
        "namespace App\\Formatters;\n",
        "use App\\Contracts\\Formatter;\n",
        "class JsonFormatter implements Formatter {\n",
        "    public function format(string $data): string { return $data; }\n",
        "}\n",
    );

    let service_php = concat!(
        "<?php\n",                                      // 0
        "namespace App\\Services;\n",                   // 1
        "use App\\Contracts\\Formatter;\n",             // 2
        "class RenderService {\n",                      // 3
        "    public function render(Formatter $f) {\n", // 4
        "        $f->format('hello');\n",               // 5
        "    }\n",                                      // 6
        "}\n",                                          // 7
    );

    let (backend, _dir) = create_psr4_workspace(
        composer,
        &[
            ("src/Contracts/Formatter.php", interface_php),
            ("src/Formatters/HtmlFormatter.php", html_formatter_php),
            ("src/Formatters/JsonFormatter.php", json_formatter_php),
            ("src/Services/RenderService.php", service_php),
        ],
    );

    // Open all files
    let iface_uri = Url::parse("file:///formatter_iface.php").unwrap();
    open(&backend, &iface_uri, interface_php).await;

    let html_uri = Url::parse("file:///html_formatter.php").unwrap();
    open(&backend, &html_uri, html_formatter_php).await;

    let json_uri = Url::parse("file:///json_formatter.php").unwrap();
    open(&backend, &json_uri, json_formatter_php).await;

    let service_uri = Url::parse("file:///render_service.php").unwrap();
    open(&backend, &service_uri, service_php).await;

    // Cursor on "format" on line 5 of service file (`$f->format('hello')`)
    let locations = implementation_at(&backend, &service_uri, 5, 14).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementations of format() across files, got {}",
        locations.len()
    );
}

// ─── Interface with no implementors ─────────────────────────────────────────

/// An interface with no implementing classes should return no locations.
#[tokio::test]
async fn test_implementation_no_implementors() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_none.php").unwrap();
    let text = concat!(
        "<?php\n",
        "interface Cacheable {\n",
        "    public function cache(): void;\n",
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

    let locations = implementation_at(&backend, &uri, 1, 12).await;
    assert!(
        locations.is_empty(),
        "Interface with no implementors should return empty, got {:?}",
        locations
    );
}

// ─── Does not crash on variable ─────────────────────────────────────────────

/// Invoking go-to-implementation on a variable should not crash.
#[tokio::test]
async fn test_implementation_on_variable_no_crash() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_var.php").unwrap();
    let text = concat!("<?php\n", "$x = 42;\n", "$x;\n",);

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

    // Should not panic, just return empty.
    let locations = implementation_at(&backend, &uri, 2, 1).await;
    let _ = locations;
}

// ─── Abstract class: only concrete subclasses included ──────────────────────

/// Abstract subclasses should NOT be included — only concrete ones.
#[tokio::test]
async fn test_implementation_skips_abstract_subclasses() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_skip_abstract.php").unwrap();
    let text = concat!(
        "<?php\n",                                                  // 0
        "abstract class Animal {\n",                                // 1
        "    abstract public function speak(): string;\n",          // 2
        "}\n",                                                      // 3
        "abstract class Pet extends Animal {\n",                    // 4
        "}\n",                                                      // 5
        "class Dog extends Pet {\n",                                // 6
        "    public function speak(): string { return 'woof'; }\n", // 7
        "}\n",                                                      // 8
        "class Cat extends Pet {\n",                                // 9
        "    public function speak(): string { return 'meow'; }\n", // 10
        "}\n",                                                      // 11
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

    // Cursor on "Animal" on line 1
    let locations = implementation_at(&backend, &uri, 1, 18).await;

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();

    // Pet is abstract, so it should NOT be included.
    assert!(
        !lines.contains(&4),
        "Should NOT include abstract Pet (line 4), got lines: {:?}",
        lines
    );

    // Dog and Cat are concrete — they should be included.
    // Dog extends Pet extends Animal (transitive).
    assert!(
        lines.contains(&6),
        "Should include Dog (line 6), got lines: {:?}",
        lines
    );
    assert!(
        lines.contains(&9),
        "Should include Cat (line 9), got lines: {:?}",
        lines
    );
}

// ─── Method on interface: only classes that override the method ──────────────

/// When a class implements an interface but inherits the method from its
/// parent (doesn't override it), it should NOT appear in method-level
/// implementation results.
#[tokio::test]
async fn test_implementation_method_only_overriders() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///impl_override.php").unwrap();
    let text = concat!(
        "<?php\n",                                               // 0
        "interface Renderable {\n",                              // 1
        "    public function render(): string;\n",               // 2
        "}\n",                                                   // 3
        "class BaseView implements Renderable {\n",              // 4
        "    public function render(): string { return ''; }\n", // 5
        "}\n",                                                   // 6
        "class ChildView extends BaseView {\n",                  // 7
        "    // Does NOT override render()\n",                   // 8
        "}\n",                                                   // 9
        "function show(Renderable $v) {\n",                      // 10
        "    $v->render();\n",                                   // 11
        "}\n",                                                   // 12
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

    // Cursor on "render" on line 11 (`$v->render()`)
    let locations = implementation_at(&backend, &uri, 11, 10).await;

    let lines: Vec<u32> = locations.iter().map(|l| l.range.start.line).collect();
    assert!(
        lines.contains(&5),
        "Should include BaseView::render() (line 5), got lines: {:?}",
        lines
    );
    // ChildView does NOT override render(), so it should NOT appear.
    assert!(
        !lines.iter().any(|&l| l == 7 || l == 8),
        "Should NOT include ChildView which doesn't override render(), got lines: {:?}",
        lines
    );
}

// ─── Server capability test ─────────────────────────────────────────────────

/// The server should advertise `implementationProvider` in its capabilities.
#[tokio::test]
async fn test_server_advertises_implementation_capability() {
    let backend = create_test_backend();

    let init_params = InitializeParams {
        root_uri: None,
        capabilities: ClientCapabilities::default(),
        ..InitializeParams::default()
    };

    let result = backend.initialize(init_params).await.unwrap();

    assert!(
        result.capabilities.implementation_provider.is_some(),
        "Server should advertise implementationProvider capability"
    );
}

// ─── Classmap file scanning (Phase 3) ───────────────────────────────────────

/// Implementors that exist on disk and are referenced by the classmap — but
/// have NOT been opened via `did_open` — should be discovered by Phase 3's
/// file-scanning logic.
#[tokio::test]
async fn test_implementation_classmap_file_scan() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
    )
    .expect("failed to write composer.json");

    // Interface file
    let interface_php = concat!(
        "<?php\n",
        "namespace App\\Contracts;\n",
        "interface Cacheable {\n",
        "    public function cacheKey(): string;\n",
        "}\n",
    );

    // Two implementors — these will only exist on disk, NOT opened.
    let redis_cache_php = concat!(
        "<?php\n",
        "namespace App\\Cache;\n",
        "use App\\Contracts\\Cacheable;\n",
        "class RedisCache implements Cacheable {\n",
        "    public function cacheKey(): string { return 'redis'; }\n",
        "}\n",
    );

    let file_cache_php = concat!(
        "<?php\n",
        "namespace App\\Cache;\n",
        "use App\\Contracts\\Cacheable;\n",
        "class FileCache implements Cacheable {\n",
        "    public function cacheKey(): string { return 'file'; }\n",
        "}\n",
    );

    // Write files to disk
    let src = dir.path().join("src");
    fs::create_dir_all(src.join("Contracts")).unwrap();
    fs::create_dir_all(src.join("Cache")).unwrap();
    fs::write(src.join("Contracts/Cacheable.php"), interface_php).unwrap();
    fs::write(src.join("Cache/RedisCache.php"), redis_cache_php).unwrap();
    fs::write(src.join("Cache/FileCache.php"), file_cache_php).unwrap();

    // Build classmap pointing to the on-disk files
    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).unwrap();
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$baseDir = dirname(dirname(__DIR__));\n",
            "return array(\n",
            "    'App\\\\Contracts\\\\Cacheable' => $baseDir . '/src/Contracts/Cacheable.php',\n",
            "    'App\\\\Cache\\\\RedisCache' => $baseDir . '/src/Cache/RedisCache.php',\n",
            "    'App\\\\Cache\\\\FileCache' => $baseDir . '/src/Cache/FileCache.php',\n",
            ");\n",
        ),
    )
    .unwrap();

    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    assert_eq!(classmap.len(), 3, "classmap should have 3 entries");

    let mappings = phpantom_lsp::composer::parse_composer_json(dir.path());
    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    // Only open the interface file — implementors stay on disk only.
    let iface_uri = Url::from_file_path(src.join("Contracts/Cacheable.php")).unwrap();
    open(&backend, &iface_uri, interface_php).await;

    // Go-to-implementation on "Cacheable" (line 2, col 12)
    let locations = implementation_at(&backend, &iface_uri, 2, 12).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementors of Cacheable via classmap scan, got {}",
        locations.len()
    );
}

// ─── PSR-4 directory scanning (Phase 5) ─────────────────────────────────────

/// Implementors that exist on disk under a PSR-4 root but are NOT in the
/// classmap (the user hasn't run `composer dump-autoload -o`) should be
/// discovered by Phase 5's directory-walking logic.
#[tokio::test]
async fn test_implementation_psr4_directory_scan() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
    )
    .expect("failed to write composer.json");

    // Interface file
    let interface_php = concat!(
        "<?php\n",
        "namespace App\\Contracts;\n",
        "interface Notifier {\n",
        "    public function send(string $msg): void;\n",
        "}\n",
    );

    // Implementors — only on disk, NOT opened and NOT in classmap.
    let email_notifier_php = concat!(
        "<?php\n",
        "namespace App\\Notifiers;\n",
        "use App\\Contracts\\Notifier;\n",
        "class EmailNotifier implements Notifier {\n",
        "    public function send(string $msg): void {}\n",
        "}\n",
    );

    let sms_notifier_php = concat!(
        "<?php\n",
        "namespace App\\Notifiers;\n",
        "use App\\Contracts\\Notifier;\n",
        "class SmsNotifier implements Notifier {\n",
        "    public function send(string $msg): void {}\n",
        "}\n",
    );

    // Write files to disk
    let src = dir.path().join("src");
    fs::create_dir_all(src.join("Contracts")).unwrap();
    fs::create_dir_all(src.join("Notifiers")).unwrap();
    fs::write(src.join("Contracts/Notifier.php"), interface_php).unwrap();
    fs::write(src.join("Notifiers/EmailNotifier.php"), email_notifier_php).unwrap();
    fs::write(src.join("Notifiers/SmsNotifier.php"), sms_notifier_php).unwrap();

    // NO classmap — simulate a project without `composer dump-autoload -o`.
    let mappings = phpantom_lsp::composer::parse_composer_json(dir.path());
    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);

    // Only open the interface file.
    let iface_uri = Url::from_file_path(src.join("Contracts/Notifier.php")).unwrap();
    open(&backend, &iface_uri, interface_php).await;

    // Go-to-implementation on "Notifier" (line 2, col 12)
    let locations = implementation_at(&backend, &iface_uri, 2, 12).await;

    assert!(
        locations.len() >= 2,
        "Should find at least 2 implementors via PSR-4 directory scan, got {}",
        locations.len()
    );
}

/// PSR-4 directory scanning should not re-process files already covered by
/// the classmap (those are handled by Phase 3).
#[tokio::test]
async fn test_implementation_psr4_scan_skips_classmap_files() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
    )
    .expect("failed to write composer.json");

    let interface_php = concat!(
        "<?php\n",
        "namespace App\\Contracts;\n",
        "interface Serializable {\n",
        "    public function serialize(): string;\n",
        "}\n",
    );

    // One implementor in classmap, one only on disk under PSR-4.
    let json_impl_php = concat!(
        "<?php\n",
        "namespace App\\Serializers;\n",
        "use App\\Contracts\\Serializable;\n",
        "class JsonSerializer implements Serializable {\n",
        "    public function serialize(): string { return '{}'; }\n",
        "}\n",
    );

    let xml_impl_php = concat!(
        "<?php\n",
        "namespace App\\Serializers;\n",
        "use App\\Contracts\\Serializable;\n",
        "class XmlSerializer implements Serializable {\n",
        "    public function serialize(): string { return '<xml/>'; }\n",
        "}\n",
    );

    let src = dir.path().join("src");
    fs::create_dir_all(src.join("Contracts")).unwrap();
    fs::create_dir_all(src.join("Serializers")).unwrap();
    fs::write(src.join("Contracts/Serializable.php"), interface_php).unwrap();
    fs::write(src.join("Serializers/JsonSerializer.php"), json_impl_php).unwrap();
    fs::write(src.join("Serializers/XmlSerializer.php"), xml_impl_php).unwrap();

    // Classmap only includes JsonSerializer — XmlSerializer is PSR-4 only.
    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).unwrap();
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$baseDir = dirname(dirname(__DIR__));\n",
            "return array(\n",
            "    'App\\\\Serializers\\\\JsonSerializer' => $baseDir . '/src/Serializers/JsonSerializer.php',\n",
            ");\n",
        ),
    )
    .unwrap();

    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    let mappings = phpantom_lsp::composer::parse_composer_json(dir.path());
    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    // Only open the interface.
    let iface_uri = Url::from_file_path(src.join("Contracts/Serializable.php")).unwrap();
    open(&backend, &iface_uri, interface_php).await;

    let locations = implementation_at(&backend, &iface_uri, 2, 12).await;

    // Both should be found: JsonSerializer via classmap (Phase 3),
    // XmlSerializer via PSR-4 walk (Phase 5).
    assert!(
        locations.len() >= 2,
        "Should find both classmap and PSR-4-only implementors, got {}",
        locations.len()
    );
}

/// Phase 5 should also find implementors of abstract classes via PSR-4 scan.
#[tokio::test]
async fn test_implementation_psr4_scan_abstract_class() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
    )
    .expect("failed to write composer.json");

    let abstract_php = concat!(
        "<?php\n",
        "namespace App\\Base;\n",
        "abstract class Handler {\n",
        "    abstract public function handle(): void;\n",
        "}\n",
    );

    let concrete_php = concat!(
        "<?php\n",
        "namespace App\\Handlers;\n",
        "use App\\Base\\Handler;\n",
        "class ConcreteHandler extends Handler {\n",
        "    public function handle(): void {}\n",
        "}\n",
    );

    let src = dir.path().join("src");
    fs::create_dir_all(src.join("Base")).unwrap();
    fs::create_dir_all(src.join("Handlers")).unwrap();
    fs::write(src.join("Base/Handler.php"), abstract_php).unwrap();
    fs::write(src.join("Handlers/ConcreteHandler.php"), concrete_php).unwrap();

    let mappings = phpantom_lsp::composer::parse_composer_json(dir.path());
    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);

    let iface_uri = Url::from_file_path(src.join("Base/Handler.php")).unwrap();
    open(&backend, &iface_uri, abstract_php).await;

    let locations = implementation_at(&backend, &iface_uri, 2, 18).await;

    assert!(
        !locations.is_empty(),
        "Should find ConcreteHandler via PSR-4 scan, got {}",
        locations.len()
    );
}

/// Method-level go-to-implementation should work when implementors are
/// discovered via classmap or PSR-4 scanning (not pre-opened).
#[tokio::test]
async fn test_implementation_method_via_psr4_scan() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
    )
    .expect("failed to write composer.json");

    let interface_php = concat!(
        "<?php\n",
        "namespace App\\Contracts;\n",
        "interface Repository {\n",
        "    public function find(int $id): object;\n",
        "}\n",
    );

    let service_php = concat!(
        "<?php\n",
        "namespace App\\Services;\n",
        "use App\\Contracts\\Repository;\n",
        "class UserService {\n",
        "    public function get(Repository $repo) {\n",
        "        $repo->find(1);\n",
        "    }\n",
        "}\n",
    );

    let impl_php = concat!(
        "<?php\n",
        "namespace App\\Repos;\n",
        "use App\\Contracts\\Repository;\n",
        "class UserRepository implements Repository {\n",
        "    public function find(int $id): object { return (object)[]; }\n",
        "}\n",
    );

    let src = dir.path().join("src");
    fs::create_dir_all(src.join("Contracts")).unwrap();
    fs::create_dir_all(src.join("Services")).unwrap();
    fs::create_dir_all(src.join("Repos")).unwrap();
    fs::write(src.join("Contracts/Repository.php"), interface_php).unwrap();
    fs::write(src.join("Services/UserService.php"), service_php).unwrap();
    fs::write(src.join("Repos/UserRepository.php"), impl_php).unwrap();

    let mappings = phpantom_lsp::composer::parse_composer_json(dir.path());
    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), mappings);

    // Open the interface and the service file (but NOT the implementor).
    let iface_uri = Url::from_file_path(src.join("Contracts/Repository.php")).unwrap();
    open(&backend, &iface_uri, interface_php).await;

    let svc_uri = Url::from_file_path(src.join("Services/UserService.php")).unwrap();
    open(&backend, &svc_uri, service_php).await;

    // Cursor on "find" in `$repo->find(1);` — line 5, col 16
    let locations = implementation_at(&backend, &svc_uri, 5, 16).await;

    assert!(
        !locations.is_empty(),
        "Should find UserRepository::find via PSR-4 scan"
    );
}
