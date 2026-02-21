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

// ─── Generic context preservation through clean_type ────────────────────────
//
// These tests verify that when a property or method return type carries
// generic parameters (e.g. `Collection<int, User>`), the generic context
// is preserved and applied so that further chaining resolves correctly.
// This was previously broken because `clean_type` stripped `<…>` generics.

/// Test: a property typed as `Collection<int, User>` via `@var` docblock
/// should resolve to `Collection` with `TValue → User`, so chaining
/// `->first()->` offers `User`'s methods.
#[tokio::test]
async fn test_generic_property_type_preserves_context() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_property_context.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "    /** @return TValue[] */\n",
        "    public function all() {}\n",
        "}\n",
        "\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "\n",
        "class UserRepository {\n",
        "    /** @var Collection<int, User> */\n",
        "    public $users;\n",
        "\n",
        "    function test() {\n",
        "        $this->users->first()->\n",
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
                line: 22,
                character: 37,
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
                "Should resolve Collection<int, User>::first() → User and show 'getName', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getEmail"),
                "Should resolve Collection<int, User>::first() → User and show 'getEmail', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: a method with `@return Collection<int, User>` preserves the
/// generic context so that chaining through the return type works.
#[tokio::test]
async fn test_generic_return_type_preserves_context() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_return_context.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "}\n",
        "\n",
        "class Product {\n",
        "    public function getPrice(): float {}\n",
        "    public function getSku(): string {}\n",
        "}\n",
        "\n",
        "class Catalog {\n",
        "    /** @return Collection<int, Product> */\n",
        "    public function getProducts() {}\n",
        "\n",
        "    function test() {\n",
        "        $this->getProducts()->first()->\n",
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
                line: 20,
                character: 45,
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
                "Should resolve Collection<int, Product>::first() → Product and show 'getPrice', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getSku"),
                "Should resolve Collection<int, Product>::first() → Product and show 'getSku', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: a variable assigned from a method returning a generic type
/// preserves generic context for further chaining.
#[tokio::test]
async fn test_generic_context_through_variable_assignment() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_var_assign.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Box {\n",
        "    /** @return T */\n",
        "    public function unwrap() {}\n",
        "}\n",
        "\n",
        "class Gift {\n",
        "    public function open(): string {}\n",
        "}\n",
        "\n",
        "class Store {\n",
        "    /** @return Box<Gift> */\n",
        "    public function getGiftBox() {}\n",
        "\n",
        "    function test() {\n",
        "        $box = $this->getGiftBox();\n",
        "        $box->unwrap()->\n",
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
                character: 28,
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
                method_names.contains(&"open"),
                "Should resolve Box<Gift>::unwrap() → Gift and show 'open', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: generic property type via `@var` with a single template param
/// resolves correctly.
#[tokio::test]
async fn test_generic_property_single_template_param() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_prop_single.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Container {\n",
        "    /** @return T */\n",
        "    public function get() {}\n",
        "}\n",
        "\n",
        "class Config {\n",
        "    public function has(string $key): bool {}\n",
        "    public function all(): array {}\n",
        "}\n",
        "\n",
        "class App {\n",
        "    /** @var Container<Config> */\n",
        "    public $config;\n",
        "\n",
        "    function test() {\n",
        "        $this->config->get()->\n",
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
                character: 34,
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
                method_names.contains(&"has"),
                "Should resolve Container<Config>::get() → Config and show 'has', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"all"),
                "Should resolve Container<Config>::get() → Config and show 'all', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: generic context preserved across PSR-4 file boundaries.
#[tokio::test]
async fn test_generic_property_context_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let collection_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "    /** @return static */\n",
        "    public function filter(callable $fn) {}\n",
        "}\n",
    );

    let user_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "    public function getAge(): int {}\n",
        "}\n",
    );

    let service_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class UserService {\n",
        "    /** @var Collection<int, User> */\n",
        "    public $users;\n",
        "\n",
        "    function test() {\n",
        "        $this->users->first()->\n",
        "    }\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/Collection.php", collection_php),
            ("src/User.php", user_php),
            ("src/UserService.php", service_php),
        ],
    );

    let uri = Url::parse("file:///src/UserService.php").unwrap();
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: service_php.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 37,
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
                "Cross-file: should resolve Collection<int, User>::first() → User, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getAge"),
                "Cross-file: should resolve Collection<int, User>::first() → User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: generic type with nullable union in `@var` (e.g. `Collection<int, User>|null`)
/// preserves generic context after stripping `|null`.
#[tokio::test]
async fn test_generic_nullable_union_preserves_context() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_nullable_union.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Optional {\n",
        "    /** @return T */\n",
        "    public function get() {}\n",
        "}\n",
        "\n",
        "class Session {\n",
        "    public function getId(): string {}\n",
        "}\n",
        "\n",
        "class Handler {\n",
        "    /** @var Optional<Session>|null */\n",
        "    public $session;\n",
        "\n",
        "    function test() {\n",
        "        $this->session->get()->\n",
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
                method_names.contains(&"getId"),
                "Should resolve Optional<Session>::get() → Session through nullable union, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: generic context works when the type has no template params
/// (plain class name in `@return` still works as before).
#[tokio::test]
async fn test_non_generic_return_type_still_works() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///non_generic_still_works.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(string $msg): void {}\n",
        "    public function error(string $msg): void {}\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    /** @return Logger */\n",
        "    public function getLogger() {}\n",
        "\n",
        "    function test() {\n",
        "        $this->getLogger()->\n",
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
                line: 11,
                character: 32,
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
                "Plain @return Logger should still work, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"error"),
                "Plain @return Logger should still work, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: generic context is preserved when `@extends` and inline generic
/// `@var` are both in play — they should not conflict.
#[tokio::test]
async fn test_generic_extends_and_inline_var_coexist() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_extends_and_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class BaseRepo {\n",
        "    /** @return T */\n",
        "    public function find(int $id) {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "}\n",
        "\n",
        "class Order {\n",
        "    public function getTotal(): float {}\n",
        "}\n",
        "\n",
        "class Item {\n",
        "    public function getQuantity(): int {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @extends BaseRepo<Order>\n",
        " */\n",
        "class OrderRepo extends BaseRepo {\n",
        "    /** @var Collection<int, Item> */\n",
        "    public $lineItems;\n",
        "\n",
        "    function testFind() {\n",
        "        $this->find(1)->\n",
        "    }\n",
        "\n",
        "    function testLineItems() {\n",
        "        $this->lineItems->first()->\n",
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

    // Test 1: @extends substitution still works (find() → Order)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 34,
                character: 29,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for find()"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getTotal"),
                "@extends BaseRepo<Order>: find() should return Order, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }

    // Test 2: inline @var generic also works (lineItems->first() → Item)
    let completion_params2 = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 38,
                character: 42,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result2 = backend.completion(completion_params2).await.unwrap();
    assert!(
        result2.is_some(),
        "Completion should return results for lineItems->first()"
    );

    match result2.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getQuantity"),
                "@var Collection<int, Item>: first() should return Item, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: directly accessing members on a property typed as a generic class
/// (e.g. `$this->collection->`) still offers the collection's own methods.
#[tokio::test]
async fn test_generic_property_own_methods_still_visible() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_prop_own_methods.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first() {}\n",
        "    public function count(): int {}\n",
        "    public function isEmpty(): bool {}\n",
        "}\n",
        "\n",
        "class Tag {\n",
        "    public function getLabel(): string {}\n",
        "}\n",
        "\n",
        "class Article {\n",
        "    /** @var Collection<int, Tag> */\n",
        "    public $tags;\n",
        "\n",
        "    function test() {\n",
        "        $this->tags->\n",
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
                line: 21,
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
                method_names.contains(&"first"),
                "Collection's own 'first' method should be visible, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"count"),
                "Collection's own 'count' method should be visible, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"isEmpty"),
                "Collection's own 'isEmpty' method should be visible, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test: `$box = $this->giftBox` where the property has `@var Box<Gift>`
/// should resolve `$box` to `Box<Gift>`, so `$box->unwrap()->` offers
/// `Gift`'s methods.
#[tokio::test]
async fn test_generic_property_assignment_to_variable() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///generic_prop_assign.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "class Box {\n",
        "    /** @return T */\n",
        "    public function unwrap() {}\n",
        "}\n",
        "\n",
        "class Gift {\n",
        "    public function open(): string {}\n",
        "    public function getTag(): string {}\n",
        "}\n",
        "\n",
        "class GiftShop {\n",
        "    /** @var Box<Gift> */\n",
        "    public $giftBox;\n",
        "\n",
        "    function test() {\n",
        "        $box = $this->giftBox;\n",
        "        $box->unwrap()->\n",
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
                line: 20,
                character: 28,
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
                method_names.contains(&"open"),
                "Should resolve $box = $this->giftBox (Box<Gift>), unwrap() → Gift, show 'open', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTag"),
                "Should resolve $box = $this->giftBox (Box<Gift>), unwrap() → Gift, show 'getTag', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Trait generic substitution (@use) tests ────────────────────────────────
//
// These tests verify that `@template` parameters declared on a trait are
// correctly substituted with concrete types when a class uses
// `@use TraitName<ConcreteType>` in its docblock.

/// Basic test: a class uses a generic trait with a concrete type.
/// Methods inherited from the trait should have their template parameter
/// return types resolved to the concrete types.
#[tokio::test]
async fn test_trait_use_generic_resolves_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_use_generic_basic.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TFactory\n",
        " */\n",
        "trait HasFactory {\n",
        "    /** @return TFactory */\n",
        "    public static function factory() {}\n",
        "}\n",
        "\n",
        "class UserFactory {\n",
        "    public function create(): void {}\n",
        "    public function count(int $n): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @use HasFactory<UserFactory>\n",
        " */\n",
        "class User {\n",
        "    use HasFactory;\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    User::factory()->\n",
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
                line: 22,
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
                method_names.contains(&"create"),
                "Should resolve TFactory to UserFactory and show 'create', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"count"),
                "Should resolve TFactory to UserFactory and show 'count', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test with two template parameters on a trait.
#[tokio::test]
async fn test_trait_use_generic_two_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_use_generic_two_params.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " */\n",
        "trait Indexable {\n",
        "    /** @return TValue */\n",
        "    public function get() {}\n",
        "    /** @return TKey */\n",
        "    public function key() {}\n",
        "}\n",
        "\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @use Indexable<int, User>\n",
        " */\n",
        "class UserList {\n",
        "    use Indexable;\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $list = new UserList();\n",
        "    $list->get()->\n",
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
                method_names.contains(&"getName"),
                "Should resolve TValue to User and show User's 'getName', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that trait property types are also substituted via @use generics.
#[tokio::test]
async fn test_trait_use_generic_property_substitution() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_use_generic_property.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TModel\n",
        " */\n",
        "trait HasRelation {\n",
        "    /** @var TModel */\n",
        "    public $related;\n",
        "}\n",
        "\n",
        "class Address {\n",
        "    public function getCity(): string {}\n",
        "    public function getZip(): string {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @use HasRelation<Address>\n",
        " */\n",
        "class User {\n",
        "    use HasRelation;\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $user = new User();\n",
        "    $user->related->\n",
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
                method_names.contains(&"getCity"),
                "Should resolve TModel to Address and show 'getCity', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getZip"),
                "Should resolve TModel to Address and show 'getZip', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that @phpstan-use variant is also accepted.
#[tokio::test]
async fn test_trait_phpstan_use_variant() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_phpstan_use.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "trait Wrapper {\n",
        "    /** @return T */\n",
        "    public function unwrap() {}\n",
        "}\n",
        "\n",
        "class Gift {\n",
        "    public function open(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @phpstan-use Wrapper<Gift>\n",
        " */\n",
        "class GiftBox {\n",
        "    use Wrapper;\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $box = new GiftBox();\n",
        "    $box->unwrap()->\n",
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
                line: 22,
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
                method_names.contains(&"open"),
                "Should resolve T to Gift via @phpstan-use and show 'open', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that class own members take precedence over substituted trait members.
#[tokio::test]
async fn test_trait_use_generic_class_own_wins() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_use_generic_precedence.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "trait Getter {\n",
        "    /** @return T */\n",
        "    public function get() {}\n",
        "    /** @return T */\n",
        "    public function first() {}\n",
        "}\n",
        "\n",
        "class Apple {\n",
        "    public function bite(): void {}\n",
        "}\n",
        "\n",
        "class Orange {\n",
        "    public function squeeze(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @use Getter<Apple>\n",
        " */\n",
        "class FruitBowl {\n",
        "    use Getter;\n",
        "    /** @return Orange */\n",
        "    public function get() {}\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $bowl = new FruitBowl();\n",
        "    $bowl->get()->\n",
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
                line: 30,
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

            // Class own get() returns Orange, so we should see Orange's methods
            assert!(
                method_names.contains(&"squeeze"),
                "Class own 'get' should override trait's, returning Orange with 'squeeze', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bite"),
                "Apple's 'bite' should NOT appear since class own 'get' returns Orange, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test trait generic substitution works across files via PSR-4.
#[tokio::test]
async fn test_trait_use_generic_cross_file_psr4() {
    let composer_json = r#"{ "autoload": { "psr-4": { "App\\": "src/" } } }"#;

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            (
                "src/Concerns/HasFactory.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Concerns;\n",
                    "\n",
                    "/**\n",
                    " * @template TFactory\n",
                    " */\n",
                    "trait HasFactory {\n",
                    "    /** @return TFactory */\n",
                    "    public static function factory() {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Factories/UserFactory.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Factories;\n",
                    "\n",
                    "class UserFactory {\n",
                    "    public function create(): void {}\n",
                    "    public function make(): void {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Models/User.php",
                concat!(
                    "<?php\n",
                    "namespace App\\Models;\n",
                    "\n",
                    "use App\\Concerns\\HasFactory;\n",
                    "use App\\Factories\\UserFactory;\n",
                    "\n",
                    "/**\n",
                    " * @use HasFactory<UserFactory>\n",
                    " */\n",
                    "class User {\n",
                    "    use HasFactory;\n",
                    "}\n",
                ),
            ),
        ],
    );

    // Open a file that uses User::factory()
    let uri = Url::parse("file:///test_trait_cross_file.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Models\\User;\n",
        "\n",
        "function test() {\n",
        "    User::factory()->\n",
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
                line: 4,
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
                method_names.contains(&"create"),
                "Should resolve TFactory to UserFactory across files and show 'create', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"make"),
                "Should resolve TFactory to UserFactory across files and show 'make', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that without @use generics, template params remain unresolved
/// (no crash, methods still available but return types are raw template names).
#[tokio::test]
async fn test_trait_use_without_generics_no_crash() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_no_use_generics.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template T\n",
        " */\n",
        "trait Wrapper {\n",
        "    /** @return T */\n",
        "    public function unwrap() {}\n",
        "    public function isEmpty(): bool {}\n",
        "}\n",
        "\n",
        "class Box {\n",
        "    use Wrapper;\n",
        "}\n",
        "\n",
        "function test() {\n",
        "    $box = new Box();\n",
        "    $box->\n",
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
                line: 16,
                character: 10,
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

            // Methods from the trait should still be visible even without @use generics
            assert!(
                method_names.contains(&"unwrap"),
                "Trait method 'unwrap' should be visible even without @use generics, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"isEmpty"),
                "Trait method 'isEmpty' should be visible, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test @use generics with $this-> context inside the class.
#[tokio::test]
async fn test_trait_use_generic_this_context() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///trait_use_generic_this.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @template TFactory\n",
        " */\n",
        "trait HasFactory {\n",
        "    /** @return TFactory */\n",
        "    public function getFactory() {}\n",
        "}\n",
        "\n",
        "class UserFactory {\n",
        "    public function create(): void {}\n",
        "}\n",
        "\n",
        "/**\n",
        " * @use HasFactory<UserFactory>\n",
        " */\n",
        "class User {\n",
        "    use HasFactory;\n",
        "\n",
        "    public function test() {\n",
        "        $this->getFactory()->\n",
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
                line: 20,
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
                method_names.contains(&"create"),
                "Should resolve TFactory to UserFactory via $this-> and show 'create', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Test that `extract_generics_tag` correctly parses @use tags.
#[test]
fn test_extract_generics_tag_use_basic() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @use HasFactory<UserFactory>\n */";
    let result = extract_generics_tag(docblock, "@use");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "HasFactory");
    assert_eq!(result[0].1, vec!["UserFactory"]);
}

/// Test that @phpstan-use variant is parsed by extract_generics_tag.
#[test]
fn test_extract_generics_tag_phpstan_use() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @phpstan-use HasFactory<UserFactory>\n */";
    let result = extract_generics_tag(docblock, "@use");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "HasFactory");
    assert_eq!(result[0].1, vec!["UserFactory"]);
}

/// Test @use with multiple template parameters.
#[test]
fn test_extract_generics_tag_use_multiple_params() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @use Indexable<int, User>\n */";
    let result = extract_generics_tag(docblock, "@use");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "Indexable");
    assert_eq!(result[0].1, vec!["int", "User"]);
}

/// Test @use with fully-qualified trait name.
#[test]
fn test_extract_generics_tag_use_fqn() {
    use phpantom_lsp::docblock::extract_generics_tag;

    let docblock = "/**\n * @use \\App\\Concerns\\HasFactory<\\App\\Factories\\UserFactory>\n */";
    let result = extract_generics_tag(docblock, "@use");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, "App\\Concerns\\HasFactory");
    assert_eq!(result[0].1, vec!["App\\Factories\\UserFactory"]);
}

// ─── Method-level @template general case tests ──────────────────────────────
//
// These tests verify that method-level @template parameters are correctly
// substituted with concrete types resolved from call-site arguments, beyond
// the special-cased `class-string<T>` pattern.

// ── Unit tests for extract_template_param_bindings ──────────────────────────

/// Basic binding: `@template T` + `@param T $model` → `[("T", "$model")]`.
#[test]
fn test_extract_template_param_bindings_basic() {
    use phpantom_lsp::docblock::extract_template_param_bindings;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param T $model\n",
        " * @return Collection<T>\n",
        " */",
    );
    let tpl_params = vec!["T".to_string()];
    let result = extract_template_param_bindings(docblock, &tpl_params);
    assert_eq!(result, vec![("T".to_string(), "$model".to_string())]);
}

/// Nullable param type: `@param ?T $model` should still bind T.
#[test]
fn test_extract_template_param_bindings_nullable() {
    use phpantom_lsp::docblock::extract_template_param_bindings;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param ?T $model\n",
        " * @return T\n",
        " */",
    );
    let tpl_params = vec!["T".to_string()];
    let result = extract_template_param_bindings(docblock, &tpl_params);
    assert_eq!(result, vec![("T".to_string(), "$model".to_string())]);
}

/// Union-with-null param type: `@param T|null $model` should bind T.
#[test]
fn test_extract_template_param_bindings_union_null() {
    use phpantom_lsp::docblock::extract_template_param_bindings;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param T|null $model\n",
        " * @return T\n",
        " */",
    );
    let tpl_params = vec!["T".to_string()];
    let result = extract_template_param_bindings(docblock, &tpl_params);
    assert_eq!(result, vec![("T".to_string(), "$model".to_string())]);
}

/// Multiple template params with separate bindings.
#[test]
fn test_extract_template_param_bindings_multiple() {
    use phpantom_lsp::docblock::extract_template_param_bindings;

    let docblock = concat!(
        "/**\n",
        " * @template TKey\n",
        " * @template TValue\n",
        " * @param TKey $key\n",
        " * @param TValue $value\n",
        " * @return Pair<TKey, TValue>\n",
        " */",
    );
    let tpl_params = vec!["TKey".to_string(), "TValue".to_string()];
    let result = extract_template_param_bindings(docblock, &tpl_params);
    assert_eq!(
        result,
        vec![
            ("TKey".to_string(), "$key".to_string()),
            ("TValue".to_string(), "$value".to_string()),
        ]
    );
}

/// Non-template param type is not bound.
#[test]
fn test_extract_template_param_bindings_non_template_ignored() {
    use phpantom_lsp::docblock::extract_template_param_bindings;

    let docblock = concat!(
        "/**\n",
        " * @template T\n",
        " * @param string $name\n",
        " * @param T $model\n",
        " * @return T\n",
        " */",
    );
    let tpl_params = vec!["T".to_string()];
    let result = extract_template_param_bindings(docblock, &tpl_params);
    assert_eq!(result, vec![("T".to_string(), "$model".to_string())]);
}

/// Empty template params → no bindings.
#[test]
fn test_extract_template_param_bindings_empty_templates() {
    use phpantom_lsp::docblock::extract_template_param_bindings;

    let docblock = concat!("/**\n", " * @param string $name\n", " */",);
    let tpl_params: Vec<String> = vec![];
    let result = extract_template_param_bindings(docblock, &tpl_params);
    assert!(result.is_empty());
}

// ── Integration tests: inline chain with general @template ──────────────────

/// `@template T` + `@param T $model` + `@return Collection<T>`:
/// `$repo->wrap($user)->` should show Collection<User> methods.
#[tokio::test]
async fn test_method_template_general_inline_chain() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_inline.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "\n",
        "/** @template TValue */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first(): mixed {}\n",
        "}\n",
        "\n",
        "class Repository {\n",
        "    /**\n",
        "     * @template T of object\n",
        "     * @param T $model\n",
        "     * @return Collection<T>\n",
        "     */\n",
        "    public function wrap(object $model): Collection {}\n",
        "}\n",
        "\n",
        "function test(Repository $repo, User $user) {\n",
        "    $repo->wrap($user)->first()->\n",
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
                line: 22,
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
                method_names.contains(&"getName"),
                "Should resolve T→User via Collection<T>.first()→User, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getEmail"),
                "Should resolve T→User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// `$this->wrap($user)->` inside a class — template resolves via $this context.
#[tokio::test]
async fn test_method_template_general_this_context() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_this.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Product {\n",
        "    public function getPrice(): float {}\n",
        "    public function getTitle(): string {}\n",
        "}\n",
        "\n",
        "/** @template TValue */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first(): mixed {}\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $item\n",
        "     * @return Collection<T>\n",
        "     */\n",
        "    public function collect(mixed $item): Collection {}\n",
        "\n",
        "    public function run(Product $product) {\n",
        "        $this->collect($product)->first()->\n",
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
                line: 21,
                character: 46,
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
                "Should resolve T→Product, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getTitle"),
                "Should resolve T→Product, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Assignment case: `$collection = $repo->wrap($user); $collection->first()->`
/// should resolve through the template.
#[tokio::test]
async fn test_method_template_general_assignment() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_assign.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "/** @template TValue */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first(): mixed {}\n",
        "}\n",
        "\n",
        "class Repository {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $model\n",
        "     * @return Collection<T>\n",
        "     */\n",
        "    public function wrap(object $model): Collection {}\n",
        "}\n",
        "\n",
        "function test(Repository $repo, User $user) {\n",
        "    $collection = $repo->wrap($user);\n",
        "    $collection->first()->\n",
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
                line: 22,
                character: 26,
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
                "Should resolve T→User through assignment, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Static method with general @template: `Repository::wrap($user)->`
#[tokio::test]
async fn test_method_template_general_static_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "/** @template TValue */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first(): mixed {}\n",
        "}\n",
        "\n",
        "class Repository {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $model\n",
        "     * @return Collection<T>\n",
        "     */\n",
        "    public static function wrap(object $model): Collection {}\n",
        "}\n",
        "\n",
        "function test(User $user) {\n",
        "    Repository::wrap($user)->first()->\n",
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
                method_names.contains(&"getName"),
                "Static method @template T + @param T should resolve T→User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Direct return of template param: `@template T` + `@param T $model` + `@return T`
/// `$repo->identity($user)->` should resolve to User.
#[tokio::test]
async fn test_method_template_general_direct_return() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_direct.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "class Util {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $item\n",
        "     * @return T\n",
        "     */\n",
        "    public function identity(mixed $item): mixed {}\n",
        "}\n",
        "\n",
        "function test(Util $util, User $user) {\n",
        "    $util->identity($user)->\n",
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
                line: 15,
                character: 29,
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
                "Direct @return T with @param T should resolve T→User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Multiple template params: `@template TKey` + `@template TValue`
/// with separate param bindings.
#[tokio::test]
async fn test_method_template_general_multiple_params() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_multi.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Category {\n",
        "    public function getLabel(): string {}\n",
        "}\n",
        "\n",
        "class Product {\n",
        "    public function getPrice(): float {}\n",
        "}\n",
        "\n",
        "/** @template TValue */\n",
        "class Wrapper {\n",
        "    /** @return TValue */\n",
        "    public function unwrap(): mixed {}\n",
        "}\n",
        "\n",
        "class Factory {\n",
        "    /**\n",
        "     * @template TKey\n",
        "     * @template TValue\n",
        "     * @param TKey $key\n",
        "     * @param TValue $value\n",
        "     * @return Wrapper<TValue>\n",
        "     */\n",
        "    public function make(mixed $key, mixed $value): Wrapper {}\n",
        "}\n",
        "\n",
        "function test(Factory $f, Category $cat, Product $prod) {\n",
        "    $f->make($cat, $prod)->unwrap()->\n",
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
                line: 27,
                character: 38,
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
                "TValue should resolve to Product (second arg), got: {:?}",
                method_names
            );
            // Category methods should NOT appear because the return uses TValue not TKey.
            assert!(
                !method_names.contains(&"getLabel"),
                "TKey (Category) should not leak into TValue resolution, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Existing class-string<T> conditional should still work (regression guard).
/// This verifies the general template path doesn't break the existing
/// synthesized conditional path.
#[tokio::test]
async fn test_method_template_general_does_not_break_class_string() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_regression.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "class Repository {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param class-string<T> $class\n",
        "     * @return T\n",
        "     */\n",
        "    public function find(string $class): object {}\n",
        "}\n",
        "\n",
        "function test(Repository $repo) {\n",
        "    $repo->find(User::class)->\n",
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
                line: 15,
                character: 30,
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
                "class-string<T> path should still work, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// $this->method($arg) inside a class body via assignment.
/// Verifies the AST-based resolve_rhs_expression path.
#[tokio::test]
async fn test_method_template_general_this_assignment() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_this_assign.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "/** @template TValue */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first(): mixed {}\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $item\n",
        "     * @return Collection<T>\n",
        "     */\n",
        "    public function wrap(mixed $item): Collection {}\n",
        "\n",
        "    public function run(User $user) {\n",
        "        $result = $this->wrap($user);\n",
        "        $result->first()->\n",
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
                line: 21,
                character: 26,
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
                "$this->wrap($user) assignment should resolve T→User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Cross-file PSR-4: method-level @template resolves across files.
/// Tests `$repo->wrap($user)->` resolves to Collection<User> and shows
/// Collection's own methods (like `first`).
#[tokio::test]
async fn test_method_template_general_cross_file_psr4() {
    let composer_json = r#"{
        "autoload": {
            "psr-4": {
                "App\\": "src/"
            }
        }
    }"#;

    let user_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "    public function getEmail(): string {}\n",
        "}\n",
    );

    let coll_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "/** @template TValue */\n",
        "class Collection {\n",
        "    /** @return TValue */\n",
        "    public function first(): mixed {}\n",
        "    public function count(): int {}\n",
        "}\n",
    );

    let repo_php = concat!(
        "<?php\n",
        "namespace App;\n",
        "\n",
        "class Repository {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $model\n",
        "     * @return Collection<T>\n",
        "     */\n",
        "    public function wrap(object $model): Collection {}\n",
        "}\n",
    );

    let (backend, _dir) = create_psr4_workspace(
        composer_json,
        &[
            ("src/User.php", user_php),
            ("src/Collection.php", coll_php),
            ("src/Repository.php", repo_php),
        ],
    );

    // Open a consumer file that uses the template method.
    // Test the simpler chain `$repo->wrap($user)->` which should
    // resolve to Collection<User> and show Collection's methods.
    let test_text = concat!(
        "<?php\n",                                                      // 0
        "namespace App;\n",                                             // 1
        "\n",                                                           // 2
        "use App\\Repository;\n",                                       // 3
        "use App\\User;\n",                                             // 4
        "\n",                                                           // 5
        "class TestConsumer {\n",                                       // 6
        "    public function handle(Repository $repo, User $user) {\n", // 7
        "        $repo->wrap($user)->\n",                               // 8
        "    }\n",                                                      // 9
        "}\n",                                                          // 10
    );

    let uri = Url::parse("file:///test_cross_tpl.php").unwrap();
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: test_text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 29,
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
                "Cross-file @template T + @param T should resolve to Collection<User> showing 'first', got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"count"),
                "Cross-file should also show Collection's 'count', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Method-level template with `new ClassName()` as argument:
/// `$util->wrap(new User())->` should resolve T to User.
#[tokio::test]
async fn test_method_template_general_new_expression_arg() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_new_arg.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "class Util {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $item\n",
        "     * @return T\n",
        "     */\n",
        "    public function identity(mixed $item): mixed {}\n",
        "}\n",
        "\n",
        "function test(Util $util) {\n",
        "    $util->identity(new User())->\n",
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
                line: 15,
                character: 34,
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
                "`new User()` arg should resolve T→User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Method with @phpstan-template variant (common in PHPStan stubs).
#[tokio::test]
async fn test_method_template_general_phpstan_prefix() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_phpstan.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "class Util {\n",
        "    /**\n",
        "     * @phpstan-template T\n",
        "     * @param T $item\n",
        "     * @return T\n",
        "     */\n",
        "    public function identity(mixed $item): mixed {}\n",
        "}\n",
        "\n",
        "function test(Util $util, User $user) {\n",
        "    $util->identity($user)->\n",
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
                line: 15,
                character: 29,
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
                "@phpstan-template variant should resolve T→User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Non-template method should still resolve normally (no regression).
#[tokio::test]
async fn test_method_template_general_non_template_unaffected() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_no_tpl.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "class Service {\n",
        "    public function getUser(): User {}\n",
        "}\n",
        "\n",
        "function test(Service $svc) {\n",
        "    $svc->getUser()->\n",
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
                line: 10,
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
                "Non-template method should still work, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Argument with `$this->property` resolves through template.
#[tokio::test]
async fn test_method_template_general_this_property_arg() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_general_prop_arg.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getName(): string {}\n",
        "}\n",
        "\n",
        "class Util {\n",
        "    /**\n",
        "     * @template T\n",
        "     * @param T $item\n",
        "     * @return T\n",
        "     */\n",
        "    public function identity(mixed $item): mixed {}\n",
        "}\n",
        "\n",
        "class Controller {\n",
        "    private User $user;\n",
        "\n",
        "    public function handle(Util $util) {\n",
        "        $util->identity($this->user)->\n",
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
                method_names.contains(&"getName"),
                "$this->property arg should resolve T→User, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// 3-level chain with `new ClassName()` argument through template:
/// `$mapper->wrap(new Product())->first()->` should resolve to Product.
#[tokio::test]
async fn test_method_template_general_deep_chain_new_arg() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_deep_chain.php").unwrap();
    let text = concat!(
        "<?php\n",                                                      // 0
        "class Product {\n",                                            // 1
        "    public function getPrice(): float {}\n",                   // 2
        "}\n",                                                          // 3
        "\n",                                                           // 4
        "/** @template TValue */\n",                                    // 5
        "class TypedCollection {\n",                                    // 6
        "    /** @return TValue */\n",                                  // 7
        "    public function first(): mixed {}\n",                      // 8
        "}\n",                                                          // 9
        "\n",                                                           // 10
        "class ObjectMapper {\n",                                       // 11
        "    /**\n",                                                    // 12
        "     * @template T\n",                                         // 13
        "     * @param T $item\n",                                      // 14
        "     * @return TypedCollection<T>\n",                          // 15
        "     */\n",                                                    // 16
        "    public function wrap(object $item): TypedCollection {}\n", // 17
        "}\n",                                                          // 18
        "\n",                                                           // 19
        "function test() {\n",                                          // 20
        "    $mapper = new ObjectMapper();\n",                          // 21
        "    $mapper->wrap(new Product())->first()->\n",                // 22
        "}\n",                                                          // 23
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
                line: 22,
                character: 47,
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
                "3-level chain: wrap(new Product())->first()-> should resolve to Product, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Same 3-level chain but inside a namespace (reproduces example.php failure).
/// The namespace causes class names to be FQ-resolved, which can break
/// the substitution path if short names don't match after resolution.
#[tokio::test]
async fn test_method_template_general_deep_chain_namespaced() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///method_tpl_deep_chain_ns.php").unwrap();
    let text = concat!(
        "<?php\n",                                                      // 0
        "namespace App;\n",                                             // 1
        "\n",                                                           // 2
        "class Product {\n",                                            // 3
        "    public function getPrice(): float {}\n",                   // 4
        "}\n",                                                          // 5
        "\n",                                                           // 6
        "/** @template TValue */\n",                                    // 7
        "class TypedCollection {\n",                                    // 8
        "    /** @return TValue */\n",                                  // 9
        "    public function first(): mixed {}\n",                      // 10
        "}\n",                                                          // 11
        "\n",                                                           // 12
        "class ObjectMapper {\n",                                       // 13
        "    /**\n",                                                    // 14
        "     * @template T\n",                                         // 15
        "     * @param T $item\n",                                      // 16
        "     * @return TypedCollection<T>\n",                          // 17
        "     */\n",                                                    // 18
        "    public function wrap(object $item): TypedCollection {}\n", // 19
        "}\n",                                                          // 20
        "\n",                                                           // 21
        "function test() {\n",                                          // 22
        "    $mapper = new ObjectMapper();\n",                          // 23
        "    $mapper->wrap(new Product())->first()->\n",                // 24
        "}\n",                                                          // 25
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
                character: 47,
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
                "Namespaced 3-level chain: wrap(new Product())->first()-> should resolve to Product, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When a variable is assigned a `::class` value through a `match` expression
/// and then passed to a method with `@template T` + `@param class-string<T>`
/// + `@return T`, the resolver should trace the class-string back through the
/// match arms and produce a union of all possible return types.
#[tokio::test]
async fn test_match_class_string_forwarded_to_method_conditional_return() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///match_class_string_method.php").unwrap();
    let text = concat!(
        "<?php\n",                                                      // 0
        "class GetCreditnotesRequest {\n",                              // 1
        "    public function getCreditnotes(): array {}\n",             // 2
        "}\n",                                                          // 3
        "\n",                                                           // 4
        "class GetOrdersRequest {\n",                                   // 5
        "    public function getOrders(): array {}\n",                  // 6
        "}\n",                                                          // 7
        "\n",                                                           // 8
        "class Container {\n",                                          // 9
        "    /**\n",                                                    // 10
        "     * @template T\n",                                         // 11
        "     * @param class-string<T> $abstract\n",                    // 12
        "     * @return T\n",                                           // 13
        "     */\n",                                                    // 14
        "    public function make(string $abstract): object {}\n",      // 15
        "}\n",                                                          // 16
        "\n",                                                           // 17
        "class App {\n",                                                // 18
        "    public function run(string $typeName): void {\n",          // 19
        "        $container = new Container();\n",                      // 20
        "        $requestType = match ($typeName) {\n",                 // 21
        "            'creditnotes' => GetCreditnotesRequest::class,\n", // 22
        "            'orders'      => GetOrdersRequest::class,\n",      // 23
        "        };\n",                                                 // 24
        "        $requestBody = $container->make($requestType);\n",     // 25
        "        $requestBody->\n",                                     // 26
        "    }\n",                                                      // 27
        "}\n",                                                          // 28
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 26,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $requestBody->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getCreditnotes"),
                "Should include getCreditnotes from GetCreditnotesRequest, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getOrders"),
                "Should include getOrders from GetOrdersRequest, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Same as above but using a standalone function instead of a method.
/// `$result = resolve($matchVar)` where `resolve` has `@template T` +
/// `@param class-string<T>` + `@return T`.
#[tokio::test]
async fn test_match_class_string_forwarded_to_function_conditional_return() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///match_class_string_func.php").unwrap();
    let text = concat!(
        "<?php\n",                                         // 0
        "class Alpha {\n",                                 // 1
        "    public function alphaMethod(): void {}\n",    // 2
        "}\n",                                             // 3
        "\n",                                              // 4
        "class Beta {\n",                                  // 5
        "    public function betaMethod(): void {}\n",     // 6
        "}\n",                                             // 7
        "\n",                                              // 8
        "/**\n",                                           // 9
        " * @template T\n",                                // 10
        " * @param class-string<T> $class\n",              // 11
        " * @return T\n",                                  // 12
        " */\n",                                           // 13
        "function resolve(string $class): object {}\n",    // 14
        "\n",                                              // 15
        "class Service {\n",                               // 16
        "    public function handle(int $type): void {\n", // 17
        "        $cls = match ($type) {\n",                // 18
        "            1 => Alpha::class,\n",                // 19
        "            2 => Beta::class,\n",                 // 20
        "        };\n",                                    // 21
        "        $obj = resolve($cls);\n",                 // 22
        "        $obj->\n",                                // 23
        "    }\n",                                         // 24
        "}\n",                                             // 25
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 23,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $obj->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"alphaMethod"),
                "Should include alphaMethod from Alpha, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"betaMethod"),
                "Should include betaMethod from Beta, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// A simple `$cls = User::class` (not a match) forwarded to a method with
/// `@template T` + `@param class-string<T>` + `@return T` should also work.
#[tokio::test]
async fn test_simple_class_string_variable_forwarded_to_conditional_return() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///simple_class_string_var.php").unwrap();
    let text = concat!(
        "<?php\n",                                              // 0
        "class User {\n",                                       // 1
        "    public function getName(): string {}\n",           // 2
        "}\n",                                                  // 3
        "\n",                                                   // 4
        "class Repository {\n",                                 // 5
        "    /**\n",                                            // 6
        "     * @template T\n",                                 // 7
        "     * @param class-string<T> $class\n",               // 8
        "     * @return T\n",                                   // 9
        "     */\n",                                            // 10
        "    public function find(string $class): object {}\n", // 11
        "}\n",                                                  // 12
        "\n",                                                   // 13
        "class Service {\n",                                    // 14
        "    public function handle(): void {\n",               // 15
        "        $repo = new Repository();\n",                  // 16
        "        $cls = User::class;\n",                        // 17
        "        $user = $repo->find($cls);\n",                 // 18
        "        $user->\n",                                    // 19
        "    }\n",                                              // 20
        "}\n",                                                  // 21
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

    let params = CompletionParams {
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

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $user->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getName"),
                "Should resolve $cls = User::class through variable and show getName, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Ternary expression assigning class-string values to a variable, then
/// forwarded to a conditional return type method.
#[tokio::test]
async fn test_ternary_class_string_forwarded_to_conditional_return() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///ternary_class_string.php").unwrap();
    let text = concat!(
        "<?php\n",                                                  // 0
        "class Admin {\n",                                          // 1
        "    public function getRole(): string {}\n",               // 2
        "}\n",                                                      // 3
        "\n",                                                       // 4
        "class Guest {\n",                                          // 5
        "    public function getToken(): string {}\n",              // 6
        "}\n",                                                      // 7
        "\n",                                                       // 8
        "class Container {\n",                                      // 9
        "    /**\n",                                                // 10
        "     * @template T\n",                                     // 11
        "     * @param class-string<T> $abstract\n",                // 12
        "     * @return T\n",                                       // 13
        "     */\n",                                                // 14
        "    public function make(string $abstract): object {}\n",  // 15
        "}\n",                                                      // 16
        "\n",                                                       // 17
        "class Handler {\n",                                        // 18
        "    public function handle(bool $isAdmin): void {\n",      // 19
        "        $container = new Container();\n",                  // 20
        "        $cls = $isAdmin ? Admin::class : Guest::class;\n", // 21
        "        $user = $container->make($cls);\n",                // 22
        "        $user->\n",                                        // 23
        "    }\n",                                                  // 24
        "}\n",                                                      // 25
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 23,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $user->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"getRole"),
                "Should include getRole from Admin (ternary true branch), got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"getToken"),
                "Should include getToken from Guest (ternary false branch), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Inline chain: `$container->make($matchVar)->` should also resolve when
/// the argument is a variable holding class-string values from a match.
#[tokio::test]
async fn test_match_class_string_inline_chain() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///match_class_string_inline.php").unwrap();
    let text = concat!(
        "<?php\n",                                                 // 0
        "class Foo {\n",                                           // 1
        "    public function fooMethod(): void {}\n",              // 2
        "}\n",                                                     // 3
        "\n",                                                      // 4
        "class Bar {\n",                                           // 5
        "    public function barMethod(): void {}\n",              // 6
        "}\n",                                                     // 7
        "\n",                                                      // 8
        "class Container {\n",                                     // 9
        "    /**\n",                                               // 10
        "     * @template T\n",                                    // 11
        "     * @param class-string<T> $abstract\n",               // 12
        "     * @return T\n",                                      // 13
        "     */\n",                                               // 14
        "    public function make(string $abstract): object {}\n", // 15
        "}\n",                                                     // 16
        "\n",                                                      // 17
        "class Runner {\n",                                        // 18
        "    public function run(int $which): void {\n",           // 19
        "        $container = new Container();\n",                 // 20
        "        $cls = match ($which) {\n",                       // 21
        "            1 => Foo::class,\n",                          // 22
        "            2 => Bar::class,\n",                          // 23
        "        };\n",                                            // 24
        "        $container->make($cls)->\n",                      // 25
        "    }\n",                                                 // 26
        "}\n",                                                     // 27
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 25,
                character: 38,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for inline chain $container->make($cls)->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"fooMethod"),
                "Should include fooMethod from Foo via inline chain, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"barMethod"),
                "Should include barMethod from Bar via inline chain, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Static method call: `ClassName::create($matchVar)` where `create` has a
/// `@template T` + `@param class-string<T>` + `@return T` conditional.
#[tokio::test]
async fn test_match_class_string_forwarded_to_static_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///match_class_string_static.php").unwrap();
    let text = concat!(
        "<?php\n",                                                       // 0
        "class Widget {\n",                                              // 1
        "    public function render(): string {}\n",                     // 2
        "}\n",                                                           // 3
        "\n",                                                            // 4
        "class Factory {\n",                                             // 5
        "    /**\n",                                                     // 6
        "     * @template T\n",                                          // 7
        "     * @param class-string<T> $class\n",                        // 8
        "     * @return T\n",                                            // 9
        "     */\n",                                                     // 10
        "    public static function create(string $class): object {}\n", // 11
        "}\n",                                                           // 12
        "\n",                                                            // 13
        "class Builder {\n",                                             // 14
        "    public function build(string $name): void {\n",             // 15
        "        $cls = match ($name) {\n",                              // 16
        "            'widget' => Widget::class,\n",                      // 17
        "        };\n",                                                  // 18
        "        $instance = Factory::create($cls);\n",                  // 19
        "        $instance->\n",                                         // 20
        "    }\n",                                                       // 21
        "}\n",                                                           // 22
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

    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 20,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for $instance->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            assert!(
                method_names.contains(&"render"),
                "Should resolve $cls from match through static Factory::create and show render, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
