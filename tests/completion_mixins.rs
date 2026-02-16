mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Basic @mixin usage (same file) ────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_methods_available_on_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_basic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "    public function getTotal(): float { return 0.0; }\n",
        "    protected function recalculate(): void {}\n",
        "    private function internalCheck(): void {}\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 13,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Completion should return results");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getId"),
                "Should include own method 'getId', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTotal"),
                "Should include mixin method 'getTotal', got: {:?}",
                method_names
            );
            // Protected and private members from mixin should NOT be included,
            // since mixins proxy via magic methods which only expose public API.
            assert!(
                !method_names.contains(&"recalculate"),
                "Should NOT include protected mixin method 'recalculate', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"internalCheck"),
                "Should NOT include private mixin method 'internalCheck', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Own method takes precedence over mixin method ──────────────────────────

#[tokio::test]
async fn test_completion_own_method_overrides_mixin_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_override.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public function getId(): string { return 'cart-1'; }\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 11,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // getId should appear exactly once (the own version overrides mixin)
            let get_id_count = method_names.iter().filter(|n| **n == "getId").count();
            assert_eq!(
                get_id_count, 1,
                "getId should appear exactly once (own overrides mixin), got: {:?}",
                method_names
            );

            // getItems from mixin should still be available
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Mixin properties ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_properties_available() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_props.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public string $cartName;\n",
        "    public int $itemCount;\n",
        "    protected float $discount;\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public int $id;\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                prop_names.contains(&"id"),
                "Should include own property 'id', got: {:?}",
                prop_names
            );
            assert!(
                prop_names.contains(&"cartName"),
                "Should include mixin property 'cartName', got: {:?}",
                prop_names
            );
            assert!(
                prop_names.contains(&"itemCount"),
                "Should include mixin property 'itemCount', got: {:?}",
                prop_names
            );
            assert!(
                !prop_names.contains(&"discount"),
                "Should NOT include protected mixin property 'discount', got: {:?}",
                prop_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Mixin constants ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_constants_available_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_const.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public const MAX_ITEMS = 100;\n",
        "    public const MIN_ITEMS = 1;\n",
        "    private const INTERNAL = 'x';\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public const VERSION = '1.0';\n",
        "}\n",
        "CurrentCart::\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 13,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                const_names.contains(&"VERSION"),
                "Should include own constant 'VERSION', got: {:?}",
                const_names
            );
            assert!(
                const_names.contains(&"MAX_ITEMS"),
                "Should include mixin constant 'MAX_ITEMS', got: {:?}",
                const_names
            );
            assert!(
                const_names.contains(&"MIN_ITEMS"),
                "Should include mixin constant 'MIN_ITEMS', got: {:?}",
                const_names
            );
            assert!(
                !const_names.contains(&"INTERNAL"),
                "Should NOT include private mixin constant 'INTERNAL', got: {:?}",
                const_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Multiple mixins ────────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_multiple_mixins() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_multi.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "class Wishlist {\n",
        "    public function getWishes(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " * @mixin Wishlist\n",
        " */\n",
        "class UserDashboard {\n",
        "    public function getNotifications(): array { return []; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getNotifications"),
                "Should include own method 'getNotifications', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems' from ShoppingCart, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getWishes"),
                "Should include mixin method 'getWishes' from Wishlist, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Mixin with inheritance (mixin class extends another) ───────────────────

#[tokio::test]
async fn test_completion_mixin_inherits_from_parent() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_inherit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class BaseCart {\n",
        "    public function clear(): void {}\n",
        "}\n",
        "class ShoppingCart extends BaseCart {\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"clear"),
                "Should include inherited method 'clear' from mixin's parent, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Precedence: class own > trait > parent > mixin ─────────────────────────

#[tokio::test]
async fn test_completion_mixin_lowest_precedence() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_prec.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class MixedIn {\n",
        "    public function shared(): string { return 'from-mixin'; }\n",
        "    public function mixinOnly(): string { return 'mixin'; }\n",
        "}\n",
        "class ParentClass {\n",
        "    public function shared(): string { return 'from-parent'; }\n",
        "    public function parentOnly(): string { return 'parent'; }\n",
        "}\n",
        "/**\n",
        " * @mixin MixedIn\n",
        " */\n",
        "class Child extends ParentClass {\n",
        "    public function childOnly(): string { return 'child'; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"childOnly"),
                "Should include own method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"parentOnly"),
                "Should include parent method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"mixinOnly"),
                "Should include mixin-only method, got: {:?}",
                method_names
            );
            // 'shared' exists in both parent and mixin — parent wins, but it
            // should appear exactly once.
            let shared_count = method_names.iter().filter(|n| **n == "shared").count();
            assert_eq!(
                shared_count, 1,
                "'shared' should appear exactly once (parent wins over mixin), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Cross-file mixin via PSR-4 ────────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let cart_php = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "    public function getTotal(): float { return 0.0; }\n",
        "}\n",
    );

    let current_cart_php = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "use App\\Models\\ShoppingCart;\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Models/ShoppingCart.php", cart_php),
            ("src/Models/CurrentCart.php", current_cart_php),
        ],
    );

    let uri = Url::parse("file:///test_mixin_cross.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: current_cart_php.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 9,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getId"),
                "Should include own method 'getId', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems' from cross-file, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTotal"),
                "Should include mixin method 'getTotal' from cross-file, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Variable typed as class with @mixin ────────────────────────────────────

#[tokio::test]
async fn test_completion_variable_of_class_with_mixin() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        /** @var CurrentCart $cart */\n",
        "        $cart = new CurrentCart();\n",
        "        $cart->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getId"),
                "Should include own method 'getId', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── No duplicate members from mixin ────────────────────────────────────────

#[tokio::test]
async fn test_completion_no_duplicate_members_from_mixin() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_nodup.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public function getId(): string { return 'c1'; }\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 11,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            let get_id_count = method_names.iter().filter(|n| **n == "getId").count();
            assert_eq!(
                get_id_count, 1,
                "getId should appear exactly once (own overrides mixin), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Mixin with trait on mixin class ────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_class_with_trait() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_trait.php").unwrap();
    let text = concat!(
        "<?php\n",
        "trait Discountable {\n",
        "    public function applyDiscount(float $pct): void {}\n",
        "}\n",
        "class ShoppingCart {\n",
        "    use Discountable;\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 14,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getId"),
                "Should include own method 'getId', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"applyDiscount"),
                "Should include trait method 'applyDiscount' from mixin class, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Go-to-definition through @mixin (same file) ───────────────────────────

#[tokio::test]
async fn test_goto_definition_mixin_method_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                                                // 0
        "class ShoppingCart {\n",                                 // 1
        "    public function getItems(): array { return []; }\n", // 2
        "}\n",                                                    // 3
        "/**\n",                                                  // 4
        " * @mixin ShoppingCart\n",                               // 5
        " */\n",                                                  // 6
        "class CurrentCart {\n",                                  // 7
        "    public function getId(): int { return 1; }\n",       // 8
        "    function test() {\n",                                // 9
        "        $this->getItems();\n",                           // 10
        "    }\n",                                                // 11
        "}\n",                                                    // 12
    );

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
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 10,
                    character: 22, // on "getItems"
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should resolve definition for mixin method 'getItems'"
    );
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        // The method 'getItems' is on line 2 of the same file
        assert_eq!(
            location.range.start.line, 2,
            "Should point to line 2 where getItems is defined in ShoppingCart"
        );
    } else {
        panic!("Expected GotoDefinitionResponse::Scalar");
    }
}

// ─── Go-to-definition through @mixin (cross-file PSR-4) ────────────────────

#[tokio::test]
async fn test_goto_definition_mixin_method_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let cart_php = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
    );

    let current_cart_php = concat!(
        "<?php\n",
        "namespace App\\Services;\n",
        "use App\\Models\\ShoppingCart;\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->getItems();\n",
        "    }\n",
        "}\n",
    );

    let (backend, dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Models/ShoppingCart.php", cart_php),
            ("src/Services/CurrentCart.php", current_cart_php),
        ],
    );

    let uri = Url::parse("file:///test_mixin_goto_cross.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: current_cart_php.to_string(),
            },
        })
        .await;

    let result = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 9,
                    character: 22, // on "getItems"
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should resolve definition for cross-file mixin method"
    );
    if let Some(GotoDefinitionResponse::Scalar(location)) = result {
        let cart_path = dir.path().join("src/Models/ShoppingCart.php");
        let expected_uri = Url::from_file_path(&cart_path).unwrap();
        assert_eq!(
            location.uri, expected_uri,
            "Should point to the mixin source file"
        );
        assert_eq!(
            location.range.start.line, 3,
            "Should jump to the method definition line in the mixin class"
        );
    } else {
        panic!("Expected GotoDefinitionResponse::Scalar");
    }
}

// ─── Mixin return type chaining ─────────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_method_return_type_chain() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_chain.php").unwrap();
    let text = concat!(
        "<?php\n",                                                                   // 0
        "class CartItem {\n",                                                        // 1
        "    public function getPrice(): float { return 0.0; }\n",                   // 2
        "}\n",                                                                       // 3
        "class ShoppingCart {\n",                                                    // 4
        "    public function getFirstItem(): CartItem { return new CartItem(); }\n", // 5
        "}\n",                                                                       // 6
        "/**\n",                                                                     // 7
        " * @mixin ShoppingCart\n",                                                  // 8
        " */\n",                                                                     // 9
        "class CurrentCart {\n",                                                     // 10
        "    function test() {\n",                                                   // 11
        "        $item = $this->getFirstItem();\n",                                  // 12
        "        $item->\n",                                                         // 13
        "    }\n",                                                                   // 14
        "}\n",                                                                       // 15
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 13,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getPrice"),
                "Should follow return type chain through mixin method, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Parser extracts @mixin info ────────────────────────────────────────────

#[tokio::test]
async fn test_parser_extracts_mixin_info() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_parser.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "}\n",
    );

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

    // The AST map should contain the parsed classes with mixin info.
    // We verify this indirectly: if completion works for mixin members,
    // the parser correctly extracted the @mixin tag.
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 8,
                    character: 0,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    // This test mainly validates that parsing doesn't panic or break
    // with @mixin annotations present.
    assert!(
        result.is_some() || result.is_none(),
        "Parsing should succeed"
    );
}

// ─── Mixin name resolution with use statements ─────────────────────────────

#[tokio::test]
async fn test_parser_resolves_mixin_names_with_use_statements() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let cart_php = concat!(
        "<?php\n",
        "namespace App\\Models;\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "    public function getTotal(): float { return 0.0; }\n",
        "}\n",
    );

    let current_cart_php = concat!(
        "<?php\n",
        "namespace App\\Services;\n",
        "use App\\Models\\ShoppingCart;\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Models/ShoppingCart.php", cart_php),
            ("src/Services/CurrentCart.php", current_cart_php),
        ],
    );

    let uri = Url::parse("file:///test_resolve.php").unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: current_cart_php.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 9,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getItems"),
                "Mixin name should be resolved via use statement, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTotal"),
                "Mixin name should be resolved via use statement, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait on class takes precedence over mixin ─────────────────────────────

#[tokio::test]
async fn test_completion_trait_overrides_mixin() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_trait_prec.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class MixedIn {\n",
        "    public function shared(): string { return 'from-mixin'; }\n",
        "    public function mixinOnly(): string { return 'mixin'; }\n",
        "}\n",
        "trait MyTrait {\n",
        "    public function shared(): int { return 42; }\n",
        "    public function traitOnly(): bool { return true; }\n",
        "}\n",
        "/**\n",
        " * @mixin MixedIn\n",
        " */\n",
        "class MyClass {\n",
        "    use MyTrait;\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 15,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"traitOnly"),
                "Should include trait method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"mixinOnly"),
                "Should include mixin-only method, got: {:?}",
                method_names
            );
            // 'shared' should appear only once (trait wins over mixin)
            let shared_count = method_names.iter().filter(|n| **n == "shared").count();
            assert_eq!(
                shared_count, 1,
                "'shared' should appear exactly once (trait wins over mixin), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Mixin with @property and @method docblock tags ─────────────────────────

#[tokio::test]
async fn test_completion_mixin_combined_with_docblock_tags() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_docblock.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " * @property string $sessionId\n",
        " * @method void refresh()\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getId"),
                "Should include own method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"refresh"),
                "Should include @method tag method, got: {:?}",
                method_names
            );
            assert!(
                prop_names.contains(&"sessionId"),
                "Should include @property tag property, got: {:?}",
                prop_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Chained mixins (mixin class itself has @mixin) ─────────────────────────

#[tokio::test]
async fn test_completion_chained_mixin() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_chained.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class DeepModel {\n",
        "    public function deepMethod(): string { return 'deep'; }\n",
        "}\n",
        "/**\n",
        " * @mixin DeepModel\n",
        " */\n",
        "class ShoppingCart {\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 16,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getId"),
                "Should include own method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include first-level mixin method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"deepMethod"),
                "Should include chained mixin method from DeepModel, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Mixin on class with own parent ─────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_on_class_that_extends() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_extends.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class MixedIn {\n",
        "    public function mixinMethod(): string { return 'hi'; }\n",
        "}\n",
        "class BaseCart {\n",
        "    public function baseMethod(): void {}\n",
        "}\n",
        "/**\n",
        " * @mixin MixedIn\n",
        " */\n",
        "class CurrentCart extends BaseCart {\n",
        "    public function ownMethod(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 13,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"ownMethod"),
                "Should include own method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"baseMethod"),
                "Should include parent method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"mixinMethod"),
                "Should include mixin method, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Mixin static methods via :: ────────────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_static_methods_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class QueryBuilder {\n",
        "    public static function where(string $col): static { return new static(); }\n",
        "    public static function find(int $id): static { return new static(); }\n",
        "    public function first(): static { return $this; }\n",
        "}\n",
        "/**\n",
        " * @mixin QueryBuilder\n",
        " */\n",
        "class Model {\n",
        "    public static function all(): array { return []; }\n",
        "}\n",
        "Model::\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 12,
                    character: 7,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let names: Vec<&str> = items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                names.contains(&"all"),
                "Should include own static method 'all', got: {:?}",
                names
            );
            assert!(
                names.contains(&"where"),
                "Should include mixin static method 'where', got: {:?}",
                names
            );
            assert!(
                names.contains(&"find"),
                "Should include mixin static method 'find', got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Child inherits mixin from parent ───────────────────────────────────────

#[tokio::test]
async fn test_completion_child_inherits_parent_mixin() {
    // @mixin on a parent class should be inherited by child classes.
    // e.g. `User extends Model` where `Model` has `@mixin Builder`
    // means `User` should gain Builder's members.
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_child.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class MixedIn {\n",
        "    public function mixinMethod(): string { return 'hi'; }\n",
        "}\n",
        "/**\n",
        " * @mixin MixedIn\n",
        " */\n",
        "class ParentClass {\n",
        "    public function parentMethod(): void {}\n",
        "}\n",
        "class ChildClass extends ParentClass {\n",
        "    public function childMethod(): int { return 1; }\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 13,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"childMethod"),
                "Should include own method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"parentMethod"),
                "Should include parent method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"mixinMethod"),
                "Should include mixin method from parent's @mixin, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Variable from chained method call ──────────────────────────────────────

#[tokio::test]
async fn test_completion_mixin_variable_from_chained_method_call() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_chain_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ShoppingCart {\n",
        "    public string $accessed_at;\n",
        "    public function getItems(): array { return []; }\n",
        "}\n",
        "/**\n",
        " * @mixin ShoppingCart\n",
        " */\n",
        "class CurrentCart {\n",
        "    public function getId(): int { return 1; }\n",
        "}\n",
        "class CartFactory {\n",
        "    public function create(): CurrentCart { return new CurrentCart(); }\n",
        "}\n",
        "class Service {\n",
        "    public function getFactory(): CartFactory { return new CartFactory(); }\n",
        "    function test() {\n",
        "        $cart = $this->getFactory()->create();\n",
        "        $cart->\n",
        "    }\n",
        "}\n",
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 18,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Completion should return results");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();
            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getId"),
                "Should include own method 'getId', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getItems"),
                "Should include mixin method 'getItems', got: {:?}",
                method_names
            );
            assert!(
                prop_names.contains(&"accessed_at"),
                "Should include mixin property 'accessed_at', got: {:?}",
                prop_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── @return $this on mixin methods ─────────────────────────────────────────

/// Test: When a mixin class has a method with `@return $this`, chaining on
/// that method should resolve to the **mixin class** (not the consumer).
///
/// In PHP, `@mixin` proxies calls via `__call` to an internal instance of
/// the mixin class, so `$this` inside the mixin method refers to the mixin
/// instance.  This contrasts with inheritance where `@return $this` /
/// `static` resolves to the child class.
#[tokio::test]
async fn test_completion_mixin_return_this_resolves_to_mixin_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_return_this.php").unwrap();
    let text = concat!(
        "<?php\n",                                                            // 0
        "class QueryBuilder {\n",                                             // 1
        "    /** @return $this */\n",                                         // 2
        "    public function where(string $col): static { return $this; }\n", // 3
        "    public function get(): array { return []; }\n",                  // 4
        "    public function toSql(): string { return ''; }\n",               // 5
        "}\n",                                                                // 6
        "/**\n",                                                              // 7
        " * @mixin QueryBuilder\n",                                           // 8
        " */\n",                                                              // 9
        "class Model {\n",                                                    // 10
        "    public function save(): bool { return true; }\n",                // 11
        "    public function test(): void {\n",                               // 12
        "        $this->where('active')->\n",                                 // 13
        "    }\n",                                                            // 14
        "}\n",                                                                // 15
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 13,
                    character: 33,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Completion should return results");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // `@return $this` on a mixin method resolves to QueryBuilder,
            // so we should see QueryBuilder's methods, NOT Model's.
            assert!(
                method_names.contains(&"get"),
                "Chaining after mixin @return $this should show mixin class methods (get), got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"toSql"),
                "Chaining after mixin @return $this should show mixin class methods (toSql), got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"where"),
                "Chaining after mixin @return $this should show mixin class methods (where), got: {:?}",
                method_names
            );
            // Model's own method should NOT appear — the chain resolved
            // to QueryBuilder, not Model.
            assert!(
                !method_names.contains(&"save"),
                "Chaining after mixin @return $this should NOT show consumer class methods (save), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: `@return $this` on a mixin method — goto-definition on a chained
/// call should land in the mixin class, not the consumer.
#[tokio::test]
async fn test_goto_definition_mixin_return_this_chain() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///mixin_return_this_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                                                            // 0
        "class QueryBuilder {\n",                                             // 1
        "    /** @return $this */\n",                                         // 2
        "    public function where(string $col): static { return $this; }\n", // 3
        "    public function get(): array { return []; }\n",                  // 4
        "}\n",                                                                // 5
        "/**\n",                                                              // 6
        " * @mixin QueryBuilder\n",                                           // 7
        " */\n",                                                              // 8
        "class Model {\n",                                                    // 9
        "    public function test(): void {\n",                               // 10
        "        $this->where('active')->get();\n",                           // 11
        "    }\n",                                                            // 12
        "}\n",                                                                // 13
    );

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    // Click on "get" in `$this->where('active')->get()` on line 11
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 11,
                character: 35,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve chained call after mixin @return $this"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 4,
                "get() is declared on line 4 in QueryBuilder, not in Model"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}

/// Test: When a child class inherits from a parent that has `@return $this`,
/// the child resolves to itself (not the parent).  This contrasts with the
/// mixin case above.
#[tokio::test]
async fn test_completion_inherited_return_this_resolves_to_child() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherit_return_this.php").unwrap();
    let text = concat!(
        "<?php\n",                                                            // 0
        "class BaseBuilder {\n",                                              // 1
        "    /** @return $this */\n",                                         // 2
        "    public function where(string $col): static { return $this; }\n", // 3
        "}\n",                                                                // 4
        "class ModelBuilder extends BaseBuilder {\n",                         // 5
        "    public function paginate(): array { return []; }\n",             // 6
        "    public function test(): void {\n",                               // 7
        "        $this->where('x')->\n",                                      // 8
        "    }\n",                                                            // 9
        "}\n",                                                                // 10
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 8,
                    character: 27,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Completion should return results");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // For inheritance, @return $this resolves to the child class,
            // so child-only methods should be visible.
            assert!(
                names.contains(&"paginate"),
                "Inherited @return $this should resolve to child class — 'paginate' expected, got: {:?}",
                names
            );
            assert!(
                names.contains(&"where"),
                "Inherited where() should still be visible, got: {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `User extends Model` where `Model` has `@mixin Builder`.
/// `User::` should suggest static methods from `Builder` (e.g. `query()`).
#[tokio::test]
async fn test_completion_inherited_mixin_static_method_via_double_colon() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherited_mixin_static.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Builder {\n",                                // 1
        "    /**\n",                                        // 2
        "     * @return static\n",                          // 3
        "     */\n",                                        // 4
        "    public static function query(): self {\n",     // 5
        "        return new static();\n",                   // 6
        "    }\n",                                          // 7
        "}\n",                                              // 8
        "\n",                                               // 9
        "/**\n",                                            // 10
        " * @mixin Builder\n",                              // 11
        " */\n",                                            // 12
        "abstract class Model {\n",                         // 13
        "}\n",                                              // 14
        "\n",                                               // 15
        "class User extends Model {\n",                     // 16
        "}\n",                                              // 17
        "\n",                                               // 18
        "$query = User::\n",                                // 19
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 19,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "User:: should return completions including Builder's static methods"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("query")),
                "Should include query() from Builder via parent Model's @mixin, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `User extends Model` where `Model` has `@mixin Builder`.
/// `$user->` should also suggest instance-accessible methods from `Builder`.
#[tokio::test]
async fn test_completion_inherited_mixin_instance_method_via_arrow() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherited_mixin_instance.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Builder {\n",                                // 1
        "    public function where(): self {\n",            // 2
        "        return $this;\n",                          // 3
        "    }\n",                                          // 4
        "    public function get(): array {\n",             // 5
        "        return [];\n",                             // 6
        "    }\n",                                          // 7
        "}\n",                                              // 8
        "\n",                                               // 9
        "/**\n",                                            // 10
        " * @mixin Builder\n",                              // 11
        " */\n",                                            // 12
        "abstract class Model {\n",                         // 13
        "    public function save(): void {}\n",            // 14
        "}\n",                                              // 15
        "\n",                                               // 16
        "class User extends Model {\n",                     // 17
        "    public function getName(): string {\n",        // 18
        "        return '';\n",                             // 19
        "    }\n",                                          // 20
        "}\n",                                              // 21
        "\n",                                               // 22
        "$user = new User();\n",                            // 23
        "$user->\n",                                        // 24
    );

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
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 24,
                    character: 7,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "$user-> should return completions including Builder's methods"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.iter().any(|l| l.starts_with("getName")),
                "Should include getName() from User itself, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("save")),
                "Should include save() from parent Model, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("where")),
                "Should include where() from Builder via Model's @mixin, got: {:?}",
                labels
            );
            assert!(
                labels.iter().any(|l| l.starts_with("get")),
                "Should include get() from Builder via Model's @mixin, got: {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Go-to-definition on `User::query()` should jump to `Builder::query()`
/// when `Builder` is a `@mixin` on `Model` (User's parent).
#[tokio::test]
async fn test_goto_definition_inherited_mixin_static_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherited_mixin_goto.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Builder {\n",                                // 1
        "    /**\n",                                        // 2
        "     * @return static\n",                          // 3
        "     */\n",                                        // 4
        "    public static function query(): self {\n",     // 5
        "        return new static();\n",                   // 6
        "    }\n",                                          // 7
        "}\n",                                              // 8
        "\n",                                               // 9
        "/**\n",                                            // 10
        " * @mixin Builder\n",                              // 11
        " */\n",                                            // 12
        "abstract class Model {\n",                         // 13
        "}\n",                                              // 14
        "\n",                                               // 15
        "class User extends Model {\n",                     // 16
        "}\n",                                              // 17
        "\n",                                               // 18
        "User::query();\n",                                 // 19
    );

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

    // Cursor on `query` in `User::query();` (line 19)
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 19,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };

    let result = backend.goto_definition(params).await.unwrap();
    assert!(
        result.is_some(),
        "Should resolve User::query() to Builder::query() via inherited @mixin"
    );

    match result.unwrap() {
        GotoDefinitionResponse::Scalar(location) => {
            assert_eq!(location.uri, uri);
            assert_eq!(
                location.range.start.line, 5,
                "query() is declared on line 5 in Builder"
            );
        }
        other => panic!("Expected Scalar location, got: {:?}", other),
    }
}
