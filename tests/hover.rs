//! Integration tests for `textDocument/hover`.

mod common;

use common::{
    create_psr4_workspace, create_test_backend, create_test_backend_with_closure_stub,
    create_test_backend_with_function_stubs, create_test_backend_with_stdclass_stub,
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
fn hover_ambiguous_variable_shows_union_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Lamp {
    public function dim(): void {}
    public function turnOff(): void {}
}

class Faucet {
    public function drip(): void {}
    public function turnOff(): void {}
}

class Consumer {
    public function run(): void {
        if (rand(0, 1)) {
            $ambiguous = new Lamp();
        } else {
            $ambiguous = new Faucet();
        }
        $ambiguous->turnOff();
    }
}
"#;

    // Hover on `$ambiguous` at line 18 (the usage after the if/else)
    let hover = hover_at(&backend, uri, content, 18, 9).expect("expected hover on $ambiguous");
    let text = hover_text(&hover);

    // Both union branches should appear.
    assert!(
        text.contains("Lamp") && text.contains("Faucet"),
        "hover should show both union types Lamp and Faucet, got: {}",
        text
    );

    // The two types should be rendered as separate code blocks
    // separated by a horizontal rule (`---`).
    assert!(
        text.contains("---"),
        "union hover should use a horizontal rule separator, got: {}",
        text
    );
}

#[test]
fn hover_ambiguous_variable_inside_if_branch_shows_single_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Lamp {
    public function dim(): void {}
    public function turnOff(): void {}
}

class Faucet {
    public function drip(): void {}
    public function turnOff(): void {}
}

class Consumer {
    public function run(): void {
        if (rand(0, 1)) {
            $ambiguous = new Lamp();
            $ambiguous->dim();
        } else {
            $ambiguous = new Faucet();
            $ambiguous->drip();
        }
        $ambiguous->turnOff();
    }
}
"#;

    // Hover on `$ambiguous` inside the if branch (line 15, the usage `$ambiguous->dim()`)
    let hover = hover_at(&backend, uri, content, 15, 13).expect("expected hover inside if branch");
    let text = hover_text(&hover);
    assert!(
        text.contains("Lamp"),
        "inside the if branch, should show Lamp: {}",
        text
    );
    assert!(
        !text.contains("Faucet"),
        "inside the if branch, should NOT show Faucet: {}",
        text
    );

    // Hover on `$ambiguous` inside the else branch (line 18, the usage `$ambiguous->drip()`)
    let hover =
        hover_at(&backend, uri, content, 18, 13).expect("expected hover inside else branch");
    let text = hover_text(&hover);
    assert!(
        text.contains("Faucet"),
        "inside the else branch, should show Faucet: {}",
        text
    );
    assert!(
        !text.contains("Lamp"),
        "inside the else branch, should NOT show Lamp: {}",
        text
    );

    // Hover on `$ambiguous` after the if/else (line 20, `$ambiguous->turnOff()`)
    let hover = hover_at(&backend, uri, content, 20, 9).expect("expected hover after if/else");
    let text = hover_text(&hover);
    assert!(
        text.contains("Lamp") && text.contains("Faucet"),
        "after the if/else, should show both Lamp and Faucet: {}",
        text
    );
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
fn hover_active_on_foreach_variable_definition_site() {
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
    let hover = hover_at(&backend, uri, content, 5, 29)
        .expect("hover should be active on foreach variable $item");
    let text = hover_text(&hover);
    assert!(text.contains("Item"), "should resolve to Item: {}", text);
}

#[test]
fn hover_active_on_catch_variable_definition_site() {
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
    let hover = hover_at(&backend, uri, content, 4, 26)
        .expect("hover should be active on catch variable $e");
    let text = hover_text(&hover);
    assert!(
        text.contains("Exception"),
        "should resolve to Exception: {}",
        text
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
    let backend = create_test_backend_with_closure_stub();
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

#[test]
fn hover_define_constant_shows_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
define('APP_VERSION', '1.0.0');
echo APP_VERSION;
"#;

    backend.update_ast(uri, content);
    let hover = hover_at(&backend, uri, content, 2, 7).expect("expected hover on APP_VERSION");
    let text = hover_text(&hover);
    assert!(
        text.contains("'1.0.0'"),
        "hover should show the constant value '1.0.0', got: {}",
        text
    );
    assert!(
        text.contains("APP_VERSION"),
        "hover should show the constant name, got: {}",
        text
    );
}

#[test]
fn hover_define_constant_integer_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
define('MAX_RETRIES', 5);
echo MAX_RETRIES;
"#;

    backend.update_ast(uri, content);
    let hover = hover_at(&backend, uri, content, 2, 7).expect("expected hover on MAX_RETRIES");
    let text = hover_text(&hover);
    assert!(
        text.contains("= 5"),
        "hover should show 'const MAX_RETRIES = 5;', got: {}",
        text
    );
}

#[test]
fn hover_top_level_const_shows_value() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
const DB_HOST = 'localhost';
echo DB_HOST;
"#;

    backend.update_ast(uri, content);
    let hover = hover_at(&backend, uri, content, 2, 7).expect("expected hover on DB_HOST");
    let text = hover_text(&hover);
    assert!(
        text.contains("'localhost'"),
        "hover should show the constant value, got: {}",
        text
    );
    assert!(
        text.contains("DB_HOST"),
        "hover should show the constant name, got: {}",
        text
    );
}

#[test]
fn hover_define_constant_no_value_still_works() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    // Register a constant without a value (e.g. from autoload discovery).
    if let Ok(mut dmap) = backend.global_defines().lock() {
        dmap.insert(
            "LEGACY_CONST".to_string(),
            phpantom_lsp::DefineInfo {
                file_uri: "file:///legacy.php".to_string(),
                name_offset: 0,
                value: None,
            },
        );
    }

    let content = r#"<?php
echo LEGACY_CONST;
"#;

    backend.update_ast(uri, content);
    let hover = hover_at(&backend, uri, content, 1, 7).expect("expected hover on LEGACY_CONST");
    let text = hover_text(&hover);
    assert!(
        text.contains("LEGACY_CONST"),
        "hover should show the constant name, got: {}",
        text
    );
    // No value available, so it should show just `const LEGACY_CONST;`
    assert!(
        !text.contains('='),
        "hover should not show '=' when value is unknown, got: {}",
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

/// Variable hover namespace is derived from the type string, not the file.
/// A parameter typed as `\Generator<int, Pencil>` in a `Demo` namespace
/// file should show no namespace line (Generator is global).
#[test]
fn hover_variable_namespace_from_type_not_file() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;
class Pencil { public function sketch(): void {} }
class Demo {
    /**
     * @param \Generator<int, Pencil> $pencils
     */
    public function foreachGeneratorParam(\Generator $pencils): void
    {
        foreach ($pencils as $pencil) {
            $pencil->sketch();
        }
    }
}
"#;

    // Hover on `$pencils` inside the foreach header (line 9)
    let hover =
        hover_at(&backend, uri, content, 9, 18).expect("hover should be active on $pencils");
    let text = hover_text(&hover);
    assert!(
        !text.contains("namespace Demo"),
        "should not show file namespace for global Generator type, got: {}",
        text
    );
    assert!(
        text.contains("Generator<int, Pencil>"),
        "should show full generic type, got: {}",
        text
    );
}

/// Catch variable hover should not show the enclosing file's namespace
/// when the exception type is global (e.g. `\RuntimeException`).
#[test]
fn hover_catch_variable_namespace_from_type_not_file() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Demo;
class Demo {
    public function risky(): void
    {
        try {
            throw new \RuntimeException('oops');
        } catch (\RuntimeException $e) {
            echo $e->getMessage();
        }
    }
}
"#;

    // Hover on `$e` at the catch binding (line 7)
    let hover =
        hover_at(&backend, uri, content, 7, 36).expect("hover should be active on catch $e");
    let text = hover_text(&hover);
    assert!(
        !text.contains("namespace Demo"),
        "should not show file namespace for global RuntimeException, got: {}",
        text
    );
    assert!(
        text.contains("RuntimeException"),
        "should show RuntimeException, got: {}",
        text
    );
}

/// When the type is in a real namespace (e.g. `\App\Models\User`),
/// the hover should show `namespace App\Models;` and the short name.
#[test]
fn hover_variable_namespaced_type_shows_type_namespace() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Services;
class MyService {
    /**
     * @param \App\Models\User $user
     */
    public function process($user): void
    {
        echo $user;
    }
}
"#;

    // Hover on `$user` in the method body (line 8)
    let hover = hover_at(&backend, uri, content, 8, 14).expect("hover should be active on $user");
    let text = hover_text(&hover);
    assert!(
        text.contains("namespace App\\Models"),
        "should show the type's namespace, got: {}",
        text
    );
    assert!(
        !text.contains("namespace App\\Services"),
        "should not show the file's namespace, got: {}",
        text
    );
}

/// When a variable's type is a `@template` parameter, hover should show
/// the template variance and bound above the code block.
#[test]
fn hover_variable_with_template_type_shows_template_info() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template-covariant TNode of AstNode
 */
class NodeList {
    /**
     * @param TNode $node
     */
    public function add($node): void {
        echo $node;
    }
}
class AstNode {}
"#;

    // Hover on `$node` in `echo $node;` (line 9)
    let hover = hover_at(&backend, uri, content, 9, 14).expect("hover should be active on $node");
    let text = hover_text(&hover);
    assert!(
        text.contains("template-covariant"),
        "should show template variance, got: {}",
        text
    );
    assert!(
        text.contains("TNode"),
        "should show template name, got: {}",
        text
    );
    assert!(
        text.contains("AstNode"),
        "should show template bound, got: {}",
        text
    );
    assert!(
        text.contains("$node = TNode"),
        "should still show type assignment, got: {}",
        text
    );
}

/// An invariant `@template` without a bound shows just the variance and name.
#[test]
fn hover_variable_with_unbounded_template_shows_variance() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template T
 */
class Box {
    /**
     * @param T $value
     */
    public function set($value): void {
        echo $value;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 9, 14).expect("hover should be active on $value");
    let text = hover_text(&hover);
    assert!(
        text.contains("template"),
        "should show template variance, got: {}",
        text
    );
    assert!(
        text.contains("`T`"),
        "should show template name, got: {}",
        text
    );
    assert!(
        text.contains("$value = T"),
        "should still show type assignment, got: {}",
        text
    );
}

/// When a property's `@var` type is a `@template` parameter on the owning
/// class, hover should show the template variance and bound.
#[test]
fn hover_property_with_template_type_shows_template_info() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template-covariant TNode of AstNode
 */
class NodeList {
    /** @var TNode */
    public $node;

    public function demo(): void {
        $this->node;
    }
}
class AstNode {
    public function getChildren(): array { return []; }
}
"#;

    // Hover on `node` in `$this->node` (line 9)
    let hover = hover_at(&backend, uri, content, 9, 16).expect("expected hover on node property");
    let text = hover_text(&hover);
    assert!(
        text.contains("template-covariant"),
        "should show template variance, got: {}",
        text
    );
    assert!(
        text.contains("TNode"),
        "should show template name, got: {}",
        text
    );
    assert!(
        text.contains("AstNode"),
        "should show template bound, got: {}",
        text
    );
}

/// A property typed with a `@template` parameter that has no bound
/// should still show the variance and name.
#[test]
fn hover_property_with_unbounded_template_shows_variance() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template T
 */
class Box {
    /** @var T */
    public $value;

    public function demo(): void {
        $this->value;
    }
}
"#;

    let hover = hover_at(&backend, uri, content, 9, 16).expect("expected hover on value property");
    let text = hover_text(&hover);
    assert!(
        text.contains("template"),
        "should show template variance, got: {}",
        text
    );
    assert!(
        text.contains("`T`"),
        "should show template name, got: {}",
        text
    );
}

/// When a method's `@return` type is a `@template` parameter on the owning
/// class, hover should show the template variance and bound.
#[test]
fn hover_method_with_template_return_type_shows_template_info() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template-covariant TNode of AstNode
 */
class NodeList {
    /**
     * @return TNode
     */
    public function first() {}

    public function demo(): void {
        $this->first();
    }
}
class AstNode {}
"#;

    // Hover on `first` in `$this->first()` (line 11)
    let hover = hover_at(&backend, uri, content, 11, 16).expect("expected hover on first()");
    let text = hover_text(&hover);
    assert!(
        text.contains("template-covariant"),
        "should show template variance, got: {}",
        text
    );
    assert!(
        text.contains("TNode"),
        "should show template name, got: {}",
        text
    );
    assert!(
        text.contains("AstNode"),
        "should show template bound, got: {}",
        text
    );
}

/// When a method's `@param` type is a `@template` parameter, hover should
/// show the template info.  Duplicate templates (same name in both param
/// and return) should appear only once.
#[test]
fn hover_method_with_template_param_type_shows_template_info() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template T
 * @template-covariant TNode of AstNode
 */
class NodeList {
    /**
     * @param TNode $node
     * @param T $extra
     * @return TNode
     */
    public function add($node, $extra) {}

    public function demo(): void {
        $this->add(null, null);
    }
}
class AstNode {}
"#;

    // Hover on `add` in `$this->add(...)` (line 14)
    let hover = hover_at(&backend, uri, content, 14, 15).expect("expected hover on add()");
    let text = hover_text(&hover);
    assert!(
        text.contains("template-covariant"),
        "should show covariant variance for TNode, got: {}",
        text
    );
    assert!(
        text.contains("`T`"),
        "should show template T info, got: {}",
        text
    );
    // TNode appears in both @return and @param — should only show once
    let count = text.matches("template-covariant").count();
    assert_eq!(
        count, 1,
        "TNode template info should appear exactly once, got {} in: {}",
        count, text
    );
}

// ─── @var scope isolation ───────────────────────────────────────────────────

/// An inline `/** @var Type $var */` annotation in one method must not
/// leak into a different method that uses the same variable name.
#[test]
fn hover_var_annotation_does_not_leak_across_method_scopes() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class ObjectShapeDemo {
    public function demo(): void {
        /** @var object{title: string, score: float} $item */
        $item = getUnknownValue();
        $item->title;
    }
}

class ObjectMapper {
    /**
     * @template T
     * @param T $item
     * @return T
     */
    public function identity(mixed $item): mixed {
        return $item;
    }
}
"#;

    // Hover on `$item` in ObjectMapper::identity (line 16, `return $item;`)
    let hover = hover_at(&backend, uri, content, 16, 16).expect("expected hover on $item");
    let text = hover_text(&hover);
    assert!(
        !text.contains("object{"),
        "should NOT leak @var from ObjectShapeDemo into ObjectMapper, got: {}",
        text
    );
    assert!(
        text.contains("$item = T"),
        "should resolve $item to template param T, got: {}",
        text
    );
}

/// Same-method `/** @var */` still works after the scope fix.
#[test]
fn hover_var_annotation_within_same_method_still_works() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Demo {
    public function run(): void {
        /** @var object{name: string} $thing */
        $thing = getUnknown();
        echo $thing;
    }
}
"#;

    // Hover on `$thing` in `echo $thing;` (line 5)
    let hover = hover_at(&backend, uri, content, 5, 14).expect("expected hover on $thing");
    let text = hover_text(&hover);
    assert!(
        text.contains("object{name: string}"),
        "should still resolve @var in the same method, got: {}",
        text
    );
}

// ─── Method-level template info in hover ────────────────────────────────────

/// When a method declares its own `@template T` and uses it in `@return`,
/// hover should show `**template** \`T\`` (method-level, always invariant).
#[test]
fn hover_method_level_template_in_return_shows_info() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class ObjectMapper {
    /**
     * @template T
     * @param T $item
     * @return T
     */
    public function identity(mixed $item): mixed {
        return $item;
    }

    public function demo(): void {
        $this->identity(null);
    }
}
"#;

    // Hover on `identity` in `$this->identity(null)` (line 12)
    let hover = hover_at(&backend, uri, content, 12, 16).expect("expected hover on identity()");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template** `T`"),
        "should show method-level template info for T, got: {}",
        text
    );
}

/// Method-level `@template T of Model` should show the bound.
#[test]
fn hover_method_level_template_with_bound_shows_info() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Model {}
class Repo {
    /**
     * @template T of Model
     * @param class-string<T> $class
     * @return T
     */
    public function find(string $class): mixed {
        return new $class();
    }

    public function demo(): void {
        $this->find(Model::class);
    }
}
"#;

    // Hover on `find` in `$this->find(...)` (line 13)
    let hover = hover_at(&backend, uri, content, 13, 16).expect("expected hover on find()");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template** `T` of `Model`"),
        "should show method-level template with bound, got: {}",
        text
    );
}

/// Method-level template takes priority over a same-named class-level template.
#[test]
fn hover_method_level_template_takes_priority_over_class_level() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Animal {}
class Plant {}
/**
 * @template T of Animal
 */
class Container {
    /**
     * @template T of Plant
     * @param T $item
     * @return T
     */
    public function wrap($item) {}

    public function demo(): void {
        $this->wrap(null);
    }
}
"#;

    // Hover on `wrap` in `$this->wrap(null)` (line 15)
    let hover = hover_at(&backend, uri, content, 15, 16).expect("expected hover on wrap()");
    let text = hover_text(&hover);
    assert!(
        text.contains("of `Plant`"),
        "should show method-level bound (Plant), not class-level (Animal), got: {}",
        text
    );
    assert!(
        !text.contains("of `Animal`"),
        "should NOT show class-level bound, got: {}",
        text
    );
}

/// When the method has no templates but the class does, class-level
/// template info should still appear (existing behavior preserved).
#[test]
fn hover_class_level_template_still_shown_when_method_has_none() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
/**
 * @template-covariant TValue of object
 */
class TypedBox {
    /**
     * @return TValue
     */
    public function get() {}

    public function demo(): void {
        $this->get();
    }
}
"#;

    // Hover on `get` in `$this->get()` (line 11)
    let hover = hover_at(&backend, uri, content, 11, 16).expect("expected hover on get()");
    let text = hover_text(&hover);
    assert!(
        text.contains("**template-covariant** `TValue` of `object`"),
        "should show class-level template info when method has no own templates, got: {}",
        text
    );
}

// ─── Scope method hover on Builder instances ────────────────────────────────

// Minimal Laravel framework stubs for hover tests.
// These mirror the stubs in completion_laravel.rs but are kept here to avoid
// cross-test-file dependencies.

const HOVER_LARAVEL_COMPOSER: &str = r#"{
    "autoload": {
        "psr-4": {
            "App\\": "src/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;

const HOVER_MODEL_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
class Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */
    public static function with(mixed $relations): Builder { return new Builder(); }
}
";

const HOVER_BUILDER_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
use Illuminate\\Database\\Concerns\\BuildsQueries;
/**
 * @template TModel of \\Illuminate\\Database\\Eloquent\\Model
 * @mixin \\Illuminate\\Database\\Query\\Builder
 */
class Builder {
    /** @use BuildsQueries<TModel> */
    use BuildsQueries;
    /** @return static */
    public function where(string $column, mixed $operator = null, mixed $value = null): static { return $this; }
    /** @return static */
    public function orderBy(string $column, string $direction = 'asc'): static { return $this; }
    /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */
    public function get(): Collection { return new Collection(); }
    /** @return static */
    public function limit(int $value): static { return $this; }
}
";

const HOVER_QUERY_BUILDER_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Query;
class Builder {
    /** @return static */
    public function whereIn(string $column, array $values): static { return $this; }
    /** @return static */
    public function groupBy(string ...$groups): static { return $this; }
}
";

const HOVER_BUILDS_QUERIES_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Concerns;
/**
 * @template TValue
 */
trait BuildsQueries {
    /** @return TValue|null */
    public function first(): mixed { return null; }
}
";

const HOVER_COLLECTION_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
/**
 * @template TKey of array-key
 * @template TModel
 */
class Collection {
    /** @return TModel|null */
    public function first(): mixed { return null; }
}
";

/// Build a PSR-4 workspace with the minimal Laravel framework stubs
/// plus extra application files.
fn make_laravel_hover_workspace(app_files: &[(&str, &str)]) -> (Backend, tempfile::TempDir) {
    let mut files: Vec<(&str, &str)> = vec![
        ("vendor/illuminate/Eloquent/Model.php", HOVER_MODEL_PHP),
        ("vendor/illuminate/Eloquent/Builder.php", HOVER_BUILDER_PHP),
        (
            "vendor/illuminate/Eloquent/Collection.php",
            HOVER_COLLECTION_PHP,
        ),
        (
            "vendor/illuminate/Query/Builder.php",
            HOVER_QUERY_BUILDER_PHP,
        ),
        (
            "vendor/illuminate/Concerns/BuildsQueries.php",
            HOVER_BUILDS_QUERIES_PHP,
        ),
    ];
    files.extend_from_slice(app_files);
    create_psr4_workspace(HOVER_LARAVEL_COMPOSER, &files)
}

/// Hovering on a scope method (or any Builder method) called on a variable
/// that holds a Builder instance should show the method hover.
///
/// Reproduces the user's exact case:
///   $query = BlogAuthor::where('genre', 'fiction');
///   $query->active();          // ← hover on `active` should work
///   $query->orderBy('name');   // ← hover on `orderBy` should work
#[test]
fn hover_scope_method_on_builder_variable() {
    let brand_php = "\
<?php
namespace App;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeActive(Builder $query): void {}
    public function test() {
        $query = Brand::where('genre', 'fiction');
        $query->active();
        $query->orderBy('name')->get();
    }
}
";
    let (backend, _dir) = make_laravel_hover_workspace(&[("src/Brand.php", brand_php)]);

    let uri = format!("file://{}", _dir.path().join("src/Brand.php").display());
    backend.update_ast(&uri, brand_php);

    // Line 8:  "        $query->active();"
    //           0         1
    //           0123456789012345678
    // `active` starts at col 16

    // Hover on `active` — a scope method on the Builder variable
    let hover = hover_at(&backend, &uri, brand_php, 8, 17);
    assert!(
        hover.is_some(),
        "hover should be shown on scope method active() called on $query (Builder variable)"
    );
    let text = hover_text(hover.as_ref().unwrap());
    assert!(
        text.contains("active"),
        "hover should mention active, got: {}",
        text
    );

    // Line 9:  "        $query->orderBy('name')->get();"
    //           0         1         2         3
    //           01234567890123456789012345678901234567890
    // `orderBy` starts at col 16, `get` starts at col 35

    // Hover on `orderBy` — a Builder-forwarded method
    let hover_ob = hover_at(&backend, &uri, brand_php, 9, 18);
    assert!(
        hover_ob.is_some(),
        "hover should be shown on orderBy() called on $query (Builder variable)"
    );

    // Hover on `get` — chained after orderBy()
    let hover_get = hover_at(&backend, &uri, brand_php, 9, 36);
    assert!(
        hover_get.is_some(),
        "hover should be shown on get() chained after $query->orderBy()"
    );
}

/// Hovering on scope methods after an inline Builder chain
/// (e.g. `Brand::where('id', 1)->active()->get()`) should work.
#[test]
fn hover_scope_method_after_inline_builder_chain() {
    let brand_php = "\
<?php
namespace App;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeActive(Builder $query): void {}
    public function scopeOfGenre(Builder $query, string $genre): void {}
    public function test() {
        Brand::where('active', 1)->active()->ofGenre('sci-fi')->get();
    }
}
";
    let (backend, _dir) = make_laravel_hover_workspace(&[("src/Brand.php", brand_php)]);

    let uri = format!("file://{}", _dir.path().join("src/Brand.php").display());
    backend.update_ast(&uri, brand_php);

    // Line 8: "        Brand::where('active', 1)->active()->ofGenre('sci-fi')->get();"
    //          0         1         2         3         4         5         6
    //          0123456789012345678901234567890123456789012345678901234567890123456789

    // Hover on `where` (col ~15)
    let h_where = hover_at(&backend, &uri, brand_php, 8, 16);
    assert!(
        h_where.is_some(),
        "hover should work on where() in Brand::where()"
    );

    // Hover on `active` (col ~35)
    let h_active = hover_at(&backend, &uri, brand_php, 8, 36);
    assert!(
        h_active.is_some(),
        "hover should work on scope method active() after Brand::where() chain"
    );

    // Hover on `ofGenre` (col ~46)
    let h_genre = hover_at(&backend, &uri, brand_php, 8, 47);
    assert!(
        h_genre.is_some(),
        "hover should work on scope method ofGenre() after chained scope"
    );
}

/// Hover on scope/Builder methods in a single-file with multiple namespace
/// blocks — simulates example.php where Eloquent stubs and user code live
/// in the same file under separate `namespace { }` blocks.
#[test]
fn hover_scope_method_multi_namespace_single_file() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Illuminate\Database\Eloquent {
   abstract class Model {
       /** @return \Illuminate\Database\Eloquent\Builder<static> */
       public static function query() {}
   }

   /**
    * @template TModel of \Illuminate\Database\Eloquent\Model
    *
    * @mixin \Illuminate\Database\Query\Builder
    */
   class Builder {
       /**
        * @param  (\Closure(static): mixed)|string|array  $column
        * @return $this
        */
       public function where($column, $operator = null, $value = null, $boolean = 'and') {}

       /** @return \Illuminate\Database\Eloquent\Collection<int, TModel> */
       public function get($columns = ['*']) { return new Collection(); }
   }

   /**
    * @template TKey of array-key
    * @template TModel of \Illuminate\Database\Eloquent\Model
    */
   class Collection {}
}

namespace Illuminate\Database\Query {
   class Builder {
       /** @return $this */
       public function orderBy($column, $direction = 'asc') { return $this; }
       /** @return $this */
       public function limit($value) { return $this; }
   }
}

namespace Demo {
   use Illuminate\Database\Eloquent\Model;
   use Illuminate\Database\Eloquent\Builder;

   class BlogAuthor extends Model {
       public function scopeActive(Builder $query): void {}
       public function scopeOfGenre(Builder $query, string $genre): void {}
   }

   class EloquentDemo {
       public function run(): void {
           $author = new BlogAuthor();
           $author->active();
           BlogAuthor::active();

           BlogAuthor::where('active', 1)->active()->ofGenre('sci-fi')->get();

           $query = BlogAuthor::where('genre', 'fiction');
           $query->active();
           $query->orderBy('name')->get();
       }
   }
}
"#;

    // Find the actual line numbers dynamically
    let author_active_line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("$author->active()"))
        .map(|(i, _)| i as u32)
        .expect("should find $author->active() line");

    let static_active_line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("BlogAuthor::active()"))
        .map(|(i, _)| i as u32)
        .expect("should find BlogAuthor::active() line");

    let inline_chain_line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("BlogAuthor::where('active', 1)->active"))
        .map(|(i, _)| i as u32)
        .expect("should find inline chain line");

    let query_active_line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.trim() == "$query->active();")
        .map(|(i, _)| i as u32)
        .expect("should find $query->active() line");

    let query_orderby_line = content
        .lines()
        .enumerate()
        .find(|(_, l)| l.contains("$query->orderBy"))
        .map(|(i, _)| i as u32)
        .expect("should find $query->orderBy line");

    // $author->active()
    let h_instance_scope = hover_at(&backend, uri, content, author_active_line, 23);
    assert!(
        h_instance_scope.is_some(),
        "hover should work on $author->active() (scope on model instance)"
    );

    // BlogAuthor::active()
    let h_static_scope = hover_at(&backend, uri, content, static_active_line, 25);
    assert!(
        h_static_scope.is_some(),
        "hover should work on BlogAuthor::active() (scope as static)"
    );

    // BlogAuthor::where('active', 1)->...
    let h_where = hover_at(&backend, uri, content, inline_chain_line, 26);
    assert!(
        h_where.is_some(),
        "hover should work on where() in BlogAuthor::where() (builder-forwarded)"
    );

    // $query->active()
    let h_var_scope = hover_at(&backend, uri, content, query_active_line, 21);
    assert!(
        h_var_scope.is_some(),
        "hover should work on $query->active() (scope on Builder variable)"
    );

    // $query->orderBy('name')->get()
    let h_order_by = hover_at(&backend, uri, content, query_orderby_line, 22);
    assert!(
        h_order_by.is_some(),
        "hover should work on $query->orderBy() (Builder method on variable)"
    );
}

/// Test hover against the real example.php to reproduce the exact user report.
/// The user observed that `$query->active()` and scope methods after `where()`
/// chains did not show hover in example.php.
#[test]
fn hover_scope_on_builder_in_example_php() {
    let backend = create_test_backend();
    let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example.php");
    let content = std::fs::read_to_string(&example_path).expect("failed to read example.php");
    let uri = format!("file://{}", example_path.display());

    backend.update_ast(&uri, &content);

    // Find the key lines dynamically so the test survives line-number shifts.
    let lines: Vec<&str> = content.lines().collect();

    // "$author->active();" inside EloquentQueryDemo::demo()
    let author_active_idx = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("$author->active()"))
        .map(|(i, _)| i)
        .expect("should find $author->active() in example.php");
    let author_active_line = lines[author_active_idx];
    let author_active_col = author_active_line
        .find("active()")
        .expect("should find active() token") as u32;

    let h = hover_at(
        &backend,
        &uri,
        &content,
        author_active_idx as u32,
        author_active_col + 1,
    );
    assert!(
        h.is_some(),
        "hover should work on $author->active() in example.php (line {})",
        author_active_idx
    );

    // "BlogAuthor::active();" inside EloquentQueryDemo::demo()
    let static_active_idx = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.trim() == "BlogAuthor::active();")
        .map(|(i, _)| i)
        .expect("should find BlogAuthor::active() in example.php");
    let static_active_line = lines[static_active_idx];
    let static_active_col = static_active_line
        .find("active()")
        .expect("should find active() token") as u32;

    let h2 = hover_at(
        &backend,
        &uri,
        &content,
        static_active_idx as u32,
        static_active_col + 1,
    );
    assert!(
        h2.is_some(),
        "hover should work on BlogAuthor::active() in example.php (line {})",
        static_active_idx
    );

    // "BlogAuthor::where('active', 1)->active()->ofGenre('sci-fi')->get();"
    let chain_idx = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("BlogAuthor::where('active', 1)->active()"))
        .map(|(i, _)| i)
        .expect("should find inline chain line in example.php");
    let chain_line = lines[chain_idx];

    // Hover on `where`
    let where_col = chain_line.find("where(").expect("should find where( token") as u32;
    let h_where = hover_at(&backend, &uri, &content, chain_idx as u32, where_col + 1);
    assert!(
        h_where.is_some(),
        "hover should work on where() in BlogAuthor::where() in example.php (line {})",
        chain_idx
    );

    // Hover on `active` after ->
    // Find the second occurrence of "active" (the first is in the argument).
    let after_arrow = chain_line
        .find(")->active()")
        .expect("should find )->active()")
        + 2; // skip )-
    let active_col = after_arrow as u32;
    let h_active = hover_at(&backend, &uri, &content, chain_idx as u32, active_col + 1);
    assert!(
        h_active.is_some(),
        "hover should work on active() after where() chain in example.php (line {})",
        chain_idx
    );

    // Hover on `ofGenre` after ->active()->
    let of_genre_col = chain_line
        .find("ofGenre(")
        .expect("should find ofGenre( token") as u32;
    let h_genre = hover_at(&backend, &uri, &content, chain_idx as u32, of_genre_col + 1);
    assert!(
        h_genre.is_some(),
        "hover should work on ofGenre() after chained scope in example.php (line {})",
        chain_idx
    );

    // "$query->active();"
    let query_active_idx = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.trim() == "$query->active();")
        .map(|(i, _)| i)
        .expect("should find $query->active() in example.php");
    let query_active_line = lines[query_active_idx];
    let query_active_col = query_active_line
        .find("active()")
        .expect("should find active() token") as u32;

    let h_qactive = hover_at(
        &backend,
        &uri,
        &content,
        query_active_idx as u32,
        query_active_col + 1,
    );
    assert!(
        h_qactive.is_some(),
        "hover should work on $query->active() in example.php (line {})",
        query_active_idx
    );

    // "$query->orderBy('name')->get();"
    let query_orderby_idx = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("$query->orderBy('name')->get()"))
        .map(|(i, _)| i)
        .expect("should find $query->orderBy line in example.php");
    let query_orderby_line = lines[query_orderby_idx];
    let orderby_col = query_orderby_line
        .find("orderBy(")
        .expect("should find orderBy( token") as u32;

    let h_orderby = hover_at(
        &backend,
        &uri,
        &content,
        query_orderby_idx as u32,
        orderby_col + 1,
    );
    assert!(
        h_orderby.is_some(),
        "hover should work on $query->orderBy() in example.php (line {})",
        query_orderby_idx
    );
}

/// Test that hovering on multiple Builder-related symbols in example.php
/// without re-parsing between requests still works.  In the real editor,
/// `update_ast` is called once on file open, and then multiple hover
/// requests reuse the same parsed state (and resolved-class cache).
/// This catches caching bugs where the first hover caches a Builder
/// without scope methods, poisoning subsequent lookups.
#[test]
fn hover_scope_on_builder_in_example_php_no_reparse() {
    let backend = create_test_backend();
    let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("example.php");
    let content = std::fs::read_to_string(&example_path).expect("failed to read example.php");
    let uri = format!("file://{}", example_path.display());

    // Parse ONCE — do NOT call update_ast again between hovers.
    backend.update_ast(&uri, &content);

    let lines: Vec<&str> = content.lines().collect();

    // Helper: find line index + column of a token on a line matching a pattern.
    let find = |pattern: &str, token: &str| -> (u32, u32) {
        let idx = lines
            .iter()
            .enumerate()
            .find(|(_, l)| l.contains(pattern))
            .map(|(i, _)| i)
            .unwrap_or_else(|| panic!("should find {:?} in example.php", pattern));
        let col = lines[idx]
            .find(token)
            .unwrap_or_else(|| panic!("should find {:?} on line {}", token, idx));
        (idx as u32, col as u32 + 1)
    };

    // 1. First hover on $author->active() — scope on model instance
    let (line, col) = find("$author->active()", "active()");
    let h1 = backend.handle_hover(
        &uri,
        &content,
        Position {
            line,
            character: col,
        },
    );
    assert!(
        h1.is_some(),
        "hover should work on $author->active() (line {})",
        line
    );

    // 2. Hover on BlogAuthor::active() — scope as static
    let (line, col) = find("BlogAuthor::active();", "active();");
    let h2 = backend.handle_hover(
        &uri,
        &content,
        Position {
            line,
            character: col,
        },
    );
    assert!(
        h2.is_some(),
        "hover should work on BlogAuthor::active() (line {})",
        line
    );

    // 3. Hover on where() in the inline chain — builder-forwarded method
    let (line, _) = find(
        "BlogAuthor::where('active', 1)->active()->ofGenre",
        "where(",
    );
    let where_col = lines[line as usize].find("where(").unwrap() as u32 + 1;
    let h3 = backend.handle_hover(
        &uri,
        &content,
        Position {
            line,
            character: where_col,
        },
    );
    assert!(
        h3.is_some(),
        "hover should work on BlogAuthor::where() (line {})",
        line
    );

    // 4. Hover on active() after where() — scope on Builder instance
    //    This is the user's exact complaint.
    let chain_line = line;
    let active_pos = lines[chain_line as usize]
        .find(")->active()")
        .expect("should find )->active()") as u32
        + 2; // skip )>
    let h4 = backend.handle_hover(
        &uri,
        &content,
        Position {
            line: chain_line,
            character: active_pos + 1,
        },
    );
    assert!(
        h4.is_some(),
        "hover should work on active() after where() chain (line {}, col {})",
        chain_line,
        active_pos + 1
    );

    // 5. Hover on ofGenre() after active() — second scope in chain
    let of_genre_col = lines[chain_line as usize]
        .find("ofGenre(")
        .expect("should find ofGenre(") as u32
        + 1;
    let h5 = backend.handle_hover(
        &uri,
        &content,
        Position {
            line: chain_line,
            character: of_genre_col,
        },
    );
    assert!(
        h5.is_some(),
        "hover should work on ofGenre() in chain (line {}, col {})",
        chain_line,
        of_genre_col
    );

    // 6. Hover on $query->active() — scope on Builder variable
    let (line, col) = find("$query->active();", "active();");
    let h6 = backend.handle_hover(
        &uri,
        &content,
        Position {
            line,
            character: col,
        },
    );
    assert!(
        h6.is_some(),
        "hover should work on $query->active() (line {})",
        line
    );

    // 7. Hover on $query->orderBy() — Builder method on variable
    let (line, col) = find("$query->orderBy('name')->get()", "orderBy(");
    let h7 = backend.handle_hover(
        &uri,
        &content,
        Position {
            line,
            character: col,
        },
    );
    assert!(
        h7.is_some(),
        "hover should work on $query->orderBy() (line {})",
        line
    );
}

/// Reproduces the Builder cache-poisoning scenario.
///
/// When `resolve_class_fully_cached` is called with a plain Builder
/// (without model-specific scope methods), the cached entry for
/// `Illuminate\Database\Eloquent\Builder` has no scopes.  If a
/// subsequent hover resolves a Builder<Model> with injected scopes
/// but then re-resolves via the cache, it gets the stale entry and
/// the scope method is not found.
///
/// This test forces a cache entry for plain Builder first (by hovering
/// on a Builder method via a different model that has no scopes), then
/// hovers on a scope method on a second model's Builder chain.
#[test]
fn hover_scope_survives_builder_cache_poisoning() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace Illuminate\Database\Eloquent {
    abstract class Model {
        /** @return \Illuminate\Database\Eloquent\Builder<static> */
        public static function query() {}
    }

    /**
     * @template TModel of \Illuminate\Database\Eloquent\Model
     * @mixin \Illuminate\Database\Query\Builder
     */
    class Builder {
        /** @return $this */
        public function where($column, $operator = null, $value = null) {}
        /** @return \Illuminate\Database\Eloquent\Collection<int, TModel> */
        public function get($columns = ['*']) { return new Collection(); }
        /** @return static */
        public function orderBy(string $column, string $direction = 'asc'): static { return $this; }
    }

    /** @template TKey @template TModel */
    class Collection {}
}

namespace Illuminate\Database\Query {
    class Builder {
        /** @return $this */
        public function limit(int $value) { return $this; }
    }
}

namespace App {
    use Illuminate\Database\Eloquent\Model;
    use Illuminate\Database\Eloquent\Builder;

    // Model with NO scope methods — hovering on its Builder chain
    // populates the cache with a plain Builder entry.
    class PlainModel extends Model {}

    // Model WITH scope methods.
    class ScopedModel extends Model {
        public function scopeFeatured(Builder $query): void {}
        public function scopeRecent(Builder $query): void {}
    }

    class Demo {
        public function run(): void {
            // Step 1: hover on get() here populates the Builder cache
            // with a plain Builder (no scope methods from PlainModel).
            PlainModel::where('id', 1)->get();

            // Step 2: hover on featured() here must still work even
            // though the Builder cache was seeded without scopes.
            ScopedModel::where('active', 1)->featured()->recent()->get();

            // Also test $variable path.
            $q = ScopedModel::where('status', 'draft');
            $q->featured();
        }
    }
}
"#;

    // Parse ONCE — do NOT re-parse between hovers.
    backend.update_ast(uri, content);

    let lines: Vec<&str> = content.lines().collect();

    // Helper to find a line and column.
    let find = |pattern: &str, token: &str| -> (u32, u32) {
        let idx = lines
            .iter()
            .enumerate()
            .find(|(_, l)| l.contains(pattern))
            .map(|(i, _)| i)
            .unwrap_or_else(|| panic!("should find {:?}", pattern));
        let col = lines[idx]
            .find(token)
            .unwrap_or_else(|| panic!("should find token {:?} on line {:?}", token, lines[idx]))
            as u32;
        (idx as u32, col + 1)
    };

    // ── Step 1: Hover on `get()` after PlainModel::where() ──────────
    // This forces `resolve_class_fully_cached` to cache the Builder
    // FQN with PlainModel's (empty) scope set.
    let (line, col) = find("PlainModel::where('id', 1)->get()", "get()");
    let h_get = backend.handle_hover(
        uri,
        content,
        Position {
            line,
            character: col,
        },
    );
    assert!(
        h_get.is_some(),
        "hover should work on get() after PlainModel::where() (line {})",
        line
    );

    // ── Step 2: Hover on `featured()` after ScopedModel::where() ────
    // The Builder cache now has an entry WITHOUT ScopedModel's scopes.
    // Before the fix, this would return None because the cached Builder
    // was missing the `featured` scope method.
    let (line, col) = find("ScopedModel::where('active', 1)->featured()", "featured()");
    let h_featured = backend.handle_hover(
        uri,
        content,
        Position {
            line,
            character: col,
        },
    );
    assert!(
        h_featured.is_some(),
        "hover should work on featured() after ScopedModel::where() even when Builder cache was seeded by PlainModel (line {})",
        line
    );
    let text = hover_text(h_featured.as_ref().unwrap());
    assert!(
        text.contains("featured"),
        "hover text should mention featured, got: {}",
        text
    );

    // ── Step 3: Hover on `recent()` chained after featured() ────────
    let chain_line = lines
        .iter()
        .enumerate()
        .find(|(_, l)| l.contains("->featured()->recent()"))
        .map(|(i, _)| i)
        .expect("should find chain line");
    let recent_col = lines[chain_line]
        .find("recent()")
        .expect("should find recent()") as u32
        + 1;
    let h_recent = backend.handle_hover(
        uri,
        content,
        Position {
            line: chain_line as u32,
            character: recent_col,
        },
    );
    assert!(
        h_recent.is_some(),
        "hover should work on recent() chained after featured() (line {})",
        chain_line
    );

    // ── Step 4: Hover on $q->featured() via variable ────────────────
    let (line, col) = find("$q->featured();", "featured();");
    let h_var = backend.handle_hover(
        uri,
        content,
        Position {
            line,
            character: col,
        },
    );
    assert!(
        h_var.is_some(),
        "hover should work on $q->featured() (Builder variable, line {})",
        line
    );
}

// ── Crash regressions ───────────────────────────────────────────────────────

/// Hovering on `$q` inside a nested closure that reuses the same variable
/// name as the outer closure used to cause infinite recursion in the hover
/// handler.  Fixed by a thread-local recursion depth guard in
/// `infer_callable_params_from_receiver`.
#[test]
fn hover_nested_closure_reused_variable_does_not_crash() {
    let backend = create_test_backend();
    let uri = "file:///nested_closure.php";
    let content = r#"<?php
namespace App;

class QueryBuilder {
    public function where(string $col, mixed $val = null): static { return $this; }
    public function whereNull(string $col): static { return $this; }
    public function orWhere(mixed ...$args): static { return $this; }
    public function whereHas(string $rel, \Closure $cb): static { return $this; }
}

class Repo {
    public function list(): void {
        $query = new QueryBuilder();
        $query->where(function ($q) {
            $q->whereNull('user_id')
                ->orWhere('user_id', 1)
                ->orWhere(function ($q): void {
                    $q->where('is_public', 1)
                        ->where('is_verified', 1);
                });
        });
    }
}
"#;
    backend.update_ast(uri, content);

    // Line 17 is `->orWhere(function ($q): void {` — the crashing line.
    // Hover at the midpoint which lands on `function` or `($q)`.
    let line_text = content.lines().nth(17).unwrap();
    let col = (line_text.len() / 2) as u32;
    let position = Position {
        line: 17,
        character: col,
    };

    // This must not stack-overflow.
    backend.handle_hover(uri, content, position);
}
