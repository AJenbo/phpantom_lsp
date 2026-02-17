//! Unit tests for docblock parsing functions.
//!
//! These tests exercise the public API of `phpantom_lsp::docblock` —
//! tag extraction, type resolution, conditional return types, etc.

use phpantom_lsp::docblock::*;
use phpantom_lsp::types::*;

// ─── @method tag extraction ─────────────────────────────────────────

#[test]
fn method_tag_simple() {
    let doc = "/** @method MockInterface mock(string $abstract) */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "mock");
    assert_eq!(methods[0].return_type.as_deref(), Some("MockInterface"));
    assert!(!methods[0].is_static);
    assert_eq!(methods[0].parameters.len(), 1);
    assert_eq!(methods[0].parameters[0].name, "$abstract");
    assert_eq!(
        methods[0].parameters[0].type_hint.as_deref(),
        Some("string")
    );
    assert!(methods[0].parameters[0].is_required);
}

#[test]
fn method_tag_static() {
    let doc = "/** @method static Decimal getAmountUntilBonusCashIsTriggered() */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "getAmountUntilBonusCashIsTriggered");
    assert_eq!(methods[0].return_type.as_deref(), Some("Decimal"));
    assert!(methods[0].is_static);
    assert!(methods[0].parameters.is_empty());
}

#[test]
fn method_tag_no_return_type() {
    let doc = "/** @method assertDatabaseHas(string $table, array<string, mixed> $data, string $connection = null) */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "assertDatabaseHas");
    assert!(methods[0].return_type.is_none());
    assert_eq!(methods[0].parameters.len(), 3);
    assert_eq!(methods[0].parameters[0].name, "$table");
    assert_eq!(
        methods[0].parameters[0].type_hint.as_deref(),
        Some("string")
    );
    assert!(methods[0].parameters[0].is_required);
    assert_eq!(methods[0].parameters[1].name, "$data");
    assert_eq!(methods[0].parameters[1].type_hint.as_deref(), Some("array"));
    assert!(methods[0].parameters[1].is_required);
    assert_eq!(methods[0].parameters[2].name, "$connection");
    assert_eq!(
        methods[0].parameters[2].type_hint.as_deref(),
        Some("string")
    );
    assert!(!methods[0].parameters[2].is_required);
}

#[test]
fn method_tag_fqn_return_type() {
    let doc = "/** @method \\Mockery\\MockInterface mock(string $abstract) */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(
        methods[0].return_type.as_deref(),
        Some("Mockery\\MockInterface")
    );
}

#[test]
fn method_tag_callable_param() {
    let doc = "/** @method MockInterface mock(string $abstract, callable():mixed $mockDefinition = null) */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].parameters.len(), 2);
    assert_eq!(methods[0].parameters[1].name, "$mockDefinition");
    assert!(!methods[0].parameters[1].is_required);
}

#[test]
fn method_tag_multiple() {
    let doc = concat!(
        "/**\n",
        " * @method \\Mockery\\MockInterface mock(string $abstract, callable():mixed $mockDefinition = null)\n",
        " * @method assertDatabaseHas(string $table, array<string, mixed> $data, string $connection = null)\n",
        " * @method assertDatabaseMissing(string $table, array<string, mixed> $data, string $connection = null)\n",
        " * @method static Decimal getAmountUntilBonusCashIsTriggered()\n",
        " */",
    );
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 4);
    assert_eq!(methods[0].name, "mock");
    assert!(!methods[0].is_static);
    assert_eq!(methods[1].name, "assertDatabaseHas");
    assert!(!methods[1].is_static);
    assert_eq!(methods[2].name, "assertDatabaseMissing");
    assert!(!methods[2].is_static);
    assert_eq!(methods[3].name, "getAmountUntilBonusCashIsTriggered");
    assert!(methods[3].is_static);
}

#[test]
fn method_tag_no_params() {
    let doc = "/** @method string getName() */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "getName");
    assert_eq!(methods[0].return_type.as_deref(), Some("string"));
    assert!(methods[0].parameters.is_empty());
}

#[test]
fn method_tag_nullable_return() {
    let doc = "/** @method ?User findUser(int $id) */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].return_type.as_deref(), Some("?User"));
}

#[test]
fn method_tag_none_when_missing() {
    let doc = "/** @property string $name */";
    let methods = extract_method_tags(doc);
    assert!(methods.is_empty());
}

#[test]
fn method_tag_variadic_param() {
    let doc = "/** @method void addItems(string ...$items) */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].parameters.len(), 1);
    assert!(methods[0].parameters[0].is_variadic);
    assert!(!methods[0].parameters[0].is_required);
}

#[test]
fn method_tag_name_matches_type_keyword() {
    let doc =
        "/** @method static string string(string $key, \\Closure|string|null $default = null) */";
    let methods = extract_method_tags(doc);
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "string");
    assert_eq!(methods[0].return_type.as_deref(), Some("string"));
    assert!(methods[0].is_static);
    assert_eq!(methods[0].parameters.len(), 2);
    assert_eq!(methods[0].parameters[0].name, "$key");
    assert_eq!(
        methods[0].parameters[0].type_hint.as_deref(),
        Some("string")
    );
}

// ─── @property tag extraction ───────────────────────────────────────

#[test]
fn property_tag_simple() {
    let doc = "/** @property Session $session */";
    let props = extract_property_tags(doc);
    assert_eq!(props, vec![("session".to_string(), "Session".to_string())]);
}

#[test]
fn property_tag_nullable() {
    let doc = "/** @property ?int $count */";
    let props = extract_property_tags(doc);
    assert_eq!(props, vec![("count".to_string(), "?int".to_string())]);
}

#[test]
fn property_tag_union_with_null() {
    let doc = "/** @property null|int $latest_id */";
    let props = extract_property_tags(doc);
    assert_eq!(props, vec![("latest_id".to_string(), "int".to_string())]);
}

#[test]
fn property_tag_fqn() {
    let doc = "/** @property \\App\\Models\\User $user */";
    let props = extract_property_tags(doc);
    assert_eq!(
        props,
        vec![("user".to_string(), "App\\Models\\User".to_string())]
    );
}

#[test]
fn property_tag_multiple() {
    let doc = concat!(
        "/**\n",
        " * @property null|int                    $latest_subscription_agreement_id\n",
        " * @property UserMobileVerificationState $mobile_verification_state\n",
        " */",
    );
    let props = extract_property_tags(doc);
    assert_eq!(props.len(), 2);
    assert_eq!(
        props[0],
        (
            "latest_subscription_agreement_id".to_string(),
            "int".to_string()
        )
    );
    assert_eq!(
        props[1],
        (
            "mobile_verification_state".to_string(),
            "UserMobileVerificationState".to_string()
        )
    );
}

#[test]
fn property_tag_read_write_variants() {
    let doc = concat!(
        "/**\n",
        " * @property-read string $name\n",
        " * @property-write int $age\n",
        " */",
    );
    let props = extract_property_tags(doc);
    assert_eq!(props.len(), 2);
    assert_eq!(props[0], ("name".to_string(), "string".to_string()));
    assert_eq!(props[1], ("age".to_string(), "int".to_string()));
}

#[test]
fn property_tag_no_type() {
    let doc = "/** @property $thing */";
    let props = extract_property_tags(doc);
    assert_eq!(props, vec![("thing".to_string(), "".to_string())]);
}

#[test]
fn property_tag_generic_stripped() {
    let doc = "/** @property Collection<int, Model> $items */";
    let props = extract_property_tags(doc);
    assert_eq!(props, vec![("items".to_string(), "Collection".to_string())]);
}

#[test]
fn property_tag_none_when_missing() {
    let doc = "/** @return Foo */";
    let props = extract_property_tags(doc);
    assert!(props.is_empty());
}

// ── extract_return_type (skips conditionals) ────────────────────────

#[test]
fn return_type_conditional_is_skipped() {
    let doc = concat!(
        "/**\n",
        " * @return ($abstract is class-string<TClass> ? TClass : mixed)\n",
        " */",
    );
    assert_eq!(extract_return_type(doc), None);
}

// ── extract_return_type ─────────────────────────────────────────────

#[test]
fn return_type_simple() {
    let doc = "/** @return Application */";
    assert_eq!(extract_return_type(doc), Some("Application".into()));
}

#[test]
fn return_type_fqn() {
    let doc = "/** @return \\Illuminate\\Session\\Store */";
    assert_eq!(
        extract_return_type(doc),
        Some("Illuminate\\Session\\Store".into())
    );
}

#[test]
fn return_type_nullable() {
    let doc = "/** @return ?Application */";
    assert_eq!(extract_return_type(doc), Some("?Application".into()));
}

#[test]
fn return_type_with_description() {
    let doc = "/** @return Application The main app instance */";
    assert_eq!(extract_return_type(doc), Some("Application".into()));
}

#[test]
fn return_type_multiline() {
    let doc = concat!(
        "/**\n",
        " * Some method.\n",
        " *\n",
        " * @param string $key\n",
        " * @return \\Illuminate\\Session\\Store\n",
        " */",
    );
    assert_eq!(
        extract_return_type(doc),
        Some("Illuminate\\Session\\Store".into())
    );
}

#[test]
fn return_type_none_when_missing() {
    let doc = "/** This is a docblock without a return tag */";
    assert_eq!(extract_return_type(doc), None);
}

#[test]
fn return_type_nullable_union() {
    let doc = "/** @return Application|null */";
    assert_eq!(extract_return_type(doc), Some("Application".into()));
}

#[test]
fn return_type_generic_stripped() {
    let doc = "/** @return Collection<int, Model> */";
    assert_eq!(extract_return_type(doc), Some("Collection".into()));
}

// ── extract_var_type ────────────────────────────────────────────────

#[test]
fn var_type_simple() {
    let doc = "/** @var Session */";
    assert_eq!(extract_var_type(doc), Some("Session".into()));
}

#[test]
fn var_type_fqn() {
    let doc = "/** @var \\App\\Models\\User */";
    assert_eq!(extract_var_type(doc), Some("App\\Models\\User".into()));
}

#[test]
fn var_type_none_when_missing() {
    let doc = "/** just a comment */";
    assert_eq!(extract_var_type(doc), None);
}

// ── extract_var_type_with_name ──────────────────────────────────────

#[test]
fn var_type_with_name_simple() {
    let doc = "/** @var Session */";
    assert_eq!(
        extract_var_type_with_name(doc),
        Some(("Session".into(), None))
    );
}

#[test]
fn var_type_with_name_has_var() {
    let doc = "/** @var Session $sess */";
    assert_eq!(
        extract_var_type_with_name(doc),
        Some(("Session".into(), Some("$sess".into())))
    );
}

#[test]
fn var_type_with_name_fqn() {
    let doc = "/** @var \\App\\Models\\User $user */";
    assert_eq!(
        extract_var_type_with_name(doc),
        Some(("App\\Models\\User".into(), Some("$user".into())))
    );
}

#[test]
fn var_type_with_name_no_var_tag() {
    let doc = "/** just a comment */";
    assert_eq!(extract_var_type_with_name(doc), None);
}

#[test]
fn var_type_with_name_description_not_var() {
    // Second token is not a $variable — should be ignored.
    let doc = "/** @var Session some description */";
    assert_eq!(
        extract_var_type_with_name(doc),
        Some(("Session".into(), None))
    );
}

#[test]
fn var_type_with_name_generic_stripped() {
    let doc = "/** @var Collection<int, User> $items */";
    assert_eq!(
        extract_var_type_with_name(doc),
        Some(("Collection".into(), Some("$items".into())))
    );
}

// ── find_inline_var_docblock ────────────────────────────────────────

#[test]
fn inline_var_docblock_simple() {
    let content = "<?php\n/** @var Session */\n$var = mystery();\n";
    let stmt_start = content.find("$var").unwrap();
    assert_eq!(
        find_inline_var_docblock(content, stmt_start),
        Some(("Session".into(), None))
    );
}

#[test]
fn inline_var_docblock_with_var_name() {
    let content = "<?php\n/** @var Session $var */\n$var = mystery();\n";
    let stmt_start = content.find("$var =").unwrap();
    assert_eq!(
        find_inline_var_docblock(content, stmt_start),
        Some(("Session".into(), Some("$var".into())))
    );
}

#[test]
fn inline_var_docblock_fqn() {
    let content = "<?php\n/** @var \\App\\Models\\User */\n$u = get();\n";
    let stmt_start = content.find("$u").unwrap();
    assert_eq!(
        find_inline_var_docblock(content, stmt_start),
        Some(("App\\Models\\User".into(), None))
    );
}

#[test]
fn inline_var_docblock_no_docblock() {
    let content = "<?php\n$var = mystery();\n";
    let stmt_start = content.find("$var").unwrap();
    assert_eq!(find_inline_var_docblock(content, stmt_start), None);
}

#[test]
fn inline_var_docblock_regular_comment_ignored() {
    // A `/* ... */` comment (not `/** */`) should not match.
    let content = "<?php\n/* @var Session */\n$var = mystery();\n";
    let stmt_start = content.find("$var").unwrap();
    assert_eq!(find_inline_var_docblock(content, stmt_start), None);
}

#[test]
fn inline_var_docblock_with_indentation() {
    let content = "<?php\nclass A {\n    public function f() {\n        /** @var Session */\n        $var = mystery();\n    }\n}\n";
    let stmt_start = content.find("$var").unwrap();
    assert_eq!(
        find_inline_var_docblock(content, stmt_start),
        Some(("Session".into(), None))
    );
}

// ── should_override_type ────────────────────────────────────────────

#[test]
fn override_object_with_class() {
    assert!(should_override_type("Session", "object"));
}

#[test]
fn override_mixed_with_class() {
    assert!(should_override_type("Session", "mixed"));
}

#[test]
fn override_class_with_subclass() {
    assert!(should_override_type("ConcreteSession", "SessionInterface"));
}

#[test]
fn no_override_int_with_class() {
    assert!(!should_override_type("Session", "int"));
}

#[test]
fn no_override_string_with_class() {
    assert!(!should_override_type("Session", "string"));
}

#[test]
fn no_override_bool_with_class() {
    assert!(!should_override_type("Session", "bool"));
}

#[test]
fn no_override_array_with_class() {
    assert!(!should_override_type("Session", "array"));
}

#[test]
fn no_override_void_with_class() {
    assert!(!should_override_type("Session", "void"));
}

#[test]
fn no_override_nullable_int_with_class() {
    assert!(!should_override_type("Session", "?int"));
}

#[test]
fn override_nullable_object_with_class() {
    assert!(should_override_type("Session", "?object"));
}

#[test]
fn no_override_scalar_union_with_class() {
    assert!(!should_override_type("Session", "string|int"));
}

#[test]
fn override_union_with_object_part() {
    // `SomeClass|null` has a non-scalar part → overridable
    assert!(should_override_type("ConcreteClass", "SomeClass|null"));
}

#[test]
fn no_override_when_docblock_is_scalar() {
    // Even if native is object, if docblock says `int`, no point overriding
    assert!(!should_override_type("int", "object"));
}

#[test]
fn override_self_with_class() {
    assert!(should_override_type("ConcreteClass", "self"));
}

#[test]
fn override_static_with_class() {
    assert!(should_override_type("ConcreteClass", "static"));
}

// ── resolve_effective_type ──────────────────────────────────────────

#[test]
fn effective_type_docblock_only() {
    assert_eq!(
        resolve_effective_type(None, Some("Session")),
        Some("Session".into())
    );
}

#[test]
fn effective_type_native_only() {
    assert_eq!(
        resolve_effective_type(Some("int"), None),
        Some("int".into())
    );
}

#[test]
fn effective_type_both_compatible() {
    assert_eq!(
        resolve_effective_type(Some("object"), Some("Session")),
        Some("Session".into())
    );
}

#[test]
fn effective_type_both_incompatible() {
    assert_eq!(
        resolve_effective_type(Some("int"), Some("Session")),
        Some("int".into())
    );
}

#[test]
fn effective_type_neither() {
    assert_eq!(resolve_effective_type(None, None), None);
}

// ── clean_type ──────────────────────────────────────────────────────

#[test]
fn clean_leading_backslash() {
    assert_eq!(clean_type("\\Foo\\Bar"), "Foo\\Bar");
}

#[test]
fn clean_generic() {
    assert_eq!(clean_type("Collection<int, Model>"), "Collection");
}

#[test]
fn clean_nullable_union() {
    assert_eq!(clean_type("Foo|null"), "Foo");
}

#[test]
fn clean_trailing_punctuation() {
    assert_eq!(clean_type("Foo."), "Foo");
}

// ── extract_conditional_return_type ─────────────────────────────────

#[test]
fn conditional_simple_class_string() {
    let doc = concat!(
        "/**\n",
        " * @return ($abstract is class-string<TClass> ? TClass : mixed)\n",
        " */",
    );
    let result = extract_conditional_return_type(doc);
    assert!(result.is_some(), "Should parse a conditional return type");
    let cond = result.unwrap();
    match cond {
        ConditionalReturnType::Conditional {
            ref param_name,
            ref condition,
            ref then_type,
            ref else_type,
        } => {
            assert_eq!(param_name, "abstract");
            assert_eq!(*condition, ParamCondition::ClassString);
            assert_eq!(
                **then_type,
                ConditionalReturnType::Concrete("TClass".into())
            );
            assert_eq!(**else_type, ConditionalReturnType::Concrete("mixed".into()));
        }
        _ => panic!("Expected Conditional, got {:?}", cond),
    }
}

#[test]
fn conditional_null_check() {
    let doc = concat!(
        "/**\n",
        " * @return ($guard is null ? \\Illuminate\\Contracts\\Auth\\Factory : \\Illuminate\\Contracts\\Auth\\StatefulGuard)\n",
        " */",
    );
    let result = extract_conditional_return_type(doc).unwrap();
    match result {
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            assert_eq!(param_name, "guard");
            assert_eq!(condition, ParamCondition::IsNull);
            assert_eq!(
                *then_type,
                ConditionalReturnType::Concrete("Illuminate\\Contracts\\Auth\\Factory".into())
            );
            assert_eq!(
                *else_type,
                ConditionalReturnType::Concrete(
                    "Illuminate\\Contracts\\Auth\\StatefulGuard".into()
                )
            );
        }
        _ => panic!("Expected Conditional"),
    }
}

#[test]
fn conditional_nested() {
    let doc = concat!(
        "/**\n",
        " * @return ($abstract is class-string<TClass> ? TClass : ($abstract is null ? \\Illuminate\\Foundation\\Application : mixed))\n",
        " */",
    );
    let result = extract_conditional_return_type(doc).unwrap();
    match result {
        ConditionalReturnType::Conditional {
            ref param_name,
            ref condition,
            ref then_type,
            ref else_type,
        } => {
            assert_eq!(param_name, "abstract");
            assert_eq!(*condition, ParamCondition::ClassString);
            assert_eq!(
                **then_type,
                ConditionalReturnType::Concrete("TClass".into())
            );
            // else_type should be another conditional
            match else_type.as_ref() {
                ConditionalReturnType::Conditional {
                    param_name: inner_param,
                    condition: inner_cond,
                    then_type: inner_then,
                    else_type: inner_else,
                } => {
                    assert_eq!(inner_param, "abstract");
                    assert_eq!(*inner_cond, ParamCondition::IsNull);
                    assert_eq!(
                        **inner_then,
                        ConditionalReturnType::Concrete(
                            "Illuminate\\Foundation\\Application".into()
                        )
                    );
                    assert_eq!(
                        **inner_else,
                        ConditionalReturnType::Concrete("mixed".into())
                    );
                }
                _ => panic!("Expected nested Conditional"),
            }
        }
        _ => panic!("Expected Conditional"),
    }
}

#[test]
fn conditional_multiline() {
    let doc = concat!(
        "/**\n",
        " * Get the available container instance.\n",
        " *\n",
        " * @param  string|callable|null  $abstract\n",
        " * @return ($abstract is class-string<TClass>\n",
        " *     ? TClass\n",
        " *     : ($abstract is null\n",
        " *         ? \\Illuminate\\Foundation\\Application\n",
        " *         : mixed))\n",
        " */",
    );
    let result = extract_conditional_return_type(doc);
    assert!(result.is_some(), "Should parse multi-line conditional");
    match result.unwrap() {
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            ..
        } => {
            assert_eq!(param_name, "abstract");
            assert_eq!(condition, ParamCondition::ClassString);
        }
        _ => panic!("Expected Conditional"),
    }
}

#[test]
fn conditional_is_type() {
    let doc = concat!(
        "/**\n",
        " * @return ($job is \\Closure ? \\Illuminate\\Foundation\\Bus\\PendingClosureDispatch : \\Illuminate\\Foundation\\Bus\\PendingDispatch)\n",
        " */",
    );
    let result = extract_conditional_return_type(doc).unwrap();
    match result {
        ConditionalReturnType::Conditional {
            param_name,
            condition,
            then_type,
            else_type,
        } => {
            assert_eq!(param_name, "job");
            assert_eq!(condition, ParamCondition::IsType("Closure".into()));
            assert_eq!(
                *then_type,
                ConditionalReturnType::Concrete(
                    "Illuminate\\Foundation\\Bus\\PendingClosureDispatch".into()
                )
            );
            assert_eq!(
                *else_type,
                ConditionalReturnType::Concrete(
                    "Illuminate\\Foundation\\Bus\\PendingDispatch".into()
                )
            );
        }
        _ => panic!("Expected Conditional"),
    }
}

#[test]
fn conditional_not_present() {
    let doc = "/** @return Application */";
    assert_eq!(extract_conditional_return_type(doc), None);
}

#[test]
fn conditional_no_return_tag() {
    let doc = "/** Just a comment */";
    assert_eq!(extract_conditional_return_type(doc), None);
}

// ─── @mixin tag extraction ──────────────────────────────────────────────

#[test]
fn mixin_tag_simple() {
    let doc = concat!("/**\n", " * @mixin ShoppingCart\n", " */",);
    let mixins = extract_mixin_tags(doc);
    assert_eq!(mixins, vec!["ShoppingCart"]);
}

#[test]
fn mixin_tag_fqn() {
    let doc = concat!("/**\n", " * @mixin \\App\\Models\\ShoppingCart\n", " */",);
    let mixins = extract_mixin_tags(doc);
    assert_eq!(mixins, vec!["App\\Models\\ShoppingCart"]);
}

#[test]
fn mixin_tag_multiple() {
    let doc = concat!(
        "/**\n",
        " * @mixin ShoppingCart\n",
        " * @mixin Wishlist\n",
        " */",
    );
    let mixins = extract_mixin_tags(doc);
    assert_eq!(mixins, vec!["ShoppingCart", "Wishlist"]);
}

#[test]
fn mixin_tag_none_when_missing() {
    let doc = "/** Just a comment */";
    let mixins = extract_mixin_tags(doc);
    assert!(mixins.is_empty());
}

#[test]
fn mixin_tag_with_description() {
    let doc = concat!(
        "/**\n",
        " * @mixin ShoppingCart Some extra description\n",
        " */",
    );
    let mixins = extract_mixin_tags(doc);
    assert_eq!(mixins, vec!["ShoppingCart"]);
}

#[test]
fn mixin_tag_generic_stripped() {
    let doc = concat!("/**\n", " * @mixin Collection<int, Model>\n", " */",);
    let mixins = extract_mixin_tags(doc);
    assert_eq!(mixins, vec!["Collection"]);
}

#[test]
fn mixin_tag_mixed_with_other_tags() {
    let doc = concat!(
        "/**\n",
        " * @property string $name\n",
        " * @mixin ShoppingCart\n",
        " * @method int getId()\n",
        " */",
    );
    let mixins = extract_mixin_tags(doc);
    assert_eq!(mixins, vec!["ShoppingCart"]);
}

#[test]
fn mixin_tag_empty_after_tag() {
    let doc = concat!("/**\n", " * @mixin\n", " */",);
    let mixins = extract_mixin_tags(doc);
    assert!(mixins.is_empty());
}

// ─── @phpstan-assert / @psalm-assert extraction ─────────────────────────

#[test]
fn assert_simple_phpstan() {
    let doc = concat!("/**\n", " * @phpstan-assert User $value\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].kind, AssertionKind::Always);
    assert_eq!(assertions[0].param_name, "$value");
    assert_eq!(assertions[0].asserted_type, "User");
    assert!(!assertions[0].negated);
}

#[test]
fn assert_simple_psalm() {
    let doc = concat!("/**\n", " * @psalm-assert AdminUser $obj\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].kind, AssertionKind::Always);
    assert_eq!(assertions[0].param_name, "$obj");
    assert_eq!(assertions[0].asserted_type, "AdminUser");
    assert!(!assertions[0].negated);
}

#[test]
fn assert_negated() {
    let doc = concat!("/**\n", " * @phpstan-assert !User $value\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].kind, AssertionKind::Always);
    assert_eq!(assertions[0].asserted_type, "User");
    assert!(assertions[0].negated);
}

#[test]
fn assert_if_true() {
    let doc = concat!("/**\n", " * @phpstan-assert-if-true User $value\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].kind, AssertionKind::IfTrue);
    assert_eq!(assertions[0].param_name, "$value");
    assert_eq!(assertions[0].asserted_type, "User");
    assert!(!assertions[0].negated);
}

#[test]
fn assert_if_false() {
    let doc = concat!("/**\n", " * @phpstan-assert-if-false User $value\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].kind, AssertionKind::IfFalse);
    assert_eq!(assertions[0].param_name, "$value");
    assert_eq!(assertions[0].asserted_type, "User");
    assert!(!assertions[0].negated);
}

#[test]
fn assert_psalm_if_true() {
    let doc = concat!("/**\n", " * @psalm-assert-if-true AdminUser $obj\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].kind, AssertionKind::IfTrue);
    assert_eq!(assertions[0].param_name, "$obj");
    assert_eq!(assertions[0].asserted_type, "AdminUser");
}

#[test]
fn assert_fqn_type() {
    let doc = concat!(
        "/**\n",
        " * @phpstan-assert \\App\\Models\\User $value\n",
        " */",
    );
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].asserted_type, "App\\Models\\User");
}

#[test]
fn assert_multiple_annotations() {
    let doc = concat!(
        "/**\n",
        " * @phpstan-assert User $first\n",
        " * @phpstan-assert AdminUser $second\n",
        " */",
    );
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 2);
    assert_eq!(assertions[0].param_name, "$first");
    assert_eq!(assertions[0].asserted_type, "User");
    assert_eq!(assertions[1].param_name, "$second");
    assert_eq!(assertions[1].asserted_type, "AdminUser");
}

#[test]
fn assert_mixed_with_other_tags() {
    let doc = concat!(
        "/**\n",
        " * Some description.\n",
        " *\n",
        " * @param mixed $value\n",
        " * @phpstan-assert User $value\n",
        " * @return void\n",
        " */",
    );
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].asserted_type, "User");
}

#[test]
fn assert_none_when_missing() {
    let doc = "/** @return void */";
    let assertions = extract_type_assertions(doc);
    assert!(assertions.is_empty());
}

#[test]
fn assert_empty_after_tag_ignored() {
    let doc = concat!("/**\n", " * @phpstan-assert\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert!(assertions.is_empty());
}

#[test]
fn assert_missing_param_ignored() {
    let doc = concat!("/**\n", " * @phpstan-assert User\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert!(assertions.is_empty());
}

#[test]
fn assert_param_without_dollar_ignored() {
    let doc = concat!("/**\n", " * @phpstan-assert User value\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert!(assertions.is_empty());
}

#[test]
fn assert_negated_if_true() {
    let doc = concat!("/**\n", " * @phpstan-assert-if-true !User $value\n", " */",);
    let assertions = extract_type_assertions(doc);
    assert_eq!(assertions.len(), 1);
    assert_eq!(assertions[0].kind, AssertionKind::IfTrue);
    assert!(assertions[0].negated);
    assert_eq!(assertions[0].asserted_type, "User");
}

// ─── @deprecated tag tests ──────────────────────────────────────

#[test]
fn deprecated_tag_bare() {
    let doc = concat!("/**\n", " * @deprecated\n", " */",);
    assert!(has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_with_message() {
    let doc = concat!("/**\n", " * @deprecated Use newMethod() instead.\n", " */",);
    assert!(has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_with_version() {
    let doc = concat!("/**\n", " * @deprecated since 2.0\n", " */",);
    assert!(has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_mixed_with_other_tags() {
    let doc = concat!(
        "/**\n",
        " * Some description.\n",
        " *\n",
        " * @param string $name\n",
        " * @deprecated Use something else.\n",
        " * @return void\n",
        " */",
    );
    assert!(has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_not_present() {
    let doc = concat!(
        "/**\n",
        " * @param string $name\n",
        " * @return void\n",
        " */",
    );
    assert!(!has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_empty_docblock() {
    let doc = "/** */";
    assert!(!has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_not_confused_with_similar_words() {
    // A word like "@deprecatedAlias" should NOT match — the tag must
    // be exactly "@deprecated" followed by whitespace or end-of-line.
    let doc = concat!("/**\n", " * @deprecatedAlias\n", " */",);
    assert!(!has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_at_end_of_line() {
    // Tag alone on the line with no trailing text.
    let doc = "/** @deprecated */";
    assert!(has_deprecated_tag(doc));
}

#[test]
fn deprecated_tag_with_tab_separator() {
    let doc = concat!("/**\n", " * @deprecated\tUse foo() instead\n", " */",);
    assert!(has_deprecated_tag(doc));
}

// ─── extract_generic_value_type ─────────────────────────────────────

#[test]
fn generic_value_type_list() {
    assert_eq!(
        extract_generic_value_type("list<User>"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_array_single_param() {
    assert_eq!(
        extract_generic_value_type("array<User>"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_array_two_params() {
    assert_eq!(
        extract_generic_value_type("array<int, User>"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_bracket_shorthand() {
    assert_eq!(
        extract_generic_value_type("User[]"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_iterable() {
    assert_eq!(
        extract_generic_value_type("iterable<User>"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_iterable_two_params() {
    assert_eq!(
        extract_generic_value_type("iterable<int, User>"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_nullable() {
    assert_eq!(
        extract_generic_value_type("?list<User>"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_fqn_bracket() {
    // clean_type strips the leading `\` but preserves namespace segments;
    // type_hint_to_classes handles the FQN → short-name lookup separately.
    assert_eq!(
        extract_generic_value_type("\\App\\Models\\User[]"),
        Some("App\\Models\\User".to_string())
    );
}

#[test]
fn generic_value_type_fqn_inside_generic() {
    assert_eq!(
        extract_generic_value_type("list<\\App\\Models\\User>"),
        Some("App\\Models\\User".to_string())
    );
}

#[test]
fn generic_value_type_collection_class() {
    assert_eq!(
        extract_generic_value_type("Collection<int, User>"),
        Some("User".to_string())
    );
}

#[test]
fn generic_value_type_scalar_element_returns_none() {
    assert_eq!(extract_generic_value_type("list<int>"), None);
    assert_eq!(extract_generic_value_type("array<string>"), None);
    assert_eq!(extract_generic_value_type("int[]"), None);
    assert_eq!(extract_generic_value_type("array<int, string>"), None);
}

#[test]
fn generic_value_type_plain_class_returns_none() {
    assert_eq!(extract_generic_value_type("User"), None);
    assert_eq!(extract_generic_value_type("string"), None);
}

#[test]
fn generic_value_type_empty_angle_brackets_returns_none() {
    assert_eq!(extract_generic_value_type("list<>"), None);
}
