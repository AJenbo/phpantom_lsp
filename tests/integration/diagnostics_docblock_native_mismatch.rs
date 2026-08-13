#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use phpantom_lsp::Backend;
    use tower_lsp::lsp_types::*;

    fn collect(php: &str) -> Vec<Diagnostic> {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        backend.update_ast(uri, &Arc::new(php.to_string()));
        let mut out = Vec::new();
        backend.collect_docblock_native_mismatch_diagnostics(uri, php, &mut out);
        out
    }

    #[test]
    fn scalar_param_narrower_than_nullable_hint_is_flagged() {
        let php = r#"<?php
/**
 * @param string $name
 */
function greet(?string $name): void {}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert_eq!(
            diags[0].code,
            Some(NumberOrString::String(
                "docblock_native_mismatch".to_string()
            ))
        );
        assert_eq!(diags[0].severity, Some(DiagnosticSeverity::WARNING));
        assert!(diags[0].message.contains("'string'"));
        assert!(diags[0].message.contains("$name"));
        assert!(diags[0].message.contains("'?string'"));
        // The range covers `?string $name` on the parameter line.
        assert_eq!(diags[0].range.start.line, 4);
        assert_eq!(diags[0].range.end.line, 4);
    }

    #[test]
    fn array_param_narrower_than_nullable_hint_is_flagged() {
        let php = r#"<?php
/**
 * @param list<int> $items
 */
function takesItems(?array $items): void {}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert!(diags[0].message.contains("'list<int>'"));
        assert!(diags[0].message.contains("'?array'"));
    }

    #[test]
    fn union_spelling_of_null_is_flagged_too() {
        let php = r#"<?php
/**
 * @param non-empty-string $name
 */
function greet(string|null $name): void {}
"#;
        assert_eq!(collect(php).len(), 1);
    }

    #[test]
    fn param_default_null_is_still_flagged() {
        let php = r#"<?php
/**
 * @param class-string $type
 */
function make(?string $type = null): void {}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert!(diags[0].message.contains("'class-string'"));
    }

    #[test]
    fn matching_nullable_docblock_is_clean() {
        let php = r#"<?php
/**
 * @param string|null $name
 * @param list<int>|null $items
 * @param ?int $count
 */
function greet(?string $name, ?array $items, ?int $count): void {}
"#;
        assert!(collect(php).is_empty());
    }

    #[test]
    fn non_nullable_native_hint_is_clean() {
        let php = r#"<?php
/**
 * @param list<int> $items
 */
function takesItems(array $items): void {}
"#;
        assert!(collect(php).is_empty());
    }

    #[test]
    fn implicit_nullable_default_is_clean() {
        // `string $name = null` is PHP's pre-8.4 implicit-nullable form. The
        // declaration itself does not spell null, so the docblock is not
        // contradicting anything written in the signature.
        let php = r#"<?php
/**
 * @param non-empty-string $name
 */
function greet(string $name = null): void {}
"#;
        assert!(collect(php).is_empty());
    }

    #[test]
    fn narrowing_mixed_is_clean() {
        let php = r#"<?php
/**
 * @param list<int> $items
 */
function takesItems(mixed $items): void {}
"#;
        assert!(collect(php).is_empty());
    }

    #[test]
    fn class_like_docblock_name_is_left_alone() {
        // `Foo` could be a `@template` parameter or an imported
        // `@psalm-type` alias standing in for a nullable type, so nothing
        // in the annotation settles whether it admits null.
        let php = r#"<?php
/**
 * @template T
 * @param T $value
 * @param Wrapper $wrapper
 */
function unwrap(?object $value, ?object $wrapper): void {}
"#;
        assert!(collect(php).is_empty());
    }

    #[test]
    fn return_type_narrower_than_nullable_hint_is_flagged() {
        let php = r#"<?php
class Entry {
    /**
     * @return string
     */
    public function getPath(): ?string
    {
        return $this->path;
    }
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert!(diags[0].message.contains("Documented return type 'string'"));
        assert!(diags[0].message.contains("'?string'"));
    }

    #[test]
    fn matching_nullable_return_docblock_is_clean() {
        let php = r#"<?php
class Entry {
    /**
     * @return string|null
     */
    public function getPath(): ?string
    {
        return $this->path;
    }
}
"#;
        assert!(collect(php).is_empty());
    }

    #[test]
    fn promoted_constructor_property_is_flagged() {
        let php = r#"<?php
final class GenerateFactory
{
    /**
     * @param class-string $resultType
     */
    public function __construct(public ?string $resultType = null) {}
}
"#;
        assert_eq!(collect(php).len(), 1);
    }

    #[test]
    fn vendor_prefixed_tag_is_read() {
        let php = r#"<?php
class Helper {
    /** @phpstan-return array<int, string> */
    public function placeholders(string $format): ?array
    {
        return null;
    }
}
"#;
        let diags = collect(php);
        assert_eq!(diags.len(), 1, "got: {diags:?}");
        assert!(diags[0].message.contains("array<int, string>"));
    }

    #[test]
    fn interface_and_trait_methods_are_checked() {
        let php = r#"<?php
namespace App;

interface Reader {
    /**
     * @param string $key
     */
    public function read(?string $key): void;
}

trait Writer {
    /**
     * @param string $key
     */
    public function write(?string $key): void {}
}
"#;
        assert_eq!(collect(php).len(), 2);
    }

    #[test]
    fn no_docblock_is_clean() {
        let php = r#"<?php
function greet(?string $name): void {}
"#;
        assert!(collect(php).is_empty());
    }
}
