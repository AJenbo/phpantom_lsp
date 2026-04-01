use crate::common::{create_psr4_workspace, create_test_backend};
use tower_lsp::lsp_types::*;

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Open a file, trigger `update_ast`, then collect unknown-member diagnostics.
fn unknown_member_diagnostics(
    backend: &phpantom_lsp::Backend,
    uri: &str,
    text: &str,
) -> Vec<Diagnostic> {
    backend.update_ast(uri, text);
    let mut out = Vec::new();
    backend.collect_unknown_member_diagnostics(uri, text, &mut out);
    out
}

// ═══════════════════════════════════════════════════════════════════════════
// Basic detection — instance methods
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_unknown_instance_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function bar(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->nonexistent();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown method diagnostic, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_existing_instance_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function bar(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->bar();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing method, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Basic detection — instance properties
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_unknown_instance_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public string $name = '';
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->missing;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("missing") && d.message.contains("not found")),
        "Expected unknown property diagnostic, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_existing_instance_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public string $name = '';
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->name;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing property, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Basic detection — static methods
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_unknown_static_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public static function bar(): void {}
}

Foo::nonexistent();
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown static method diagnostic, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_existing_static_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public static function bar(): void {}
}

Foo::bar();
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing static method, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Basic detection — constants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_unknown_class_constant() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    const BAR = 1;
}

$x = Foo::MISSING;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("MISSING") && d.message.contains("not found")),
        "Expected unknown constant diagnostic, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_existing_class_constant() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    const BAR = 1;
}

$x = Foo::BAR;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing constant, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Static properties
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_existing_static_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Config {
    public static string $appName = 'test';
}

$name = Config::$appName;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing static property, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// ::class magic constant
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_class_keyword() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {}

$name = Foo::class;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for ::class, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Magic method suppression
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_when_class_has_magic_call() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Magic {
    public function __call(string $name, array $args): mixed {}
}

class Consumer {
    public function run(): void {
        $m = new Magic();
        $m->anything();
        $m->whatever();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when __call exists, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_when_class_has_magic_get() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class DynProps {
    public function __get(string $name): mixed {}
}

class Consumer {
    public function run(): void {
        $d = new DynProps();
        $d->anything;
        $d->whatever;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when __get exists, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_when_class_has_magic_call_static() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class StaticMagic {
    public static function __callStatic(string $name, array $args): mixed {}
}

StaticMagic::anything();
StaticMagic::whatever();
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when __callStatic exists, got: {:?}",
        diags
    );
}

#[test]
fn magic_call_does_not_suppress_property_diagnostics() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Magic {
    public function __call(string $name, array $args): mixed {}
}

class Consumer {
    public function run(): void {
        $m = new Magic();
        $m->missingProp;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    // __call only handles method calls, not property access.
    // Without __get, property access should still be flagged.
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("missingProp") && d.message.contains("not found")),
        "Expected unknown property diagnostic even with __call (no __get), got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Inherited magic methods
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_when_parent_has_magic_call() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Base {
    public function __call(string $name, array $args): mixed {}
}

class Child extends Base {}

class Consumer {
    public function run(): void {
        $c = new Child();
        $c->anything();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when parent has __call, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_when_trait_has_magic_get() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
trait DynamicProperties {
    public function __get(string $name): mixed {}
}

class Widget {
    use DynamicProperties;
}

class Consumer {
    public function run(): void {
        $w = new Widget();
        $w->anything;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when trait provides __get, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Inheritance — methods, properties, constants
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_inherited_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Base {
    public function baseMethod(): void {}
}

class Child extends Base {}

class Consumer {
    public function run(): void {
        $c = new Child();
        $c->baseMethod();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for inherited method, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_inherited_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Base {
    public string $baseProp = '';
}

class Child extends Base {}

class Consumer {
    public function run(): void {
        $c = new Child();
        $c->baseProp;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for inherited property, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_inherited_constant() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Base {
    const BASE_CONST = 42;
}

class Child extends Base {}

$x = Child::BASE_CONST;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for inherited constant, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Trait members
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_trait_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
trait Greetable {
    public function greet(): string { return 'hello'; }
}

class Greeter {
    use Greetable;
}

class Consumer {
    public function run(): void {
        $g = new Greeter();
        $g->greet();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for trait method, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_trait_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
trait HasName {
    public string $name = '';
}

class User {
    use HasName;
}

class Consumer {
    public function run(): void {
        $u = new User();
        $u->name;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for trait property, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Virtual members (@method / @property / @mixin)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_phpdoc_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
/**
 * @method string getName()
 * @method void setName(string $name)
 */
class VirtualClass {}

class Consumer {
    public function run(): void {
        $v = new VirtualClass();
        $v->getName();
        $v->setName('test');
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for @method virtual member, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_phpdoc_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
/**
 * @property string $name
 * @property-read int $id
 */
class VirtualClass {
    public function __get(string $name): mixed {}
}

class Consumer {
    public function run(): void {
        $v = new VirtualClass();
        $v->name;
        $v->id;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for @property virtual member, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_mixin_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Helper {
    public function doHelp(): void {}
}

/**
 * @mixin Helper
 */
class Service {}

class Consumer {
    public function run(): void {
        $s = new Service();
        $s->doHelp();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for @mixin method, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// $this / self / static / parent contexts
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_unknown_method_on_this() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function bar(): void {
        $this->nonexistent();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown method diagnostic for $this->nonexistent(), got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_this_existing_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function bar(): void {}

    public function baz(): void {
        $this->bar();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for $this->bar(), got: {:?}",
        diags
    );
}

#[test]
fn flags_unknown_method_on_self() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function bar(): void {
        self::nonexistent();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown method diagnostic for self::nonexistent(), got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_self_existing_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public static function bar(): void {}

    public function baz(): void {
        self::bar();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for self::bar(), got: {:?}",
        diags
    );
}

#[test]
fn flags_unknown_method_on_static() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function bar(): void {
        static::nonexistent();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown method diagnostic for static::nonexistent(), got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_parent_existing_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Base {
    public function parentMethod(): void {}
}

class Child extends Base {
    public function childMethod(): void {
        parent::parentMethod();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for parent::parentMethod(), got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Case-insensitive method matching
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn method_matching_is_case_insensitive() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function getData(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->getdata();
        $f->GETDATA();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "PHP methods are case-insensitive, no diagnostic expected, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Multiple unknown members in one file
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_multiple_unknown_members() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function known(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->unknown1();
        $f->known();
        $f->unknown2();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert_eq!(
        diags.len(),
        2,
        "Expected exactly 2 diagnostics, got: {:?}",
        diags
    );
    assert!(diags.iter().any(|d| d.message.contains("unknown1")));
    assert!(diags.iter().any(|d| d.message.contains("unknown2")));
}

// ═══════════════════════════════════════════════════════════════════════════
// Unresolvable subject — no false positives
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_when_subject_unresolvable() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
function getUnknown(): mixed { return null; }

$x = getUnknown();
$x->whatever();
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when subject type is unresolvable, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_when_class_not_found() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
UnknownClass::method();
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    // The class itself is unknown — that's a different diagnostic
    // (unknown_classes). We should not also flag the member.
    assert!(
        diags.is_empty(),
        "No member diagnostic expected when the class itself is unknown, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Enum cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_enum_case() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;
}

$c = Color::Red;
$d = Color::Green;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for enum case access, got: {:?}",
        diags
    );
}

#[test]
fn flags_unknown_enum_case() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
enum Color {
    case Red;
    case Green;
    case Blue;
}

$c = Color::Purple;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("Purple") && d.message.contains("not found")),
        "Expected unknown member diagnostic for Color::Purple, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_backed_enum_case() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
}

$s = Status::Active;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for backed enum case, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Parameter type hint resolution
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_unknown_method_via_parameter_type_hint() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Service {
    public function doWork(): void {}
}

class Handler {
    public function handle(Service $svc): void {
        $svc->nonexistent();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown method diagnostic via parameter type, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_method_via_parameter_type_hint() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Service {
    public function doWork(): void {}
}

class Handler {
    public function handle(Service $svc): void {
        $svc->doWork();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing method via parameter, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_method_via_param_docblock_override() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Node {}

class FuncCall extends Node {
    public function isFirstClassCallable(): bool {}
}

class Handler {
    /**
     * @param FuncCall $node
     */
    public function handle(Node $node): void {
        $node->isFirstClassCallable();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing method via @param override, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Interface method access
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_interface_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
interface Renderable {
    public function render(): string;
}

class View implements Renderable {
    public function render(): string { return ''; }
}

class Consumer {
    public function run(Renderable $r): void {
        $r->render();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for interface method, got: {:?}",
        diags
    );
}

#[test]
fn flags_unknown_method_on_interface() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
interface Renderable {
    public function render(): string;
}

class Consumer {
    public function run(Renderable $r): void {
        $r->nonexistent();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown method diagnostic on interface, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Diagnostic metadata
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn diagnostic_has_warning_severity() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->missing();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(!diags.is_empty(), "Expected at least one diagnostic");
    assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
}

#[test]
fn diagnostic_has_code_and_source() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->missing();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(!diags.is_empty(), "Expected at least one diagnostic");
    assert_eq!(
        diags[0].code,
        Some(NumberOrString::String("unknown_member".to_string()))
    );
    assert_eq!(diags[0].source, Some("phpantom".to_string()));
}

#[test]
fn diagnostic_message_includes_class_name() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class MyService {}

class Consumer {
    public function run(): void {
        $s = new MyService();
        $s->missing();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(!diags.is_empty(), "Expected at least one diagnostic");
    assert!(
        diags[0].message.contains("MyService"),
        "Diagnostic should mention the class name, got: {}",
        diags[0].message
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Constructor calls should not flag
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_constructor_call() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function __construct() {}
}

$f = new Foo();
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for constructor call, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Method return type chain resolution
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_method_chain_existing_members() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Builder {
    public function where(): Builder { return $this; }
    public function get(): array { return []; }
}

class Service {
    public function query(): Builder { return new Builder(); }
}

class Consumer {
    public function run(): void {
        $s = new Service();
        $s->query()->where();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for valid method chain, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Cross-file resolution (PSR-4)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_unknown_member_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[(
            "src/Service.php",
            r#"<?php
namespace App;

class Service {
    public function doWork(): void {}
}
"#,
        )],
    );

    let uri = "file:///consumer.php";
    let text = r#"<?php
use App\Service;

class Consumer {
    public function run(Service $svc): void {
        $svc->nonexistent();
    }
}
"#;
    backend.update_ast(uri, text);
    let mut diags = Vec::new();
    backend.collect_unknown_member_diagnostics(uri, text, &mut diags);

    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("nonexistent") && d.message.contains("not found")),
        "Expected unknown method diagnostic across files, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_existing_member_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[(
            "src/Service.php",
            r#"<?php
namespace App;

class Service {
    public function doWork(): void {}
}
"#,
        )],
    );

    let uri = "file:///consumer.php";
    let text = r#"<?php
use App\Service;

class Consumer {
    public function run(Service $svc): void {
        $svc->doWork();
    }
}
"#;
    backend.update_ast(uri, text);
    let mut diags = Vec::new();
    backend.collect_unknown_member_diagnostics(uri, text, &mut diags);

    assert!(
        diags.is_empty(),
        "No diagnostics expected for existing member across files, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Mixed known and unknown in same access chain
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn only_flags_the_unknown_member_not_the_known() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    public function bar(): void {}
    public string $name = '';
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->bar();
        $f->name;
        $f->missing;
        $f->alsoMissing();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert_eq!(
        diags.len(),
        2,
        "Expected exactly 2 diagnostics (missing, alsoMissing), got: {:?}",
        diags
    );
    assert!(
        !diags.iter().any(|d| d.message.contains("'bar'")),
        "bar() should not be flagged"
    );
    assert!(
        !diags.iter().any(|d| d.message.contains("'name'")),
        "name should not be flagged"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Abstract class members
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_abstract_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
abstract class Shape {
    abstract public function area(): float;
}

class Consumer {
    public function run(Shape $s): void {
        $s->area();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for abstract method, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Promoted constructor properties
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_promoted_property() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class User {
    public function __construct(
        public readonly string $name,
        public readonly string $email,
    ) {}
}

class Consumer {
    public function run(): void {
        $u = new User('John', 'john@example.com');
        $u->name;
        $u->email;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for promoted properties, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Visibility should not affect detection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_private_method_on_this() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    private function secret(): void {}

    public function bar(): void {
        $this->secret();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    // We don't check visibility — the member exists, so no diagnostic.
    // Visibility violations are a different diagnostic (not implemented yet).
    assert!(
        diags.is_empty(),
        "No diagnostics expected for private method via $this, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_protected_method_on_this() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    protected function helper(): void {}

    public function bar(): void {
        $this->helper();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for protected method via $this, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Empty class produces diagnostic
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn flags_method_on_empty_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Empty_ {}

class Consumer {
    public function run(): void {
        $e = new Empty_();
        $e->anything();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("anything") && d.message.contains("not found")),
        "Expected unknown method diagnostic on empty class, got: {:?}",
        diags
    );
}

#[test]
fn flags_property_on_empty_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Empty_ {}

class Consumer {
    public function run(): void {
        $e = new Empty_();
        $e->anything;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("anything") && d.message.contains("not found")),
        "Expected unknown property diagnostic on empty class, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Enum constant access (not a case)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_enum_constant() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
enum Color {
    case Red;
    const DEFAULT = self::Red;
}

$x = Color::DEFAULT;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for enum constant, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Interface virtual members (@method on interface)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_interface_phpdoc_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
/**
 * @method string format()
 */
interface Formattable {}

class Widget implements Formattable {}

class Consumer {
    public function run(): void {
        $w = new Widget();
        $w->format();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for interface @method, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Self constant access
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_self_constant() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    const MAX = 100;

    public function getMax(): int {
        return self::MAX;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for self::MAX, got: {:?}",
        diags
    );
}

#[test]
fn flags_unknown_self_constant() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Foo {
    const MAX = 100;

    public function getMin(): int {
        return self::MIN;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("MIN") && d.message.contains("not found")),
        "Expected unknown constant diagnostic for self::MIN, got: {:?}",
        diags
    );
}

// ── stdClass suppression ────────────────────────────────────────────────

/// stdClass is a universal object container — any property access on it
/// should be silently accepted.
#[test]
fn no_diagnostic_for_property_on_stdclass() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
$obj = new \stdClass();
$obj->anything;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for property access on stdClass, got: {:?}",
        diags
    );
}

/// Method calls on stdClass should also be suppressed.
#[test]
fn no_diagnostic_for_method_on_stdclass() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
$obj = new \stdClass();
$obj->whatever();
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for method call on stdClass, got: {:?}",
        diags
    );
}

/// When stdClass appears as a branch in a union type, suppress diagnostics
/// for the entire union since the property could be on the stdClass branch.
#[test]
fn no_diagnostic_for_stdclass_in_union() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Strict {
    public function known(): void {}
}

/** @var Strict|\stdClass $obj */
$obj = new Strict();
$obj->unknown_prop;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when any union branch is stdClass, got: {:?}",
        diags
    );
}

/// stdClass passed as a parameter type hint should suppress diagnostics.
#[test]
fn no_diagnostic_for_stdclass_parameter() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
function process(\stdClass $obj): void {
    $obj->foo;
    $obj->bar;
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for property access on stdClass parameter, got: {:?}",
        diags
    );
}

/// A method returning stdClass should suppress diagnostics on the result.
#[test]
fn no_diagnostic_for_stdclass_return_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Factory {
    public function create(): \stdClass {
        return new \stdClass();
    }
}
$f = new Factory();
$f->create()->name;
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for property access on stdClass return type, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Method return → array access: $c->items()[0]->getLabel()
// ═══════════════════════════════════════════════════════════════════════════

/// When a method returns `Item[]` and the caller indexes inline
/// (`$c->items()[0]->getLabel()`), the element type should resolve
/// and no false "cannot verify" warning should appear.
#[test]
fn no_diagnostic_for_method_return_array_access_bracket_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Item {
    public function getLabel(): string { return ''; }
}
class Collection {
    /** @return Item[] */
    public function items(): array { return []; }
}
class Consumer {
    public function run(): void {
        $c = new Collection();
        $c->items()[0]->getLabel();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        !diags.iter().any(|d| d.message.contains("getLabel")),
        "No diagnostic expected for getLabel on Item resolved via method-return array access, got: {:?}",
        diags
    );
}

/// Same pattern with `array<int, Item>` generic return type.
#[test]
fn no_diagnostic_for_method_return_array_access_generic_type() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Item {
    public function getLabel(): string { return ''; }
}
class Collection {
    /** @return array<int, Item> */
    public function items(): array { return []; }
}
class Consumer {
    public function run(): void {
        $c = new Collection();
        $c->items()[0]->getLabel();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        !diags.iter().any(|d| d.message.contains("getLabel")),
        "No diagnostic expected for getLabel on Item resolved via generic method-return array access, got: {:?}",
        diags
    );
}

/// Static method returning an array: `Collection::all()[0]->getLabel()`.
#[test]
fn no_diagnostic_for_function_return_type_resolved_cross_file() {
    // Regression test: standalone functions store return types as short
    // names from the declaring file.  After FQN resolution in update_ast,
    // consumers in other files should resolve the type correctly.
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[(
            "src/Clock.php",
            r#"<?php
namespace App;

interface Clock {
    public function subMinutes(int $value = 1): Clock;
}
"#,
        )],
    );

    // A helper file that imports Clock via `use` and returns the short name.
    let helpers_uri = "file:///helpers.php";
    let helpers = r#"<?php
use App\Clock;

function now(): Clock {
    // stub
}
"#;
    backend.update_ast(helpers_uri, helpers);

    // Consumer file does NOT import App\Clock — it relies on the
    // function's return type being resolved to FQN at parse time.
    let uri = "file:///test.php";
    let text = r#"<?php
class Consumer {
    public function run(): void {
        now()->subMinutes(5);
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        !diags.iter().any(|d| d.message.contains("subMinutes")),
        "No diagnostic expected for subMinutes on function return type resolved via FQN, got: {:?}",
        diags
    );
}

#[test]
fn no_diagnostic_for_static_method_return_array_access() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Item {
    public function getLabel(): string { return ''; }
}
class Collection {
    /** @return Item[] */
    public static function all(): array { return []; }
}
class Consumer {
    public function run(): void {
        Collection::all()[0]->getLabel();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        !diags.iter().any(|d| d.message.contains("getLabel")),
        "No diagnostic expected for getLabel on Item resolved via static method-return array access, got: {:?}",
        diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Assert narrowing boundary prevents stale diagnostic cache reuse
// ═══════════════════════════════════════════════════════════════════════════

/// When a variable is used in a member access *before* an
/// `assert($var instanceof X)` and then used again *after* the assert,
/// the diagnostic cache must not reuse the pre-assert resolution.
/// Without the assert-offset discriminator in the cache key, the second
/// access would reuse the cached pre-assert type and produce a false
/// positive "property not found" diagnostic.
///
/// This reproduces the real-world Mockery pattern: `mock()` returns
/// `MockInterface`, the test calls `->shouldReceive()` (valid on
/// `MockInterface`), then `assert($x instanceof ConcreteClass)` narrows
/// the type so that `->id` (a property on the concrete class) is valid.
#[test]
fn no_false_positive_after_assert_instanceof() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
interface MockInterface {
    public function shouldReceive(string $name): self;
}
class MolliePayment {
    public string $id = '';
    public function canBeRefunded(): bool { return true; }
}
class TestCase {
    protected function mock(string $class): MockInterface {}
}
class Test extends TestCase {
    public function test(): void {
        $x = $this->mock(MolliePayment::class);
        $x->shouldReceive('canBeRefunded');
        assert($x instanceof MolliePayment);
        echo $x->id;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        !diags.iter().any(|d| d.message.contains("id")),
        "No diagnostic expected for 'id' after assert($x instanceof MolliePayment), got: {:?}",
        diags
    );
}

/// Verify that the pre-assert access is still correctly diagnosed when
/// the member does NOT exist on the pre-assert type.
#[test]
fn still_flags_unknown_member_before_assert_instanceof() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
interface MockInterface {
    public function shouldReceive(string $name): self;
}
class MolliePayment {
    public string $id = '';
    public function canBeRefunded(): bool { return true; }
}
class TestCase {
    protected function mock(string $class): MockInterface {}
}
class Test extends TestCase {
    public function test(): void {
        $x = $this->mock(MolliePayment::class);
        echo $x->id;
        assert($x instanceof MolliePayment);
        echo $x->id;
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    // The first $x->id (before the assert) should be flagged because
    // $x is MockInterface and MockInterface has no 'id' property.
    let id_diags: Vec<_> = diags.iter().filter(|d| d.message.contains("id")).collect();
    assert_eq!(
        id_diags.len(),
        1,
        "Expected exactly one diagnostic for 'id' (the pre-assert access), got: {:?}",
        id_diags
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Static return type resolution to concrete subclass
// ═══════════════════════════════════════════════════════════════════════════

/// When a parent class declares `public static function first(): ?static`,
/// calling `ChildClass::first()` should resolve `static` to `ChildClass`,
/// not the parent. No false-positive diagnostics should be emitted for
/// members that exist on the child class.
#[test]
fn no_diagnostic_for_static_return_type_on_subclass_static_call() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Model {
    /** @return ?static */
    public static function first(): ?static { return null; }
    public function save(): bool { return true; }
}
class AdminUser extends Model {
    public function assignRole(string $role): void {}
}
class Seeder {
    public function run(): void {
        $admin = AdminUser::first();
        $admin->assignRole('admin');
        $admin->save();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected when static return type resolves to subclass, got: {:?}",
        diags
    );
}

/// Same scenario but with a bare `static` return (non-nullable).
#[test]
fn no_diagnostic_for_bare_static_return_type_on_subclass() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Builder {
    /** @return static */
    public static function create(): static { return new static(); }
    public function build(): void {}
}
class AppBuilder extends Builder {
    public function setDebug(): void {}
}
class Factory {
    public function make(): void {
        $b = AppBuilder::create();
        $b->setDebug();
        $b->build();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for bare static return on subclass, got: {:?}",
        diags
    );
}

/// Chained static method calls: `Product::query()->where('x')->get()`
/// where `query()` and `where()` both return `static`.
#[test]
fn no_diagnostic_for_static_return_chained_static_call() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let text = r#"<?php
class Model {
    /** @return static */
    public static function query(): static { return new static(); }
    /** @return static */
    public function where(string $col): static { return $this; }
    public function get(): array { return []; }
}
class Product extends Model {
    public function applyDiscount(): void {}
}
class Controller {
    public function index(): void {
        $q = Product::query();
        $q->where('active');
        $q->applyDiscount();
        $q->get();
    }
}
"#;
    let diags = unknown_member_diagnostics(&backend, uri, text);
    assert!(
        diags.is_empty(),
        "No diagnostics expected for chained static return calls, got: {:?}",
        diags
    );
}

/// Cross-file variant: parent with `?static` return lives in a separate
/// PSR-4 file. Accessing subclass-specific members after a static method
/// call should not produce false-positive diagnostics.
#[test]
fn no_diagnostic_for_static_return_type_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[
            (
                "src/Model.php",
                r#"<?php
namespace App;

class Model {
    /** @return ?static */
    public static function first(): ?static { return null; }
    public function save(): bool { return true; }
}
"#,
            ),
            (
                "src/AdminUser.php",
                r#"<?php
namespace App;

class AdminUser extends Model {
    public function assignRole(string $role): void {}
}
"#,
            ),
        ],
    );

    let uri = "file:///consumer.php";
    let text = r#"<?php
use App\AdminUser;

class Seeder {
    public function run(): void {
        $admin = AdminUser::first();
        $admin->assignRole('admin');
        $admin->save();
    }
}
"#;
    backend.update_ast(uri, text);
    let mut diags = Vec::new();
    backend.collect_unknown_member_diagnostics(uri, text, &mut diags);

    assert!(
        diags.is_empty(),
        "No diagnostics expected when static return type resolves to subclass cross-file, got: {:?}",
        diags
    );
}
