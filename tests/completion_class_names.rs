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
    let text = concat!("<?php\n", "new \\Unit\n",);

    let items = complete_at(&backend, &uri, text, 1, 9).await;
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
    let text = concat!("<?php\n", "new \\Backed\n",);

    let items = complete_at(&backend, &uri, text, 1, 11).await;
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
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"User"),
        "Typing '\\Us' should match 'User', got: {:?}",
        class_labels
    );
}

// ─── Stub class name completion tests ───────────────────────────────────────

#[tokio::test]
async fn test_class_name_completion_includes_stubs() {
    let backend = create_test_backend_with_stubs();

    let uri = Url::parse("file:///test.php").unwrap();

    // Check UnitEnum is found when typing "Unit"
    let text_unit = concat!("<?php\n", "new Unit\n",);
    let items_unit = complete_at(&backend, &uri, text_unit, 1, 8).await;
    let classes_unit = class_items(&items_unit);
    let labels_unit: Vec<&str> = classes_unit.iter().map(|i| i.label.as_str()).collect();

    assert!(
        !classes_unit.is_empty(),
        "Should return class name completions when typing a class name"
    );
    assert!(
        labels_unit.contains(&"UnitEnum"),
        "Should include stub class 'UnitEnum', got: {:?}",
        labels_unit
    );

    // Check BackedEnum is found when typing "Backed"
    let text_backed = concat!("<?php\n", "new Backed\n",);
    let items_backed = complete_at(&backend, &uri, text_backed, 1, 10).await;
    let classes_backed = class_items(&items_backed);
    let labels_backed: Vec<&str> = classes_backed.iter().map(|i| i.label.as_str()).collect();

    assert!(
        labels_backed.contains(&"BackedEnum"),
        "Should include stub class 'BackedEnum', got: {:?}",
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
    let text = concat!(
        "<?php\n",
        "class Foo {\n",
        "    function bar() {\n",
        "        $x = new Back\n",
        "    }\n",
        "}\n",
    );

    let items = complete_at(&backend, &uri, text, 3, 21).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"BackedEnum"),
        "Should offer class names after 'new' keyword, got: {:?}",
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
    let text = concat!("<?php\n", "class MyEnum extends Back\n",);

    let items = complete_at(&backend, &uri, text, 1, 28).await;
    let classes = class_items(&items);
    let class_labels: Vec<&str> = classes.iter().map(|i| i.label.as_str()).collect();

    assert!(
        class_labels.contains(&"BackedEnum"),
        "Should offer class names in extends clause, got: {:?}",
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
    let text = concat!("<?php\n", "new Uni\n",);

    let items = complete_at(&backend, &uri, text, 1, 7).await;
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
async fn test_new_context_demotes_likely_non_instantiable_stubs() {
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

    let concrete = classes
        .iter()
        .find(|i| i.label == "ConcreteService")
        .expect("Should find ConcreteService");
    let abstract_item = classes
        .iter()
        .find(|i| i.label == "AbstractService")
        .expect("Should find AbstractService (unloaded stub, included but demoted)");

    assert!(
        concrete.sort_text < abstract_item.sort_text,
        "ConcreteService ({:?}) should sort before AbstractService ({:?})",
        concrete.sort_text,
        abstract_item.sort_text
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
