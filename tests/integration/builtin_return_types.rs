//! Integration tests for builtins whose return type is decided by an
//! argument.
//!
//! The stubs can only declare the union of every shape a function can return,
//! so a call that provably takes one branch still carries the others. Each
//! test here pins one such function to the branch its arguments select, and
//! checks that a call whose argument cannot be pinned down keeps both.

use crate::common::create_test_backend_with_full_stubs;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

/// Register file content in the backend (sync) and return the hover result
/// at the given (0-based) line and character.
fn hover_at(
    backend: &Backend,
    uri: &str,
    content: &str,
    line: u32,
    character: u32,
) -> Option<Hover> {
    backend.update_ast(uri, content);
    backend.handle_hover(uri, content, Position { line, character })
}

/// The resolved type of the variable assigned on `line` (0-based), read off
/// the hover response.
fn assigned_type(backend: &Backend, uri: &str, content: &str, line: u32) -> String {
    let hover = hover_at(backend, uri, content, line, 6)
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
    let uri = "file:///builtin_return_types.php";
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

/// `pathinfo()` only returns the component array for the all-elements form:
/// every other flag asks for one part and gets a string. `PATHINFO_ALL` is
/// the parameter's default, so the one-argument call takes the array branch.
#[test]
fn pathinfo_returns_a_string_for_a_single_component() {
    const SHAPE: &str =
        "array{dirname: string, basename: string, extension?: string, filename: string}";
    let content = r#"<?php
function probe(string $path, int $flags): void {
    $all = pathinfo($path);
    $allExplicit = pathinfo($path, PATHINFO_ALL);
    $filename = pathinfo($path, PATHINFO_FILENAME);
    $extension = pathinfo($path, \PATHINFO_EXTENSION);
    $unknown = pathinfo($path, $flags);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$all", SHAPE),
            ("$allExplicit", SHAPE),
            ("$filename", "string"),
            ("$extension", "string"),
            ("$unknown", &format!("{SHAPE}|string")),
        ],
    );
}

/// `print_r()` returns the rendered string only when asked to; otherwise it
/// prints and reports that it did. The declared `string|bool` carries a
/// `false` php-src never returns.
#[test]
fn print_r_returns_a_string_only_when_asked_to() {
    let content = r#"<?php
function probe(mixed $value, bool $capture): void {
    $printed = print_r($value);
    $rendered = print_r($value, true);
    $notRendered = print_r($value, false);
    $unknown = print_r($value, $capture);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$printed", "true"),
            ("$rendered", "string"),
            ("$notRendered", "true"),
            ("$unknown", "string|true"),
        ],
    );
}

/// `hrtime(true)` is a number; the `[seconds, nanoseconds]` pair is what the
/// default form returns.
#[test]
fn hrtime_follows_its_as_number_argument() {
    let content = r#"<?php
function probe(bool $asNumber): void {
    $number = hrtime(true);
    $pair = hrtime();
    $pairExplicit = hrtime(false);
    $unknown = hrtime($asNumber);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$number", "int|float"),
            ("$pair", "array{int, int}|false"),
            ("$pairExplicit", "array{int, int}|false"),
            ("$unknown", "int|float|array{int, int}|false"),
        ],
    );
}

/// `microtime()`'s stub carries `#[TypeContract(true: 'float', false:
/// 'string')]`, which says exactly which branch each call takes.
#[test]
fn microtime_follows_its_as_float_argument() {
    let content = r#"<?php
function probe(bool $asFloat): void {
    $seconds = microtime(true);
    $text = microtime();
    $unknown = microtime($asFloat);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$seconds", "float"),
            ("$text", "string"),
            ("$unknown", "float|string"),
        ],
    );
}

/// Only the no-argument `getenv()` returns the whole environment.
#[test]
fn getenv_returns_the_environment_only_without_a_name() {
    let content = r#"<?php
function probe(string $name): void {
    $one = getenv('HOME');
    $all = getenv();
    $named = getenv($name);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$one", "string|false"),
            ("$all", "array<string, string>"),
            ("$named", "string|false"),
        ],
    );
}

/// `mb_convert_encoding()` answers in the shape it was handed, like the
/// replace family. An array subject is converted per element, so no error
/// branch survives there.
#[test]
fn mb_convert_encoding_follows_its_subject() {
    let content = r#"<?php
/**
 * @param list<string> $lines
 */
function probe(string $text, array $lines, mixed $anything): void {
    $one = mb_convert_encoding($text, 'UTF-8');
    $many = mb_convert_encoding($lines, 'UTF-8');
    $unknown = mb_convert_encoding($anything, 'UTF-8');
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$one", "string|false"),
            ("$many", "array<array-key, string>"),
            ("$unknown", "array<array-key, string>|string|false"),
        ],
    );
}

/// `abs()` returns the type it was given; the declared `int|float` leaves an
/// `int` argument's result carrying a `float` branch that cannot happen.
#[test]
fn abs_returns_the_type_it_was_given() {
    let content = r#"<?php
function probe(int $i, float $f, mixed $anything): void {
    $whole = abs($i);
    $fractional = abs($f);
    $unknown = abs($anything);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$whole", "int"),
            ("$fractional", "float"),
            ("$unknown", "int|float"),
        ],
    );
}

/// `SimpleXMLElement::asXML()` / `saveXML()` serialise to a string without a
/// filename and report success with one. The declared `string|bool` splits
/// neither result.
#[test]
fn simple_xml_element_serialisers_follow_their_filename() {
    let content = r#"<?php
function probe(\SimpleXMLElement $xml, string $path): void {
    $serialised = $xml->asXML();
    $written = $xml->saveXML('/tmp/out.xml');
    $writtenToPath = $xml->asXML($path);
}
"#;
    assert_assigned_types(
        content,
        &[
            ("$serialised", "string|false"),
            ("$written", "bool"),
            ("$writtenToPath", "bool"),
        ],
    );
}
