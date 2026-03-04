//! Integration tests for `textDocument/hover`.

mod common;

use common::{
    create_psr4_workspace, create_test_backend, create_test_backend_with_function_stubs,
    create_test_backend_with_stdclass_stub,
};
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Register file content in the backend (sync) and return the hover result
/// at the given (0-based) line and character.
fn hover_at(
    backend: &Backend,
    uri: &str,
    content: &str,
    line: u32,
    character: u32,
) -> Option<Hover> {
    // Parse and populate ast_map, use_map, namespace_map, symbol_maps
    backend.update_ast(uri, content);

    backend.handle_hover(uri, content, Position { line, character })
}

/// Extract the Markdown text from a Hover response.
fn hover_text(hover: &Hover) -> &str {
    match &hover.contents {
        HoverContents::Markup(markup) => &markup.value,
        _ => panic!("Expected MarkupContent"),
    }
}

// ─── Variable hover ─────────────────────────────────────────────────────────

#[test]
fn hover_this_variable() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class User {
    public function greet(): string {
        return $this->name();
    }
}
"#;

    // Hover on `$this` (line 3, within the `$this` token)
    let hover = hover_at(&backend, uri, content, 3, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("$this"), "should mention $this: {}", text);
    assert!(text.contains("User"), "should resolve to User: {}", text);
}

#[test]
fn hover_variable_with_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Order {
    public string $id;
}
class Service {
    public function run(): void {
        $order = new Order();
        $order->id;
    }
}
"#;

    // Hover on `$order` at line 7 (the usage)
    let hover = hover_at(&backend, uri, content, 7, 9).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("$order"), "should mention $order: {}", text);
    assert!(text.contains("Order"), "should resolve to Order: {}", text);
}

#[test]
fn hover_variable_without_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
function test() {
    $x = 42;
    echo $x;
}
"#;

    // Hover on `$x` at line 3
    let hover = hover_at(&backend, uri, content, 3, 10).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("$x"), "should mention $x: {}", text);
}

#[test]
fn hover_suppressed_on_parameter_definition_site() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Builder {
    public function scopeOfGenre(\Illuminate\Database\Eloquent\Builder $query, string $genre): void {
        $query->where('genre', $genre);
    }
}
"#;

    // Hover on `$query` at the parameter definition site (line 2, col ~72)
    let hover = hover_at(&backend, uri, content, 2, 73);
    assert!(
        hover.is_none(),
        "hover should be suppressed on parameter $query"
    );

    // Hover on `$genre` at the parameter definition site (line 2, col ~87)
    let hover = hover_at(&backend, uri, content, 2, 88);
    assert!(
        hover.is_none(),
        "hover should be suppressed on parameter $genre"
    );
}

#[test]
fn hover_suppressed_on_foreach_variable_definition_site() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Item { public string $name; }
class Service {
    /** @param Item[] $items */
    public function run(array $items): void {
        foreach ($items as $item) {
            $item->name;
        }
    }
}
"#;

    // Hover on `$item` at the foreach binding site (line 5)
    let hover = hover_at(&backend, uri, content, 5, 29);
    assert!(
        hover.is_none(),
        "hover should be suppressed on foreach variable $item"
    );
}

#[test]
fn hover_suppressed_on_catch_variable_definition_site() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
function risky(): void {
    try {
        throw new \Exception('oops');
    } catch (\Exception $e) {
        echo $e->getMessage();
    }
}
"#;

    // Hover on `$e` at the catch binding site (line 4)
    let hover = hover_at(&backend, uri, content, 4, 26);
    assert!(
        hover.is_none(),
        "hover should be suppressed on catch variable $e"
    );
}

#[test]
fn hover_active_on_variable_assignment() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Order { public string $id; }
class Service {
    public function run(): void {
        $order = new Order();
        $order->id;
    }
}
"#;

    // Hover on `$order` at the assignment site (line 4) should still work
    let hover = hover_at(&backend, uri, content, 4, 9)
        .expect("hover should be active on assignment $order");
    let text = hover_text(&hover);
    assert!(text.contains("Order"), "should resolve to Order: {}", text);
}

// ─── Method hover ───────────────────────────────────────────────────────────

#[test]
fn hover_method_call() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Calculator {
    public function add(int $a, int $b): int {
        return $a + $b;
    }
    public function run(): void {
        $this->add(1, 2);
    }
}
"#;

    // Hover on `add` in `$this->add(1, 2)` (line 6)
    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("add"), "should contain method name: {}", text);
    assert!(text.contains("int $a"), "should show params: {}", text);
    assert!(text.contains(": int"), "should show return type: {}", text);
    assert!(
        text.contains("Calculator"),
        "should show owner class: {}",
        text
    );
}

#[test]
fn hover_static_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Factory {
    public static function create(string $name): self {
        return new self();
    }
}
class Usage {
    public function run(): void {
        Factory::create('test');
    }
}
"#;

    // Hover on `create` in `Factory::create` (line 8)
    let hover = hover_at(&backend, uri, content, 8, 18).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("create"),
        "should contain method name: {}",
        text
    );
    assert!(text.contains("static"), "should indicate static: {}", text);
    assert!(
        text.contains("string $name"),
        "should show params: {}",
        text
    );
}

// ─── Property hover ─────────────────────────────────────────────────────────

#[test]
fn hover_property_access() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    public string $name;
    public function show(): void {
        echo $this->name;
    }
}
"#;

    // Hover on `name` in `$this->name` (line 4)
    let hover = hover_at(&backend, uri, content, 4, 21).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("name"),
        "should contain property name: {}",
        text
    );
    assert!(text.contains("string"), "should show type: {}", text);
    assert!(text.contains("Config"), "should show owner: {}", text);
}

#[test]
fn hover_static_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Registry {
    public static int $count;
}
class Usage {
    public function run(): void {
        echo Registry::$count;
    }
}
"#;

    // Hover on `$count` in `Registry::$count` (line 6)
    let hover = hover_at(&backend, uri, content, 6, 24).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("count"),
        "should contain property name: {}",
        text
    );
    assert!(text.contains("static"), "should indicate static: {}", text);
    assert!(text.contains("int"), "should show type: {}", text);
}

// ─── Constant hover ─────────────────────────────────────────────────────────

#[test]
fn hover_class_constant() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Status {
    const ACTIVE = 'active';
}
class Usage {
    public function run(): void {
        echo Status::ACTIVE;
    }
}
"#;

    // Hover on `ACTIVE` in `Status::ACTIVE` (line 6)
    let hover = hover_at(&backend, uri, content, 6, 22).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("ACTIVE"),
        "should contain constant name: {}",
        text
    );
    assert!(text.contains("Status"), "should show owner: {}", text);
}

// ─── Class hover ────────────────────────────────────────────────────────────

#[test]
fn hover_class_reference() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Animal {
    public string $species;
}
class Zoo {
    public function adopt(Animal $pet): void {}
}
"#;

    // Hover on `Animal` in the type hint (line 5)
    let hover = hover_at(&backend, uri, content, 5, 28).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("class"), "should show class kind: {}", text);
    assert!(text.contains("Animal"), "should show class name: {}", text);
}

#[test]
fn hover_interface_reference() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
interface Printable {
    public function print(): void;
}
class Document implements Printable {
    public function print(): void {}
}
"#;

    // Hover on `Printable` in the implements clause (line 4)
    let hover = hover_at(&backend, uri, content, 4, 32).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("interface"),
        "should show interface kind: {}",
        text
    );
    assert!(
        text.contains("Printable"),
        "should show interface name: {}",
        text
    );
}

#[test]
fn hover_interface_extending_interface_no_duplicate_extends() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TKey
 * @template-covariant TValue
 * @template-extends iterable<TKey, TValue>
 */
interface Traversable extends iterable {}

function test(Traversable $t): void {}
"#;

    // Hover on `Traversable` in the function parameter (line 8)
    let hover = hover_at(&backend, uri, content, 8, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("interface Traversable extends iterable"),
        "should show extends once: {}",
        text
    );
    // Must NOT contain the keyword "extends" twice
    let extends_count = text.matches("extends").count();
    assert_eq!(
        extends_count, 1,
        "should contain 'extends' exactly once, got {}: {}",
        extends_count, text
    );
}

#[test]
fn hover_class_declaration_returns_none() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Represents a blog post.
 */
class BlogPost {
    public string $title;
}
"#;

    // Hover on `BlogPost` at its declaration site should return None —
    // the user is already looking at the definition.
    let hover = hover_at(&backend, uri, content, 4, 8);
    assert!(
        hover.is_none(),
        "should not show hover on class declaration site"
    );
}

#[test]
fn hover_class_declaration_disambiguates_by_namespace_returns_none() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo {
    class Builder {
        public function demo(): void {}
    }
}

namespace Illuminate\Contracts\Database\Eloquent {
    /**
     * @mixin \Illuminate\Database\Eloquent\Builder
     */
    interface Builder {}
}
"#;

    // Hover on declaration sites should return None.
    let hover = hover_at(&backend, uri, content, 11, 16);
    assert!(
        hover.is_none(),
        "should not show hover on interface declaration site"
    );

    let hover = hover_at(&backend, uri, content, 2, 12);
    assert!(
        hover.is_none(),
        "should not show hover on class declaration site"
    );
}

#[test]
fn hover_abstract_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
abstract class Shape {
    abstract public function area(): float;
}
class Circle extends Shape {
    public function area(): float { return 3.14; }
}
"#;

    // Hover on `Shape` in extends clause (line 4)
    let hover = hover_at(&backend, uri, content, 4, 23).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("abstract class"),
        "should show abstract class: {}",
        text
    );
    assert!(text.contains("Shape"), "should show class name: {}", text);
}

#[test]
fn hover_final_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
final class Singleton {
    public static function instance(): self { return new self(); }
}
function test(Singleton $s): void {}
"#;

    // Hover on `Singleton` in function param (line 4)
    let hover = hover_at(&backend, uri, content, 4, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("final class"),
        "should show final class: {}",
        text
    );
}

// ─── Self / static / parent hover ───────────────────────────────────────────

#[test]
fn hover_self_keyword() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Foo {
    public static function make(): self {
        return new self();
    }
}
"#;

    // Hover on `self` at line 3 inside `new self()`
    let hover = hover_at(&backend, uri, content, 3, 20).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("self"), "should mention self: {}", text);
    assert!(text.contains("Foo"), "should resolve to Foo: {}", text);
}

#[test]
fn hover_parent_keyword() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Base {
    public function hello(): string { return 'hi'; }
}
class Child extends Base {
    public function hello(): string {
        return parent::hello();
    }
}
"#;

    // Hover on `parent` at line 6
    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("parent"), "should mention parent: {}", text);
    assert!(text.contains("Base"), "should resolve to Base: {}", text);
}

// ─── Function call hover ────────────────────────────────────────────────────

#[test]
fn hover_user_function() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
function greet(string $name): string {
    return "Hello, $name!";
}
greet('World');
"#;

    // Hover on `greet` at line 4
    let hover = hover_at(&backend, uri, content, 4, 2).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("greet"),
        "should contain function name: {}",
        text
    );
    assert!(
        text.contains("string $name"),
        "should show params: {}",
        text
    );
    assert!(
        text.contains(": string"),
        "should show return type: {}",
        text
    );
}

// ─── Deprecated marker ──────────────────────────────────────────────────────

#[test]
fn hover_deprecated_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Legacy {
    /**
     * @deprecated Use newMethod() instead.
     */
    public function oldMethod(): void {}
    public function run(): void {
        $this->oldMethod();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("oldMethod"),
        "should contain method name: {}",
        text
    );
    assert!(
        text.contains("🪦 **deprecated** Use newMethod() instead."),
        "should show deprecated with message: {}",
        text
    );
}

#[test]
fn hover_deprecated_method_without_message() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Legacy {
    /**
     * @deprecated
     */
    public function oldMethod(): void {}
    public function run(): void {
        $this->oldMethod();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("🪦 **deprecated**"),
        "should show bare deprecated: {}",
        text
    );
    // Should NOT contain any message text after the label
    assert!(
        !text.contains("🪦 **deprecated** "),
        "should not have trailing text after deprecated: {}",
        text
    );
}

#[test]
fn hover_deprecated_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @deprecated Use NewApi instead.
 */
class OldApi {
    public function run(): void {}
}
function test(OldApi $api): void {}
"#;

    // Hover on OldApi in function param (line 7)
    let hover = hover_at(&backend, uri, content, 7, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("OldApi"), "should show class name: {}", text);
    assert!(
        text.contains("🪦 **deprecated** Use NewApi instead."),
        "should show deprecated with message: {}",
        text
    );
}

#[test]
fn hover_deprecated_property_shows_message() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    /**
     * @deprecated Use getDebugMode() instead.
     */
    public bool $debug = false;

    public function test(): void {
        $this->debug;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 8, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("🪦 **deprecated** Use getDebugMode() instead."),
        "should show deprecated with message: {}",
        text
    );
}

#[test]
fn hover_deprecated_constant_shows_message() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class HttpStatus {
    /**
     * @deprecated Use OK instead.
     */
    const SUCCESS = 200;

    const OK = 200;
}
$x = HttpStatus::SUCCESS;
"#;

    let hover = hover_at(&backend, uri, content, 9, 20).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("🪦 **deprecated** Use OK instead."),
        "should show deprecated with message: {}",
        text
    );
}

#[test]
fn hover_deprecated_function_shows_message() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @deprecated Use newHelper() instead.
 */
function oldHelper(): void {}

oldHelper();
"#;

    let hover = hover_at(&backend, uri, content, 6, 4).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("🪦 **deprecated** Use newHelper() instead."),
        "should show deprecated with message: {}",
        text
    );
}

// ─── Cross-file hover ───────────────────────────────────────────────────────

#[test]
fn hover_cross_file_class() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": { "App\\": "src/" }
            }
        }"#,
        &[
            (
                "src/Models/Product.php",
                r#"<?php
namespace App\Models;
/**
 * Represents a product in the catalog.
 */
class Product {
    public string $name;
    public float $price;
    public function discount(float $percent): float {
        return $this->price * (1 - $percent / 100);
    }
}
"#,
            ),
            (
                "src/Service.php",
                r#"<?php
namespace App;
use App\Models\Product;
class Service {
    public function run(): void {
        $p = new Product();
        $p->discount(10);
    }
}
"#,
            ),
        ],
    );

    let product_uri = format!(
        "file://{}",
        _dir.path().join("src/Models/Product.php").display()
    );
    let product_content =
        std::fs::read_to_string(_dir.path().join("src/Models/Product.php")).unwrap();
    backend.update_ast(&product_uri, &product_content);

    let service_uri = format!("file://{}", _dir.path().join("src/Service.php").display());
    let service_content = std::fs::read_to_string(_dir.path().join("src/Service.php")).unwrap();

    // Hover on `Product` type reference (line 5: `$p = new Product()`)
    let hover = hover_at(&backend, &service_uri, &service_content, 5, 20)
        .expect("expected hover on Product");
    let text = hover_text(&hover);
    assert!(
        text.contains("Product"),
        "should resolve cross-file class: {}",
        text
    );
    assert!(
        text.contains("Represents a product"),
        "should include docblock from cross-file class: {}",
        text
    );
}

#[test]
fn hover_cross_file_method() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": { "App\\": "src/" }
            }
        }"#,
        &[
            (
                "src/Models/Item.php",
                r#"<?php
namespace App\Models;
class Item {
    public function getLabel(): string {
        return 'label';
    }
}
"#,
            ),
            (
                "src/Handler.php",
                r#"<?php
namespace App;
use App\Models\Item;
class Handler {
    public function process(): void {
        $item = new Item();
        $item->getLabel();
    }
}
"#,
            ),
        ],
    );

    let item_uri = format!(
        "file://{}",
        _dir.path().join("src/Models/Item.php").display()
    );
    let item_content = std::fs::read_to_string(_dir.path().join("src/Models/Item.php")).unwrap();
    backend.update_ast(&item_uri, &item_content);

    let handler_uri = format!("file://{}", _dir.path().join("src/Handler.php").display());
    let handler_content = std::fs::read_to_string(_dir.path().join("src/Handler.php")).unwrap();

    // Hover on `getLabel` (line 6)
    let hover = hover_at(&backend, &handler_uri, &handler_content, 6, 16)
        .expect("expected hover on getLabel");
    let text = hover_text(&hover);
    assert!(
        text.contains("getLabel"),
        "should resolve cross-file method: {}",
        text
    );
    assert!(
        text.contains(": string"),
        "should show return type: {}",
        text
    );
    assert!(text.contains("Item"), "should show owner class: {}", text);
}

// ─── Enum hover ─────────────────────────────────────────────────────────────

#[test]
fn hover_enum_declaration() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Possible statuses for an order.
 */
enum OrderStatus: string {
    case Pending = 'pending';
    case Shipped = 'shipped';
}
function process(OrderStatus $status): void {}
"#;

    // Hover on `OrderStatus` in the function param (line 8)
    let hover = hover_at(&backend, uri, content, 8, 20).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("enum"), "should show enum kind: {}", text);
    assert!(
        text.contains("OrderStatus"),
        "should show enum name: {}",
        text
    );
    assert!(
        text.contains("Possible statuses"),
        "should include docblock: {}",
        text
    );
}

// ─── Trait hover ────────────────────────────────────────────────────────────

#[test]
fn hover_trait_reference() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Provides soft-delete functionality.
 */
trait SoftDeletes {
    public function trash(): void {}
}
class Post {
    use SoftDeletes;
}
"#;

    // Hover on `SoftDeletes` in the use statement (line 8)
    let hover = hover_at(&backend, uri, content, 8, 10).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("trait"), "should show trait kind: {}", text);
    assert!(
        text.contains("SoftDeletes"),
        "should show trait name: {}",
        text
    );
    assert!(
        text.contains("Provides soft-delete"),
        "should include docblock: {}",
        text
    );
}

// ─── Visibility display ─────────────────────────────────────────────────────

#[test]
fn hover_shows_visibility() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Vault {
    private string $secret;
    protected int $level;
    public function getSecret(): string {
        echo $this->secret;
        echo $this->level;
        return $this->secret;
    }
}
"#;

    // Hover on `secret` property (line 5)
    let hover = hover_at(&backend, uri, content, 5, 22).expect("expected hover on secret");
    let text = hover_text(&hover);
    assert!(
        text.contains("private"),
        "should show private visibility: {}",
        text
    );

    // Hover on `level` property (line 6)
    let hover = hover_at(&backend, uri, content, 6, 22).expect("expected hover on level");
    let text = hover_text(&hover);
    assert!(
        text.contains("protected"),
        "should show protected visibility: {}",
        text
    );
}

// ─── Inheritance hover ──────────────────────────────────────────────────────

#[test]
fn hover_inherited_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class BaseRepo {
    public function findAll(): array {
        return [];
    }
}
class UserRepo extends BaseRepo {
    public function run(): void {
        $this->findAll();
    }
}
"#;

    // Hover on `findAll` in the child class (line 8)
    let hover = hover_at(&backend, uri, content, 8, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("findAll"),
        "should show inherited method: {}",
        text
    );
    assert!(
        text.contains(": array"),
        "should show return type: {}",
        text
    );
}

// ─── Class with parent and implements ───────────────────────────────────────

#[test]
fn hover_class_with_extends_and_implements() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
interface Loggable {
    public function log(): void;
}
class Base {}
class App extends Base implements Loggable {
    public function log(): void {}
}
function test(App $app): void {}
"#;

    // Hover on `App` in the function parameter (line 8)
    let hover = hover_at(&backend, uri, content, 8, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(text.contains("class App"), "should show class: {}", text);
    // Parent/interface names may have a leading `\` from the parser
    assert!(
        text.contains("extends") && text.contains("Base"),
        "should show parent: {}",
        text
    );
    assert!(
        text.contains("implements") && text.contains("Loggable"),
        "should show interfaces: {}",
        text
    );
}

// ─── No hover on whitespace ─────────────────────────────────────────────────

#[test]
fn hover_on_whitespace_returns_none() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php

class Foo {}
"#;

    // Hover on the blank line (line 1)
    let hover = hover_at(&backend, uri, content, 1, 0);
    assert!(hover.is_none(), "should not produce hover on blank line");
}

// ─── Stub function hover ────────────────────────────────────────────────────

#[test]
fn hover_stub_function() {
    let backend = create_test_backend_with_function_stubs();
    let uri = "file:///test.php";
    let content = r#"<?php
$x = str_contains('hello', 'ell');
"#;

    // Hover on `str_contains` (line 1)
    let hover = hover_at(&backend, uri, content, 1, 8).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("str_contains"),
        "should show function name: {}",
        text
    );
    assert!(
        text.contains("string $haystack"),
        "should show params: {}",
        text
    );
    assert!(text.contains(": bool"), "should show return type: {}", text);
}

// ─── Namespaced class hover ─────────────────────────────────────────────────

#[test]
fn hover_shows_fqn() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Models;

/**
 * A customer entity.
 */
class Customer {
    public string $email;
}

class Service {
    public function run(): void {
        $c = new Customer();
        $c->email;
    }
}
"#;

    // Hover on Customer reference at line 12
    let hover = hover_at(&backend, uri, content, 12, 18).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("namespace App\\Models;"),
        "should show namespace line: {}",
        text
    );
    assert!(
        text.contains("class Customer"),
        "should show short class name: {}",
        text
    );
    assert!(
        text.contains("A customer entity"),
        "should include docblock: {}",
        text
    );
}

// ─── Method with reference and variadic params ──────────────────────────────

#[test]
fn hover_method_with_reference_param() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Sorter {
    public function sort(array &$items): void {}
    public function run(): void {
        $this->sort([]);
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("&$items"),
        "should show reference param: {}",
        text
    );
}

#[test]
fn hover_method_with_variadic_param() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Logger {
    public function log(string ...$messages): void {}
    public function run(): void {
        $this->log('a', 'b');
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("...$messages"),
        "should show variadic param: {}",
        text
    );
}

// ─── Docblock array/object shape type hover ─────────────────────────────────

/// Hovering on a class name inside an array shape value type in a docblock
/// should resolve the class and show hover info.
#[test]
fn hover_class_in_array_shape_value_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Pen {
    public string $color;
}
/**
 * @return array{logger: Pen, debug: bool}
 */
function getAppConfig(): array { return []; }
"#;

    // Hover on `Pen` inside the array shape (line 5, find "Pen" after "logger: ")
    let hover =
        hover_at(&backend, uri, content, 5, 25).expect("expected hover on Pen in array shape");
    let text = hover_text(&hover);
    assert!(
        text.contains("Pen"),
        "should resolve Pen inside array shape, got: {}",
        text
    );
    assert!(
        text.contains("class"),
        "should show class kind for Pen, got: {}",
        text
    );
}

// ─── Docblock callable type hover ───────────────────────────────────────────

/// Hovering on a class name in a callable return type inside a docblock
/// should show the class info, not treat the whole callable as one token.
#[test]
fn hover_class_in_callable_return_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Pencil {
    public string $color;
}
class Factory {
    /** @var \Closure(): Pencil $supplier */
    private $supplier;
}
"#;

    // Hover on `Pencil` in `\Closure(): Pencil` (line 5, character ~29)
    let hover = hover_at(&backend, uri, content, 5, 29).expect("expected hover on Pencil");
    let text = hover_text(&hover);
    assert!(
        text.contains("Pencil"),
        "should show Pencil class: {}",
        text
    );
    assert!(
        !text.contains("Closure(): Pencil"),
        "should not treat whole callable as class name: {}",
        text
    );
}

/// Hovering on a class name used as a callable parameter type in a docblock.
#[test]
fn hover_class_in_callable_param_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Request {
    public string $body;
}
class Response {
    public int $status;
}
class Handler {
    /** @var callable(Request): Response $handler */
    private $handler;
}
"#;

    // Hover on `Request` in `callable(Request)` (line 8)
    let hover = hover_at(&backend, uri, content, 8, 24).expect("expected hover on Request");
    let text = hover_text(&hover);
    assert!(
        text.contains("Request"),
        "should show Request class: {}",
        text
    );

    // Hover on `Response` in callable return type (line 8)
    let hover = hover_at(&backend, uri, content, 8, 34).expect("expected hover on Response");
    let text = hover_text(&hover);
    assert!(
        text.contains("Response"),
        "should show Response class: {}",
        text
    );
}

/// Hovering on `\Closure` itself inside a callable annotation.
#[test]
fn hover_closure_base_in_callable_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Result {}
class Worker {
    /** @param \Closure(int): Result $cb */
    public function run($cb) {}
}
"#;

    // Hover on `Result` in `\Closure(int): Result` (line 3)
    let hover = hover_at(&backend, uri, content, 3, 35).expect("expected hover on Result");
    let text = hover_text(&hover);
    assert!(
        text.contains("Result"),
        "should show Result class: {}",
        text
    );
}

// ─── Docblock description in hover ──────────────────────────────────────────

#[test]
fn hover_property_shows_docblock_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Zoo {
    /** @var list<string> The animal names */
    public array $animals;
    public function show(): void {
        echo $this->animals;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 5, 22).expect("expected hover on animals");
    let text = hover_text(&hover);
    assert!(
        text.contains("The animal names"),
        "should include docblock description: {}",
        text
    );
    assert!(
        text.contains("@var list<string>"),
        "should show effective docblock type as @var annotation: {}",
        text
    );
}

#[test]
fn hover_method_shows_docblock_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Greeter {
    /**
     * Say hello to someone.
     * @param string $name The person's name
     * @return string
     */
    public function greet(string $name): string {
        return "Hello, $name!";
    }
    public function run(): void {
        $this->greet('World');
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 11, 16).expect("expected hover on greet");
    let text = hover_text(&hover);
    assert!(
        text.contains("Say hello to someone."),
        "should include method docblock description: {}",
        text
    );
}

#[test]
fn hover_constant_shows_docblock_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    /** The maximum retry count. */
    const MAX_RETRIES = 3;
}
class Worker {
    public function run(): void {
        echo Config::MAX_RETRIES;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 22).expect("expected hover on MAX_RETRIES");
    let text = hover_text(&hover);
    assert!(
        text.contains("The maximum retry count."),
        "should include constant docblock description: {}",
        text
    );
}

// ─── Native vs effective type display ───────────────────────────────────────

#[test]
fn hover_property_shows_native_type_in_code_block_and_effective_as_annotation() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;
class Pen {
    public string $color;
}
class ScaffoldingIteration {
    /** @var list<Pen> The batches */
    public array $batch;
    public function show(): void {
        echo $this->batch;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 9, 22).expect("expected hover on batch");
    let text = hover_text(&hover);

    // The effective (docblock) type should appear as a @var annotation with short names
    assert!(
        text.contains("@var list<Pen>"),
        "should show effective docblock type as @var annotation with short names: {}",
        text
    );
    // The description should appear
    assert!(
        text.contains("The batches"),
        "should show docblock description: {}",
        text
    );
    // The code block should use the native PHP type hint
    assert!(
        text.contains("public array $batch;"),
        "should show native type in PHP code block: {}",
        text
    );
    // The member should be wrapped with namespace line + short class name
    assert!(
        text.contains("namespace Demo;"),
        "should show namespace line: {}",
        text
    );
    assert!(
        text.contains("class ScaffoldingIteration {"),
        "should show short owning class name: {}",
        text
    );
}

#[test]
fn hover_property_without_docblock_type_shows_native_in_both() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Simple {
    public string $name;
    public function show(): void {
        echo $this->name;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 22).expect("expected hover on name");
    let text = hover_text(&hover);
    assert!(
        text.contains("public string $name;"),
        "should show native type in code block: {}",
        text
    );
}

#[test]
fn hover_method_shows_namespace_and_short_names_in_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App;
class User {
    public string $email;
}
class UserRepo {
    /**
     * Find all users.
     * @return list<User>
     */
    public function findAll(): array {
        return [];
    }
    public function run(): void {
        $this->findAll();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 14, 16).expect("expected hover on findAll");
    let text = hover_text(&hover);

    // The effective (docblock) return type should appear in the return section
    assert!(
        text.contains("**return** `list<User>`"),
        "should show effective return type with short names in return section: {}",
        text
    );
    // The code block should use the native PHP return type
    assert!(
        text.contains("function findAll(): array;"),
        "should show native return type in PHP code block: {}",
        text
    );
    // Description
    assert!(
        text.contains("Find all users."),
        "should show method docblock description: {}",
        text
    );
    // The method should be wrapped in the owning class
    assert!(
        text.contains("namespace App;"),
        "should show namespace line: {}",
        text
    );
    assert!(
        text.contains("class UserRepo {"),
        "should show short owning class name: {}",
        text
    );
}

#[test]
fn hover_contains_php_open_tag() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Box {
    public int $size;
    public function show(): void {
        echo $this->size;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 22).expect("expected hover on size");
    let text = hover_text(&hover);
    assert!(
        text.contains("<?php"),
        "should contain <?php marker in code block: {}",
        text
    );
}

#[test]
fn hover_function_shows_description_and_native_return() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Calculate the sum of values.
 * @param list<int> $values
 * @return int
 */
function total(array $values): int {
    return array_sum($values);
}
total([1, 2, 3]);
"#;

    let hover = hover_at(&backend, uri, content, 9, 2).expect("expected hover on total");
    let text = hover_text(&hover);
    assert!(
        text.contains("Calculate the sum of values."),
        "should show function docblock description: {}",
        text
    );
    assert!(
        text.contains("<?php"),
        "should contain <?php marker: {}",
        text
    );
}

// ─── Variable hover format ──────────────────────────────────────────────────

#[test]
fn hover_variable_shows_type_in_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Order {
    public string $id;
}
class Service {
    public function run(): void {
        $order = new Order();
        $order->id;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 9).expect("expected hover on $order");
    let text = hover_text(&hover);
    // Code block should show variable = type inside <?php block
    assert!(
        text.contains("$order = Order"),
        "should show variable with type in code block: {}",
        text
    );
    assert!(
        text.contains("<?php"),
        "should contain <?php marker: {}",
        text
    );
}

#[test]
fn hover_variable_without_type_shows_php_tag() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
function test() {
    $x = 42;
    echo $x;
}
"#;

    let hover = hover_at(&backend, uri, content, 3, 10).expect("expected hover on $x");
    let text = hover_text(&hover);
    assert!(
        text.contains("<?php"),
        "should contain <?php marker for unresolved variable: {}",
        text
    );
}

// ─── self / static / parent / $this hover format ────────────────────────────

#[test]
fn hover_self_shows_namespace_and_short_name_in_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App;
class Foo {
    public static function make(): self {
        return new self();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 20).expect("expected hover on self");
    let text = hover_text(&hover);
    assert!(
        text.contains("namespace App;"),
        "should show namespace line: {}",
        text
    );
    assert!(
        text.contains("self = Foo"),
        "should show self = short name in code block: {}",
        text
    );
    assert!(
        text.contains("<?php"),
        "should contain <?php marker: {}",
        text
    );
}

#[test]
fn hover_parent_shows_fqn_in_header_and_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App;
class Base {
    public function hello(): string { return 'hi'; }
}
class Child extends Base {
    public function hello(): string {
        return parent::hello();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover on parent");
    let text = hover_text(&hover);
    assert!(text.contains("parent"), "should mention parent: {}", text);
    assert!(
        text.contains("<?php"),
        "should contain <?php marker: {}",
        text
    );
}

#[test]
fn hover_this_shows_namespace_and_short_name_in_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App;
class Widget {
    public function run(): void {
        $this->run();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 9).expect("expected hover on $this");
    let text = hover_text(&hover);
    assert!(
        text.contains("namespace App;"),
        "should show namespace line: {}",
        text
    );
    assert!(
        text.contains("$this = Widget"),
        "should show $this = short name in code block: {}",
        text
    );
    assert!(
        text.contains("<?php"),
        "should contain <?php marker: {}",
        text
    );
}

#[test]
fn hover_self_includes_class_docblock() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * A reusable widget.
 */
class Widget {
    public static function make(): self {
        return new self();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 6, 20).expect("expected hover on self");
    let text = hover_text(&hover);
    assert!(
        text.contains("A reusable widget."),
        "should include class docblock description: {}",
        text
    );
}

#[test]
fn hover_self_shows_deprecated_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @deprecated Use NewWidget instead.
 */
class OldWidget {
    public static function make(): self {
        return new self();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 6, 20).expect("expected hover on self");
    let text = hover_text(&hover);
    assert!(
        text.contains("🪦 **deprecated** Use NewWidget instead."),
        "should show deprecated with message: {}",
        text
    );
}

// ─── Constant reference hover format ────────────────────────────────────────

#[test]
fn hover_class_constant_shows_php_tag_and_const_syntax() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    const APP_VERSION = '1.0.0';
}
class Usage {
    public function run(): void {
        echo Config::APP_VERSION;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 6, 24).expect("expected hover on APP_VERSION");
    let text = hover_text(&hover);
    assert!(
        text.contains("<?php"),
        "should contain <?php marker: {}",
        text
    );
    assert!(
        text.contains("const APP_VERSION = '1.0.0';"),
        "should show const declaration with value: {}",
        text
    );
    // Constant should be wrapped in its owning class
    assert!(
        text.contains("class Config {"),
        "should show owning class wrapper: {}",
        text
    );
}

#[test]
fn hover_class_constant_shows_integer_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Limits {
    const MAX_RETRIES = 3;
}
$x = Limits::MAX_RETRIES;
"#;

    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("const MAX_RETRIES = 3;"),
        "should show integer value: {}",
        text
    );
}

#[test]
fn hover_class_constant_shows_array_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    const ALLOWED = ['a', 'b', 'c'];
}
$x = Config::ALLOWED;
"#;

    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("const ALLOWED = ['a', 'b', 'c'];"),
        "should show array value: {}",
        text
    );
}

#[test]
fn hover_typed_constant_shows_type_and_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    const string APP_NAME = 'PHPantom';
}
$x = Config::APP_NAME;
"#;

    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("const APP_NAME: string = 'PHPantom';"),
        "should show type hint and value: {}",
        text
    );
}

#[test]
fn hover_constant_via_self_shows_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    const TIMEOUT = 30;
    public function get(): int {
        return self::TIMEOUT;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 22).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("const TIMEOUT = 30;"),
        "should show value via self::: {}",
        text
    );
}

#[test]
fn hover_constant_expression_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Math {
    const TWO_PI = 2 * 3.14159;
}
$x = Math::TWO_PI;
"#;

    let hover = hover_at(&backend, uri, content, 4, 14).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("const TWO_PI = 2 * 3.14159;"),
        "should show expression value: {}",
        text
    );
}

// ─── Native param types in code block ───────────────────────────────────────

#[test]
fn hover_method_shows_native_param_types_in_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App;
class User {
    public string $email;
}
class UserRepo {
    /**
     * Find users by criteria.
     * @param list<User> $criteria
     * @return list<User>
     */
    public function find(array $criteria): array {
        return [];
    }
    public function run(): void {
        $this->find([]);
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 15, 16).expect("expected hover on find");
    let text = hover_text(&hover);
    // The PHP code block should show the native param type (array), not the docblock type
    assert!(
        text.contains("function find(array $criteria)"),
        "should show native param type 'array' in PHP code block: {}",
        text
    );
}

#[test]
fn hover_function_shows_native_param_types_in_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class User {
    public string $name;
}
/**
 * Process users.
 * @param list<User> $users
 */
function processUsers(array $users): void {}
processUsers([]);
"#;

    let hover = hover_at(&backend, uri, content, 9, 2).expect("expected hover on processUsers");
    let text = hover_text(&hover);
    // The PHP code block should show the native param type (array), not the docblock type
    assert!(
        text.contains("function processUsers(array $users)"),
        "should show native param type 'array' in PHP code block: {}",
        text
    );
}

// ─── Unresolved fallback hover format ───────────────────────────────────────

#[test]
fn hover_unresolved_function_shows_php_tag() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
unknownFunction();
"#;

    backend.update_ast(uri, content);
    let hover = backend.handle_hover(
        uri,
        content,
        Position {
            line: 1,
            character: 5,
        },
    );
    // If hover is returned for an unknown function, it should use the new format
    if let Some(h) = hover {
        let text = hover_text(&h);
        assert!(
            text.contains("<?php"),
            "should contain <?php marker for unresolved function: {}",
            text
        );
    }
}

// ─── @param description tests ───────────────────────────────────────────────

#[test]
fn hover_function_shows_param_descriptions() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Process a batch of items.
 * @param list<string> $items The items to process.
 * @param bool $force Whether to force processing.
 */
function processBatch(array $items, bool $force = false): void {}
processBatch([]);
"#;

    let hover = hover_at(&backend, uri, content, 7, 2).expect("expected hover on processBatch");
    let text = hover_text(&hover);
    assert!(
        text.contains("**$items** `list<string>`"),
        "should show param name and effective type: {}",
        text
    );
    assert!(
        text.contains("The items to process."),
        "should show param description: {}",
        text
    );
}

#[test]
fn hover_method_shows_param_descriptions() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Processor {
    /**
     * Process a single item.
     * @param list<int> $ids The IDs to process.
     * @return bool
     */
    public function process(array $ids): bool {
        return true;
    }
    public function run(): void {
        $this->process([]);
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 11, 16).expect("expected hover on process");
    let text = hover_text(&hover);
    assert!(
        text.contains("**$ids** `list<int>`"),
        "should show param name and effective type for method: {}",
        text
    );
    assert!(
        text.contains("The IDs to process."),
        "should show param description for method: {}",
        text
    );
}

// ─── @param suppression tests ───────────────────────────────────────────────

#[test]
fn hover_param_not_shown_when_native_equals_effective_and_no_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Simple function.
 * @param string $name
 */
function greet(string $name): void {}
greet('World');
"#;

    let hover = hover_at(&backend, uri, content, 6, 2).expect("expected hover on greet");
    let text = hover_text(&hover);
    assert!(
        !text.contains("@param"),
        "should NOT show @param when native == effective and no description: {}",
        text
    );
}

#[test]
fn hover_param_shown_when_type_differs_but_no_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Takes a list.
 * @param list<int> $items
 */
function sum(array $items): int { return 0; }
sum([]);
"#;

    let hover = hover_at(&backend, uri, content, 6, 2).expect("expected hover on sum");
    let text = hover_text(&hover);
    assert!(
        text.contains("**$items** `list<int>`"),
        "should show param when effective type differs from native even without description: {}",
        text
    );
}

// ─── @return description tests ──────────────────────────────────────────────

#[test]
fn hover_method_shows_return_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Repo {
    /**
     * Find all records.
     * @return list<string> The matching records.
     */
    public function findAll(): array {
        return [];
    }
    public function run(): void {
        $this->findAll();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 10, 16).expect("expected hover on findAll");
    let text = hover_text(&hover);
    assert!(
        text.contains("**return** `list<string>`"),
        "should show return type: {}",
        text
    );
    assert!(
        text.contains("The matching records."),
        "should show return description: {}",
        text
    );
}

#[test]
fn hover_function_shows_return_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Get all names.
 * @return list<string> All available names.
 */
function getNames(): array { return []; }
getNames();
"#;

    let hover = hover_at(&backend, uri, content, 6, 2).expect("expected hover on getNames");
    let text = hover_text(&hover);
    assert!(
        text.contains("**return** `list<string>`"),
        "should show return type for standalone function: {}",
        text
    );
    assert!(
        text.contains("All available names."),
        "should show return description for standalone function: {}",
        text
    );
}

// ─── @link URL tests ────────────────────────────────────────────────────────

#[test]
fn hover_function_shows_link_url() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Map over an array.
 * @link https://php.net/manual/en/function.array-map.php
 * @param callable $callback The callback.
 * @return array The mapped array.
 */
function my_map(callable $callback, array $items): array { return []; }
my_map(fn($x) => $x, []);
"#;

    let hover = hover_at(&backend, uri, content, 8, 2).expect("expected hover on my_map");
    let text = hover_text(&hover);
    assert!(
        text.contains("https://php.net/manual/en/function.array-map.php"),
        "should show @link URL in hover output: {}",
        text
    );
    // The URL should appear outside the code block (before it)
    let url_pos = text
        .find("https://php.net/manual/en/function.array-map.php")
        .unwrap();
    let code_pos = text.find("```php").unwrap();
    assert!(
        url_pos < code_pos,
        "URL should appear before the code block: {}",
        text
    );
}

#[test]
fn hover_method_shows_link_url() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Helper {
    /**
     * Do something useful.
     * @link https://example.com/docs
     */
    public function doStuff(): void {}
    public function run(): void {
        $this->doStuff();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 8, 16).expect("expected hover on doStuff");
    let text = hover_text(&hover);
    assert!(
        text.contains("https://example.com/docs"),
        "should show @link URL for method hover: {}",
        text
    );
}

// ─── Combined annotations test ──────────────────────────────────────────────

#[test]
fn hover_function_shows_combined_param_and_return_annotations() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Transform items.
 * @link https://example.com/transform
 * @param list<int> $items The input items.
 * @param callable $fn The transform function.
 * @return list<string> The transformed items.
 */
function transform(array $items, callable $fn): array { return []; }
transform([], fn($x) => (string)$x);
"#;

    let hover = hover_at(&backend, uri, content, 9, 2).expect("expected hover on transform");
    let text = hover_text(&hover);
    assert!(
        text.contains("Transform items."),
        "should show description: {}",
        text
    );
    assert!(
        text.contains("https://example.com/transform"),
        "should show link URL: {}",
        text
    );
    assert!(
        text.contains("**$items** `list<int>`"),
        "should show param for items: {}",
        text
    );
    assert!(
        text.contains("The input items."),
        "should show param description for items: {}",
        text
    );
    assert!(
        text.contains("**return** `list<string>`"),
        "should show return type: {}",
        text
    );
    assert!(
        text.contains("The transformed items."),
        "should show return description: {}",
        text
    );
    assert!(
        text.contains("function transform(array $items, callable $fn): array;"),
        "should show native signature: {}",
        text
    );
}

// ─── Param with description but same type ───────────────────────────────────

#[test]
fn hover_param_shown_when_types_match_but_has_description() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Say hello.
 * @param string $name The person's name to greet.
 */
function sayHello(string $name): void {}
sayHello('Alice');
"#;

    let hover = hover_at(&backend, uri, content, 6, 2).expect("expected hover on sayHello");
    let text = hover_text(&hover);
    assert!(
        text.contains("**$name** The person's name to greet."),
        "should show param with description when types match: {}",
        text
    );
}

// ─── Docblock type shown even when matching native type ─────────────────────

#[test]
fn hover_shows_docblock_param_and_return_when_types_match_native() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Applies the callback to the elements of the given arrays
 * @link https://php.net/manual/en/function.array-map.php
 * @param callable|null $callback Callback function to run for each element in each array.
 * @param array $array An array to run through the callback function.
 * @param array ...$arrays
 * @return array an array containing all the elements of arr1
 * after applying the callback function to each one.
 */
function array_map(?callable $callback, array $array, array ...$arrays): array {}
array_map(null, []);
"#;

    let hover = hover_at(&backend, uri, content, 11, 2).expect("expected hover on array_map");
    let text = hover_text(&hover);

    // Description
    assert!(
        text.contains("Applies the callback to the elements of the given arrays"),
        "should show description: {}",
        text
    );

    // Link
    assert!(
        text.contains("https://php.net/manual/en/function.array-map.php"),
        "should show link URL: {}",
        text
    );

    // $callback's docblock type `callable|null` is semantically equivalent to
    // native `?callable`, so types match — description only, no backtick type.
    assert!(
        text.contains("**$callback** Callback function to run for each element in each array."),
        "should show $callback with description (types match after nullable normalisation): {}",
        text
    );
    // $array's types match (array == array), so description only.
    assert!(
        text.contains("**$array** An array to run through the callback function."),
        "should show $array with description (types match): {}",
        text
    );

    // $arrays has a @param tag but no description and types match — should NOT show.
    assert!(
        !text.contains("**$arrays**"),
        "should NOT show $arrays param entry (no description, types match): {}",
        text
    );

    // @return types match (array == array), so description only.
    assert!(
        text.contains("**return** an array containing all the elements of arr1 after applying the callback function to each one."),
        "should show return with description (types match): {}",
        text
    );

    // The code block should use native types.
    assert!(
        text.contains(
            "function array_map(?callable $callback, array $array, array ...$arrays): array;"
        ),
        "should show native signature in code block: {}",
        text
    );
}

// ─── Rich callable signature differs from native ────────────────────────────

#[test]
fn hover_shows_rich_callable_type_when_docblock_refines_native() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Applies the callback to the elements of the given arrays
 * @param (callable(mixed $item): mixed)|null $callback Callback function to run for each element.
 * @param array $array An array to run through the callback function.
 * @return array the mapped array.
 */
function array_map(?callable $callback, array $array): array {}
array_map(null, []);
"#;

    let hover = hover_at(&backend, uri, content, 8, 2).expect("expected hover on array_map");
    let text = hover_text(&hover);

    // $callback's effective type `(callable(mixed $item): mixed)|null` is richer
    // than native `?callable`, so it shows with backtick type + description.
    assert!(
        text.contains("**$callback** `(callable(mixed $item): mixed)|null`"),
        "should show $callback with rich effective type: {}",
        text
    );
    assert!(
        text.contains("Callback function to run for each element."),
        "should show $callback description: {}",
        text
    );
}

// ─── @var annotation suppression for equivalent types ───────────────────────

#[test]
fn hover_property_suppresses_var_when_effective_is_fqn_of_native() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;
class Brush {
    public string $color;
}
class Easel {
    /** @var Brush */
    public Brush $brush;
    public function show(): void {
        echo $this->brush;
    }
}
"#;

    // Hover on `brush` in `$this->brush` (line 9)
    let hover = hover_at(&backend, uri, content, 9, 22).expect("expected hover on brush");
    let text = hover_text(&hover);
    // The effective type is `Demo\Brush` and the native type is `Brush`.
    // These refer to the same class, so the @var annotation should be suppressed.
    assert!(
        !text.contains("@var"),
        "should NOT show @var when effective type is just FQN of native type: {}",
        text
    );
    assert!(
        text.contains("public Brush $brush;"),
        "should still show native type in code block: {}",
        text
    );
}

#[test]
fn hover_property_shows_var_when_effective_genuinely_differs() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;
class Pen {
    public string $color;
}
class Drawer {
    /** @var list<Pen> */
    public array $pens;
    public function show(): void {
        echo $this->pens;
    }
}
"#;

    // Hover on `pens` in `$this->pens` (line 9)
    let hover = hover_at(&backend, uri, content, 9, 22).expect("expected hover on pens");
    let text = hover_text(&hover);
    // The effective type `list<Demo\Pen>` genuinely differs from the native `array`.
    assert!(
        text.contains("@var list<Pen>"),
        "should show @var with short names when effective type genuinely differs from native: {}",
        text
    );
}

#[test]
fn hover_property_suppresses_var_when_fqn_with_leading_backslash() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App;
class Widget {}
class Factory {
    /** @var \App\Widget */
    public Widget $widget;
    public function show(): void {
        echo $this->widget;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 22).expect("expected hover on widget");
    let text = hover_text(&hover);
    assert!(
        !text.contains("@var"),
        "should suppress @var for FQN with leading backslash: {}",
        text
    );
}

#[test]
fn hover_method_suppresses_return_annotation_when_fqn_matches_native() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;
class Item {}
class Store {
    /** @return Item */
    public function getItem(): Item { return new Item(); }
    public function run(): void {
        $this->getItem();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover on getItem");
    let text = hover_text(&hover);
    // The effective return type `Demo\Item` is just FQN of native `Item`.
    // The return annotation should be suppressed.
    assert!(
        !text.contains("**return**"),
        "should suppress return annotation when FQN matches native: {}",
        text
    );
}

#[test]
fn hover_method_shows_return_annotation_when_types_genuinely_differ() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;
class Item {}
class Store {
    /** @return list<Item> */
    public function getItems(): array { return []; }
    public function run(): void {
        $this->getItems();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover on getItems");
    let text = hover_text(&hover);
    assert!(
        text.contains("**return** `list<Item>`"),
        "should show return annotation with short names when effective genuinely differs: {}",
        text
    );
}

// ─── new ClassName hover ────────────────────────────────────────────────────

#[test]
fn hover_new_class_shows_constructor() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Widget {
    /**
     * Create a new Widget.
     *
     * @param string $name The widget name
     */
    public function __construct(string $name) {}

    public function run(): void {}
}

function demo(): void {
    $w = new Widget("hello");
}
"#;

    // Hover on `Widget` in `new Widget("hello")` (line 13, "Widget" starts at col 14)
    let hover = hover_at(&backend, uri, content, 13, 15).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("__construct"),
        "should show __construct method, got: {}",
        text
    );
    assert!(
        text.contains("string $name"),
        "should show constructor params: {}",
        text
    );
    assert!(
        text.contains("Create a new Widget"),
        "should show constructor description: {}",
        text
    );
}

#[test]
fn hover_new_class_shows_constructor_default_values() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Zoo {
    public function __construct(
        int $buffalo = 0,
        string $name = 'default',
        ?array $items = null,
        bool $active = true
    ) {}
}

function demo(): void {
    $z = new Zoo();
}
"#;

    // Hover on `Zoo` in `new Zoo()` (line 11)
    let hover = hover_at(&backend, uri, content, 11, 15).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("__construct"),
        "should show __construct: {}",
        text
    );
    assert!(
        text.contains("int $buffalo = 0"),
        "should show int default value, got: {}",
        text
    );
    assert!(
        text.contains("string $name = 'default'"),
        "should show string default value, got: {}",
        text
    );
    assert!(
        text.contains("?array $items = null"),
        "should show null default value, got: {}",
        text
    );
    assert!(
        text.contains("bool $active = true"),
        "should show bool default value, got: {}",
        text
    );
    // Should NOT contain `= ...` placeholder
    assert!(
        !text.contains("= ..."),
        "should not contain placeholder '= ...', got: {}",
        text
    );
}

#[test]
fn hover_method_shows_default_values() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Formatter {
    public function format(string $text, int $indent = 4, string $sep = ', '): string {
        return $text;
    }
    public function run(): void {
        $this->format('hello');
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover on format");
    let text = hover_text(&hover);
    assert!(
        text.contains("int $indent = 4"),
        "should show int default: {}",
        text
    );
    assert!(
        text.contains("string $sep = ', '"),
        "should show string default: {}",
        text
    );
    assert!(
        !text.contains("= ..."),
        "should not contain placeholder: {}",
        text
    );
}

#[test]
fn hover_method_shows_array_default_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    public function load(array $options = []): void {}
    public function run(): void {
        $this->load();
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover on load");
    let text = hover_text(&hover);
    assert!(
        text.contains("array $options = []"),
        "should show empty array default: {}",
        text
    );
}

#[test]
fn hover_class_reference_without_new_shows_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Widget {
    public function __construct(string $name) {}
}

function demo(Widget $w): void {}
"#;

    // Hover on `Widget` in the parameter type hint (line 5, "Widget" starts at col 15)
    let hover = hover_at(&backend, uri, content, 5, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("class"),
        "should show class kind, got: {}",
        text
    );
    assert!(
        !text.contains("__construct"),
        "should NOT show __construct for a type-hint reference, got: {}",
        text
    );
}

#[test]
fn hover_new_class_without_constructor_shows_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class EmptyClass {}

function demo(): void {
    $e = new EmptyClass();
}
"#;

    // Hover on `EmptyClass` in `new EmptyClass()` (line 4)
    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("class"),
        "should fall back to class hover when no __construct: {}",
        text
    );
    assert!(
        text.contains("EmptyClass"),
        "should show class name: {}",
        text
    );
}

#[test]
fn hover_new_class_shows_inherited_constructor() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Base {
    /** Build a base instance. */
    public function __construct(int $id) {}
}
class Child extends Base {}

function demo(): void {
    $c = new Child(42);
}
"#;

    // Hover on `Child` in `new Child(42)` (line 8)
    let hover = hover_at(&backend, uri, content, 8, 14).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("__construct"),
        "should show inherited __construct: {}",
        text
    );
    assert!(
        text.contains("int $id"),
        "should show inherited constructor params: {}",
        text
    );
}

#[test]
fn hover_static_method_context_shows_class_not_constructor() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Factory {
    public function __construct(string $name) {}
    public static function create(): self { return new self("x"); }
}

function demo(): void {
    Factory::create();
}
"#;

    // Hover on `Factory` in `Factory::create()` (line 7) — NOT a `new` context
    let hover = hover_at(&backend, uri, content, 7, 5).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("class"),
        "should show class hover for static access, got: {}",
        text
    );
    assert!(
        !text.contains("__construct"),
        "should NOT show __construct for static access context, got: {}",
        text
    );
}

// ─── Class template display ─────────────────────────────────────────────────

/// Hovering a generic class shows its template parameters with variance and bounds.
#[test]
fn hover_class_shows_template_params() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TKey
 * @template TValue
 */
class Collection {
    /** @return TValue */
    public function first(): mixed { return null; }
}

function test(Collection $c): void {}
"#;

    // Hover on `Collection` in the function parameter (line 10)
    let hover = hover_at(&backend, uri, content, 10, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template** `TKey`"),
        "should show TKey template param, got: {}",
        text
    );
    assert!(
        text.contains("**template** `TValue`"),
        "should show TValue template param, got: {}",
        text
    );
}

#[test]
fn hover_class_shows_covariant_template_with_bound() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TKey of array-key
 * @template-covariant TValue of object
 */
class TypedMap {}

function test(TypedMap $m): void {}
"#;

    // Hover on `TypedMap` in the function parameter (line 7)
    let hover = hover_at(&backend, uri, content, 7, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template** `TKey` of `array-key`"),
        "should show TKey with bound, got: {}",
        text
    );
    assert!(
        text.contains("**template-covariant** `TValue` of `object`"),
        "should show TValue as covariant with bound, got: {}",
        text
    );
}

#[test]
fn hover_class_shows_contravariant_template() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template-contravariant TInput
 */
class Consumer {}

function test(Consumer $c): void {}
"#;

    // Hover on `Consumer` in the function parameter (line 6)
    let hover = hover_at(&backend, uri, content, 6, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template-contravariant** `TInput`"),
        "should show TInput as contravariant, got: {}",
        text
    );
}

#[test]
fn hover_interface_shows_template_params() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TKey
 * @template-covariant TValue
 * @template-extends iterable<TKey, TValue>
 */
interface Traversable extends iterable {}

function test(Traversable $t): void {}
"#;

    // Hover on `Traversable` in the function parameter (line 8)
    let hover = hover_at(&backend, uri, content, 8, 17).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template** `TKey`"),
        "should show TKey template param, got: {}",
        text
    );
    assert!(
        text.contains("**template-covariant** `TValue`"),
        "should show TValue as covariant, got: {}",
        text
    );
}

#[test]
fn hover_template_param_shows_covariant_variance() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template-covariant TValue
 */
class Box {
    /** @return TValue */
    public function get(): mixed { return null; }
}
"#;

    // Hover on `TValue` in `@return TValue` (line 5)
    let hover = hover_at(&backend, uri, content, 5, 19).expect("expected hover on TValue");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template-covariant**"),
        "should show covariant variance, got: {}",
        text
    );
    assert!(
        text.contains("`TValue`"),
        "should show the template name, got: {}",
        text
    );
}

// ─── Template parameter hover ───────────────────────────────────────────────

/// Hovering a template parameter name in a docblock type position should
/// show `**template** \`TKey\` of \`array-key\`` rather than `class TKey`.
#[test]
fn hover_template_param_in_callable_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TKey of array-key
 * @template TValue
 */
class Collection {
    /**
     * @param callable(TValue, TKey): mixed $callback
     * @return static
     */
    public function each(callable $callback): static { return $this; }
}
"#;

    // Hover on `TKey` inside the callable param type (line 7)
    // `callable(TValue, TKey): mixed` — TKey starts around character 30
    let hover = hover_at(&backend, uri, content, 7, 31).expect("expected hover on TKey");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template**"),
        "should show template hover, got: {}",
        text
    );
    assert!(
        text.contains("`TKey`"),
        "should show the template name, got: {}",
        text
    );
    assert!(
        text.contains("`array-key`"),
        "should show the bound type, got: {}",
        text
    );
    assert!(
        !text.contains("class TKey"),
        "should NOT show 'class TKey', got: {}",
        text
    );
}

/// Template parameter without an `of` bound shows just the name.
#[test]
fn hover_template_param_without_bound() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TValue
 */
class Box {
    /** @return TValue */
    public function get(): mixed { return null; }
}
"#;

    // Hover on `TValue` in `@return TValue` (line 5)
    let hover = hover_at(&backend, uri, content, 5, 19).expect("expected hover on TValue");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template**"),
        "should show template hover, got: {}",
        text
    );
    assert!(
        text.contains("`TValue`"),
        "should show the template name, got: {}",
        text
    );
    assert!(
        !text.contains(" of "),
        "should NOT show 'of' when there is no bound, got: {}",
        text
    );
}

/// Template parameter with a class-like bound shows the bound.
#[test]
fn hover_template_param_with_class_bound() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Animal {}
/**
 * @template T of Animal
 */
class Zoo {
    /** @return T */
    public function first(): mixed { return null; }
}
"#;

    // Hover on `T` in `@return T` (line 6)
    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover on T");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template**"),
        "should show template hover, got: {}",
        text
    );
    assert!(
        text.contains("`T`"),
        "should show the template name, got: {}",
        text
    );
    assert!(
        text.contains("`Animal`"),
        "should show the bound class, got: {}",
        text
    );
}

/// Method-level template parameter shows hover within the method body.
#[test]
fn hover_method_level_template_param() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Util {
    /**
     * @template TItem of object
     * @param TItem $item
     * @return TItem
     */
    public function identity(object $item): object { return $item; }
}
"#;

    // Hover on `TItem` in `@param TItem $item` (line 4)
    let hover = hover_at(&backend, uri, content, 4, 14).expect("expected hover on TItem");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template**"),
        "should show template hover, got: {}",
        text
    );
    assert!(
        text.contains("`TItem`"),
        "should show the template name, got: {}",
        text
    );
    assert!(
        text.contains("`object`"),
        "should show the bound, got: {}",
        text
    );
}

/// Hovering a fully-qualified class name (`\stdClass`) inside a docblock
/// in a namespaced file should resolve the class via the FQN path, not
/// prepend the current namespace.
#[test]
fn hover_fqn_class_in_docblock_resolves_stub() {
    let backend = create_test_backend_with_stdclass_stub();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Models;

class Repo {
    /** @return \stdClass */
    public function find(): \stdClass { return new \stdClass(); }
}
"#;

    // Hover on `\stdClass` in the @return tag (line 4, on "stdClass" portion)
    let hover = hover_at(&backend, uri, content, 4, 19).expect("expected hover on \\stdClass");
    let text = hover_text(&hover);
    assert!(
        text.contains("class stdClass"),
        "should resolve stdClass from stubs, got: {}",
        text
    );
    assert!(
        !text.contains("class stdClass;"),
        "should not show the unknown-class fallback (with semicolon), got: {}",
        text
    );
    // The stub docblock has @link — verify it appears in hover.
    assert!(
        text.contains("php.net"),
        "should include the @link URL from the stub docblock, got: {}",
        text
    );
    // The stub docblock has a description — verify it appears in hover.
    assert!(
        text.contains("Created by typecasting to object"),
        "should include the docblock description from the stub, got: {}",
        text
    );
}

/// Same as above but with a FQN inside a generic type argument:
/// `Collection<int, \stdClass>`.
#[test]
fn hover_fqn_class_in_generic_arg_resolves_stub() {
    let backend = create_test_backend_with_stdclass_stub();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Models;

class Repo {
    /** @return array<int, \stdClass> */
    public function all(): array { return []; }
}
"#;

    // Hover on `\stdClass` inside the generic (line 4)
    let hover = hover_at(&backend, uri, content, 4, 30).expect("expected hover on \\stdClass");
    let text = hover_text(&hover);
    assert!(
        text.contains("class stdClass"),
        "should resolve stdClass from stubs inside generic arg, got: {}",
        text
    );
    assert!(
        !text.contains("class stdClass;"),
        "should not show the unknown-class fallback, got: {}",
        text
    );
}

/// A user-defined class with a `@link` tag should display the URL in hover.
#[test]
fn hover_class_with_link_tag() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * Handles user authentication.
 * @link https://example.com/docs/auth
 */
class AuthService {}

function demo(): void {
    $a = new AuthService();
}
"#;

    // Hover on `AuthService` in `new AuthService()` — but since there's
    // no constructor, it falls through to class hover.
    let hover = hover_at(&backend, uri, content, 8, 14).expect("expected hover on AuthService");
    let text = hover_text(&hover);
    assert!(
        text.contains("class AuthService"),
        "should show class name, got: {}",
        text
    );
    assert!(
        text.contains("Handles user authentication"),
        "should show docblock description, got: {}",
        text
    );
    assert!(
        text.contains("https://example.com/docs/auth"),
        "should show @link URL, got: {}",
        text
    );
}

#[test]
fn hover_closure_in_parenthesized_callable_union() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Builder {
    /**
     * @param  (\Closure(static): mixed)|string|array  $column
     * @return $this
     */
    public function where($column) {}
}
"#;

    // Hover on `\Closure` inside `(\Closure(static): mixed)` (line 3).
    // The `\` is at column 15, `Closure` spans columns 16–22.
    let hover = hover_at(&backend, uri, content, 3, 16).expect("expected hover on \\Closure");
    let text = hover_text(&hover);
    assert!(
        text.contains("class Closure"),
        "should show Closure class info, got: {}",
        text
    );
    // Must NOT contain the leading `(` in the class name.
    assert!(
        !text.contains("(\\Closure"),
        "should not include leading paren in class name, got: {}",
        text
    );
}

#[test]
fn hover_template_param_in_use_tag_generic_arg() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TModel of \stdClass
 */
class Builder {
    /** @use SomeTrait<TModel> */
    use SomeTrait;
}
"#;

    // Hover on `TModel` inside `@use SomeTrait<TModel>` (line 5).
    let hover = hover_at(&backend, uri, content, 5, 24).expect("expected hover on TModel");
    let text = hover_text(&hover);
    assert!(
        text.contains("template") && text.contains("TModel"),
        "should show template param info for TModel, got: {}",
        text
    );
}

#[test]
fn hover_static_in_docblock_generic_arg() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Model {
    /** @return Builder<static> */
    public static function query() {}
}
"#;

    // Hover on `static` inside `Builder<static>` (line 2).
    let hover = hover_at(&backend, uri, content, 2, 25).expect("expected hover on static");
    let text = hover_text(&hover);
    assert!(
        text.contains("Model"),
        "should resolve static to the enclosing class Model, got: {}",
        text
    );
}

#[test]
fn hover_backed_enum_case_shows_case_syntax_and_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
enum OrderStatus: string {
    case Pending = 'pending';
    case Processing = 'processing';

    public function isPending(): bool { return $this === self::Pending; }
}
"#;

    // Hover on `Pending` in `self::Pending` (line 5).
    let hover = hover_at(&backend, uri, content, 5, 63).expect("expected hover on Pending");
    let text = hover_text(&hover);
    assert!(
        text.contains("case Pending = 'pending';"),
        "should show enum case syntax with value, got: {}",
        text
    );
    assert!(
        text.contains("enum OrderStatus: string"),
        "should show enum keyword with backing type, got: {}",
        text
    );
    assert!(
        !text.contains("class "),
        "should not show 'class' for an enum, got: {}",
        text
    );
    assert!(
        !text.contains("const "),
        "should not show 'const' for an enum case, got: {}",
        text
    );
}

#[test]
fn hover_unit_enum_case_shows_case_syntax_without_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
enum Suit {
    case Hearts;
    case Diamonds;

    public function isRed(): bool { return $this === self::Hearts; }
}
"#;

    // Hover on `Hearts` in `self::Hearts` (line 5).
    let hover = hover_at(&backend, uri, content, 5, 59).expect("expected hover on Hearts");
    let text = hover_text(&hover);
    assert!(
        text.contains("case Hearts;"),
        "should show enum case syntax without value, got: {}",
        text
    );
    assert!(
        text.contains("enum Suit"),
        "should show enum keyword without backing type, got: {}",
        text
    );
    assert!(
        !text.contains("enum Suit:"),
        "should not show colon for unit enum, got: {}",
        text
    );
    assert!(
        !text.contains("const "),
        "should not show 'const' for a unit enum case, got: {}",
        text
    );
}

#[test]
fn hover_method_without_native_param_types_omits_docblock_types_from_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Builder {
    /**
     * @param  (\Closure(static): mixed)|string|array  $column
     * @return $this
     */
    public function where($column, $operator = null, $value = null, $boolean = 'and') {}

    public function run(): void {
        $this->where('active', true);
    }
}
"#;

    // Hover on `where` in `$this->where(...)` (line 9).
    let hover = hover_at(&backend, uri, content, 9, 16).expect("expected hover on where");
    let text = hover_text(&hover);
    // The code block should show untyped params (no native types exist),
    // NOT the docblock type `(\Closure(static): mixed)|string|array`.
    assert!(
        text.contains("function where($column, $operator = null, $value = null, $boolean = 'and')"),
        "should show untyped params in PHP code block, got: {}",
        text
    );
    // The code block (between ```php fences) must not contain the docblock type.
    let code_block = text
        .split("```php")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .unwrap_or("");
    assert!(
        !code_block.contains("Closure"),
        "code block should not contain docblock Closure type, got code block: {}",
        code_block
    );
}

#[test]
fn hover_class_reference_in_property_default() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class FrostingCast {}
class Bread {
    protected $casts = [
        'icing' => FrostingCast::class,
    ];
}
"#;

    // Hover on `FrostingCast` in `FrostingCast::class` (line 4).
    let hover = hover_at(&backend, uri, content, 4, 20).expect("expected hover on FrostingCast");
    let text = hover_text(&hover);
    assert!(
        text.contains("FrostingCast"),
        "should show FrostingCast class info, got: {}",
        text
    );
}

#[test]
fn hover_class_in_multiline_docblock_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class SomeCollection {}
class Demo {
    /**
     * @return array<
     *   string,
     *   SomeCollection<int>
     * >
     */
    public function grouped() {}

    public function run(): void {
        $this->grouped();
    }
}
"#;

    // Hover on `SomeCollection` inside the multiline @return type (line 6).
    let hover = hover_at(&backend, uri, content, 6, 10).expect("expected hover on SomeCollection");
    let text = hover_text(&hover);
    assert!(
        text.contains("SomeCollection"),
        "should show SomeCollection class info, got: {}",
        text
    );
}

#[test]
fn hover_template_param_in_multiline_docblock_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template TValue
 */
class FluentCollection {
    /**
     * @return array<
     *   string,
     *   FluentCollection<int, TValue>
     * >
     */
    public function grouped() {}
}
"#;

    // Hover on `TValue` inside the multiline @return type (line 8).
    let hover = hover_at(&backend, uri, content, 8, 32).expect("expected hover on TValue");
    let text = hover_text(&hover);
    assert!(
        text.contains("template") && text.contains("TValue"),
        "should show template param info for TValue, got: {}",
        text
    );
}

// ── Anonymous class ─────────────────────────────────────────────────────────

#[test]
fn hover_anonymous_class_extends() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Animal {
    public string $species;
}
function make() {
    return new class extends Animal {};
}
"#;

    // Hover on `Animal` in `new class extends Animal` (line 5, col ~30).
    let hover = hover_at(&backend, uri, content, 5, 31).expect("expected hover on Animal");
    let text = hover_text(&hover);
    assert!(
        text.contains("Animal"),
        "should show Animal class info, got: {}",
        text
    );
}

#[test]
fn hover_anonymous_class_implements() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
interface Runnable {
    public function run(): void;
}
function make() {
    return new class implements Runnable {
        public function run(): void {}
    };
}
"#;

    // Hover on `Runnable` in `new class implements Runnable` (line 5).
    let hover = hover_at(&backend, uri, content, 5, 34).expect("expected hover on Runnable");
    let text = hover_text(&hover);
    assert!(
        text.contains("Runnable"),
        "should show Runnable interface info, got: {}",
        text
    );
}

#[test]
fn hover_anonymous_class_method_param_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Widget {}
function make() {
    return new class {
        public function process(Widget $w): void {}
    };
}
"#;

    // Hover on `Widget` in anonymous class method param (line 4).
    let hover = hover_at(&backend, uri, content, 4, 32).expect("expected hover on Widget");
    let text = hover_text(&hover);
    assert!(
        text.contains("Widget"),
        "should show Widget class info, got: {}",
        text
    );
}

// ── Top-level const ─────────────────────────────────────────────────────────

#[test]
fn hover_class_in_top_level_const_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Handler {}
const DEFAULT_HANDLER = Handler::class;
"#;

    // Hover on `Handler` in `Handler::class` (line 2, col ~24).
    let hover = hover_at(&backend, uri, content, 2, 24).expect("expected hover on Handler");
    let text = hover_text(&hover);
    assert!(
        text.contains("Handler"),
        "should show Handler class info, got: {}",
        text
    );
}

// ── Language constructs ─────────────────────────────────────────────────────

#[test]
fn hover_variable_inside_isset() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Config {
    public string $key;
}
function check(Config $cfg) {
    isset($cfg->key);
}
"#;

    // Hover on `key` inside `isset($cfg->key)` (line 5).
    let hover = hover_at(&backend, uri, content, 5, 16).expect("expected hover on key");
    let text = hover_text(&hover);
    assert!(
        text.contains("key"),
        "should show property info for key, got: {}",
        text
    );
}

#[test]
fn hover_variable_inside_empty() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Box {
    public string $label;
}
function check(Box $b) {
    empty($b->label);
}
"#;

    // Hover on `label` inside `empty($b->label)` (line 5).
    let hover = hover_at(&backend, uri, content, 5, 15).expect("expected hover on label");
    let text = hover_text(&hover);
    assert!(
        text.contains("label"),
        "should show property info for label, got: {}",
        text
    );
}

// ── String interpolation ────────────────────────────────────────────────────

#[test]
fn hover_variable_inside_interpolated_string() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Greeter {
    public string $name;
}
function greet(Greeter $g) {
    echo "Hello {$g->name}!";
}
"#;

    // Hover on `name` inside the interpolated string (line 5).
    let hover = hover_at(&backend, uri, content, 5, 22).expect("expected hover on name");
    let text = hover_text(&hover);
    assert!(
        text.contains("name"),
        "should show property info for name, got: {}",
        text
    );
}

// ── First-class callable ────────────────────────────────────────────────────

#[test]
fn hover_first_class_callable_static_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Formatter {
    public static function bold(string $text): string {
        return "<b>$text</b>";
    }
}
function test() {
    $fn = Formatter::bold(...);
}
"#;

    // Hover on `Formatter` in `Formatter::bold(...)` (line 7).
    let hover = hover_at(&backend, uri, content, 7, 10).expect("expected hover on Formatter");
    let text = hover_text(&hover);
    assert!(
        text.contains("Formatter"),
        "should show Formatter class info, got: {}",
        text
    );

    // Hover on `bold` in `Formatter::bold(...)` (line 7).
    let hover2 = hover_at(&backend, uri, content, 7, 22).expect("expected hover on bold");
    let text2 = hover_text(&hover2);
    assert!(
        text2.contains("bold"),
        "should show bold method info, got: {}",
        text2
    );
}

#[test]
fn hover_first_class_callable_instance_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Printer {
    public function printLine(string $line): void {}
}
function test(Printer $p) {
    $fn = $p->printLine(...);
}
"#;

    // Hover on `printLine` in `$p->printLine(...)` (line 5).
    let hover = hover_at(&backend, uri, content, 5, 15).expect("expected hover on printLine");
    let text = hover_text(&hover);
    assert!(
        text.contains("printLine"),
        "should show printLine method info, got: {}",
        text
    );
}

// ─── Origin indicator tests ─────────────────────────────────────────────────

#[test]
fn hover_method_override_shows_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Animal {
    public function speak(): string { return ''; }
}
class Dog extends Animal {
    public function speak(): string { return 'woof'; }
    public function run(): void {
        $this->speak();
    }
}
"#;

    // Hover on `speak` called on `$this` inside Dog (line 7).
    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("overrides **Animal**"),
        "should show override indicator, got: {}",
        text
    );
}

#[test]
fn hover_method_implements_shows_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
interface Loggable {
    public function log(string $msg): void;
}
class FileLogger implements Loggable {
    public function log(string $msg): void {}
    public function run(): void {
        $this->log('hi');
    }
}
"#;

    // Hover on `log` called on `$this` inside FileLogger (line 7).
    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("implements **Loggable**"),
        "should show implements indicator, got: {}",
        text
    );
}

#[test]
fn hover_method_override_and_implements_shows_both() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
interface Renderable {
    public function render(): string;
}
class BaseView {
    public function render(): string { return ''; }
}
class HtmlView extends BaseView implements Renderable {
    public function render(): string { return '<html>'; }
    public function test(): void {
        $this->render();
    }
}
"#;

    // Hover on `render` called on `$this` inside HtmlView (line 10).
    let hover = hover_at(&backend, uri, content, 10, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("overrides **BaseView**"),
        "should show override indicator, got: {}",
        text
    );
    assert!(
        text.contains("implements **Renderable**"),
        "should show implements indicator, got: {}",
        text
    );
}

#[test]
fn hover_virtual_method_shows_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @method string getName()
 */
class Magic {
    public function test(): void {
        $this->getName();
    }
}
"#;

    // Hover on `getName` called on `$this` inside Magic (line 6).
    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("virtual"),
        "should show virtual indicator, got: {}",
        text
    );
}

#[test]
fn hover_virtual_property_shows_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @property string $title
 */
class Document {
    public function test(): void {
        $this->title;
    }
}
"#;

    // Hover on `title` accessed on `$this` inside Document (line 6).
    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("virtual"),
        "should show virtual indicator for property, got: {}",
        text
    );
}

#[test]
fn hover_non_overriding_method_has_no_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Simple {
    public function doStuff(): void {}
    public function test(): void {
        $this->doStuff();
    }
}
"#;

    // Hover on `doStuff` called on `$this` (line 4).
    let hover = hover_at(&backend, uri, content, 4, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        !text.contains("overrides"),
        "should NOT show override indicator, got: {}",
        text
    );
    assert!(
        !text.contains("implements"),
        "should NOT show implements indicator, got: {}",
        text
    );
    assert!(
        !text.contains("virtual"),
        "should NOT show virtual indicator, got: {}",
        text
    );
}

#[test]
fn hover_constant_implements_interface_shows_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
interface HasVersion {
    const VERSION = '1.0';
}
class App implements HasVersion {
    const VERSION = '2.0';
    public function test(): void {
        self::VERSION;
    }
}
"#;

    // Hover on `VERSION` via `self::VERSION` (line 7).
    let hover = hover_at(&backend, uri, content, 7, 15).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("implements **HasVersion**"),
        "should show implements indicator for constant, got: {}",
        text
    );
}

#[test]
fn hover_property_override_shows_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Base {
    public string $label = '';
}
class Child extends Base {
    public string $label = 'child';
    public function test(): void {
        $this->label;
    }
}
"#;

    // Hover on `label` on `$this` inside Child (line 7).
    let hover = hover_at(&backend, uri, content, 7, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("overrides **Base**"),
        "should show override indicator for property, got: {}",
        text
    );
}

#[test]
fn hover_inherited_method_no_override_indicator() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class ParentClass {
    public function inherited(): void {}
}
class ChildClass extends ParentClass {
    public function test(): void {
        $this->inherited();
    }
}
"#;

    // Hover on `inherited` called on `$this` in ChildClass (line 6).
    // The method is inherited (not overridden), so no indicator.
    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        !text.contains("overrides"),
        "inherited method should NOT show override, got: {}",
        text
    );
}

#[test]
fn hover_cross_file_method_override_shows_indicator() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": { "App\\": "src/" }
            }
        }"#,
        &[
            (
                "src/Base.php",
                r#"<?php
namespace App;
class Base {
    public function process(): void {}
}
"#,
            ),
            (
                "src/Child.php",
                r#"<?php
namespace App;
class Child extends Base {
    public function process(): void {}
    public function test(): void {
        $this->process();
    }
}
"#,
            ),
        ],
    );

    let base_uri = format!("file://{}", _dir.path().join("src/Base.php").display());
    let base_content = std::fs::read_to_string(_dir.path().join("src/Base.php")).unwrap();
    backend.update_ast(&base_uri, &base_content);

    let child_uri = format!("file://{}", _dir.path().join("src/Child.php").display());
    let child_content = std::fs::read_to_string(_dir.path().join("src/Child.php")).unwrap();

    // Hover on `process` called on `$this` inside Child (line 5).
    let hover = hover_at(&backend, &child_uri, &child_content, 5, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("overrides **Base**"),
        "cross-file override should show indicator, got: {}",
        text
    );
}

#[test]
fn hover_cross_file_method_implements_shows_indicator() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": { "App\\": "src/" }
            }
        }"#,
        &[
            (
                "src/Loggable.php",
                r#"<?php
namespace App;
interface Loggable {
    public function log(string $msg): void;
}
"#,
            ),
            (
                "src/FileLogger.php",
                r#"<?php
namespace App;
class FileLogger implements Loggable {
    public function log(string $msg): void {}
    public function test(): void {
        $this->log('hi');
    }
}
"#,
            ),
        ],
    );

    let iface_uri = format!("file://{}", _dir.path().join("src/Loggable.php").display());
    let iface_content = std::fs::read_to_string(_dir.path().join("src/Loggable.php")).unwrap();
    backend.update_ast(&iface_uri, &iface_content);

    let impl_uri = format!(
        "file://{}",
        _dir.path().join("src/FileLogger.php").display()
    );
    let impl_content = std::fs::read_to_string(_dir.path().join("src/FileLogger.php")).unwrap();

    // Hover on `log` called on `$this` inside FileLogger (line 5).
    let hover = hover_at(&backend, &impl_uri, &impl_content, 5, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("implements **Loggable**"),
        "cross-file implements should show indicator, got: {}",
        text
    );
}

#[test]
fn hover_implements_indicator_same_file_with_namespace() {
    // Mimics example.php's HoverOriginsDemo scenario: interface and class
    // in the same namespace block of the same file.
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;

interface Renderable {
    public function format(string $template): string;
}

abstract class Model {
    abstract public function toArray(): array;
}

class HoverOriginsDemo extends Model implements Renderable {
    public function format(string $template): string { return ''; }
    public function toArray(): array { return []; }
    public function demo(): void {
        $this->format('x');
        $this->toArray();
    }
}
"#;

    // Hover on `format` called on `$this` (line 15).
    let hover = hover_at(&backend, uri, content, 15, 16).expect("expected hover on format");
    let text = hover_text(&hover);
    assert!(
        text.contains("implements **Renderable**"),
        "should show implements indicator for format(), got: {}",
        text
    );

    // Hover on `toArray` called on `$this` (line 16).
    let hover = hover_at(&backend, uri, content, 16, 16).expect("expected hover on toArray");
    let text = hover_text(&hover);
    assert!(
        text.contains("overrides **Model**"),
        "should show overrides indicator for toArray(), got: {}",
        text
    );
}

#[test]
fn hover_implements_indicator_multi_namespace_block_file() {
    // Mirrors example.php's structure: a single file with one big
    // `namespace Demo { ... }` block containing both the interface
    // and the implementing class.
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo {

interface Renderable {
    public function format(string $template): string;
}

abstract class Model {
    abstract public function toArray(): array;
    public function getName(): string { return ''; }
}

class HoverOriginsDemo extends Model implements Renderable {
    public function format(string $template): string { return ''; }
    public function toArray(): array { return []; }
    public function demo(): void {
        $this->format('x');
        $this->toArray();
        $this->getName();
    }
}

}
"#;

    // Hover on `format` called on `$this` (line 16).
    // `format` is declared on Renderable, so should show implements indicator.
    let hover = hover_at(&backend, uri, content, 16, 16).expect("expected hover on format");
    let text = hover_text(&hover);
    assert!(
        text.contains("implements **Renderable**"),
        "should show implements indicator for format(), got: {}",
        text
    );

    // Hover on `toArray` called on `$this` (line 17).
    // `toArray` is declared on Model, so should show overrides indicator.
    let hover = hover_at(&backend, uri, content, 17, 16).expect("expected hover on toArray");
    let text = hover_text(&hover);
    assert!(
        text.contains("overrides **Model**"),
        "should show overrides indicator for toArray(), got: {}",
        text
    );

    // Hover on `getName` called on `$this` (line 18).
    // `getName` is inherited from Model (not overridden), so NO indicator.
    let hover = hover_at(&backend, uri, content, 18, 16).expect("expected hover on getName");
    let text = hover_text(&hover);
    assert!(
        !text.contains("overrides"),
        "inherited method should NOT show overrides, got: {}",
        text
    );
    assert!(
        !text.contains("implements"),
        "inherited method should NOT show implements, got: {}",
        text
    );
}

#[test]
fn hover_implements_indicator_on_example_php() {
    // Load the actual example.php and verify the implements indicator fires
    // on HoverOriginsDemo.format(), which implements Renderable::format().
    let backend = create_test_backend();
    let uri = "file:///example.php";
    let content = std::fs::read_to_string("example.php").expect("example.php must exist");

    // Parse the file so all classes are available.
    backend.update_ast(uri, &content);

    // Find the line containing `$this->format()` inside HoverOriginsDemo::demo().
    let (format_line, format_col) = content
        .lines()
        .enumerate()
        .find_map(|(i, line)| {
            if line.contains("$this->format()") && i > 1710 && i < 1730 {
                let col = line.find("format").unwrap();
                Some((i as u32, col as u32))
            } else {
                None
            }
        })
        .expect("should find $this->format() in HoverOriginsDemo");

    let hover = hover_at(&backend, uri, &content, format_line, format_col)
        .expect("expected hover on format");
    let text = hover_text(&hover);
    assert!(
        text.contains("implements **Renderable**"),
        "HoverOriginsDemo.format() should show implements Renderable, got: {}",
        text
    );

    // Also verify toArray shows overrides Model.
    let (toarray_line, toarray_col) = content
        .lines()
        .enumerate()
        .find_map(|(i, line)| {
            if line.contains("$this->toArray()") && i > 1710 && i < 1730 {
                let col = line.find("toArray").unwrap();
                Some((i as u32, col as u32))
            } else {
                None
            }
        })
        .expect("should find $this->toArray() in HoverOriginsDemo");

    let hover = hover_at(&backend, uri, &content, toarray_line, toarray_col)
        .expect("expected hover on toArray");
    let text = hover_text(&hover);
    assert!(
        text.contains("overrides **Model**"),
        "HoverOriginsDemo.toArray() should show overrides Model, got: {}",
        text
    );
}

// ─── Enum case listing tests ────────────────────────────────────────────────

#[test]
fn hover_enum_shows_cases_in_code_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;
}
function paint(Color $c): void {}
"#;

    // Hover on `Color` in the function parameter (line 6).
    let hover = hover_at(&backend, uri, content, 6, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("case Red;"),
        "should list Red case, got: {}",
        text
    );
    assert!(
        text.contains("case Green;"),
        "should list Green case, got: {}",
        text
    );
    assert!(
        text.contains("case Blue;"),
        "should list Blue case, got: {}",
        text
    );
}

#[test]
fn hover_backed_enum_shows_cases_with_values() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
}
function check(Status $s): void {}
"#;

    // Hover on `Status` in the function parameter (line 5).
    let hover = hover_at(&backend, uri, content, 5, 16).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("case Active = 'active';"),
        "should list Active case with value, got: {}",
        text
    );
    assert!(
        text.contains("case Inactive = 'inactive';"),
        "should list Inactive case with value, got: {}",
        text
    );
}

#[test]
fn hover_enum_does_not_show_regular_constants() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
enum Suit: string {
    const TABLE = 'suits';
    case Hearts = 'H';
    case Diamonds = 'D';
}
function deal(Suit $s): void {}
"#;

    // Hover on `Suit` in the function parameter (line 6).
    let hover = hover_at(&backend, uri, content, 6, 14).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("case Hearts"),
        "should list Hearts case, got: {}",
        text
    );
    assert!(
        !text.contains("const TABLE"),
        "should NOT list regular constant TABLE in enum body, got: {}",
        text
    );
}

#[test]
fn hover_enum_with_no_cases_shows_plain_signature() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
enum Permission {
    const ADMIN_ROLE = 'admin';
    public function label(): string { return ''; }
}
function f(Permission $p): void {}
"#;

    // Hover on `Permission` in the function parameter (line 5).
    let hover = hover_at(&backend, uri, content, 5, 13).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("enum Permission"),
        "should show enum signature, got: {}",
        text
    );
    // No enum cases exist, so no curly-brace body should be rendered.
    assert!(
        !text.contains('{'),
        "should not have curly brace body when no cases, got: {}",
        text
    );
}

// ─── Trait method signature listing tests ───────────────────────────────────

#[test]
fn hover_trait_shows_public_method_signatures() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
trait Cacheable {
    public function getCacheKey(): string { return ''; }
    public static function flushCache(): void {}
    protected function internalCache(): void {}
}
class Item {
    use Cacheable;
}
"#;

    // Hover on `Cacheable` in the use statement (line 7).
    let hover = hover_at(&backend, uri, content, 7, 9).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("function getCacheKey(): string;"),
        "should show getCacheKey signature, got: {}",
        text
    );
    assert!(
        text.contains("static function flushCache(): void;"),
        "should show static flushCache signature, got: {}",
        text
    );
    assert!(
        !text.contains("internalCache"),
        "should NOT show protected method, got: {}",
        text
    );
}

#[test]
fn hover_trait_shows_public_properties() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
trait HasName {
    public string $name;
    protected int $id;
    public function getName(): string { return $this->name; }
}
class Person {
    use HasName;
}
"#;

    // Hover on `HasName` in the use statement (line 7).
    let hover = hover_at(&backend, uri, content, 7, 9).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("public string $name;"),
        "should show public property, got: {}",
        text
    );
    assert!(
        !text.contains("$id"),
        "should NOT show protected property, got: {}",
        text
    );
    assert!(
        text.contains("function getName(): string;"),
        "should show public method, got: {}",
        text
    );
}

#[test]
fn hover_trait_with_no_public_members_shows_plain_signature() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
trait Internal {
    protected function secret(): void {}
}
class Box {
    use Internal;
}
"#;

    // Hover on `Internal` in the use statement (line 5).
    let hover = hover_at(&backend, uri, content, 5, 9).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("trait Internal"),
        "should show trait signature, got: {}",
        text
    );
    assert!(
        !text.contains('{'),
        "should not have curly brace body when no public members, got: {}",
        text
    );
}

#[test]
fn hover_class_does_not_show_member_body() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Widget {
    public string $label;
    public function render(): string { return ''; }
}
function f(Widget $w): void {}
"#;

    // Hover on `Widget` in the function parameter (line 5).
    let hover = hover_at(&backend, uri, content, 5, 12).expect("expected hover");
    let text = hover_text(&hover);
    assert!(
        text.contains("class Widget"),
        "should show class name, got: {}",
        text
    );
    // Regular classes should NOT show a member body.
    assert!(
        !text.contains("$label"),
        "should NOT list properties for a regular class, got: {}",
        text
    );
    assert!(
        !text.contains("function render"),
        "should NOT list methods for a regular class, got: {}",
        text
    );
}
