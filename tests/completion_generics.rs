mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Generic type resolution tests ──────────────────────────────────────────
//
// These tests verify that `@template` parameters declared on a parent class
// are correctly substituted with concrete types when a child class uses
// `@extends Parent<ConcreteType1, ConcreteType2>`.

/// Basic test: a child class extends a generic parent with concrete types.
/// Methods inherited from the parent should have their template parameter
/// return types resolved to the concrete types.
#[tokio::test]
async fn test_generic_extends_resolves_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_basic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Box {\n",
        "    /** @return T */\n",
        "    public function get() {}\n",
        "    /** @return void */\n",
        "    public function set() {}\n",
        "}\n",
        "\n",
        "class Apple {\n",
        "    public function bite(): void {}\n",
        "    public function peel(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends Box<Apple>\n",
        " */\n",
        "class AppleBox extends Box {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $box = new AppleBox();\n",
        "    $box->get()->\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 24,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"bite"),
                "Should resolve T to Apple and show Apple's 'bite' method, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"peel"),
                "Should resolve T to Apple and show Apple's 'peel' method, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test with two template parameters (like Collection<TKey, TValue>).
#[tokio::test]
async fn test_generic_extends_two_params_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_two_params.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "    /** @return TValue|null */\n",
        "    public function last() {}\n",
        "}\n",
        "\n",
        "class Language {\n",
        "    public int $priority;\n",
        "    public function getCode(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends Collection<int, Language>\n",
        " */\n",
        "class LanguageCollection extends Collection {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $col = new LanguageCollection();\n",
        "    $col->first()->\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 25,
                character: 21,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getCode"),
                "Should resolve TValue to Language and show 'getCode', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that @template-covariant is also parsed correctly.
#[tokio::test]
async fn test_generic_template_covariant() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_covariant.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TKey of array-key\n",
        " * @template-covariant TValue\n",
        " */\n",
        "class TypedList {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "}\n",
        "\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends TypedList<int, User>\n",
        " */\n",
        "class UserList extends TypedList {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $list = new UserList();\n",
        "    $list->first()->\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 23,
                character: 21,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should resolve TValue (covariant) to User, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getEmail"),
                "Should resolve TValue (covariant) to User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that the child's own methods are still available alongside
/// inherited generic-resolved methods.
#[tokio::test]
async fn test_generic_child_own_methods_preserved() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_own_methods.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class GenericRepo {\n",
        "    /** @return T */\n",
        "    public function find() {}\n",
        "}\n",
        "\n",
        "class Product {\n",
        "    public function getPrice(): float {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends GenericRepo<Product>\n",
        " */\n",
        "class ProductRepo extends GenericRepo {\n",
        "    public function findByCategory(string $cat): void {}\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
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

    let completion_params = CompletionParams {
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
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            // Own method
            assert!(
                method_names.contains(&"findByCategory"),
                "Should include own method 'findByCategory', got: {:?}",
                method_names
            );

            // Inherited method with resolved generic
            assert!(
                method_names.contains(&"find"),
                "Should include inherited 'find' method, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that inherited generic method return type resolves in a chained call.
#[tokio::test]
async fn test_generic_method_return_type_chain() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Order {\n",
        "    public function getTotal(): float {}\n",
        "    public function getStatus(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Repository {\n",
        "    /** @return T */\n",
        "    public function findFirst() {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends Repository<Order>\n",
        " */\n",
        "class OrderRepository extends Repository {\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    public function getRepo(): OrderRepository {}\n",
        "    function test() {\n",
        "        $this->getRepo()->findFirst()->\n",
        "    }\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 23,
                character: 42,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getTotal"),
                "Chain should resolve T→Order and show 'getTotal', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getStatus"),
                "Chain should resolve T→Order and show 'getStatus', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test nullable generic return type: `@return ?TValue` → `?Language`.
#[tokio::test]
async fn test_generic_nullable_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_nullable.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Container {\n",
        "    /** @return ?T */\n",
        "    public function maybeGet() {}\n",
        "}\n",
        "\n",
        "class Widget {\n",
        "    public function render(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends Container<Widget>\n",
        " */\n",
        "class WidgetContainer extends Container {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $c = new WidgetContainer();\n",
        "    $c->maybeGet()->\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 21,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"render"),
                "Should resolve ?T to ?Widget and show 'render', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test property type substitution: inherited properties with template
/// types should be substituted.  Uses `$this->value->` inside the child
/// class, which is the supported property-chain resolution path.
#[tokio::test]
async fn test_generic_property_type_substitution() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_property.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Wrapper {\n",
        "    /** @var T */\n",
        "    public $value;\n",
        "}\n",
        "\n",
        "class Config {\n",
        "    public function get(string $key): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends Wrapper<Config>\n",
        " */\n",
        "class ConfigWrapper extends Wrapper {\n",
        "    function test() {\n",
        "        $this->value->\n",
        "    }\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 18,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"get"),
                "Should resolve property type T→Config and show 'get', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test generic resolution across files via PSR-4.
#[tokio::test]
async fn test_generic_extends_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let parent_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "class GenericCollection {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "    /** @return TValue|null */\n",
        "    public function last() {}\n",
        "    /** @return array<TKey, TValue> */\n",
        "    public function all() {}\n",
        "}\n",
    );

    let item_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Item {\n",
        "    public function getName(): string {}\n",
        "    public function getPrice(): float {}\n",
        "}\n",
    );

    let child_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "use App\\GenericCollection;\n",
        "\n",
        "/**\n",
        " * @extends GenericCollection<int, Item>\n",
        " */\n",
        "class ItemCollection extends GenericCollection {\n",
        "    public function filterExpensive(): self {}\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/GenericCollection.php", parent_php),
            ("src/Item.php", item_php),
            ("src/ItemCollection.php", child_php),
        ],
    );

    // Open the file that uses ItemCollection
    let usage_text = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "use App\\ItemCollection;\n",
        "\n",
        "function test() {\n",
        "    $items = new ItemCollection();\n",
        "    $items->first()->\n",
        "}\n",
    );

    let uri = Url::parse("file:///test_usage.php").unwrap();
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: usage_text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 7,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Cross-file: should resolve TValue→Item and show 'getName', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getPrice"),
                "Cross-file: should resolve TValue→Item and show 'getPrice', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that non-template return types remain unchanged after substitution.
/// E.g. a method returning `void` or `self` should not be affected.
#[tokio::test]
async fn test_generic_non_template_return_types_unchanged() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_non_template.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class BaseList {\n",
        "    /** @return T */\n",
        "    public function first() {}\n",
        "    /** @return self */\n",
        "    public function filter(): self {}\n",
        "    /** @return int */\n",
        "    public function count(): int {}\n",
        "}\n",
        "\n",
        "class Task {\n",
        "    public function run(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends BaseList<Task>\n",
        " */\n",
        "class TaskList extends BaseList {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $list = new TaskList();\n",
        "    $list->\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 25,
                character: 11,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"first"),
                "Should include 'first', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"filter"),
                "Should include 'filter' (returns self, not a template), got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"count"),
                "Should include 'count' (returns int, not a template), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test chained generics: C extends B<Foo>, B extends A<T>.
/// A's methods with template param U should resolve to Foo for C.
#[tokio::test]
async fn test_generic_chained_inheritance() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_chained.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template U\n",
        " */\n",
        "class GrandParent_ {\n",
        "    /** @return U */\n",
        "    public function getItem() {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @template T\n",
        " * @extends GrandParent_<T>\n",
        " */\n",
        "class Parent_ extends GrandParent_ {\n",
        "    /** @return T */\n",
        "    public function findItem() {}\n",
        "}\n",
        "\n",
        "class Car {\n",
        "    public function drive(): void {}\n",
        "    public function park(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends Parent_<Car>\n",
        " */\n",
        "class CarStore extends Parent_ {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $store = new CarStore();\n",
        "    $store->findItem()->\n",
        "}\n",
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

    // Test that Parent_::findItem() resolves T → Car
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 31,
                character: 27,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"drive"),
                "Should resolve T→Car on Parent_::findItem() and show 'drive', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that grandparent methods with template params also resolve
/// through the chain.
#[tokio::test]
async fn test_generic_grandparent_method_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_grandparent.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template U\n",
        " */\n",
        "class BaseRepo {\n",
        "    /** @return U */\n",
        "    public function find() {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @template T\n",
        " * @extends BaseRepo<T>\n",
        " */\n",
        "class CachingRepo extends BaseRepo {\n",
        "    public function clearCache(): void {}\n",
        "}\n",
        "\n",
        "class Invoice {\n",
        "    public function getPdf(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends CachingRepo<Invoice>\n",
        " */\n",
        "class InvoiceRepo extends CachingRepo {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $repo = new InvoiceRepo();\n",
        "    $repo->find()->\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 29,
                character: 20,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getPdf"),
                "Grandparent: should resolve U→T→Invoice and show 'getPdf', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test @phpstan-extends variant is also recognized.
#[tokio::test]
async fn test_generic_phpstan_extends_variant() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_phpstan.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class GenericStack {\n",
        "    /** @return T */\n",
        "    public function pop() {}\n",
        "}\n",
        "\n",
        "class Message {\n",
        "    public function send(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @phpstan-extends GenericStack<Message>\n",
        " */\n",
        "class MessageStack extends GenericStack {\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $stack = new MessageStack();\n",
        "    $stack->pop()->\n",
        "}\n",
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 21,
                character: 20,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"send"),
                "@phpstan-extends should resolve T→Message, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that when no @extends is present, template params remain unresolved
/// (no crash, methods still inherited, just without concrete types).
#[tokio::test]
async fn test_generic_without_extends_annotation_no_crash() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generics_no_extends.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class GenericParent {\n",
        "    /** @return T */\n",
        "    public function get() {}\n",
        "    public function size(): int {}\n",
        "}\n",
        "\n",
        // No @extends annotation — just plain extends
        "class PlainChild extends GenericParent {\n",
        "    function test() {\n",
        "        $this->\n",
        "    }\n",
        "}\n",
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

    let completion_params = CompletionParams {
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
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            // Methods should still be inherited even without @extends generics
            assert!(
                method_names.contains(&"get"),
                "Should inherit 'get' even without @extends, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"size"),
                "Should inherit 'size' even without @extends, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Docblock parsing unit tests ────────────────────────────────────────────

/// Test that `extract_template_params` correctly parses various @template variants.
#[test]
fn test_extract_template_params_basic() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @template T\n */";
    assert_eq!(extract_template_params(docblock), vec!["T"]);
}

#[test]
fn test_extract_template_params_multiple() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @template TKey\n * @template TValue\n */";
    assert_eq!(extract_template_params(docblock), vec!["TKey", "TValue"]);
}

#[test]
fn test_extract_template_params_with_constraint() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @template TKey of array-key\n * @template TValue\n */";
    assert_eq!(extract_template_params(docblock), vec!["TKey", "TValue"]);
}

#[test]
fn test_extract_template_params_covariant() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @template TKey\n * @template-covariant TValue\n */";
    assert_eq!(extract_template_params(docblock), vec!["TKey", "TValue"]);
}

#[test]
fn test_extract_template_params_contravariant() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @template-contravariant TInput\n */";
    assert_eq!(extract_template_params(docblock), vec!["TInput"]);
}

#[test]
fn test_extract_template_params_phpstan_prefix() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @phpstan-template T\n */";
    assert_eq!(extract_template_params(docblock), vec!["T"]);
}

#[test]
fn test_extract_template_params_phpstan_covariant() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @phpstan-template-covariant TValue\n */";
    assert_eq!(extract_template_params(docblock), vec!["TValue"]);
}

#[test]
fn test_extract_template_params_empty() {
    use phpantom_lsp::docblock::extract_template_params;

    let docblock = "/**\n * @return void\n */";
    assert_eq!(extract_template_params(docblock), Vec::<String>::new());
}

/// Test that `extract_generics_tag` correctly parses @extends tags.
#[test]
fn test_extract_generics_tag_extends_basic() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @extends Collection<int, Language>\n */";
    let result = extract_generics_tag(docblock, "@extends");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Collection");
    assert_eq!(result[0].1, vec!["int", "Language"]);
}

#[test]
fn test_extract_generics_tag_extends_single_param() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @extends Box<Apple>\n */";
    let result = extract_generics_tag(docblock, "@extends");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Box");
    assert_eq!(result[0].1, vec!["Apple"]);
}

#[test]
fn test_extract_generics_tag_phpstan_extends() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @phpstan-extends Collection<int, User>\n */";
    let result = extract_generics_tag(docblock, "@extends");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Collection");
    assert_eq!(result[0].1, vec!["int", "User"]);
}

#[test]
fn test_extract_generics_tag_implements() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @implements ArrayAccess<string, User>\n */";
    let result = extract_generics_tag(docblock, "@implements");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "ArrayAccess");
    assert_eq!(result[0].1, vec!["string", "User"]);
}

#[test]
fn test_extract_generics_tag_with_fqn() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @extends \\Illuminate\\Support\\Collection<int, \\App\\Model>\n */";
    let result = extract_generics_tag(docblock, "@extends");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Illuminate\\Support\\Collection");
    assert_eq!(result[0].1, vec!["int", "App\\Model"]);
}

#[test]
fn test_extract_generics_tag_nested_generic() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @extends Base<array<int, string>, User>\n */";
    let result = extract_generics_tag(docblock, "@extends");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Base");
    assert_eq!(result[0].1, vec!["array<int, string>", "User"]);
}

#[test]
fn test_extract_generics_tag_no_generics() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @return void\n */";
    let result = extract_generics_tag(docblock, "@extends");
    assert!(result.is_empty());
}

#[test]
fn test_extract_generics_tag_extends_without_angle_brackets() {
    use phpantom_lsp::docblock::extract_generics_tag;

    // @extends without generics should be ignored by extract_generics_tag
    let docblock = "/**\n * @extends SomeClass\n */";
    let result = extract_generics_tag(docblock, "@extends");
    assert!(result.is_empty());
}

#[test]
fn test_extract_generics_tag_multiple_implements() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = concat!(
        "/**\n",
        " * @implements ArrayAccess<int, User>\n",
        " * @implements Countable\n",
        " * @implements IteratorAggregate<int, User>\n",
        " */",
    );
    let result = extract_generics_tag(docblock, "@implements");
    // Only entries with generics are returned
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0, "ArrayAccess");
    assert_eq!(result[0].1, vec!["int", "User"]);
    assert_eq!(result[1].0, "IteratorAggregate");
    assert_eq!(result[1].1, vec!["int", "User"]);
}

// ─── Method-level @template tests ───────────────────────────────────────────
//
// These tests verify that `@template` parameters declared on individual
// methods (not classes) are resolved from call-site arguments.
//
// The canonical pattern:
//   @template T
//   @param class-string<T> $class
//   @return T
// Calling `find(User::class)` should resolve the return type to `User`.

/// Unit test: `synthesize_template_conditional` creates a conditional
/// from a basic `@template T` + `@param class-string<T>` + `@return T`.
#[test]
fn test_synthesize_template_conditional_basic() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param class-string<T> $class\n",
        " * @return T\n",
        " */",
    );
    let template_params = vec!["T".to_string()];
    let result = synthesize_template_conditional(docblock, &template_params, Some("T"), false);
    assert!(
        result.is_some(),
        "Should synthesize a conditional for @template T with class-string<T>"
    );
}

/// Unit test: no synthesis when return type is not a template param.
#[test]
fn test_synthesize_template_conditional_non_template_return() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param class-string<T> $class\n",
        " * @return string\n",
        " */",
    );
    let template_params = vec!["T".to_string()];
    let result = synthesize_template_conditional(docblock, &template_params, Some("string"), false);
    assert!(
        result.is_none(),
        "Should NOT synthesize when return type is not a template param"
    );
}

/// Unit test: no synthesis when there are no template params.
#[test]
fn test_synthesize_template_conditional_no_templates() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @param string $class\n",
        " * @return string\n",
        " */",
    );
    let template_params: Vec<String> = vec![];
    let result = synthesize_template_conditional(docblock, &template_params, Some("string"), false);
    assert!(
        result.is_none(),
        "Should NOT synthesize when there are no template params"
    );
}

/// Unit test: no synthesis when an existing conditional is present.
#[test]
fn test_synthesize_template_conditional_existing_conditional() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param class-string<T> $class\n",
        " * @return T\n",
        " */",
    );
    let template_params = vec!["T".to_string()];
    let result = synthesize_template_conditional(docblock, &template_params, Some("T"), true);
    assert!(
        result.is_none(),
        "Should NOT synthesize when has_existing_conditional is true"
    );
}

/// Unit test: handles nullable return type `?T`.
#[test]
fn test_synthesize_template_conditional_nullable_return() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param class-string<T> $class\n",
        " * @return ?T\n",
        " */",
    );
    let template_params = vec!["T".to_string()];
    let result = synthesize_template_conditional(docblock, &template_params, Some("?T"), false);
    assert!(
        result.is_some(),
        "Should synthesize for nullable return type ?T"
    );
}

/// Unit test: no synthesis when no class-string param matches the template.
#[test]
fn test_synthesize_template_conditional_no_class_string_param() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param string $class\n",
        " * @return T\n",
        " */",
    );
    let template_params = vec!["T".to_string()];
    let result = synthesize_template_conditional(docblock, &template_params, Some("T"), false);
    assert!(
        result.is_none(),
        "Should NOT synthesize when no @param has class-string<T>"
    );
}

/// Unit test: handles nullable class-string param `?class-string<T>`.
#[test]
fn test_synthesize_template_conditional_nullable_class_string() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param ?class-string<T> $class\n",
        " * @return T\n",
        " */",
    );
    let template_params = vec!["T".to_string()];
    let result = synthesize_template_conditional(docblock, &template_params, Some("T"), false);
    assert!(
        result.is_some(),
        "Should synthesize for nullable class-string param ?class-string<T>"
    );
}

/// Unit test: handles class-string param with null union `class-string<T>|null`.
#[test]
fn test_synthesize_template_conditional_class_string_null_union() {
    use phpantom_lsp::docblock::synthesize_template_conditional;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param class-string<T>|null $class\n",
        " * @return T\n",
        " */",
    );
    let template_params = vec!["T".to_string()];
    let result = synthesize_template_conditional(docblock, &template_params, Some("T"), false);
    assert!(
        result.is_some(),
        "Should synthesize for class-string<T>|null union param"
    );
}

/// Integration test: method-level @template resolves in assignment context.
/// `$user = $repo->find(User::class)` should resolve $user to User.
#[tokio::test]
async fn test_method_template_assignment_resolves_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_template_assign.php").unwrap();
    let text = concat!(
        "<?php\n",                                              // 0
        "class User {\n",                                       // 1
        "    public function getName(): string {}\n",           // 2
        "    public function getEmail(): string {}\n",          // 3
        "}\n",                                                  // 4
        "\n",                                                   // 5
        "class Repository {\n",                                 // 6
        "    /**\n",                                            // 7
        "     * @template T\n",                                 // 8
        "     * @param class-string<T> $class\n",               // 9
        "     * @return T\n",                                   // 10
        "     */\n",                                            // 11
        "    public function find(string $class): object {}\n", // 12
        "}\n",                                                  // 13
        "\n",                                                   // 14
        "function test() {\n",                                  // 15
        "    $repo = new Repository();\n",                      // 16
        "    $user = $repo->find(User::class);\n",              // 17
        "    $user->\n",                                        // 18
        "}\n",                                                  // 19
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 18,
                character: 11,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should resolve T to User and show 'getName', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getEmail"),
                "Should resolve T to User and show 'getEmail', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: method-level @template resolves in inline chain context.
/// `$repo->find(User::class)->` should show User's members directly.
#[tokio::test]
async fn test_method_template_inline_chain_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_template_chain.php").unwrap();
    let text = concat!(
        "<?php\n",                                                    // 0
        "class Product {\n",                                          // 1
        "    public function getPrice(): float {}\n",                 // 2
        "    public function getTitle(): string {}\n",                // 3
        "}\n",                                                        // 4
        "\n",                                                         // 5
        "class EntityManager {\n",                                    // 6
        "    /**\n",                                                  // 7
        "     * @template T\n",                                       // 8
        "     * @param class-string<T> $entityClass\n",               // 9
        "     * @return T\n",                                         // 10
        "     */\n",                                                  // 11
        "    public function find(string $entityClass): object {}\n", // 12
        "}\n",                                                        // 13
        "\n",                                                         // 14
        "function test(EntityManager $em) {\n",                       // 15
        "    $em->find(Product::class)->\n",                          // 16
        "}\n",                                                        // 17
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 16,
                character: 35,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getPrice"),
                "Should resolve T to Product and show 'getPrice', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTitle"),
                "Should resolve T to Product and show 'getTitle', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: method-level @template works on static methods.
/// `Repository::find(Order::class)->` should resolve to Order.
#[tokio::test]
async fn test_method_template_static_method_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_template_static.php").unwrap();
    let text = concat!(
        "<?php\n",                                                     // 0
        "class Order {\n",                                             // 1
        "    public function getTotal(): float {}\n",                  // 2
        "    public function getStatus(): string {}\n",                // 3
        "}\n",                                                         // 4
        "\n",                                                          // 5
        "class Repository {\n",                                        // 6
        "    /**\n",                                                   // 7
        "     * @template T\n",                                        // 8
        "     * @param class-string<T> $class\n",                      // 9
        "     * @return T\n",                                          // 10
        "     */\n",                                                   // 11
        "    public static function find(string $class): object {}\n", // 12
        "}\n",                                                         // 13
        "\n",                                                          // 14
        "function test() {\n",                                         // 15
        "    Repository::find(Order::class)->\n",                      // 16
        "}\n",                                                         // 17
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 16,
                character: 39,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getTotal"),
                "Should resolve T to Order and show 'getTotal', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getStatus"),
                "Should resolve T to Order and show 'getStatus', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: method-level @template on a standalone function.
/// `resolve(Config::class)->` should resolve to Config.
#[tokio::test]
async fn test_function_template_resolves_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///function_template.php").unwrap();
    let text = concat!(
        "<?php\n",                                                     // 0
        "class Config {\n",                                            // 1
        "    public function get(string $key): mixed {}\n",            // 2
        "    public function set(string $key, mixed $val): void {}\n", // 3
        "}\n",                                                         // 4
        "\n",                                                          // 5
        "/**\n",                                                       // 6
        " * @template T\n",                                            // 7
        " * @param class-string<T> $class\n",                          // 8
        " * @return T\n",                                              // 9
        " */\n",                                                       // 10
        "function resolve(string $class): object {}\n",                // 11
        "\n",                                                          // 12
        "function test() {\n",                                         // 13
        "    resolve(Config::class)->\n",                              // 14
        "}\n",                                                         // 15
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 14,
                character: 33,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"get"),
                "Should resolve T to Config and show 'get', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"set"),
                "Should resolve T to Config and show 'set', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: function-level @template with assignment.
/// `$config = resolve(Config::class); $config->` should show Config's members.
#[tokio::test]
async fn test_function_template_assignment_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///function_template_assign.php").unwrap();
    let text = concat!(
        "<?php\n",                                           // 0
        "class Logger {\n",                                  // 1
        "    public function info(string $msg): void {}\n",  // 2
        "    public function error(string $msg): void {}\n", // 3
        "}\n",                                               // 4
        "\n",                                                // 5
        "/**\n",                                             // 6
        " * @template T\n",                                  // 7
        " * @param class-string<T> $abstract\n",             // 8
        " * @return T\n",                                    // 9
        " */\n",                                             // 10
        "function resolve(string $abstract): object {}\n",   // 11
        "\n",                                                // 12
        "function test() {\n",                               // 13
        "    $logger = resolve(Logger::class);\n",           // 14
        "    $logger->\n",                                   // 15
        "}\n",                                               // 16
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 15,
                character: 13,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"info"),
                "Should resolve T to Logger and show 'info', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"error"),
                "Should resolve T to Logger and show 'error', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: method-level @template used inside $this-> context.
/// `$this->find(User::class)->` from within the same class.
#[tokio::test]
async fn test_method_template_this_context_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_template_this.php").unwrap();
    let text = concat!(
        "<?php\n",                                          // 0
        "class Address {\n",                                // 1
        "    public function getCity(): string {}\n",       // 2
        "    public function getZip(): string {}\n",        // 3
        "}\n",                                              // 4
        "\n",                                               // 5
        "class Container {\n",                              // 6
        "    /**\n",                                        // 7
        "     * @template T\n",                             // 8
        "     * @param class-string<T> $id\n",              // 9
        "     * @return T\n",                               // 10
        "     */\n",                                        // 11
        "    public function get(string $id): object {}\n", // 12
        "\n",                                               // 13
        "    public function test() {\n",                   // 14
        "        $this->get(Address::class)->\n",           // 15
        "    }\n",                                          // 16
        "}\n",                                              // 17
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 15,
                character: 40,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getCity"),
                "Should resolve T to Address and show 'getCity', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getZip"),
                "Should resolve T to Address and show 'getZip', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: method-level @template does NOT break methods
/// that have an explicit PHPStan conditional return type.
#[tokio::test]
async fn test_method_template_does_not_override_existing_conditional() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_template_existing.php").unwrap();
    let text = concat!(
        "<?php\n",                                                             // 0
        "class Session {\n",                                                   // 1
        "    public function getId(): string {}\n",                            // 2
        "}\n",                                                                 // 3
        "\n",                                                                  // 4
        "class App {\n",                                                       // 5
        "    /**\n",                                                           // 6
        "     * @template TClass\n",                                           // 7
        "     * @param class-string<TClass>|null $abstract\n",                 // 8
        "     * @return ($abstract is class-string<TClass> ? TClass : App)\n", // 9
        "     */\n",                                                           // 10
        "    public function make(?string $abstract = null): mixed {}\n",      // 11
        "}\n",                                                                 // 12
        "\n",                                                                  // 13
        "function test(App $app) {\n",                                         // 14
        "    $app->make(Session::class)->\n",                                  // 15
        "}\n",                                                                 // 16
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 15,
                character: 35,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            // The explicit conditional should still work — Session is resolved.
            assert!(
                method_names.contains(&"getId"),
                "Explicit conditional should still resolve Session, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: @template with @phpstan-template variant.
#[tokio::test]
async fn test_method_phpstan_template_resolves() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_phpstan_template.php").unwrap();
    let text = concat!(
        "<?php\n",                                                // 0
        "class Invoice {\n",                                      // 1
        "    public function getAmount(): float {}\n",            // 2
        "}\n",                                                    // 3
        "\n",                                                     // 4
        "class Finder {\n",                                       // 5
        "    /**\n",                                              // 6
        "     * @phpstan-template T\n",                           // 7
        "     * @param class-string<T> $type\n",                  // 8
        "     * @return T\n",                                     // 9
        "     */\n",                                              // 10
        "    public function findOne(string $type): object {}\n", // 11
        "}\n",                                                    // 12
        "\n",                                                     // 13
        "function test(Finder $f) {\n",                           // 14
        "    $f->findOne(Invoice::class)->\n",                    // 15
        "}\n",                                                    // 16
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 15,
                character: 35,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getAmount"),
                "@phpstan-template should resolve T to Invoice, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: @template with cross-file PSR-4 resolution.
/// The target class is defined in a separate file, loaded via PSR-4.
#[tokio::test]
async fn test_method_template_cross_file_resolves() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#,
        &[(
            "src/Payment.php",
            "<?php\nnamespace App;\nclass Payment {\n    public function charge(): void {}\n    public function refund(): void {}\n}\n",
        )],
    );

    let uri = Url::parse("file:///method_template_cross.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Payment;\n",
        "\n",
        "class ServiceLocator {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param class-string<T> $id\n",
        "     * @return T\n",
        "     */\n",
        "    public function get(string $id): object {}\n",
        "}\n",
        "\n",
        "function test(ServiceLocator $sl) {\n",
        "    $sl->get(Payment::class)->\n",
        "}\n",
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

    let completion_params = CompletionParams {
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
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"charge"),
                "Should resolve T to Payment cross-file and show 'charge', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"refund"),
                "Should resolve T to Payment cross-file and show 'refund', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Integration test: method-level @template with a different param name.
/// Uses `$entityClass` instead of `$class`.
#[tokio::test]
async fn test_method_template_different_param_name() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_template_param_name.php").unwrap();
    let text = concat!(
        "<?php\n",                                                    // 0
        "class Customer {\n",                                         // 1
        "    public function getLoyaltyPoints(): int {}\n",           // 2
        "}\n",                                                        // 3
        "\n",                                                         // 4
        "class ORM {\n",                                              // 5
        "    /**\n",                                                  // 6
        "     * @template TEntity\n",                                 // 7
        "     * @param class-string<TEntity> $entityClass\n",         // 8
        "     * @return TEntity\n",                                   // 9
        "     */\n",                                                  // 10
        "    public function find(string $entityClass): object {}\n", // 11
        "}\n",                                                        // 12
        "\n",                                                         // 13
        "function test(ORM $orm) {\n",                                // 14
        "    $orm->find(Customer::class)->\n",                        // 15
        "}\n",                                                        // 16
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

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 15,
                character: 35,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Completion should return results");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getLoyaltyPoints"),
                "Should resolve TEntity to Customer and show 'getLoyaltyPoints', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
