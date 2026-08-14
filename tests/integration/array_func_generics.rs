//! Integration tests for the array builtins that answer in terms of their
//! input's generics.
//!
//! phpstorm-stubs declare these as returning a bare `array`, an `int[]|string[]`
//! union, or `string|int|false`, because a signature without generics cannot
//! say "the key type of whatever you passed". Each test here pins one function
//! to the type its argument implies, and checks that an argument with nothing
//! to say (a bare `array`) still gets the declared type.
//!
//! The last test covers the binding machinery the rest of them rest on: which
//! alternative of a union `@param` a `@template` binds from.

use crate::common::create_test_backend_with_full_stubs;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

/// The resolved type of the variable assigned on `line` (0-based), read off
/// the hover response.
fn assigned_type(backend: &Backend, uri: &str, content: &str, line: u32) -> String {
    backend.update_ast(uri, content);
    let hover = backend
        .handle_hover(uri, content, Position { line, character: 6 })
        .unwrap_or_else(|| panic!("no hover on line {line}"));
    let HoverContents::Markup(markup) = &hover.contents else {
        panic!("Expected MarkupContent");
    };
    markup
        .value
        .lines()
        .find_map(|l| l.split_once(" = ").map(|(_, ty)| ty.trim().to_string()))
        .unwrap_or_else(|| panic!("no assignment in hover on line {line}: {}", markup.value))
}

/// Assert the type of each assignment in `content`, keyed by the variable it
/// assigns to. Line numbers are found by scanning for `$name = `.
fn assert_assigned_types(content: &str, expected: &[(&str, &str)]) {
    let backend = create_test_backend_with_full_stubs();
    let uri = "file:///array_func_generics.php";
    for (var, want) in expected {
        let needle = format!("{var} = ");
        let line = content
            .lines()
            .position(|l| l.trim_start().starts_with(&needle))
            .unwrap_or_else(|| panic!("no assignment to {var} in the fixture"))
            as u32;
        let got = assigned_type(&backend, uri, content, line);
        assert_eq!(&got, want, "{var}");
    }
}

/// The key-reading builtins report the input's *key* type. The stubs spell
/// these out as `int[]|string[]` and `string|int|null`, which then fails
/// against a declared `array<string>` on the wrong branch.
#[test]
fn key_readers_report_the_input_key_type() {
    let content = r#"<?php
class User {}
/**
 * @param array<string, User> $byName
 * @param list<User> $users
 */
function probe(array $byName, array $users, array $bare): void {
    $names = array_keys($byName);
    $indices = array_keys($users);
    $first = array_key_first($byName);
    $last = array_key_last($byName);
    $cursor = key($byName);
    $unknown = array_keys($bare);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$names", "list<string>"),
            ("$indices", "list<int>"),
            ("$first", "string|null"),
            ("$last", "string|null"),
            ("$cursor", "string|null"),
            // A bare `array` says nothing about its keys beyond PHP's own
            // rule that a key is an `array-key`.
            ("$unknown", "list<array-key>"),
        ],
    );
}

/// `array_search` hands back a *key*, so the `int` half of the stub's
/// `string|int|false` is impossible for a string-keyed array.
#[test]
fn array_search_reports_the_input_key_type() {
    let content = r#"<?php
/**
 * @param array<string, int> $byName
 * @param list<string> $names
 */
function probe(array $byName, array $names): void {
    $key = array_search(1, $byName);
    $index = array_search('a', $names);
}
"#;
    assert_assigned_types(
        content,
        &[("$key", "string|false"), ("$index", "int|false")],
    );
}

/// `array_values` renumbers, so it keeps the element type and drops the key
/// type — it is a `list<V>`, never the `array<K, V>` it was handed.
#[test]
fn array_values_renumbers_to_a_list() {
    let content = r#"<?php
class User {}
/**
 * @param array<string, User> $byName
 * @param array<string, int> $counts
 */
function probe(array $byName, array $counts): void {
    $users = array_values($byName);
    $numbers = array_values($counts);
}
"#;
    assert_assigned_types(
        content,
        &[("$users", "list<User>"), ("$numbers", "list<int>")],
    );
}

/// The type-preserving family keeps its element type whether or not that
/// element is a scalar. A `list<string>` is as worth preserving as a
/// `list<User>`; both used to fall back to a bare `array`.
#[test]
fn preserving_builtins_keep_scalar_elements() {
    let content = r#"<?php
class User {}
/**
 * @param list<string> $names
 * @param list<User> $users
 */
function probe(array $names, array $users, array $bare): void {
    $unique = array_unique($names);
    $slice = array_slice($names, 0, 2);
    $merged = array_merge($names, $names);
    $reversed = array_reverse($names);
    $objects = array_reverse($users);
    $unknown = array_unique($bare);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$unique", "list<string>"),
            ("$slice", "list<string>"),
            ("$merged", "list<string>"),
            ("$reversed", "list<string>"),
            ("$objects", "list<User>"),
            ("$unknown", "array"),
        ],
    );
}

/// The element-extracting family has the same scalar blind spot:
/// `array_pop(list<string>)` is a `string`, not `mixed`.
#[test]
fn element_extractors_keep_scalar_elements() {
    let content = r#"<?php
class User {}
/**
 * @param list<string> $names
 * @param list<User> $users
 */
function probe(array $names, array $users): void {
    $popped = array_pop($names);
    $shifted = array_shift($names);
    $cursor = current($names);
    $object = array_pop($users);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$popped", "string"),
            ("$shifted", "string"),
            ("$cursor", "string"),
            ("$object", "User"),
        ],
    );
}

/// `array_filter` with no callback keeps exactly the truthy members, so the
/// element type drops `null`. With a callback the surviving members are
/// whatever it approves of, which says nothing about their type.
#[test]
fn array_filter_without_a_callback_drops_falsy_members() {
    let content = r#"<?php
class User {}
/**
 * @param array<string, string|null> $maybe
 * @param list<User|null> $users
 * @param list<string> $plain
 */
function probe(array $maybe, array $users, array $plain, callable $cb): void {
    $kept = array_filter($maybe);
    $present = array_filter($users);
    $chosen = array_filter($maybe, $cb);
    $unchanged = array_filter($plain);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$kept", "array<string, string>"),
            ("$present", "list<User>"),
            ("$chosen", "array<string, string|null>"),
            ("$unchanged", "list<string>"),
        ],
    );
}

/// `array_sum`/`array_product` are declared `int|float` for PHP's numeric
/// promotion, but an all-`int` array can only sum to an `int`. A union that
/// really can go either way keeps both.
#[test]
fn numeric_folds_narrow_on_an_all_int_array() {
    let content = r#"<?php
/**
 * @param array<int> $ints
 * @param list<float> $floats
 * @param list<int|float> $either
 * @param list<string> $strings
 */
function probe(array $ints, array $floats, array $either, array $strings, array $bare): void {
    $total = array_sum($ints);
    $product = array_product($ints);
    $money = array_sum($floats);
    $mixed = array_sum($either);
    $numeric = array_sum($strings);
    $unknown = array_sum($bare);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$total", "int"),
            ("$product", "int"),
            ("$money", "float"),
            ("$mixed", "int|float"),
            ("$numeric", "int|float"),
            ("$unknown", "int|float"),
        ],
    );
}

/// A `@template` named in more than one alternative of a union `@param`
/// binds from the alternative the argument's own shape matches, not from
/// whichever one happens to be written first.
#[test]
fn a_template_binds_through_a_union_param() {
    let content = r#"<?php
/**
 * @template TKey of array-key
 * @template TValue
 */
class Collection {}

/**
 * @template TKey of array-key
 * @template TValue
 * @param Collection<TKey, TValue>|array<TKey, TValue> $items
 * @return array<TKey, TValue>
 */
function pick($items): array { return []; }

/**
 * @param array<string, int> $rows
 * @param list<string> $names
 */
function probe(array $rows, array $names): void {
    $picked = pick($rows);
    $listed = pick($names);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$picked", "array<string, int>"),
            ("$listed", "array<int, string>"),
        ],
    );
}

/// `array_filter` in one of the two modes that hand the callback the key
/// keeps only the keys the callback approves of, so what its body asserts
/// about them describes the result's key type.
#[test]
fn array_filter_narrows_the_key_type_from_its_callback() {
    let content = r#"<?php
/**
 * @param array<string|int, string> $data
 */
function probe(array $data): void {
    $keyed = array_filter($data, fn (string|int $k): bool => is_string($k), ARRAY_FILTER_USE_KEY);
    $both = array_filter($data, fn (string $v, string|int $k): bool => is_string($k), ARRAY_FILTER_USE_BOTH);
    $closure = array_filter($data, function ($k) { return is_int($k); }, ARRAY_FILTER_USE_KEY);
    $negated = array_filter($data, fn ($k) => !is_int($k), ARRAY_FILTER_USE_KEY);
    $conjunction = array_filter($data, fn ($k) => is_string($k) && $k !== '', ARRAY_FILTER_USE_KEY);
    $named = array_filter($data, 'is_string', ARRAY_FILTER_USE_KEY);
    $inline = array_keys(array_filter($data, fn ($k) => is_string($k), ARRAY_FILTER_USE_KEY));
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$keyed", "array<string, string>"),
            ("$both", "array<string, string>"),
            ("$closure", "array<int, string>"),
            ("$negated", "array<string, string>"),
            ("$conjunction", "array<string, string>"),
            ("$named", "array<string, string>"),
            ("$inline", "list<string>"),
        ],
    );
}

/// A callback that proves nothing about the key it is handed leaves the
/// key type alone, and so does one whose key never reaches it.
#[test]
fn array_filter_keeps_the_key_type_a_callback_says_nothing_about() {
    let content = r#"<?php
/**
 * @param array<string|int, string> $data
 */
function probe(array $data): void {
    $plain = array_filter($data, fn (string $v): bool => $v !== '');
    $value_mode = array_filter($data, fn (string $v): bool => is_string($v));
    $unrelated = array_filter($data, fn ($k) => strlen((string) $k) > 2, ARRAY_FILTER_USE_KEY);
    $either = array_filter($data, fn ($v, $k) => is_int($k) || is_string($k), ARRAY_FILTER_USE_BOTH);
    $branching = array_filter($data, function ($k) { if ($k === 0) { return true; } return is_string($k); }, ARRAY_FILTER_USE_KEY);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$plain", "array<string|int, string>"),
            ("$value_mode", "array<string|int, string>"),
            ("$unrelated", "array<string|int, string>"),
            ("$either", "array<string|int, string>"),
            ("$branching", "array<string|int, string>"),
        ],
    );
}
