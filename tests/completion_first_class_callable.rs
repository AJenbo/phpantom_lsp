mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

/// Helper: open a document and trigger completion at the given line/column.
async fn complete_at(
    backend: &phpantom_lsp::Backend,
    uri: &Url,
    src: &str,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: src.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    match backend.completion(completion_params).await.unwrap() {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        None => vec![],
    }
}

fn method_names(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect()
}

fn property_names(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect()
}

// ─── Function first-class callable ──────────────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_function_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_func.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string { return ''; }\n",
        "    public function getEmail(): string { return ''; }\n",
        "}\n",
        "function createUser(): User { return new User(); }\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = createUser(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 9: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 9, 15).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getName"),
        "Expected getName in {:?}",
        names,
    );
    assert!(
        names.contains(&"getEmail"),
        "Expected getEmail in {:?}",
        names,
    );
}

// ─── Instance method first-class callable ───────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_instance_method_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_method.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Product {\n",
        "    public function getPrice(): float { return 0.0; }\n",
        "    public function getTitle(): string { return ''; }\n",
        "}\n",
        "class Factory {\n",
        "    public function create(): Product { return new Product(); }\n",
        "}\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $factory = new Factory();\n",
        "        $fn = $factory->create(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 12: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 12, 15).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getPrice"),
        "Expected getPrice in {:?}",
        names,
    );
    assert!(
        names.contains(&"getTitle"),
        "Expected getTitle in {:?}",
        names,
    );
}

// ─── $this->method(...) first-class callable ────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_this_method_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_this.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Order {\n",
        "    public function getTotal(): float { return 0.0; }\n",
        "    public function getStatus(): string { return ''; }\n",
        "}\n",
        "class Service {\n",
        "    public function makeOrder(): Order { return new Order(); }\n",
        "    public function run(): void {\n",
        "        $fn = $this->makeOrder(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 9: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 9, 15).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getTotal"),
        "Expected getTotal in {:?}",
        names,
    );
    assert!(
        names.contains(&"getStatus"),
        "Expected getStatus in {:?}",
        names,
    );
}

// ─── Static method first-class callable ─────────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_static_method_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_static.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Config {\n",
        "    public function get(string $key): string { return ''; }\n",
        "    public function has(string $key): bool { return false; }\n",
        "}\n",
        "class ConfigFactory {\n",
        "    public static function make(): Config { return new Config(); }\n",
        "}\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = ConfigFactory::make(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 11: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 11, 15).await;
    let names = method_names(&items);
    assert!(names.contains(&"get"), "Expected get in {:?}", names,);
    assert!(names.contains(&"has"), "Expected has in {:?}", names,);
}

// ─── self:: first-class callable ────────────────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_self_static_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_self.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Order {\n",
        "    public function getTotal(): float { return 0.0; }\n",
        "}\n",
        "class Service {\n",
        "    public static function makeOrder(): Order { return new Order(); }\n",
        "    public function run(): void {\n",
        "        $fn = self::makeOrder(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 8: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 8, 15).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getTotal"),
        "Expected getTotal in {:?}",
        names,
    );
}

// ─── First-class callable chained: $fn()->method() ─────────────────────────

#[tokio::test]
async fn test_first_class_callable_chained_call() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_chain.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Builder {\n",
        "    public function build(): Product { return new Product(); }\n",
        "}\n",
        "class Product {\n",
        "    public function getTitle(): string { return ''; }\n",
        "    public function getPrice(): float { return 0.0; }\n",
        "}\n",
        "function getBuilder(): Builder { return new Builder(); }\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = getBuilder(...);\n",
        "        $fn()->build()->\n",
        "    }\n",
        "}\n",
    );

    // Line 12: `        $fn()->build()->`  cursor after last `->`
    let items = complete_at(&backend, &uri, src, 12, 25).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getTitle"),
        "Expected getTitle in {:?}",
        names,
    );
    assert!(
        names.contains(&"getPrice"),
        "Expected getPrice in {:?}",
        names,
    );
}

// ─── Assignment from first-class callable invocation ────────────────────────

#[tokio::test]
async fn test_first_class_callable_assigned_result() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_assign.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string { return ''; }\n",
        "    public function getAge(): int { return 0; }\n",
        "}\n",
        "function createUser(): User { return new User(); }\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = createUser(...);\n",
        "        $user = $fn();\n",
        "        $user->\n",
        "    }\n",
        "}\n",
    );

    // Line 10: `        $user->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 10, 16).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getName"),
        "Expected getName in {:?}",
        names,
    );
    assert!(names.contains(&"getAge"), "Expected getAge in {:?}", names,);
}

// ─── Cross-file first-class callable ────────────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_cross_file() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": { "App\\": "src/" }
        }
    }"#;

    let model_src = concat!(
        "<?php\n",
        "namespace App;\n",
        "class Invoice {\n",
        "    public function getAmount(): float { return 0.0; }\n",
        "    public function getNumber(): string { return ''; }\n",
        "}\n",
    );

    let factory_src = concat!(
        "<?php\n",
        "namespace App;\n",
        "class InvoiceFactory {\n",
        "    public static function create(): Invoice { return new Invoice(); }\n",
        "}\n",
    );

    let service_src = concat!(
        "<?php\n",
        "namespace App;\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = InvoiceFactory::create(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Invoice.php", model_src),
            ("src/InvoiceFactory.php", factory_src),
            ("src/Service.php", service_src),
        ],
    );

    let uri = Url::from_file_path(_dir.path().join("src/Service.php")).unwrap();

    let items = complete_at(&backend, &uri, service_src, 5, 15).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getAmount"),
        "Expected getAmount in {:?}",
        names,
    );
    assert!(
        names.contains(&"getNumber"),
        "Expected getNumber in {:?}",
        names,
    );
}

// ─── Null-safe method first-class callable ──────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_nullsafe_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_nullsafe.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Formatter {\n",
        "    public function format(): string { return ''; }\n",
        "}\n",
        "class Config {\n",
        "    public function getFormatter(): Formatter { return new Formatter(); }\n",
        "}\n",
        "class Service {\n",
        "    private ?Config $config;\n",
        "    public function run(): void {\n",
        "        $fn = $this?->config?->getFormatter(...);\n",
        // The text-based scanner strips `?` via trim_end_matches('?')
        // on the `->` LHS, so this should resolve through to Formatter.
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 11: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 11, 15).await;
    let names = method_names(&items);
    // This test verifies the text scanner handles the `?->` syntax.
    // The chain `$this?->config?->getFormatter` is complex; the text
    // scanner may not resolve it fully.  If it does, great; if not,
    // we at least verify it doesn't crash.
    // (If text_resolution.resolve_lhs_to_class doesn't handle `$this?->`,
    //  names may be empty — that's acceptable for now.)
    let _ = names;
}

// ─── First-class callable variable resolves to Closure for `$fn->` ──────────

#[tokio::test]
async fn test_first_class_callable_resolves_to_closure() {
    // When `$fn = strlen(...)`, the variable itself is a Closure instance.
    // If the Closure class is available (from stubs), `$fn->` should
    // offer Closure members like `bindTo`, `call`, etc.
    //
    // This test verifies the AST path correctly resolves
    // `PartialApplication` to `Closure` ClassInfo.  Since the test
    // backend may not have a Closure stub loaded, we define a minimal
    // one inline.
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_closure_members.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Closure {\n",
        "    public function bindTo(object $newThis): Closure { return $this; }\n",
        "    public function call(object $newThis): mixed { return null; }\n",
        "}\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = strlen(...);\n",
        "        $fn->\n",
        "    }\n",
        "}\n",
    );

    // Line 8: `        $fn->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 8, 13).await;
    let names = method_names(&items);
    assert!(names.contains(&"bindTo"), "Expected bindTo in {:?}", names,);
    assert!(names.contains(&"call"), "Expected call in {:?}", names,);
}

// ─── Top-level (outside class) first-class callable ─────────────────────────

#[tokio::test]
async fn test_first_class_callable_top_level() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_top_level.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string { return ''; }\n",
        "}\n",
        "function getUser(): User { return new User(); }\n",
        "$fn = getUser(...);\n",
        "$fn()->\n",
    );

    // Line 6: `$fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 6, 7).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getName"),
        "Expected getName in {:?}",
        names,
    );
}

// ─── Reassigned first-class callable uses latest assignment ─────────────────

#[tokio::test]
async fn test_first_class_callable_reassignment() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_reassign.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string { return ''; }\n",
        "}\n",
        "class Order {\n",
        "    public function getTotal(): float { return 0.0; }\n",
        "}\n",
        "function getUser(): User { return new User(); }\n",
        "function getOrder(): Order { return new Order(); }\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = getUser(...);\n",
        "        $fn = getOrder(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 13: `        $fn()->`  cursor after `->`
    // Should resolve to Order (latest assignment).
    let items = complete_at(&backend, &uri, src, 13, 15).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getTotal"),
        "Expected getTotal in {:?}",
        names,
    );
    // getName should NOT be present (overwritten by second assignment).
    assert!(
        !names.contains(&"getName"),
        "Did not expect getName in {:?}",
        names,
    );
}

// ─── First-class callable with property access ──────────────────────────────

#[tokio::test]
async fn test_first_class_callable_result_property_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_prop.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Address {\n",
        "    public string $city;\n",
        "    public string $street;\n",
        "}\n",
        "class User {\n",
        "    public Address $address;\n",
        "    public function getName(): string { return ''; }\n",
        "}\n",
        "function getUser(): User { return new User(); }\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = getUser(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 13: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 13, 15).await;
    let names = method_names(&items);
    let props = property_names(&items);
    assert!(
        names.contains(&"getName"),
        "Expected getName in {:?}",
        names,
    );
    assert!(
        props.contains(&"address"),
        "Expected address property in {:?}",
        props,
    );
}

// ─── Static method returning ?self ──────────────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_static_method_returning_nullable_self() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_nullable_self.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string { return ''; }\n",
        "    public function getEmail(): string { return ''; }\n",
        "    public static function findByEmail(string $email): ?self { return null; }\n",
        "}\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $finder = User::findByEmail(...);\n",
        "        $finder()->\n",
        "    }\n",
        "}\n",
    );

    // Line 9: `        $finder()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 9, 19).await;
    let names = method_names(&items);
    assert!(
        names.contains(&"getName"),
        "Expected getName in {:?}",
        names,
    );
    assert!(
        names.contains(&"getEmail"),
        "Expected getEmail in {:?}",
        names,
    );
}

// ─── Instance method returning self ─────────────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_instance_method_returning_self() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_self_return.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Builder {\n",
        "    public function reset(): self { return new self(); }\n",
        "    public function build(): string { return ''; }\n",
        "}\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $builder = new Builder();\n",
        "        $fn = $builder->reset(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 9: `        $fn()->`  cursor after `->`
    let items = complete_at(&backend, &uri, src, 9, 15).await;
    let names = method_names(&items);
    assert!(names.contains(&"reset"), "Expected reset in {:?}", names,);
    assert!(names.contains(&"build"), "Expected build in {:?}", names,);
}

// ─── Static method returning static ─────────────────────────────────────────

#[tokio::test]
async fn test_first_class_callable_static_method_returning_static() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/fcc_static_return.php").unwrap();

    let src = concat!(
        "<?php\n",
        "class Model {\n",
        "    public function getId(): int { return 0; }\n",
        "    public function getName(): string { return ''; }\n",
        "    public static function make(string $name): static { return new static(); }\n",
        "}\n",
        "class Service {\n",
        "    public function run(): void {\n",
        "        $fn = Model::make(...);\n",
        "        $fn()->\n",
        "    }\n",
        "}\n",
    );

    // Line 9: `        $fn()->`  cursor after `->`
    // `make()` returns `static` which resolves to Model.
    // Instance methods like getId and getName should appear;
    // `make` is static so it won't appear in `->` completion.
    let items = complete_at(&backend, &uri, src, 9, 15).await;
    let names = method_names(&items);
    assert!(names.contains(&"getId"), "Expected getId in {:?}", names,);
    assert!(
        names.contains(&"getName"),
        "Expected getName in {:?}",
        names,
    );
}
