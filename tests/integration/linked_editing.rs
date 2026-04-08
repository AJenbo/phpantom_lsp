use crate::common::create_test_backend;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

/// Helper: open a file, trigger linked editing range at a position, and return results.
fn linked_editing_at(
    backend: &Backend,
    uri: &str,
    php: &str,
    line: u32,
    character: u32,
) -> Option<LinkedEditingRanges> {
    backend.update_ast(uri, php);
    backend.handle_linked_editing_range(uri, php, Position { line, character })
}

/// Shorthand to check that a range has the expected line, start col, and end col.
fn assert_range(r: &Range, line: u32, start_char: u32, end_char: u32) {
    assert_eq!(
        r.start.line, line,
        "expected line {}, got {}",
        line, r.start.line
    );
    assert_eq!(
        r.start.character, start_char,
        "expected start char {}, got {}",
        start_char, r.start.character
    );
    assert_eq!(
        r.end.character, end_char,
        "expected end char {}, got {}",
        end_char, r.end.character
    );
}

// ─── Basic variable linked editing ──────────────────────────────────────────

#[test]
fn linked_editing_variable_single_assignment() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo() {
    $user = new User();
    echo $user->name;
    return $user;
}
"#;

    // Cursor on `$user` at line 2 (the assignment)
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("expected linked editing ranges").ranges;

    // Ranges exclude the leading `$`, so `$user` (col 4..9) becomes col 5..9.
    assert_eq!(ranges.len(), 3);
    assert_range(&ranges[0], 2, 5, 9);
    assert_range(&ranges[1], 3, 10, 14);
    assert_range(&ranges[2], 4, 12, 16);
}

#[test]
fn linked_editing_no_word_pattern() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo() {
    $x = 1;
    echo $x;
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let linked = result.expect("expected linked editing ranges");

    // word_pattern should be None — ranges already exclude the `$` sigil
    // so no custom pattern is needed.
    assert!(
        linked.word_pattern.is_none(),
        "expected no word_pattern since ranges exclude the $ sigil"
    );
}

#[test]
fn linked_editing_scoped_to_function() {
    let backend = create_test_backend();
    let php = r#"<?php
function foo() {
    $x = 1;
    return $x;
}
function bar() {
    $x = 2;
    return $x;
}
"#;

    // Cursor on `$x` in foo() — should only include occurrences within foo
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("expected linked editing ranges").ranges;

    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].start.line, 2);
    assert_eq!(ranges[1].start.line, 3);
}

#[test]
fn linked_editing_includes_parameter() {
    let backend = create_test_backend();
    let php = r#"<?php
function greet(string $name) {
    echo $name;
    return $name;
}
"#;

    // Cursor on `$name` at the echo usage
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 10);
    let ranges = result.expect("expected linked editing ranges").ranges;

    // Should include: parameter def, echo usage, return usage
    assert!(
        ranges.len() >= 3,
        "expected at least 3 ranges (parameter + two usages), got {}",
        ranges.len()
    );
}

#[test]
fn linked_editing_foreach_variable() {
    let backend = create_test_backend();
    let php = r#"<?php
function process(array $items) {
    foreach ($items as $item) {
        echo $item;
    }
}
"#;

    // Cursor on `$item` in the foreach binding
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 24);
    let ranges = result.expect("expected linked editing ranges").ranges;

    assert!(
        ranges.len() >= 2,
        "expected at least 2 ranges for $item, got {}",
        ranges.len()
    );
}

// ─── Definition region splitting (reassignment) ─────────────────────────────

#[test]
fn linked_editing_reassignment_splits_regions() {
    let backend = create_test_backend();
    let php = r#"<?php
function test() {
    $foobar = new StaticPropHolder();
    $foobar->holder;
    $foobar = 'tank';
    echo $foobar;
}
"#;

    // Cursor on first `$foobar` (line 2) — region 1: lines 2-3
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result
        .expect("expected linked editing ranges for region 1")
        .ranges;
    assert_eq!(ranges.len(), 2, "region 1 should have 2 occurrences");
    assert_eq!(ranges[0].start.line, 2);
    assert_eq!(ranges[1].start.line, 3);

    // Cursor on second `$foobar` (line 4) — region 2: lines 4-5
    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 5);
    let ranges = result
        .expect("expected linked editing ranges for region 2")
        .ranges;
    assert_eq!(ranges.len(), 2, "region 2 should have 2 occurrences");
    assert_eq!(ranges[0].start.line, 4);
    assert_eq!(ranges[1].start.line, 5);
}

#[test]
fn linked_editing_reassignment_read_on_usage_line() {
    let backend = create_test_backend();
    let php = r#"<?php
function test() {
    $foobar = new StaticPropHolder();
    $foobar->holder;
    $foobar = 'tank';
    echo $foobar;
}
"#;

    // Cursor on the read of `$foobar` at line 3 (the ->holder line)
    let result = linked_editing_at(&backend, "file:///test.php", php, 3, 5);
    let ranges = result.expect("expected linked editing ranges").ranges;
    assert_eq!(ranges.len(), 2, "should be in region 1");
    assert_eq!(ranges[0].start.line, 2);
    assert_eq!(ranges[1].start.line, 3);

    // Cursor on the read of `$foobar` at line 5 (the echo line)
    let result = linked_editing_at(&backend, "file:///test.php", php, 5, 10);
    let ranges = result.expect("expected linked editing ranges").ranges;
    assert_eq!(ranges.len(), 2, "should be in region 2");
    assert_eq!(ranges[0].start.line, 4);
    assert_eq!(ranges[1].start.line, 5);
}

#[test]
fn linked_editing_self_reassignment_rhs_belongs_to_old_region() {
    let backend = create_test_backend();
    // In `$foobar = $foobar->value;`, the RHS `$foobar` reads the OLD
    // value, so it belongs to region 1.  The LHS `$foobar` starts region 2.
    let php = r#"<?php
function test() {
    $foobar = new Foo();
    echo $foobar;
    $foobar = $foobar->value;
    echo $foobar;
}
"#;

    // Cursor on the RHS `$foobar` at line 4 (inside `$foobar->value`)
    // col 15 should land on the second $foobar on that line
    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 15);
    let ranges = result.expect("RHS $foobar should link to region 1").ranges;
    // Region 1: assignment on line 2, read on line 3, RHS read on line 4
    assert_eq!(ranges.len(), 3, "region 1 should have 3 occurrences");
    assert_eq!(ranges[0].start.line, 2);
    assert_eq!(ranges[1].start.line, 3);
    assert_eq!(ranges[2].start.line, 4);

    // Cursor on the LHS `$foobar` at line 4 (the assignment target)
    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 5);
    let ranges = result.expect("LHS $foobar should link to region 2").ranges;
    // Region 2: assignment on line 4, read on line 5
    assert_eq!(ranges.len(), 2, "region 2 should have 2 occurrences");
    assert_eq!(ranges[0].start.line, 4);
    assert_eq!(ranges[1].start.line, 5);
}

#[test]
fn linked_editing_three_regions() {
    let backend = create_test_backend();
    let php = r#"<?php
function test() {
    $x = 1;
    echo $x;
    $x = 2;
    echo $x;
    $x = 3;
    echo $x;
}
"#;

    // Region 1: lines 2-3
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("region 1").ranges;
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].start.line, 2);
    assert_eq!(ranges[1].start.line, 3);

    // Region 2: lines 4-5
    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 5);
    let ranges = result.expect("region 2").ranges;
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].start.line, 4);
    assert_eq!(ranges[1].start.line, 5);

    // Region 3: lines 6-7
    let result = linked_editing_at(&backend, "file:///test.php", php, 6, 5);
    let ranges = result.expect("region 3").ranges;
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].start.line, 6);
    assert_eq!(ranges[1].start.line, 7);
}

#[test]
fn linked_editing_parameter_then_reassignment() {
    let backend = create_test_backend();
    let php = r#"<?php
function process(string $name) {
    echo $name;
    $name = strtoupper($name);
    echo $name;
}
"#;

    // Cursor on `$name` at line 2 (first echo) — should be in region 1
    // which includes the parameter and all reads before reassignment
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 10);
    let ranges = result.expect("region 1 with parameter").ranges;
    // Parameter def, echo on line 2, RHS $name in strtoupper on line 3
    assert!(
        ranges.len() >= 2,
        "expected at least 2 ranges in parameter region, got {}",
        ranges.len()
    );
    // All ranges should be before the reassignment's effective_from
    for r in &ranges {
        assert!(
            r.start.line <= 3,
            "parameter region range should be on line <= 3, got {}",
            r.start.line
        );
    }

    // Cursor on `$name` at line 4 (second echo) — should be in region 2
    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 10);
    let ranges = result.expect("region 2 after reassignment").ranges;
    assert_eq!(ranges.len(), 2);
    assert_eq!(ranges[0].start.line, 3); // the $name = ... assignment
    assert_eq!(ranges[1].start.line, 4); // echo $name
}

// ─── Cases that should return None ──────────────────────────────────────────

#[test]
fn linked_editing_returns_none_on_whitespace() {
    let backend = create_test_backend();
    let php = r#"<?php
function foo() {}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 0, 0);
    assert!(
        result.is_none(),
        "expected None when cursor is on non-variable token"
    );
}

#[test]
fn linked_editing_returns_none_on_class_name() {
    let backend = create_test_backend();
    let php = r#"<?php
class Foo {
    public function bar(): Foo {
        return new Foo();
    }
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 28);
    assert!(
        result.is_none(),
        "expected None for class name (not a variable)"
    );
}

#[test]
fn linked_editing_returns_none_on_member_access() {
    let backend = create_test_backend();
    let php = r#"<?php
class Calculator {
    public function add(int $a): int { return $a; }
    public function demo() {
        $this->add(1);
        $this->add(2);
    }
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 16);
    assert!(
        result.is_none(),
        "expected None for member access (not a local variable)"
    );
}

#[test]
fn linked_editing_returns_none_on_function_name() {
    let backend = create_test_backend();
    let php = r#"<?php
function helper() {}
helper();
helper();
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 1);
    assert!(
        result.is_none(),
        "expected None for function name (not a local variable)"
    );
}

#[test]
fn linked_editing_returns_none_on_single_occurrence() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo() {
    $onlyOnce = 42;
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    assert!(
        result.is_none(),
        "expected None when variable has only one occurrence"
    );
}

#[test]
fn linked_editing_returns_none_on_property_declaration() {
    let backend = create_test_backend();
    let php = r#"<?php
class Dog {
    public string $name;
    public function greet() {
        echo $this->name;
    }
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 19);
    assert!(result.is_none(), "expected None for property declarations");
}

#[test]
fn linked_editing_returns_none_on_this() {
    let backend = create_test_backend();
    let php = r#"<?php
class Example {
    public function demo() {
        $this->foo();
        $this->bar();
    }
    public function foo() {}
    public function bar() {}
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 3, 9);
    assert!(
        result.is_none(),
        "expected None for $this (not a renameable variable)"
    );
}

// ─── Closure scoping ────────────────────────────────────────────────────────

#[test]
fn linked_editing_closure_variable_scoped() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo() {
    $x = 1;
    $fn = function () {
        $x = 2;
        return $x;
    };
    return $x;
}
"#;

    // Cursor on `$x` inside the closure (line 4)
    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 9);
    let ranges = result
        .expect("expected linked editing ranges for closure $x")
        .ranges;

    assert_eq!(ranges.len(), 2, "expected 2 ranges in closure scope");
    assert_eq!(ranges[0].start.line, 4);
    assert_eq!(ranges[1].start.line, 5);
}

// ─── Ranges are sorted by position ──────────────────────────────────────────

#[test]
fn linked_editing_ranges_are_sorted() {
    let backend = create_test_backend();
    let php = r#"<?php
function test() {
    $a = 1;
    echo $a;
    echo $a;
    echo $a;
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("expected linked editing ranges").ranges;

    for i in 1..ranges.len() {
        let prev = &ranges[i - 1];
        let curr = &ranges[i];
        assert!(
            prev.start.line < curr.start.line
                || (prev.start.line == curr.start.line
                    && prev.start.character <= curr.start.character),
            "ranges should be sorted: {:?} should come before {:?}",
            prev,
            curr
        );
    }
}

// ─── All ranges have identical length ───────────────────────────────────────

#[test]
fn linked_editing_ranges_have_identical_length() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo() {
    $counter = 0;
    $counter++;
    echo $counter;
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("expected linked editing ranges").ranges;

    assert!(ranges.len() >= 2);

    let first_len = ranges[0].end.character - ranges[0].start.character;
    for (i, r) in ranges.iter().enumerate() {
        let len = r.end.character - r.start.character;
        assert_eq!(
            len, first_len,
            "range {} has length {} but expected {} (same as first range)",
            i, len, first_len
        );
    }
}

// ─── Compound assignment does not start a new region ────────────────────────

#[test]
fn linked_editing_compound_assignment_same_region() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo() {
    $count = 0;
    $count += 1;
    $count++;
    echo $count;
}
"#;

    // All four should be in the same region since += and ++ are not
    // plain assignments that rebind the variable.
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("expected linked editing ranges").ranges;
    assert_eq!(
        ranges.len(),
        4,
        "compound assignment should not split the region"
    );
}

// ─── Ranges exclude the `$` sigil ──────────────────────────────────────────

#[test]
fn linked_editing_ranges_exclude_dollar_sigil() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo() {
    $abc = 1;
    echo $abc;
}
"#;

    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("expected linked editing ranges").ranges;

    assert_eq!(ranges.len(), 2);
    // `$abc` starts at col 4, so the name `abc` starts at col 5 and ends at col 8.
    assert_range(&ranges[0], 2, 5, 8);
    // `$abc` in `echo $abc` starts at col 9, so `abc` is col 10..13.
    assert_range(&ranges[1], 3, 10, 13);
}
