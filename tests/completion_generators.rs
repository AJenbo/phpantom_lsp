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

// ─── Foreach over Generator with @var annotation ────────────────────────────

/// When a variable is annotated as `Generator<int, User>` and iterated with
/// foreach, the value variable should resolve to `User`.
#[tokio::test]
async fn test_foreach_generator_var_annotation_two_params() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_var_two.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "class Service {\n",
        "    public function process() {\n",
        "        /** @var \\Generator<int, User> $gen */\n",
        "        $gen = $this->getUsers();\n",
        "        foreach ($gen as $user) {\n",
        "            $user->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 10, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("name")),
        "Should include 'name' from User via Generator<int, User> foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getEmail")),
        "Should include 'getEmail' from User via Generator<int, User> foreach. Got: {:?}",
        labels
    );
}

/// When `Generator` has a single type parameter, it should be treated as the
/// value type (the yield type).
#[tokio::test]
async fn test_foreach_generator_var_annotation_single_param() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_var_single.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Product {\n",
        "    public string $title;\n",
        "    public function getPrice(): float {}\n",
        "}\n",
        "class Loader {\n",
        "    public function load() {\n",
        "        /** @var \\Generator<Product> $gen */\n",
        "        $gen = $this->loadProducts();\n",
        "        foreach ($gen as $product) {\n",
        "            $product->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 10, 22).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("title")),
        "Should include 'title' from Product via Generator<Product> foreach. Got: {:?}",
        labels
    );
}

/// When `Generator` has all four type parameters, the value (yield) type is
/// still the second parameter.
#[tokio::test]
async fn test_foreach_generator_var_annotation_four_params() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_var_four.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Order {\n",
        "    public int $id;\n",
        "    public function getTotal(): float {}\n",
        "}\n",
        "class Response {\n",
        "    public int $statusCode;\n",
        "}\n",
        "class Pipeline {\n",
        "    public function run() {\n",
        "        /** @var \\Generator<int, Order, mixed, Response> $gen */\n",
        "        $gen = $this->process();\n",
        "        foreach ($gen as $order) {\n",
        "            $order->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 13, 20).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("id")),
        "Should include 'id' from Order (2nd param of Generator<int, Order, mixed, Response>). Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getTotal")),
        "Should include 'getTotal' from Order (2nd param). Got: {:?}",
        labels
    );
    // Make sure we're NOT getting Response members (4th param / TReturn).
    assert!(
        !labels.iter().any(|l| l.starts_with("statusCode")),
        "Should NOT include 'statusCode' from Response (4th param / TReturn). Got: {:?}",
        labels
    );
}

// ─── Foreach over Generator from method return type ─────────────────────────

/// When a method's `@return` specifies `Generator<int, User>`, iterating
/// the method call result should resolve the value variable to `User`.
#[tokio::test]
async fn test_foreach_generator_method_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_method_ret.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getAddress(): string {}\n",
        "}\n",
        "class CustomerRepository {\n",
        "    /** @return \\Generator<int, Customer> */\n",
        "    public function findAll(): \\Generator {}\n",
        "    public function process() {\n",
        "        foreach ($this->findAll() as $customer) {\n",
        "            $customer->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 10, 24).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("name")),
        "Should include 'name' from Customer via method returning Generator<int, Customer>. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getAddress")),
        "Should include 'getAddress' from Customer. Got: {:?}",
        labels
    );
}

/// Method returning `Generator` with four type parameters — value is still
/// the 2nd parameter.
#[tokio::test]
async fn test_foreach_generator_method_return_four_params() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_method_four.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Task {\n",
        "    public string $title;\n",
        "    public function run(): void {}\n",
        "}\n",
        "class Result {\n",
        "    public bool $success;\n",
        "}\n",
        "class TaskRunner {\n",
        "    /** @return \\Generator<int, Task, mixed, Result> */\n",
        "    public function tasks(): \\Generator {}\n",
        "    public function execute() {\n",
        "        foreach ($this->tasks() as $task) {\n",
        "            $task->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 13, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("title")),
        "Should include 'title' from Task (2nd param of 4-param Generator). Got: {:?}",
        labels
    );
    assert!(
        !labels.iter().any(|l| l.starts_with("success")),
        "Should NOT include 'success' from Result (TReturn, 4th param). Got: {:?}",
        labels
    );
}

// ─── Foreach over Generator from standalone function return type ─────────────

/// When a standalone function's `@return` specifies `Generator<int, User>`,
/// iterating the function call result should resolve the value variable.
#[tokio::test]
async fn test_foreach_generator_function_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_func_ret.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Invoice {\n",
        "    public int $number;\n",
        "    public function send(): void {}\n",
        "}\n",
        "/** @return \\Generator<int, Invoice> */\n",
        "function generateInvoices(): \\Generator {}\n",
        "foreach (generateInvoices() as $invoice) {\n",
        "    $invoice->\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 8, 14).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("number")),
        "Should include 'number' from Invoice via function returning Generator<int, Invoice>. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("send")),
        "Should include 'send' from Invoice. Got: {:?}",
        labels
    );
}

// ─── Foreach over iterable<T> ───────────────────────────────────────────────

/// `iterable<User>` should resolve the foreach value variable to `User`.
#[tokio::test]
async fn test_foreach_iterable_single_param_var_annotation() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///iter_var_single.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "class Handler {\n",
        "    public function handle() {\n",
        "        /** @var iterable<User> $items */\n",
        "        $items = $this->getItems();\n",
        "        foreach ($items as $item) {\n",
        "            $item->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 10, 19).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("name")),
        "Should include 'name' from User via iterable<User> foreach. Got: {:?}",
        labels
    );
}

/// `iterable<int, User>` should resolve the foreach value variable to `User`.
#[tokio::test]
async fn test_foreach_iterable_two_params_var_annotation() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///iter_var_two.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Order {\n",
        "    public int $id;\n",
        "    public function getTotal(): float {}\n",
        "}\n",
        "class Service {\n",
        "    public function process() {\n",
        "        /** @var iterable<int, Order> $orders */\n",
        "        $orders = $this->loadOrders();\n",
        "        foreach ($orders as $order) {\n",
        "            $order->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 10, 20).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("id")),
        "Should include 'id' from Order via iterable<int, Order> foreach. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getTotal")),
        "Should include 'getTotal' from Order. Got: {:?}",
        labels
    );
}

/// When a method returns `iterable<User>`, iterating should resolve value type.
#[tokio::test]
async fn test_foreach_iterable_method_return_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///iter_method_ret.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Product {\n",
        "    public string $title;\n",
        "    public function getPrice(): float {}\n",
        "}\n",
        "class Catalog {\n",
        "    /** @return iterable<Product> */\n",
        "    public function products(): iterable {}\n",
        "    public function display() {\n",
        "        foreach ($this->products() as $product) {\n",
        "            $product->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 10, 22).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("title")),
        "Should include 'title' from Product via method returning iterable<Product>. Got: {:?}",
        labels
    );
}

// ─── Foreach over Generator with @param annotation ──────────────────────────

/// When a method parameter is annotated with `@param Generator<int, User>`,
/// iterating it should resolve the value variable.
#[tokio::test]
async fn test_foreach_generator_param_annotation() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_param.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Ticket {\n",
        "    public string $subject;\n",
        "    public function close(): void {}\n",
        "}\n",
        "class Processor {\n",
        "    /**\n",
        "     * @param \\Generator<int, Ticket> $tickets\n",
        "     */\n",
        "    public function process($tickets) {\n",
        "        foreach ($tickets as $ticket) {\n",
        "            $ticket->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 11, 21).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("subject")),
        "Should include 'subject' from Ticket via @param Generator<int, Ticket>. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("close")),
        "Should include 'close' from Ticket. Got: {:?}",
        labels
    );
}

// ─── Generator with nested generic value types ──────────────────────────────

/// `Generator<int, Collection<string, Order>>` — the value type should be
/// `Collection<string, Order>`, which resolves to Collection and offers its
/// members.
#[tokio::test]
async fn test_foreach_generator_nested_generic_value() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_nested.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Order {\n",
        "    public int $id;\n",
        "}\n",
        "class Collection {\n",
        "    public function first(): mixed {}\n",
        "    public function count(): int {}\n",
        "}\n",
        "class Batcher {\n",
        "    public function batches() {\n",
        "        /** @var \\Generator<int, Collection<string, Order>> $gen */\n",
        "        $gen = $this->getBatches();\n",
        "        foreach ($gen as $batch) {\n",
        "            $batch->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 13, 20).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("first")),
        "Should include 'first' from Collection via Generator<int, Collection<…>>. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("count")),
        "Should include 'count' from Collection. Got: {:?}",
        labels
    );
}

// ─── Cross-file Generator resolution ────────────────────────────────────────

/// Generator yield type should resolve across files via PSR-4.
#[tokio::test]
async fn test_foreach_generator_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[(
            "src/Models/Article.php",
            concat!(
                "<?php\n",
                "namespace App\\Models;\n",
                "class Article {\n",
                "    public string $title;\n",
                "    public function getAuthor(): string {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///gen_cross.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Models\\Article;\n",
        "/** @var \\Generator<int, Article> $articles */\n",
        "$articles = loadArticles();\n",
        "foreach ($articles as $article) {\n",
        "    $article->\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 5, 14).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("title")),
        "Should include 'title' from Article via cross-file Generator. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("getAuthor")),
        "Should include 'getAuthor' from Article. Got: {:?}",
        labels
    );
}

// ─── Property chain on Generator foreach value ──────────────────────────────

/// Property chains on a foreach value from a Generator should work.
#[tokio::test]
async fn test_foreach_generator_property_chain() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///gen_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Address {\n",
        "    public string $city;\n",
        "    public string $street;\n",
        "}\n",
        "class Employee {\n",
        "    public Address $address;\n",
        "}\n",
        "class Report {\n",
        "    public function generate() {\n",
        "        /** @var \\Generator<int, Employee> $employees */\n",
        "        $employees = $this->loadEmployees();\n",
        "        foreach ($employees as $emp) {\n",
        "            $emp->address->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 13, 27).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("city")),
        "Should include 'city' from Address via Generator foreach property chain. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("street")),
        "Should include 'street' from Address. Got: {:?}",
        labels
    );
}

// ─── Iterable method return type with @param on method parameter ────────────

/// When a method parameter is typed as `iterable` with a docblock override,
/// the foreach value variable should resolve.
#[tokio::test]
async fn test_foreach_iterable_param_annotation() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///iter_param.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Event {\n",
        "    public string $name;\n",
        "    public function fire(): void {}\n",
        "}\n",
        "class Dispatcher {\n",
        "    /**\n",
        "     * @param iterable<Event> $events\n",
        "     */\n",
        "    public function dispatch(iterable $events) {\n",
        "        foreach ($events as $event) {\n",
        "            $event->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 11, 20).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.iter().any(|l| l.starts_with("name")),
        "Should include 'name' from Event via @param iterable<Event>. Got: {:?}",
        labels
    );
    assert!(
        labels.iter().any(|l| l.starts_with("fire")),
        "Should include 'fire' from Event. Got: {:?}",
        labels
    );
}
