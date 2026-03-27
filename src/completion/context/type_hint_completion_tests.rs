use super::*;

/// Helper: detect at a given line/character.
fn detect(content: &str, line: u32, character: u32) -> Option<TypeHintContext> {
    detect_type_hint_context(content, Position { line, character })
}

// ── Function parameter type hints ───────────────────────────────

#[test]
fn after_open_paren_in_function() {
    let src = "<?php\nfunction foo(Us) {}";
    let ctx = detect(src, 1, 15).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn empty_after_open_paren() {
    let src = "<?php\nfunction foo() {}";
    // cursor right after `(`
    let ctx = detect(src, 1, 13);
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().partial, "");
}

#[test]
fn after_comma_in_function_params() {
    let src = "<?php\nfunction foo(string $a, Us) {}";
    let ctx = detect(src, 1, 26).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn after_comma_empty_partial() {
    let src = "<?php\nfunction foo(string $a, ) {}";
    let ctx = detect(src, 1, 24);
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().partial, "");
}

#[test]
fn not_after_comma_incomplete_param() {
    // The first param has no $variable yet — the user is still typing
    // the type, so the comma doesn't indicate a new param type position.
    let src = "<?php\nfunction foo(string,) {}";
    let ctx = detect(src, 1, 20);
    assert!(ctx.is_none());
}

// ── Return type hints ───────────────────────────────────────────

#[test]
fn return_type_after_colon() {
    let src = "<?php\nfunction foo(): Us {}";
    let ctx = detect(src, 1, 18).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn return_type_empty() {
    let src = "<?php\nfunction foo():  {}";
    // cursor right after `: `
    let ctx = detect(src, 1, 16);
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().partial, "");
}

// ── Nullable / union / intersection modifiers ───────────────────

#[test]
fn nullable_param_type() {
    let src = "<?php\nfunction foo(?Us) {}";
    let ctx = detect(src, 1, 16).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn union_param_type() {
    let src = "<?php\nfunction foo(string|Us) {}";
    let ctx = detect(src, 1, 22).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn intersection_param_type() {
    let src = "<?php\nfunction foo(A&Us) {}";
    let ctx = detect(src, 1, 17).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn union_return_type() {
    let src = "<?php\nfunction foo(): string|Us {}";
    let ctx = detect(src, 1, 25).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn nullable_return_type() {
    let src = "<?php\nfunction foo(): ?Us {}";
    let ctx = detect(src, 1, 19).unwrap();
    assert_eq!(ctx.partial, "Us");
}

// ── Method definitions ──────────────────────────────────────────

#[test]
fn method_param_type() {
    let src = "<?php\nclass Foo {\n    public function bar(Us) {}\n}";
    let ctx = detect(src, 2, 26).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn method_return_type() {
    let src = "<?php\nclass Foo {\n    public function bar(): Us {}\n}";
    let ctx = detect(src, 2, 29).unwrap();
    assert_eq!(ctx.partial, "Us");
}

// ── Property type hints ─────────────────────────────────────────

#[test]
fn property_after_public() {
    let src = "<?php\nclass Foo {\n    public Us\n}";
    let ctx = detect(src, 2, 13).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn no_type_hint_immediately_after_public_modifier() {
    let src = "<?php\nclass Foo {\n    public \n}";
    let ctx = detect(src, 2, 11);
    assert!(
        ctx.is_none(),
        "Empty partial right after `public` should defer to keyword completion"
    );
}

#[test]
fn property_after_private_readonly() {
    let src = "<?php\nclass Foo {\n    private readonly Us\n}";
    let ctx = detect(src, 2, 23).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn property_after_protected_static() {
    let src = "<?php\nclass Foo {\n    protected static Us\n}";
    let ctx = detect(src, 2, 23).unwrap();
    assert_eq!(ctx.partial, "Us");
}

// ── Promoted constructor parameters ─────────────────────────────

#[test]
fn promoted_param_after_modifier() {
    let src = "<?php\nclass Foo {\n    public function __construct(private Us) {}\n}";
    let ctx = detect(src, 2, 42).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn promoted_param_after_readonly() {
    let src = "<?php\nclass Foo {\n    public function __construct(private readonly Us) {}\n}";
    let ctx = detect(src, 2, 51).unwrap();
    assert_eq!(ctx.partial, "Us");
}

// ── Closures and arrow functions ────────────────────────────────

#[test]
fn closure_param_type() {
    let src = "<?php\n$f = function(Us) {};";
    let ctx = detect(src, 1, 16).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn arrow_fn_param_type() {
    let src = "<?php\n$f = fn(Us) => null;";
    let ctx = detect(src, 1, 10).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn closure_return_type() {
    let src = "<?php\n$f = function(): Us {};";
    let ctx = detect(src, 1, 19).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn arrow_fn_return_type() {
    let src = "<?php\n$f = fn(): Us => null;";
    let ctx = detect(src, 1, 13).unwrap();
    assert_eq!(ctx.partial, "Us");
}

// ── Multi-line function definitions ─────────────────────────────

#[test]
fn multiline_param_type() {
    let src = "<?php\nfunction foo(\n    string $a,\n    Us\n) {}";
    let ctx = detect(src, 3, 6).unwrap();
    assert_eq!(ctx.partial, "Us");
}

#[test]
fn multiline_after_comma_empty() {
    let src = "<?php\nfunction foo(\n    string $a,\n    \n) {}";
    let ctx = detect(src, 3, 4);
    assert!(ctx.is_some());
    assert_eq!(ctx.unwrap().partial, "");
}

// ── Negative cases: should NOT detect ───────────────────────────

#[test]
fn not_in_function_call() {
    let src = "<?php\nfoo(Us);";
    let ctx = detect(src, 1, 6);
    assert!(ctx.is_none());
}

#[test]
fn not_in_method_call() {
    let src = "<?php\n$obj->foo(Us);";
    let ctx = detect(src, 1, 13);
    assert!(ctx.is_none());
}

#[test]
fn not_variable() {
    let src = "<?php\nfunction foo($us) {}";
    let ctx = detect(src, 1, 15);
    assert!(ctx.is_none());
}

#[test]
fn not_member_access() {
    let src = "<?php\n$this->Us";
    let ctx = detect(src, 1, 10);
    assert!(ctx.is_none());
}

#[test]
fn not_static_access() {
    let src = "<?php\nFoo::Us";
    let ctx = detect(src, 1, 8);
    assert!(ctx.is_none());
}

#[test]
fn not_assignment() {
    let src = "<?php\n$x = Us;";
    let ctx = detect(src, 1, 7);
    assert!(ctx.is_none());
}

#[test]
fn not_after_function_keyword() {
    // Typing the function name after `function` should not suggest types.
    let src = "<?php\npublic function Us";
    let ctx = detect(src, 1, 20);
    // `function` is not a modifier keyword, so this should not match.
    assert!(ctx.is_none());
}

#[test]
fn partial_is_function_keyword_after_modifier() {
    // `public function` — the partial "function" should be filtered out
    // so we don't offer type hints when the user is typing the keyword.
    let src = "<?php\nclass Foo {\n    public function\n}";
    let ctx = detect(src, 2, 19);
    assert!(ctx.is_none());
}

// ── Native types constant ───────────────────────────────────────

#[test]
fn native_types_includes_common_types() {
    assert!(PHP_NATIVE_TYPES.contains(&"string"));
    assert!(PHP_NATIVE_TYPES.contains(&"int"));
    assert!(PHP_NATIVE_TYPES.contains(&"float"));
    assert!(PHP_NATIVE_TYPES.contains(&"bool"));
    assert!(PHP_NATIVE_TYPES.contains(&"array"));
    assert!(PHP_NATIVE_TYPES.contains(&"mixed"));
    assert!(PHP_NATIVE_TYPES.contains(&"void"));
    assert!(PHP_NATIVE_TYPES.contains(&"never"));
    assert!(PHP_NATIVE_TYPES.contains(&"callable"));
    assert!(PHP_NATIVE_TYPES.contains(&"self"));
    assert!(PHP_NATIVE_TYPES.contains(&"static"));
    assert!(PHP_NATIVE_TYPES.contains(&"null"));
    assert!(PHP_NATIVE_TYPES.contains(&"true"));
    assert!(PHP_NATIVE_TYPES.contains(&"false"));
}

#[test]
fn native_types_excludes_phpstan_only() {
    assert!(!PHP_NATIVE_TYPES.contains(&"class-string"));
    assert!(!PHP_NATIVE_TYPES.contains(&"positive-int"));
    assert!(!PHP_NATIVE_TYPES.contains(&"non-empty-string"));
    assert!(!PHP_NATIVE_TYPES.contains(&"resource"));
}
