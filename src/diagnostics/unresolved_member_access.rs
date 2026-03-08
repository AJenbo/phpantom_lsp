//! Unresolved member access diagnostics (opt-in).
//!
//! Walk the precomputed [`SymbolMap`] for a file and flag every
//! `MemberAccess` span where the **subject type** could not be
//! resolved at all. This is different from the `unknown_members`
//! diagnostic which fires when the subject resolves but the specific
//! member is missing.
//!
//! This diagnostic is **off by default** because most PHP codebases
//! lack comprehensive type annotations, which means PHPantom cannot
//! infer a type for many variables. Enabling this diagnostic on such
//! a codebase would flood the editor with noise.
//!
//! Enable it by adding the following to `.phpantom.toml`:
//!
//! ```toml
//! [diagnostics]
//! unresolved-member-access = true
//! ```
//!
//! The diagnostic uses `Severity::HINT` (not warning) because the
//! code is almost certainly correct. The purpose is to surface gaps
//! in type coverage so the developer can add annotations or discover
//! places where PHPantom's inference falls short.
//!
//! Subject resolution reuses the full completion resolver pipeline
//! ([`resolve_target_classes`]) so that property chains, method call
//! return types, call expressions, and all other subject forms are
//! handled identically to completion and go-to-definition.

use std::collections::HashMap;

use tower_lsp::lsp_types::*;

use crate::Backend;
use crate::completion::resolver::{ResolutionCtx, resolve_target_classes};
use crate::symbol_map::SymbolKind;
use crate::types::{AccessKind, ClassInfo};

use super::offset_range_to_lsp_range;

/// Diagnostic code used for unresolved-member-access diagnostics.
pub(crate) const UNRESOLVED_MEMBER_ACCESS_CODE: &str = "unresolved_member_access";

impl Backend {
    /// Collect unresolved-member-access diagnostics for a single file.
    ///
    /// This is a no-op unless `[diagnostics] unresolved-member-access`
    /// is set to `true` in `.phpantom.toml`. Appends diagnostics to
    /// `out`. The caller is responsible for publishing them via
    /// `textDocument/publishDiagnostics`.
    pub fn collect_unresolved_member_access_diagnostics(
        &self,
        uri: &str,
        content: &str,
        out: &mut Vec<Diagnostic>,
    ) {
        // ── Check config gate ───────────────────────────────────────────
        if !self.config().diagnostics.unresolved_member_access_enabled() {
            return;
        }

        // ── Gather context under locks ──────────────────────────────────
        let symbol_map = {
            let maps = self.symbol_maps.read();
            match maps.get(uri) {
                Some(sm) => sm.clone(),
                None => return,
            }
        };

        let file_use_map: HashMap<String, String> =
            self.use_map.read().get(uri).cloned().unwrap_or_default();

        let file_namespace: Option<String> = self.namespace_map.read().get(uri).cloned().flatten();

        let local_classes: Vec<ClassInfo> =
            self.ast_map.read().get(uri).cloned().unwrap_or_default();

        let class_loader = self.class_loader_with(&local_classes, &file_use_map, &file_namespace);
        let function_loader = self.function_loader_with(&file_use_map, &file_namespace);
        let cache = &self.resolved_class_cache;

        // ── Walk every symbol span ──────────────────────────────────────
        for span in &symbol_map.spans {
            let (subject_text, member_name, is_static, _is_method_call) = match &span.kind {
                SymbolKind::MemberAccess {
                    subject_text,
                    member_name,
                    is_static,
                    is_method_call,
                } => (subject_text, member_name, *is_static, *is_method_call),
                _ => continue,
            };

            // ── Skip the magic `::class` constant ───────────────────────
            if member_name == "class" && is_static {
                continue;
            }

            // ── Resolve the subject using the full completion pipeline ───
            let access_kind = if is_static {
                AccessKind::DoubleColon
            } else {
                AccessKind::Arrow
            };

            // Find the innermost enclosing class for `$this`/`self`/`static`
            // resolution inside the completion resolver.
            let current_class = find_innermost_enclosing_class(&local_classes, span.start);

            let rctx = ResolutionCtx {
                current_class,
                all_classes: &local_classes,
                content,
                cursor_offset: span.start,
                class_loader: &class_loader,
                resolved_class_cache: Some(cache),
                function_loader: Some(&function_loader),
            };

            let base_classes = resolve_target_classes(subject_text, access_kind, &rctx);

            if !base_classes.is_empty() {
                // Subject resolved. The unknown_members diagnostic
                // handles the case where the member itself is missing.
                continue;
            }

            // ── Skip call-expression subjects ───────────────────────────
            // When the subject is a function or method call (e.g.
            // `end($arr)`, `$obj->getX()`), the failure is usually
            // because the symbol map's subject_text doesn't preserve
            // full argument text, not because the user is missing a
            // type annotation.  Flagging these would produce noise the
            // user cannot act on.
            if subject_text.contains('(') {
                continue;
            }

            // ── Subject is unresolvable — emit diagnostic ───────────────
            let range =
                match offset_range_to_lsp_range(content, span.start as usize, span.end as usize) {
                    Some(r) => r,
                    None => continue,
                };

            let subject_display = subject_text.trim();
            let message = format!(
                "Cannot resolve type of '{}'. Add a type annotation or PHPDoc tag to enable full IDE support.",
                subject_display,
            );

            out.push(Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::HINT),
                code: Some(NumberOrString::String(
                    UNRESOLVED_MEMBER_ACCESS_CODE.to_string(),
                )),
                code_description: None,
                source: Some("phpantom".to_string()),
                message,
                related_information: None,
                tags: None,
                data: None,
            });
        }
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Find the innermost class whose body span contains `offset`.
fn find_innermost_enclosing_class(local_classes: &[ClassInfo], offset: u32) -> Option<&ClassInfo> {
    local_classes
        .iter()
        .filter(|c| offset >= c.start_offset && offset <= c.end_offset)
        .min_by_key(|c| c.end_offset.saturating_sub(c.start_offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: set up a backend with the unresolved-member-access
    /// diagnostic enabled and collect diagnostics.
    fn collect_enabled(backend: &Backend, uri: &str, content: &str) -> Vec<Diagnostic> {
        // Enable the diagnostic via config.
        backend.config.lock().diagnostics.unresolved_member_access = Some(true);
        backend.update_ast(uri, content);
        let mut out = Vec::new();
        backend.collect_unresolved_member_access_diagnostics(uri, content, &mut out);
        out
    }

    /// Helper: collect diagnostics with the feature disabled (default).
    fn collect_disabled(backend: &Backend, uri: &str, content: &str) -> Vec<Diagnostic> {
        backend.update_ast(uri, content);
        let mut out = Vec::new();
        backend.collect_unresolved_member_access_diagnostics(uri, content, &mut out);
        out
    }

    // ── Gate: disabled by default ───────────────────────────────────────

    #[test]
    fn disabled_by_default() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function unknown(): mixed { return null; }
$x = unknown();
$x->whatever();
"#;
        let diags = collect_disabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected when feature is disabled, got: {:?}",
            diags
        );
    }

    // ── Basic detection ─────────────────────────────────────────────────

    #[test]
    fn flags_member_access_on_unresolvable_variable() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
function getUnknown(): mixed { return null; }
$x = getUnknown();
$x->whatever();
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.iter().any(|d| d.message.contains("$x")),
            "Expected diagnostic mentioning $x, got: {:?}",
            diags
        );
    }

    #[test]
    fn flags_unresolvable_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
$y = something_undefined();
$y->foo()->bar();
"#;
        let diags = collect_enabled(&backend, uri, content);
        // At least one diagnostic for the unresolvable subject
        assert!(
            !diags.is_empty(),
            "Expected at least one diagnostic for unresolvable chain, got none"
        );
    }

    // ── No false positives ──────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_resolvable_variable() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {}
}

class Consumer {
    public function run(): void {
        $f = new Foo();
        $f->bar();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for resolvable variable, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_this() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {
        $this->bar();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for $this access, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_self() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public const X = 1;
    public function bar(): void {
        self::X;
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for self:: access, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_static() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public static function create(): static { return new static(); }
    public function bar(): void {
        static::create();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for static:: access, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_parent() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Base {
    public function run(): void {}
}

class Child extends Base {
    public function run(): void {
        parent::run();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for parent:: access, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_static_class_access() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public static function create(): void {}
}

Foo::create();
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for Foo:: access, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_class_constant() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public const BAR = 1;
}
$x = Foo::class;
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for ::class access, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_typed_parameter() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {}
}

function doSomething(Foo $f): void {
    $f->bar();
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for typed parameter, got: {:?}",
            diags
        );
    }

    // ── Fluent chains ───────────────────────────────────────────────────

    #[test]
    fn no_diagnostic_for_fluent_method_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Brush {
    public function setSize(string $size): static { return $this; }
    public function setStyle(string $style): static { return $this; }
    public function stroke(): string { return ''; }
}

class Studio {
    public Brush $brush;
    public function __construct() { $this->brush = new Brush(); }
}

class Demo {
    public function run(): void {
        $studio = new Studio();
        $studio->brush->setSize('large')->setStyle('pointed')->stroke();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for fluent method chain, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_property_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Inner {
    public string $value = '';
    public function get(): string { return $this->value; }
}

class Outer {
    public Inner $inner;
    public function __construct() { $this->inner = new Inner(); }
}

class Demo {
    public function run(): void {
        $o = new Outer();
        $o->inner->get();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for property chain, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_return_type_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Canvas {
    public function title(): string { return ''; }
}

class Brush {
    public function getCanvas(): Canvas { return new Canvas(); }
}

class Demo {
    public function run(): void {
        $b = new Brush();
        $b->getCanvas()->title();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for return type chain, got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_new_expression() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Foo {
    public function bar(): void {}
}

(new Foo())->bar();
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for (new Foo())->bar(), got: {:?}",
            diags
        );
    }

    #[test]
    fn no_diagnostic_for_inline_function_call_chain() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
class Pen {
    public function write(): string { return ''; }
}

class Demo {
    /** @var Pen[] */
    public array $members = [];

    public function run(): void {
        $src = new Demo();
        end($src->members)->write();
    }
}
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags.is_empty(),
            "No diagnostics expected for end($arr)->method(), got: {:?}",
            diags
        );
    }

    // ── Diagnostic metadata ─────────────────────────────────────────────

    #[test]
    fn diagnostic_has_hint_severity() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
$x = unknown_fn();
$x->foo();
"#;
        let diags = collect_enabled(&backend, uri, content);
        assert!(
            diags
                .iter()
                .all(|d| d.severity == Some(DiagnosticSeverity::HINT)),
            "All diagnostics should have HINT severity, got: {:?}",
            diags
        );
    }

    #[test]
    fn diagnostic_has_code_and_source() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
$x = unknown_fn();
$x->foo();
"#;
        let diags = collect_enabled(&backend, uri, content);
        for d in &diags {
            assert_eq!(
                d.source.as_deref(),
                Some("phpantom"),
                "source should be phpantom"
            );
            assert_eq!(
                d.code,
                Some(NumberOrString::String(
                    UNRESOLVED_MEMBER_ACCESS_CODE.to_string()
                )),
                "code should be unresolved_member_access"
            );
        }
    }

    #[test]
    fn diagnostic_message_mentions_subject() {
        let backend = Backend::new_test();
        let uri = "file:///test.php";
        let content = r#"<?php
$mystery = get_mystery();
$mystery->doStuff();
"#;
        let diags = collect_enabled(&backend, uri, content);
        let has_subject_mention = diags.iter().any(|d| d.message.contains("$mystery"));
        assert!(
            has_subject_mention,
            "Diagnostic message should mention the subject variable, got: {:?}",
            diags
        );
    }
}
