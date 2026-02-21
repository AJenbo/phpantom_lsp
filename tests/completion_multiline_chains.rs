mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Multi-line method chain completion tests ───────────────────────────────
//
// These tests verify that fluent method chains spanning multiple lines produce
// completions. The cursor is on a continuation line (one that starts with
// `->` or `?->` after optional whitespace), and the resolver must join the
// preceding lines to reconstruct the full chain expression.

/// Helper: open a document, send a completion request, return item labels.
async fn complete_at(
    backend: &phpantom_lsp::Backend,
    uri: &Url,
    text: &str,
    line: u32,
    character: u32,
) -> Vec<String> {
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    match result {
        Some(CompletionResponse::Array(items)) => items.iter().map(|i| i.label.clone()).collect(),
        _ => vec![],
    }
}

// ─── Basic multi-line chain ─────────────────────────────────────────────────

/// A two-line chain: `$this->getRepo()\n    ->` should resolve the return
/// type of `getRepo()` and offer its members.
#[tokio::test]
async fn test_multiline_chain_basic() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_basic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Repo {\n",
        "    public function findAll(): array { return []; }\n",
        "}\n",
        "class Service {\n",
        "    public function getRepo(): Repo { return new Repo(); }\n",
        "    public function run(): void {\n",
        "        $this->getRepo()\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 8 (`            ->`) right after `->`
    let names = complete_at(&backend, &uri, text, 8, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("findAll(")),
        "Should offer Repo::findAll(), got: {names:?}"
    );
}

// ─── Three-line chain ───────────────────────────────────────────────────────

/// A chain spanning three continuation lines.
#[tokio::test]
async fn test_multiline_chain_three_lines() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_three.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Builder {\n",
        "    public function where(): static { return $this; }\n",
        "    public function orderBy(): static { return $this; }\n",
        "    public function limit(): static { return $this; }\n",
        "    public function get(): array { return []; }\n",
        "}\n",
        "class QueryService {\n",
        "    public function query(): Builder { return new Builder(); }\n",
        "    public function run(): void {\n",
        "        $this->query()\n",
        "            ->where()\n",
        "            ->orderBy()\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 13 (`            ->`) after `->`
    let names = complete_at(&backend, &uri, text, 13, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("limit(")),
        "Should offer Builder::limit(), got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.starts_with("get(")),
        "Should offer Builder::get(), got: {names:?}"
    );
}

// ─── Nullsafe continuation ──────────────────────────────────────────────────

/// A chain that uses `?->` on a continuation line.
#[tokio::test]
async fn test_multiline_chain_nullsafe_continuation() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_nullsafe.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class City { public function getName(): string { return ''; } }\n",
        "class Address { public function getCity(): ?City { return null; } }\n",
        "class User { public function getAddress(): ?Address { return null; } }\n",
        "class App {\n",
        "    public function run(User $user): void {\n",
        "        $user->getAddress()\n",
        "            ?->getCity()\n",
        "            ?->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 8 (`            ?->`) after `?->`
    let names = complete_at(&backend, &uri, text, 8, 15).await;
    assert!(
        names.iter().any(|n| n.starts_with("getName(")),
        "Should offer City::getName() through nullsafe chain, got: {names:?}"
    );
}

// ─── Static method call base ────────────────────────────────────────────────

/// A chain starting with a static method call on the base line.
#[tokio::test]
async fn test_multiline_chain_static_base() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class QBuilder {\n",
        "    public static function create(): static { return new static(); }\n",
        "    public function select(): static { return $this; }\n",
        "    public function execute(): array { return []; }\n",
        "}\n",
        "class Runner {\n",
        "    public function run(): void {\n",
        "        QBuilder::create()\n",
        "            ->select()\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 10 (`            ->`) after `->`
    let names = complete_at(&backend, &uri, text, 10, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("execute(")),
        "Should offer QBuilder::execute(), got: {names:?}"
    );
}

// ─── Variable assignment chain ──────────────────────────────────────────────

/// A multi-line chain assigned to a variable: `$result = $this->foo()\n    ->bar()`.
/// Cursor is after `->` on the continuation, not at the end.
#[tokio::test]
async fn test_multiline_chain_with_variable_assignment() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_assign.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Paginator { public function total(): int { return 0; } }\n",
        "class QueryResult {\n",
        "    public function paginate(): Paginator { return new Paginator(); }\n",
        "}\n",
        "class DataService {\n",
        "    public function getData(): QueryResult { return new QueryResult(); }\n",
        "    public function run(): void {\n",
        "        $result = $this->getData()\n",
        "            ->paginate()\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 10 (`            ->`) after `->`
    let names = complete_at(&backend, &uri, text, 10, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("total(")),
        "Should offer Paginator::total(), got: {names:?}"
    );
}

// ─── Chain with arguments ───────────────────────────────────────────────────

/// Continuation lines include method calls with arguments.
#[tokio::test]
async fn test_multiline_chain_with_arguments() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_args.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Fluent {\n",
        "    public function set(string $key, mixed $value): static { return $this; }\n",
        "    public function build(): string { return ''; }\n",
        "}\n",
        "class Maker {\n",
        "    public function create(): Fluent { return new Fluent(); }\n",
        "    public function run(): void {\n",
        "        $this->create()\n",
        "            ->set('name', 'foo')\n",
        "            ->set('value', 42)\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 11 (`            ->`) after `->`
    let names = complete_at(&backend, &uri, text, 11, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("build(")),
        "Should offer Fluent::build(), got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.starts_with("set(")),
        "Should offer Fluent::set(), got: {names:?}"
    );
}

// ─── Single-line chain still works ──────────────────────────────────────────

/// Ensure single-line chains are not broken by the multi-line logic.
#[tokio::test]
async fn test_single_line_chain_still_works() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///single_line.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Item { public function getName(): string { return ''; } }\n",
        "class Container { public function first(): Item { return new Item(); } }\n",
        "class App {\n",
        "    public function getContainer(): Container { return new Container(); }\n",
        "    public function run(): void {\n",
        "        $this->getContainer()->first()->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 6 after `->`
    let names = complete_at(&backend, &uri, text, 6, 43).await;
    assert!(
        names.iter().any(|n| n.starts_with("getName(")),
        "Single-line chain should still offer Item::getName(), got: {names:?}"
    );
}

// ─── Cross-file multi-line chain ────────────────────────────────────────────

/// Multi-line chain where the return type comes from a different file
/// resolved via PSR-4.
#[tokio::test]
async fn test_multiline_chain_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[
            (
                "src/Builder.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Builder {\n",
                    "    public function where(): static { return $this; }\n",
                    "    public function first(): ?Item { return null; }\n",
                    "}\n",
                ),
            ),
            (
                "src/Item.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Item {\n",
                    "    public function getId(): int { return 0; }\n",
                    "}\n",
                ),
            ),
        ],
    );

    let uri = Url::parse("file:///test_multiline_cross.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Builder;\n",
        "class Service {\n",
        "    public function getBuilder(): Builder { return new Builder(); }\n",
        "    public function run(): void {\n",
        "        $this->getBuilder()\n",
        "            ->where()\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    let names = complete_at(&backend, &uri, text, 7, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("where(")),
        "Should offer Builder::where() through multi-line PSR-4 chain, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.starts_with("first(")),
        "Should offer Builder::first() through multi-line PSR-4 chain, got: {names:?}"
    );
}

// ─── Partial identifier on continuation line ────────────────────────────────

/// When the user has started typing an identifier on the continuation line,
/// completion should still work (the partial text is filtered by the editor).
#[tokio::test]
async fn test_multiline_chain_partial_identifier() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_partial.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Pipeline {\n",
        "    public function through(): static { return $this; }\n",
        "    public function then(): mixed { return null; }\n",
        "    public function thenReturn(): mixed { return null; }\n",
        "}\n",
        "class App {\n",
        "    public function pipe(): Pipeline { return new Pipeline(); }\n",
        "    public function run(): void {\n",
        "        $this->pipe()\n",
        "            ->through()\n",
        "            ->the\n",
        "    }\n",
        "}\n",
    );

    // Cursor after `the` on line 11
    let names = complete_at(&backend, &uri, text, 11, 17).await;
    assert!(
        names.iter().any(|n| n.starts_with("then(")),
        "Should offer Pipeline::then() with partial identifier, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.starts_with("thenReturn(")),
        "Should offer Pipeline::thenReturn() with partial identifier, got: {names:?}"
    );
}

// ─── $this on its own line ──────────────────────────────────────────────────

/// When `$this` is on the line above and the continuation starts with `->`.
#[tokio::test]
async fn test_multiline_chain_this_on_own_line() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_this.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Widget {\n",
        "    public function render(): string { return ''; }\n",
        "    public function show(): void {\n",
        "        $this\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 5 (`            ->`) after `->`
    let names = complete_at(&backend, &uri, text, 5, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("render(")),
        "Should offer Widget::render() when $this is on preceding line, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.starts_with("show(")),
        "Should offer Widget::show() when $this is on preceding line, got: {names:?}"
    );
}

// ─── Chain with closure argument ────────────────────────────────────────────

/// Continuation lines that include closure arguments (contains `->` inside
/// the closure that should not confuse the extraction).
#[tokio::test]
async fn test_multiline_chain_with_closure_arg() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///multiline_closure.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Collection {\n",
        "    public function filter(callable $fn): static { return $this; }\n",
        "    public function map(callable $fn): static { return $this; }\n",
        "    public function count(): int { return 0; }\n",
        "}\n",
        "class Processor {\n",
        "    public function items(): Collection { return new Collection(); }\n",
        "    public function run(): void {\n",
        "        $this->items()\n",
        "            ->filter(fn($x) => $x > 0)\n",
        "            ->map(fn($x) => $x * 2)\n",
        "            ->\n",
        "    }\n",
        "}\n",
    );

    // Cursor on line 12 (`            ->`) after `->`
    let names = complete_at(&backend, &uri, text, 12, 14).await;
    assert!(
        names.iter().any(|n| n.starts_with("count(")),
        "Should offer Collection::count() after chain with closure args, got: {names:?}"
    );
    assert!(
        names.iter().any(|n| n.starts_with("filter(")),
        "Should offer Collection::filter() after chain with closure args, got: {names:?}"
    );
}
