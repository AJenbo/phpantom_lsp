//! Integration tests for typing a property read through the Reflection API.
//!
//! `ReflectionClass::getProperty()` returns a bare `ReflectionProperty` and
//! `ReflectionProperty::getValue()` a bare `mixed`, so a reflected read used
//! to lose the type the property declares. It is recoverable whenever the
//! reflected class is known and the property name is a literal, which is the
//! shape reflection-based accessors are written in.

use crate::common::create_test_backend_with_full_stubs;
use phpantom_lsp::Backend;
use tower_lsp::lsp_types::*;

/// The resolved type of the assignment on the line that assigns `var`, read
/// off the hover response.
fn assigned_type(backend: &Backend, uri: &str, content: &str, var: &str) -> String {
    let needle = format!("{var} = ");
    let line = content
        .lines()
        .position(|l| l.trim_start().starts_with(&needle))
        .unwrap_or_else(|| panic!("no assignment to {var} in the fixture")) as u32;
    let indent = content
        .lines()
        .nth(line as usize)
        .map_or(0, |l| (l.len() - l.trim_start().len() + 1) as u32);
    let hover = backend
        .handle_hover(
            uri,
            content,
            Position {
                line,
                character: indent,
            },
        )
        .unwrap_or_else(|| panic!("no hover on the assignment to {var}"));
    let HoverContents::Markup(markup) = &hover.contents else {
        panic!("Expected MarkupContent");
    };
    markup
        .value
        .lines()
        .find_map(|l| l.split_once(" = ").map(|(_, ty)| ty.trim().to_string()))
        .unwrap_or_else(|| panic!("no assignment in hover for {var}: {}", markup.value))
}

fn assert_assigned_types(content: &str, expected: &[(&str, &str)]) {
    let backend = create_test_backend_with_full_stubs();
    let uri = "file:///reflection_property_types.php";
    backend.update_ast(uri, content);
    for (var, want) in expected {
        assert_eq!(&assigned_type(&backend, uri, content, var), want, "{var}");
    }
}

const FIXTURE: &str = r#"<?php
class Shell {
    const VERSION = 'v1.0.0';
}
class BaseConfiguration {
    protected ?Shell $inheritedShell = null;
}
class Configuration extends BaseConfiguration {
    private ?Shell $shell = null;
    private int $verbosity = 0;
    private $untyped = null;
}
function probe(Configuration $config, string $dynamicName, $unknown): void {
    $reflObject = new \ReflectionObject($config);
    $reflClass = new \ReflectionClass(Configuration::class);
    $reflNamed = new \ReflectionClass('Configuration');

    $property = $reflObject->getProperty('shell');
    $shellValue = $property->getValue($config);
    $verbosityValue = $reflClass->getProperty('verbosity')->getValue($config);
    $namedValue = $reflNamed->getProperty('shell')->getValue($config);
    $inheritedValue = $reflObject->getProperty('inheritedShell')->getValue($config);

    $dynamicValue = $reflObject->getProperty($dynamicName)->getValue($config);
    $untypedValue = $reflObject->getProperty('untyped')->getValue($config);
    $absentValue = $reflObject->getProperty('noSuchProperty')->getValue($config);

    $reflUnknown = new \ReflectionObject($unknown);
    $unknownValue = $reflUnknown->getProperty('shell')->getValue($unknown);
}
"#;

/// `ReflectionObject` is `ReflectionClass` narrowed to an instance, but
/// phpstorm-stubs give it neither the `@template` nor the `@extends`, so it
/// used to forget the class it reflects.
#[test]
fn reflection_object_carries_the_class_it_reflects() {
    assert_assigned_types(
        FIXTURE,
        &[
            ("$reflObject", "ReflectionObject<Configuration>"),
            ("$reflClass", "ReflectionClass<Configuration>"),
            ("$reflNamed", "ReflectionClass<Configuration>"),
        ],
    );
}

/// A reflected read types as the property does, whether the property is
/// declared on the reflected class or inherited, and whichever spelling
/// produced the reflection.
#[test]
fn a_reflected_property_read_types_as_the_property_declares() {
    assert_assigned_types(
        FIXTURE,
        &[
            ("$property", "ReflectionProperty<Configuration, 'shell'>"),
            ("$shellValue", "?Shell"),
            ("$verbosityValue", "int"),
            ("$namedValue", "?Shell"),
            ("$inheritedValue", "?Shell"),
        ],
    );
}

/// Everything the rule cannot decide keeps `getValue()`'s declared `mixed`:
/// a property name that is not a literal, a property with no declared type,
/// a name that matches no property, and a reflected value whose class is
/// unknown.
#[test]
fn an_undecidable_reflected_read_stays_mixed() {
    assert_assigned_types(
        FIXTURE,
        &[
            ("$dynamicValue", "mixed"),
            ("$untypedValue", "mixed"),
            ("$absentValue", "mixed"),
            ("$unknownValue", "mixed"),
        ],
    );
}
