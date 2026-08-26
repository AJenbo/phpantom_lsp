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

// ─── Variables a branch writes together share one null ─────────────────────

const REGISTRY: &str = r#"
class Reflection {}
class Acceptor {}
class Registry {
    public function find(string $name): ?Reflection { return null; }
    public function select(Reflection $r): Acceptor { return new Acceptor(); }
}
"#;

/// `$acceptor` is written on exactly the path that leaves `$reflection`
/// holding a value, so the two are null together or not at all.  Testing
/// one of them therefore settles the other, even though the branch that
/// correlated them is long over and the test never names `$acceptor`.
#[test]
fn variables_written_on_the_same_path_share_their_null() {
    let backend = create_test_backend();
    let uri = "file:///correlated_null.php";
    let content = format!(
        r#"<?php
{REGISTRY}
function f(Registry $registry, string $name): void {{
    $acceptor = null;
    $reflection = null;
    if ($name !== '') {{
        $reflection = $registry->find($name);
        if ($reflection !== null) {{
            $acceptor = $registry->select($reflection);
        }}
    }}

    if ($reflection !== null) {{
        $acceptor; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Acceptor"), "expected Acceptor, got: {text}");
    assert!(
        !text.contains("null"),
        "the check on $reflection rules out $acceptor's null too, got: {text}"
    );
}

/// Two variables written under conditions of their own are not
/// correlated, however alike the two branches look.  Proving one is not
/// null says nothing about whether the other's branch ran.
#[test]
fn variables_written_in_separate_branches_stay_independent() {
    let backend = create_test_backend();
    let uri = "file:///independent_null.php";
    let content = format!(
        r#"<?php
{REGISTRY}
function f(bool $a, bool $b): void {{
    $acceptor = null;
    $reflection = null;
    if ($a) {{
        $reflection = new Reflection();
    }}
    if ($b) {{
        $acceptor = new Acceptor();
    }}

    if ($reflection !== null) {{
        $acceptor; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("null"),
        "$acceptor's branch may not have run, got: {text}"
    );
}

/// Writing to one of the pair on a path that leaves the other alone
/// breaks the correlation: past that branch, a value in `$reflection` no
/// longer means the branch that filled `$acceptor` is the one that ran.
#[test]
fn a_later_write_breaks_the_correlation() {
    let backend = create_test_backend();
    let uri = "file:///broken_correlation.php";
    let content = format!(
        r#"<?php
{REGISTRY}
function f(bool $a, bool $b): void {{
    $acceptor = null;
    $reflection = null;
    if ($a) {{
        $reflection = new Reflection();
        $acceptor = new Acceptor();
    }}
    if ($b) {{
        $reflection = new Reflection();
    }}

    if ($reflection !== null) {{
        $acceptor; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("null"),
        "the second branch fills only $reflection, got: {text}"
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

/// `array_key_exists()` proves the key is there, so the read is no
/// longer the `null` a missing offset yields.
#[test]
fn array_key_exists_on_an_optional_shape_key_proves_it_is_there() {
    let backend = create_test_backend();
    let uri = "file:///key_exists_shape_key.php";
    let content = r#"<?php
/** @param array{file: string, type?: string} $frame */
function f(array $frame): void {
    if (array_key_exists('type', $frame)) {
        $type = $frame['type'];
        $type; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("string"), "expected string, got: {text}");
    assert!(
        !text.contains("null"),
        "the key is known to be present: {text}"
    );
}

/// The subject may be a property rather than a local, and the guard may
/// be written as an early return.
#[test]
fn array_key_exists_guard_clause_proves_a_property_shape_key() {
    let backend = create_test_backend();
    let uri = "file:///key_exists_property.php";
    let content = r#"<?php
class Excluder {
    /** @var array{analyse?: list<string>} */
    private array $paths = [];

    public function f(): void {
        if (!array_key_exists('analyse', $this->paths)) {
            return;
        }
        $analyse = $this->paths['analyse'];
        $analyse; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("list<string>"),
        "expected list<string>, got: {text}"
    );
    assert!(
        !text.contains("null"),
        "the guard's fall-through proves the key is present: {text}"
    );
}

/// Presence is all `array_key_exists()` proves: a key declared nullable
/// keeps its `null`, which is where it differs from `isset()`.
#[test]
fn array_key_exists_does_not_rule_out_a_null_value() {
    let backend = create_test_backend();
    let uri = "file:///key_exists_nullable_value.php";
    let content = r#"<?php
/** @param array{type?: string|null} $frame */
function f(array $frame): void {
    if (array_key_exists('type', $frame)) {
        $type = $frame['type'];
        $type; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("null") || text.contains("?string"),
        "the value itself may still be null: {text}"
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

// ─── The proof reaches the chain through the value it was stored in ─────────

const CHAIN_SCAFFOLD: &str = r#"
class Period {}
class Agreement {
    public function latestPeriod(): ?Period { return null; }
}
function accept(Agreement $agreement): void {}
"#;

/// Run the slow diagnostic pipeline and keep the argument-type errors a
/// lost narrowing produces.
fn argument_type_errors(backend: &Backend, uri: &str, php: &str) -> Vec<String> {
    backend.update_ast(uri, php);
    let mut out = Vec::new();
    backend.collect_slow_diagnostics(uri, php, &mut out);
    out.iter()
        .filter(|d| {
            d.code.as_ref().is_some_and(
                |c| matches!(c, NumberOrString::String(s) if s == "type_mismatch_argument"),
            )
        })
        .map(|d| d.message.clone())
        .collect()
}

/// The guard names only the chain's *result*, but a null `$agreement`
/// short-circuits the chain to `null`, so past the guard the receiver
/// cannot be null either.
#[test]
fn a_guard_on_a_stored_chain_result_narrows_the_receiver() {
    let backend = create_test_backend();
    let uri = "file:///chain_stored.php";
    let content = format!(
        r#"<?php
{CHAIN_SCAFFOLD}
function process(?Agreement $agreement): void {{
    $period = $agreement?->latestPeriod();
    if (!$period instanceof Period) {{
        return;
    }}
    accept($agreement);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert!(
        errors.is_empty(),
        "the receiver cannot be null past the guard, got: {errors:?}"
    );
}

/// The branch that runs when the chain *did* yield null learns nothing:
/// a null receiver is exactly one of the ways it gets there.
#[test]
fn a_stored_chain_result_leaves_the_failing_path_alone() {
    let backend = create_test_backend();
    let uri = "file:///chain_stored_else.php";
    let content = format!(
        r#"<?php
{CHAIN_SCAFFOLD}
function process(?Agreement $agreement): void {{
    $period = $agreement?->latestPeriod();
    if ($period !== null) {{
        return;
    }}
    accept($agreement);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert_eq!(
        errors.len(),
        1,
        "a failing chain leaves the receiver's null in play, got: {errors:?}"
    );
}

/// Writing to the receiver between the chain and the guard drops the
/// proof: what the guard rules out is the value the chain ran against,
/// not whatever the variable holds now.
#[test]
fn reassigning_the_receiver_drops_the_chain_proof() {
    let backend = create_test_backend();
    let uri = "file:///chain_reassigned.php";
    let content = format!(
        r#"<?php
{CHAIN_SCAFFOLD}
function process(?Agreement $agreement): void {{
    $period = $agreement?->latestPeriod();
    $agreement = null;
    if ($period !== null) {{
        accept($agreement);
    }}
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert_eq!(
        errors.len(),
        1,
        "the reassigned receiver is not what the guard proved, got: {errors:?}"
    );
}

// ─── Comparing a chain to a value that cannot be null proves it ran ─────────

const COMPARE_SCAFFOLD: &str = r#"
final class NodeX {
    public function getParent(): ?NodeX { return null; }
    public function getChild(): NodeX { return $this; }
    public function getInner(): object { return $this; }
    public function maybeInner(): ?object { return null; }
}
"#;

/// A `?->` chain that short-circuited would hold `null`, and `null` is
/// never identical to a value whose type excludes it — so the comparison
/// succeeding proves the chain ran, receivers and all.  The plain `->`
/// links written after the `?->` are part of the same chain.
#[test]
fn a_chain_compared_to_a_non_nullable_value_narrows_the_receiver() {
    let backend = create_test_backend();
    let uri = "file:///chain_identity.php";
    let content = format!(
        r#"<?php
{COMPARE_SCAFFOLD}
function strip(NodeX $n): void {{
    $parent = $n->getParent();
    if ($parent?->getChild()->getInner() === $n->getInner()) {{
        $parent; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("NodeX"), "expected NodeX, got: {text}");
    assert!(
        !text.contains("null") && !text.contains("?NodeX"),
        "the chain cannot have short-circuited inside the branch, got: {text}"
    );
}

/// The mirror image: a `!==` that failed is an `===` that held, so the
/// else branch carries the same proof.
#[test]
fn a_failing_inequality_narrows_the_receiver_in_the_else_branch() {
    let backend = create_test_backend();
    let uri = "file:///chain_identity_else.php";
    let content = format!(
        r#"<?php
{COMPARE_SCAFFOLD}
function strip(NodeX $n): void {{
    $parent = $n->getParent();
    if ($parent?->getChild()->getInner() !== $n->getInner()) {{
        return;
    }}
    $parent; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("NodeX"), "expected NodeX, got: {text}");
    assert!(
        !text.contains("null") && !text.contains("?NodeX"),
        "the chain cannot have short-circuited past the guard, got: {text}"
    );
}

/// Nothing is proven when the other side can be null too: both sides
/// being `null` is one of the ways the comparison succeeds.
#[test]
fn a_chain_compared_to_a_nullable_value_leaves_the_receiver_alone() {
    let backend = create_test_backend();
    let uri = "file:///chain_identity_nullable.php";
    let content = format!(
        r#"<?php
{COMPARE_SCAFFOLD}
function strip(NodeX $n): void {{
    $parent = $n->getParent();
    if ($parent?->getChild()->getInner() === $n->maybeInner()) {{
        $parent; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("?NodeX") || text.contains("null"),
        "a nullable comparand proves nothing, got: {text}"
    );
}

// ─── A `match (true)` arm's conditions hold inside its result ───────────────

const MATCH_SCAFFOLD: &str = r#"
function takesInt(int $i): void {}
/** @param list<int> $args */
function takesList(array $args): void {}
"#;

/// The `!== null` conjuncts of an arm's condition prove the values
/// non-null within that arm's result, exactly as the equivalent `if`
/// does.
#[test]
fn a_match_true_arm_narrows_inside_its_result() {
    let backend = create_test_backend();
    let uri = "file:///match_arm_narrowing.php";
    let content = format!(
        r#"<?php
{MATCH_SCAFFOLD}
function label(?int $buy, string $kind): void {{
    $value = match (true) {{
        $kind === 'xy' && $buy !== null => $buy,
        default => 0,
    }};
    takesInt($value);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert!(
        errors.is_empty(),
        "the arm's own condition rules the null out, got: {errors:?}"
    );
}

/// The proof reaches the elements of an array the arm builds, not just a
/// value it returns directly.
#[test]
fn a_match_true_arm_narrows_the_array_it_builds() {
    let backend = create_test_backend();
    let uri = "file:///match_arm_array.php";
    let content = format!(
        r#"<?php
{MATCH_SCAFFOLD}
function label(?int $buy, ?int $pay): void {{
    $textArgs = match (true) {{
        $buy !== null && $pay !== null => [$buy, $pay],
        default => [],
    }};
    takesList($textArgs);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert!(
        errors.is_empty(),
        "the elements are what the condition proved them to be, got: {errors:?}"
    );
}

/// An arm runs when *any* of its conditions matched, so a fact only one
/// of them establishes is not a fact in the body.
#[test]
fn an_arm_condition_that_only_sometimes_proves_it_narrows_nothing() {
    let backend = create_test_backend();
    let uri = "file:///match_arm_partial.php";
    let content = format!(
        r#"<?php
{MATCH_SCAFFOLD}
function label(?int $buy, ?int $pay): void {{
    $value = match (true) {{
        $buy !== null, $pay !== null => $buy,
        default => 0,
    }};
    takesInt($value);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert_eq!(
        errors.len(),
        1,
        "only one of the two conditions rules the null out, got: {errors:?}"
    );
}

/// Reaching a later arm means every arm above it was tested and failed,
/// so the `default` sees the inverse of their conditions.
#[test]
fn a_later_arm_sees_the_inverse_of_the_arms_above_it() {
    let backend = create_test_backend();
    let uri = "file:///match_arm_default.php";
    let content = format!(
        r#"<?php
{MATCH_SCAFFOLD}
function label(?int $buy): void {{
    $value = match (true) {{
        $buy === null => 0,
        default => $buy,
    }};
    takesInt($value);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert!(
        errors.is_empty(),
        "the arm above ruled the null out, got: {errors:?}"
    );
}

// ─── A helper that bails out on its condition proves the other half ─────────

const BAIL_SCAFFOLD: &str = r#"
class Admin {
    public function grantPermission(string $name): void {}
}
class User {
    public string $email = '';
}
function abort_if(bool $boolean, int $code): void {}
function abort_unless(bool $boolean, int $code): void {}
function throw_if(bool $condition, string $exception): void {}
function throw_unless(bool $condition, string $exception): void {}
function takesUser(User $user): void {}
function takesAdmin(Admin $admin): void {}
"#;

/// `abort_if($user === null, 404)` returns only when the condition was
/// false, so the null is gone from there on.
#[test]
fn abort_if_proves_the_inverse_of_its_condition() {
    let backend = create_test_backend();
    let uri = "file:///abort_if_null.php";
    let content = format!(
        r#"<?php
{BAIL_SCAFFOLD}
function show(?User $user): void {{
    abort_if($user === null, 404);
    $user; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("$user = User"),
        "the call returned, so the condition was false, got: {text}"
    );
}

/// `abort_unless($user instanceof Admin, 403)` returns only when the
/// condition held, so the subject is the checked class afterwards.
#[test]
fn abort_unless_proves_its_condition() {
    let backend = create_test_backend();
    let uri = "file:///abort_unless_instanceof.php";
    let content = format!(
        r#"<?php
{BAIL_SCAFFOLD}
function show(User|Admin $user): void {{
    abort_unless($user instanceof Admin, 403);
    $user; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Admin"), "expected Admin, got: {text}");
    assert!(
        !text.contains("User"),
        "the check rules the other member out, got: {text}"
    );
}

/// The narrowing has to reach the member lookups that follow, not just
/// hover: `grantPermission()` only exists on the proven class.
#[test]
fn a_bailing_helper_narrows_the_calls_that_follow_it() {
    let backend = create_test_backend();
    let uri = "file:///abort_unless_members.php";
    let content = format!(
        r#"<?php
{BAIL_SCAFFOLD}
function show(?User $user, User|Admin $account): void {{
    abort_if($user === null, 404);
    takesUser($user);
    abort_unless($account instanceof Admin, 403);
    takesAdmin($account);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert!(
        errors.is_empty(),
        "both helpers proved what their arguments need, got: {errors:?}"
    );
}

/// `throw_if` / `throw_unless` bail out the same way `abort_if` /
/// `abort_unless` do, so they prove the same thing.
#[test]
fn throw_if_and_throw_unless_narrow_like_their_abort_counterparts() {
    let backend = create_test_backend();
    let uri = "file:///throw_if_unless.php";
    let content = format!(
        r#"<?php
{BAIL_SCAFFOLD}
function show(?User $user, User|Admin $account): void {{
    throw_if($user === null, \RuntimeException::class);
    takesUser($user);
    throw_unless($account instanceof Admin, \RuntimeException::class);
    takesAdmin($account);
}}
"#
    );

    let errors = argument_type_errors(&backend, uri, &content);
    assert!(
        errors.is_empty(),
        "the throwing helpers prove what their arguments need, got: {errors:?}"
    );
}

/// The condition is found by parameter name, so re-ordered named
/// arguments narrow the same as positional ones.
#[test]
fn a_named_condition_argument_narrows_wherever_it_sits() {
    let backend = create_test_backend();
    let uri = "file:///abort_if_named.php";
    let content = format!(
        r#"<?php
{BAIL_SCAFFOLD}
function show(?User $user): void {{
    abort_if(code: 404, boolean: $user === null);
    $user; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("$user = User"),
        "the named condition proves the same thing, got: {text}"
    );
}

/// A helper reached through a namespace is not the global one, so it
/// proves nothing about its argument.
#[test]
fn a_namespaced_lookalike_proves_nothing() {
    let backend = create_test_backend();
    let uri = "file:///abort_if_namespaced.php";
    let content = format!(
        r#"<?php
{BAIL_SCAFFOLD}
namespace Other {{
    function abort_if(bool $boolean, int $code): void {{}}
}}
namespace App {{
    function show(?\User $user): void {{
        \Other\abort_if($user === null, 404);
        $user; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("?User"),
        "a different function proves nothing, got: {text}"
    );
}

// ─── An identity comparison carries the comparand's proof ───────────────────

/// `$a === $b` holding means both sides carried the same value, so a
/// nullable subject compared identical to a value that cannot be null
/// holds no null in that branch.
#[test]
fn identity_against_a_non_null_operand_strips_null() {
    let backend = create_test_backend();
    let uri = "file:///identity_non_null.php";
    let content = r#"<?php
class Ctx {
    public function name(): ?string { return null; }
}
function f(Ctx $ctx, string $wanted): void {
    $name = $ctx->name();
    if ($name === $wanted) {
        $name; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("string") && !text.contains("?string") && !text.contains("null"),
        "the identity rules out the null, got: {text}"
    );
}

/// Operand order does not change what the identity proves.
#[test]
fn identity_narrows_with_the_subject_on_the_right() {
    let backend = create_test_backend();
    let uri = "file:///identity_reversed.php";
    let content = r#"<?php
class Ctx {
    public function name(): ?string { return null; }
}
function f(Ctx $ctx, string $wanted): void {
    $name = $ctx->name();
    if ($wanted === $name) {
        $name; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("string") && !text.contains("?string") && !text.contains("null"),
        "the identity rules out the null, got: {text}"
    );
}

/// `$a !== $b` returning false is the same proof, so the fall-through of
/// a `!==` guard clause narrows too.
#[test]
fn a_not_identical_guard_clause_strips_null_on_fall_through() {
    let backend = create_test_backend();
    let uri = "file:///identity_guard.php";
    let content = r#"<?php
class Ctx {
    public function name(): ?string { return null; }
}
function f(Ctx $ctx, string $wanted): void {
    $name = $ctx->name();
    if ($name !== $wanted) {
        return;
    }
    $name; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("string") && !text.contains("?string") && !text.contains("null"),
        "the guard's fall-through proves the identity held, got: {text}"
    );
}

/// A comparand that is itself nullable proves nothing: both sides could
/// have been null.
#[test]
fn identity_against_a_nullable_operand_proves_nothing() {
    let backend = create_test_backend();
    let uri = "file:///identity_nullable_operand.php";
    let content = r#"<?php
class Ctx {
    public function name(): ?string { return null; }
    public function other(): ?string { return null; }
}
function f(Ctx $ctx): void {
    $name = $ctx->name();
    if ($name === $ctx->other()) {
        $name; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("?string"),
        "both sides could have been null, got: {text}"
    );
}

/// A loose comparison proves nothing: `null == 0` and `null == false`
/// are both true.
#[test]
fn a_loose_comparison_does_not_strip_null() {
    let backend = create_test_backend();
    let uri = "file:///loose_comparison.php";
    let content = r#"<?php
class Ctx {
    public function count(): ?int { return null; }
}
function f(Ctx $ctx, int $wanted): void {
    $count = $ctx->count();
    if ($count == $wanted) {
        $count; // <-- here
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("?int"),
        "`null == 0` holds, so the loose check proves nothing, got: {text}"
    );
}

// ─── A type guard's negative branch strips the member it checked ────────────

/// `is_float()` failing rules out `float`, so the else branch of the check
/// (and a reassignment merged back from the taken branch) leaves plain
/// `int` to flow into an array literal.
#[test]
fn is_float_negative_branch_strips_float_from_a_declared_union() {
    let backend = create_test_backend();
    let uri = "file:///is_float_else.php";
    let content = r#"<?php
function f(int $offsetValue, int $max): void {
    /** @var int|float $newAutoIndex */
    $newAutoIndex = $offsetValue + 1;
    if (is_float($newAutoIndex)) {
        $newAutoIndex = $max;
    }
    $indexes = [$newAutoIndex];
    $indexes; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("array{int}"),
        "both paths leave an int, got: {text}"
    );
}

/// The mirror case: `is_int()` failing leaves `float`.
#[test]
fn is_int_negative_branch_strips_int_from_a_declared_union() {
    let backend = create_test_backend();
    let uri = "file:///is_int_else.php";
    let content = r#"<?php
/** @param int|float $value */
function f($value): void {
    if (is_int($value)) {
        return;
    }
    $value; // <-- here
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(text.contains("float"), "expected float, got: {text}");
    assert!(!text.contains("int"), "the int is ruled out, got: {text}");
}

// ─── `instanceof` against a value holding the class name ────────────────────

const DYNAMIC_SCAFFOLD: &str = r#"
class Stmt {}
class Continue_ extends Stmt { public ?int $num = null; }
class Break_ extends Stmt { public ?int $num = null; }
"#;

/// `$x instanceof $class` resolves against whatever `$class` holds, so a
/// `class-string<T>` narrows the subject to `T`.
#[test]
fn instanceof_a_class_string_variable_narrows_the_subject() {
    let backend = create_test_backend();
    let uri = "file:///dynamic_instanceof.php";
    let content = format!(
        r#"<?php
{DYNAMIC_SCAFFOLD}
/**
 * @param class-string<Continue_> $stmtClass
 */
function f(Stmt $statement, string $stmtClass): void {{
    if ($statement instanceof $stmtClass) {{
        $statement; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("Continue_"),
        "expected Continue_, got: {text}"
    );
}

/// A union of `class-string`s narrows to the union of the classes they
/// name, and the guard-clause form proves the same thing.
#[test]
fn a_negated_dynamic_instanceof_guard_narrows_past_it() {
    let backend = create_test_backend();
    let uri = "file:///dynamic_instanceof_guard.php";
    let content = format!(
        r#"<?php
{DYNAMIC_SCAFFOLD}
/**
 * @param list<Stmt> $stmts
 * @param class-string<Continue_>|class-string<Break_> $stmtClass
 */
function f(array $stmts, string $stmtClass): void {{
    foreach ($stmts as $statement) {{
        if (!$statement instanceof $stmtClass) {{
            continue;
        }}
        $statement; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        text.contains("Continue_") || text.contains("Break_"),
        "expected one of the checked classes, got: {text}"
    );
    assert!(
        !text.contains("$statement = Stmt\n"),
        "the check filtered the union, got: {text}"
    );
}

/// An object-typed right-hand side stands for its own class, which is
/// what `$a instanceof $b` checks at runtime.
#[test]
fn instanceof_an_object_valued_operand_narrows_the_subject() {
    let backend = create_test_backend();
    let uri = "file:///dynamic_instanceof_object.php";
    let content = format!(
        r#"<?php
{DYNAMIC_SCAFFOLD}
function f(Stmt $statement, Break_ $other): void {{
    if ($statement instanceof $other) {{
        $statement; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Break_"), "expected Break_, got: {text}");
}

/// An unsubstituted `@template T` names no loadable class, so the check
/// proves nothing.  Narrowing to it would leave the subject unresolvable
/// and report every later member access on it.
#[test]
fn instanceof_an_unsubstituted_template_operand_leaves_the_subject_alone() {
    let backend = create_test_backend();
    let uri = "file:///dynamic_instanceof_template.php";
    let content = r#"<?php
class Node {
    public function name(): string { return 'n'; }
}
interface Finder {
    /**
     * @template T of Node
     * @param class-string<T> $targetType
     * @return T[]
     */
    public function findChildren($targetType): array;
}
class Artifact implements Finder {
    /** @return list<Node> */
    public function children(): array { return []; }

    public function findChildren($targetType): array
    {
        foreach ($this->children() as $node) {
            if ($node instanceof $targetType) {
                echo 'match';
            }
            $found = $node;
            $found; // <-- here
        }
        return [];
    }
}
"#;

    let text = hover_marked(&backend, uri, content);
    assert!(
        text.contains("Node"),
        "the subject keeps its declared type, got: {text}"
    );
}

/// A plain `string` names no class, so the check proves nothing and the
/// subject keeps its declared type rather than being narrowed to noise.
#[test]
fn instanceof_an_unspecific_string_operand_proves_nothing() {
    let backend = create_test_backend();
    let uri = "file:///dynamic_instanceof_plain.php";
    let content = format!(
        r#"<?php
{DYNAMIC_SCAFFOLD}
function f(Stmt $statement, string $stmtClass): void {{
    if ($statement instanceof $stmtClass) {{
        $statement; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Stmt"), "expected Stmt, got: {text}");
}

// ─── An `elseif`'s own condition narrows its later operands ─────────────────

const ELSEIF_SCAFFOLD: &str = r#"
class Ty {}
class ShapeTy extends Ty {
    public function propName(): string { return 'x'; }
}
"#;

/// The right-hand operand of an `&&` sees what the left one proved,
/// whether the condition belongs to the leading `if` or to an `elseif`.
#[test]
fn an_elseif_condition_narrows_its_own_and_chain() {
    let backend = create_test_backend();
    let uri = "file:///elseif_and_chain.php";
    let content = format!(
        r#"<?php
{ELSEIF_SCAFFOLD}
function f(Ty $t, bool $flag): void {{
    if ($flag) {{
        echo 'flag';
    }} elseif ($t instanceof ShapeTy && $t->propName() !== '') {{
        $t; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("ShapeTy"), "expected ShapeTy, got: {text}");
}

/// The same for a property subject, which is the shape the check is
/// usually written on.
#[test]
fn an_elseif_condition_narrows_a_property_subject_for_its_later_operands() {
    let backend = create_test_backend();
    let uri = "file:///elseif_property.php";
    let content = format!(
        r#"<?php
{ELSEIF_SCAFFOLD}
class Holder {{
    private Ty $type;

    public function __construct(Ty $type) {{ $this->type = $type; }}

    public function f(bool $flag): void {{
        if ($flag) {{
            echo 'flag';
        }} elseif ($this->type instanceof ShapeTy && $this->type->propName() !== '') {{
            $narrowed = $this->type;
            $narrowed; // <-- here
        }}
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("ShapeTy"), "expected ShapeTy, got: {text}");
}

// ─── An array element addressed by a variable is one subject ───────────────

/// `$types[$i]` written twice is the same read, so a guard on the first
/// narrows the second — the same way a constant offset already did.
#[test]
fn instanceof_narrows_an_array_element_addressed_by_a_variable() {
    let backend = create_test_backend();
    let uri = "file:///array_variable_index.php";
    let content = format!(
        r#"<?php
{ELSEIF_SCAFFOLD}
/** @param list<Ty> $types */
function f(array $types): void {{
    for ($i = 0; $i < count($types); $i++) {{
        if ($types[$i] instanceof ShapeTy && $types[$i]->propName() !== '') {{
            $element = $types[$i];
            $element; // <-- here
        }}
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("ShapeTy"), "expected ShapeTy, got: {text}");
}

/// An offset computed from a variable (`$stmts[$count - 2]`) addresses one
/// element just as a bare variable index does, so the guard carries.
#[test]
fn instanceof_narrows_an_array_element_addressed_by_a_computed_index() {
    let backend = create_test_backend();
    let uri = "file:///array_computed_index.php";
    let content = format!(
        r#"<?php
{ELSEIF_SCAFFOLD}
/** @param list<Ty> $types */
function f(array $types, int $count): void {{
    if (!$types[$count - 2] instanceof ShapeTy) {{
        return;
    }}
    $element = $types[$count - 2];
    $element; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("ShapeTy"), "expected ShapeTy, got: {text}");
}

/// Writing to a variable the offset reads makes the element key stale, so
/// the guard stops describing it.
#[test]
fn writing_to_an_index_operand_drops_the_computed_element_narrowing() {
    let backend = create_test_backend();
    let uri = "file:///array_computed_index_write.php";
    let content = format!(
        r#"<?php
{ELSEIF_SCAFFOLD}
/** @param list<Ty> $types */
function f(array $types, int $count): void {{
    if (!$types[$count - 2] instanceof ShapeTy) {{
        return;
    }}
    $count = 0;
    $element = $types[$count - 2];
    $element; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        !text.contains("ShapeTy"),
        "a moved offset proves nothing, got: {text}"
    );
}

/// A different index is a different subject, so the guard proves nothing
/// about it.
#[test]
fn a_guard_on_one_index_does_not_narrow_another() {
    let backend = create_test_backend();
    let uri = "file:///array_other_index.php";
    let content = format!(
        r#"<?php
{ELSEIF_SCAFFOLD}
/** @param list<Ty> $types */
function f(array $types, int $i, int $j): void {{
    if ($types[$i] instanceof ShapeTy) {{
        $other = $types[$j];
        $other; // <-- here
    }}
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(
        !text.contains("ShapeTy"),
        "a different index proves nothing, got: {text}"
    );
}

// ─── A check no value can pass leaves no state behind ────────────────────────

const ACCUMULATOR_SCAFFOLD: &str = r#"
class Scope {
    public function mergeWith(?Scope $other): Scope { return $this; }
}
class BranchEnd {
    public function getScope(): Scope { return new Scope(); }
}
"#;

/// The loop-fold accumulator: seed on the run where the variable is still
/// `null`, merge on every run after it.  On the first pass the merge arm
/// cannot be entered at all — `$acc` is exactly `null` there — so the
/// unresolvable `null->mergeWith()` it would perform must not join back
/// and erase the seed the other arm produced.
#[test]
fn a_loop_fold_accumulator_keeps_the_type_its_seed_branch_established() {
    let backend = create_test_backend();
    let uri = "file:///accumulator_guard.php";
    let content = format!(
        r#"<?php
{ACCUMULATOR_SCAFFOLD}
/** @param BranchEnd[] $ends */
function f(array $ends): void {{
    $acc = null;
    foreach ($ends as $end) {{
        if ($acc === null) {{
            $acc = $end->getScope();
            continue;
        }}
        $acc = $acc->mergeWith($end->getScope());
    }}
    $acc; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Scope"), "expected Scope, got: {text}");
}

/// The same fold written as an `if`/`else` rather than a guard clause.
#[test]
fn a_loop_fold_accumulator_written_as_if_else_keeps_its_seed_type() {
    let backend = create_test_backend();
    let uri = "file:///accumulator_if_else.php";
    let content = format!(
        r#"<?php
{ACCUMULATOR_SCAFFOLD}
/** @param BranchEnd[] $ends */
function f(array $ends): void {{
    $acc = null;
    foreach ($ends as $end) {{
        if ($acc === null) {{
            $acc = $end->getScope();
        }} else {{
            $acc = $acc->mergeWith($end->getScope());
        }}
    }}
    $acc; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Scope"), "expected Scope, got: {text}");
}

/// And as a ternary, where the dead arm is pruned by the expression
/// resolver rather than by the statement walker.
#[test]
fn a_ternary_fold_accumulator_drops_the_arm_its_condition_rules_out() {
    let backend = create_test_backend();
    let uri = "file:///accumulator_ternary.php";
    let content = format!(
        r#"<?php
{ACCUMULATOR_SCAFFOLD}
/** @param BranchEnd[] $ends */
function f(array $ends): void {{
    $acc = null;
    foreach ($ends as $end) {{
        $acc = $acc === null ? $end->getScope() : $acc->mergeWith($end->getScope());
    }}
    $acc; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Scope"), "expected Scope, got: {text}");
}

/// The pruning is a statement about the *path*, not about the variable: a
/// guard whose subject really can be non-null still leaves the rest of the
/// body reachable.
#[test]
fn a_guard_that_rules_out_null_leaves_the_rest_of_the_body_reachable() {
    let backend = create_test_backend();
    let uri = "file:///accumulator_live_guard.php";
    let content = format!(
        r#"<?php
{ACCUMULATOR_SCAFFOLD}
function f(?Scope $acc, BranchEnd $end): void {{
    if ($acc === null) {{
        return;
    }}
    $acc = $acc->mergeWith($end->getScope());
    $acc; // <-- here
}}
"#
    );

    let text = hover_marked(&backend, uri, &content);
    assert!(text.contains("Scope"), "expected Scope, got: {text}");
}

// ─── get_class() identity narrows whatever instanceof would ─────────────────

/// A global function is as often written `\get_class(…)` as `get_class(…)`,
/// and the two spellings name the same function.
#[test]
fn a_fully_qualified_get_class_narrows_too() {
    let backend = create_test_backend();
    let uri = "file:///get_class_fqn.php";
    let content = r#"<?php
namespace App;

class Base {}
class Sub extends Base {}

function f(Base $x): void {
    if (\get_class($x) === Sub::class) {
        $x; // <-- here
    }
}
"#;

    let hover = hover_marked(&backend, uri, content);
    assert!(
        hover.contains("Sub"),
        "a leading backslash does not make it a different function, got: {hover}"
    );
}
