//! Integration tests for the "Add `#[Override]`" code action.
//!
//! These tests exercise the full pipeline: a PHPStan diagnostic with
//! identifier `method.missingOverride` triggers a code action that
//! inserts `#[Override]` above the method declaration, adding a
//! `use Override;` import when the file declares a namespace.

use std::sync::Arc;

use crate::common::create_test_backend;
use tower_lsp::lsp_types::*;

/// Inject a PHPStan diagnostic into the backend's cache and return it.
fn inject_phpstan_diag(
    backend: &phpantom_lsp::Backend,
    uri: &str,
    line: u32,
    message: &str,
    identifier: &str,
) -> Diagnostic {
    let diag = Diagnostic {
        range: Range {
            start: Position::new(line, 0),
            end: Position::new(line, 80),
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(identifier.to_string())),
        source: Some("PHPStan".to_string()),
        message: message.to_string(),
        ..Default::default()
    };
    {
        let mut cache = backend.phpstan_last_diags().lock();
        cache.entry(uri.to_string()).or_default().push(diag.clone());
    }
    diag
}

/// Helper: send a code action request at the given line/character.
fn get_code_actions(
    backend: &phpantom_lsp::Backend,
    uri: &str,
    content: &str,
    line: u32,
    character: u32,
) -> Vec<CodeActionOrCommand> {
    let params = CodeActionParams {
        text_document: TextDocumentIdentifier {
            uri: uri.parse().unwrap(),
        },
        range: Range {
            start: Position::new(line, character),
            end: Position::new(line, character),
        },
        context: CodeActionContext {
            diagnostics: vec![],
            only: None,
            trigger_kind: None,
        },
        work_done_progress_params: WorkDoneProgressParams {
            work_done_token: None,
        },
        partial_result_params: PartialResultParams {
            partial_result_token: None,
        },
    };

    backend.handle_code_action(uri, content, &params)
}

/// Find the "Add #[Override]" code action.
fn find_add_override_action(actions: &[CodeActionOrCommand]) -> Option<&CodeAction> {
    actions.iter().find_map(|a| match a {
        CodeActionOrCommand::CodeAction(ca) if ca.title.contains("#[Override]") => Some(ca),
        _ => None,
    })
}

/// Extract all text edits from a code action's workspace edit.
/// Resolve a deferred code action by storing file content in open_files
/// and calling resolve_code_action.
fn resolve_action(
    backend: &phpantom_lsp::Backend,
    uri: &str,
    content: &str,
    action: &CodeAction,
) -> CodeAction {
    backend
        .open_files()
        .write()
        .insert(uri.to_string(), Arc::new(content.to_string()));
    let (resolved, _) = backend.resolve_code_action(action.clone());
    assert!(
        resolved.edit.is_some(),
        "resolved action should have an edit, title: {}",
        resolved.title
    );
    resolved
}

fn extract_edits(action: &CodeAction) -> Vec<TextEdit> {
    let edit = action.edit.as_ref().expect("action should have an edit");
    let changes = edit.changes.as_ref().expect("edit should have changes");
    changes.values().flat_map(|v| v.iter()).cloned().collect()
}

/// Combine text edits into the original content to produce the result.
/// Edits are applied in reverse order of their start position so that
/// earlier edits don't invalidate later offsets.
fn apply_edits(content: &str, edits: &[TextEdit]) -> String {
    let mut result = content.to_string();
    let mut sorted: Vec<&TextEdit> = edits.iter().collect();
    sorted.sort_by(|a, b| {
        b.range
            .start
            .line
            .cmp(&a.range.start.line)
            .then(b.range.start.character.cmp(&a.range.start.character))
    });

    for edit in sorted {
        let start = lsp_pos_to_offset(&result, edit.range.start);
        let end = lsp_pos_to_offset(&result, edit.range.end);
        result.replace_range(start..end, &edit.new_text);
    }
    result
}

fn lsp_pos_to_offset(content: &str, pos: Position) -> usize {
    let mut offset = 0;
    for (i, line) in content.lines().enumerate() {
        if i == pos.line as usize {
            return offset + pos.character as usize;
        }
        offset += line.len() + 1; // +1 for newline
    }
    content.len()
}

// ── Basic: adds #[Override] to a simple method (no namespace) ───────────────

#[test]
fn adds_override_to_simple_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions).expect("should offer Add #[Override] action");

    assert_eq!(action.kind, Some(CodeActionKind::QUICKFIX));
    assert_eq!(action.is_preferred, Some(true));
    assert!(
        action.title.contains("foo"),
        "title should mention method name: {}",
        action.title
    );

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    // Non-namespaced file → short form, no import.
    assert!(
        result.contains("#[Override]"),
        "should insert #[Override]:\n{}",
        result
    );
    assert!(
        !result.contains("use Override;"),
        "should NOT add use import in non-namespaced file:\n{}",
        result
    );
    // Verify placement: #[Override] should appear before `public function`.
    let override_pos = result.find("#[Override]").unwrap();
    let func_pos = result.find("public function foo").unwrap();
    assert!(
        override_pos < func_pos,
        "#[Override] should appear before `public function`"
    );
    // Verify indentation matches the method.
    assert!(
        result.contains("    #[Override]\n    public function foo"),
        "should be indented to match the method:\n{}",
        result
    );
}

// ── Method with docblock ────────────────────────────────────────────────────

#[test]
fn adds_override_to_method_with_docblock() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    /**
     * Do something.
     */
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        5, // `public function foo()` line
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 5, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    // #[Override] should be between the docblock and `public function`.
    assert!(
        result.contains("*/\n    #[Override]\n    public function foo"),
        "should insert between docblock and method:\n{}",
        result
    );
}

// ── Method with existing attributes ─────────────────────────────────────────

#[test]
fn inserts_before_existing_attributes() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    #[Route('/foo')]
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        3, // `public function foo()` line
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 3, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    // #[Override] should appear before #[Route].
    assert!(
        result.contains("    #[Override]\n    #[Route('/foo')]\n    public function foo"),
        "should insert before existing attributes:\n{}",
        result
    );
}

// ── Method with multiple existing attributes ────────────────────────────────

#[test]
fn inserts_before_multiple_existing_attributes() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    #[Route('/foo')]
    #[Deprecated]
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        4, // `public function foo()` line
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 4, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    // #[Override] should appear before both existing attributes.
    assert!(
        result.contains(
            "    #[Override]\n    #[Route('/foo')]\n    #[Deprecated]\n    public function foo"
        ),
        "should insert before all existing attributes:\n{}",
        result
    );
}

// ── No action when #[\Override] already present ─────────────────────────────

#[test]
fn no_action_when_fqn_override_already_present() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    #[\Override]
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        3, // `public function foo()` line
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 3, 10);
    let action = find_add_override_action(&actions);
    assert!(
        action.is_none(),
        "should NOT offer action when #[\\Override] already present"
    );
}

// ── No action when #[Override] (without backslash) already present ──────────

#[test]
fn no_action_when_short_override_already_present() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    #[Override]
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        3,
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 3, 10);
    let action = find_add_override_action(&actions);
    assert!(
        action.is_none(),
        "should NOT offer action when #[Override] (without backslash) already present"
    );
}

// ── Ignores non-matching diagnostic identifiers ─────────────────────────────

#[test]
fn ignores_other_phpstan_identifiers() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Foo {
    public function bar(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Some other PHPStan error.",
        "return.unusedType",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions);
    assert!(
        action.is_none(),
        "should NOT offer action for non-missingOverride identifiers"
    );
}

// ── Protected method ────────────────────────────────────────────────────────

#[test]
fn adds_override_to_protected_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    protected function handle(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Child::handle() overrides method Base::handle() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("    #[Override]\n    protected function handle"),
        "should insert #[Override] before protected method:\n{}",
        result
    );
}

// ── Static method ───────────────────────────────────────────────────────────

#[test]
fn adds_override_to_static_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    public static function create(): static {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Child::create() overrides method Base::create() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("    #[Override]\n    public static function create"),
        "should insert #[Override] before public static method:\n{}",
        result
    );
}

// ── Constructor override ────────────────────────────────────────────────────

#[test]
fn adds_override_to_constructor() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    public function __construct() {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Child::__construct() overrides method Base::__construct() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    assert!(
        action.title.contains("__construct"),
        "title should mention __construct: {}",
        action.title
    );

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("    #[Override]\n    public function __construct"),
        "should insert #[Override] before constructor:\n{}",
        result
    );
}

// ── Namespaced class — adds use import ──────────────────────────────────────

#[test]
fn adds_override_and_use_import_in_namespaced_class() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Http\Controllers;

class UserController extends Controller {
    public function index(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        4,
        "Method App\\Http\\Controllers\\UserController::index() overrides method App\\Http\\Controllers\\Controller::index() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 4, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("#[Override]"),
        "should insert #[Override] in namespaced class:\n{}",
        result
    );
    assert!(
        !result.contains("#[\\Override]"),
        "should use short form, not FQN:\n{}",
        result
    );
    assert!(
        result.contains("    #[Override]\n    public function index"),
        "should have correct indentation:\n{}",
        result
    );
    assert!(
        result.contains("use Override;"),
        "should add `use Override;` import in namespaced file:\n{}",
        result
    );
}

// ── Namespaced class — no duplicate import ──────────────────────────────────

#[test]
fn no_duplicate_import_when_already_imported() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Controllers;

use Override;

class Child extends Base {
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        6,
        "Method App\\Controllers\\Child::foo() overrides method App\\Controllers\\Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 6, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("#[Override]"),
        "should insert #[Override]:\n{}",
        result
    );
    // Count occurrences of `use Override;` — should still be exactly 1.
    let use_count = result.matches("use Override;").count();
    assert_eq!(
        use_count, 1,
        "should NOT duplicate existing use import:\n{}",
        result
    );
}

// ── Namespaced class with existing use block — import is sorted ─────────────

#[test]
fn import_sorted_into_existing_use_block() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Controllers;

use App\Models\User;
use Illuminate\Http\Request;

class UserController extends Controller {
    public function index(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        7,
        "Method App\\Controllers\\UserController::index() overrides method App\\Controllers\\Controller::index() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 7, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("use Override;"),
        "should add use Override; import:\n{}",
        result
    );
    assert!(
        result.contains("#[Override]"),
        "should insert #[Override] attribute:\n{}",
        result
    );
}

// ── Non-namespaced file — no import needed ──────────────────────────────────

#[test]
fn no_import_in_non_namespaced_file() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);

    // Only one edit (the attribute), no import.
    assert_eq!(edits.len(), 1, "should have only attribute edit, no import");

    let result = apply_edits(content, &edits);
    assert!(
        result.contains("#[Override]"),
        "should insert #[Override]:\n{}",
        result
    );
    assert!(
        !result.contains("use Override;"),
        "should NOT add use import in non-namespaced file:\n{}",
        result
    );
}

// ── Method with docblock and existing attributes ────────────────────────────

#[test]
fn adds_override_between_docblock_and_attributes() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    /**
     * Handle the request.
     */
    #[Route('/foo')]
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        6, // `public function foo()` line
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 6, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    // #[Override] should be inserted before the existing #[Route] attribute.
    let override_pos = result.find("#[Override]").unwrap();
    let route_pos = result.find("#[Route").unwrap();
    assert!(
        override_pos < route_pos,
        "#[Override] should come before #[Route]:\n{}",
        result
    );
}

// ── Deep indentation ────────────────────────────────────────────────────────

#[test]
fn preserves_deep_indentation() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Outer {
    class Inner extends Base {
            public function deeply(): void {}
    }
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        3,
        "Method Inner::deeply() overrides method Base::deeply() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 3, 12);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("            #[Override]\n            public function deeply"),
        "should match the deep indentation:\n{}",
        result
    );
}

// ── Diagnostic attached to the code action ──────────────────────────────────

#[test]
fn attaches_diagnostic_to_code_action() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    let diag = inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let attached_diags = action
        .diagnostics
        .as_ref()
        .expect("should have diagnostics");
    assert_eq!(attached_diags.len(), 1);
    assert_eq!(attached_diags[0].message, diag.message);
}

// ── Override in attribute list with other attrs ─────────────────────────────

#[test]
fn no_action_when_override_in_combined_attr_list() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    #[Override, Deprecated]
    public function foo(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        3,
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 3, 10);
    let action = find_add_override_action(&actions);
    assert!(
        action.is_none(),
        "should NOT offer action when Override already in combined attribute list"
    );
}

// ── Multiple methods, only the targeted one gets the action ─────────────────

#[test]
fn only_targets_diagnosed_method() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Child extends Base {
    public function foo(): void {}
    public function bar(): void {}
}
"#;
    backend.update_ast(uri, content);

    // Only `foo` has the diagnostic.
    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Child::foo() overrides method Base::foo() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    // Request code actions on `bar` (line 3).
    let actions = get_code_actions(&backend, uri, content, 3, 10);
    let action = find_add_override_action(&actions);
    assert!(
        action.is_none(),
        "should NOT offer action on a different method"
    );

    // Request code actions on `foo` (line 2).
    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions);
    assert!(
        action.is_some(),
        "should offer action on the diagnosed method"
    );
}

// ── Interface method implementation ─────────────────────────────────────────

#[test]
fn adds_override_for_interface_implementation() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
class Foo implements BarInterface {
    public function handle(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        2,
        "Method Foo::handle() overrides method BarInterface::handle() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 2, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("    #[Override]\n    public function handle"),
        "should insert #[Override] for interface implementation:\n{}",
        result
    );
}

// ── Namespaced interface implementation — gets import ────────────────────────

#[test]
fn adds_import_for_namespaced_interface_implementation() {
    let backend = create_test_backend();
    let uri = "file:///test.php";
    let content = r#"<?php
namespace App\Services;

class Handler implements HandlerInterface {
    public function handle(): void {}
}
"#;
    backend.update_ast(uri, content);

    inject_phpstan_diag(
        &backend,
        uri,
        4,
        "Method App\\Services\\Handler::handle() overrides method App\\Services\\HandlerInterface::handle() but is missing the #[\\Override] attribute.",
        "method.missingOverride",
    );

    let actions = get_code_actions(&backend, uri, content, 4, 10);
    let action = find_add_override_action(&actions).expect("should offer action");

    let resolved = resolve_action(&backend, uri, content, action);
    let edits = extract_edits(&resolved);
    let result = apply_edits(content, &edits);

    assert!(
        result.contains("#[Override]"),
        "should insert #[Override]:\n{}",
        result
    );
    assert!(
        !result.contains("#[\\Override]"),
        "should use short form:\n{}",
        result
    );
    assert!(
        result.contains("use Override;"),
        "should add use import for namespaced file:\n{}",
        result
    );
}
