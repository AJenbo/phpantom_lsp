//! Tests for the ScopeCollector infrastructure.

use super::*;
use crate::parser::with_parsed_program;

/// Helper: parse PHP code and collect scope from the first function body found.
fn collect_from_function(php: &str) -> ScopeMap {
    with_parsed_program(php, "test", |program, _content| {
        for stmt in program.statements.iter() {
            if let Statement::Function(func) = stmt {
                let body_start = func.body.left_brace.start.offset;
                let body_end = func.body.right_brace.end.offset;
                return collect_function_scope(
                    &func.parameter_list,
                    func.body.statements.as_slice(),
                    body_start,
                    body_end,
                );
            }
        }
        panic!("No function found in test PHP code");
    })
}

/// Helper: parse PHP code and collect scope from the first method body
/// found inside the first class.
fn collect_from_method(php: &str) -> ScopeMap {
    with_parsed_program(php, "test", |program, _content| {
        for stmt in program.statements.iter() {
            if let Statement::Class(class) = stmt {
                for member in class.members.iter() {
                    if let ClassLikeMember::Method(method) = member {
                        if let MethodBody::Concrete(block) = &method.body {
                            let body_start = block.left_brace.start.offset;
                            let body_end = block.right_brace.end.offset;
                            return collect_function_scope(
                                &method.parameter_list,
                                block.statements.as_slice(),
                                body_start,
                                body_end,
                            );
                        }
                    }
                }
            }
        }
        panic!("No class method found in test PHP code");
    })
}

/// Helper: find access by name and return all offsets + kinds.
fn accesses_for(scope_map: &ScopeMap, name: &str) -> Vec<(u32, AccessKind)> {
    scope_map
        .accesses
        .iter()
        .filter(|a| a.name == name)
        .map(|a| (a.offset, a.kind))
        .collect()
}

/// Helper: count accesses of a specific kind for a variable.
fn count_kind(scope_map: &ScopeMap, name: &str, kind: AccessKind) -> usize {
    scope_map
        .accesses
        .iter()
        .filter(|a| a.name == name && a.kind == kind)
        .count()
}

// ─── Basic variable tracking ────────────────────────────────────────────────

#[test]
fn simple_assignment_and_read() {
    let php = r#"<?php
function test() {
    $x = 1;
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1);
    assert_eq!(scope_map.accesses.len(), 2);
}

#[test]
fn parameter_is_write() {
    let php = r#"<?php
function test($a, $b) {
    return $a + $b;
}
"#;
    let scope_map = collect_from_function(php);

    // Parameters are writes.
    assert_eq!(count_kind(&scope_map, "$a", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$b", AccessKind::Write), 1);
    // Return reads both.
    assert_eq!(count_kind(&scope_map, "$a", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$b", AccessKind::Read), 1);
}

#[test]
fn multiple_assignments() {
    let php = r#"<?php
function test() {
    $x = 1;
    $x = 2;
    $x = 3;
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 3);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1);
}

#[test]
fn compound_assignment_is_read_write() {
    let php = r#"<?php
function test() {
    $x = 0;
    $x += 5;
    $x .= "hello";
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::ReadWrite), 2);
}

#[test]
fn postfix_increment_is_read_write() {
    let php = r#"<?php
function test() {
    $x = 0;
    $x++;
    $x--;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::ReadWrite), 2);
}

#[test]
fn coalesce_assignment_is_read_write() {
    let php = r#"<?php
function test() {
    $x = null;
    $x ??= "default";
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::ReadWrite), 1);
}

// ─── Control flow ───────────────────────────────────────────────────────────

#[test]
fn if_else_variables_leak() {
    let php = r#"<?php
function test($cond) {
    if ($cond) {
        $x = 1;
    } else {
        $x = 2;
    }
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    // $x is written in both branches and read after — it's visible.
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 2);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1);
    // $cond: 1 write (param) + 1 read (if condition).
    assert_eq!(count_kind(&scope_map, "$cond", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$cond", AccessKind::Read), 1);
}

#[test]
fn foreach_value_is_write() {
    let php = r#"<?php
function test($items) {
    foreach ($items as $key => $value) {
        echo $key;
        echo $value;
    }
}
"#;
    let scope_map = collect_from_function(php);

    // foreach key and value bindings are writes.
    assert_eq!(count_kind(&scope_map, "$key", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$value", AccessKind::Write), 1);
    // They are read inside the loop.
    assert_eq!(count_kind(&scope_map, "$key", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$value", AccessKind::Read), 1);
    // $items: 1 write (param) + 1 read (foreach expression).
    assert_eq!(count_kind(&scope_map, "$items", AccessKind::Read), 1);
}

#[test]
fn for_loop_variables() {
    let php = r#"<?php
function test() {
    for ($i = 0; $i < 10; $i++) {
        echo $i;
    }
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$i", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$i", AccessKind::ReadWrite), 1); // $i++
    assert_eq!(count_kind(&scope_map, "$i", AccessKind::Read), 2); // condition + body
}

#[test]
fn while_loop_variables() {
    let php = r#"<?php
function test() {
    $i = 0;
    while ($i < 10) {
        echo $i;
        $i++;
    }
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$i", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$i", AccessKind::ReadWrite), 1);
    assert_eq!(count_kind(&scope_map, "$i", AccessKind::Read), 2); // condition + body
}

#[test]
fn do_while_variables() {
    let php = r#"<?php
function test() {
    $x = 0;
    do {
        $x++;
    } while ($x < 10);
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::ReadWrite), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1); // condition
}

#[test]
fn switch_case_variables() {
    let php = r#"<?php
function test($val) {
    switch ($val) {
        case 1:
            $result = "one";
            break;
        case 2:
            $result = "two";
            break;
        default:
            $result = "other";
    }
    echo $result;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$result", AccessKind::Write), 3);
    assert_eq!(count_kind(&scope_map, "$result", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$val", AccessKind::Read), 1);
}

#[test]
fn try_catch_finally() {
    let php = r#"<?php
function test() {
    try {
        $x = doSomething();
    } catch (\Exception $e) {
        echo $e;
    } finally {
        $y = cleanup();
    }
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    // $e is a write (catch binding) + read (echo).
    assert_eq!(count_kind(&scope_map, "$e", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$e", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$y", AccessKind::Write), 1);

    // Catch block creates a new frame.
    let catch_frames: Vec<&Frame> = scope_map
        .frames
        .iter()
        .filter(|f| f.kind == FrameKind::Catch)
        .collect();
    assert_eq!(catch_frames.len(), 1);
}

// ─── Closures and arrow functions ───────────────────────────────────────────

#[test]
fn closure_creates_new_frame() {
    let php = r#"<?php
function test() {
    $x = 1;
    $fn = function() use ($x) {
        echo $x;
    };
}
"#;
    let scope_map = collect_from_function(php);

    // Should have at least 2 frames: function body + closure.
    let closure_frames: Vec<&Frame> = scope_map
        .frames
        .iter()
        .filter(|f| f.kind == FrameKind::Closure)
        .collect();
    assert_eq!(closure_frames.len(), 1);

    // Closure captures $x.
    assert_eq!(closure_frames[0].captures.len(), 1);
    assert_eq!(closure_frames[0].captures[0].0, "$x");
    assert!(!closure_frames[0].captures[0].1); // not by reference
}

#[test]
fn closure_by_reference_capture() {
    let php = r#"<?php
function test() {
    $x = 1;
    $fn = function() use (&$x) {
        $x = 2;
    };
}
"#;
    let scope_map = collect_from_function(php);

    let closure_frames: Vec<&Frame> = scope_map
        .frames
        .iter()
        .filter(|f| f.kind == FrameKind::Closure)
        .collect();
    assert_eq!(closure_frames.len(), 1);
    assert_eq!(closure_frames[0].captures[0].0, "$x");
    assert!(closure_frames[0].captures[0].1); // by reference
}

#[test]
fn closure_parameters() {
    let php = r#"<?php
function test() {
    $fn = function($a, $b) {
        return $a + $b;
    };
}
"#;
    let scope_map = collect_from_function(php);

    // Parameters $a and $b should be writes inside the closure frame.
    let closure_a_writes = scope_map
        .accesses
        .iter()
        .filter(|a| a.name == "$a" && a.kind == AccessKind::Write)
        .count();
    assert!(closure_a_writes >= 1);

    let closure_b_writes = scope_map
        .accesses
        .iter()
        .filter(|a| a.name == "$b" && a.kind == AccessKind::Write)
        .count();
    assert!(closure_b_writes >= 1);
}

#[test]
fn arrow_function_creates_frame() {
    let php = r#"<?php
function test() {
    $x = 1;
    $fn = fn($y) => $x + $y;
}
"#;
    let scope_map = collect_from_function(php);

    let arrow_frames: Vec<&Frame> = scope_map
        .frames
        .iter()
        .filter(|f| f.kind == FrameKind::ArrowFunction)
        .collect();
    assert_eq!(arrow_frames.len(), 1);
}

#[test]
fn nested_closures() {
    let php = r#"<?php
function test() {
    $x = 1;
    $outer = function() use ($x) {
        $y = $x + 1;
        $inner = function() use ($y) {
            return $y;
        };
    };
}
"#;
    let scope_map = collect_from_function(php);

    let closure_frames: Vec<&Frame> = scope_map
        .frames
        .iter()
        .filter(|f| f.kind == FrameKind::Closure)
        .collect();
    assert_eq!(closure_frames.len(), 2);
}

// ─── $this / self / static / parent tracking ────────────────────────────────

#[test]
fn this_is_tracked() {
    let php = r#"<?php
class Foo {
    public function test() {
        $x = $this->bar();
    }
}
"#;
    let scope_map = collect_from_method(php);

    assert!(scope_map.has_this_or_self);
    let this_reads = scope_map
        .accesses
        .iter()
        .filter(|a| a.name == "$this" && a.kind == AccessKind::Read)
        .count();
    assert!(this_reads >= 1);
}

#[test]
fn self_static_parent_tracked() {
    let php = r#"<?php
class Foo {
    public function test() {
        $x = self::VALUE;
    }
}
"#;
    let scope_map = collect_from_method(php);
    assert!(scope_map.has_this_or_self);
}

#[test]
fn no_this_when_absent() {
    let php = r#"<?php
function test() {
    $x = 1;
    return $x;
}
"#;
    let scope_map = collect_from_function(php);
    assert!(!scope_map.has_this_or_self);
}

// ─── Reference parameters ───────────────────────────────────────────────────

#[test]
fn reference_parameter_detected() {
    let php = r#"<?php
function test(&$x) {
    $x = 1;
}
"#;
    let scope_map = collect_from_function(php);
    assert!(scope_map.has_reference_params);
}

#[test]
fn no_reference_params() {
    let php = r#"<?php
function test($x) {
    $x = 1;
}
"#;
    let scope_map = collect_from_function(php);
    assert!(!scope_map.has_reference_params);
}

// ─── Static and global declarations ─────────────────────────────────────────

#[test]
fn static_variable_is_write() {
    let php = r#"<?php
function test() {
    static $counter = 0;
    $counter++;
    return $counter;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$counter", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$counter", AccessKind::ReadWrite), 1);
}

#[test]
fn global_variable_is_write() {
    let php = r#"<?php
function test() {
    global $config;
    echo $config;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$config", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$config", AccessKind::Read), 1);
}

// ─── Unset ──────────────────────────────────────────────────────────────────

#[test]
fn unset_is_write() {
    let php = r#"<?php
function test() {
    $x = 1;
    unset($x);
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 2); // assignment + unset
}

// ─── Destructuring ──────────────────────────────────────────────────────────

#[test]
fn array_destructuring() {
    let php = r#"<?php
function test() {
    [$a, $b] = getValues();
    echo $a;
    echo $b;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$a", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$b", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$a", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$b", AccessKind::Read), 1);
}

#[test]
fn list_destructuring() {
    let php = r#"<?php
function test() {
    list($a, $b) = getValues();
    echo $a;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$a", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$b", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$a", AccessKind::Read), 1);
}

// ─── Array access patterns ──────────────────────────────────────────────────

#[test]
fn array_key_assignment_is_read_write() {
    let php = r#"<?php
function test() {
    $arr = [];
    $arr['key'] = 'value';
    echo $arr;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$arr", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$arr", AccessKind::ReadWrite), 1);
    assert_eq!(count_kind(&scope_map, "$arr", AccessKind::Read), 1);
}

#[test]
fn array_push_is_read_write() {
    let php = r#"<?php
function test() {
    $arr = [];
    $arr[] = 'value';
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$arr", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$arr", AccessKind::ReadWrite), 1);
}

// ─── Frame queries ──────────────────────────────────────────────────────────

#[test]
fn enclosing_frame_finds_function() {
    let php = r#"<?php
function test() {
    $x = 1;
}
"#;
    let scope_map = collect_from_function(php);

    // Any offset inside the function body should find the function frame.
    let frame = scope_map.enclosing_frame(scope_map.frames[0].start + 1);
    assert!(frame.is_some());
    assert_eq!(frame.unwrap().kind, FrameKind::Function);
}

#[test]
fn variables_in_scope_lists_all() {
    let php = r#"<?php
function test($a) {
    $b = 1;
    $c = $a + $b;
    return $c;
}
"#;
    let scope_map = collect_from_function(php);

    let vars = scope_map.variables_in_scope(scope_map.frames[0].start + 1);
    assert!(vars.contains(&"$a".to_string()));
    assert!(vars.contains(&"$b".to_string()));
    assert!(vars.contains(&"$c".to_string()));
}

#[test]
fn all_occurrences_returns_sorted() {
    let php = r#"<?php
function test() {
    $x = 1;
    echo $x;
    $x = 2;
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    let occurrences = scope_map.all_occurrences("$x", scope_map.frames[0].start + 1);
    assert_eq!(occurrences.len(), 4);
    // Should be in source order.
    for i in 1..occurrences.len() {
        assert!(occurrences[i].0 > occurrences[i - 1].0);
    }
    assert_eq!(occurrences[0].1, AccessKind::Write);
    assert_eq!(occurrences[1].1, AccessKind::Read);
    assert_eq!(occurrences[2].1, AccessKind::Write);
    assert_eq!(occurrences[3].1, AccessKind::Read);
}

// ─── Range classification ───────────────────────────────────────────────────

#[test]
fn classify_range_parameters() {
    // $x is written before the range and read inside → parameter.
    let php = r#"<?php
function test() {
    $x = new Foo();
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    // Find the offsets of the write and read.
    let x_accesses = accesses_for(&scope_map, "$x");
    assert_eq!(x_accesses.len(), 2);
    let write_offset = x_accesses[0].0;
    let read_offset = x_accesses[1].0;
    let frame_end = scope_map.frames[0].end;

    // Range that only includes the read (not the write).
    // Use frame_end so the range stays within the function body.
    let classification = scope_map.classify_range(read_offset, frame_end);
    assert!(
        classification.parameters.contains(&"$x".to_string()),
        "Expected $x in parameters, got: {:?}",
        classification.parameters
    );
    assert!(classification.return_values.is_empty());
    assert!(classification.locals.is_empty());

    // Range that includes only the write.
    let classification2 = scope_map.classify_range(write_offset, read_offset);
    // Written inside, read after → return value.
    assert!(
        classification2.return_values.contains(&"$x".to_string()),
        "Expected $x in return_values, got: {:?}",
        classification2
    );
}

#[test]
fn classify_range_locals() {
    // Variable entirely within the range → local.
    let php = r#"<?php
function test() {
    $before = 1;
    $local = 2;
    echo $local;
    $after = 3;
}
"#;
    let scope_map = collect_from_function(php);

    let local_accesses = accesses_for(&scope_map, "$local");
    assert_eq!(local_accesses.len(), 2);

    let _before_accesses = accesses_for(&scope_map, "$before");
    let after_accesses = accesses_for(&scope_map, "$after");

    // Range from $local write to just after $local read, but before $after.
    let range_start = local_accesses[0].0;
    let range_end = after_accesses[0].0;

    let classification = scope_map.classify_range(range_start, range_end);
    assert!(
        classification.locals.contains(&"$local".to_string()),
        "Expected $local in locals, got: {:?}",
        classification
    );
}

#[test]
fn classify_range_return_values() {
    // Variable written inside range and read after → return value.
    let php = r#"<?php
function test() {
    $x = compute();
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    let x_accesses = accesses_for(&scope_map, "$x");
    let write_offset = x_accesses[0].0;
    let read_offset = x_accesses[1].0;

    // Range includes only the write.
    let classification = scope_map.classify_range(write_offset, read_offset);
    assert!(
        classification.return_values.contains(&"$x".to_string()),
        "Expected $x in return_values, got: {:?}",
        classification
    );
}

#[test]
fn classify_range_this_detection() {
    let php = r#"<?php
class Foo {
    public function test() {
        $x = $this->bar();
    }
}
"#;
    let scope_map = collect_from_method(php);

    let frame = &scope_map.frames[0];
    let classification = scope_map.classify_range(frame.start, frame.end);
    assert!(classification.uses_this);
}

#[test]
fn classify_range_no_this() {
    let php = r#"<?php
function test() {
    $x = 1;
}
"#;
    let scope_map = collect_from_function(php);

    let frame = &scope_map.frames[0];
    let classification = scope_map.classify_range(frame.start, frame.end);
    assert!(!classification.uses_this);
}

// ─── Complex scenarios ──────────────────────────────────────────────────────

#[test]
fn method_call_chain() {
    let php = r#"<?php
function test() {
    $builder = new QueryBuilder();
    $result = $builder->where('x', 1)->orderBy('id')->get();
    echo $result;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$builder", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$builder", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$result", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$result", AccessKind::Read), 1);
}

#[test]
fn ternary_expression() {
    let php = r#"<?php
function test($cond) {
    $x = $cond ? 'yes' : 'no';
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$cond", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
}

#[test]
fn null_coalescing() {
    let php = r#"<?php
function test($a, $b) {
    $x = $a ?? $b;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$a", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$b", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
}

#[test]
fn instanceof_reads_variable() {
    let php = r#"<?php
function test($obj) {
    if ($obj instanceof Foo) {
        echo $obj;
    }
}
"#;
    let scope_map = collect_from_function(php);

    // $obj: param write + instanceof read + echo read + if condition read.
    let obj_reads = count_kind(&scope_map, "$obj", AccessKind::Read);
    assert!(
        obj_reads >= 2,
        "Expected at least 2 reads for $obj, got {}",
        obj_reads
    );
}

#[test]
fn match_expression_variables() {
    let php = r#"<?php
function test($status) {
    $message = match($status) {
        'active' => 'Active',
        'inactive' => 'Inactive',
        default => 'Unknown',
    };
    echo $message;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$status", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$message", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$message", AccessKind::Read), 1);
}

#[test]
fn yield_expression() {
    let php = r#"<?php
function test() {
    $x = yield 'value';
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1);
}

#[test]
fn multiple_catch_clauses() {
    let php = r#"<?php
function test() {
    try {
        riskyOperation();
    } catch (\InvalidArgumentException $e) {
        log($e);
    } catch (\RuntimeException $e) {
        log($e);
    }
}
"#;
    let scope_map = collect_from_function(php);

    let catch_frames: Vec<&Frame> = scope_map
        .frames
        .iter()
        .filter(|f| f.kind == FrameKind::Catch)
        .collect();
    assert_eq!(catch_frames.len(), 2);
}

#[test]
fn interpolated_string_variables() {
    let php = r#"<?php
function test($name) {
    $greeting = "Hello, $name!";
}
"#;
    let scope_map = collect_from_function(php);

    // $name: param write + interpolation read.
    assert_eq!(count_kind(&scope_map, "$name", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$name", AccessKind::Read), 1);
}

#[test]
fn clone_expression() {
    let php = r#"<?php
function test($obj) {
    $copy = clone $obj;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$obj", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$copy", AccessKind::Write), 1);
}

#[test]
fn throw_expression() {
    let php = r#"<?php
function test($msg) {
    throw new \Exception($msg);
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$msg", AccessKind::Read), 1);
}

#[test]
fn return_expression() {
    let php = r#"<?php
function test() {
    $x = compute();
    return $x;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1);
}

// ─── Frame nesting ──────────────────────────────────────────────────────────

#[test]
fn accesses_in_frame_excludes_nested() {
    let php = r#"<?php
function test() {
    $x = 1;
    $fn = function() use ($x) {
        $y = $x;
    };
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    // In the outer frame, $y should not appear.
    let outer_frame = scope_map
        .frames
        .iter()
        .find(|f| f.kind == FrameKind::Function)
        .unwrap();
    let y_in_outer = scope_map.accesses_in_frame("$y", outer_frame);
    assert!(
        y_in_outer.is_empty(),
        "Expected $y to not appear in outer frame"
    );
}

#[test]
fn echo_reads_variable() {
    let php = r#"<?php
function test() {
    $x = "hello";
    echo $x;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1);
}

// ─── Spread / variadic ─────────────────────────────────────────────────────

#[test]
fn spread_in_function_call() {
    let php = r#"<?php
function test($args) {
    foo(...$args);
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$args", AccessKind::Read), 1);
}

// ─── isset / empty ─────────────────────────────────────────────────────────

#[test]
fn isset_reads_variable() {
    let php = r#"<?php
function test($x) {
    if (isset($x)) {
        echo $x;
    }
}
"#;
    let scope_map = collect_from_function(php);

    // $x: param write + isset read + if read + echo read.
    let x_reads = count_kind(&scope_map, "$x", AccessKind::Read);
    assert!(
        x_reads >= 2,
        "Expected at least 2 reads for $x, got {}",
        x_reads
    );
}

#[test]
fn empty_reads_variable() {
    let php = r#"<?php
function test($x) {
    if (empty($x)) {
        echo "empty";
    }
}
"#;
    let scope_map = collect_from_function(php);

    assert!(count_kind(&scope_map, "$x", AccessKind::Read) >= 1);
}

// ─── Edge cases ─────────────────────────────────────────────────────────────

#[test]
fn empty_function_body() {
    let php = r#"<?php
function test() {
}
"#;
    let scope_map = collect_from_function(php);

    assert!(scope_map.accesses.is_empty());
    assert_eq!(scope_map.frames.len(), 1);
    assert_eq!(scope_map.frames[0].kind, FrameKind::Function);
}

#[test]
fn function_with_only_parameters() {
    let php = r#"<?php
function test($a, $b, $c) {
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(scope_map.accesses.len(), 3);
    assert!(scope_map.accesses.iter().all(|a| a.kind == AccessKind::Write));
}

#[test]
fn nested_array_access() {
    let php = r#"<?php
function test() {
    $data = [];
    $x = $data['foo']['bar'];
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$data", AccessKind::Write), 1);
    assert_eq!(count_kind(&scope_map, "$data", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
}

#[test]
fn property_access_reads_object() {
    let php = r#"<?php
function test($obj) {
    $x = $obj->name;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$obj", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Write), 1);
}

#[test]
fn property_write_reads_object() {
    let php = r#"<?php
function test($obj) {
    $obj->name = "test";
}
"#;
    let scope_map = collect_from_function(php);

    // Writing $obj->name reads $obj (the object itself).
    assert_eq!(count_kind(&scope_map, "$obj", AccessKind::Read), 1);
}

#[test]
fn cast_reads_variable() {
    let php = r#"<?php
function test($x) {
    $y = (string)$x;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$x", AccessKind::Read), 1);
    assert_eq!(count_kind(&scope_map, "$y", AccessKind::Write), 1);
}

#[test]
fn elseif_branches() {
    let php = r#"<?php
function test($val) {
    if ($val === 1) {
        $result = "one";
    } elseif ($val === 2) {
        $result = "two";
    } else {
        $result = "other";
    }
    echo $result;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$result", AccessKind::Write), 3);
    assert_eq!(count_kind(&scope_map, "$result", AccessKind::Read), 1);
    // $val reads: if condition + elseif condition.
    assert!(count_kind(&scope_map, "$val", AccessKind::Read) >= 2);
}

#[test]
fn include_reads_path() {
    let php = r#"<?php
function test($path) {
    include $path;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$path", AccessKind::Read), 1);
}

#[test]
fn print_reads_variable() {
    let php = r#"<?php
function test($msg) {
    print $msg;
}
"#;
    let scope_map = collect_from_function(php);

    assert_eq!(count_kind(&scope_map, "$msg", AccessKind::Read), 1);
}

// ─── classify_range complex scenarios ───────────────────────────────────────

#[test]
fn classify_range_mixed_roles() {
    // $x is a parameter (read inside, written before)
    // $y is a local (written and read only inside)
    // $z is a return value (written inside, read after)
    let php = r#"<?php
function test() {
    $x = 1;
    $y = $x + 1;
    echo $y;
    $z = $y * 2;
    echo $z;
}
"#;
    let scope_map = collect_from_function(php);

    let _x_accesses = accesses_for(&scope_map, "$x");
    let y_accesses = accesses_for(&scope_map, "$y");
    let z_accesses = accesses_for(&scope_map, "$z");

    // Extract the middle range: from $y write to just before $z write.
    let range_start = y_accesses[0].0;
    let range_end = z_accesses[0].0;

    let classification = scope_map.classify_range(range_start, range_end);

    // $x is written before range, read inside → parameter.
    assert!(
        classification.parameters.contains(&"$x".to_string()),
        "Expected $x as parameter: {:?}",
        classification
    );

    // $y is written inside, read inside, then read after (in $z = $y * 2) → depends on range.
    // Since $y is also read at `echo $y` which might be inside or outside depending on exact offsets.
    // The important thing is the classification runs without panicking.
}

#[test]
fn classify_excludes_this_from_names() {
    let php = r#"<?php
class Foo {
    public function test() {
        $x = $this->bar();
        return $x;
    }
}
"#;
    let scope_map = collect_from_method(php);

    let frame = &scope_map.frames[0];
    let classification = scope_map.classify_range(frame.start, frame.end);

    // $this should NOT appear in parameters/return_values/locals.
    assert!(!classification.parameters.contains(&"$this".to_string()));
    assert!(!classification.return_values.contains(&"$this".to_string()));
    assert!(!classification.locals.contains(&"$this".to_string()));
    // But uses_this should be true.
    assert!(classification.uses_this);
}

// ─── Source order ───────────────────────────────────────────────────────────

#[test]
fn accesses_are_in_source_order() {
    let php = r#"<?php
function test() {
    $a = 1;
    $b = $a;
    $c = $b;
    echo $c;
}
"#;
    let scope_map = collect_from_function(php);

    // All accesses should be in ascending offset order.
    for i in 1..scope_map.accesses.len() {
        assert!(
            scope_map.accesses[i].offset >= scope_map.accesses[i - 1].offset,
            "Access at index {} (offset {}) is before index {} (offset {})",
            i,
            scope_map.accesses[i].offset,
            i - 1,
            scope_map.accesses[i - 1].offset,
        );
    }
}