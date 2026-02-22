mod common;

use common::{
    create_psr4_workspace, create_test_backend_with_function_stubs, create_test_backend_with_stubs,
};
use phpantom_lsp::Backend;
use phpantom_lsp::composer::parse_autoload_classmap;
use std::collections::HashMap;
use std::fs;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Helper ─────────────────────────────────────────────────────────────────

/// Open a file in the backend and request completion at the given position.
async fn complete_at(
    backend: &Backend,
    uri: &Url,
    text: &str,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
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
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        None => vec![],
    }
}

/// Filter completion items to only those with kind == CLASS.
fn class_items(items: &[CompletionItem]) -> Vec<&CompletionItem> {
    items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::CLASS))
        .collect()
}

/// Extract labels from a list of completion items.
fn labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|i| i.label.as_str()).collect()
}

// ─── extract_partial_class_name tests ───────────────────────────────────────

#[test]
fn test_extract_partial_class_name_simple() {
    let content = "<?php\nnew Dat\n";
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 7,
        },
    );
    assert_eq!(result, Some("Dat".to_string()));
}

#[test]
fn test_extract_partial_class_name_with_namespace() {
    let content = "<?php\nnew App\\Models\\Us\n";
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 19,
        },
    );
    assert_eq!(result, Some("App\\Models\\Us".to_string()));
}

#[test]
fn test_extract_partial_class_name_variable_returns_none() {
    let content = "<?php\n$var\n";
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 4,
        },
    );
    assert!(
        result.is_none(),
        "Variables ($var) should not trigger class name completion"
    );
}

#[test]
fn test_extract_partial_class_name_empty_returns_none() {
    let content = "<?php\n\n";
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 0,
        },
    );
    assert!(
        result.is_none(),
        "Empty position should not trigger class name completion"
    );
}

#[test]
fn test_extract_partial_class_name_after_arrow_returns_none() {
    let content = "<?php\n$this->meth\n";
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 11,
        },
    );
    assert!(
        result.is_none(),
        "After -> should not trigger class name completion"
    );
}

#[test]
fn test_extract_partial_class_name_after_double_colon_returns_none() {
    let content = "<?php\nFoo::bar\n";
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 8,
        },
    );
    assert!(
        result.is_none(),
        "After :: should not trigger class name completion"
    );
}

#[test]
fn test_extract_partial_class_name_type_hint_context() {
    let content = "<?php\nfunction foo(Str $x) {}\n";
    // Cursor after "Str" at position 16
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 16,
        },
    );
    assert_eq!(result, Some("Str".to_string()));
}

#[test]
fn test_extract_partial_class_name_with_leading_backslash() {
    let content = "<?php\nnew \\Run\n";
    let result = Backend::extract_partial_class_name(
        content,
        Position {
            line: 1,
            character: 8,
        },
    );
    assert_eq!(
        result,
        Some("\\Run".to_string()),
        "Leading backslash should be included in the partial"
    );
}

// ─── Backslash-prefixed completion matching ─────────────────────────────────

/// When the user types `\Unit`, the leading `\` should be stripped for
/// matching so that stub class `UnitEnum` is still found.
#[tokio::test]
async fn test_class_name_completion_with_leading_backslash() {
    let backend = create_test_backend_with_stubs();
    let uri = Url::parse("file:///backslash.php").unwrap();
    // Use a type-hint context (Any) so the interface stub isn't filtered.
    let text = concat!("<?php\n", "function foo(\\Unit $x) {}\n",);

    let items = complete_at(&backend, &uri, text, 1, 17).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"UnitEnum"),
        "Typing '\\Unit' should match 'UnitEnum', got: {:?}",
        class_labels
    );
}

/// When the user types `\Backed`, the leading `\` should be stripped for
/// matching so that stub class `BackedEnum` is still found.
#[tokio::test]
async fn test_class_name_completion_backslash_backed() {
    let backend = create_test_backend_with_stubs();
    let uri = Url::parse("file:///backslash2.php").unwrap();
    // Use a type-hint context (Any) so the interface stub isn't filtered.
    let text = concat!("<?php\n", "function foo(\\Backed $x) {}\n",);

    let items = complete_at(&backend, &uri, text, 1, 19).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"BackedEnum"),
        "Typing '\\Backed' should match 'BackedEnum', got: {:?}",
        class_labels
    );
}

/// FQN prefix like `\App\Models\Us` should still match via the
/// namespace portion — the leading `\` must not break matching.
#[tokio::test]
async fn test_class_name_completion_fqn_prefix() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Models/User.php",
            concat!(
                "<?php\n",
                "namespace App\\Models;\n",
                "class User {\n",
                "    public function getName(): string { return ''; }\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///fqn_test.php").unwrap();
    let text = concat!("<?php\n", "new \\Us\n",);

    // Open the User file so it's in ast_map
    let user_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Models/User.php").display()
    ))
    .unwrap();
    let user_content = std::fs::read_to_string(_dir.path().join("src/Models/User.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: user_uri,
                language_id: "php".to_string(),
                version: 1,
                text: user_content,
            },
        })
        .await;

    let items = complete_at(&backend, &uri, text, 1, 7).await;
    let classes = class_items(&items);

    // With a leading `\`, FQN mode is active: the label for namespaced
    // classes is the full FQN, not the short name.
    let user_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("App\\Models\\User"));
    assert!(
        user_item.is_some(),
        "Typing '\\Us' should match App\\Models\\User, got details: {:?}",
        classes
            .iter()
            .map(|i| i.detail.as_deref())
            .collect::<Vec<_>>()
    );
}

// ─── Stub class name completion tests ───────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_includes_stubs() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///test.php").unwrap();

    // Use instanceof context so interface stubs pass through.
    // Check UnitEnum is found when typing "Unit"
    let text_unit = concat!("<?php\n", "$x instanceof Unit\n",);
    let items_unit = complete_at(&backend, &uri, text_unit, 1, 18).await;
    let classes_unit = class_items(&items_unit);
    let labels_unit: Vec<&str> = classes_unit.iter().map(|i| i.label.as_str()).collect();

    assert!(
        !classes_unit.is_empty(),
        "Should return class name completions when typing a class name"
    );
    assert!(
        labels_unit.contains(&"UnitEnum"),
        "Should include stub interface 'UnitEnum', got: {:?}",
        labels_unit
    );

    // Check BackedEnum is found when typing "Backed"
    let text_backed = concat!("<?php\n", "$x instanceof Backed\n",);
    let items_backed = complete_at(&backend, &uri, text_backed, 1, 20).await;
    let classes_backed = class_items(&items_backed);
    let labels_backed: Vec<&str> = classes_backed.iter().map(|i| i.label.as_str()).collect();

    assert!(
        labels_backed.contains(&"BackedEnum"),
        "Should include stub interface 'BackedEnum', got: {:?}",
        labels_backed
    );
}

#[tokio::test]
async fn test_class_name_completion_not_triggered_for_variables() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "$unit\n",);

    let items = complete_at(&backend, &uri, text, 1, 5).await;
    let classes = class_items(&items);

    // Should NOT return class completions when typing a variable
    assert!(
        classes.is_empty(),
        "Should not return class name completions after $, got: {:?}",
        labels(&items)
    );
}

// ─── Use-imported class completion tests ────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_includes_use_imports() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Service.php",
            concat!(
                "<?php\n",
                "namespace Acme;\n",
                "class Service {\n",
                "    public function run(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!("<?php\n", "use Acme\\Service;\n", "new Ser\n",);

    let items = complete_at(&backend, &uri, text, 2, 7).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"Service"),
        "Should include use-imported class 'Service', got: {:?}",
        class_labels
    );

    // Check that the detail shows the FQN
    let service_item = classes.iter().find(|i| i.label == "Service").unwrap();
    assert_eq!(
        service_item.detail.as_deref(),
        Some("Acme\\Service"),
        "Detail should show FQN"
    );
}

#[tokio::test]
async fn test_class_name_completion_use_import_has_higher_sort_priority() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Acme\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Widget.php",
            concat!("<?php\n", "namespace Acme;\n", "class Widget {}\n",),
        )],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!("<?php\n", "use Acme\\Widget;\n", "new Wid\n",);

    let items = complete_at(&backend, &uri, text, 2, 7).await;
    let classes = class_items(&items);

    let widget_item = classes.iter().find(|i| i.label == "Widget").unwrap();
    let sort = widget_item.sort_text.as_deref().unwrap_or("");
    assert!(
        sort.starts_with("0_"),
        "Use-imported classes should have sort prefix '0_', got: {:?}",
        sort
    );
}

// ─── Same-namespace class completion tests ──────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_same_namespace() {
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
                "src/UserService.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class UserService {\n",
                    "    public function find(): void {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Controller.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Controller {\n",
                    "    public function index() {\n",
                    "        new User\n",
                    "    }\n",
                    "}\n",
                ),
            ),
        ],
    );

    // Open the UserService file first so it gets into the ast_map
    let service_uri = Url::parse("file:///service.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: service_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class UserService {\n",
                    "    public function find(): void {}\n",
                    "}\n",
                )
                .to_string(),
            },
        })
        .await;

    // Open the Controller file — same namespace "App"
    let uri = Url::parse("file:///controller.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "class Controller {\n",
        "    public function index() {\n",
        "        new User\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 16).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"UserService"),
        "Should include same-namespace class 'UserService', got: {:?}",
        class_labels
    );

    // Same-namespace should have sort prefix "1_"
    let service_item = classes.iter().find(|i| i.label == "UserService").unwrap();
    let sort = service_item.sort_text.as_deref().unwrap_or("");
    assert!(
        sort.starts_with("1_"),
        "Same-namespace classes should have sort prefix '1_', got: {:?}",
        sort
    );
}

// ─── Classmap-based class name completion tests ─────────────────────────────

#[tokio::test]
async fn test_class_name_completion_from_classmap() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"name": "test/project"}"#,
    )
    .expect("failed to write composer.json");

    // Create the classmap
    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).expect("failed to create vendor/composer");
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$vendorDir = dirname(__DIR__);\n",
            "$baseDir = dirname($vendorDir);\n",
            "\n",
            "return array(\n",
            "    'Illuminate\\\\Support\\\\Collection' => $vendorDir . '/laravel/framework/src/Illuminate/Support/Collection.php',\n",
            "    'Illuminate\\\\Database\\\\Eloquent\\\\Model' => $vendorDir . '/laravel/framework/src/Illuminate/Database/Eloquent/Model.php',\n",
            "    'Carbon\\\\Carbon' => $vendorDir . '/nesbot/carbon/src/Carbon/Carbon.php',\n",
            ");\n",
        ),
    )
    .expect("failed to write autoload_classmap.php");

    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), vec![]);

    // Populate classmap
    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    assert_eq!(classmap.len(), 3);
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    let uri = Url::parse("file:///app.php").unwrap();

    // Check Collection matches prefix "Coll"
    let text = concat!("<?php\n", "new Coll\n",);
    let items = complete_at(&backend, &uri, text, 1, 8).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"Collection"),
        "Should include classmap class 'Collection', got: {:?}",
        class_labels
    );

    // Check Model matches prefix "Mo"
    let text_mo = concat!("<?php\n", "new Mo\n",);
    let items_mo = complete_at(&backend, &uri, text_mo, 1, 6).await;
    let classes_mo = class_items(&items_mo);
    let labels_mo: Vec<&str> = classes_mo.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels_mo.contains(&"Model"),
        "Should include classmap class 'Model', got: {:?}",
        labels_mo
    );

    // Check Carbon matches prefix "Car"
    let text_car = concat!("<?php\n", "new Car\n",);
    let items_car = complete_at(&backend, &uri, text_car, 1, 7).await;
    let classes_car = class_items(&items_car);
    let labels_car: Vec<&str> = classes_car.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels_car.contains(&"Carbon"),
        "Should include classmap class 'Carbon', got: {:?}",
        labels_car
    );

    // Check that detail shows the FQN
    let collection = classes
        .iter()
        .find(|i| {
            i.label == "Collection"
                && i.detail.as_deref() == Some("Illuminate\\Support\\Collection")
        })
        .expect("Should have a Collection item with FQN Illuminate\\Support\\Collection in detail");
    assert_eq!(
        collection.detail.as_deref(),
        Some("Illuminate\\Support\\Collection"),
        "Detail should show FQN for classmap entries"
    );
}

// ─── class_index-based class name completion tests ──────────────────────────

#[tokio::test]
async fn test_class_name_completion_from_class_index() {
    let backend = create_test_backend_with_stubs();

    // Manually populate the class_index with a discovered class
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "App\\Models\\User".to_string(),
            "file:///app/Models/User.php".to_string(),
        );
        idx.insert(
            "App\\Models\\Order".to_string(),
            "file:///app/Models/Order.php".to_string(),
        );
    }

    let uri = Url::parse("file:///test.php").unwrap();

    // Check User matches prefix "Us"
    let text = concat!("<?php\n", "new Us\n",);
    let items = complete_at(&backend, &uri, text, 1, 6).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"User"),
        "Should include class_index class 'User', got: {:?}",
        class_labels
    );

    // Check Order matches prefix "Or"
    let text_or = concat!("<?php\n", "new Or\n",);
    let items_or = complete_at(&backend, &uri, text_or, 1, 6).await;
    let classes_or = class_items(&items_or);
    let labels_or: Vec<&str> = classes_or.iter().map(|i| i.label.as_str()).collect();

    assert!(
        labels_or.contains(&"Order"),
        "Should include class_index class 'Order', got: {:?}",
        labels_or
    );
}

// ─── Deduplication tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_deduplicates_by_fqn() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"name": "test/project"}"#,
    )
    .expect("failed to write composer.json");

    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).expect("failed to create vendor/composer");
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$vendorDir = dirname(__DIR__);\n",
            "$baseDir = dirname($vendorDir);\n",
            "\n",
            "return array(\n",
            "    'Acme\\\\Duplicated' => $vendorDir . '/acme/src/Duplicated.php',\n",
            ");\n",
        ),
    )
    .expect("failed to write autoload_classmap.php");

    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), vec![]);

    // Add to classmap
    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    // Also add to class_index (same FQN)
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Acme\\Duplicated".to_string(),
            "file:///acme/src/Duplicated.php".to_string(),
        );
    }

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "new Dup\n",);

    let items = complete_at(&backend, &uri, text, 1, 7).await;
    let classes = class_items(&items);

    // Count how many times "Duplicated" appears
    let dup_count = classes.iter().filter(|i| i.label == "Duplicated").count();
    assert_eq!(
        dup_count, 1,
        "Should deduplicate classes with the same FQN, got {} occurrences",
        dup_count
    );
}

// ─── Context sensitivity tests ──────────────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_after_new_keyword() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///test.php").unwrap();
    // Use a locally-defined class since `new` context correctly
    // filters out interfaces (BackedEnum, UnitEnum).
    let text = concat!(
        "<?php\n",
        "class BackupService {\n",
        "    function bar() {\n",
        "        $x = new Back\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 21).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"BackupService"),
        "Should offer class names after 'new' keyword, got: {:?}",
        class_labels
    );
    // BackedEnum is an interface — it should be filtered out in new context.
    assert!(
        !class_labels.contains(&"BackedEnum"),
        "Should not offer interface stubs after 'new', got: {:?}",
        class_labels
    );
}

#[tokio::test]
async fn test_class_name_completion_in_type_hint() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "function process(Unit $x) {}\n",);

    // Cursor after "Unit" at character 21
    let items = complete_at(&backend, &uri, text, 1, 21).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"UnitEnum"),
        "Should offer class names in type hint position, got: {:?}",
        class_labels
    );
}

#[tokio::test]
async fn test_class_name_completion_in_extends_clause() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///test.php").unwrap();
    // BackedEnum is an interface, so it belongs in `interface extends`,
    // not `class extends`.
    let text = concat!("<?php\n", "interface MyEnum extends Back\n",);

    let items = complete_at(&backend, &uri, text, 1, 32).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"BackedEnum"),
        "Should offer interface names in interface extends clause, got: {:?}",
        class_labels
    );
}

// ─── No class completion when member access is detected ─────────────────────

#[tokio::test]
async fn test_class_name_completion_not_after_arrow() {
    let backend = create_test_backend_with_stubs();

    // Open a file where `->` triggers member completion
    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function bar(): void {}\n",
        "}\n",
        "$f = new Foo();\n",
        "$f->ba\n",
    );

    let items = complete_at(&backend, &uri, text, 5, 6).await;

    // Should get member completion items, NOT class name items
    let classes = class_items(&items);
    assert!(
        classes.is_empty(),
        "Should NOT return class name completions after ->, got: {:?}",
        labels(&items)
    );
}

// ─── All items have CLASS kind ──────────────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_items_have_class_kind() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///test.php").unwrap();
    // Use instanceof context so the interface stub passes the filter.
    let text = concat!("<?php\n", "$x instanceof Uni\n",);

    let items = complete_at(&backend, &uri, text, 1, 17).await;
    let classes = class_items(&items);

    assert!(
        !classes.is_empty(),
        "Should have at least one class completion"
    );

    for item in &classes {
        assert_eq!(
            item.kind,
            Some(CompletionItemKind::CLASS),
            "All class name completions should have kind=CLASS, item '{}' has kind={:?}",
            item.label,
            item.kind
        );
    }
}

// ─── Combined sources test ──────────────────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_combines_all_sources() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"name": "test/project"}"#,
    )
    .expect("failed to write composer.json");

    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).expect("failed to create vendor/composer");
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$vendorDir = dirname(__DIR__);\n",
            "$baseDir = dirname($vendorDir);\n",
            "return array(\n",
            "    'Vendor\\\\ClassmapClass' => $vendorDir . '/vendor/src/ClassmapClass.php',\n",
            ");\n",
        ),
    )
    .expect("failed to write autoload_classmap.php");

    let mut stubs: HashMap<&str, &str> = HashMap::new();
    stubs.insert(
        "StubClass",
        "<?php\nclass StubClass {\n    public function stubMethod(): void {}\n}\n",
    );
    let backend = Backend::new_test_with_stubs(stubs);
    *backend.workspace_root().lock().unwrap() = Some(dir.path().to_path_buf());

    // Populate classmap
    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    // Add a class_index entry
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "App\\IndexedClass".to_string(),
            "file:///app/IndexedClass.php".to_string(),
        );
    }

    // Open a file with a use statement — use a prefix that matches
    // classes from all three sources.  All test class names end with
    // "Class", so the prefix "Cl" only matches "ClassmapClass".
    // Instead we use separate checks per source.
    let uri = Url::parse("file:///test.php").unwrap();

    // Check stubs: "Stub" matches "StubClass"
    let text_stub = concat!("<?php\n", "use App\\IndexedClass;\n", "new Stub\n",);
    let items_stub = complete_at(&backend, &uri, text_stub, 2, 8).await;
    let classes_stub = class_items(&items_stub);
    let labels_stub: Vec<&str> = classes_stub.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels_stub.contains(&"StubClass"),
        "Should include stub class, got: {:?}",
        labels_stub
    );

    // Check classmap: "Classmap" matches "ClassmapClass"
    let text_cm = concat!("<?php\n", "use App\\IndexedClass;\n", "new Classmap\n",);
    let items_cm = complete_at(&backend, &uri, text_cm, 2, 12).await;
    let classes_cm = class_items(&items_cm);
    let labels_cm: Vec<&str> = classes_cm.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels_cm.contains(&"ClassmapClass"),
        "Should include classmap class, got: {:?}",
        labels_cm
    );

    // Check use-import / class_index: "Indexed" matches "IndexedClass"
    let text_idx = concat!("<?php\n", "use App\\IndexedClass;\n", "new Indexed\n",);
    let items_idx = complete_at(&backend, &uri, text_idx, 2, 11).await;
    let classes_idx = class_items(&items_idx);
    let labels_idx: Vec<&str> = classes_idx.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels_idx.contains(&"IndexedClass"),
        "Should include use-imported / class_index class, got: {:?}",
        labels_idx
    );
}

// ─── Insert text tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_insert_text_is_short_name() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"name": "test/project"}"#,
    )
    .expect("failed to write composer.json");

    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).expect("failed to create vendor/composer");
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$vendorDir = dirname(__DIR__);\n",
            "$baseDir = dirname($vendorDir);\n",
            "return array(\n",
            "    'Deep\\\\Nested\\\\Namespace\\\\MyClass' => $vendorDir . '/pkg/src/MyClass.php',\n",
            ");\n",
        ),
    )
    .expect("failed to write autoload_classmap.php");

    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), vec![]);
    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "new My\n",);

    let items = complete_at(&backend, &uri, text, 1, 6).await;
    let classes = class_items(&items);

    let my_class = classes.iter().find(|i| i.label == "MyClass").unwrap();
    assert_eq!(
        my_class.insert_text.as_deref(),
        Some("MyClass()$0"),
        "insert_text should be the short class name with parens in `new` context"
    );
    assert_eq!(
        my_class.insert_text_format,
        Some(InsertTextFormat::SNIPPET),
        "insert_text_format should be Snippet in `new` context"
    );
    assert_eq!(
        my_class.detail.as_deref(),
        Some("Deep\\Nested\\Namespace\\MyClass"),
        "detail should show the FQN"
    );
}

// ─── Auto-import (additional_text_edits) tests ──────────────────────────────

/// Selecting a classmap class should add `use FQN;` after existing use statements.
#[tokio::test]
async fn test_auto_import_classmap_class_adds_use_statement() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"name": "test/project"}"#,
    )
    .expect("failed to write composer.json");

    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).expect("failed to create vendor/composer");
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$vendorDir = dirname(__DIR__);\n",
            "return array(\n",
            "    'Illuminate\\\\Support\\\\Collection' => $vendorDir . '/laravel/framework/src/Collection.php',\n",
            ");\n",
        ),
    )
    .expect("failed to write autoload_classmap.php");

    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), vec![]);
    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "use App\\Helpers\\Foo;\n",
        "\n",
        "new Coll\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 8).await;
    let collection = items
        .iter()
        .find(|i| {
            i.label == "Collection"
                && i.detail.as_deref() == Some("Illuminate\\Support\\Collection")
        })
        .expect("Should have Collection completion");

    let edits = collection
        .additional_text_edits
        .as_ref()
        .expect("Classmap class should have additional_text_edits for auto-import");

    assert_eq!(edits.len(), 1);
    assert_eq!(
        edits[0].new_text, "use Illuminate\\Support\\Collection;\n",
        "Should insert a use statement for the FQN"
    );
    // Should insert after the last `use` line (line 2), so at line 3
    assert_eq!(
        edits[0].range.start,
        Position {
            line: 3,
            character: 0,
        },
        "Should insert after the last existing use statement"
    );
}

/// Selecting a class_index class should add `use FQN;` too.
#[tokio::test]
async fn test_auto_import_class_index_adds_use_statement() {
    let backend = create_test_backend_with_stubs();

    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "App\\Services\\PaymentService".to_string(),
            "file:///app/Services/PaymentService.php".to_string(),
        );
    }

    let uri = Url::parse("file:///controller.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App\\Controllers;\n",
        "\n",
        "new Payment\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 11).await;
    let payment = items
        .iter()
        .find(|i| {
            i.label == "PaymentService"
                && i.detail.as_deref() == Some("App\\Services\\PaymentService")
        })
        .expect("Should have PaymentService completion");

    let edits = payment
        .additional_text_edits
        .as_ref()
        .expect("class_index class should have additional_text_edits");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "use App\\Services\\PaymentService;\n",);
    // No existing use statements; should insert after namespace (line 1), so at line 2
    assert_eq!(
        edits[0].range.start,
        Position {
            line: 2,
            character: 0,
        },
    );
}

/// Non-namespaced classes (e.g. `DateTime`) should NOT get auto-import edits.
#[tokio::test]
async fn test_no_auto_import_for_non_namespaced_class() {
    let mut stubs: HashMap<&str, &str> = HashMap::new();
    stubs.insert(
        "DateTime",
        "<?php\nclass DateTime {\n    public function format(string $f): string {}\n}\n",
    );
    let backend = Backend::new_test_with_stubs(stubs);

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "new DateT\n",);

    let items = complete_at(&backend, &uri, text, 1, 9).await;
    let dt = items
        .iter()
        .find(|i| i.label == "DateTime")
        .expect("Should have DateTime completion");

    assert!(
        dt.additional_text_edits.is_none(),
        "Non-namespaced class should not get auto-import edits, got: {:?}",
        dt.additional_text_edits
    );
}

/// Already-imported classes (source 1) should NOT get auto-import edits.
#[tokio::test]
async fn test_no_auto_import_for_already_imported_class() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"name": "test/project"}"#,
    )
    .expect("failed to write composer.json");

    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).expect("failed to create vendor/composer");
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$vendorDir = dirname(__DIR__);\n",
            "return array(\n",
            "    'Illuminate\\\\Support\\\\Collection' => $vendorDir . '/laravel/framework/src/Collection.php',\n",
            ");\n",
        ),
    )
    .expect("failed to write autoload_classmap.php");

    let backend = Backend::new_test_with_workspace(dir.path().to_path_buf(), vec![]);
    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use Illuminate\\Support\\Collection;\n",
        "\n",
        "new Coll\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 8).await;
    // The use-imported version (source 1) should be the first match
    let collection = items
        .iter()
        .find(|i| i.label == "Collection")
        .expect("Should have Collection completion");

    assert!(
        collection.additional_text_edits.is_none(),
        "Already-imported class should not get auto-import edits"
    );
}

/// When there are no use statements and no namespace, insert after `<?php`.
#[tokio::test]
async fn test_auto_import_inserts_after_php_open_tag() {
    let backend = create_test_backend_with_stubs();

    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Vendor\\Lib\\Widget".to_string(),
            "file:///vendor/lib/Widget.php".to_string(),
        );
    }

    let uri = Url::parse("file:///bare.php").unwrap();
    let text = concat!("<?php\n", "\n", "new Wid\n",);

    let items = complete_at(&backend, &uri, text, 2, 7).await;
    let widget = items
        .iter()
        .find(|i| i.label == "Widget" && i.detail.as_deref() == Some("Vendor\\Lib\\Widget"))
        .expect("Should have Widget completion");

    let edits = widget
        .additional_text_edits
        .as_ref()
        .expect("Should have auto-import edit");

    assert_eq!(edits[0].new_text, "use Vendor\\Lib\\Widget;\n");
    // Insert after `<?php` (line 0), so at line 1
    assert_eq!(
        edits[0].range.start,
        Position {
            line: 1,
            character: 0,
        },
    );
}

/// Trait `use` statements inside a class body must NOT be mistaken for
/// namespace `use` imports.  The auto-import should insert after the
/// top-level `use` statements, not after `use HasSlug;` etc.
#[tokio::test]
async fn test_auto_import_not_confused_by_trait_use_in_class_body() {
    let backend = create_test_backend_with_stubs();

    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Cassandra\\DefaultCluster".to_string(),
            "file:///vendor/cassandra/DefaultCluster.php".to_string(),
        );
    }

    let uri = Url::parse("file:///showcase.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // line 0
        "\n",                                               // line 1
        "namespace Demo;\n",                                // line 2
        "\n",                                               // line 3
        "use Exception;\n",                                 // line 4
        "use Stringable;\n",                                // line 5
        "\n",                                               // line 6
        "class User extends Model implements Renderable\n", // line 7
        "{\n",                                              // line 8
        "    use HasTimestamps;\n",                         // line 9
        "    use HasSlug;\n",                               // line 10
        "\n",                                               // line 11
        "    function test() {\n",                          // line 12
        "        new Default\n",                            // line 13
        "    }\n",                                          // line 14
        "}\n",                                              // line 15
    );

    let items = complete_at(&backend, &uri, text, 13, 19).await;
    let cluster = items
        .iter()
        .find(|i| {
            i.label == "DefaultCluster" && i.detail.as_deref() == Some("Cassandra\\DefaultCluster")
        })
        .expect("Should have DefaultCluster completion");

    let edits = cluster
        .additional_text_edits
        .as_ref()
        .expect("Should have auto-import edit");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "use Cassandra\\DefaultCluster;\n",);
    // Should insert after the last top-level `use` (line 5: `use Stringable;`),
    // NOT after `use HasSlug;` (line 10) which is a trait import inside the class.
    assert_eq!(
        edits[0].range.start,
        Position {
            line: 6,
            character: 0,
        },
        "Auto-import should go after top-level use statements, not after trait use in class body"
    );
}

/// Global classes (no namespace separator in FQN, e.g. `PDO`) should get a
/// `use PDO;` import when the current file declares a namespace.
#[tokio::test]
async fn test_auto_import_global_class_when_file_has_namespace() {
    let mut stubs: HashMap<&str, &str> = HashMap::new();
    stubs.insert(
        "PDO",
        "<?php\nclass PDO {\n    public function query(string $q): mixed {}\n}\n",
    );
    let backend = Backend::new_test_with_stubs(stubs);

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",              // line 0
        "\n",                   // line 1
        "namespace App\\Db;\n", // line 2
        "\n",                   // line 3
        "new PD\n",             // line 4
    );

    let items = complete_at(&backend, &uri, text, 4, 6).await;
    let pdo = items
        .iter()
        .find(|i| i.label == "PDO")
        .expect("Should have PDO completion");

    let edits = pdo
        .additional_text_edits
        .as_ref()
        .expect("Global class should get auto-import when file has a namespace");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "use PDO;\n");
    // Insert after `namespace App\Db;` (line 2), so at line 3
    assert_eq!(
        edits[0].range.start,
        Position {
            line: 3,
            character: 0,
        },
    );
}

/// Global classes should NOT get an auto-import when the file has no namespace.
/// (This complements `test_no_auto_import_for_non_namespaced_class`.)
#[tokio::test]
async fn test_no_auto_import_global_class_when_file_has_no_namespace() {
    let mut stubs: HashMap<&str, &str> = HashMap::new();
    stubs.insert(
        "PDO",
        "<?php\nclass PDO {\n    public function query(string $q): mixed {}\n}\n",
    );
    let backend = Backend::new_test_with_stubs(stubs);

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "new PD\n",);

    let items = complete_at(&backend, &uri, text, 1, 6).await;
    let pdo = items
        .iter()
        .find(|i| i.label == "PDO")
        .expect("Should have PDO completion");

    assert!(
        pdo.additional_text_edits.is_none(),
        "Global class should NOT get auto-import when file has no namespace, got: {:?}",
        pdo.additional_text_edits
    );
}

/// When a file has a namespace and existing use statements, the global class
/// import should be inserted after the last use statement.
#[tokio::test]
async fn test_auto_import_global_class_inserts_after_existing_use_statements() {
    let mut stubs: HashMap<&str, &str> = HashMap::new();
    stubs.insert(
        "PDO",
        "<?php\nclass PDO {\n    public function query(string $q): mixed {}\n}\n",
    );
    let backend = Backend::new_test_with_stubs(stubs);

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",                                // line 0
        "\n",                                     // line 1
        "namespace App\\Service;\n",              // line 2
        "\n",                                     // line 3
        "use App\\Repository\\UserRepository;\n", // line 4
        "use App\\Entity\\User;\n",               // line 5
        "\n",                                     // line 6
        "new PD\n",                               // line 7
    );

    let items = complete_at(&backend, &uri, text, 7, 6).await;
    let pdo = items
        .iter()
        .find(|i| i.label == "PDO")
        .expect("Should have PDO completion");

    let edits = pdo
        .additional_text_edits
        .as_ref()
        .expect("Global class should get auto-import when file has a namespace");

    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].new_text, "use PDO;\n");
    // Insert after last use statement (line 5), so at line 6
    assert_eq!(
        edits[0].range.start,
        Position {
            line: 6,
            character: 0,
        },
    );
}

// ─── `new` context filtering tests ─────────────────────────────────────────

/// After `new`, completion should NOT include constants or functions.
#[tokio::test]
async fn test_new_context_excludes_constants_and_functions() {
    let backend = create_test_backend_with_function_stubs();
    let uri = Url::parse("file:///test_new_no_const_func.php").unwrap();
    let text = concat!("<?php\n", "new Date\n",);

    let items = complete_at(&backend, &uri, text, 1, 8).await;

    // Should find DateTime (a class stub).
    let has_class = items
        .iter()
        .any(|i| i.kind == Some(CompletionItemKind::CLASS));
    assert!(has_class, "Should include class completions after `new`");

    // Should NOT find any constants.
    let constants: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
        .map(|i| i.label.as_str())
        .collect();
    assert!(
        constants.is_empty(),
        "Should not include constants after `new`, got: {:?}",
        constants
    );

    // Should NOT find any functions.
    let functions: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::FUNCTION))
        .map(|i| i.label.as_str())
        .collect();
    assert!(
        functions.is_empty(),
        "Should not include functions after `new`, got: {:?}",
        functions
    );
}

/// Without `new`, completion SHOULD include constants and functions.
#[tokio::test]
async fn test_non_new_context_includes_constants_and_functions() {
    let backend = create_test_backend_with_function_stubs();
    let uri = Url::parse("file:///test_no_new.php").unwrap();
    // Bare identifier context — no `new` keyword.
    let text = concat!("<?php\n", "PHP_\n",);

    let items = complete_at(&backend, &uri, text, 1, 4).await;

    let constants: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
        .map(|i| i.label.as_str())
        .collect();
    assert!(
        !constants.is_empty(),
        "Should include constants without `new`, got: {:?}",
        labels(&items)
    );
}

/// After `new`, loaded abstract classes should be excluded.
#[tokio::test]
async fn test_new_context_excludes_loaded_abstract_class() {
    let backend = create_test_backend_with_stubs();
    let uri = Url::parse("file:///test_new_abstract.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "abstract class AbstractWidget {}\n",
        "class ConcreteWidget extends AbstractWidget {}\n",
        "new Wid\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 7).await;
    let class_labels: Vec<&str> = class_items(&items)
        .iter()
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        class_labels.contains(&"ConcreteWidget"),
        "Should include concrete class, got: {:?}",
        class_labels
    );
    assert!(
        !class_labels.contains(&"AbstractWidget"),
        "Should exclude loaded abstract class, got: {:?}",
        class_labels
    );
}

/// After `new`, loaded interfaces should be excluded.
#[tokio::test]
async fn test_new_context_excludes_loaded_interface() {
    let backend = create_test_backend_with_stubs();
    let uri = Url::parse("file:///test_new_iface.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "interface Renderable {}\n",
        "class HtmlRenderer implements Renderable {}\n",
        "new Render\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 10).await;
    let class_labels: Vec<&str> = class_items(&items)
        .iter()
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        class_labels.contains(&"HtmlRenderer"),
        "Should include concrete class, got: {:?}",
        class_labels
    );
    assert!(
        !class_labels.contains(&"Renderable"),
        "Should exclude loaded interface, got: {:?}",
        class_labels
    );
}

/// After `new`, loaded traits should be excluded.
#[tokio::test]
async fn test_new_context_excludes_loaded_trait() {
    let backend = create_test_backend_with_stubs();
    let uri = Url::parse("file:///test_new_trait.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "trait Loggable {}\n",
        "class Logger { use Loggable; }\n",
        "new Logg\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 8).await;
    let class_labels: Vec<&str> = class_items(&items)
        .iter()
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        class_labels.contains(&"Logger"),
        "Should include concrete class, got: {:?}",
        class_labels
    );
    assert!(
        !class_labels.contains(&"Loggable"),
        "Should exclude loaded trait, got: {:?}",
        class_labels
    );
}

/// After `new`, loaded enums should be excluded (enums cannot be instantiated).
#[tokio::test]
async fn test_new_context_excludes_loaded_enum() {
    let backend = create_test_backend_with_stubs();
    let uri = Url::parse("file:///test_new_enum.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace App;\n",
        "enum ColorEnum { case Red; case Blue; }\n",
        "class ColorPicker {}\n",
        "new Color\n",
    );

    let items = complete_at(&backend, &uri, text, 4, 9).await;
    let class_labels: Vec<&str> = class_items(&items)
        .iter()
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        class_labels.contains(&"ColorPicker"),
        "Should include concrete class, got: {:?}",
        class_labels
    );
    assert!(
        !class_labels.contains(&"ColorEnum"),
        "Should exclude loaded enum, got: {:?}",
        class_labels
    );
}

/// After `new`, unloaded classmap entries whose name matches non-instantiable
/// naming conventions should sort below normal names:
/// - ends/starts with "Abstract"
/// - ends with "Interface"
/// - starts with `I[A-Z]` (C#-style interface prefix)
/// - starts/ends with case-sensitive "Base"
#[tokio::test]
async fn test_new_context_demotes_likely_non_instantiable_classmap() {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    fs::write(
        dir.path().join("composer.json"),
        r#"{"name": "test/project"}"#,
    )
    .expect("failed to write composer.json");

    let composer_dir = dir.path().join("vendor").join("composer");
    fs::create_dir_all(&composer_dir).expect("failed to create vendor/composer");
    fs::write(
        composer_dir.join("autoload_classmap.php"),
        concat!(
            "<?php\n",
            "$vendorDir = dirname(__DIR__);\n",
            "$baseDir = dirname($vendorDir);\n",
            "return array(\n",
            // Concrete — should NOT be demoted
            "    'Vendor\\\\ConcreteHandler' => $vendorDir . '/src/ConcreteHandler.php',\n",
            "    'Vendor\\\\ImageHandler' => $vendorDir . '/src/ImageHandler.php',\n",
            "    'Vendor\\\\DatabaseHandler' => $vendorDir . '/src/DatabaseHandler.php',\n",
            "    'Vendor\\\\BaselineHandler' => $vendorDir . '/src/BaselineHandler.php',\n",
            // Abstract prefix/suffix — should be demoted
            "    'Vendor\\\\AbstractHandler' => $vendorDir . '/src/AbstractHandler.php',\n",
            "    'Vendor\\\\HandlerAbstract' => $vendorDir . '/src/HandlerAbstract.php',\n",
            // Interface suffix — should be demoted
            "    'Vendor\\\\HandlerInterface' => $vendorDir . '/src/HandlerInterface.php',\n",
            // I[A-Z] prefix — should be demoted
            "    'Vendor\\\\IHandler' => $vendorDir . '/src/IHandler.php',\n",
            // Base[A-Z] prefix — should be demoted
            "    'Vendor\\\\BaseHandler' => $vendorDir . '/src/BaseHandler.php',\n",
            ");\n",
        ),
    )
    .expect("failed to write autoload_classmap.php");

    let backend = Backend::new_test();
    *backend.workspace_root().lock().unwrap() = Some(dir.path().to_path_buf());

    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    if let Ok(mut cm) = backend.classmap().lock() {
        *cm = classmap;
    }

    let uri = Url::parse("file:///test_new_demote.php").unwrap();
    let text = concat!("<?php\n", "new Handler\n",);

    let items = complete_at(&backend, &uri, text, 1, 11).await;
    let classes = class_items(&items);

    let concrete = classes
        .iter()
        .find(|i| i.label == "ConcreteHandler")
        .expect("Should find ConcreteHandler");

    // These should all be demoted below ConcreteHandler.
    let demoted_names = [
        "AbstractHandler",
        "HandlerAbstract",
        "HandlerInterface",
        "IHandler",
        "BaseHandler",
    ];
    for name in &demoted_names {
        let item = classes
            .iter()
            .find(|i| i.label == *name)
            .unwrap_or_else(|| panic!("Should find {} (unloaded, included but demoted)", name));
        assert!(
            concrete.sort_text < item.sort_text,
            "ConcreteHandler ({:?}) should sort before {} ({:?})",
            concrete.sort_text,
            name,
            item.sort_text
        );
    }

    // ImageHandler starts with "I" but second char is lowercase — NOT demoted.
    let image = classes
        .iter()
        .find(|i| i.label == "ImageHandler")
        .expect("Should find ImageHandler");
    assert_eq!(
        concrete.sort_text.as_deref().map(|s| &s[..2]),
        image.sort_text.as_deref().map(|s| &s[..2]),
        "ImageHandler should have the same sort prefix as ConcreteHandler (not demoted)"
    );

    // DatabaseHandler contains "base" but not case-sensitive "Base" — NOT demoted.
    let database = classes
        .iter()
        .find(|i| i.label == "DatabaseHandler")
        .expect("Should find DatabaseHandler");
    assert_eq!(
        concrete.sort_text.as_deref().map(|s| &s[..2]),
        database.sort_text.as_deref().map(|s| &s[..2]),
        "DatabaseHandler should have the same sort prefix as ConcreteHandler (not demoted)"
    );

    // BaselineHandler starts with "Base" but 5th char is lowercase — NOT demoted.
    let baseline = classes
        .iter()
        .find(|i| i.label == "BaselineHandler")
        .expect("Should find BaselineHandler");
    assert_eq!(
        concrete.sort_text.as_deref().map(|s| &s[..2]),
        baseline.sort_text.as_deref().map(|s| &s[..2]),
        "BaselineHandler should have the same sort prefix as ConcreteHandler (not demoted)"
    );
}

/// After `new`, unloaded stub entries whose name starts with "Abstract"
/// should sort below normal stub names.
#[tokio::test]
async fn test_new_context_excludes_abstract_stubs() {
    let mut stubs: HashMap<&str, &str> = HashMap::new();
    stubs.insert("ConcreteService", "<?php\nclass ConcreteService {}\n");
    stubs.insert(
        "AbstractService",
        "<?php\nabstract class AbstractService {}\n",
    );
    let backend = Backend::new_test_with_stubs(stubs);

    let uri = Url::parse("file:///test_new_demote_stubs.php").unwrap();
    let text = concat!("<?php\n", "new Service\n",);

    let items = complete_at(&backend, &uri, text, 1, 11).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"ConcreteService"),
        "Should find ConcreteService in new context, got: {:?}",
        class_labels
    );
    // The lightweight source scanner detects `abstract class` and
    // excludes it from new context.
    assert!(
        !class_labels.contains(&"AbstractService"),
        "Abstract stub should be excluded from new context, got: {:?}",
        class_labels
    );
}

/// After `new`, use-imported classes that are loaded as interfaces should
/// be excluded.
#[tokio::test]
async fn test_new_context_excludes_use_imported_interface() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{"autoload": {"psr-4": {"App\\": "src/"}}}"#,
        &[
            (
                "src/Contracts/Cacheable.php",
                "<?php\nnamespace App\\Contracts;\ninterface Cacheable {\n    public function cacheKey(): string;\n}\n",
            ),
            (
                "src/Models/CacheStore.php",
                "<?php\nnamespace App\\Models;\nclass CacheStore {}\n",
            ),
        ],
    );

    // Open both files so they are loaded into the ast_map.
    let iface_uri = Url::parse("file:///iface.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: iface_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: "<?php\nnamespace App\\Contracts;\ninterface Cacheable {\n    public function cacheKey(): string;\n}\n".to_string(),
            },
        })
        .await;

    let class_uri = Url::parse("file:///cls.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: class_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: "<?php\nnamespace App\\Models;\nclass CacheStore {}\n".to_string(),
            },
        })
        .await;

    let uri = Url::parse("file:///test_new_use_iface.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Contracts\\Cacheable;\n",
        "use App\\Models\\CacheStore;\n",
        "new Cache\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 9).await;
    let class_labels: Vec<&str> = class_items(&items)
        .iter()
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        class_labels.contains(&"CacheStore"),
        "Should include concrete use-imported class, got: {:?}",
        class_labels
    );
    assert!(
        !class_labels.contains(&"Cacheable"),
        "Should exclude use-imported interface in `new` context, got: {:?}",
        class_labels
    );
}

/// After `new`, class_index entries that are loaded as abstract should be
/// excluded.
#[tokio::test]
async fn test_new_context_excludes_class_index_abstract() {
    let backend = create_test_backend_with_stubs();

    // Load an abstract class into the ast_map.
    let abs_uri = Url::parse("file:///app/AbstractRepo.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: abs_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: "<?php\nnamespace App;\nabstract class AbstractRepo {}\n".to_string(),
            },
        })
        .await;

    // Also put it in the class_index.
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert("App\\AbstractRepo".to_string(), abs_uri.to_string());
    }

    let uri = Url::parse("file:///test_new_idx_abs.php").unwrap();
    let text = concat!("<?php\n", "new AbstractR\n",);

    let items = complete_at(&backend, &uri, text, 1, 13).await;
    let class_labels: Vec<&str> = class_items(&items)
        .iter()
        .map(|i| i.label.as_str())
        .collect();

    assert!(
        !class_labels.contains(&"AbstractRepo"),
        "Should exclude class_index entry that is loaded as abstract, got: {:?}",
        class_labels
    );
}

// ─── FQN-prefix matching tests ─────────────────────────────────────────────

/// Typing `App\Models\U` should match classes whose FQN contains that
/// namespace path, using the FQN as label and insert text.
#[tokio::test]
async fn test_fqn_prefix_matches_by_namespace() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Models/User.php",
            concat!(
                "<?php\n",
                "namespace App\\Models;\n",
                "class User {\n",
                "    public function getName(): string { return ''; }\n",
                "}\n",
            ),
        )],
    );

    // Open the User file so it's in ast_map
    let user_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Models/User.php").display()
    ))
    .unwrap();
    let user_content = std::fs::read_to_string(_dir.path().join("src/Models/User.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: user_uri,
                language_id: "php".to_string(),
                version: 1,
                text: user_content,
            },
        })
        .await;

    let uri = Url::parse("file:///fqn_prefix.php").unwrap();
    // Cursor after `App\Models\U` (12 chars on line 1, 0-indexed col 16)
    let text = concat!("<?php\n", "new App\\Models\\U\n",);

    let items = complete_at(&backend, &uri, text, 1, 16).await;
    let classes = class_items(&items);

    let user_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("App\\Models\\User"))
        .expect("Should find User via FQN prefix App\\Models\\U");

    // Label should be the FQN (not just the short name) in FQN mode.
    assert_eq!(
        user_item.label, "App\\Models\\User",
        "Label should be the FQN in FQN-prefix mode"
    );

    // text_edit should cover the entire typed prefix so the editor
    // replaces `App\Models\U` with the full FQN.
    assert!(
        user_item.text_edit.is_some(),
        "FQN-prefix completions should have a text_edit with explicit range"
    );
}

/// Typing `\App\Models\U` (with leading backslash) should match and
/// insert text with a leading backslash.
#[tokio::test]
async fn test_fqn_prefix_with_leading_backslash() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Models/User.php",
            concat!("<?php\n", "namespace App\\Models;\n", "class User {}\n",),
        )],
    );

    let user_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Models/User.php").display()
    ))
    .unwrap();
    let user_content = std::fs::read_to_string(_dir.path().join("src/Models/User.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: user_uri,
                language_id: "php".to_string(),
                version: 1,
                text: user_content,
            },
        })
        .await;

    let uri = Url::parse("file:///fqn_leading.php").unwrap();
    let text = concat!("<?php\n", "new \\App\\Models\\U\n",);

    let items = complete_at(&backend, &uri, text, 1, 17).await;
    let classes = class_items(&items);

    let user_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("App\\Models\\User"))
        .expect("Should find User via FQN prefix \\App\\Models\\U");

    // insert_text should include the leading backslash.
    let insert = user_item.insert_text.as_deref().unwrap_or("");
    assert!(
        insert.starts_with("\\App\\Models\\User"),
        "insert_text should start with \\App\\Models\\User, got: {:?}",
        insert
    );
}

/// In FQN-prefix mode, no auto-import `use` statement should be added
/// because the user is explicitly typing the fully-qualified name.
#[tokio::test]
async fn test_fqn_prefix_skips_auto_import() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Models/User.php",
            concat!("<?php\n", "namespace App\\Models;\n", "class User {}\n",),
        )],
    );

    let user_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Models/User.php").display()
    ))
    .unwrap();
    let user_content = std::fs::read_to_string(_dir.path().join("src/Models/User.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: user_uri,
                language_id: "php".to_string(),
                version: 1,
                text: user_content,
            },
        })
        .await;

    let uri = Url::parse("file:///fqn_noimport.php").unwrap();
    let text = concat!("<?php\n", "namespace Other;\n", "new App\\Models\\U\n",);

    let items = complete_at(&backend, &uri, text, 2, 16).await;
    let classes = class_items(&items);

    let user_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("App\\Models\\User"));

    if let Some(item) = user_item {
        assert!(
            item.additional_text_edits.is_none(),
            "FQN-prefix completions should NOT have additional_text_edits (auto-import), got: {:?}",
            item.additional_text_edits
        );
    }
}

/// The filter_text should include the FQN so that the client's fuzzy
/// filter can match against namespace segments (not just the short name).
#[tokio::test]
async fn test_filter_text_includes_fqn_for_namespace_matching() {
    let backend = create_test_backend_with_stubs();

    let scaffolding_uri = Url::parse("file:///filter_scaffold.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: scaffolding_uri,
                language_id: "php".to_string(),
                version: 1,
                text: concat!(
                    "<?php\n",
                    "namespace Vendor\\Package;\n",
                    "class Widget {}\n",
                )
                .to_string(),
            },
        })
        .await;

    let uri = Url::parse("file:///filter_test.php").unwrap();
    let text = concat!("<?php\n", "new Wid\n",);

    let items = complete_at(&backend, &uri, text, 1, 7).await;
    let classes = class_items(&items);

    let widget = classes
        .iter()
        .find(|i| i.label == "Widget" && i.detail.as_deref() == Some("Vendor\\Package\\Widget"))
        .expect("Should find Widget");

    let filter = widget.filter_text.as_deref().unwrap_or("");
    assert!(
        filter.contains("Vendor\\Package"),
        "filter_text should include the FQN so namespace segments are matchable, got: {:?}",
        filter
    );
}

/// The text_edit replacement range should cover the entire typed prefix
/// so that `http\En` is fully replaced with the selected FQN, not
/// appended after the last `\`.
#[tokio::test]
async fn test_fqn_prefix_text_edit_replaces_full_prefix() {
    let backend = create_test_backend_with_stubs();

    let scaffolding_uri = Url::parse("file:///textedit_scaffold.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: scaffolding_uri,
                language_id: "php".to_string(),
                version: 1,
                text: concat!(
                    "<?php\n",
                    "namespace http\\Exception;\n",
                    "class BadUrlException {}\n",
                )
                .to_string(),
            },
        })
        .await;

    let uri = Url::parse("file:///textedit_test.php").unwrap();
    // User typed `http\Ex` (7 chars) starting at col 22
    let text = concat!("<?php\n", "if ($user instanceof http\\Ex) {}\n",);

    let items = complete_at(&backend, &uri, text, 1, 28).await;
    let classes = class_items(&items);

    let exc_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("http\\Exception\\BadUrlException"))
        .expect("Should find BadUrlException via FQN prefix http\\Ex");

    // Must have a text_edit that covers the full prefix `http\Ex`.
    let te = exc_item
        .text_edit
        .as_ref()
        .expect("FQN-prefix completions must have a text_edit");

    match te {
        CompletionTextEdit::Edit(edit) => {
            // The prefix `http\Ex` is 7 characters, starting at col 21.
            assert_eq!(
                edit.range.start,
                Position {
                    line: 1,
                    character: 21,
                },
                "text_edit range should start where the FQN prefix begins"
            );
            assert_eq!(
                edit.range.end,
                Position {
                    line: 1,
                    character: 28,
                },
                "text_edit range should end at the cursor"
            );
            assert!(
                edit.new_text.contains("http\\Exception\\BadUrlException"),
                "text_edit new_text should be the full FQN, got: {:?}",
                edit.new_text
            );
        }
        _ => panic!("Expected CompletionTextEdit::Edit"),
    }
}

/// When the user types `\Demo\` in namespace `Demo` and picks `Demo\Box`,
/// the insert text should be simplified to `Box` (not `\Demo\Box` or `\Box`)
/// because the class is already in the current namespace.
#[tokio::test]
async fn test_fqn_prefix_same_namespace_simplifies_to_short_name() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Demo\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Box.php",
            concat!("<?php\n", "namespace Demo;\n", "class Box {}\n",),
        )],
    );

    // Open the Box file so it's in ast_map.
    let box_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Box.php").display()
    ))
    .unwrap();
    let box_content = std::fs::read_to_string(_dir.path().join("src/Box.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: box_uri,
                language_id: "php".to_string(),
                version: 1,
                text: box_content,
            },
        })
        .await;

    let uri = Url::parse("file:///fqn_same_ns.php").unwrap();
    // We are in namespace Demo, typing `\Demo\` (6 chars, col 25).
    let text = concat!(
        "<?php\n",
        "namespace Demo;\n",
        "if ($user instanceof \\Demo\\) {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 27).await;
    let classes = class_items(&items);

    let box_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("Demo\\Box"))
        .expect("Should find Box via FQN prefix \\Demo\\");

    // The label and insert text should be simplified to `Box`.
    assert_eq!(
        box_item.label, "Box",
        "Label should be the relative name 'Box' when FQN is in current namespace"
    );

    // The text_edit should replace `\Demo\` with just `Box`.
    let te = box_item
        .text_edit
        .as_ref()
        .expect("FQN-prefix completions should have a text_edit");
    match te {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(
                edit.new_text, "Box",
                "text_edit should insert 'Box', not the full FQN"
            );
        }
        _ => panic!("Expected CompletionTextEdit::Edit"),
    }
}

/// When the user types `\Other\` in namespace `Demo` and picks `Other\Foo`,
/// the full FQN should be preserved (not simplified).
#[tokio::test]
async fn test_fqn_prefix_different_namespace_keeps_fqn() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Other\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Foo.php",
            concat!("<?php\n", "namespace Other;\n", "class Foo {}\n",),
        )],
    );

    let foo_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Foo.php").display()
    ))
    .unwrap();
    let foo_content = std::fs::read_to_string(_dir.path().join("src/Foo.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: foo_uri,
                language_id: "php".to_string(),
                version: 1,
                text: foo_content,
            },
        })
        .await;

    let uri = Url::parse("file:///fqn_diff_ns.php").unwrap();
    // We are in namespace Demo, typing `\Other\` (7 chars).
    let text = concat!(
        "<?php\n",
        "namespace Demo;\n",
        "if ($user instanceof \\Other\\) {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 28).await;
    let classes = class_items(&items);

    let foo_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("Other\\Foo"))
        .expect("Should find Foo via FQN prefix \\Other\\");

    // Label should be the full FQN since it's not in the current namespace.
    assert_eq!(
        foo_item.label, "Other\\Foo",
        "Label should be the full FQN when class is in a different namespace"
    );

    let te = foo_item
        .text_edit
        .as_ref()
        .expect("FQN-prefix completions should have a text_edit");
    match te {
        CompletionTextEdit::Edit(edit) => {
            assert!(
                edit.new_text.contains("\\Other\\Foo"),
                "text_edit should insert the full FQN with leading backslash, got: {:?}",
                edit.new_text
            );
        }
        _ => panic!("Expected CompletionTextEdit::Edit"),
    }
}

/// `namespace ` completion should suggest known namespace names, not class names.
#[tokio::test]
async fn test_namespace_declaration_suggests_namespaces() {
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
                "src/Models/User.php",
                concat!("<?php\n", "namespace App\\Models;\n", "class User {}\n",),
            ),
            (
                "src/Services/AuthService.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Services;\n",
                    "class AuthService {}\n",
                ),
            ),
        ],
    );

    // Open one file so its namespace appears in namespace_map.
    let user_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Models/User.php").display()
    ))
    .unwrap();
    let user_content = std::fs::read_to_string(_dir.path().join("src/Models/User.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: user_uri,
                language_id: "php".to_string(),
                version: 1,
                text: user_content,
            },
        })
        .await;

    let uri = Url::parse("file:///ns_decl.php").unwrap();
    let text = concat!("<?php\n", "namespace App\n",);

    let items = complete_at(&backend, &uri, text, 1, 13).await;
    let all_labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // Should include namespace names that are under the PSR-4 prefix.
    assert!(
        all_labels.iter().any(|l| l.contains("App\\Models")),
        "Should suggest App\\Models namespace, got labels: {:?}",
        all_labels
    );

    // Should NOT include class names like "User".
    assert!(
        !all_labels.contains(&"User"),
        "Should NOT suggest class names in namespace context, got labels: {:?}",
        all_labels
    );

    // Should NOT include stub namespaces that are outside PSR-4 prefixes.
    assert!(
        !all_labels.contains(&"Decimal"),
        "Should NOT suggest stub-only namespaces outside PSR-4 prefixes, got labels: {:?}",
        all_labels
    );

    // Items should have MODULE kind.
    for item in &items {
        assert_eq!(
            item.kind,
            Some(CompletionItemKind::MODULE),
            "Namespace items should have MODULE kind, got {:?} for {:?}",
            item.kind,
            item.label
        );
    }
}

/// `namespace ` completion should discover sub-namespaces from cached files
/// that fall under a PSR-4 prefix.
#[tokio::test]
async fn test_namespace_declaration_discovers_cached_sub_namespaces() {
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
                "src/Models/User.php",
                concat!("<?php\n", "namespace App\\Models;\n", "class User {}\n",),
            ),
            (
                "src/Models/Concerns/HasUuids.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Models\\Concerns;\n",
                    "trait HasUuids {}\n",
                ),
            ),
        ],
    );

    // Open the Concerns file so its namespace enters namespace_map.
    let concerns_uri = Url::parse(&format!(
        "file://{}",
        _dir.path()
            .join("src/Models/Concerns/HasUuids.php")
            .display()
    ))
    .unwrap();
    let concerns_content =
        std::fs::read_to_string(_dir.path().join("src/Models/Concerns/HasUuids.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: concerns_uri,
                language_id: "php".to_string(),
                version: 1,
                text: concerns_content,
            },
        })
        .await;

    let uri = Url::parse("file:///ns_subdir.php").unwrap();
    let text = concat!("<?php\n", "namespace App\\Models\n",);

    let items = complete_at(&backend, &uri, text, 1, 20).await;
    let all_labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(
        all_labels.contains(&"App\\Models\\Concerns"),
        "Should discover sub-namespace from cached files, got labels: {:?}",
        all_labels
    );
}

/// `namespace` inside a class body (e.g. `namespace\func()`) should NOT
/// produce namespace-declaration completions.  It should fall through to
/// normal class/function completion instead of MODULE items.
#[tokio::test]
async fn test_namespace_context_not_inside_class_body() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///ns_in_body.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    public function bar() {\n",
        "        namespace\n",
        "    }\n",
        "}\n",
    );

    // Cursor at end of `namespace` on line 3.
    let items = complete_at(&backend, &uri, text, 3, 17).await;

    // If this were incorrectly detected as NamespaceDeclaration, all
    // items would have MODULE kind.  Verify that at least some items
    // have a different kind (CLASS, FUNCTION, etc.).
    let has_non_module = items
        .iter()
        .any(|i| i.kind != Some(CompletionItemKind::MODULE));
    assert!(
        items.is_empty() || has_non_module,
        "`namespace` inside a class body should NOT produce only MODULE completions"
    );
}

/// When the user types `\Demo` (leading backslash, single segment) in
/// namespace `Demo` and picks `Demo\Box`, the result should be `Box`
/// (not `\Box`).  The leading `\` activates FQN mode, and the same-
/// namespace check simplifies the reference.
#[tokio::test]
async fn test_fqn_leading_backslash_single_segment_same_namespace() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Demo\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Box.php",
            concat!("<?php\n", "namespace Demo;\n", "class Box {}\n",),
        )],
    );

    let box_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Box.php").display()
    ))
    .unwrap();
    let box_content = std::fs::read_to_string(_dir.path().join("src/Box.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: box_uri,
                language_id: "php".to_string(),
                version: 1,
                text: box_content,
            },
        })
        .await;

    let uri = Url::parse("file:///fqn_backslash_single.php").unwrap();
    // In namespace Demo, typing `\Demo` (5 chars starting at col 21).
    let text = concat!(
        "<?php\n",
        "namespace Demo;\n",
        "if ($user instanceof \\Demo) {}\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 26).await;
    let classes = class_items(&items);

    let box_item = classes
        .iter()
        .find(|i| i.detail.as_deref() == Some("Demo\\Box"))
        .expect("Should find Box via prefix \\Demo");

    // The text_edit should replace `\Demo` with just `Box`.
    let te = box_item
        .text_edit
        .as_ref()
        .expect("Leading-backslash completions should have a text_edit");
    match te {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(
                edit.new_text, "Box",
                "text_edit should insert 'Box' (same namespace), not '\\Box' or '\\Demo\\Box'"
            );
        }
        _ => panic!("Expected CompletionTextEdit::Edit"),
    }
}

/// `use function` completions should NOT include parentheses and should
/// end with a semicolon so the statement is complete.
#[tokio::test]
async fn test_use_function_no_parentheses() {
    let backend = create_test_backend_with_function_stubs();

    let uri = Url::parse("file:///use_func_parens.php").unwrap();
    let text = concat!("<?php\n", "use function array_ma\n",);

    let items = complete_at(&backend, &uri, text, 1, 21).await;

    let func_items: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::FUNCTION))
        .collect();

    assert!(
        !func_items.is_empty(),
        "Should have function completions for 'array_ma'"
    );

    for item in &func_items {
        let insert = item.insert_text.as_deref().unwrap_or(&item.label);
        assert!(
            !insert.contains('('),
            "use function completions should NOT contain parentheses, got insert_text: {:?} for {:?}",
            insert,
            item.label
        );
        assert!(
            insert.ends_with(';'),
            "use function completions should end with ';', got insert_text: {:?} for {:?}",
            insert,
            item.label
        );
        assert!(
            item.insert_text_format != Some(InsertTextFormat::SNIPPET),
            "use function completions should be plain text, not snippets, for {:?}",
            item.label
        );
    }
}

/// `use const` completions should end with a semicolon.
#[tokio::test]
async fn test_use_const_semicolon_termination() {
    let backend = create_test_backend_with_stubs();

    if let Ok(mut dmap) = backend.global_defines().lock() {
        dmap.insert("MY_CONST".to_string(), "file:///defs.php".to_string());
    }

    let uri = Url::parse("file:///use_const_semi.php").unwrap();
    let text = concat!("<?php\n", "use const MY_C\n",);

    let items = complete_at(&backend, &uri, text, 1, 14).await;

    let const_items: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
        .collect();

    assert!(
        !const_items.is_empty(),
        "Should have constant completions for 'MY_C'"
    );

    for item in &const_items {
        let insert = item.insert_text.as_deref().unwrap_or(&item.label);
        assert!(
            insert.ends_with(';'),
            "use const completions should end with ';', got insert_text: {:?} for {:?}",
            insert,
            item.label
        );
    }
}

/// `use` (class import) completions should end with a semicolon, but the
/// `function` / `const` keyword hints should NOT (they continue the statement).
#[tokio::test]
async fn test_use_class_import_semicolon_termination() {
    let backend = create_test_backend_with_stubs();

    // Register a class so that `use DateT` has something to complete.
    let scaffold_uri = Url::parse("file:///scaffold_datetime.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: scaffold_uri,
                language_id: "php".to_string(),
                version: 1,
                text: "<?php\nnamespace App;\nclass DateTransformer {}\n".to_string(),
            },
        })
        .await;

    let uri = Url::parse("file:///use_class_semi.php").unwrap();
    let text = concat!("<?php\n", "use DateT\n",);

    let items = complete_at(&backend, &uri, text, 1, 9).await;

    let class_items: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::CLASS))
        .collect();

    assert!(
        !class_items.is_empty(),
        "Should have class completions for 'DateT', got: {:?}",
        labels(&items)
    );

    for item in &class_items {
        let insert = item.insert_text.as_deref().unwrap_or(&item.label);
        assert!(
            insert.ends_with(';'),
            "use class completions should end with ';', got insert_text: {:?} for {:?}",
            insert,
            item.label
        );
    }

    // The `function` / `const` keyword hints should NOT have semicolons
    // because they continue the statement (e.g. `use function `).
    let keyword_items: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::KEYWORD))
        .collect();
    for item in &keyword_items {
        let insert = item.insert_text.as_deref().unwrap_or(&item.label);
        assert!(
            !insert.ends_with(';'),
            "keyword hints should NOT end with ';', got insert_text: {:?} for {:?}",
            insert,
            item.label
        );
    }
}

/// Namespace declaration completion should exclude namespaces that are
/// not under any PSR-4 prefix (e.g. stub-only namespaces like `Decimal`).
#[tokio::test]
async fn test_namespace_declaration_excludes_non_psr4_namespaces() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/Models/User.php",
            concat!("<?php\n", "namespace App\\Models;\n", "class User {}\n",),
        )],
    );

    // Open User so its namespace enters namespace_map.
    let user_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Models/User.php").display()
    ))
    .unwrap();
    let user_content = std::fs::read_to_string(_dir.path().join("src/Models/User.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: user_uri,
                language_id: "php".to_string(),
                version: 1,
                text: user_content,
            },
        })
        .await;

    // Also inject a class_index entry for a non-PSR-4 namespace.
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "MySql\\Enums\\IntTypes".to_string(),
            "file:///somewhere.php".to_string(),
        );
    }

    let uri = Url::parse("file:///ns_filter.php").unwrap();
    let text = concat!("<?php\n", "namespace My\n",);

    let items = complete_at(&backend, &uri, text, 1, 12).await;
    let all_labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // MySql is NOT under the App\ PSR-4 prefix, so it should be excluded.
    assert!(
        !all_labels.iter().any(|l| l.contains("MySql")),
        "Should NOT suggest namespaces outside PSR-4 prefixes, got labels: {:?}",
        all_labels
    );
}

/// Namespace completion should only include PSR-4 prefixes and cached
/// namespaces under those prefixes, with each level exploded.
#[tokio::test]
async fn test_namespace_declaration_psr4_and_cached_only() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "JohnyDogood\\": "src/",
                    "JUtils\\": "utils/"
                }
            }
        }"#,
        &[
            (
                "src/Money/USD.php",
                concat!(
                    "<?php\n",
                    "namespace JohnyDogood\\Money;\n",
                    "class USD {}\n",
                ),
            ),
            ("utils/.gitkeep", ""),
        ],
    );

    // Open the USD file so its namespace is cached.
    let usd_uri = Url::parse(&format!(
        "file://{}",
        _dir.path().join("src/Money/USD.php").display()
    ))
    .unwrap();
    let usd_content = std::fs::read_to_string(_dir.path().join("src/Money/USD.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: usd_uri,
                language_id: "php".to_string(),
                version: 1,
                text: usd_content,
            },
        })
        .await;

    // Also inject a class_index entry for a class outside PSR-4.
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "MySql\\Enums\\IntTypes".to_string(),
            "file:///ext.php".to_string(),
        );
    }

    let uri = Url::parse("file:///ns_psr4.php").unwrap();
    let text = concat!("<?php\n", "namespace J\n",);

    let items = complete_at(&backend, &uri, text, 1, 11).await;
    let all_labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    // PSR-4 prefixes should be present.
    assert!(
        all_labels.contains(&"JohnyDogood"),
        "Should suggest PSR-4 prefix JohnyDogood, got: {:?}",
        all_labels
    );
    assert!(
        all_labels.contains(&"JUtils"),
        "Should suggest PSR-4 prefix JUtils, got: {:?}",
        all_labels
    );

    // Cached sub-namespace under PSR-4 prefix should be present.
    assert!(
        all_labels.contains(&"JohnyDogood\\Money"),
        "Should suggest cached sub-namespace JohnyDogood\\Money, got: {:?}",
        all_labels
    );

    // Class basename should NOT appear.
    assert!(
        !all_labels.contains(&"USD"),
        "Should NOT suggest class names, got: {:?}",
        all_labels
    );

    // Non-PSR-4 namespace should NOT appear.
    assert!(
        !all_labels.iter().any(|l| l.contains("MySql")),
        "Should NOT suggest non-PSR-4 namespaces, got: {:?}",
        all_labels
    );
}

/// When typing `namespace Tests\Feature\D` and picking `Tests\Feature\Domain`,
/// the text_edit must replace the entire typed prefix (`Tests\Feature\D`) so
/// the result is `namespace Tests\Feature\Domain;` — not the doubled
/// `namespace Tests\Feature\Tests\Feature\Domain;`.
#[tokio::test]
async fn test_namespace_declaration_replaces_full_prefix() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "Tests\\": "tests/"
                }
            }
        }"#,
        &[(
            "tests/Feature/Domain/SomeTest.php",
            concat!(
                "<?php\n",
                "namespace Tests\\Feature\\Domain;\n",
                "class SomeTest {}\n",
            ),
        )],
    );

    // Open the file so its namespace enters namespace_map / ast_map.
    let file_uri = Url::parse(&format!(
        "file://{}",
        _dir.path()
            .join("tests/Feature/Domain/SomeTest.php")
            .display()
    ))
    .unwrap();
    let file_content =
        std::fs::read_to_string(_dir.path().join("tests/Feature/Domain/SomeTest.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri,
                language_id: "php".to_string(),
                version: 1,
                text: file_content,
            },
        })
        .await;

    let uri = Url::parse("file:///ns_replace.php").unwrap();
    // Typing `namespace Tests\Feature\D` — cursor at col 27.
    let text = concat!("<?php\n", "namespace Tests\\Feature\\D\n",);

    let items = complete_at(&backend, &uri, text, 1, 25).await;

    let domain_item = items
        .iter()
        .find(|i| i.label == "Tests\\Feature\\Domain")
        .expect("Should find Tests\\Feature\\Domain in namespace completions");

    // The item MUST carry a text_edit that replaces the full typed prefix.
    let te = domain_item
        .text_edit
        .as_ref()
        .expect("Namespace completions with backslash should have a text_edit");
    match te {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(
                edit.new_text, "Tests\\Feature\\Domain",
                "text_edit should insert the full namespace"
            );
            // The range should start at the beginning of the typed prefix
            // (col 10, right after `namespace `).
            assert_eq!(
                edit.range.start,
                Position {
                    line: 1,
                    character: 10
                },
                "replacement range should start at the beginning of the typed prefix"
            );
            assert_eq!(
                edit.range.end,
                Position {
                    line: 1,
                    character: 25
                },
                "replacement range should end at the cursor"
            );
        }
        _ => panic!("Expected CompletionTextEdit::Edit"),
    }
}

/// When the user has `use Cassandra\Exception;` and types `Exception\AlreadyEx`,
/// picking `Cassandra\Exception\AlreadyExistsException` should insert
/// `Exception\AlreadyExistsException` (shortened via the use-map prefix),
/// not the full FQN.
#[tokio::test]
async fn test_fqn_shortened_via_use_map_prefix() {
    let backend = create_test_backend_with_stubs();

    // Put the class in class_index so it appears in completions.
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Cassandra\\Exception\\AlreadyExistsException".to_string(),
            "file:///vendor/cassandra.php".to_string(),
        );
    }

    let uri = Url::parse("file:///shorten_prefix.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use Cassandra\\Exception;\n",
        "if ($user instanceof Exception\\AlreadyEx) {}\n",
    );

    // Cursor at end of `AlreadyEx` on line 2 (col 40).
    let items = complete_at(&backend, &uri, text, 2, 40).await;
    let cls = class_items(&items);

    let item = cls
        .iter()
        .find(|i| i.detail.as_deref() == Some("Cassandra\\Exception\\AlreadyExistsException"))
        .expect("Should find AlreadyExistsException in completions");

    // The label should be the shortened form, not the full FQN.
    assert_eq!(
        item.label, "Exception\\AlreadyExistsException",
        "label should be shortened via use-map prefix"
    );

    // The text_edit should insert the shortened form.
    let te = item
        .text_edit
        .as_ref()
        .expect("FQN completions should have a text_edit");
    match te {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(
                edit.new_text, "Exception\\AlreadyExistsException",
                "text_edit should insert the shortened form"
            );
        }
        _ => panic!("Expected CompletionTextEdit::Edit"),
    }

    // No additional use statement should be generated.
    assert!(
        item.additional_text_edits.is_none()
            || item.additional_text_edits.as_ref().unwrap().is_empty(),
        "should not generate a use import when already reachable via existing import"
    );
}

/// When the user has `use Cassandra\Exception\AlreadyExistsException;` and
/// types `\Cassa`, picking the class should insert just
/// `AlreadyExistsException` (the short imported name) rather than the FQN.
#[tokio::test]
async fn test_fqn_shortened_via_use_map_exact_match_leading_backslash() {
    let backend = create_test_backend_with_stubs();

    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Cassandra\\Exception\\AlreadyExistsException".to_string(),
            "file:///vendor/cassandra.php".to_string(),
        );
    }

    let uri = Url::parse("file:///shorten_exact.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use Cassandra\\Exception\\AlreadyExistsException;\n",
        "if ($user instanceof \\Cassa) {}\n",
    );

    // Cursor at end of `\Cassa` on line 2 (col 27).
    let items = complete_at(&backend, &uri, text, 2, 27).await;
    let cls = class_items(&items);

    let item = cls
        .iter()
        .find(|i| i.detail.as_deref() == Some("Cassandra\\Exception\\AlreadyExistsException"))
        .expect("Should find AlreadyExistsException in completions");

    // The label should be the short imported name.
    assert_eq!(
        item.label, "AlreadyExistsException",
        "label should be shortened to the imported alias"
    );

    // The text_edit should replace `\Cassa` with just the short name.
    let te = item
        .text_edit
        .as_ref()
        .expect("FQN completions should have a text_edit");
    match te {
        CompletionTextEdit::Edit(edit) => {
            assert_eq!(
                edit.new_text, "AlreadyExistsException",
                "text_edit should insert the short imported name, not the FQN"
            );
        }
        _ => panic!("Expected CompletionTextEdit::Edit"),
    }
}

/// Use-map shortening should NOT apply in `use` import context —
/// the user is writing a `use` statement and needs the full FQN.
#[tokio::test]
async fn test_use_import_context_does_not_shorten() {
    let backend = create_test_backend_with_stubs();

    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Cassandra\\Exception\\AlreadyExistsException".to_string(),
            "file:///vendor/cassandra.php".to_string(),
        );
    }

    let uri = Url::parse("file:///use_no_shorten.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use Cassandra\\Exception;\n",
        "use Cassandra\\Exception\\Already\n",
    );

    // Cursor at end of `Already` on line 2.
    let items = complete_at(&backend, &uri, text, 2, 31).await;
    let cls = class_items(&items);

    let item = cls
        .iter()
        .find(|i| i.detail.as_deref() == Some("Cassandra\\Exception\\AlreadyExistsException"))
        .expect("Should find AlreadyExistsException in use-import completions");

    // In use-import context, the label should be the full FQN.
    assert_eq!(
        item.label, "Cassandra\\Exception\\AlreadyExistsException",
        "use-import context should NOT shorten via use-map"
    );
}

/// A `use` statement that imports a namespace (not a class) should NOT
/// produce a phantom class completion item.  E.g. `use Luxplus\Core\Enums as LCE;`
/// where `Enums` is a namespace containing enum classes, not a class itself.
#[tokio::test]
async fn test_namespace_alias_import_not_shown_as_class() {
    let backend = create_test_backend_with_stubs();

    // Register classes UNDER the namespace so the LSP knows it's a namespace.
    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Luxplus\\Core\\Enums\\Status".to_string(),
            "file:///vendor/luxplus/enums/Status.php".to_string(),
        );
        idx.insert(
            "Luxplus\\Core\\Enums\\Color".to_string(),
            "file:///vendor/luxplus/enums/Color.php".to_string(),
        );
    }

    // Use prefix "LCE" which matches the alias for the namespace import.
    let uri = Url::parse("file:///ns_alias.php").unwrap();
    let text = concat!("<?php\n", "use Luxplus\\Core\\Enums as LCE;\n", "new LCE\n",);

    let items = complete_at(&backend, &uri, text, 2, 7).await;
    let cls = class_items(&items);

    // `Luxplus\Core\Enums` is a namespace, not a class — it should NOT
    // appear as a completion item.
    let phantom = cls
        .iter()
        .find(|i| i.detail.as_deref() == Some("Luxplus\\Core\\Enums"));
    assert!(
        phantom.is_none(),
        "Namespace alias should not appear as a class completion, got: {:?}",
        phantom
    );
}

/// Classes under a namespace-aliased import should still appear when
/// the typed prefix matches their short name.
#[tokio::test]
async fn test_classes_under_namespace_alias_still_available() {
    let backend = create_test_backend_with_stubs();

    if let Ok(mut idx) = backend.class_index().lock() {
        idx.insert(
            "Luxplus\\Core\\Enums\\Status".to_string(),
            "file:///vendor/luxplus/enums/Status.php".to_string(),
        );
    }

    let uri = Url::parse("file:///ns_alias_child.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use Luxplus\\Core\\Enums as LCE;\n",
        "new Stat\n",
    );

    let items = complete_at(&backend, &uri, text, 2, 8).await;
    let cls = class_items(&items);

    let status = cls
        .iter()
        .find(|i| i.detail.as_deref() == Some("Luxplus\\Core\\Enums\\Status"));
    assert!(
        status.is_some(),
        "Classes under the namespace should still appear in completions"
    );
}

/// A `use` import for a class that hasn't been discovered yet should
/// still appear in completions (benefit of the doubt).
#[tokio::test]
async fn test_undiscovered_use_import_still_shown() {
    let backend = create_test_backend_with_stubs();

    // Don't register anything in class_index or classmap — the class
    // is imported but completely unknown to the LSP.

    let uri = Url::parse("file:///undiscovered.php").unwrap();
    let text = concat!("<?php\n", "use Vendor\\SomeLibrary\\Widget;\n", "new Wid\n",);

    let items = complete_at(&backend, &uri, text, 2, 7).await;
    let cls = class_items(&items);
    let labels: Vec<&str> = cls.iter().map(|i| i.label.as_str()).collect();

    // The imported class should appear even though it's not in any index.
    assert!(
        labels.contains(&"Widget"),
        "Undiscovered use-imported class should still appear, got: {:?}",
        labels
    );
}
