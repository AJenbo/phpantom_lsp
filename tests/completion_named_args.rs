mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

/// Helper: open a file and request completion at the given line/character.
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
        _ => vec![],
    }
}

/// Collect the filter_text values from completion items.
fn filter_texts(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter_map(|i| i.filter_text.as_deref())
        .collect()
}

// ─── Basic: method on same-file class ───────────────────────────────────────

/// Named arg completion for `$this->method(|)` inside the same class.
#[tokio::test]
async fn test_named_args_this_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_this.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Greeter {\n",
        "    public function greet(string $name, int $age): string {\n",
        "        return '';\n",
        "    }\n",
        "    public function test() {\n",
        "        $this->greet(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 6, 22).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"name"),
        "Should suggest 'name' param. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"age"),
        "Should suggest 'age' param. Got: {:?}",
        tags
    );
}

/// Named arg items should have VARIABLE kind.
#[tokio::test]
async fn test_named_args_have_variable_kind() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_kind.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function run(int $count): void {}\n",
        "    public function test() {\n",
        "        $this->run(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 20).await;
    for item in &items {
        assert_eq!(
            item.kind,
            Some(CompletionItemKind::VARIABLE),
            "Named arg '{:?}' should use VARIABLE kind",
            item.label
        );
    }
}

/// The insert text should be `name: ` (with colon and trailing space).
#[tokio::test]
async fn test_named_args_insert_text_format() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_insert.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function run(int $count): void {}\n",
        "    public function test() {\n",
        "        $this->run(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 20).await;
    let count_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("count"));
    assert!(count_item.is_some(), "Should have 'count' completion");
    assert_eq!(
        count_item.unwrap().insert_text.as_deref(),
        Some("count: "),
        "Insert text should be 'name: '"
    );
}

/// The label should show the type when available.
#[tokio::test]
async fn test_named_args_label_with_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_label.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function create(string $name, ?int $priority): void {}\n",
        "    public function test() {\n",
        "        $this->create(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 23).await;

    let name_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("name"));
    assert!(name_item.is_some(), "Should have 'name' completion");
    assert_eq!(name_item.unwrap().label, "name: string");

    let priority_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("priority"));
    assert!(priority_item.is_some(), "Should have 'priority' completion");
    assert_eq!(priority_item.unwrap().label, "priority: ?int");
}

/// Label without type hint should be `name:` (no space before colon).
#[tokio::test]
async fn test_named_args_label_without_type() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_notype.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function process($data): void {}\n",
        "    public function test() {\n",
        "        $this->process(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 24).await;
    let data_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("data"));
    assert!(data_item.is_some(), "Should have 'data' completion");
    assert_eq!(data_item.unwrap().label, "data:");
}

// ─── Skipping already-used named args ───────────────────────────────────────

/// Already-specified named args should not appear in suggestions.
#[tokio::test]
async fn test_named_args_skip_already_used() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_skip.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function greet(string $name, int $age, bool $formal): string {\n",
        "        return '';\n",
        "    }\n",
        "    public function test() {\n",
        "        $this->greet(name: 'Alice', \n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 6, 36).await;
    let tags = filter_texts(&items);

    assert!(
        !tags.contains(&"name"),
        "Should NOT suggest 'name' (already used). Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"age"),
        "Should suggest 'age'. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"formal"),
        "Should suggest 'formal'. Got: {:?}",
        tags
    );
}

// ─── Positional arguments ───────────────────────────────────────────────────

/// Positional arguments before the cursor should exclude leading params.
#[tokio::test]
async fn test_named_args_skip_positional() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_pos.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function setup(string $host, int $port, bool $ssl): void {}\n",
        "    public function test() {\n",
        "        $this->setup('localhost', \n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 34).await;
    let tags = filter_texts(&items);

    assert!(
        !tags.contains(&"host"),
        "Should NOT suggest 'host' (covered by positional). Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"port"),
        "Should suggest 'port'. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"ssl"),
        "Should suggest 'ssl'. Got: {:?}",
        tags
    );
}

// ─── Prefix filtering ──────────────────────────────────────────────────────

/// Typing a partial prefix should filter the suggestions.
#[tokio::test]
async fn test_named_args_prefix_filter() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_prefix.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function config(string $name, int $notify, bool $narrow): void {}\n",
        "    public function test() {\n",
        "        $this->config(na\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 24).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"name"),
        "Should suggest 'name' (matches 'na'). Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"narrow"),
        "Should suggest 'narrow' (matches 'na'). Got: {:?}",
        tags
    );
    assert!(
        !tags.contains(&"notify"),
        "Should NOT suggest 'notify' (doesn't match 'na'). Got: {:?}",
        tags
    );
}

// ─── Constructor ────────────────────────────────────────────────────────────

/// Named args should work for `new ClassName(|)`.
#[tokio::test]
async fn test_named_args_constructor() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_ctor.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class UserDTO {\n",
        "    public function __construct(\n",
        "        public string $name,\n",
        "        public int $age,\n",
        "        public string $email = '',\n",
        "    ) {}\n",
        "}\n",
        "$dto = new UserDTO(\n",
    );

    let items = complete_at(&backend, &uri, text, 8, 19).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"name"),
        "Should suggest 'name'. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"age"),
        "Should suggest 'age'. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"email"),
        "Should suggest 'email'. Got: {:?}",
        tags
    );
}

/// Constructor with some args already filled.
#[tokio::test]
async fn test_named_args_constructor_partial() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_ctor2.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Point {\n",
        "    public function __construct(public int $x, public int $y, public int $z = 0) {}\n",
        "}\n",
        "$p = new Point(x: 1, \n",
    );

    let items = complete_at(&backend, &uri, text, 4, 21).await;
    let tags = filter_texts(&items);

    assert!(
        !tags.contains(&"x"),
        "Should NOT suggest 'x' (already used). Got: {:?}",
        tags
    );
    assert!(tags.contains(&"y"), "Should suggest 'y'. Got: {:?}", tags);
    assert!(tags.contains(&"z"), "Should suggest 'z'. Got: {:?}", tags);
}

// ─── Static method ──────────────────────────────────────────────────────────

/// Named args should work for `ClassName::method(|)`.
#[tokio::test]
async fn test_named_args_static_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Factory {\n",
        "    public static function create(string $type, array $options = []): self {\n",
        "        return new self();\n",
        "    }\n",
        "}\n",
        "Factory::create(\n",
    );

    let items = complete_at(&backend, &uri, text, 6, 17).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"type"),
        "Should suggest 'type'. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"options"),
        "Should suggest 'options'. Got: {:?}",
        tags
    );
}

// ─── self:: / static:: / parent:: ──────────────────────────────────────────

/// Named args for `self::method(|)`.
#[tokio::test]
async fn test_named_args_self_static_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_self.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Builder {\n",
        "    public static function make(string $name, int $count = 1): self {\n",
        "        return new self();\n",
        "    }\n",
        "    public function test() {\n",
        "        self::make(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 6, 19).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"name"),
        "Should suggest 'name' via self::. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"count"),
        "Should suggest 'count' via self::. Got: {:?}",
        tags
    );
}

/// Named args for `parent::__construct(|)`.
#[tokio::test]
async fn test_named_args_parent_construct() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_parent.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function __construct(public string $name, public int $id = 0) {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    public function __construct() {\n",
        "        parent::__construct(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 6, 28).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"name"),
        "Should suggest 'name' via parent::. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"id"),
        "Should suggest 'id' via parent::. Got: {:?}",
        tags
    );
}

// ─── Variable method call ───────────────────────────────────────────────────

/// Named args for `$var->method(|)` where $var is typed.
#[tokio::test]
async fn test_named_args_variable_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function log(string $message, int $level = 0): void {}\n",
        "}\n",
        "class App {\n",
        "    public function run() {\n",
        "        $logger = new Logger();\n",
        "        $logger->log(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 7, 22).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"message"),
        "Should suggest 'message'. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"level"),
        "Should suggest 'level'. Got: {:?}",
        tags
    );
}

// ─── Not triggered in wrong contexts ────────────────────────────────────────

/// Should NOT trigger named args when typing a variable inside call parens.
#[tokio::test]
async fn test_named_args_not_triggered_for_variables() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_novar.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function greet(string $name): void {}\n",
        "    public function test() {\n",
        "        $val = 'hello';\n",
        "        $this->greet($va\n",
        "    }\n",
        "}\n",
    );

    // Cursor is after `$va` — variable completion should handle, not named args
    let items = complete_at(&backend, &uri, text, 5, 24).await;

    // Named arg items use VARIABLE kind with `name:` format.
    // Variable completions also use VARIABLE kind but start with `$`.
    let named_arg_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.insert_text
                .as_deref()
                .map_or(false, |t| t.ends_with(": "))
        })
        .collect();

    assert!(
        named_arg_items.is_empty(),
        "Should NOT suggest named args when typing a variable. Got: {:?}",
        named_arg_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

/// Should NOT trigger named args after `->` inside call parens.
#[tokio::test]
async fn test_named_args_not_triggered_after_arrow() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_noarrow.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public string $value = '';\n",
        "    public function greet(string $name): void {}\n",
        "    public function test() {\n",
        "        $this->greet($this->\n",
        "    }\n",
        "}\n",
    );

    // Cursor after `$this->` — member completion should handle this
    let items = complete_at(&backend, &uri, text, 5, 28).await;

    let named_arg_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.insert_text
                .as_deref()
                .map_or(false, |t| t.ends_with(": "))
        })
        .collect();

    assert!(
        named_arg_items.is_empty(),
        "Should NOT suggest named args after '->'. Got: {:?}",
        named_arg_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

// ─── Multiline calls ────────────────────────────────────────────────────────

/// Named args should work across multiple lines.
#[tokio::test]
async fn test_named_args_multiline() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_multi.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    public function set(string $key, string $value, bool $overwrite = true): void {}\n",
        "    public function test() {\n",
        "        $this->set(\n",
        "            key: 'app.name',\n",
        "            \n",
        "        );\n",
        "    }\n",
        "}\n",
    );

    // Cursor on the empty line (line 6, after indentation)
    let items = complete_at(&backend, &uri, text, 6, 12).await;
    let tags = filter_texts(&items);

    assert!(
        !tags.contains(&"key"),
        "Should NOT suggest 'key' (already used). Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"value"),
        "Should suggest 'value'. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"overwrite"),
        "Should suggest 'overwrite'. Got: {:?}",
        tags
    );
}

// ─── Detail text ────────────────────────────────────────────────────────────

/// Optional parameters should show "(optional)" in detail.
#[tokio::test]
async fn test_named_args_detail_optional() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_detail.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function config(string $host, int $port = 80): void {}\n",
        "    public function test() {\n",
        "        $this->config(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 23).await;

    let host_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("host"));
    assert!(host_item.is_some(), "Should have 'host' completion");
    assert_eq!(host_item.unwrap().detail.as_deref(), Some("Named argument"),);

    let port_item = items
        .iter()
        .find(|i| i.filter_text.as_deref() == Some("port"));
    assert!(port_item.is_some(), "Should have 'port' completion");
    assert_eq!(
        port_item.unwrap().detail.as_deref(),
        Some("Named argument (optional)"),
    );
}

// ─── No params function ─────────────────────────────────────────────────────

/// Functions with no parameters should produce no named arg completions.
#[tokio::test]
async fn test_named_args_no_params() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_noparams.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function noop(): void {}\n",
        "    public function test() {\n",
        "        $this->noop(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 20).await;

    let named_arg_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.insert_text
                .as_deref()
                .map_or(false, |t| t.ends_with(": "))
        })
        .collect();

    assert!(
        named_arg_items.is_empty(),
        "Should not suggest named args for parameterless method. Got: {:?}",
        named_arg_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

// ─── All params already used ────────────────────────────────────────────────

/// When all params are already specified, no named arg completions.
#[tokio::test]
async fn test_named_args_all_used() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_allused.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Svc {\n",
        "    public function pair(int $x, int $y): void {}\n",
        "    public function test() {\n",
        "        $this->pair(x: 1, y: 2, \n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 33).await;

    let named_arg_items: Vec<_> = items
        .iter()
        .filter(|i| {
            i.insert_text
                .as_deref()
                .map_or(false, |t| t.ends_with(": "))
        })
        .collect();

    assert!(
        named_arg_items.is_empty(),
        "Should not suggest named args when all are used. Got: {:?}",
        named_arg_items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

// ─── Inherited method ───────────────────────────────────────────────────────

/// Named args should work for inherited methods.
#[tokio::test]
async fn test_named_args_inherited_method() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///na_inherit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function save(string $filename, bool $overwrite = false): void {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    public function test() {\n",
        "        $this->save(\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 6, 20).await;
    let tags = filter_texts(&items);

    assert!(
        tags.contains(&"filename"),
        "Should suggest 'filename' from inherited method. Got: {:?}",
        tags
    );
    assert!(
        tags.contains(&"overwrite"),
        "Should suggest 'overwrite' from inherited method. Got: {:?}",
        tags
    );
}
