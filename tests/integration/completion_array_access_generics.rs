use crate::common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

/// Helper: open a document and request completion at the given line/character.
async fn complete_at(
    backend: &phpantom_lsp::Backend,
    uri: &Url,
    text: &str,
    line: u32,
    character: u32,
) -> Option<CompletionResponse> {
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

    backend.completion(completion_params).await.unwrap()
}

fn assert_has_member(items: &[CompletionItem], member: &str) {
    let names: Vec<&str> = items
        .iter()
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();
    assert!(
        names.contains(&member),
        "Should suggest '{}', got: {:?}",
        member,
        names
    );
}

fn unwrap_items(response: Option<CompletionResponse>) -> Vec<CompletionItem> {
    match response.expect("Should return completion results") {
        CompletionResponse::Array(items) => items,
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Case 1a: /** @var array<int, Customer> $thing */ $thing = []; $thing[0]-> ──

#[tokio::test]
async fn test_var_array_int_customer_named_annotation() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_arr_named.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var array<int, Customer> $thing */\n",
        "$thing = [];\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 7, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 1b: /** @var array<int, Customer> */ $thing = []; $thing[0]-> ─────
// No variable name in the annotation — applies to the next assignment line.

#[tokio::test]
async fn test_var_array_int_customer_no_varname_annotation() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_arr_no_varname.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var array<int, Customer> */\n",
        "$thing = [];\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 7, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 1c: /** @var array<int, Customer> */ $thing = []; $thing[0]-> ──────

#[tokio::test]
async fn test_var_array_int_customer_empty_array_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_arr_int_cust.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var array<int, Customer> $thing */\n",
        "$thing = getUnknownValue();\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 7, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 2: /** @var array<Customer> */ $thing = []; $thing[0]-> ───────────

#[tokio::test]
async fn test_var_array_single_param_customer_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_arr_single_cust.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var array<Customer> $thing */\n",
        "$thing = [];\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 7, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 3a: /** @var list<Customer> $thing */ $thing = []; $thing[0]-> ────

#[tokio::test]
async fn test_var_list_customer_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_list_cust.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var list<Customer> $thing */\n",
        "$thing = [];\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 7, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 3b: /** @var list<Customer> */ $thing = []; $thing[0]-> ───────────
// No variable name in the annotation.

#[tokio::test]
async fn test_var_list_customer_no_varname_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_list_cust_novar.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var list<Customer> */\n",
        "$thing = [];\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 7, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 4: $thing = [new Customer()]; $thing[0]-> ────────────────────────

#[tokio::test]
async fn test_inferred_array_new_object_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_inferred_arr.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "$thing = [new Customer()];\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 6, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 5: [Customer::first()][0]-> ──────────────────────────────────────

#[tokio::test]
async fn test_inline_array_literal_static_call_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_inline_arr.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "    /** @return static */\n",
        "    public static function first(): static {}\n",
        "}\n",
        "[Customer::first()][0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 7, 24).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Case 6: end(Customer::get()->all())-> ─────────────────────────────────

#[tokio::test]
async fn test_end_of_method_chain_returning_array() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_end_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "    /** @return Collection<int, static> */\n",
        "    public static function get(): Collection {}\n",
        "}\n",
        "class Collection {\n",
        "    /** @return array<int, Customer> */\n",
        "    public function all(): array {}\n",
        "}\n",
        "end(Customer::get()->all())->\n",
    );

    let result = complete_at(&backend, &uri, text, 11, 29).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Extra: variable assigned from end() ────────────────────────────────────

#[tokio::test]
async fn test_variable_assigned_from_end_array_generic() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_end_assign.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var array<int, Customer> $customers */\n",
        "$customers = [];\n",
        "$last = end($customers);\n",
        "$last->\n",
    );

    let result = complete_at(&backend, &uri, text, 8, 7).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ─── Extra: @var without explicit assignment to getUnknownValue() ───────────
// This pattern is known to work — serves as a sanity/regression check.

#[tokio::test]
async fn test_var_array_generic_with_unknown_value_rhs() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_arr_unknown_rhs.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Customer {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "function getUnknownValue(): mixed { return null; }\n",
        "/** @var array<int, Customer> $thing */\n",
        "$thing = getUnknownValue();\n",
        "$thing[0]->\n",
    );

    let result = complete_at(&backend, &uri, text, 8, 11).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "name");
    assert_has_member(&items, "getEmail");
}

// ═══════════════════════════════════════════════════════════════════════════
// Method return → array access: $c->items()[0]->
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_method_return_array_access_bracket_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_method_arr.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Item {\n",
        "    public function getLabel(): string { return ''; }\n",
        "}\n",
        "class Collection {\n",
        "    /** @return Item[] */\n",
        "    public function items(): array { return []; }\n",
        "}\n",
        "class Consumer {\n",
        "    public function run(): void {\n",
        "        $c = new Collection();\n",
        "        $c->items()[0]->\n",
        "    }\n",
        "}\n",
    );

    let result = complete_at(&backend, &uri, text, 11, 24).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "getLabel");
}

#[tokio::test]
async fn test_method_return_array_access_generic_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_method_arr_generic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Item {\n",
        "    public function getLabel(): string { return ''; }\n",
        "}\n",
        "class Collection {\n",
        "    /** @return array<int, Item> */\n",
        "    public function items(): array { return []; }\n",
        "}\n",
        "class Consumer {\n",
        "    public function run(): void {\n",
        "        $c = new Collection();\n",
        "        $c->items()[0]->\n",
        "    }\n",
        "}\n",
    );

    let result = complete_at(&backend, &uri, text, 11, 24).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "getLabel");
}

#[tokio::test]
async fn test_static_method_return_array_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_static_method_arr.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Item {\n",
        "    public function getLabel(): string { return ''; }\n",
        "}\n",
        "class Collection {\n",
        "    /** @return Item[] */\n",
        "    public static function all(): array { return []; }\n",
        "}\n",
        "class Consumer {\n",
        "    public function run(): void {\n",
        "        Collection::all()[0]->\n",
        "    }\n",
        "}\n",
    );

    let result = complete_at(&backend, &uri, text, 10, 30).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "getLabel");
}

#[tokio::test]
async fn test_method_return_list_array_access() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///test_method_list_arr.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Item {\n",
        "    public function getLabel(): string { return ''; }\n",
        "}\n",
        "class Collection {\n",
        "    /** @return list<Item> */\n",
        "    public function items(): array { return []; }\n",
        "}\n",
        "class Consumer {\n",
        "    public function run(): void {\n",
        "        $c = new Collection();\n",
        "        $c->items()[0]->\n",
        "    }\n",
        "}\n",
    );

    let result = complete_at(&backend, &uri, text, 11, 24).await;
    let items = unwrap_items(result);
    assert_has_member(&items, "getLabel");
}
