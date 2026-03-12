use super::*;

#[test]
fn extract_description_simple() {
    let doc = "/** This is a simple description. */";
    assert_eq!(
        extract_docblock_description(Some(doc)),
        Some("This is a simple description.".to_string())
    );
}

#[test]
fn extract_description_multiline() {
    let doc = "/**\n * First line.\n * Second line.\n * @param string $x\n */";
    assert_eq!(
        extract_docblock_description(Some(doc)),
        Some("First line.\nSecond line.".to_string())
    );
}

#[test]
fn extract_description_none_when_only_tags() {
    let doc = "/**\n * @return string\n */";
    assert_eq!(extract_docblock_description(Some(doc)), None);
}

#[test]
fn extract_description_none_when_empty() {
    assert_eq!(extract_docblock_description(None), None);
}

#[test]
fn namespace_line_with_namespace() {
    assert_eq!(
        namespace_line(&Some("App\\Models".to_string())),
        "namespace App\\Models;\n"
    );
}

#[test]
fn namespace_line_without_namespace() {
    assert_eq!(namespace_line(&None), "");
}

#[test]
fn format_params_empty() {
    assert_eq!(format_params(&[]), "");
}

#[test]
fn format_params_with_types() {
    let params = vec![
        ParameterInfo {
            name: "$name".to_string(),
            type_hint: Some("string".to_string()),
            native_type_hint: Some("string".to_string()),
            description: None,
            default_value: None,
            is_required: true,
            is_variadic: false,
            is_reference: false,
            closure_this_type: None,
        },
        ParameterInfo {
            name: "$age".to_string(),
            type_hint: Some("int".to_string()),
            native_type_hint: Some("int".to_string()),
            description: None,
            default_value: None,
            is_required: false,
            is_variadic: false,
            is_reference: false,
            closure_this_type: None,
        },
    ];
    assert_eq!(format_params(&params), "string $name, int $age = ...");
}

#[test]
fn format_params_variadic() {
    let params = vec![ParameterInfo {
        name: "$items".to_string(),
        type_hint: Some("string".to_string()),
        native_type_hint: Some("string".to_string()),
        description: None,
        default_value: None,
        is_required: false,
        is_variadic: true,
        is_reference: false,
        closure_this_type: None,
    }];
    assert_eq!(format_params(&params), "string ...$items");
}

#[test]
fn format_params_reference() {
    let params = vec![ParameterInfo {
        name: "$arr".to_string(),
        type_hint: Some("array".to_string()),
        native_type_hint: Some("array".to_string()),
        description: None,
        default_value: None,
        is_required: true,
        is_variadic: false,
        is_reference: true,
        closure_this_type: None,
    }];
    assert_eq!(format_params(&params), "array &$arr");
}

#[test]
fn format_visibility_all() {
    assert_eq!(format_visibility(Visibility::Public), "public ");
    assert_eq!(format_visibility(Visibility::Protected), "protected ");
    assert_eq!(format_visibility(Visibility::Private), "private ");
}

// ─── short_name tests ───────────────────────────────────────────────────────

#[test]
fn short_name_plain() {
    assert_eq!(short_name("User"), "User");
}

#[test]
fn short_name_namespaced() {
    assert_eq!(short_name("App\\Models\\User"), "User");
}

#[test]
fn short_name_leading_backslash() {
    assert_eq!(short_name("\\App\\Models\\User"), "User");
}

#[test]
fn short_name_scalar() {
    assert_eq!(short_name("string"), "string");
}

#[test]
fn short_name_single_namespace() {
    assert_eq!(short_name("Demo\\Brush"), "Brush");
}

// ─── types_equivalent tests ─────────────────────────────────────────────────

#[test]
fn types_equivalent_identical_strings() {
    assert!(types_equivalent("Brush", "Brush"));
}

#[test]
fn types_equivalent_fqn_vs_short() {
    assert!(types_equivalent("Brush", "Demo\\Brush"));
    assert!(types_equivalent("Demo\\Brush", "Brush"));
}

#[test]
fn types_equivalent_leading_backslash_fqn() {
    assert!(types_equivalent("Brush", "\\Demo\\Brush"));
    assert!(types_equivalent("\\Demo\\Brush", "Brush"));
}

#[test]
fn types_equivalent_nullable() {
    assert!(types_equivalent("?Brush", "?Demo\\Brush"));
    assert!(types_equivalent("?Demo\\Brush", "?Brush"));
}

#[test]
fn types_equivalent_union_with_null() {
    assert!(types_equivalent("Brush|null", "Demo\\Brush|null"));
    assert!(types_equivalent("null|Brush", "Demo\\Brush|null"));
}

#[test]
fn types_equivalent_different_types() {
    assert!(!types_equivalent("array", "list<User>"));
}

#[test]
fn types_equivalent_different_component_count() {
    assert!(!types_equivalent("Brush", "Brush|null"));
}

#[test]
fn types_equivalent_scalars() {
    assert!(types_equivalent("string", "string"));
    assert!(!types_equivalent("string", "int"));
}

#[test]
fn types_equivalent_intersection() {
    assert!(types_equivalent(
        "Countable&Traversable",
        "Countable&Traversable"
    ));
    assert!(types_equivalent(
        "Countable&Traversable",
        "App\\Countable&App\\Traversable"
    ));
}

#[test]
fn types_equivalent_different_short_names() {
    assert!(!types_equivalent("Brush", "Demo\\Canvas"));
}

// ─── shorten_type_string tests ──────────────────────────────────────────────

#[test]
fn shorten_type_string_plain_class() {
    assert_eq!(shorten_type_string("App\\Models\\User"), "User");
}

#[test]
fn shorten_type_string_already_short() {
    assert_eq!(shorten_type_string("User"), "User");
}

#[test]
fn shorten_type_string_scalar() {
    assert_eq!(shorten_type_string("string"), "string");
}

#[test]
fn shorten_type_string_nullable() {
    assert_eq!(shorten_type_string("?App\\Models\\User"), "?User");
}

#[test]
fn shorten_type_string_union() {
    assert_eq!(shorten_type_string("App\\Models\\User|null"), "User|null");
}

#[test]
fn shorten_type_string_generic() {
    assert_eq!(shorten_type_string("list<App\\Models\\User>"), "list<User>");
}

#[test]
fn shorten_type_string_nested_generic() {
    assert_eq!(
        shorten_type_string("array<int, App\\Collection<string, App\\Models\\User>>"),
        "array<int, Collection<string, User>>"
    );
}

#[test]
fn shorten_type_string_intersection() {
    assert_eq!(
        shorten_type_string("App\\Countable&App\\Traversable"),
        "Countable&Traversable"
    );
}

#[test]
fn shorten_type_string_leading_backslash() {
    assert_eq!(shorten_type_string("\\App\\Models\\User"), "User");
}

#[test]
fn shorten_type_string_object_shape() {
    assert_eq!(
        shorten_type_string("object{name: string, user: App\\Models\\User}"),
        "object{name: string, user: User}"
    );
}

#[test]
fn shorten_type_string_mixed_union_with_generics() {
    assert_eq!(
        shorten_type_string("App\\Collection<int, App\\Models\\User>|null"),
        "Collection<int, User>|null"
    );
}

#[test]
fn shorten_type_string_parenthesized_callable_union() {
    assert_eq!(
        shorten_type_string(
            "(\\Closure(static): mixed)|string|array|\\Illuminate\\Contracts\\Database\\Query\\Expression"
        ),
        "(Closure(static): mixed)|string|array|Expression"
    );
}

// ─── split_top_level_union tests ────────────────────────────────────────────

#[test]
fn split_union_simple() {
    assert_eq!(split_top_level_union("Lamp|Faucet"), vec!["Lamp", "Faucet"]);
}

#[test]
fn split_union_single_type() {
    assert_eq!(split_top_level_union("Lamp"), vec!["Lamp"]);
}

#[test]
fn split_union_three_types() {
    assert_eq!(
        split_top_level_union("Lamp|Faucet|Switch"),
        vec!["Lamp", "Faucet", "Switch"]
    );
}

#[test]
fn split_union_with_null() {
    assert_eq!(split_top_level_union("Lamp|null"), vec!["Lamp", "null"]);
}

#[test]
fn split_union_preserves_generics() {
    assert_eq!(
        split_top_level_union("Generator<int, Foo>|null"),
        vec!["Generator<int, Foo>", "null"]
    );
}

#[test]
fn split_union_nested_generics() {
    assert_eq!(
        split_top_level_union("Collection<int, User>|array<string, int>|null"),
        vec!["Collection<int, User>", "array<string, int>", "null"]
    );
}

#[test]
fn split_union_parenthesized_callable() {
    assert_eq!(
        split_top_level_union("(Closure(string): int)|string|array"),
        vec!["(Closure(string): int)", "string", "array"]
    );
}

#[test]
fn split_union_array_shape() {
    assert_eq!(
        split_top_level_union("array{name: string, age: int}|null"),
        vec!["array{name: string, age: int}", "null"]
    );
}

// ─── build_variable_hover_body tests ────────────────────────────────────────

#[test]
fn variable_hover_body_single_type() {
    let body = build_variable_hover_body("$user", "User", &|_| None, None);
    assert_eq!(body, "```php\n<?php\n$user = User\n```");
}

#[test]
fn variable_hover_body_union_splits_into_blocks() {
    let body = build_variable_hover_body("$ambiguous", "Lamp|Faucet", &|_| None, None);
    assert_eq!(
        body,
        "```php\n<?php\n$ambiguous = Lamp\n```\n\n---\n\n```php\n<?php\n$ambiguous = Faucet\n```"
    );
}

#[test]
fn variable_hover_body_union_with_template_line() {
    let body =
        build_variable_hover_body("$item", "Lamp|Faucet", &|_| None, Some("**template** `T`"));
    assert!(body.starts_with("**template** `T`\n\n"));
    assert!(body.contains("$item = Lamp"));
    assert!(body.contains("---"));
    assert!(body.contains("$item = Faucet"));
}

#[test]
fn variable_hover_body_generic_union_not_split() {
    // A single generic type is not split even though it contains `|` inside `<>`.
    let body = build_variable_hover_body("$gen", "Generator<int, Foo>", &|_| None, None);
    assert!(!body.contains("---"));
    assert!(body.contains("$gen = Generator<int, Foo>"));
}

#[test]
fn variable_hover_body_three_way_union() {
    let body = build_variable_hover_body("$x", "A|B|C", &|_| None, None);
    let blocks: Vec<&str> = body.split("\n\n---\n\n").collect();
    assert_eq!(blocks.len(), 3);
    assert!(blocks[0].contains("$x = A"));
    assert!(blocks[1].contains("$x = B"));
    assert!(blocks[2].contains("$x = C"));
}

#[test]
fn variable_hover_body_nullable_class_not_split() {
    // `Foo|null` has only one class-like type, so it should stay in a single block.
    let body = build_variable_hover_body("$x", "Foo|null", &|_| None, None);
    assert!(!body.contains("---"), "Foo|null should not split: {}", body);
    assert!(body.contains("$x = Foo|null"), "got: {}", body);
}

#[test]
fn variable_hover_body_scalar_not_split() {
    let body = build_variable_hover_body("$val", "string", &|_| None, None);
    assert!(!body.contains("---"));
    assert!(body.contains("$val = string"));
}
