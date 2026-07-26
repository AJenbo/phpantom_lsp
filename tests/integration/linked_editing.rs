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

#[test]
fn linked_editing_conditional_reassignment_belongs_to_outer_region() {
    let backend = create_test_backend();
    let php = r#"<?php
function test(bool $bool): void {
    $a = 'a';
    $a .= 'a';
    $a = $a . 'b';
    if ($bool) {
        $a = 'b';
    }
    echo $a;
}
"#;

    // Cursor on `echo $a` (line 8, col 10) — region 2 starts at `$a = $a . 'b'`
    // and should include the conditional `$a = 'b'` inside the if and `echo $a`.
    let result = linked_editing_at(&backend, "file:///test.php", php, 8, 10);
    let ranges = result.expect("expected linked editing ranges").ranges;
    // Region 2: `$a` on line 4 (LHS of `$a = $a . 'b'`), `$a` on line 6
    // (inside if), `$a` on line 8 (echo)
    assert_eq!(
        ranges.len(),
        3,
        "should include LHS assignment, conditional reassignment, and echo"
    );
    assert_range(&ranges[0], 4, 5, 6); // $a = $a . 'b' (LHS)
    assert_range(&ranges[1], 6, 9, 10); // $a = 'b' inside if
    assert_range(&ranges[2], 8, 10, 11); // echo $a
}

#[test]
fn linked_editing_try_catch_reassignment_same_region() {
    let backend = create_test_backend();
    let php = r#"<?php
function test(): void {
    $conn = null;
    try {
        $conn = 'connected';
    } catch (\Exception $e) {
        $conn = 'failed';
    }
    echo $conn;
}
"#;

    // Cursor on `echo $conn` (line 8) — all $conn should be one region
    let result = linked_editing_at(&backend, "file:///test.php", php, 8, 10);
    let ranges = result.expect("expected linked editing ranges").ranges;
    assert_eq!(
        ranges.len(),
        4,
        "should include initial assignment, try, catch, and echo: got {:?}",
        ranges
    );
}

#[test]
fn linked_editing_closure_use_capture_links_across_scopes() {
    let backend = create_test_backend();
    let php = r#"<?php
function demo(): void {
    $name = 'world';
    $fn = function () use ($name) {
        echo $name;
    };
    echo $name;
}
"#;

    // All 4 occurrences of $name should link: the outer assignment,
    // the use() capture, the inner echo, and the outer echo.
    // Cursor on `$name` inside the closure body (line 4)
    let result = linked_editing_at(&backend, "file:///test.php", php, 4, 14);
    let ranges = result.expect("inner $name should bridge to outer").ranges;
    assert_eq!(
        ranges.len(),
        4,
        "expected all 4 occurrences linked: got {:?}",
        ranges
    );

    // Cursor on `$name` in the use() clause (line 3)
    let result = linked_editing_at(&backend, "file:///test.php", php, 3, 29);
    let ranges = result.expect("use($name) should link all").ranges;
    assert_eq!(
        ranges.len(),
        4,
        "expected all 4 occurrences from use() cursor: got {:?}",
        ranges
    );

    // Cursor on outer `$name` (line 2, assignment)
    let result = linked_editing_at(&backend, "file:///test.php", php, 2, 5);
    let ranges = result.expect("outer $name should bridge to closure").ranges;
    assert_eq!(
        ranges.len(),
        4,
        "expected all 4 occurrences from outer cursor: got {:?}",
        ranges
    );

    // Cursor on outer `echo $name` (line 6)
    let result = linked_editing_at(&backend, "file:///test.php", php, 6, 10);
    let ranges = result.expect("outer echo $name should link all").ranges;
    assert_eq!(
        ranges.len(),
        4,
        "expected all 4 occurrences from outer echo: got {:?}",
        ranges
    );
}

// ─── Range integrity guards ─────────────────────────────────────────────────
//
// Linked editing hands the editor ranges it will mirror every keystroke
// into, so a range that does not point at the variable corrupts the buffer
// silently.  These tests cover the three invariants the handler enforces.

/// Assert that every returned range delimits `$name`'s name in `php` and
/// that the range containing the cursor is among them.
fn assert_ranges_are_the_variable(php: &str, name: &str, ranges: &[Range]) {
    let lines: Vec<&str> = php.lines().collect();
    for r in ranges {
        assert_eq!(
            r.start.line, r.end.line,
            "a variable token cannot span lines: {r:?}"
        );
        assert!(
            r.start.character < r.end.character,
            "range must be non-empty and not inverted: {r:?}"
        );
        let line = lines
            .get(r.start.line as usize)
            .unwrap_or_else(|| panic!("range past end of file: {r:?}"));
        let text: String = line
            .chars()
            .skip(r.start.character as usize)
            .take((r.end.character - r.start.character) as usize)
            .collect();
        assert_eq!(
            text, name,
            "range {r:?} does not cover `{name}` in `{line}`"
        );
        assert_eq!(
            line.chars().nth(r.start.character as usize - 1),
            Some('$'),
            "range {r:?} is not preceded by the `$` sigil in `{line}`"
        );
    }
}

/// A symbol map built from older text must never be used to build ranges.
///
/// `symbol_maps` is refreshed on a background task after each keystroke, so
/// a request can arrive while the map still describes the previous buffer.
/// Converting its byte offsets against the newer text produced ranges over
/// unrelated code — including an inverted one that made the editor enter
/// linked editing with the cursor in a comment and mirror the typing into
/// two arbitrary spots further down the file.
#[test]
fn linked_editing_returns_none_for_stale_symbol_map() {
    let backend = create_test_backend();
    let before = r#"<?php
class Cache {
    function refresh($store, $key, $staleSeconds) {
        $createdKey = self::PREFIX . $key;
        $created = $store->get($createdKey); // Peak at the create time
        if (is_numeric($created) && (int)$created < self::epoch($store)) {
            $store->put($createdKey, 1, $staleSeconds);
        }
    }
}
"#;
    // The user inserts a docblock above the `$created` assignment.  Every
    // offset from `before` now points one line too early.
    let after = before.replace(
        "        $created = $store->get",
        "        /** */\n        $created = $store->get",
    );

    backend.update_ast("file:///cache.php", before);

    // Cursor inside the freshly typed `/** */`, and on either side of it.
    for (line, character) in [(4u32, 16u32), (4, 15), (4, 11), (3, 12), (5, 10)] {
        let result = backend.handle_linked_editing_range(
            "file:///cache.php",
            &after,
            Position { line, character },
        );
        assert!(
            result.is_none(),
            "stale map must not produce ranges at {line}:{character}, got {:?}",
            result.map(|r| r.ranges)
        );
    }

    // Once the map catches up, linked editing works again.
    backend.update_ast("file:///cache.php", &after);
    let ranges = backend
        .handle_linked_editing_range(
            "file:///cache.php",
            &after,
            Position {
                line: 5,
                character: 10,
            },
        )
        .expect("fresh map should link $created")
        .ranges;
    assert_eq!(ranges.len(), 3, "got {ranges:?}");
    assert_ranges_are_the_variable(&after, "created", &ranges);
}

/// A same-length edit does not shift offsets, so the length check cannot
/// catch it.  The per-range byte check must: once `$created` is overtyped
/// with a different name, no range spells `$created` any more.
#[test]
fn linked_editing_returns_none_when_token_text_changed_in_place() {
    let backend = create_test_backend();
    let before = r#"<?php
function demo($store) {
    $created = $store->get('k');
    return is_numeric($created) ? $created : null;
}
"#;
    // Same byte count, so `matches_source` still agrees.
    let after = before.replace("$created", "$updated");
    assert_eq!(before.len(), after.len());

    backend.update_ast("file:///demo.php", before);
    let result = backend.handle_linked_editing_range(
        "file:///demo.php",
        &after,
        Position {
            line: 2,
            character: 5,
        },
    );
    assert!(
        result.is_none(),
        "renamed-in-place token must not produce ranges, got {:?}",
        result.map(|r| r.ranges)
    );
}

/// Whatever cursor position the editor asks about, the response must be
/// well-formed: real `$name` tokens, no overlaps, and the occurrence under
/// the cursor among them.  Sweeping every position of each snippet exercises
/// region selection and closure bridging across variable shapes far more
/// widely than the hand-written cases above.
#[test]
fn linked_editing_ranges_are_well_formed_at_every_position() {
    let backend = create_test_backend();
    let snippets = [
        // Parameters, foreach bindings, reassignment, closure capture.
        r#"<?php
function demo(string $name, array $rows) {
    $out = [];
    foreach ($rows as $row) {
        $out[] = $row . $name;
    }
    $out = array_filter($out);
    $join = function (string $sep) use ($out, $name) {
        return $name . implode($sep, $out);
    };
    $name .= '!';
    return $join(',') . $name;
}
"#,
        // Static and global declarations, every loop form, if/elseif/else.
        r#"<?php
function counters($a) {
    static $seen = 0;
    global $conf;
    $out = $conf;
    if ($a) { $out = 1; } elseif (!$a) { $out = 2; } else { $out = 3; }
    while ($a) { $out .= $a; }
    do { $seen++; } while ($seen < 5);
    for ($k = 0; $k < 3; $k++) { $out += $k; }
    return $out . $seen;
}
"#,
        // List destructuring, by-reference parameters, variadics.
        r#"<?php
function spread(&$ref, ...$rest) {
    [$x, $y] = $rest;
    $ref = $x + $y + $x;
    foreach ($rest as $r) { $ref .= $r; }
    return $ref;
}
"#,
        // Interpolation and heredoc bodies.
        r#"<?php
function interpolate($x) {
    $msg = "val $x and {$x} end";
    return $msg . <<<EOT
        $x here $msg
        EOT;
}
"#,
        // switch/match arms and try/catch/finally branches.
        r#"<?php
function branches($v) {
    switch ($v) { case 1: $r = 'a'; break; default: $r = 'b'; }
    try { $c = conn(); } catch (\Throwable $e) { $c = null; } finally { log($c . $r); }
    return match ($v) { 1 => $c, default => $r . $v };
}
"#,
        // Promoted constructor property alongside a same-named local, and a
        // docblock `@var` mention.
        r#"<?php
class Ids {
    public function __construct(private int $id) {}
    function bump(int $id) {
        /** @var int $id */
        $id = $id + 1;
        return $id;
    }
}
"#,
        // Nested closures shadowing the captured name.
        r#"<?php
function nested($n) {
    $cb = function ($n) use ($n) {
        return function () use ($n) { return $n; };
    };
    return $cb($n)();
}
"#,
    ];

    let mut linked_positions = 0;
    for (idx, php) in snippets.iter().enumerate() {
        let uri = format!("file:///sweep{idx}.php");
        backend.update_ast(&uri, php);

        for (line_no, line) in php.lines().enumerate() {
            for character in 0..=line.chars().count() {
                let pos = Position {
                    line: line_no as u32,
                    character: character as u32,
                };
                let Some(result) = backend.handle_linked_editing_range(&uri, php, pos) else {
                    continue;
                };
                linked_positions += 1;
                assert!(
                    result.ranges.len() >= 2,
                    "a linked response needs at least 2 ranges in {uri} at {pos:?}: {:?}",
                    result.ranges
                );

                // Every range must cover the same variable name...
                let first = &result.ranges[0];
                let name: String = php
                    .lines()
                    .nth(first.start.line as usize)
                    .unwrap()
                    .chars()
                    .skip(first.start.character as usize)
                    .take((first.end.character - first.start.character) as usize)
                    .collect();
                assert_ranges_are_the_variable(php, &name, &result.ranges);

                // ...the ranges must be sorted, and must not overlap or repeat...
                let mut sorted = result.ranges.clone();
                sorted.sort_by_key(|r| (r.start.line, r.start.character));
                assert_eq!(
                    sorted, result.ranges,
                    "ranges must be sorted in {uri} at {pos:?}"
                );
                for pair in sorted.windows(2) {
                    assert!(
                        (pair[0].end.line, pair[0].end.character)
                            < (pair[1].start.line, pair[1].start.character),
                        "ranges overlap in {uri} at {pos:?}: {:?}",
                        result.ranges
                    );
                }

                // ...and one of them must be where the user is typing.
                assert!(
                    result.ranges.iter().any(|r| r.start.line == pos.line
                        // The cursor may sit on the `$` (one before the range)
                        // or just past the last character.
                        && pos.character + 1 >= r.start.character
                        && pos.character <= r.end.character),
                    "no returned range contains the cursor in {uri} at {pos:?}: {:?}",
                    result.ranges
                );
            }
        }
    }

    assert!(
        linked_positions > 100,
        "expected the sweep to find plenty of linkable positions, got {linked_positions}"
    );
}

/// A Blade template is analysed as generated PHP, so the ranges start life in
/// coordinates the user cannot see.  They must come back as positions in the
/// template itself, verified against the template text.
#[tokio::test]
async fn linked_editing_blade_ranges_land_in_template_source() {
    use tower_lsp::LanguageServer;

    let backend = create_test_backend();
    let uri = Url::parse("file:///view.blade.php").unwrap();
    let text = "<div>\n@foreach ($rows as $row)\n  <p>{{ $row }}</p>\n  <p>{{ $row->id }}</p>\n@endforeach\n</div>\n";
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "blade".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    // The foreach binding and both `{{ $row }}` reads.
    for (line, character) in [(1u32, 22u32), (2, 10), (3, 10)] {
        let result = backend
            .linked_editing_range(LinkedEditingRangeParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: Position { line, character },
                },
                work_done_progress_params: Default::default(),
            })
            .await
            .unwrap()
            .unwrap_or_else(|| panic!("expected $row to be linked at {line}:{character}"));

        assert_eq!(
            result.ranges.len(),
            3,
            "expected the binding plus both reads at {line}:{character}: {:?}",
            result.ranges
        );
        assert_ranges_are_the_variable(text, "row", &result.ranges);
    }
}
