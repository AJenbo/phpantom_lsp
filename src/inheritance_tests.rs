use super::*;

#[test]
fn test_apply_substitution_direct() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Language".to_string());
    subs.insert("TKey".to_string(), "int".to_string());

    assert_eq!(apply_substitution("TValue", &subs), "Language");
    assert_eq!(apply_substitution("TKey", &subs), "int");
    assert_eq!(apply_substitution("string", &subs), "string");
}

#[test]
fn test_apply_substitution_nullable() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Language".to_string());

    assert_eq!(apply_substitution("?TValue", &subs), "?Language");
}

#[test]
fn test_apply_substitution_union() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Language".to_string());

    assert_eq!(apply_substitution("TValue|null", &subs), "Language|null");
    assert_eq!(
        apply_substitution("TValue|string", &subs),
        "Language|string"
    );
}

#[test]
fn test_apply_substitution_intersection() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Language".to_string());

    assert_eq!(
        apply_substitution("TValue&Countable", &subs),
        "Language&Countable"
    );
}

#[test]
fn test_apply_substitution_generic() {
    let mut subs = HashMap::new();
    subs.insert("TKey".to_string(), "int".to_string());
    subs.insert("TValue".to_string(), "Language".to_string());

    assert_eq!(
        apply_substitution("array<TKey, TValue>", &subs),
        "array<int, Language>"
    );
}

#[test]
fn test_apply_substitution_nested_generic() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(
        apply_substitution("Collection<int, list<TValue>>", &subs),
        "Collection<int, list<User>>"
    );
}

#[test]
fn test_apply_substitution_array_shorthand() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(apply_substitution("TValue[]", &subs), "User[]");
}

#[test]
fn test_apply_substitution_no_match() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(apply_substitution("string", &subs), "string");
    assert_eq!(apply_substitution("void", &subs), "void");
    assert_eq!(apply_substitution("$this", &subs), "$this");
}

#[test]
fn test_apply_substitution_complex_union_with_generic() {
    let mut subs = HashMap::new();
    subs.insert("TKey".to_string(), "int".to_string());
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(
        apply_substitution("array<TKey, TValue>|null", &subs),
        "array<int, User>|null"
    );
}

#[test]
fn test_apply_substitution_dnf_parens() {
    let mut subs = HashMap::new();
    subs.insert("T".to_string(), "User".to_string());

    assert_eq!(
        apply_substitution("(T&Countable)", &subs),
        "(User&Countable)"
    );
}

#[test]
fn test_apply_substitution_callable_params() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(
        apply_substitution("callable(TValue): void", &subs),
        "callable(User): void"
    );
}

#[test]
fn test_apply_substitution_callable_multiple_params() {
    let mut subs = HashMap::new();
    subs.insert("TKey".to_string(), "int".to_string());
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(
        apply_substitution("callable(TKey, TValue): mixed", &subs),
        "callable(int, User): mixed"
    );
}

#[test]
fn test_apply_substitution_callable_return_type() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Order".to_string());

    assert_eq!(
        apply_substitution("callable(string): TValue", &subs),
        "callable(string): Order"
    );
}

#[test]
fn test_apply_substitution_closure_syntax() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Product".to_string());

    assert_eq!(
        apply_substitution("Closure(TValue): bool", &subs),
        "Closure(Product): bool"
    );
}

#[test]
fn test_apply_substitution_callable_empty_params() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(
        apply_substitution("callable(): TValue", &subs),
        "callable(): User"
    );
}

#[test]
fn test_apply_substitution_callable_no_match() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "User".to_string());

    // No template params inside callable — returned unchanged.
    assert_eq!(
        apply_substitution("callable(string): void", &subs),
        "callable(string): void"
    );
}

#[test]
fn test_apply_substitution_callable_generic_param() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "User".to_string());

    assert_eq!(
        apply_substitution("callable(Collection<int, TValue>): void", &subs),
        "callable(Collection<int, User>): void"
    );
}

#[test]
fn test_apply_substitution_fqn_closure() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Item".to_string());

    assert_eq!(
        apply_substitution("\\Closure(TValue): void", &subs),
        "\\Closure(Item): void"
    );
}

#[test]
fn test_build_substitution_map_basic() {
    let child = ClassInfo {
        name: "LanguageCollection".to_string(),
        parent_class: Some("Collection".to_string()),
        is_final: true,
        extends_generics: vec![(
            "Collection".to_string(),
            vec!["int".to_string(), "Language".to_string()],
        )],
        ..ClassInfo::default()
    };

    let parent = ClassInfo {
        name: "Collection".to_string(),
        template_params: vec!["TKey".to_string(), "TValue".to_string()],
        ..ClassInfo::default()
    };

    let subs = build_substitution_map(&child, &parent, &HashMap::new());
    assert_eq!(subs.get("TKey").unwrap(), "int");
    assert_eq!(subs.get("TValue").unwrap(), "Language");
}

#[test]
fn test_build_substitution_map_chained() {
    // Simulates: C extends B<Foo>, B extends A<T>, A has @template U
    // When resolving A's methods for C, active_subs = {T => Foo}
    // B's @extends A<T> should resolve to A<Foo>, giving {U => Foo}

    let current_b = ClassInfo {
        name: "B".to_string(),
        parent_class: Some("A".to_string()),
        template_params: vec!["T".to_string()],
        extends_generics: vec![("A".to_string(), vec!["T".to_string()])],
        ..ClassInfo::default()
    };

    let parent_a = ClassInfo {
        name: "A".to_string(),
        template_params: vec!["U".to_string()],
        ..ClassInfo::default()
    };

    let mut active = HashMap::new();
    active.insert("T".to_string(), "Foo".to_string());

    let subs = build_substitution_map(&current_b, &parent_a, &active);
    assert_eq!(subs.get("U").unwrap(), "Foo");
}

#[test]
fn test_short_name() {
    use crate::util::short_name;
    assert_eq!(short_name("Collection"), "Collection");
    assert_eq!(short_name("Illuminate\\Support\\Collection"), "Collection");
    assert_eq!(short_name("\\Collection"), "Collection");
}

#[test]
fn test_apply_substitution_to_method_modifies_return_and_params() {
    let mut subs = HashMap::new();
    subs.insert("TValue".to_string(), "Language".to_string());
    subs.insert("TKey".to_string(), "int".to_string());

    let mut method = MethodInfo {
        name: "first".to_string(),
        name_offset: 0,
        parameters: vec![crate::types::ParameterInfo {
            name: "$key".to_string(),
            is_required: false,
            type_hint: Some("TKey".to_string()),
            native_type_hint: Some("TKey".to_string()),
            description: None,
            default_value: None,
            is_variadic: false,
            is_reference: false,
        }],
        return_type: Some("TValue".to_string()),
        native_return_type: None,
        description: None,
        return_description: None,
        link: None,
        is_static: false,
        visibility: Visibility::Public,
        conditional_return: None,
        deprecation_message: None,
        template_params: Vec::new(),
        template_bindings: Vec::new(),
        has_scope_attribute: false,
        is_virtual: false,
    };

    apply_substitution_to_method(&mut method, &subs);

    assert_eq!(method.return_type.as_deref(), Some("Language"));
    assert_eq!(method.parameters[0].type_hint.as_deref(), Some("int"));
}
