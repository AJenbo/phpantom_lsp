mod common;

use common::{create_psr4_workspace, create_test_backend_with_stubs};
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
    if let Ok(mut cm) = backend.classmap.lock() {
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
    if let Ok(mut idx) = backend.class_index.lock() {
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
    if let Ok(mut cm) = backend.classmap.lock() {
        *cm = classmap;
    }

    // Also add to class_index (same FQN)
    if let Ok(mut idx) = backend.class_index.lock() {
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
    *backend.workspace_root.lock().unwrap() = Some(dir.path().to_path_buf());

    // Populate classmap
    let classmap = parse_autoload_classmap(dir.path(), "vendor");
    if let Ok(mut cm) = backend.classmap.lock() {
        *cm = classmap;
    }

    // Add a class_index entry
    if let Ok(mut idx) = backend.class_index.lock() {
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
    if let Ok(mut cm) = backend.classmap.lock() {
        *cm = classmap;
    }

    let uri = Url::parse("file:///test.php").unwrap();
    let text = concat!("<?php\n", "new My\n",);

    let items = complete_at(&backend, &uri, text, 1, 6).await;
    let classes = class_items(&items);

    let my_class = classes.iter().find(|i| i.label == "MyClass").unwrap();
    assert_eq!(
        my_class.insert_text.as_deref(),
        Some("MyClass"),
        "insert_text should be the short class name"
    );
    assert_eq!(
        my_class.detail.as_deref(),
        Some("Deep\\Nested\\Namespace\\MyClass"),
        "detail should show the FQN"
    );
}
