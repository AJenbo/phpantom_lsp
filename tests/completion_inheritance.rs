mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Inheritance tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_completion_inherits_public_and_protected_methods_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Animal {\n",
        "    public function breathe(): void {}\n",
        "    protected function sleep(): void {}\n",
        "    private function digest(): void {}\n",
        "}\n",
        "class Dog extends Animal {\n",
        "    public function bark(): void {}\n",
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
                line: 9,
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
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Own methods
            assert!(
                method_names.contains(&"bark"),
                "Should include own method 'bark'"
            );
            assert!(
                method_names.contains(&"test"),
                "Should include own method 'test'"
            );

            // Inherited public
            assert!(
                method_names.contains(&"breathe"),
                "Should include inherited public 'breathe'"
            );

            // Inherited protected
            assert!(
                method_names.contains(&"sleep"),
                "Should include inherited protected 'sleep'"
            );

            // Private should NOT be inherited
            assert!(
                !method_names.contains(&"digest"),
                "Should NOT include inherited private 'digest'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inherits_properties_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherit_props.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public string $publicProp;\n",
        "    protected int $protectedProp;\n",
        "    private float $privateProp;\n",
        "}\n",
        "class Child extends Base {\n",
        "    public string $ownProp;\n",
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
                line: 9,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let prop_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.label.as_str())
                .collect();

            assert!(
                prop_names.contains(&"ownProp"),
                "Should include own property"
            );
            assert!(
                prop_names.contains(&"publicProp"),
                "Should include inherited public property"
            );
            assert!(
                prop_names.contains(&"protectedProp"),
                "Should include inherited protected property"
            );
            assert!(
                !prop_names.contains(&"privateProp"),
                "Should NOT include inherited private property"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inherits_constants_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherit_const.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public const PUB_CONST = 1;\n",
        "    protected const PROT_CONST = 2;\n",
        "    private const PRIV_CONST = 3;\n",
        "}\n",
        "class Child extends Base {\n",
        "    const OWN_CONST = 4;\n",
        "    function test() {\n",
        "        self::\n",
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
                line: 9,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let const_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::CONSTANT))
                .map(|i| i.label.as_str())
                .collect();

            assert!(
                const_names.contains(&"OWN_CONST"),
                "Should include own constant"
            );
            assert!(
                const_names.contains(&"PUB_CONST"),
                "Should include inherited public constant"
            );
            assert!(
                const_names.contains(&"PROT_CONST"),
                "Should include inherited protected constant"
            );
            assert!(
                !const_names.contains(&"PRIV_CONST"),
                "Should NOT include inherited private constant"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_child_method_overrides_parent() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///override.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function greet(string $name): string { return ''; }\n",
        "    public function hello(): void {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    public function greet(string $name, string $greeting): string { return ''; }\n",
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
                line: 8,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();

            // 'greet' should appear exactly once (child's version)
            let greet_items: Vec<&&CompletionItem> = method_items
                .iter()
                .filter(|i| i.filter_text.as_deref() == Some("greet"))
                .collect();
            assert_eq!(
                greet_items.len(),
                1,
                "Overridden method 'greet' should appear exactly once"
            );

            // The label should show the child's signature (2 params)
            let greet_label = &greet_items[0].label;
            assert!(
                greet_label.contains("$greeting"),
                "Should use child's signature with $greeting, got: {}",
                greet_label
            );

            // 'hello' should be inherited from parent
            let hello_items: Vec<&&CompletionItem> = method_items
                .iter()
                .filter(|i| i.filter_text.as_deref() == Some("hello"))
                .collect();
            assert_eq!(hello_items.len(), 1, "Inherited 'hello' should appear once");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_multi_level_inheritance_same_file() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///multi_level.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Grandparent {\n",
        "    public function ancestorMethod(): void {}\n",
        "    private function gpPrivate(): void {}\n",
        "}\n",
        "class ParentClass extends Grandparent {\n",
        "    public function parentMethod(): void {}\n",
        "    protected function parentProtected(): void {}\n",
        "}\n",
        "class ChildClass extends ParentClass {\n",
        "    public function childMethod(): void {}\n",
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
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Own method
            assert!(
                method_names.contains(&"childMethod"),
                "Should include own 'childMethod'"
            );
            assert!(method_names.contains(&"test"), "Should include own 'test'");

            // From parent
            assert!(
                method_names.contains(&"parentMethod"),
                "Should include parent's 'parentMethod'"
            );
            assert!(
                method_names.contains(&"parentProtected"),
                "Should include parent's protected 'parentProtected'"
            );

            // From grandparent
            assert!(
                method_names.contains(&"ancestorMethod"),
                "Should include grandparent's 'ancestorMethod'"
            );

            // Private from grandparent should NOT appear
            assert!(
                !method_names.contains(&"gpPrivate"),
                "Should NOT include grandparent's private 'gpPrivate'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_inherits_static_members() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///inherit_static.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public static function staticBase(): void {}\n",
        "    protected static string $staticProp = '';\n",
        "    private static function privateStatic(): void {}\n",
        "}\n",
        "class Child extends Base {\n",
        "    public static function staticChild(): void {}\n",
        "    function test() {\n",
        "        self::\n",
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
                line: 9,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
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
                .map(|i| i.label.as_str())
                .collect();

            assert!(
                method_names.contains(&"staticChild"),
                "Should include own static method"
            );
            assert!(
                method_names.contains(&"staticBase"),
                "Should include inherited public static method"
            );
            assert!(
                !method_names.contains(&"privateStatic"),
                "Should NOT include inherited private static method"
            );
            assert!(
                prop_names.contains(&"$staticProp"),
                "Should include inherited protected static property"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_cross_file_inheritance_psr4() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/BaseModel.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "class BaseModel {\n",
                "    public function save(): bool { return true; }\n",
                "    protected function validate(): bool { return true; }\n",
                "    private function internalLog(): void {}\n",
                "    public string $id;\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\BaseModel;\n",
        "class User extends BaseModel {\n",
        "    public string $name;\n",
        "    public function getName(): string { return $this->name; }\n",
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
                line: 6,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for cross-file inheritance"
    );

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
                .map(|i| i.label.as_str())
                .collect();

            // Own members
            assert!(
                method_names.contains(&"getName"),
                "Should include own 'getName'"
            );
            assert!(
                prop_names.contains(&"name"),
                "Should include own property 'name'"
            );

            // Inherited public from BaseModel
            assert!(
                method_names.contains(&"save"),
                "Should include inherited 'save' from BaseModel"
            );
            assert!(
                prop_names.contains(&"id"),
                "Should include inherited property 'id' from BaseModel"
            );

            // Inherited protected from BaseModel
            assert!(
                method_names.contains(&"validate"),
                "Should include inherited protected 'validate'"
            );

            // Private should NOT be inherited
            assert!(
                !method_names.contains(&"internalLog"),
                "Should NOT include inherited private 'internalLog'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_cross_file_multi_level_inheritance_psr4() {
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
                "src/Base.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Base {\n",
                    "    public function baseMethod(): void {}\n",
                    "    protected function baseProtected(): void {}\n",
                    "    private function basePrivate(): void {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Middle.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Middle extends Base {\n",
                    "    public function middleMethod(): void {}\n",
                    "}\n",
                ),
            ),
        ],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Middle;\n",
        "class Leaf extends Middle {\n",
        "    public function leafMethod(): void {}\n",
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
                line: 5,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Completion should return results for multi-level cross-file inheritance"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"leafMethod"),
                "Should include own 'leafMethod'"
            );
            assert!(
                method_names.contains(&"middleMethod"),
                "Should include parent's 'middleMethod'"
            );
            assert!(
                method_names.contains(&"baseMethod"),
                "Should include grandparent's 'baseMethod'"
            );
            assert!(
                method_names.contains(&"baseProtected"),
                "Should include grandparent's protected 'baseProtected'"
            );
            assert!(
                !method_names.contains(&"basePrivate"),
                "Should NOT include grandparent's private 'basePrivate'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_override_across_files() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{
            "autoload": {
                "psr-4": {
                    "App\\": "src/"
                }
            }
        }"#,
        &[(
            "src/ParentClass.php",
            concat!(
                "<?php\n",
                "namespace App;\n",
                "class ParentClass {\n",
                "    public function render(): string { return ''; }\n",
                "    public function prepare(): void {}\n",
                "}\n",
            ),
        )],
    );

    let uri = Url::parse("file:///app.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\ParentClass;\n",
        "class ChildView extends ParentClass {\n",
        "    public function render(string $template): string { return ''; }\n",
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
                line: 5,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_items: Vec<&CompletionItem> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .collect();

            // 'render' should appear exactly once (child's version with $template param)
            let render_items: Vec<&&CompletionItem> = method_items
                .iter()
                .filter(|i| i.filter_text.as_deref() == Some("render"))
                .collect();
            assert_eq!(render_items.len(), 1, "'render' should appear exactly once");
            assert!(
                render_items[0].label.contains("$template"),
                "Should use child's signature with $template, got: {}",
                render_items[0].label
            );

            // 'prepare' should be inherited from parent
            let prepare_items: Vec<&&CompletionItem> = method_items
                .iter()
                .filter(|i| i.filter_text.as_deref() == Some("prepare"))
                .collect();
            assert_eq!(
                prepare_items.len(),
                1,
                "'prepare' should be inherited from parent"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_no_parent_class_unchanged_behavior() {
    // Verify that classes without extends still work exactly as before
    let backend = create_test_backend();

    let uri = Url::parse("file:///standalone.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Standalone {\n",
        "    public function doStuff(): void {}\n",
        "    private function internal(): void {}\n",
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
                line: 5,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"doStuff"),
                "Should include 'doStuff'"
            );
            assert!(
                method_names.contains(&"internal"),
                "Own private methods should still appear"
            );
            assert!(method_names.contains(&"test"), "Should include 'test'");
            assert_eq!(method_names.len(), 3, "Should have exactly 3 methods");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_variable_of_child_type_includes_inherited() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///var_inherit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Vehicle {\n",
        "    public function start(): void {}\n",
        "    protected function fuelCheck(): bool { return true; }\n",
        "    private function internalDiag(): void {}\n",
        "}\n",
        "class Car extends Vehicle {\n",
        "    public function openTrunk(): void {}\n",
        "}\n",
        "class Garage {\n",
        "    function test() {\n",
        "        $car = new Car();\n",
        "        $car->\n",
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
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"openTrunk"),
                "Should include Car's own 'openTrunk'"
            );
            assert!(
                method_names.contains(&"start"),
                "Should include inherited 'start' from Vehicle"
            );
            assert!(
                method_names.contains(&"fuelCheck"),
                "Should include inherited protected 'fuelCheck'"
            );
            assert!(
                !method_names.contains(&"internalDiag"),
                "Should NOT include inherited private 'internalDiag'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_completion_magic_methods_not_inherited() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///magic_inherit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Base {\n",
        "    public function __construct() {}\n",
        "    public function __toString(): string { return ''; }\n",
        "    public function realMethod(): void {}\n",
        "}\n",
        "class Child extends Base {\n",
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
                line: 8,
                character: 15,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some());

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Magic methods are filtered out by build_completion_items (existing behavior)
            assert!(
                !method_names.contains(&"__construct"),
                "Magic methods should be filtered"
            );
            assert!(
                !method_names.contains(&"__toString"),
                "Magic methods should be filtered"
            );
            // Non-magic inherited method should appear
            assert!(
                method_names.contains(&"realMethod"),
                "Should include inherited 'realMethod'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
