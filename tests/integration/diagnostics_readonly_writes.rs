use crate::common::create_test_backend;
use tower_lsp::lsp_types::*;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn collect(php: &str) -> Vec<Diagnostic> {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    backend.update_ast(uri, php);
    let mut out = Vec::new();
    backend.collect_slow_diagnostics(uri, php, &mut out);
    out.retain(|d| {
        d.code.as_ref().is_some_and(
            |c| matches!(c, NumberOrString::String(s) if s == "invalid_readonly_write"),
        )
    });
    out
}

fn messages(php: &str) -> Vec<String> {
    collect(php).into_iter().map(|d| d.message).collect()
}

// ─── Writes from outside the declaring class ────────────────────────────────

#[test]
fn a_readonly_property_written_from_top_level_code_is_flagged() {
    let php = r#"<?php
final class Box {
    public function __construct(public readonly int $value) {}
}

$box = new Box(1);
$box->value = 2;
"#;
    let msgs = messages(php);
    assert_eq!(msgs.len(), 1, "expected one diagnostic, got {msgs:?}");
    assert!(
        msgs[0].contains("Box::$value") && msgs[0].contains("outside its declaring class"),
        "unexpected message: {}",
        msgs[0]
    );
}

#[test]
fn a_declared_readonly_property_written_from_another_class_is_flagged() {
    let php = r#"<?php
class Box {
    public readonly int $value;

    public function __construct(int $value)
    {
        $this->value = $value;
    }
}

class Mutator {
    public function change(Box $box): void
    {
        $box->value = 2;
    }
}
"#;
    let msgs = messages(php);
    assert_eq!(msgs.len(), 1, "expected one diagnostic, got {msgs:?}");
    assert!(msgs[0].contains("Box::$value"), "message: {}", msgs[0]);
}

#[test]
fn a_subclass_initializing_its_parents_readonly_property_is_flagged() {
    let php = r#"<?php
class Base {
    public readonly int $value;
}

class Child extends Base {
    public function __construct()
    {
        $this->value = 1;
    }
}
"#;
    let msgs = messages(php);
    assert_eq!(msgs.len(), 1, "expected one diagnostic, got {msgs:?}");
    assert!(msgs[0].contains("Base::$value"), "message: {}", msgs[0]);
}

#[test]
fn a_compound_assignment_and_an_increment_are_both_flagged() {
    let php = r#"<?php
final class Box {
    public function __construct(public readonly int $value) {}
}

$box = new Box(1);
$box->value += 2;
$box->value++;
--$box->value;
"#;
    assert_eq!(collect(php).len(), 3);
}

// ─── Writes inside the declaring class ──────────────────────────────────────

#[test]
fn the_constructor_may_initialize_its_own_readonly_properties() {
    let php = r#"<?php
final class Box {
    public readonly int $value;
    public readonly string $label;

    public function __construct(int $value)
    {
        $this->value = $value;
        $this->label = 'box';
    }
}
"#;
    assert!(collect(php).is_empty());
}

#[test]
fn a_write_in_another_method_after_the_constructor_initialized_it_is_flagged() {
    let php = r#"<?php
final class Box {
    public function __construct(public readonly int $value) {}

    public function withValue(int $value): self
    {
        $clone = clone $this;
        $clone->value = $value;

        return $clone;
    }
}
"#;
    let msgs = messages(php);
    assert_eq!(msgs.len(), 1, "expected one diagnostic, got {msgs:?}");
    assert!(
        msgs[0].contains("after the constructor has initialized it"),
        "unexpected message: {}",
        msgs[0]
    );
}

#[test]
fn a_lazily_initialized_readonly_property_is_not_flagged() {
    let php = r#"<?php
final class Box {
    public readonly int $value;

    public function init(int $value): void
    {
        $this->value = $value;
    }
}
"#;
    assert!(collect(php).is_empty());
}

#[test]
fn a_conditional_constructor_assignment_does_not_make_a_later_write_an_error() {
    let php = r#"<?php
final class Box {
    public readonly int $value;

    public function __construct(?int $value)
    {
        if ($value !== null) {
            $this->value = $value;
        }
    }

    public function init(int $value): void
    {
        $this->value = $value;
    }
}
"#;
    assert!(collect(php).is_empty());
}

#[test]
fn clone_may_reinitialize_a_readonly_property() {
    let php = r#"<?php
final class Box {
    public function __construct(public readonly int $value) {}

    public function __clone(): void
    {
        $this->value = 0;
    }
}
"#;
    assert!(collect(php).is_empty());
}

#[test]
fn a_readonly_property_declared_by_a_trait_is_initialized_in_the_using_class() {
    let php = r#"<?php
trait HasValue {
    public readonly int $value;
}

final class Box {
    use HasValue;

    public function __construct(int $value)
    {
        $this->value = $value;
    }
}
"#;
    assert!(collect(php).is_empty());
}

// ─── Receivers other than a plain variable ──────────────────────────────────

#[test]
fn a_receiver_reached_through_a_property_chain_is_resolved() {
    let php = r#"<?php
final class Box {
    public function __construct(public readonly int $value) {}
}

final class Holder {
    public Box $box;

    public function change(): void
    {
        $this->box->value = 2;
    }
}
"#;
    let msgs = messages(php);
    assert_eq!(msgs.len(), 1, "expected one diagnostic, got {msgs:?}");
    assert!(msgs[0].contains("Box::$value"), "message: {}", msgs[0]);
}

#[test]
fn a_receiver_returned_by_a_call_is_resolved() {
    let php = r#"<?php
final class Box {
    public function __construct(public readonly int $value) {}
}

final class Holder {
    public function box(): Box
    {
        return new Box(1);
    }

    public function change(): void
    {
        $this->box()->value = 2;
    }
}
"#;
    assert_eq!(collect(php).len(), 1);
}

#[test]
fn a_union_receiver_with_one_writable_branch_is_not_flagged() {
    let php = r#"<?php
final class Locked {
    public function __construct(public readonly int $value) {}
}

final class Open {
    public int $value = 0;
}

function change(Locked|Open $target): void
{
    $target->value = 2;
}
"#;
    assert!(collect(php).is_empty());
}

// ─── Non-readonly writes ────────────────────────────────────────────────────

#[test]
fn a_writable_property_is_never_flagged() {
    let php = r#"<?php
final class Box {
    public function __construct(public int $value) {}
}

$box = new Box(1);
$box->value = 2;
"#;
    assert!(collect(php).is_empty());
}

#[test]
fn a_readonly_property_read_outside_the_class_is_not_a_write() {
    let php = r#"<?php
final class Box {
    public function __construct(public readonly int $value) {}
}

$box = new Box(1);
$other = $box->value;
"#;
    assert!(collect(php).is_empty());
}

#[test]
fn an_unresolved_receiver_is_never_flagged() {
    let php = r#"<?php
function mystery() {}

$thing = mystery();
$thing->value = 2;
"#;
    assert!(collect(php).is_empty());
}
