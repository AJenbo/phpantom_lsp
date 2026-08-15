//! What a condition *proves* about the values it names, and how that
//! proof reaches the scope.
//!
//! A successful `instanceof` filters the subject's union down to the
//! checked class rather than adding it beside the old members; a proof
//! about a `?->` chain's result is a proof about every receiver the chain
//! would have short-circuited on; a type guard is the only thing that
//! says what a value read from an unknown source is; and a null check on
//! an array element records the element that was checked, not the whole
//! array's value type.

use crate::common::create_test_backend;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

// ─── Helpers ────────────────────────────────────────────────────────────────

fn hover_at(backend: &Backend, uri: &str, content: &str, line: u32, character: u32) -> Hover {
    backend.update_ast(uri, content);
    backend
        .handle_hover(uri, content, Position { line, character })
        .expect("expected hover")
}

fn hover_text(hover: &Hover) -> &str {
    match &hover.contents {
        HoverContents::Markup(markup) => &markup.value,
        _ => panic!("Expected MarkupContent"),
    }
}

/// Hover on the variable that the marker line `// <-- here` points at.
///
/// Keeps the tests readable when scaffolding shifts: the assertion names
/// the line by its marker rather than by a literal line number.
fn hover_marked(backend: &Backend, uri: &str, content: &str) -> String {
    let line = content
        .lines()
        .position(|l| l.contains("// <-- here"))
        .expect("fixture should carry a `// <-- here` marker") as u32;
    let text = content.lines().nth(line as usize).unwrap();
    let column = text.find('$').expect("marked line should name a variable") as u32 + 1;
    hover_text(&hover_at(backend, uri, content, line, column)).to_string()
}

const SCAFFOLD: &str = r#"
class Configuration {}
class Node {}
class AbstractNode {
    public function getNode(): Node { return new Node(); }
}
class Container {
    /** @return object|null */
    public function get(string $id) { return null; }
}
class Image {
    public ?int $fileId = null;
    public static function first(): ?Image { return null; }
}
"#;

// ─── instanceof filters the union, it does not extend it ────────────────────

/// `assert($x instanceof C)` on an `object|null` subject leaves `C`, not
/// `object|null|C`: the check rules out everything the class does not
/// cover, including the null.
#[test]
fn assert_instanceof_filters_a_broad_union() {
    let backend = create_test_backend();
    let uri = "file:///assert_instanceof.php";
    let content = format!(
        r#"<?php
{SCAFFOLD}
function f(Container $c): void {{
    $obj = $c->get('config');
    assert($obj instanceof Configuration);
    $obj; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("Configuration"),
        "expected Configuration, got: {text}"
    );
    assert!(
        !text.contains("object") && !text.contains("null"),
        "the check rules out the rest of the union, got: {text}"
    );
}

/// The `if (!$x instanceof C) { throw; }` guard proves exactly what the
/// `assert()` above does, so its fall-through must narrow the same way.
#[test]
fn negated_instanceof_guard_filters_a_broad_union() {
    let backend = create_test_backend();
    let uri = "file:///guard_instanceof.php";
    let content = format!(
        r#"<?php
{SCAFFOLD}
function f(Container $c): void {{
    $obj = $c->get('config');
    if (!$obj instanceof Configuration) {{
        throw new \RuntimeException('missing');
    }}
    $obj; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("Configuration"),
        "expected Configuration, got: {text}"
    );
    assert!(
        !text.contains("object") && !text.contains("null"),
        "the guard rules out the rest of the union, got: {text}"
    );
}

// ─── A proof about a `?->` chain is a proof about its receivers ─────────────

/// `$image?->fileId !== null` can only hold when `$image` is not null:
/// otherwise the chain short-circuits to `null` and the check fails.
#[test]
fn nullsafe_non_null_check_narrows_the_receiver() {
    let backend = create_test_backend();
    let uri = "file:///nullsafe_check.php";
    let content = format!(
        r#"<?php
{SCAFFOLD}
function f(): void {{
    $image = Image::first();
    if ($image?->fileId !== null) {{
        $image; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Image"), "expected Image, got: {text}");
    assert!(
        !text.contains("null"),
        "the receiver cannot be null inside the branch, got: {text}"
    );
}

/// The same proof arrives through a guard clause: a falsy chain leaves
/// the function, so past it the receiver is non-null.
#[test]
fn nullsafe_truthy_guard_narrows_the_receiver() {
    let backend = create_test_backend();
    let uri = "file:///nullsafe_guard.php";
    let content = format!(
        r#"<?php
{SCAFFOLD}
function f(): void {{
    $image = Image::first();
    if (!$image?->fileId) {{
        return;
    }}
    $image; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Image"), "expected Image, got: {text}");
    assert!(
        !text.contains("null"),
        "the receiver cannot be null past the guard, got: {text}"
    );
}

/// The else branch gets no such proof — a null receiver is exactly one of
/// the ways the check fails, so the declared type must survive intact.
#[test]
fn nullsafe_check_leaves_the_else_branch_alone() {
    let backend = create_test_backend();
    let uri = "file:///nullsafe_else.php";
    let content = format!(
        r#"<?php
{SCAFFOLD}
function f(): void {{
    $image = Image::first();
    if ($image?->fileId !== null) {{
        return;
    }}
    $image; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("?Image") || text.contains("null"),
        "a failing check leaves null in play, got: {text}"
    );
}

// ─── A type guard on a value of unknown type establishes it ────────────────

/// `$row->version` on a bare `stdClass` resolves to nothing, and the
/// `assert(is_string(...))` is then the only statement that says what the
/// value is.  Skipping the guard for want of a prior type throws away the
/// one piece of information available.
#[test]
fn type_guard_types_a_value_read_from_an_unknown_source() {
    let backend = create_test_backend();
    let uri = "file:///guard_unknown.php";
    let content = r#"<?php
/** @param list<stdClass> $rows */
function f(array $rows): void {
    foreach ($rows as $row) {
        $version = $row->version;
        assert(is_string($version));
        $version; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
}

/// The fall-through of a guard carries the same proof, so an unknown
/// subject is typed there too.
#[test]
fn type_guard_types_an_unknown_subject_past_a_guard_clause() {
    let backend = create_test_backend();
    let uri = "file:///guard_fallthrough.php";
    let content = r#"<?php
/** @param list<stdClass> $rows */
function f(array $rows): void {
    foreach ($rows as $row) {
        $version = $row->version;
        if (!is_string($version)) {
            continue;
        }
        $version; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
}

/// The proof only holds where the branch does.  An entry the scope has no
/// type for stands for a value that could be anything, and joining that
/// with the branch's `Configuration` is still anything — otherwise a bare
/// `if` would type a variable the code never learned anything about.
#[test]
fn a_proof_about_an_unknown_subject_does_not_outlive_its_branch() {
    let backend = create_test_backend();
    let uri = "file:///guard_join.php";
    let content = format!(
        r#"<?php{SCAFFOLD}
/** @param list<stdClass> $rows */
function f(array $rows): void {{
    foreach ($rows as $row) {{
        $version = $row->version;
        if ($version instanceof Configuration) {{
        }}
        $version; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        !text.contains("Configuration"),
        "the branch-local proof must not survive the join, got: {text}"
    );
}

/// The negated spelling proves the same thing on the implicit-else path,
/// and leaks the same way if the join adopts it.
#[test]
fn a_negated_proof_about_an_unknown_subject_does_not_outlive_its_branch() {
    let backend = create_test_backend();
    let uri = "file:///guard_join_negated.php";
    let content = format!(
        r#"<?php{SCAFFOLD}
/** @param list<stdClass> $rows */
function f(array $rows): void {{
    foreach ($rows as $row) {{
        $version = $row->version;
        if (!$version instanceof Configuration) {{
        }}
        $version; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        !text.contains("Configuration"),
        "the implicit-else proof must not survive the join, got: {text}"
    );
}

// ─── Each branch contributes its end state, and only that ──────────────────

/// The implicit else of a check the subject already satisfies describes a
/// run that cannot happen, so the reassignment the branch made is the only
/// thing the join has to carry.
#[test]
fn a_reassignment_under_an_always_true_check_survives_the_join() {
    let backend = create_test_backend();
    let uri = "file:///branch_reassign.php";
    let content = format!(
        r#"<?php{SCAFFOLD}
function f(AbstractNode $v): void {{
    if ($v instanceof AbstractNode) {{
        $v = $v->getNode();
    }}
    $v; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Node"), "expected Node, got: {text}");
    assert!(
        !text.contains("AbstractNode"),
        "the impossible else path carries no AbstractNode to the join, got: {text}"
    );
}

/// The other direction: a branch that only narrowed hands the join an
/// intersection its sibling path already covers, so the declared type is
/// what comes out.
#[test]
fn a_branch_local_intersection_collapses_back_at_the_join() {
    let backend = create_test_backend();
    let uri = "file:///branch_narrow.php";
    let content = format!(
        r#"<?php{SCAFFOLD}
interface Verbose {{}}
function f(Node $r): void {{
    if ($r instanceof Verbose) {{
    }}
    $r; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Node"), "expected Node, got: {text}");
    assert!(
        !text.contains("Verbose"),
        "the branch-local intersection must not survive the join, got: {text}"
    );
}

// ─── A null check on an array element refines that element ─────────────────

/// A generic `array<int, string|null>` has no per-key slot to refine, so
/// the proof has to be recorded against the element the check named.
#[test]
fn isset_on_an_array_element_refines_that_element() {
    let backend = create_test_backend();
    let uri = "file:///isset_element.php";
    let content = r#"<?php
/** @param array<int, string|null> $m */
function f(array $m): void {
    if (isset($m[0])) {
        $x = $m[0];
        $x; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
    assert!(
        !text.contains("null"),
        "the isset() rules out null for this element, got: {text}"
    );
}

/// The `!== null` and `assert(isset(...))` spellings carry the same proof
/// and must land in the same place.
#[test]
fn non_null_check_on_an_array_element_refines_that_element() {
    let backend = create_test_backend();
    let uri = "file:///element_not_null.php";
    let content = r#"<?php
/** @param array<int, string|null> $m */
function f(array $m): void {
    assert(isset($m[0]));
    $x = $m[0];
    $x; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
    assert!(
        !text.contains("null"),
        "the assertion rules out null for this element, got: {text}"
    );
}

// ─── An optional shape key may not be there at all ─────────────────────────

/// Reading a key the shape marks optional yields the `null` PHP gives for an
/// offset an array does not have, so code that has not checked for it sees
/// that it might be missing. A key the shape requires reads as itself.
#[test]
fn reading_an_optional_shape_key_may_yield_null() {
    let backend = create_test_backend();
    let uri = "file:///optional_shape_key.php";
    let content = r#"<?php
/** @param array{file: string, type?: string} $frame */
function f(array $frame): void {
    $required = $frame['file'];
    $optional = $frame['type'];
    $optional; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("?string"),
        "the key may be absent, and reading a missing offset is null: {text}"
    );

    let required = hover_at(&backend, uri, content, 3, 5);
    let required = hover_text(&required);
    assert!(
        !required.contains("null"),
        "a required key is always there: {required}"
    );
}

/// `!empty($frame['type'])` proves the key is there and truthy, exactly as
/// `isset()` does — including in an expression position, where the proof has
/// to travel through the ternary's own narrowing rather than a branch body.
#[test]
fn not_empty_on_an_optional_shape_key_proves_it_is_there() {
    let backend = create_test_backend();
    let uri = "file:///not_empty_shape_key.php";
    let content = r#"<?php
/** @param array{file: string, type?: string} $frame */
function f(array $frame): void {
    $type = !empty($frame['type']) ? $frame['type'] : 'fallback';
    $type; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
    assert!(
        !text.contains("null"),
        "the check rules out both the missing key and a falsy value: {text}"
    );
}

// ─── An inline `@var` seeds the assignment, then flows ─────────────────────

/// The annotation describes what the assignment produced, not what the
/// variable is at every later read: an ordinary `!== null` guard has to
/// strip the null half the same way it would without the annotation.
#[test]
fn an_inline_var_annotation_submits_to_a_later_null_guard() {
    let backend = create_test_backend();
    let uri = "file:///var_then_guard.php";
    let content = r#"<?php
class Cache {
    /** @return mixed */
    public static function get(string $key) { return null; }
}
function f(): void {
    /** @var null|list<int> $cached */
    $cached = Cache::get('key');
    if ($cached !== null) {
        $cached; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("list<int>"),
        "expected list<int>, got: {text}"
    );
    assert!(
        !text.contains("null"),
        "the `!== null` guard rules out the null half, got: {text}"
    );
}

/// A reassignment below the annotation wins over it, exactly as it would
/// over any other type the walker had established.
#[test]
fn an_inline_var_annotation_yields_to_a_later_reassignment() {
    let backend = create_test_backend();
    let uri = "file:///var_then_reassign.php";
    let content = r#"<?php
class Ticket {}
function f(): void {
    /** @var array<int> $item */
    $item = [];
    $item = new Ticket();
    $item; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("Ticket"), "expected Ticket, got: {text}");
    assert!(
        !text.contains("array"),
        "the reassignment replaces the annotated type, got: {text}"
    );
}

// ─── A truthy test rules out the members that are always falsy ─────────────

/// `?? []` on a nullable value leaves `string|array{}`, and `array{}` is
/// the empty array, which PHP always treats as false.  A truthy guard
/// therefore proves the value is the string half, and passing it on to a
/// function that wants a `string` is not a mistake.
#[test]
fn a_truthy_guard_rules_out_the_empty_array_shape() {
    let backend = create_test_backend();
    let uri = "file:///truthy_empty_shape.php";
    let content = r#"<?php
function f(?string $s): void {
    $value = $s ?? [];
    if ($value) {
        $value; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
    assert!(
        !text.contains("array"),
        "an empty array shape cannot survive a truthy test, got: {text}"
    );
}

/// `!empty()` reaches the same rule through the same narrowing, so the
/// two spellings of the guard agree.
#[test]
fn a_not_empty_guard_rules_out_the_empty_array_shape() {
    let backend = create_test_backend();
    let uri = "file:///not_empty_empty_shape.php";
    let content = r#"<?php
function f(?string $s): void {
    $value = $s ?? [];
    if (!empty($value)) {
        $value; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
    assert!(
        !text.contains("array"),
        "an empty array shape cannot survive a truthy test, got: {text}"
    );
}

/// A shape with a required field is a non-empty array, so it is truthy and
/// must survive the same guard the empty one is dropped by.
#[test]
fn a_truthy_guard_keeps_a_shape_that_has_a_required_field() {
    let backend = create_test_backend();
    let uri = "file:///truthy_filled_shape.php";
    let content = r#"<?php
/** @return array{id: int}|null */
function load(): ?array { return null; }
function f(): void {
    $row = load();
    if ($row) {
        $row; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("id"),
        "a shape with a required field is truthy, got: {text}"
    );
}

/// The falsy string and int literals go the same way: `'0'` and `0` are
/// false in PHP, so a truthy branch cannot be holding either.
#[test]
fn a_truthy_guard_rules_out_the_falsy_literals() {
    let backend = create_test_backend();
    let uri = "file:///truthy_literals.php";
    let content = r#"<?php
/** @return 'yes'|'0'|0|7 */
function pick() { return 7; }
function f(): void {
    $picked = pick();
    if ($picked) {
        $picked; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("yes"), "the truthy string stays, got: {text}");
    assert!(text.contains('7'), "the truthy int stays, got: {text}");
    assert!(
        !text.contains("'0'") && !text.contains('0'),
        "`'0'` and `0` are falsy in PHP, got: {text}"
    );
}

/// An `isset()` on a chain whose middle segment is a variable index proves
/// the optional shape key at the end of it is there, the same way a chain
/// of literal keys does. The subject key records the index variable, so the
/// proof survives to the read that follows.
#[test]
fn isset_narrows_a_chain_through_a_variable_index() {
    let backend = create_test_backend();
    let uri = "file:///isset_variable_index.php";
    let content = r#"<?php
/**
 * @param array{files?: array<string, array{violations?: list<string>}>} $state
 */
function f(array $state, string $path): void {
    if (!isset($state['files'][$path]['violations'])) {
        return;
    }
    $found = $state['files'][$path]['violations']; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("list<string>"),
        "expected the shape's list, got: {text}"
    );
    assert!(
        !text.contains("null"),
        "an optional key proved present is not null, got: {text}"
    );
}

/// The same proof read from inside the `isset()` branch rather than after
/// a guard clause.
#[test]
fn isset_narrows_a_chain_through_a_variable_index_inside_the_branch() {
    let backend = create_test_backend();
    let uri = "file:///isset_variable_index_branch.php";
    let content = r#"<?php
/**
 * @param array{files?: array<string, array{violations?: list<string>}>} $state
 */
function f(array $state, string $path): void {
    if (isset($state['files'][$path]['violations'])) {
        $found = $state['files'][$path]['violations']; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("list<string>"),
        "expected the shape's list, got: {text}"
    );
    assert!(
        !text.contains("null"),
        "an optional key proved present is not null, got: {text}"
    );
}

/// The index variable is part of what the proof was made about, so writing
/// to it makes the recorded narrowing stale: the chain now addresses a
/// different element and the optional key is unproven again.
#[test]
fn writing_the_index_variable_drops_the_isset_proof() {
    let backend = create_test_backend();
    let uri = "file:///isset_variable_index_reassigned.php";
    let content = r#"<?php
/**
 * @param array{files?: array<string, array{violations?: list<string>}>} $state
 */
function f(array $state, string $path, string $other): void {
    if (!isset($state['files'][$path]['violations'])) {
        return;
    }
    $path = $other;
    $found = $state['files'][$path]['violations']; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("null"),
        "a proof about a different element does not carry over, got: {text}"
    );
}
