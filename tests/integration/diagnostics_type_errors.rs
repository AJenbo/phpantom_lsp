use crate::common::{create_test_backend, create_test_backend_with_function_stubs};
use tower_lsp::lsp_types::*;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn collect(php: &str) -> Vec<Diagnostic> {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    backend.update_ast(uri, php);
    let mut out = Vec::new();
    backend.collect_type_error_diagnostics(uri, php, &mut out);
    out
}

fn collect_with_stubs(php: &str) -> Vec<Diagnostic> {
    let backend = create_test_backend_with_function_stubs();
    let uri = "file:///test.php";
    backend.update_ast(uri, php);
    let mut out = Vec::new();
    backend.collect_type_error_diagnostics(uri, php, &mut out);
    out
}

fn has_type_error(diags: &[Diagnostic]) -> bool {
    diags.iter().any(|d| {
        d.code
            .as_ref()
            .is_some_and(|c| matches!(c, NumberOrString::String(s) if s == "type_error.argument"))
    })
}

fn type_error_messages(diags: &[Diagnostic]) -> Vec<String> {
    diags
        .iter()
        .filter(|d| {
            d.code.as_ref().is_some_and(
                |c| matches!(c, NumberOrString::String(s) if s == "type_error.argument"),
            )
        })
        .map(|d| d.message.clone())
        .collect()
}

// ─── Basic: string passed to int parameter ──────────────────────────────────

#[test]
fn flags_string_passed_to_int_param() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    $s = "hello";
    takes_int($s);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for string passed to int, got: {diags:?}"
    );
    let msgs = type_error_messages(&diags);
    assert!(
        msgs.iter()
            .any(|m| m.contains("int") && m.contains("string")),
        "Expected message mentioning int and string, got: {msgs:?}"
    );
}

// ─── PHP juggling: int passed to string is accepted ─────────────────────────

#[test]
fn no_diagnostic_for_int_to_string_juggling() {
    let php = r#"<?php
function takes_string(string $x): void {}

function test(): void {
    $n = 42;
    takes_string($n);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag int passed to string (PHP type juggling), got: {diags:?}"
    );
}

// ─── Basic: null passed to non-nullable parameter ───────────────────────────

#[test]
fn flags_null_passed_to_non_nullable_param() {
    let php = r#"<?php
function takes_string(string $x): void {}

function test(): void {
    takes_string(null);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for null passed to string, got: {diags:?}"
    );
}

// ─── No diagnostic: correct types ──────────────────────────────────────────

#[test]
fn no_diagnostic_for_correct_types() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    $n = 42;
    takes_int($n);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag correct int argument, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_string_to_string() {
    let php = r#"<?php
function takes_string(string $x): void {}

function test(): void {
    $s = "hello";
    takes_string($s);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag correct string argument, got: {diags:?}"
    );
}

// ─── No diagnostic: nullable parameter accepts null ─────────────────────────

#[test]
fn no_diagnostic_for_null_to_nullable() {
    let php = r#"<?php
function takes_nullable(?string $x): void {}

function test(): void {
    takes_nullable(null);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag null passed to nullable param, got: {diags:?}"
    );
}

// ─── No diagnostic: subclass passed to parent type ──────────────────────────

#[test]
fn no_diagnostic_for_subclass() {
    let php = r#"<?php
class Animal {}
class Cat extends Animal {}

function takes_animal(Animal $a): void {}

function test(): void {
    $cat = new Cat();
    takes_animal($cat);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag subclass Cat passed to Animal param, got: {diags:?}"
    );
}

// ─── No diagnostic: mixed parameter accepts anything ────────────────────────

#[test]
fn no_diagnostic_for_mixed_param() {
    let php = r#"<?php
function takes_mixed(mixed $x): void {}

function test(): void {
    takes_mixed(42);
    takes_mixed("hello");
    takes_mixed(null);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag arguments to mixed param, got: {diags:?}"
    );
}

// ─── No diagnostic: untyped parameter ───────────────────────────────────────

#[test]
fn no_diagnostic_for_untyped_param() {
    let php = r#"<?php
function takes_anything($x): void {}

function test(): void {
    takes_anything(42);
    takes_anything("hello");
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag arguments to untyped param, got: {diags:?}"
    );
}

// ─── No diagnostic: argument unpacking ──────────────────────────────────────

#[test]
fn no_diagnostic_for_unpacking() {
    let php = r#"<?php
function takes_ints(int $a, int $b): void {}

function test(): void {
    $args = ["hello", "world"];
    takes_ints(...$args);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag call with argument unpacking, got: {diags:?}"
    );
}

// ─── No diagnostic: unresolvable function ───────────────────────────────────

#[test]
fn no_diagnostic_for_unresolvable_function() {
    let php = r#"<?php
function test(): void {
    unknown_function(42);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag call to unresolvable function, got: {diags:?}"
    );
}

// ─── Flags: array passed to string ──────────────────────────────────────────

#[test]
fn flags_array_passed_to_string() {
    let php = r#"<?php
function takes_string(string $x): void {}

function test(): void {
    $arr = [1, 2, 3];
    takes_string($arr);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for array passed to string, got: {diags:?}"
    );
}

// ─── Flags: bool passed to int ──────────────────────────────────────────────

#[test]
fn flags_bool_passed_to_string() {
    let php = r#"<?php
function takes_string(string $x): void {}

function test(): void {
    $b = true;
    takes_string($b);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for bool passed to string, got: {diags:?}"
    );
}

// ─── No diagnostic: int to float (PHP widening) ────────────────────────────

#[test]
fn no_diagnostic_for_int_to_float() {
    let php = r#"<?php
function takes_float(float $x): void {}

function test(): void {
    $n = 42;
    takes_float($n);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag int passed to float (PHP widening), got: {diags:?}"
    );
}

// ─── No diagnostic: callable param with closure ─────────────────────────────

#[test]
fn no_diagnostic_for_closure_to_callable() {
    let php = r#"<?php
function takes_callable(callable $fn): void {}

function test(): void {
    takes_callable(function() { return 1; });
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag closure passed to callable, got: {diags:?}"
    );
}

// ─── No diagnostic: object param with class instance ────────────────────────

#[test]
fn no_diagnostic_for_class_to_object() {
    let php = r#"<?php
class Foo {}

function takes_object(object $x): void {}

function test(): void {
    $f = new Foo();
    takes_object($f);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag class instance passed to object, got: {diags:?}"
    );
}

// ─── Flags: wrong class type ────────────────────────────────────────────────

#[test]
fn flags_wrong_class_type() {
    let php = r#"<?php
class Dog {}
class Cat {}

function takes_dog(Dog $d): void {}

function test(): void {
    $cat = new Cat();
    takes_dog($cat);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for Cat passed to Dog param, got: {diags:?}"
    );
}

// ─── No diagnostic: interface implementation ────────────────────────────────

#[test]
fn no_diagnostic_for_interface_impl() {
    let php = r#"<?php
interface Printable {
    public function print(): void;
}
class Report implements Printable {
    public function print(): void {}
}

function takes_printable(Printable $p): void {}

function test(): void {
    $r = new Report();
    takes_printable($r);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag interface impl passed to interface param, got: {diags:?}"
    );
}

// ─── Diagnostic has correct code and severity ───────────────────────────────

#[test]
fn diagnostic_has_correct_code_and_severity() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    $s = "hello";
    takes_int($s);
}
"#;
    let diags = collect(php);
    let type_diags: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.code.as_ref().is_some_and(
                |c| matches!(c, NumberOrString::String(s) if s == "type_error.argument"),
            )
        })
        .collect();
    assert!(
        !type_diags.is_empty(),
        "Expected at least one type error diagnostic"
    );
    assert_eq!(
        type_diags[0].severity,
        Some(DiagnosticSeverity::ERROR),
        "Type error should be ERROR severity"
    );
    assert_eq!(
        type_diags[0].source.as_deref(),
        Some("phpantom"),
        "Source should be phpantom"
    );
}

// ─── Method calls: flags wrong type to method parameter ─────────────────────

#[test]
fn flags_wrong_type_to_method_param() {
    let php = r#"<?php
class Formatter {
    public function format(string $text): string {
        return $text;
    }
}

function test(): void {
    $f = new Formatter();
    $arr = [1, 2, 3];
    $f->format($arr);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for array passed to string method param, got: {diags:?}"
    );
}

// ─── Method calls: no diagnostic for correct type ───────────────────────────

#[test]
fn no_diagnostic_for_correct_method_arg() {
    let php = r#"<?php
class Formatter {
    public function format(string $text): string {
        return $text;
    }
}

function test(): void {
    $f = new Formatter();
    $f->format("hello");
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag correct string argument to method, got: {diags:?}"
    );
}

// ─── Static method calls ────────────────────────────────────────────────────

#[test]
fn flags_wrong_type_to_static_method() {
    let php = r#"<?php
class MathHelper {
    public static function add(int $a, int $b): int {
        return $a + $b;
    }
}

function test(): void {
    MathHelper::add("hello", "world");
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected type errors for strings passed to int static method params, got: {diags:?}"
    );
}

// ─── Constructor calls ──────────────────────────────────────────────────────

#[test]
fn flags_wrong_type_to_constructor() {
    let php = r#"<?php
class User {
    public function __construct(
        public string $name,
        public int $age,
    ) {}
}

function test(): void {
    new User(42, "not a number");
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected type errors for wrong types in constructor, got: {diags:?}"
    );
}

// ─── Multiple arguments: only wrong ones flagged ────────────────────────────

#[test]
fn flags_only_wrong_argument() {
    let php = r#"<?php
function mixed_params(int $a, string $b, float $c): void {}

function test(): void {
    $arr = [1, 2];
    mixed_params(42, $arr, 3.14);
}
"#;
    let diags = collect(php);
    let msgs = type_error_messages(&diags);
    // Argument 2 ($b) should be flagged: array passed to string
    assert!(
        msgs.iter()
            .any(|m| m.contains("$b") && m.contains("string")),
        "Expected type error for arg 2 ($b), got: {msgs:?}"
    );
}

// ─── Union type: compatible if any branch matches ───────────────────────────

#[test]
fn no_diagnostic_for_matching_union_branch() {
    let php = r#"<?php
function takes_int_or_string(int|string $x): void {}

function test(): void {
    $s = "hello";
    takes_int_or_string($s);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag string passed to int|string, got: {diags:?}"
    );
}

// ─── Literal values ─────────────────────────────────────────────────────────

#[test]
fn no_diagnostic_for_literal_int_to_int() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    takes_int(42);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag literal int passed to int, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_literal_string_to_string() {
    let php = r#"<?php
function takes_string(string $x): void {}

function test(): void {
    takes_string("hello");
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag literal string passed to string, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_literal_true_to_bool() {
    let php = r#"<?php
function takes_bool(bool $x): void {}

function test(): void {
    takes_bool(true);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag literal true passed to bool, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_literal_float_to_float() {
    let php = r#"<?php
function takes_float(float $x): void {}

function test(): void {
    takes_float(3.14);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag literal float passed to float, got: {diags:?}"
    );
}

// ─── No diagnostic for self/static parameters ──────────────────────────────

#[test]
fn no_diagnostic_for_self_param() {
    let php = r#"<?php
class Node {
    public function merge(self $other): void {}
}

function test(): void {
    $a = new Node();
    $b = new Node();
    $a->merge($b);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag self parameter (skipped conservatively), got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_self_in_union_param() {
    let php = r#"<?php
class Decimal {
    public function add(int|self|string $value): self { return $this; }
}

function test(): void {
    $a = new Decimal();
    $b = new Decimal();
    $a->add($b);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag Decimal passed to int|self|string (self in union), got: {diags:?}"
    );
}

// ─── No diagnostic: iterable param with array ───────────────────────────────

#[test]
fn no_diagnostic_for_array_to_iterable() {
    let php = r#"<?php
function takes_iterable(iterable $items): void {}

function test(): void {
    $arr = [1, 2, 3];
    takes_iterable($arr);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag array passed to iterable, got: {diags:?}"
    );
}

// ─── Message format ─────────────────────────────────────────────────────────

#[test]
fn message_mentions_param_name_and_types() {
    let php = r#"<?php
function takes_int(int $count): void {}

function test(): void {
    $s = "hello";
    takes_int($s);
}
"#;
    let diags = collect(php);
    let msgs = type_error_messages(&diags);
    assert!(!msgs.is_empty(), "Expected at least one type error");
    let msg = &msgs[0];
    assert!(
        msg.contains("$count"),
        "Message should mention parameter name, got: {msg}"
    );
    assert!(
        msg.contains("int"),
        "Message should mention expected type, got: {msg}"
    );
    assert!(
        msg.contains("string"),
        "Message should mention actual type, got: {msg}"
    );
}

// ─── Message shows FQN when short names collide ─────────────────────────────

#[test]
fn message_always_shows_fqn() {
    // Diagnostic messages always show full type names (FQN) so the
    // developer can find and fix the types.  Short names strip the
    // namespace which is the very information needed to resolve a
    // mismatch.
    let php = r#"<?php
/** @param \Vendor\Foo $f */
function takes_vendor(\Vendor\Foo $f): void {}

function test(): void {
    /** @var \App\Foo $f */
    $f = null;
    takes_vendor($f);
}
"#;
    let diags = collect(php);
    let msgs = type_error_messages(&diags);
    assert!(
        !msgs.is_empty(),
        "Expected a type error for different-FQN same-short-name classes"
    );
    let msg = &msgs[0];
    // The message must include namespace-qualified names so the two
    // types are distinguishable.  Never "expects Foo, got Foo".
    assert!(
        msg.contains("Vendor\\Foo") && msg.contains("App\\Foo"),
        "Message should show FQN when short names collide, got: {msg}"
    );
}

// ─── Built-in function with stubs ───────────────────────────────────────────

#[test]
fn flags_array_passed_to_stub_function() {
    // str_contains expects (string $haystack, string $needle)
    let php = r#"<?php
function test(): void {
    $arr = [1, 2, 3];
    str_contains($arr, "x");
}
"#;
    let diags = collect_with_stubs(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for array passed to str_contains, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_correct_stub_function() {
    let php = r#"<?php
function test(): void {
    str_contains("hello world", "hello");
}
"#;
    let diags = collect_with_stubs(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag correct string args to str_contains, got: {diags:?}"
    );
}

// ─── No diagnostic: nullable arg to nullable param ──────────────────────────

#[test]
fn no_diagnostic_for_nullable_to_nullable() {
    let php = r#"<?php
function takes_nullable(?int $x): void {}

function test(?int $val): void {
    takes_nullable($val);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag ?int passed to ?int, got: {diags:?}"
    );
}

// ─── No diagnostic: default value param when argument omitted ───────────────

#[test]
fn no_false_positive_for_default_params() {
    let php = r#"<?php
function with_defaults(int $a, string $b = "hello"): void {}

function test(): void {
    with_defaults(42);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag when fewer args than params (defaults cover), got: {diags:?}"
    );
}

// ─── Multiple calls in same function ────────────────────────────────────────

#[test]
fn flags_multiple_bad_calls() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    $s = "hello";
    takes_int($s);
    $arr = [1, 2];
    takes_int($arr);
}
"#;
    let diags = collect(php);
    let msgs = type_error_messages(&diags);
    assert!(
        msgs.len() >= 2,
        "Expected at least 2 type errors, got {}: {msgs:?}",
        msgs.len()
    );
}

// ─── No diagnostic for array to array param ─────────────────────────────────

#[test]
fn no_diagnostic_for_array_to_array() {
    let php = r#"<?php
function takes_array(array $items): void {}

function test(): void {
    $arr = [1, 2, 3];
    takes_array($arr);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag array passed to array, got: {diags:?}"
    );
}

// ─── Flags: string literal passed to int param ─────────────────────────────

#[test]
fn flags_string_literal_to_int() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    takes_int("hello");
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for string literal to int, got: {diags:?}"
    );
}

// ─── Flags: null literal to non-nullable ────────────────────────────────────

#[test]
fn flags_null_literal_to_non_nullable() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    takes_int(null);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for null to int, got: {diags:?}"
    );
}

// ─── Nested calls ───────────────────────────────────────────────────────────

#[test]
fn no_diagnostic_in_nested_scope() {
    let php = r#"<?php
function takes_int(int $x): void {}

class Foo {
    public function bar(): void {
        $n = 42;
        takes_int($n);
    }
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag correct call inside method, got: {diags:?}"
    );
}

#[test]
fn flags_error_in_nested_scope() {
    let php = r#"<?php
function takes_int(int $x): void {}

class Foo {
    public function bar(): void {
        $s = "hello";
        takes_int($s);
    }
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error inside method body, got: {diags:?}"
    );
}

// ─── No diagnostic for bool to bool ─────────────────────────────────────────

#[test]
fn no_diagnostic_for_bool_to_bool() {
    let php = r#"<?php
function takes_bool(bool $x): void {}

function test(): void {
    $b = false;
    takes_bool($b);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag bool passed to bool, got: {diags:?}"
    );
}

// ─── No diagnostic for null to nullable union ───────────────────────────────

#[test]
fn no_diagnostic_for_null_to_nullable_union() {
    let php = r#"<?php
function takes_nullable_union(string|null $x): void {}

function test(): void {
    takes_nullable_union(null);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag null to string|null, got: {diags:?}"
    );
}

// ─── No diagnostic for new expression to matching class param ───────────────

#[test]
fn no_diagnostic_for_new_to_class_param() {
    let php = r#"<?php
class User {}

function takes_user(User $u): void {}

function test(): void {
    takes_user(new User());
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag new User() passed to User param, got: {diags:?}"
    );
}

// ─── Flags: incompatible new expression ─────────────────────────────────────

#[test]
fn flags_new_wrong_class() {
    let php = r#"<?php
class Dog {}
class Cat {}

function takes_dog(Dog $d): void {}

function test(): void {
    takes_dog(new Cat());
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected a type error for new Cat() passed to Dog param, got: {diags:?}"
    );
}

// ─── Numeric string literals vs numeric-string ──────────────────────────────

#[test]
fn no_diagnostic_for_numeric_string_literal_to_numeric_string() {
    let php = r#"<?php
/** @param numeric-string $v */
function takes_numeric_string(string $v): void {}

function test(): void {
    takes_numeric_string('0.00');
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag numeric string literal '0.00' passed to numeric-string param, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_integer_string_literal_to_numeric_string() {
    let php = r#"<?php
/** @param numeric-string $v */
function takes_numeric_string(string $v): void {}

function test(): void {
    takes_numeric_string('42');
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag numeric string literal '42' passed to numeric-string param, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_numeric_literal_to_union_with_numeric_string() {
    let php = r#"<?php
class Decimal {
    /** @param Decimal|int|numeric-string $value */
    public function add(int|self|string $value): self { return $this; }
}

function test(): void {
    $d = new Decimal();
    $d->add('0.00');
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag '0.00' passed to Decimal|int|numeric-string union, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_non_numeric_string_literal_to_numeric_string() {
    // String literals resolve as bare `string` in the expression resolver,
    // so we cannot distinguish numeric from non-numeric at the type level.
    // Conservative: stay silent because the string *might* be numeric.
    let php = r#"<?php
/** @param numeric-string $v */
function takes_numeric_string(string $v): void {}

function test(): void {
    takes_numeric_string('hello');
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag string literal passed to numeric-string param (conservative), got: {diags:?}"
    );
}

// ─── Array shape vs generic array ───────────────────────────────────────────

#[test]
fn no_diagnostic_for_array_shape_to_generic_array_string_mixed() {
    let php = r#"<?php
function takes_data(array $data): void {}

/** @param array<string, mixed> $data */
function takes_typed_data(array $data): void {}

function test(): void {
    takes_typed_data(['id' => 1, 'refunded_amount' => 'foo']);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag array shape passed to array<string, mixed> param, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// New rules: bare array ↔ typed array
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_bare_array_to_typed_array_generic() {
    let php = r#"<?php
/** @param array<string> $items */
function takes_string_array(array $items): void {}

function test(): void {
    $arr = [];
    takes_string_array($arr);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag bare array passed to array<string>, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_bare_array_to_list_generic() {
    let php = r#"<?php
/** @param list<int> $ids */
function takes_ids(array $ids): void {}

function test(): void {
    $arr = [];
    takes_ids($arr);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag bare array passed to list<int>, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// New rules: nullable arg → non-nullable param (MAYBE)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_nullable_arg_to_non_nullable_param() {
    let php = r#"<?php
class Carbon {}

function takes_carbon(Carbon $c): void {}

function test(?Carbon $c): void {
    takes_carbon($c);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag ?Carbon passed to Carbon (developer may have null-checked), got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_nullable_string_to_string() {
    let php = r#"<?php
function takes_string(string $s): void {}

function test(?string $s): void {
    takes_string($s);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag ?string passed to string (MAYBE), got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_non_nullable_to_nullable_param() {
    let php = r#"<?php
class Carbon {}

function takes_nullable(?Carbon $c): void {}

function test(): void {
    $c = new Carbon();
    takes_nullable($c);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag Carbon passed to ?Carbon, got: {diags:?}"
    );
}

#[test]
fn flags_nullable_string_to_int() {
    // ?string should NOT be accepted where int is expected —
    // the non-null part (string) is still incompatible with int.
    let php = r#"<?php
function takes_int(int $x): void {}

function test(?string $s): void {
    takes_int($s);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected type error for ?string passed to int, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// New rules: Stringable objects accepted as string
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_object_to_string() {
    let php = r#"<?php
class HtmlString {
    public function __toString(): string { return ''; }
}

function takes_string(string $s): void {}

function test(): void {
    $h = new HtmlString();
    takes_string($h);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag Stringable object passed to string, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// New rules: PHP type juggling
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_int_to_string_method() {
    let php = r#"<?php
class Logger {
    public function log(string $message): void {}
}

function test(): void {
    $l = new Logger();
    $l->log(42);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag int passed to string method param (PHP juggling), got: {diags:?}"
    );
}

#[test]
fn still_flags_array_to_string() {
    // array → string is NOT type juggling, it's a real error.
    let php = r#"<?php
function takes_string(string $x): void {}

function test(): void {
    $arr = [1, 2, 3];
    takes_string($arr);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected type error for array passed to string, got: {diags:?}"
    );
}

#[test]
fn still_flags_bool_to_int() {
    let php = r#"<?php
function takes_int(int $x): void {}

function test(): void {
    $b = true;
    takes_int($b);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Expected type error for bool passed to int, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// New rules: list<X> ↔ array<int, X>
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_list_to_array_int() {
    let php = r#"<?php
/** @param array<int, string> $items */
function takes_indexed(array $items): void {}

/** @return list<string> */
function get_list(): array { return []; }

function test(): void {
    takes_indexed(get_list());
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag list<string> passed to array<int, string>, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_array_int_to_list() {
    let php = r#"<?php
/** @param list<string> $items */
function takes_list(array $items): void {}

/** @return array<int, string> */
function get_array(): array { return []; }

function test(): void {
    takes_list(get_array());
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag array<int, string> passed to list<string>, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// New rules: class-string covariance
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_class_string_covariance() {
    let php = r#"<?php
class Animal {}
class Cat extends Animal {}

/** @param class-string<Animal> $cls */
function takes_animal_class(string $cls): void {}

/** @return class-string<Cat> */
function get_cat_class(): string { return Cat::class; }

function test(): void {
    takes_animal_class(get_cat_class());
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag class-string<Cat> passed to class-string<Animal>, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// New rules: iterable<...> accepts arrays
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_array_to_iterable_generic() {
    let php = r#"<?php
/** @param iterable<mixed> $items */
function takes_iterable_generic(iterable $items): void {}

function test(): void {
    $arr = [1, 2, 3];
    takes_iterable_generic($arr);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag array passed to iterable<mixed>, got: {diags:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Template parameter detection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn no_diagnostic_for_expected_type_template_param() {
    // PHPUnit uses ExpectedType as a template parameter name.
    // We should recognise it and suppress the diagnostic.
    let php = r#"<?php
class TestCase {
    /**
     * @template ExpectedType
     * @param ExpectedType $expected
     */
    public function assertEquals(mixed $expected, mixed $actual): void {}
}

function test(): void {
    $t = new TestCase();
    $t->assertEquals("hello", "world");
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag args to method with ExpectedType template param, got: {diags:?}"
    );
}

// ─── Interface → concrete implementor: MAYBE (reverse hierarchy) ────────────

#[test]
fn no_diagnostic_for_interface_arg_to_concrete_param() {
    // CarbonInterface passed where Carbon is expected.
    // Carbon implements CarbonInterface, so the value *might* be
    // the right concrete type at runtime (MAYBE → stay silent).
    let php = r#"<?php
interface CarbonInterface {}
class Carbon implements CarbonInterface {}

function takes_carbon(Carbon $c): void {}

function test(CarbonInterface $ci): void {
    takes_carbon($ci);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag interface arg passed to concrete param (MAYBE), got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_parent_arg_to_child_param() {
    // Parent class passed where child is expected.
    // The value might be the child at runtime.
    let php = r#"<?php
class Animal {}
class Cat extends Animal {}

function takes_cat(Cat $c): void {}

function test(Animal $a): void {
    takes_cat($a);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should not flag parent arg to child param (MAYBE), got: {diags:?}"
    );
}

// ─── Final class: reverse direction is NO ───────────────────────────────────

#[test]
fn flags_final_class_arg_to_child_param() {
    // A final class cannot have subtypes, so if `Jack` is final and
    // does not extend `JackSparrow`, it is definitely NOT a
    // JackSparrow.  The reverse-direction MAYBE does not apply.
    let php = r#"<?php
final class Jack {}
class JackSparrow {}

function takes_sparrow(JackSparrow $j): void {}

function test(Jack $j): void {
    takes_sparrow($j);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Should flag final class that is not a subtype (NO), got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_final_class_that_implements_interface() {
    // A final class that implements the expected interface is
    // definitely compatible (direction 1: arg extends param → YES).
    let php = r#"<?php
interface Printable {}
final class Report implements Printable {}

function takes_printable(Printable $p): void {}

function test(): void {
    $r = new Report();
    takes_printable($r);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Final class implementing interface should be accepted (YES), got: {diags:?}"
    );
}

#[test]
fn flags_final_class_to_unrelated_interface() {
    // A final class that does NOT implement the interface is
    // definitely wrong — it can't be narrowed to anything else.
    let php = r#"<?php
interface Serializable {}
final class Rock {}

function takes_serializable(Serializable $s): void {}

function test(Rock $r): void {
    takes_serializable($r);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Final class not implementing interface should be flagged (NO), got: {diags:?}"
    );
}

// ─── object and stdClass are not universal supertypes ───────────────────────

#[test]
fn no_diagnostic_for_object_arg_to_specific_class() {
    // `object` passed where a specific class is expected is MAYBE.
    // The developer may have narrowed via instanceof before the call.
    // We flag `$obj->method()` as unknown-member instead — that's
    // where the developer learns they need better types.
    let php = r#"<?php
class User {}

function takes_user(User $u): void {}

function test(object $o): void {
    takes_user($o);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "object to specific class should be MAYBE (silent), got: {diags:?}"
    );
}

#[test]
fn flags_stdclass_to_unrelated_class() {
    // stdClass is a concrete class, not a universal parent.
    // Passing stdClass where User is expected is wrong.
    let php = r#"<?php
class User {}

function takes_user(User $u): void {}

function test(): void {
    $o = new \stdClass();
    takes_user($o);
}
"#;
    let diags = collect(php);
    assert!(
        has_type_error(&diags),
        "Should flag stdClass passed to unrelated class param, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_specific_class_to_object_param() {
    // Any class instance IS an object — this is always valid.
    let php = r#"<?php
class User {}

function takes_object(object $o): void {}

function test(): void {
    $u = new User();
    takes_object($u);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Should accept class instance passed to object param, got: {diags:?}"
    );
}

// ─── Non-final parent with unrelated child: MAYBE ───────────────────────────

#[test]
fn no_diagnostic_for_non_final_unrelated_with_common_parent() {
    // If a non-final class is passed where a sibling subclass is
    // expected, the developer might have narrowed.  Stay silent.
    let php = r#"<?php
class Animal {}
class Dog extends Animal {}
class Cat extends Animal {}

function takes_dog(Dog $d): void {}

function test(Animal $a): void {
    takes_dog($a);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Non-final parent to child should be MAYBE (silent), got: {diags:?}"
    );
}

// ─── Hierarchy resolution: Collection implements Countable ──────────────────

#[test]
fn no_diagnostic_for_collection_implementing_countable() {
    // Collection implements Countable.  The hierarchy check should
    // resolve this through the class loader without needing a
    // blanket "any object → Countable" rule.
    let php = r#"<?php
interface Countable {
    public function count(): int;
}
class Collection implements Countable {
    public function count(): int { return 0; }
}

function takes_countable(Countable $c): void {}

function test(): void {
    $col = new Collection();
    takes_countable($col);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Collection implementing Countable should be accepted via hierarchy, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_generic_collection_to_countable() {
    // Generic Collection<int, User> still implements Countable.
    // The base_name() of Generic("Collection", [int, User]) is
    // "Collection", and the hierarchy walk should find Countable.
    let php = r#"<?php
interface Countable {
    public function count(): int;
}

/** @template T */
class Collection implements Countable {
    public function count(): int { return 0; }
}

/** @param Collection<int, string> $items */
function takes_countable(Countable $c): void {}

function test(): void {
    /** @var Collection<int, string> $col */
    $col = new Collection();
    takes_countable($col);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Generic Collection<int, string> implementing Countable should be accepted, got: {diags:?}"
    );
}

// ── Transitive interface inheritance ────────────────────────────────────────

#[test]
fn no_diagnostic_for_transitive_interface_inheritance() {
    // ResponseInterface extends MessageInterface.
    // Response implements ResponseInterface.
    // Passing Response where MessageInterface is expected should
    // succeed via transitive interface walk.
    let php = r#"<?php
interface MessageInterface {
    public function getBody(): string;
}
interface ResponseInterface extends MessageInterface {
    public function getStatusCode(): int;
}
class Response implements ResponseInterface {
    public function getBody(): string { return ''; }
    public function getStatusCode(): int { return 200; }
}

function takes_message(MessageInterface $msg): void {}

function test(): void {
    $r = new Response();
    takes_message($r);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Response implementing ResponseInterface (extends MessageInterface) should be accepted, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_deep_transitive_interface() {
    // A extends B extends C extends D.
    // Class implements A.
    // Passing Class where D is expected should work through
    // the full transitive interface chain.
    let php = r#"<?php
interface D {}
interface C extends D {}
interface B extends C {}
interface A extends B {}
class Impl implements A {}

function takes_d(D $x): void {}

function test(): void {
    $impl = new Impl();
    takes_d($impl);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Class implementing A (extends B extends C extends D) should satisfy D, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_parent_class_transitive_interface() {
    // Parent class implements an interface that extends another.
    // Child class should satisfy the grandparent interface.
    let php = r#"<?php
interface Base {}
interface Middle extends Base {}
class Parent1 implements Middle {}
class Child extends Parent1 {}

function takes_base(Base $x): void {}

function test(): void {
    $c = new Child();
    takes_base($c);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Child (extends Parent1 implements Middle extends Base) should satisfy Base, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_multi_extends_interface() {
    // Interface extends multiple parent interfaces.
    // Class implementing the child interface should satisfy any parent.
    let php = r#"<?php
interface Readable {}
interface Writable {}
interface ReadWritable extends Readable, Writable {}
class Stream implements ReadWritable {}

function takes_readable(Readable $r): void {}
function takes_writable(Writable $w): void {}

function test(): void {
    $s = new Stream();
    takes_readable($s);
    takes_writable($s);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Stream implementing ReadWritable (extends Readable, Writable) should satisfy both, got: {diags:?}"
    );
}

// ── Array slice covariance ──────────────────────────────────────────────────

#[test]
fn no_diagnostic_for_array_slice_subclass() {
    // Child[] should be accepted where Parent[] is expected.
    let php = r#"<?php
class Animal {}
class Cat extends Animal {}

/** @param Animal[] $items */
function takes_animals(array $items): void {}

function test(): void {
    /** @var Cat[] $cats */
    $cats = [];
    takes_animals($cats);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Cat[] should be accepted where Animal[] is expected, got: {diags:?}"
    );
}

// ── Object-like arg to callable param ───────────────────────────────────────

#[test]
fn no_diagnostic_for_object_to_callable_param() {
    // An object might implement __invoke, making it callable.
    // We can't verify this statically, so stay silent (MAYBE).
    let php = r#"<?php
class MyHandler {}

function takes_callable(callable $fn): void {}

function test(): void {
    $handler = new MyHandler();
    takes_callable($handler);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Object passed to callable param should be MAYBE (might have __invoke), got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_object_to_callable_union() {
    // When param is callable|array and arg is an object, the union
    // recursion should check each branch.  The callable branch
    // should accept the object (MAYBE via __invoke).
    let php = r#"<?php
class Sequence {}

/** @param callable|array<string, mixed> $state */
function apply_state(callable|array $state): void {}

function test(): void {
    $seq = new Sequence();
    apply_state($seq);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Object passed to callable|array union should be accepted (MAYBE), got: {diags:?}"
    );
}

// ── BackedEnum hierarchy ────────────────────────────────────────────────────

#[test]
fn no_diagnostic_for_backed_enum_to_backed_enum_param() {
    // A specific backed enum should be accepted where BackedEnum
    // is expected, since all backed enums implement BackedEnum.
    let php = r#"<?php
interface BackedEnum {}
enum Color: string implements BackedEnum {
    case Red = 'red';
    case Blue = 'blue';
}

function takes_backed_enum(BackedEnum $e): void {}

function test(): void {
    $c = Color::Red;
    takes_backed_enum($c);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Backed enum implementing BackedEnum should be accepted, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_list_of_backed_enum_to_array_int_backed_enum() {
    // list<Color> should be accepted where array<int, BackedEnum>
    // is expected, combining list↔array and enum hierarchy rules.
    let php = r#"<?php
interface BackedEnum {}
enum Color: string implements BackedEnum {
    case Red = 'red';
    case Blue = 'blue';
}

/** @param array<int, BackedEnum> $items */
function takes_backed_enums(array $items): void {}

function test(): void {
    /** @var list<Color> $colors */
    $colors = [Color::Red, Color::Blue];
    takes_backed_enums($colors);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "list<Color> should be accepted where array<int, BackedEnum> is expected, got: {diags:?}"
    );
}

// ── Implicit BackedEnum/UnitEnum interface on enums ─────────────────────────

#[test]
fn no_diagnostic_for_backed_enum_implicit_backed_enum_interface() {
    // PHP backed enums automatically implement BackedEnum.
    // The parser adds this implicit interface so the hierarchy check
    // recognises the relationship.
    let php = r#"<?php
interface UnitEnum {}
interface BackedEnum extends UnitEnum {}

enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
}

function takes_backed_enum(BackedEnum $e): void {}

function test(): void {
    takes_backed_enum(Status::Active);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "String-backed enum should satisfy BackedEnum via implicit interface, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_unit_enum_implicit_unit_enum_interface() {
    // PHP enums without a backing type automatically implement UnitEnum.
    let php = r#"<?php
interface UnitEnum {}

enum Suit {
    case Hearts;
    case Diamonds;
}

function takes_unit_enum(UnitEnum $e): void {}

function test(): void {
    takes_unit_enum(Suit::Hearts);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Unit enum should satisfy UnitEnum via implicit interface, got: {diags:?}"
    );
}

// ── Anonymous class arguments ───────────────────────────────────────────────

#[test]
fn no_diagnostic_for_anonymous_class_extending_expected_type() {
    // `new class extends Foo { … }` passed where Foo is expected.
    // Anonymous classes can't be verified reliably (synthetic names
    // aren't globally indexed), so we stay silent.
    let php = r#"<?php
class Model {
    public function save(): void {}
}

function takes_model(Model $m): void {}

function test(): void {
    $anon = new class extends Model {};
    takes_model($anon);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Anonymous class extending Model should be accepted where Model is expected, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_for_anonymous_class_implementing_interface() {
    // `new class implements Iface { … }` passed where Iface is expected.
    let php = r#"<?php
interface Renderable {
    public function render(): string;
}

function takes_renderable(Renderable $r): void {}

function test(): void {
    $anon = new class implements Renderable {
        public function render(): string { return ''; }
    };
    takes_renderable($anon);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Anonymous class implementing Renderable should be accepted, got: {diags:?}"
    );
}

// ─── Type guard narrowing (B4) ──────────────────────────────────────────────

#[test]
fn no_diagnostic_when_is_string_guard_narrows_before_call() {
    let php = r#"<?php
function takes_string(string $s): void {}

function test(mixed $val): void {
    if (!is_string($val)) {
        return;
    }
    takes_string($val);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "is_string guard should narrow mixed to string, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_when_instanceof_guard_narrows_before_call() {
    let php = r#"<?php
class Foo {}
function takes_foo(Foo $f): void {}

function test(mixed $val): void {
    if (!($val instanceof Foo)) {
        throw new \Exception('not Foo');
    }
    takes_foo($val);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "instanceof guard should narrow mixed to Foo, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_when_null_coalesce_with_throw_narrows() {
    let php = r#"<?php
function takes_string(string $s): void {}

function test(array $params): void {
    $authToken = $params['authToken'] ?? null;
    if (!$authToken || !is_string($authToken)) {
        throw new \Exception('missing');
    }
    takes_string($authToken);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Guard clause with throw should narrow type before call site, got: {diags:?}"
    );
}

#[test]
fn no_diagnostic_when_is_int_guard_narrows_nullable() {
    let php = r#"<?php
function takes_int(int $i): void {}

function test(?int $val): void {
    if ($val === null) {
        return;
    }
    takes_int($val);
}
"#;
    let diags = collect(php);
    assert!(
        !has_type_error(&diags),
        "Null check with early return should narrow ?int to int, got: {diags:?}"
    );
}
