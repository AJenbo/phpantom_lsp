use phpantom_lsp::Backend;
use std::collections::HashMap;
use tower_lsp::lsp_types::*;

// ─── Word Extraction Tests ──────────────────────────────────────────────────

#[test]
fn test_extract_word_simple_class_name() {
    let content = "<?php\nclass Foo {}\n";
    // Cursor on "Foo"
    let pos = Position {
        line: 1,
        character: 7,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("Foo"));
}

#[test]
fn test_extract_word_fully_qualified_name() {
    let content = "<?php\nuse Illuminate\\Database\\Eloquent\\Model;\n";
    // Cursor somewhere inside the FQN
    let pos = Position {
        line: 1,
        character: 20,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(
        word.as_deref(),
        Some("Illuminate\\Database\\Eloquent\\Model")
    );
}

#[test]
fn test_extract_word_at_end_of_name() {
    let content = "<?php\nnew Exception();\n";
    // Cursor right after "Exception" (on the `(`)
    let pos = Position {
        line: 1,
        character: 13,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("Exception"));
}

#[test]
fn test_extract_word_class_reference() {
    let content = "<?php\n$x = OrderProductCollection::class;\n";
    // Cursor on "OrderProductCollection"
    let pos = Position {
        line: 1,
        character: 10,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("OrderProductCollection"));
}

#[test]
fn test_extract_word_type_hint() {
    let content = "<?php\npublic function order(): BelongsTo {}\n";
    // Cursor on "BelongsTo"
    let pos = Position {
        line: 1,
        character: 28,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("BelongsTo"));
}

#[test]
fn test_extract_word_on_whitespace_returns_none() {
    let content = "<?php\n   \n";
    let pos = Position {
        line: 1,
        character: 1,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert!(word.is_none());
}

#[test]
fn test_extract_word_leading_backslash_stripped() {
    let content = "<?php\nnew \\Exception();\n";
    // Cursor on "\\Exception"
    let pos = Position {
        line: 1,
        character: 6,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("Exception"));
}

#[test]
fn test_extract_word_past_end_of_file_returns_none() {
    let content = "<?php\n";
    let pos = Position {
        line: 10,
        character: 0,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert!(word.is_none());
}

#[test]
fn test_extract_word_parameter_type_hint() {
    let content = "<?php\npublic function run(IShoppingCart $cart): void {}\n";
    // Cursor on "IShoppingCart"
    let pos = Position {
        line: 1,
        character: 24,
    };
    let word = Backend::extract_word_at_position(content, pos);
    assert_eq!(word.as_deref(), Some("IShoppingCart"));
}

// ─── FQN Resolution Tests ──────────────────────────────────────────────────

#[test]
fn test_resolve_to_fqn_via_use_map() {
    let mut use_map = HashMap::new();
    use_map.insert(
        "BelongsTo".to_string(),
        "Illuminate\\Database\\Eloquent\\Relations\\BelongsTo".to_string(),
    );

    let fqn = Backend::resolve_to_fqn("BelongsTo", &use_map, &None);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Relations\\BelongsTo");
}

#[test]
fn test_resolve_to_fqn_via_namespace() {
    let use_map = HashMap::new();
    let namespace = Some("Luxplus\\Core\\Database\\Model\\Orders".to_string());

    let fqn = Backend::resolve_to_fqn("OrderProductCollection", &use_map, &namespace);
    assert_eq!(
        fqn,
        "Luxplus\\Core\\Database\\Model\\Orders\\OrderProductCollection"
    );
}

#[test]
fn test_resolve_to_fqn_already_qualified() {
    let use_map = HashMap::new();
    let fqn = Backend::resolve_to_fqn("Illuminate\\Database\\Eloquent\\Model", &use_map, &None);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Model");
}

#[test]
fn test_resolve_to_fqn_partial_qualified_with_use_map() {
    let mut use_map = HashMap::new();
    use_map.insert(
        "Eloquent".to_string(),
        "Illuminate\\Database\\Eloquent".to_string(),
    );

    let fqn = Backend::resolve_to_fqn("Eloquent\\Model", &use_map, &None);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Model");
}

#[test]
fn test_resolve_to_fqn_bare_name_no_context() {
    let use_map = HashMap::new();
    let fqn = Backend::resolve_to_fqn("Exception", &use_map, &None);
    assert_eq!(fqn, "Exception");
}

#[test]
fn test_resolve_to_fqn_use_map_takes_precedence_over_namespace() {
    let mut use_map = HashMap::new();
    use_map.insert(
        "HasFactory".to_string(),
        "Illuminate\\Database\\Eloquent\\Factories\\HasFactory".to_string(),
    );
    let namespace = Some("App\\Models".to_string());

    let fqn = Backend::resolve_to_fqn("HasFactory", &use_map, &namespace);
    assert_eq!(fqn, "Illuminate\\Database\\Eloquent\\Factories\\HasFactory");
}

// ─── Find Definition Position Tests ─────────────────────────────────────────

#[test]
fn test_find_definition_position_class() {
    let content = "<?php\n\nclass Customer {\n    public function name() {}\n}\n";
    let pos = Backend::find_definition_position(content, "Customer");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_interface() {
    let content = "<?php\n\ninterface Loggable {\n    public function log(): void;\n}\n";
    let pos = Backend::find_definition_position(content, "Loggable");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_trait() {
    let content = "<?php\n\ntrait HasFactory {\n}\n";
    let pos = Backend::find_definition_position(content, "HasFactory");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_enum() {
    let content = "<?php\n\nenum LineItemType: string {\n    case Product = 'product';\n}\n";
    let pos = Backend::find_definition_position(content, "LineItemType");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_abstract_class() {
    let content = "<?php\n\nabstract class BaseModel {\n}\n";
    let pos = Backend::find_definition_position(content, "BaseModel");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    // "class" starts after "abstract "
    assert_eq!(pos.character, 9);
}

#[test]
fn test_find_definition_position_final_class() {
    let content = "<?php\n\nfinal class ImmutableValue {\n}\n";
    let pos = Backend::find_definition_position(content, "ImmutableValue");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 2);
    assert_eq!(pos.character, 6);
}

#[test]
fn test_find_definition_position_not_found() {
    let content = "<?php\n\nclass Foo {}\n";
    let pos = Backend::find_definition_position(content, "Bar");
    assert!(pos.is_none());
}

#[test]
fn test_find_definition_position_no_partial_match() {
    let content = "<?php\n\nclass FooBar {}\n";
    // Should NOT match "Foo" inside "FooBar"
    let pos = Backend::find_definition_position(content, "Foo");
    assert!(pos.is_none());
}

#[test]
fn test_find_definition_position_skips_line_comment() {
    let content = concat!(
        "<?php\n",
        "// class AdminUser extends Model\n",
        "\n",
        "class AdminUser {\n",
        "}\n",
    );
    let pos = Backend::find_definition_position(content, "AdminUser");
    assert!(pos.is_some());
    let pos = pos.unwrap();
    assert_eq!(pos.line, 3, "Should skip the commented-out class on line 1");
    assert_eq!(pos.character, 0);
}

#[test]
fn test_find_definition_position_skips_hash_comment() {
    let content = concat!(
        "<?php\n",
        "# class Foo extends Bar\n",
        "\n",
        "class Foo {\n",
        "}\n",
    );
    let pos = Backend::find_definition_position(content, "Foo");
    assert!(pos.is_some());
    assert_eq!(
        pos.unwrap().line,
        3,
        "Should skip the #-commented class on line 1"
    );
}

#[test]
fn test_find_definition_position_skips_block_comment() {
    let content = concat!(
        "<?php\n",
        "/* class Widget {} */\n",
        "\n",
        "class Widget {\n",
        "}\n",
    );
    let pos = Backend::find_definition_position(content, "Widget");
    assert!(pos.is_some());
    assert_eq!(
        pos.unwrap().line,
        3,
        "Should skip the block-commented class on line 1"
    );
}

#[test]
fn test_find_definition_position_skips_multiline_block_comment() {
    let content = concat!(
        "<?php\n",
        "/*\n",
        " * class Order extends Model\n",
        " */\n",
        "\n",
        "class Order {\n",
        "}\n",
    );
    let pos = Backend::find_definition_position(content, "Order");
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 5, "Should skip multi-line block comment");
}

#[test]
fn test_find_definition_position_skips_docblock() {
    let content = concat!(
        "<?php\n",
        "/**\n",
        " * class Response extends BaseResponse\n",
        " */\n",
        "\n",
        "class Response {\n",
        "}\n",
    );
    let pos = Backend::find_definition_position(content, "Response");
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 5, "Should skip docblock comment");
}

#[test]
fn test_find_definition_position_inline_comment_after_code() {
    // The class declaration is real code; the comment is after it.
    let content = concat!("<?php\n", "class Config { // class Settings\n", "}\n",);
    let pos = Backend::find_definition_position(content, "Config");
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 1);

    // "Settings" appears only inside the comment — should not match.
    let pos2 = Backend::find_definition_position(content, "Settings");
    assert!(
        pos2.is_none(),
        "Should not match class name inside trailing comment"
    );
}

#[test]
fn test_find_definition_position_only_in_comment_returns_none() {
    let content = concat!("<?php\n", "// class Ghost extends Model\n",);
    let pos = Backend::find_definition_position(content, "Ghost");
    assert!(
        pos.is_none(),
        "Should return None when class only exists in a comment"
    );
}

#[test]
fn test_find_definition_position_with_namespace() {
    let content = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "\n",
        "class User {\n",
        "}\n",
    );
    let pos = Backend::find_definition_position(content, "User");
    assert!(pos.is_some());
    assert_eq!(pos.unwrap().line, 3);
}
