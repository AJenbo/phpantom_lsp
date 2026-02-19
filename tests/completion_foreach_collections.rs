mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

async fn complete_at(
    backend: &phpantom_lsp::Backend,
    uri: &Url,
    text: &str,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
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

// ─── Foreach over collection class assigned via `new` ───────────────────────

/// When a variable is assigned via `new PaymentOptionLocaleCollection()` and
/// the class has `@extends Collection<int, PaymentOptionLocale>`, iterating
/// with foreach should resolve the value variable to `PaymentOptionLocale`.
#[tokio::test]
async fn test_foreach_collection_new_with_extends_generics() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_collection_new.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class PaymentOptionLocale {\n",
        "    public string $locale;\n",
        "    public function getLabel(): string {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, PaymentOptionLocale>\n",
        " */\n",
        "final class PaymentOptionLocaleCollection extends Collection {}\n",
        "class Service {\n",
        "    public function process() {\n",
        "        $items = new PaymentOptionLocaleCollection();\n",
        "        foreach ($items as $item) {\n",
        "            $item->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 18, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("locale")),
        "Should include 'locale' from PaymentOptionLocale via collection foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getLabel")),
        "Should include 'getLabel' from PaymentOptionLocale via collection foreach. Got: {:?}",
        labels
    );
}

// ─── Foreach over collection returned by method ─────────────────────────────

/// When a method's return type is a collection class (not a generic type
/// string), foreach should still resolve the value variable.
#[tokio::test]
async fn test_foreach_collection_from_method_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_collection_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, User>\n",
        " */\n",
        "class UserCollection extends Collection {}\n",
        "class UserRepository {\n",
        "    public function getUsers(): UserCollection { return new UserCollection(); }\n",
        "    public function process() {\n",
        "        foreach ($this->getUsers() as $user) {\n",
        "            $user->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 18, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("name")),
        "Should include 'name' from User via method-returned collection foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getEmail")),
        "Should include 'getEmail' from User via method-returned collection foreach. Got: {:?}",
        labels
    );
}

// ─── Foreach over collection with implements_generics ───────────────────────

/// When a class directly `@implements IteratorAggregate<int, Order>`, foreach
/// should resolve the value variable to `Order`.
#[tokio::test]
async fn test_foreach_collection_with_implements_generics() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_implements_generic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Order {\n",
        "    public int $id;\n",
        "    public function getTotal(): float {}\n",
        "}\n",
        "/**\n",
        " * @implements IteratorAggregate<int, Order>\n",
        " */\n",
        "class OrderList implements IteratorAggregate {\n",
        "    public function getIterator(): ArrayIterator { return new ArrayIterator([]); }\n",
        "}\n",
        "class Service {\n",
        "    public function process() {\n",
        "        $orders = new OrderList();\n",
        "        foreach ($orders as $order) {\n",
        "            $order->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 15, 21).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("id")),
        "Should include 'id' from Order via @implements IteratorAggregate foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getTotal")),
        "Should include 'getTotal' from Order via @implements IteratorAggregate foreach. Got: {:?}",
        labels
    );
}

// ─── Multi-level inheritance chain (Laravel-style) ──────────────────────────

/// Simulates the Laravel Eloquent collection hierarchy:
///   PaymentOptionLocaleCollection
///     @extends EloquentCollection<int, PaymentOptionLocale>
///   EloquentCollection<TKey, TModel>
///     @extends BaseCollection<TKey, TModel>
///   BaseCollection<TKey, TValue>
///     @implements ArrayAccess<TKey, TValue>
#[tokio::test]
async fn test_foreach_laravel_style_collection_chain() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_laravel_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class PaymentOptionLocale {\n",
        "    public string $locale;\n",
        "    public function getName(): string {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template-covariant TValue\n",
        " * @implements ArrayAccess<TKey, TValue>\n",
        " */\n",
        "class BaseCollection implements ArrayAccess {\n",
        "    public function offsetExists(mixed $offset): bool {}\n",
        "    public function offsetGet(mixed $offset): mixed {}\n",
        "    public function offsetSet(mixed $offset, mixed $value): void {}\n",
        "    public function offsetUnset(mixed $offset): void {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TModel\n",
        " * @extends BaseCollection<TKey, TModel>\n",
        " */\n",
        "class EloquentCollection extends BaseCollection {}\n",
        "/**\n",
        " * @extends EloquentCollection<int, PaymentOptionLocale>\n",
        " */\n",
        "final class PaymentOptionLocaleCollection extends EloquentCollection {}\n",
        "class Service {\n",
        "    public function process() {\n",
        "        $items = new PaymentOptionLocaleCollection();\n",
        "        foreach ($items as $item) {\n",
        "            $item->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 30, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("locale")),
        "Should include 'locale' from PaymentOptionLocale via Laravel-style chain. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getName")),
        "Should include 'getName' from PaymentOptionLocale via Laravel-style chain. Got: {:?}",
        labels
    );
}

// ─── Foreach over collection stored in a property ───────────────────────────

/// When a property's type hint is a collection class, iterating over it
/// should resolve the value variable.
#[tokio::test]
async fn test_foreach_collection_from_property() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_collection_prop.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Product {\n",
        "    public string $sku;\n",
        "    public function getPrice(): float {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, Product>\n",
        " */\n",
        "class ProductCollection extends Collection {}\n",
        "class Cart {\n",
        "    public ProductCollection $products;\n",
        "    public function listProducts() {\n",
        "        foreach ($this->products as $product) {\n",
        "            $product->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 18, 23).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("sku")),
        "Should include 'sku' from Product via property collection foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getPrice")),
        "Should include 'getPrice' from Product via property collection foreach. Got: {:?}",
        labels
    );
}

// ─── Cross-file foreach over collection ─────────────────────────────────────

/// Foreach over a collection class defined in a different file via PSR-4.
#[tokio::test]
async fn test_foreach_collection_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[
            (
                "src/Models/Language.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Models;\n",
                    "class Language {\n",
                    "    public string $code;\n",
                    "    public function getDisplayName(): string {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Collections/LanguageCollection.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Collections;\n",
                    "use App\\Models\\Language;\n",
                    "/**\n",
                    " * @template TKey of array-key\n",
                    " * @template TValue\n",
                    " */\n",
                    "class Collection {}\n",
                    "/**\n",
                    " * @extends Collection<int, Language>\n",
                    " */\n",
                    "final class LanguageCollection extends Collection {}\n",
                ),
            ),
            (
                "src/Services/LanguageService.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Services;\n",
                    "use App\\Collections\\LanguageCollection;\n",
                    "class LanguageService {\n",
                    "    public function process() {\n",
                    "        $langs = new LanguageCollection();\n",
                    "        foreach ($langs as $lang) {\n",
                    "            $lang->\n",
                    "        }\n",
                    "    }\n",
                    "}\n",
                ),
            ),
        ],
    );

    let service_path = _dir.path().join("src/Services/LanguageService.php");
    let uri = Url::from_file_path(&service_path).unwrap();
    let text = std::fs::read_to_string(&service_path).unwrap();

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for foreach over cross-file collection"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("code")),
                "Should include 'code' from Language via cross-file collection foreach. Got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("getDisplayName")),
                "Should include 'getDisplayName' from Language via cross-file collection foreach. Got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Top-level foreach over collection ──────────────────────────────────────

/// Foreach over a collection in top-level (non-class) code should still work.
#[tokio::test]
async fn test_foreach_collection_top_level() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_collection_top.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getAddress(): string {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, Customer>\n",
        " */\n",
        "class CustomerCollection extends Collection {}\n",
        "$customers = new CustomerCollection();\n",
        "foreach ($customers as $customer) {\n",
        "    $customer->\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 16, 15).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("name")),
        "Should include 'name' from Customer in top-level foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getAddress")),
        "Should include 'getAddress' from Customer in top-level foreach. Got: {:?}",
        labels
    );
}

// ─── Foreach over collection via variable assigned from method ──────────────

/// When a variable is assigned from a method that returns a collection
/// class name (not a generic type), foreach should resolve the value.
#[tokio::test]
async fn test_foreach_collection_variable_from_method_call() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_collection_var_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Invoice {\n",
        "    public int $number;\n",
        "    public function send(): void {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, Invoice>\n",
        " */\n",
        "class InvoiceCollection extends Collection {}\n",
        "class InvoiceService {\n",
        "    public function getInvoices(): InvoiceCollection { return new InvoiceCollection(); }\n",
        "    public function process() {\n",
        "        $invoices = $this->getInvoices();\n",
        "        foreach ($invoices as $invoice) {\n",
        "            $invoice->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 19, 22).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("number")),
        "Should include 'number' from Invoice via variable-from-method collection foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("send")),
        "Should include 'send' from Invoice via variable-from-method collection foreach. Got: {:?}",
        labels
    );
}

// ─── Single generic param (e.g. @extends AbstractList<User>) ───────────────

/// When a collection class has a single generic type parameter in its
/// @extends annotation, it should be treated as the value type.
#[tokio::test]
async fn test_foreach_collection_single_generic_param() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_single_param.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Task {\n",
        "    public string $title;\n",
        "    public function execute(): void {}\n",
        "}\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class TypedList {}\n",
        "/**\n",
        " * @extends TypedList<Task>\n",
        " */\n",
        "class TaskList extends TypedList {}\n",
        "class Runner {\n",
        "    public function run() {\n",
        "        $tasks = new TaskList();\n",
        "        foreach ($tasks as $task) {\n",
        "            $task->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 17, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("title")),
        "Should include 'title' from Task via single-param generic extends. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("execute")),
        "Should include 'execute' from Task via single-param generic extends. Got: {:?}",
        labels
    );
}

// ─── Existing docblock @var annotation still takes precedence ───────────────

/// When a `@var` annotation is present, it should be used instead of the
/// class's generic annotations (existing behaviour preserved).
#[tokio::test]
async fn test_foreach_var_annotation_still_works_with_collection() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_var_precedence.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Account {\n",
        "    public int $balance;\n",
        "    public function deposit(int $amount): void {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, Account>\n",
        " */\n",
        "class AccountCollection extends Collection {}\n",
        "class Service {\n",
        "    public function process() {\n",
        "        /** @var list<Account> $items */\n",
        "        $items = $this->getAccounts();\n",
        "        foreach ($items as $item) {\n",
        "            $item->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 19, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("balance")),
        "Should include 'balance' from Account via @var annotation. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("deposit")),
        "Should include 'deposit' from Account via @var annotation. Got: {:?}",
        labels
    );
}

// ─── Foreach over collection with inherited members ─────────────────────────

/// The value class should include members from its own parent chain.
#[tokio::test]
async fn test_foreach_collection_element_has_inherited_members() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_element_inherited.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class BaseModel {\n",
        "    public int $id;\n",
        "    public function save(): void {}\n",
        "}\n",
        "class Article extends BaseModel {\n",
        "    public string $title;\n",
        "    public function publish(): void {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, Article>\n",
        " */\n",
        "class ArticleCollection extends Collection {}\n",
        "class Editor {\n",
        "    public function edit() {\n",
        "        $articles = new ArticleCollection();\n",
        "        foreach ($articles as $article) {\n",
        "            $article->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 22, 22).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("title")),
        "Should include own 'title' from Article. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("publish")),
        "Should include own 'publish' from Article. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("id")),
        "Should include inherited 'id' from BaseModel. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("save")),
        "Should include inherited 'save' from BaseModel. Got: {:?}",
        labels
    );
}

// ─── Foreach over collection from static method ─────────────────────────────

/// When a static method returns a collection class, foreach should resolve.
#[tokio::test]
async fn test_foreach_collection_from_static_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///foreach_static_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Tag {\n",
        "    public string $label;\n",
        "    public function getSlug(): string {}\n",
        "}\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {}\n",
        "/**\n",
        " * @extends Collection<int, Tag>\n",
        " */\n",
        "class TagCollection extends Collection {}\n",
        "class TagFactory {\n",
        "    public static function all(): TagCollection { return new TagCollection(); }\n",
        "}\n",
        "class Controller {\n",
        "    public function index() {\n",
        "        foreach (TagFactory::all() as $tag) {\n",
        "            $tag->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 20, 18).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("label")),
        "Should include 'label' from Tag via static method returning collection. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getSlug")),
        "Should include 'getSlug' from Tag via static method returning collection. Got: {:?}",
        labels
    );
}
