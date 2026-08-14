//! Integration tests for the shape `preg_match` leaves in `$matches`.
//!
//! A literal pattern says which keys the out-parameter has, so a group read
//! off it inside the guard resolves to `string` instead of the `mixed` a bare
//! `array` yields.

use crate::common::create_test_backend_with_full_stubs;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

/// The type shown for the variable assigned on the line whose trimmed text
/// starts with `$name = `, read off the hover response.
fn assigned_type(backend: &Backend, uri: &str, content: &str, name: &str) -> String {
    let needle = format!("{name} = ");
    let line = content
        .lines()
        .position(|l| l.trim_start().starts_with(&needle))
        .unwrap_or_else(|| panic!("no assignment to {name} in the fixture")) as u32;
    let character = content
        .lines()
        .nth(line as usize)
        .unwrap()
        .find(name)
        .unwrap() as u32
        + 1;
    backend.update_ast(uri, content);
    let hover = backend
        .handle_hover(
            uri,
            content,
            Position {
                line,
                character: character + 1,
            },
        )
        .unwrap_or_else(|| panic!("no hover on {name}"));
    let HoverContents::Markup(markup) = &hover.contents else {
        panic!("expected MarkupContent");
    };
    markup
        .value
        .lines()
        .find_map(|l| l.split_once(" = ").map(|(_, ty)| ty.trim().to_string()))
        .unwrap_or_else(|| panic!("no assignment in hover on {name}: {}", markup.value))
}

/// Assert the type of each assignment in `content`, keyed by the variable it
/// assigns to.
fn assert_assigned_types(content: &str, expected: &[(&str, &str)]) {
    let backend = create_test_backend_with_full_stubs();
    let uri = "file:///preg_match_shapes.php";
    for (var, want) in expected {
        assert_eq!(&assigned_type(&backend, uri, content, var), want, "{var}");
    }
}

/// A named group contributes its name as a key, so reading it needs no cast
/// and no `??` — and the numbered key it also gets resolves the same.
#[test]
fn named_groups_are_readable_by_name_and_by_number() {
    let content = r#"<?php
function probe(string $size): void {
    if (preg_match('/(?<amount>\d+)(?<unit>\w*)/', $size, $match)) {
        $unit = $match['unit'];
        $amount = $match['amount'];
        $byNumber = $match[2];
        $whole = $match[0];
    }
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$unit", "string"),
            ("$amount", "string"),
            ("$byNumber", "string"),
            ("$whole", "string"),
        ],
    );
}

/// The shape itself, as hover reports it for the out-parameter.
#[test]
fn the_out_parameter_carries_the_group_shape() {
    let content = r#"<?php
function probe(string $s): void {
    preg_match('/(\d+)-(?<name>\w+)/', $s, $literal);
    $shape = $literal;
}
"#;
    assert_assigned_types(
        content,
        &[(
            "$shape",
            "array{0: string, 1: string, name: string, 2: string}",
        )],
    );
}

/// `preg_match_all` collects every match of a group, so a group read is a
/// list of strings rather than one.
#[test]
fn match_all_group_reads_are_lists() {
    let content = r#"<?php
function probe(string $s): void {
    preg_match_all('/(\d+)/', $s, $matches);
    $group = $matches[1];
    $first = $matches[1][0];
}
"#;
    assert_assigned_types(content, &[("$group", "list<string>"), ("$first", "string")]);
}

/// `PREG_SET_ORDER` inverts the nesting: one entry per match, each holding
/// that match's groups.
#[test]
fn match_all_in_set_order_yields_one_shape_per_match() {
    let content = r#"<?php
function probe(string $s): void {
    preg_match_all('/(\d+)/', $s, $matches, PREG_SET_ORDER);
    $set = $matches[0];
    $group = $matches[0][1];
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$set", "array{0: string, 1: string}"),
            ("$group", "string"),
        ],
    );
}

/// `PREG_OFFSET_CAPTURE` pairs every entry with the position it matched at.
#[test]
fn offset_capture_pairs_each_group_with_its_position() {
    let content = r#"<?php
function probe(string $s): void {
    preg_match('/(\d+)/', $s, $matches, PREG_OFFSET_CAPTURE);
    $group = $matches[1];
    $text = $matches[1][0];
    $offset = $matches[1][1];
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$group", "array{string, int<-1, max>}"),
            ("$text", "string"),
            ("$offset", "int<-1, max>"),
        ],
    );
}

/// A pattern that is not a literal, and one whose groups cannot be counted,
/// still type their entries: every entry of a `preg_match` result is a
/// string, whatever the keys turn out to be.
#[test]
fn an_unreadable_pattern_still_types_the_entries() {
    let content = r#"<?php
function probe(string $s, string $pattern): void {
    preg_match($pattern, $s, $dynamic);
    $fromDynamic = $dynamic['whatever'];
    preg_match('/(?|(a)|(b))/', $s, $branchReset);
    $fromBranchReset = $branchReset[1];
}
"#;
    assert_assigned_types(
        content,
        &[("$fromDynamic", "string"), ("$fromBranchReset", "string")],
    );
}

/// A `$flags` argument that cannot be read falls back to the parameter's own
/// declared type rather than assuming the default flags:
/// `PREG_OFFSET_CAPTURE` would change every entry from a string to a pair.
#[test]
fn unreadable_flags_fall_back_to_the_declared_parameter_type() {
    let content = r#"<?php
function probe(string $s, int $flags): void {
    preg_match('/(\d+)/', $s, $matches, $flags);
    $shape = $matches;
}
"#;
    assert_assigned_types(content, &[("$shape", "array<string>")]);
}

/// A flag the analysis does not model is the same case:
/// `PREG_SPLIT_OFFSET_CAPTURE` belongs to `preg_split`, and a call that
/// passes it is not one whose result this analysis knows the shape of.
#[test]
fn unmodelled_flags_fall_back_to_the_declared_parameter_type() {
    let content = r#"<?php
function probe(string $s): void {
    preg_match('/(\d+)/', $s, $matches, PREG_SPLIT_OFFSET_CAPTURE);
    $shape = $matches;
}
"#;
    assert_assigned_types(content, &[("$shape", "array<string>")]);
}
