#![cfg(test)]

use crate::Backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

/// Helper: open a file in the backend.
async fn open_file(backend: &Backend, uri: &Url, text: &str) {
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;
}

/// Helper: send a prepare-rename request and return the response.
async fn prepare_rename(
    backend: &Backend,
    uri: &Url,
    line: u32,
    character: u32,
) -> Option<PrepareRenameResponse> {
    let params = TextDocumentPositionParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        position: Position { line, character },
    };

    backend.prepare_rename(params).await.unwrap()
}

/// Helper: send a rename request and return the workspace edit.
async fn rename(
    backend: &Backend,
    uri: &Url,
    line: u32,
    character: u32,
    new_name: &str,
) -> Option<WorkspaceEdit> {
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        new_name: new_name.to_string(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    };

    backend.rename(params).await.unwrap()
}

/// Collect all text edits for a given URI from a WorkspaceEdit.
fn edits_for_uri(edit: &WorkspaceEdit, uri: &Url) -> Vec<TextEdit> {
    edit.changes
        .as_ref()
        .and_then(|changes| changes.get(uri))
        .cloned()
        .unwrap_or_default()
}

/// Apply a set of text edits to source text and return the result.
/// Edits must not overlap; they are applied from last to first.
fn apply_edits(source: &str, edits: &[TextEdit]) -> String {
    let mut sorted: Vec<_> = edits.to_vec();
    // Sort by start position descending so we can apply from the end.
    sorted.sort_by(|a, b| {
        b.range
            .start
            .line
            .cmp(&a.range.start.line)
            .then(b.range.start.character.cmp(&a.range.start.character))
    });

    let lines: Vec<&str> = source.lines().collect();
    let mut result = source.to_string();

    for edit in &sorted {
        let start_offset = line_col_to_offset(&lines, edit.range.start);
        let end_offset = line_col_to_offset(&lines, edit.range.end);
        result.replace_range(start_offset..end_offset, &edit.new_text);
    }

    result
}

fn line_col_to_offset(lines: &[&str], pos: Position) -> usize {
    let mut offset = 0;
    for (i, line) in lines.iter().enumerate() {
        if i == pos.line as usize {
            return offset + pos.character as usize;
        }
        offset += line.len() + 1; // +1 for newline
    }
    offset
}

// ─── Variable Rename ────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_variable_in_function() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function demo(): void {\n",
        "    $user = new User();\n",
        "    $user->name = 'Alice';\n",
        "    echo $user->name;\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // Rename $user on line 2 (the assignment)
    let edit = rename(&backend, &uri, 2, 5, "$person").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for variable rename"
    );

    let edit = edit.unwrap();
    let file_edits = edits_for_uri(&edit, &uri);
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for $user (decl + 2 usages), got {}",
        file_edits.len()
    );

    // All edits should use the new name with `$`.
    for te in &file_edits {
        assert_eq!(te.new_text, "$person");
    }
}

#[tokio::test]
async fn rename_variable_without_dollar_prefix() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function demo(): void {\n",
        "    $x = 1;\n",
        "    echo $x;\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // User provides new name without `$` — the handler should add it.
    let edit = rename(&backend, &uri, 2, 5, "y").await;
    assert!(edit.is_some());

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    for te in &file_edits {
        assert_eq!(te.new_text, "$y");
    }
}

#[tokio::test]
async fn prepare_rename_variable() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function demo(): void {\n",
        "    $count = 0;\n",
        "    $count++;\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    let response = prepare_rename(&backend, &uri, 2, 6).await;
    assert!(
        response.is_some(),
        "Expected prepare rename to succeed for $count"
    );

    if let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) = response {
        assert_eq!(placeholder, "$count");
    } else {
        panic!("Expected RangeWithPlaceholder response");
    }
}

// ─── Non-Renameable Symbols ─────────────────────────────────────────────────

#[tokio::test]
async fn prepare_rename_rejects_this() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function bar(): void {\n",
        "        $this->baz();\n",
        "    }\n",
        "    public function baz(): void {}\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // `$this` should not be renameable.
    let response = prepare_rename(&backend, &uri, 3, 9).await;
    assert!(response.is_none(), "$this should not be renameable");
}

#[tokio::test]
async fn prepare_rename_rejects_self() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public static function create(): self {\n",
        "        return new self();\n",
        "    }\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // `self` keyword on line 3 should not be renameable.
    let response = prepare_rename(&backend, &uri, 3, 20).await;
    assert!(response.is_none(), "self keyword should not be renameable");
}

#[tokio::test]
async fn prepare_rename_rejects_static() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public static function create(): static {\n",
        "        return new static();\n",
        "    }\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    let response = prepare_rename(&backend, &uri, 3, 22).await;
    assert!(
        response.is_none(),
        "static keyword should not be renameable"
    );
}

#[tokio::test]
async fn prepare_rename_rejects_parent() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function hello(): void {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    public function hello(): void {\n",
        "        parent::hello();\n",
        "    }\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    let response = prepare_rename(&backend, &uri, 6, 10).await;
    assert!(
        response.is_none(),
        "parent keyword should not be renameable"
    );
}

// ─── Class Rename ───────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_class_same_file() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function log(string $msg): void {}\n",
        "}\n",
        "function demo(Logger $logger): void {\n",
        "    $obj = new Logger();\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // Rename from a reference site (type hint on line 4).
    let edit = rename(&backend, &uri, 4, 16, "AppLogger").await;
    assert!(edit.is_some(), "Expected a workspace edit for class rename");

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    // Should find: declaration (L1), type hint (L4), new (L5) = at least 3.
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for Logger, got {}",
        file_edits.len()
    );

    for te in &file_edits {
        assert_eq!(te.new_text, "AppLogger");
    }
}

#[tokio::test]
async fn rename_class_from_declaration() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Widget {\n",
        "    public function render(): string { return ''; }\n",
        "}\n",
        "function demo(Widget $w): void {\n",
        "    $obj = new Widget();\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // Rename from the declaration site (line 1).
    let edit = rename(&backend, &uri, 1, 7, "Component").await;
    assert!(edit.is_some());

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for Widget, got {}",
        file_edits.len()
    );

    for te in &file_edits {
        assert_eq!(te.new_text, "Component");
    }
}

#[tokio::test]
async fn prepare_rename_class() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {}\n",
        "function demo(Foo $f): void {}\n",
    );

    open_file(&backend, &uri, text).await;

    let response = prepare_rename(&backend, &uri, 1, 7).await;
    assert!(response.is_some());

    if let Some(PrepareRenameResponse::RangeWithPlaceholder { placeholder, .. }) = response {
        assert_eq!(placeholder, "Foo");
    } else {
        panic!("Expected RangeWithPlaceholder response");
    }
}

// ─── Method Rename ──────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_method() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Service {\n",
        "    public function process(): void {}\n",
        "}\n",
        "function demo(): void {\n",
        "    $s = new Service();\n",
        "    $s->process();\n",
        "    $s->process();\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // Rename from call site (line 6).
    let edit = rename(&backend, &uri, 6, 9, "execute").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for method rename"
    );

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    // Should find: declaration (L2) + 2 call sites (L6, L7) = at least 3.
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for process, got {}",
        file_edits.len()
    );

    for te in &file_edits {
        assert_eq!(te.new_text, "execute");
    }
}

#[tokio::test]
async fn rename_static_method() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public static function create(): self { return new self(); }\n",
        "}\n",
        "function demo(): void {\n",
        "    Factory::create();\n",
        "    Factory::create();\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    let edit = rename(&backend, &uri, 5, 14, "build").await;
    assert!(edit.is_some());

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for create, got {}",
        file_edits.len()
    );

    for te in &file_edits {
        assert_eq!(te.new_text, "build");
    }
}

// ─── Property Rename ────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_property_from_access() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name = '';\n",
        "    public function greet(): string {\n",
        "        return $this->name;\n",
        "    }\n",
        "}\n",
        "function demo(): void {\n",
        "    $u = new User();\n",
        "    $u->name = 'Alice';\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // Rename from access site (line 9, `$u->name`).
    let edit = rename(&backend, &uri, 9, 9, "displayName").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for property rename"
    );

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    // Should have edits for: declaration ($name), $this->name, $u->name.
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for name property, got {}",
        file_edits.len()
    );

    // The declaration site includes `$`, access sites don't.
    for te in &file_edits {
        assert!(
            te.new_text == "displayName" || te.new_text == "$displayName",
            "Unexpected edit text: {}",
            te.new_text
        );
    }
}

// ─── Function Rename ────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_function() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function helper(): void {}\n",
        "function demo(): void {\n",
        "    helper();\n",
        "    helper();\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    let edit = rename(&backend, &uri, 3, 6, "utility").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for function rename"
    );

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    // declaration (L1) + 2 call sites (L3, L4) = at least 3.
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for helper, got {}",
        file_edits.len()
    );

    for te in &file_edits {
        assert_eq!(te.new_text, "utility");
    }
}

// ─── Constant Rename ────────────────────────────────────────────────────────

#[tokio::test]
async fn rename_class_constant() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Status {\n",
        "    const ACTIVE = 1;\n",
        "}\n",
        "function demo(): void {\n",
        "    echo Status::ACTIVE;\n",
        "    $x = Status::ACTIVE;\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    let edit = rename(&backend, &uri, 5, 19, "ENABLED").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for constant rename"
    );

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    assert!(
        file_edits.len() >= 3,
        "Expected at least 3 edits for ACTIVE, got {}",
        file_edits.len()
    );

    for te in &file_edits {
        assert_eq!(te.new_text, "ENABLED");
    }
}

// ─── Cross-file Rename ─────────────────────────────────────────────────────

#[tokio::test]
async fn rename_class_cross_file() {
    let backend = Backend::new_test();
    let uri_a = Url::parse("file:///a.php").unwrap();
    let uri_b = Url::parse("file:///b.php").unwrap();

    let text_a = concat!(
        "<?php\n",
        "class Animal {\n",
        "    public function speak(): string { return ''; }\n",
        "}\n",
    );

    let text_b = concat!(
        "<?php\n",
        "function demo(Animal $a): void {\n",
        "    $obj = new Animal();\n",
        "}\n",
    );

    open_file(&backend, &uri_a, text_a).await;
    open_file(&backend, &uri_b, text_b).await;

    // Rename from file a (declaration).
    let edit = rename(&backend, &uri_a, 1, 7, "Creature").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for cross-file class rename"
    );

    let edit = edit.unwrap();
    let edits_a = edits_for_uri(&edit, &uri_a);
    let edits_b = edits_for_uri(&edit, &uri_b);

    assert!(
        !edits_a.is_empty(),
        "Expected edits in file a (declaration)"
    );
    assert!(!edits_b.is_empty(), "Expected edits in file b (references)");

    for te in edits_a.iter().chain(edits_b.iter()) {
        assert_eq!(te.new_text, "Creature");
    }
}

#[tokio::test]
async fn rename_method_cross_file() {
    let backend = Backend::new_test();
    let uri_a = Url::parse("file:///a.php").unwrap();
    let uri_b = Url::parse("file:///b.php").unwrap();

    let text_a = concat!(
        "<?php\n",
        "class Printer {\n",
        "    public function print(): void {}\n",
        "}\n",
    );

    let text_b = concat!(
        "<?php\n",
        "function demo(): void {\n",
        "    $p = new Printer();\n",
        "    $p->print();\n",
        "}\n",
    );

    open_file(&backend, &uri_a, text_a).await;
    open_file(&backend, &uri_b, text_b).await;

    // Rename from the call site in file b.
    let edit = rename(&backend, &uri_b, 3, 9, "output").await;
    assert!(edit.is_some());

    let edit = edit.unwrap();
    let edits_a = edits_for_uri(&edit, &uri_a);
    let edits_b = edits_for_uri(&edit, &uri_b);

    assert!(
        !edits_a.is_empty(),
        "Expected edits in file a (declaration)"
    );
    assert!(!edits_b.is_empty(), "Expected edits in file b (call site)");

    for te in edits_a.iter().chain(edits_b.iter()) {
        assert_eq!(te.new_text, "output");
    }
}

// ─── Whitespace / No Symbol ─────────────────────────────────────────────────

#[tokio::test]
async fn prepare_rename_on_whitespace_returns_none() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "\n", "function demo(): void {}\n",);

    open_file(&backend, &uri, text).await;

    // Line 1 is blank.
    let response = prepare_rename(&backend, &uri, 1, 0).await;
    assert!(response.is_none(), "Expected no rename on whitespace");
}

#[tokio::test]
async fn rename_on_whitespace_returns_none() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "\n", "function demo(): void {}\n",);

    open_file(&backend, &uri, text).await;

    let edit = rename(&backend, &uri, 1, 0, "anything").await;
    assert!(edit.is_none(), "Expected no edit on whitespace");
}

// ─── Result Correctness ─────────────────────────────────────────────────────

#[tokio::test]
async fn rename_variable_produces_valid_php() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function demo(): void {\n",
        "    $a = 1;\n",
        "    $b = $a + 2;\n",
        "    echo $a;\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    let edit = rename(&backend, &uri, 2, 5, "$z").await;
    assert!(edit.is_some());

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    let result = apply_edits(text, &file_edits);

    // The renamed variable should appear as `$z` everywhere.
    assert!(result.contains("$z = 1;"), "Declaration not renamed");
    assert!(result.contains("$b = $z + 2;"), "RHS usage not renamed");
    assert!(result.contains("echo $z;"), "Echo usage not renamed");
    // And the old name should be gone.
    assert!(!result.contains("$a"), "Old variable name still present");
}

// ─── Variable Scoping ───────────────────────────────────────────────────────

#[tokio::test]
async fn rename_variable_does_not_leak_across_functions() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "function alpha(): void {\n",
        "    $x = 1;\n",
        "    echo $x;\n",
        "}\n",
        "function beta(): void {\n",
        "    $x = 2;\n",
        "    echo $x;\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // Rename $x in alpha (line 2).
    let edit = rename(&backend, &uri, 2, 5, "$y").await;
    assert!(edit.is_some());

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    let result = apply_edits(text, &file_edits);

    // alpha should have $y, beta should still have $x.
    assert!(result.contains("function alpha(): void {\n    $y = 1;\n    echo $y;\n}"));
    assert!(result.contains("function beta(): void {\n    $x = 2;\n    echo $x;\n}"));
}

// ─── Class-Aware Member Rename ──────────────────────────────────────────────

#[tokio::test]
async fn rename_method_does_not_leak_to_unrelated_class() {
    // Two unrelated classes with the same method name.  Renaming the
    // method on one class must not touch the other.
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                 // L0
        "class Dog {\n",                           // L1
        "    public function speak(): void {}\n",  // L2
        "}\n",                                     // L3
        "class Cat {\n",                           // L4
        "    public function speak(): void {}\n",  // L5
        "}\n",                                     // L6
        "function demo(Dog $d, Cat $c): void {\n", // L7
        "    $d->speak();\n",                      // L8
        "    $c->speak();\n",                      // L9
        "}\n",                                     // L10
    );

    open_file(&backend, &uri, text).await;

    // Rename speak() from the Dog::speak declaration (line 2, col 21).
    // "    public function speak(): void {}"
    //                     ^ col 20
    let edit = rename(&backend, &uri, 2, 21, "bark").await;
    assert!(edit.is_some(), "Rename should produce edits");

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    let result = apply_edits(text, &file_edits);

    // Dog::speak and $d->speak should be renamed to bark.
    assert!(
        result.contains("function bark()"),
        "Dog's method should be renamed to bark; got:\n{}",
        result
    );
    assert!(
        result.contains("$d->bark()"),
        "$d->speak() should become $d->bark(); got:\n{}",
        result
    );

    // Cat::speak and $c->speak must NOT be renamed.
    assert!(
        result.contains("class Cat {\n    public function speak(): void {}"),
        "Cat's method should remain speak; got:\n{}",
        result
    );
    assert!(
        result.contains("$c->speak()"),
        "$c->speak() should remain unchanged; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_method_includes_inherited_class() {
    // Renaming a method on a parent class should also rename it on
    // accesses through a child class.
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                    // L0
        "class Base {\n",                             // L1
        "    public function run(): void {}\n",       // L2
        "}\n",                                        // L3
        "class Child extends Base {}\n",              // L4
        "function demo(Base $b, Child $c): void {\n", // L5
        "    $b->run();\n",                           // L6
        "    $c->run();\n",                           // L7
        "}\n",                                        // L8
    );

    open_file(&backend, &uri, text).await;

    // Rename run() from $b->run() (line 6, col 10).
    let edit = rename(&backend, &uri, 6, 10, "execute").await;
    assert!(edit.is_some(), "Rename should produce edits");

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    let result = apply_edits(text, &file_edits);

    // Both $b->run() and $c->run() should be renamed (Child extends Base).
    assert!(
        result.contains("$b->execute()"),
        "$b->run() should become $b->execute(); got:\n{}",
        result
    );
    assert!(
        result.contains("$c->execute()"),
        "$c->run() should become $c->execute() (inherited); got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_static_method_does_not_leak_to_unrelated_class() {
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",                                        // L0
        "class Alpha {\n",                                // L1
        "    public static function create(): void {}\n", // L2
        "}\n",                                            // L3
        "class Beta {\n",                                 // L4
        "    public static function create(): void {}\n", // L5
        "}\n",                                            // L6
        "function demo(): void {\n",                      // L7
        "    Alpha::create();\n",                         // L8
        "    Beta::create();\n",                          // L9
        "}\n",                                            // L10
    );

    open_file(&backend, &uri, text).await;

    // Rename create() from Alpha::create() call (line 8, col 12).
    // "    Alpha::create();"
    //             ^ col 11
    let edit = rename(&backend, &uri, 8, 12, "make").await;
    assert!(edit.is_some(), "Rename should produce edits");

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    let result = apply_edits(text, &file_edits);

    // Alpha::create should be renamed.
    assert!(
        result.contains("Alpha::make()"),
        "Alpha::create() should become Alpha::make(); got:\n{}",
        result
    );

    // Beta::create must NOT be renamed.
    assert!(
        result.contains("Beta::create()"),
        "Beta::create() should remain unchanged; got:\n{}",
        result
    );
}

// ─── Use-Statement-Aware Class Rename ───────────────────────────────────────

#[tokio::test]
async fn rename_class_updates_use_import() {
    // Renaming a class should update the `use` statement FQN (last segment)
    // as well as all in-code references.
    let backend = Backend::new_test();
    let uri_decl = Url::parse("file:///src/TaskResource.php").unwrap();
    let uri_usage = Url::parse("file:///src/Task.php").unwrap();

    let text_decl = concat!(
        "<?php\n",
        "namespace Eagle\\Tasks\\Resources;\n",
        "\n",
        "class TaskResource {}\n",
    );

    let text_usage = concat!(
        "<?php\n",
        "namespace Eagle\\Tasks;\n",
        "\n",
        "use Eagle\\Tasks\\Resources\\TaskResource;\n",
        "\n",
        "class Task {\n",
        "    protected static string $service = TaskResource::class;\n",
        "}\n",
    );

    open_file(&backend, &uri_decl, text_decl).await;
    open_file(&backend, &uri_usage, text_usage).await;

    // Rename from the declaration site (line 3, col 6 = "TaskResource").
    let edit = rename(&backend, &uri_decl, 3, 6, "TaskResourceService").await;
    assert!(edit.is_some(), "Expected a workspace edit for class rename");

    let ws = edit.unwrap();
    let edits_usage = edits_for_uri(&ws, &uri_usage);
    assert!(!edits_usage.is_empty(), "Expected edits in the usage file");

    let result = apply_edits(text_usage, &edits_usage);

    // The use statement should have the FQN last segment updated.
    assert!(
        result.contains("use Eagle\\Tasks\\Resources\\TaskResourceService;"),
        "Use statement should be updated; got:\n{}",
        result
    );

    // The in-code reference should be renamed.
    assert!(
        result.contains("TaskResourceService::class"),
        "In-code reference should be renamed; got:\n{}",
        result
    );

    // The old name should NOT appear.
    assert!(
        !result.contains("TaskResource::class"),
        "Old name should not remain; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_class_preserves_explicit_alias() {
    // When a file imports the class with an explicit alias, the alias
    // should be preserved and in-code references should NOT be renamed.
    let backend = Backend::new_test();
    let uri_decl = Url::parse("file:///src/TaskResource.php").unwrap();
    let uri_usage = Url::parse("file:///src/Controller.php").unwrap();

    let text_decl = concat!(
        "<?php\n",
        "namespace Eagle\\Tasks\\Resources;\n",
        "\n",
        "class TaskResource {}\n",
    );

    let text_usage = concat!(
        "<?php\n",
        "namespace Eagle\\Tasks\\Http;\n",
        "\n",
        "use Eagle\\Tasks\\Resources\\TaskResource as ResourceService;\n",
        "\n",
        "class Controller {\n",
        "    private ResourceService $service;\n",
        "}\n",
    );

    open_file(&backend, &uri_decl, text_decl).await;
    open_file(&backend, &uri_usage, text_usage).await;

    // Rename from the declaration.
    let edit = rename(&backend, &uri_decl, 3, 6, "TaskResourceService").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for aliased class rename"
    );

    let ws = edit.unwrap();
    let edits_usage = edits_for_uri(&ws, &uri_usage);

    let result = apply_edits(text_usage, &edits_usage);

    // The use statement FQN should be updated, but the alias kept.
    assert!(
        result.contains("use Eagle\\Tasks\\Resources\\TaskResourceService as ResourceService;"),
        "Use statement FQN should update, alias preserved; got:\n{}",
        result
    );

    // In-code references via the alias should NOT change.
    assert!(
        result.contains("private ResourceService $service;"),
        "Alias-based references should remain unchanged; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_class_with_collision_adds_alias() {
    // When renaming would produce a short name that collides with an
    // existing import, an alias should be introduced.
    let backend = Backend::new_test();
    let uri_a = Url::parse("file:///src/OldName.php").unwrap();
    let uri_b = Url::parse("file:///src/NewName.php").unwrap();
    let uri_usage = Url::parse("file:///src/Usage.php").unwrap();

    let text_a = concat!("<?php\n", "namespace Ns\\A;\n", "\n", "class OldName {}\n",);

    let text_b = concat!("<?php\n", "namespace Ns\\B;\n", "\n", "class NewName {}\n",);

    let text_usage = concat!(
        "<?php\n",
        "use Ns\\A\\OldName;\n",
        "use Ns\\B\\NewName;\n",
        "\n",
        "function demo(OldName $a, NewName $b): void {}\n",
    );

    open_file(&backend, &uri_a, text_a).await;
    open_file(&backend, &uri_b, text_b).await;
    open_file(&backend, &uri_usage, text_usage).await;

    // Rename OldName → NewName (which collides with an existing import).
    let edit = rename(&backend, &uri_a, 3, 6, "NewName").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit for colliding class rename"
    );

    let ws = edit.unwrap();
    let edits_usage = edits_for_uri(&ws, &uri_usage);
    let result = apply_edits(text_usage, &edits_usage);

    // The existing `use Ns\B\NewName;` should remain unchanged.
    assert!(
        result.contains("use Ns\\B\\NewName;"),
        "Existing import should remain unchanged; got:\n{}",
        result
    );

    // The renamed import should get an alias to avoid collision.
    assert!(
        result.contains("use Ns\\A\\NewName as NewNameAlias;"),
        "Renamed import should get an alias; got:\n{}",
        result
    );

    // In-code references to the renamed class should use the alias.
    assert!(
        result.contains("NewNameAlias $a"),
        "In-code references should use the alias; got:\n{}",
        result
    );

    // The other class's references should be unaffected.
    assert!(
        result.contains("NewName $b"),
        "Other class references should remain; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_class_same_file_no_use_statement() {
    // Renaming a class in the same file (no use statement) should still work.
    let backend = Backend::new_test();
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function log(string $msg): void {}\n",
        "}\n",
        "function demo(Logger $logger): void {\n",
        "    $obj = new Logger();\n",
        "}\n",
    );

    open_file(&backend, &uri, text).await;

    // Rename from the declaration.
    let edit = rename(&backend, &uri, 1, 7, "AppLogger").await;
    assert!(edit.is_some(), "Expected a workspace edit");

    let file_edits = edits_for_uri(&edit.unwrap(), &uri);
    let result = apply_edits(text, &file_edits);

    assert!(
        result.contains("class AppLogger"),
        "Declaration should be renamed; got:\n{}",
        result
    );
    assert!(
        result.contains("function demo(AppLogger"),
        "Type hint should be renamed; got:\n{}",
        result
    );
    assert!(
        result.contains("new AppLogger()"),
        "new expression should be renamed; got:\n{}",
        result
    );
    // Verify no standalone "Logger" remains (AppLogger is fine).
    let has_standalone_old_name = result
        .lines()
        .any(|l| l.contains("Logger") && !l.contains("AppLogger"));
    assert!(
        !has_standalone_old_name,
        "Old standalone name should not remain; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_class_updates_use_import_from_reference_site() {
    // Trigger rename from a reference site (not the declaration) and
    // verify the use statement is still updated.
    let backend = Backend::new_test();
    let uri_decl = Url::parse("file:///src/Animal.php").unwrap();
    let uri_usage = Url::parse("file:///src/Zoo.php").unwrap();

    let text_decl = concat!(
        "<?php\n",
        "namespace Zoo\\Models;\n",
        "\n",
        "class Animal {}\n",
    );

    let text_usage = concat!(
        "<?php\n",
        "use Zoo\\Models\\Animal;\n",
        "\n",
        "function feed(Animal $a): void {}\n",
    );

    open_file(&backend, &uri_decl, text_decl).await;
    open_file(&backend, &uri_usage, text_usage).await;

    // Rename from the reference site in the usage file (line 3, col 15).
    // "function feed(Animal $a): void {}"
    //                ^ col 14
    let edit = rename(&backend, &uri_usage, 3, 15, "Creature").await;
    assert!(
        edit.is_some(),
        "Expected a workspace edit when renaming from reference"
    );

    let ws = edit.unwrap();
    let edits_usage = edits_for_uri(&ws, &uri_usage);
    let result = apply_edits(text_usage, &edits_usage);

    assert!(
        result.contains("use Zoo\\Models\\Creature;"),
        "Use statement should be updated; got:\n{}",
        result
    );
    assert!(
        result.contains("function feed(Creature $a)"),
        "In-code reference should be renamed; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_class_cross_file_use_import_multiple_refs() {
    // A file with multiple references to the renamed class (via use
    // import) should have all references and the use statement updated.
    let backend = Backend::new_test();
    let uri_decl = Url::parse("file:///src/Repo.php").unwrap();
    let uri_usage = Url::parse("file:///src/Service.php").unwrap();

    let text_decl = concat!(
        "<?php\n",
        "namespace App\\Repos;\n",
        "\n",
        "class UserRepo {}\n",
    );

    let text_usage = concat!(
        "<?php\n",
        "use App\\Repos\\UserRepo;\n",
        "\n",
        "class Service {\n",
        "    private UserRepo $repo;\n",
        "    public function getRepo(): UserRepo {\n",
        "        return new UserRepo();\n",
        "    }\n",
        "}\n",
    );

    open_file(&backend, &uri_decl, text_decl).await;
    open_file(&backend, &uri_usage, text_usage).await;

    let edit = rename(&backend, &uri_decl, 3, 6, "UserRepository").await;
    assert!(edit.is_some());

    let ws = edit.unwrap();
    let edits_usage = edits_for_uri(&ws, &uri_usage);
    let result = apply_edits(text_usage, &edits_usage);

    assert!(
        result.contains("use App\\Repos\\UserRepository;"),
        "Use statement should be updated; got:\n{}",
        result
    );
    assert!(
        result.contains("private UserRepository $repo;"),
        "Property type should be renamed; got:\n{}",
        result
    );
    assert!(
        result.contains("getRepo(): UserRepository"),
        "Return type should be renamed; got:\n{}",
        result
    );
    assert!(
        result.contains("new UserRepository()"),
        "new expression should be renamed; got:\n{}",
        result
    );
    // Verify no standalone "UserRepo" remains (UserRepository is fine).
    let has_standalone_old_name = result
        .lines()
        .any(|l| l.contains("UserRepo") && !l.contains("UserRepository"));
    assert!(
        !has_standalone_old_name,
        "Old standalone name should not remain; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_class_fqn_inline_reference() {
    // When a file uses the class via an inline FQN (no use statement),
    // only the last segment should be renamed.
    let backend = Backend::new_test();
    let uri_decl = Url::parse("file:///src/Item.php").unwrap();
    let uri_usage = Url::parse("file:///src/other.php").unwrap();

    let text_decl = concat!("<?php\n", "namespace Shop;\n", "\n", "class Item {}\n",);

    let text_usage = concat!(
        "<?php\n",
        "function demo(): void {\n",
        "    $x = new \\Shop\\Item();\n",
        "}\n",
    );

    open_file(&backend, &uri_decl, text_decl).await;
    open_file(&backend, &uri_usage, text_usage).await;

    let edit = rename(&backend, &uri_decl, 3, 6, "Product").await;
    assert!(edit.is_some());

    let ws = edit.unwrap();
    let edits_usage = edits_for_uri(&ws, &uri_usage);
    let result = apply_edits(text_usage, &edits_usage);

    // The inline FQN should have only the last segment renamed.
    assert!(
        result.contains("\\Shop\\Product()"),
        "Inline FQN should update last segment only; got:\n{}",
        result
    );
}

#[tokio::test]
async fn rename_class_declaration_updates_in_same_namespace() {
    // Two files in the same namespace — references use the short name
    // without a use statement.  The rename should just update the short name.
    let backend = Backend::new_test();
    let uri_a = Url::parse("file:///src/Foo.php").unwrap();
    let uri_b = Url::parse("file:///src/Bar.php").unwrap();

    let text_a = concat!("<?php\n", "namespace App;\n", "\n", "class Foo {}\n",);

    let text_b = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Bar extends Foo {}\n",
    );

    open_file(&backend, &uri_a, text_a).await;
    open_file(&backend, &uri_b, text_b).await;

    let edit = rename(&backend, &uri_a, 3, 6, "Baz").await;
    assert!(edit.is_some());

    let ws = edit.unwrap();
    let edits_a = edits_for_uri(&ws, &uri_a);
    let edits_b = edits_for_uri(&ws, &uri_b);

    let result_a = apply_edits(text_a, &edits_a);
    let result_b = apply_edits(text_b, &edits_b);

    assert!(
        result_a.contains("class Baz"),
        "Declaration should be renamed; got:\n{}",
        result_a
    );
    assert!(
        result_b.contains("extends Baz"),
        "Cross-file reference should be renamed; got:\n{}",
        result_b
    );
}
