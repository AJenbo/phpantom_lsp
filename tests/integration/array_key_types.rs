//! Key and element types read off arrays: what a `foreach` binds, what a
//! list-destructuring position picks, and what the key-reading builtins
//! report for an array whose key type the caller established.

use crate::common::create_test_backend_with_full_stubs;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

/// The type reported for the variable right after a `/*NAME*/` marker.
///
/// The marker sits on a *use* of the variable rather than its assignment,
/// so this reads the type the forward walker bound at that point — which is
/// what a `foreach` key/value and a destructuring position produce.
fn type_at_marker(backend: &Backend, uri: &str, content: &str, marker: &str) -> String {
    let needle = format!("/*{marker}*/$");
    let (line, character) = content
        .lines()
        .enumerate()
        .find_map(|(i, l)| {
            l.find(&needle)
                .map(|c| (i as u32, (c + needle.len()) as u32))
        })
        .unwrap_or_else(|| panic!("marker {marker} not found in the fixture"));
    let hover = backend
        .handle_hover(uri, content, Position { line, character })
        .unwrap_or_else(|| panic!("no hover at marker {marker}"));
    let HoverContents::Markup(markup) = &hover.contents else {
        panic!("Expected MarkupContent");
    };
    markup
        .value
        .lines()
        .find_map(|l| l.split_once(" = ").map(|(_, ty)| ty.trim().to_string()))
        .unwrap_or_else(|| panic!("no type in hover at marker {marker}: {}", markup.value))
}

/// Assert the marked type of every `(marker, expected)` pair in one file.
fn assert_marked_types(content: &str, expected: &[(&str, &str)]) {
    let backend = create_test_backend_with_full_stubs();
    let uri = "file:///array_key_types.php";
    backend.update_ast(uri, content);
    for (marker, want) in expected {
        assert_eq!(
            &type_at_marker(&backend, uri, content, marker),
            want,
            "marker {marker}"
        );
    }
}

/// A `@phpstan-type` alias names an array type like any other, so iterating
/// a variable typed through one has to see the array behind the alias.
/// Resolving the variable straight out of scope used to skip the expansion
/// that every other branch of the same function performs.
#[test]
fn foreach_over_an_aliased_array_narrows_the_key() {
    let content = r#"<?php
/**
 * @phpstan-type LinesToIgnore array<string, array<int, string>>
 */
class Analyser
{
    /** @param LinesToIgnore $lines */
    public function viaAlias(array $lines): void
    {
        foreach ($lines as $file => $inner) {
            echo /*ALIAS*/$file;
            echo count($inner);
        }
    }

    /** @param array<string, array<int, string>> $plain */
    public function spelledOut(array $plain): void
    {
        foreach ($plain as $file => $inner) {
            echo /*PLAIN*/$file;
            echo count($inner);
        }
    }
}
"#;
    assert_marked_types(content, &[("ALIAS", "string"), ("PLAIN", "string")]);
}

/// A hole in a destructuring pattern names nothing but still consumes its
/// position, so everything after it shifts along.
#[test]
fn a_skipped_destructuring_position_shifts_the_rest() {
    let content = r#"<?php
class Aaa {}
class Bbb {}
class Ccc {}

/** @return array{Aaa, Bbb, Ccc} */
function triple(): array { return [new Aaa(), new Bbb(), new Ccc()]; }

/** @return list<array{Aaa, Bbb}> */
function pairs(): array { return []; }

function probe(): void
{
    [$first, , ] = triple();
    echo /*FIRST*/$first->foo;
    [, $second, ] = triple();
    echo /*SECOND*/$second->foo;
    [, , $third] = triple();
    echo /*THIRD*/$third->foo;

    foreach (pairs() as [, $right]) {
        echo /*RIGHT*/$right->foo;
    }
}
"#;
    assert_marked_types(
        content,
        &[
            ("FIRST", "Aaa"),
            ("SECOND", "Bbb"),
            ("THIRD", "Ccc"),
            ("RIGHT", "Bbb"),
        ],
    );
}

/// `array_map` keeps the input's keys, so an input that never named its key
/// type produces a result that cannot name one either — `array<T>`, not the
/// `array<int, T>` that would claim keys the input never promised.
#[test]
fn array_map_over_an_open_key_domain_keeps_it_open() {
    let content = r#"<?php
class Tag {}
class Alias {}

class Holder
{
    /** @return array<Tag> */
    public function openKeys(): array { return []; }

    /** @return array<int, Tag> */
    public function intKeys(): array { return []; }

    /** @return array<string, Tag> */
    public function stringKeys(): array { return []; }

    /** @return list<Tag> */
    public function listKeys(): array { return []; }

    public function probe(): void
    {
        $open = array_map(fn ($t) => new Alias(), $this->openKeys());
        echo /*OPEN*/$open;
        $ints = array_map(fn ($t) => new Alias(), $this->intKeys());
        echo /*INTS*/$ints;
        $strings = array_map(fn ($t) => new Alias(), $this->stringKeys());
        echo /*STRINGS*/$strings;
        $sequential = array_map(fn ($t) => new Alias(), $this->listKeys());
        echo /*LIST*/$sequential;
    }
}
"#;
    assert_marked_types(
        content,
        &[
            ("OPEN", "array<Alias>"),
            ("INTS", "array<int, Alias>"),
            ("STRINGS", "array<string, Alias>"),
            ("LIST", "list<Alias>"),
        ],
    );
}

/// The key-reading builtins answer from whatever pinned the input's keys
/// down: an array shape, an element read out of a nested array, or an
/// `array_fill_keys` call that turned a list of names into keys.
#[test]
fn array_keys_reports_the_key_type_of_every_pinned_input() {
    let content = r#"<?php
/**
 * @param array<string, array<string, int>> $nested
 * @param list<string> $names
 * @param array<Tag> $openKeys
 */
function probe(array $nested, array $names, array $openKeys): void
{
    $shape = ['alpha' => 1, 'beta' => 2];
    $fromShape = array_keys($shape);
    echo /*SHAPE*/$fromShape;

    $fromDim = array_keys($nested['old']);
    echo /*DIM*/$fromDim;

    $fromFilled = array_keys(array_fill_keys($names, true));
    echo /*FILLED*/$fromFilled;

    $fromOpen = array_keys($openKeys);
    echo /*OPEN*/$fromOpen;
}
class Tag {}
"#;
    assert_marked_types(
        content,
        &[
            ("SHAPE", "list<string>"),
            ("DIM", "list<string>"),
            ("FILLED", "list<string>"),
            // `array<T>` says nothing about its keys, so the only honest
            // answer is the whole key domain PHP allows.
            ("OPEN", "list<array-key>"),
        ],
    );
}

/// An argument the text-driven resolver cannot read (the array-union `+`,
/// which it has no rule for) still binds `array_keys()`'s key template,
/// because the binding falls back to the type the walker resolved for that
/// very expression.
#[test]
fn array_keys_reads_an_argument_only_the_walker_can_resolve() {
    let content = r#"<?php
class Holder {}
/**
 * @param array<string, Holder> $a
 * @param array<string, Holder> $b
 * @param array<int, Holder> $ints
 */
function probe(array $a, array $b, array $ints): void
{
    foreach (array_keys($a + $b) as $key) {
        echo /*MERGED*/$key;
    }
    foreach (array_keys($a + $ints) as $mixed) {
        echo /*MIXED*/$mixed;
    }
}
"#;
    assert_marked_types(content, &[("MERGED", "string"), ("MIXED", "string|int")]);
}

/// A single-quoted key holding a backslash is the characters it spells:
/// `'~\n~'` is a four-character string, not the newline a double-quoted
/// literal would decode, so nothing about it can be an integer key.
#[test]
fn a_backslash_in_a_single_quoted_array_key_leaves_it_a_string_key() {
    let content = r#"<?php
function probe(): void
{
    $replacements = ['~\n~' => '|n', '~\r~' => '|r'];
    foreach (array_keys($replacements) as $key) {
        echo /*KEY*/$key;
    }

    $decoded = ["\x38" => 'eight'];
    foreach (array_keys($decoded) as $index) {
        echo /*DECODED*/$index;
    }
}
"#;
    assert_marked_types(content, &[("KEY", "string"), ("DECODED", "int")]);
}

/// A key type nobody wrote down is benevolent: `int|string` here is PHP's
/// whole key domain standing in for an unknown, not a union the array
/// declared, so holding a call to both branches of it is a false positive.
#[test]
fn an_unknown_foreach_key_satisfies_a_single_branch_parameter() {
    let content = r#"<?php
declare(strict_types=1);

function takesString(string $s): void {}

function probe(array $bare): void
{
    foreach ($bare as $key => $value) {
        takesString($key);
        echo $value;
    }
}

/** @param int|string $declared */
function spelledOut($declared): void
{
    takesString($declared);
}
"#;
    let backend = create_test_backend_with_full_stubs();
    let uri = "file:///benevolent_foreach_key.php";
    backend.update_ast(uri, content);
    let mut diagnostics = Vec::new();
    backend.collect_argument_type_diagnostics(uri, content, &mut diagnostics);
    let messages: Vec<String> = diagnostics
        .into_iter()
        .map(|d| format!("{}: {}", d.range.start.line, d.message))
        .collect();
    assert!(
        messages.iter().any(|m| m.starts_with("16:")),
        "a declared `int|string` still has to satisfy both branches: {messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.starts_with("8:")),
        "an unknown foreach key must not be held to both branches: {messages:?}"
    );
}

/// An array the walk watched being built inside a loop keeps its element
/// type when a later iteration reads it back through a destructuring
/// `foreach`. The first walk of the outer loop reaches the inner `foreach`
/// before anything has been written to the accumulator, and the element
/// types that walk cannot resolve used to be unioned into the accumulator
/// for good, so the tuple slots came back `mixed` however many times the
/// loop was re-walked.
#[test]
fn a_loop_carried_tuple_slot_keeps_the_type_the_write_put_there() {
    let content = r#"<?php
class Trin
{
    public function and(Trin $other): Trin { return $this; }
}
class Ty
{
    public function isConstant(): bool { return true; }
}

function makeTrin(): Trin { return new Trin(); }

/** @param list<Ty> $inputs */
function build(array $inputs): void
{
    $offsetTypes = [];
    foreach ($inputs as $input) {
        if ($input->isConstant()) {
            $offsetTypes['k'] = [makeTrin(), $input];
        } else {
            foreach ($offsetTypes as $key => [$carried, $value]) {
                echo /*CARRIED*/$carried->foo;
                $offsetTypes[$key] = [$carried->and(makeTrin()), $input];
            }
        }
    }

    foreach ($offsetTypes as $key => [$hasOffsetValue, $offsetType]) {
        echo /*SLOT_ZERO*/$hasOffsetValue->foo;
        echo /*SLOT_ONE*/$offsetType->foo;
    }
}
"#;
    assert_marked_types(
        content,
        &[
            ("CARRIED", "Trin"),
            ("SLOT_ZERO", "Trin"),
            ("SLOT_ONE", "Ty"),
        ],
    );
}

/// A write through a written-out index updates that one slot of a
/// tuple-style shape. Folding it into the generic key/value pair instead
/// unioned every slot together, so reading any single slot back gave the
/// union of all of them.
#[test]
fn a_write_to_one_tuple_slot_leaves_the_others_alone() {
    let content = r#"<?php
class Expr {}
class Term {}

/** @param list<Term> $terms
 *  @param list<Term> $more
 *  @return list<Term> */
function conjoin(array $terms, array $more): array { return $terms; }

/**
 * @param list<Expr> $nodes
 * @param list<Term> $terms
 */
function merge(array $nodes, array $terms, string $key): void
{
    $alternatives = [];
    foreach ($nodes as $node) {
        if (!isset($alternatives[$key])) {
            $alternatives[$key] = [$node, $terms];
            continue;
        }
        $alternatives[$key][1] = conjoin($alternatives[$key][1], $terms);
    }

    // Indexing straight through both dimensions has to select the slot
    // rather than union the tuple together.
    $slotZero = $alternatives[$key][0];
    echo /*SLOT_ZERO*/$slotZero->foo;
    $slotOne = $alternatives[$key][1];
    echo count(/*SLOT_ONE*/$slotOne);
}
"#;
    assert_marked_types(
        content,
        &[("SLOT_ZERO", "Expr"), ("SLOT_ONE", "list<Term>")],
    );
}

/// `isset($arr['k'])` on an array that may also be the empty `[]` proves
/// the key is there, so a member access on that offset resolves against
/// the value type. A guaranteed-miss alternative used to contribute
/// `mixed` to the offset's type, and `mixed` survives the null strip the
/// check exists to license, so the access was reported unresolvable.
#[test]
fn isset_on_a_possibly_empty_array_types_the_offset_it_guards() {
    let content = r#"<?php
class Ty {
    public function describe(): string { return ''; }
}

class Helper {
    /** @return array<string, ?Ty> */
    private function getOptions(bool $flag): array { return []; }

    public function probe(bool $hasOptions): string
    {
        $options = $hasOptions ? $this->getOptions($hasOptions) : [];
        if (isset($options['default'])) {
            $defaultType = $options['default'];
        } else {
            $defaultType = new Ty();
        }
        return $defaultType->describe();
    }
}
"#;
    let backend = create_test_backend_with_full_stubs();
    {
        let mut cfg = backend.config();
        cfg.diagnostics.unresolved_member_access = Some(true);
        backend.set_config(cfg);
    }
    let uri = "file:///isset_possibly_empty.php";
    backend.update_ast(uri, content);
    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri, content, &mut diagnostics);
    let unresolved: Vec<String> = diagnostics
        .into_iter()
        .filter(|d| {
            d.code.as_ref().is_some_and(
                |c| matches!(c, NumberOrString::String(s) if s == "unresolved_member_access"),
            )
        })
        .map(|d| d.message)
        .collect();
    assert!(
        unresolved.is_empty(),
        "the guarded offset resolves to Ty: {unresolved:?}"
    );
}
