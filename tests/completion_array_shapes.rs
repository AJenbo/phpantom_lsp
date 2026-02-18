mod common;

use common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Array Shape Key Completion Tests ───────────────────────────────────────

#[tokio::test]
async fn test_array_shape_key_completion_var_annotation() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{name: string, age: int, email: string} $config */\n",
        "$config = getConfig();\n",
        "$config['\n",
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
                line: 3,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for array shape keys"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.contains(&"name"),
                "Should suggest 'name' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"age"),
                "Should suggest 'age' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"email"),
                "Should suggest 'email' key, got {:?}",
                labels
            );
            assert_eq!(items.len(), 3, "Should have exactly 3 key suggestions");

            // Check that details include the type info
            let name_item = items.iter().find(|i| i.label == "name").unwrap();
            assert_eq!(name_item.detail.as_deref(), Some("name: string"));
            assert_eq!(name_item.kind, Some(CompletionItemKind::FIELD));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_key_completion_param_annotation() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_param.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    /**\n",
        "     * @param array{host: string, port: int, ssl: bool} $options\n",
        "     */\n",
        "    public function connect(array $options): void {\n",
        "        $options['\n",
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
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for @param array shape"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"host"), "Should suggest 'host' key");
            assert!(labels.contains(&"port"), "Should suggest 'port' key");
            assert!(labels.contains(&"ssl"), "Should suggest 'ssl' key");
            assert_eq!(items.len(), 3);
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_key_completion_partial_filter() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_partial.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{name: string, namespace: string, age: int} $data */\n",
        "$data = getData();\n",
        "$data['na\n",
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
                line: 3,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should return filtered completions");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"name"), "Should suggest 'name'");
            assert!(labels.contains(&"namespace"), "Should suggest 'namespace'");
            assert!(
                !labels.contains(&"age"),
                "Should NOT suggest 'age' (doesn't start with 'na')"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_key_completion_double_quote() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_dquote.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{first_name: string, last_name: string} $user */\n",
        "$user = getUser();\n",
        "$user[\"\n",
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
                line: 3,
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions with double quotes"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.contains(&"first_name"),
                "Should suggest 'first_name'"
            );
            assert!(labels.contains(&"last_name"), "Should suggest 'last_name'");

            // Verify text_edit uses double quotes for closing
            let item = items.iter().find(|i| i.label == "first_name").unwrap();
            let edit_text = match &item.text_edit {
                Some(CompletionTextEdit::Edit(te)) => &te.new_text,
                _ => panic!("Expected text_edit on completion item"),
            };
            assert!(
                edit_text.contains('"'),
                "Text edit should use double quotes: {:?}",
                edit_text
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_key_completion_optional_keys() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_optional.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{name: string, age?: int, email?: string} $profile */\n",
        "$profile = getProfile();\n",
        "$profile['\n",
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
                line: 3,
                character: 10,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions including optional keys"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert_eq!(
                items.len(),
                3,
                "Should have 3 keys (including optional ones)"
            );

            // Check optional marker in detail
            let age_item = items.iter().find(|i| i.label == "age").unwrap();
            assert_eq!(
                age_item.detail.as_deref(),
                Some("age?: int"),
                "Optional key should show '?' in detail"
            );

            let name_item = items.iter().find(|i| i.label == "name").unwrap();
            assert_eq!(
                name_item.detail.as_deref(),
                Some("name: string"),
                "Required key should not show '?'"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_key_completion_bracket_only() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_bracket.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{id: int, title: string} $item */\n",
        "$item = getItem();\n",
        "$item[\n",
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
                line: 3,
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions when just [ is typed"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"id"), "Should suggest 'id'");
            assert!(labels.contains(&"title"), "Should suggest 'title'");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_key_completion_nested_generic_value() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_nested.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{users: list<User>, count: int, meta: array<string, mixed>} $result */\n",
        "$result = query();\n",
        "$result['\n",
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
                line: 3,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should handle nested generic types in shape values"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"users"), "Should suggest 'users'");
            assert!(labels.contains(&"count"), "Should suggest 'count'");
            assert!(labels.contains(&"meta"), "Should suggest 'meta'");
            assert_eq!(items.len(), 3);

            // Verify nested generics are preserved in the detail
            let users_item = items.iter().find(|i| i.label == "users").unwrap();
            assert_eq!(users_item.detail.as_deref(), Some("users: list<User>"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_no_completion_for_non_shape_array() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_no_shape.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var list<string> $names */\n",
        "$names = getNames();\n",
        "$names['\n",
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
                line: 3,
                character: 8,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    // Should NOT return array shape key completions for non-shape arrays.
    // It may return fallback completions though.
    if let Some(CompletionResponse::Array(items)) = result {
        // If we get items, none of them should be FIELD-kind array keys
        let field_items: Vec<&CompletionItem> = items
            .iter()
            .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
            .collect();
        assert!(
            field_items.is_empty(),
            "Should NOT suggest array shape keys for list<string>"
        );
    }
}

// ─── Array Shape Value Type Resolution (Chained Access) ─────────────────────

#[tokio::test]
async fn test_array_shape_value_type_member_access() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "class Handler {\n",
        "    /**\n",
        "     * @param array{user: User, count: int} $data\n",
        "     */\n",
        "    public function process(array $data): void {\n",
        "        $data['user']->\n",
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

    // Cursor right after `$data['user']->`
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 10,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for array shape value type"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                method_names.contains(&"getEmail"),
                "Should suggest User::getEmail(), got {:?}",
                method_names
            );
            // Check for the property too
            let prop_labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                prop_labels.contains(&"name"),
                "Should suggest User::$name property, got {:?}",
                prop_labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_value_type_inline_var() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_inline.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Address {\n",
        "    public string $city;\n",
        "    public string $zip;\n",
        "    public function format(): string {}\n",
        "}\n",
        "/** @var array{address: Address, phone: string} $contact */\n",
        "$contact = getContact();\n",
        "$contact['address']->\n",
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

    // Cursor right after `$contact['address']->`
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for inline @var array shape"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                method_names.contains(&"format"),
                "Should suggest Address::format(), got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_value_type_scalar_no_completion() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_scalar.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Service {\n",
        "    /**\n",
        "     * @param array{name: string, count: int} $data\n",
        "     */\n",
        "    public function handle(array $data): void {\n",
        "        $data['name']->\n",
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

    // Cursor right after `$data['name']->` — 'name' is a string scalar,
    // so no class members should be suggested.
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 23,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    // The result should either be None or fallback (no class members).
    if let Some(CompletionResponse::Array(items)) = result {
        let method_items: Vec<&CompletionItem> = items
            .iter()
            .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
            .collect();
        assert!(
            method_items.is_empty(),
            "Should NOT suggest class methods for scalar value type"
        );
    }
}

#[tokio::test]
async fn test_array_shape_value_type_double_quote_access() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_dquote_access.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(): void {}\n",
        "    public function error(): void {}\n",
        "}\n",
        "class App {\n",
        "    /**\n",
        "     * @param array{logger: Logger, name: string} $deps\n",
        "     */\n",
        "    public function boot(array $deps): void {\n",
        "        $deps[\"logger\"]->\n",
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

    // Cursor right after `$deps["logger"]->`
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 10,
                character: 25,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for double-quoted key access"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                method_names.contains(&"info"),
                "Should suggest Logger::info(), got {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"error"),
                "Should suggest Logger::error(), got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Array Shape from Return Type ───────────────────────────────────────────

#[tokio::test]
async fn test_array_shape_key_from_function_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_return.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/**\n",
        " * @return array{id: int, name: string, active: bool}\n",
        " */\n",
        "function getUser(): array { return []; }\n",
        "$user = getUser();\n",
        "$user['\n",
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
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from function return type"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"id"),
                "Should suggest 'id' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"name"),
                "Should suggest 'name' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"active"),
                "Should suggest 'active' key, got {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_chain_from_method_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_method_return.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(): void {}\n",
        "    public function warning(): void {}\n",
        "}\n",
        "class Service {\n",
        "    /**\n",
        "     * @return array{logger: Logger, debug: bool}\n",
        "     */\n",
        "    public function getDeps(): array { return []; }\n",
        "    public function run(): void {\n",
        "        $deps = $this->getDeps();\n",
        "        $deps['logger']->\n",
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

    // Cursor right after `$deps['logger']->`
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 12,
                character: 26,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions via method return shape"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                method_names.contains(&"info"),
                "Should suggest Logger::info(), got {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"warning"),
                "Should suggest Logger::warning(), got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Array Shape Parsing Edge Cases ─────────────────────────────────────────

#[tokio::test]
async fn test_array_shape_nullable_annotation() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_nullable.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var ?array{key: string, value: int} $map */\n",
        "$map = getMap();\n",
        "$map['\n",
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
                line: 3,
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should handle nullable array shape");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"key"), "Should suggest 'key'");
            assert!(labels.contains(&"value"), "Should suggest 'value'");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_empty_shape() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_empty.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{} $empty */\n",
        "$empty = getEmpty();\n",
        "$empty['\n",
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
                line: 3,
                character: 8,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    // No keys to suggest — should either be None or fallback
    if let Some(CompletionResponse::Array(items)) = result {
        let field_items: Vec<&CompletionItem> = items
            .iter()
            .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
            .collect();
        assert!(
            field_items.is_empty(),
            "Empty shape should not suggest any keys"
        );
    }
}

#[tokio::test]
async fn test_array_shape_insert_text_with_quote() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_insert.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{name: string, age: int} $data */\n",
        "$data = getData();\n",
        "$data['\n",
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
                line: 3,
                character: 7,
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
            let name_item = items.iter().find(|i| i.label == "name").unwrap();
            // When a quote was already typed, text_edit new_text should be: key + closing quote + ]
            let edit_text = match &name_item.text_edit {
                Some(CompletionTextEdit::Edit(te)) => &te.new_text,
                _ => panic!("Expected text_edit on completion item"),
            };
            assert_eq!(
                edit_text, "name']",
                "Text edit should close the quote and bracket"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_insert_text_no_quote() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_insert_noquote.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{name: string, age: int} $data */\n",
        "$data = getData();\n",
        "$data[\n",
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
                line: 3,
                character: 6,
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
            let name_item = items.iter().find(|i| i.label == "name").unwrap();
            // When no quote was typed, text_edit new_text should include quotes and bracket
            let edit_text = match &name_item.text_edit {
                Some(CompletionTextEdit::Edit(te)) => &te.new_text,
                _ => panic!("Expected text_edit on completion item"),
            };
            assert_eq!(
                edit_text, "'name']",
                "Text edit should include opening quote, key, closing quote, and bracket"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Array Shape Parsing Unit Tests ─────────────────────────────────────────

#[test]
fn test_parse_array_shape_basic() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("array{name: string, age: int}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "name");
    assert_eq!(entries[0].value_type, "string");
    assert!(!entries[0].optional);
    assert_eq!(entries[1].key, "age");
    assert_eq!(entries[1].value_type, "int");
    assert!(!entries[1].optional);
}

#[test]
fn test_parse_array_shape_optional_keys() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("array{name: string, age?: int, email?: string}").unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].key, "name");
    assert!(!entries[0].optional);
    assert_eq!(entries[1].key, "age");
    assert!(entries[1].optional);
    assert_eq!(entries[2].key, "email");
    assert!(entries[2].optional);
}

#[test]
fn test_parse_array_shape_positional() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("array{string, int, bool}").unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].key, "0");
    assert_eq!(entries[0].value_type, "string");
    assert_eq!(entries[1].key, "1");
    assert_eq!(entries[1].value_type, "int");
    assert_eq!(entries[2].key, "2");
    assert_eq!(entries[2].value_type, "bool");
}

#[test]
fn test_parse_array_shape_numeric_keys() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("array{0: User, 1: Address}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "0");
    assert_eq!(entries[0].value_type, "User");
    assert_eq!(entries[1].key, "1");
    assert_eq!(entries[1].value_type, "Address");
}

#[test]
fn test_parse_array_shape_nested_generics() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries =
        parse_array_shape("array{users: list<User>, meta: array<string, mixed>}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "users");
    assert_eq!(entries[0].value_type, "list<User>");
    assert_eq!(entries[1].key, "meta");
    assert_eq!(entries[1].value_type, "array<string, mixed>");
}

#[test]
fn test_parse_array_shape_empty() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("array{}").unwrap();
    assert!(entries.is_empty());
}

#[test]
fn test_parse_array_shape_not_a_shape() {
    use phpantom_lsp::docblock::parse_array_shape;

    assert!(parse_array_shape("array<int, User>").is_none());
    assert!(parse_array_shape("list<User>").is_none());
    assert!(parse_array_shape("string").is_none());
    assert!(parse_array_shape("User").is_none());
}

#[test]
fn test_parse_array_shape_nullable() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("?array{name: string}").unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "name");
}

#[test]
fn test_extract_array_shape_value_type() {
    use phpantom_lsp::docblock::extract_array_shape_value_type;

    assert_eq!(
        extract_array_shape_value_type("array{name: string, user: User}", "user"),
        Some("User".to_string())
    );
    assert_eq!(
        extract_array_shape_value_type("array{name: string, user: User}", "name"),
        Some("string".to_string())
    );
    assert_eq!(
        extract_array_shape_value_type("array{name: string, user: User}", "missing"),
        None
    );
    assert_eq!(
        extract_array_shape_value_type("list<User>", "anything"),
        None
    );
}

#[test]
fn test_parse_array_shape_nested_shapes() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries =
        parse_array_shape("array{user: array{name: string, age: int}, active: bool}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "user");
    assert_eq!(entries[0].value_type, "array{name: string, age: int}");
    assert_eq!(entries[1].key, "active");
    assert_eq!(entries[1].value_type, "bool");
}

// ─── split_type_token and clean_type with array shapes ──────────────────────

#[test]
fn test_clean_type_preserves_array_shape() {
    use phpantom_lsp::docblock::clean_type;

    assert_eq!(
        clean_type("array{name: string, age: int}"),
        "array{name: string, age: int}"
    );
    assert_eq!(
        clean_type("array{name: string, age: int}|null"),
        "array{name: string, age: int}"
    );
    assert_eq!(clean_type("\\array{name: string}"), "array{name: string}");
}

#[test]
fn test_base_class_name_strips_shape() {
    use phpantom_lsp::docblock::base_class_name;

    assert_eq!(base_class_name("array{name: string, age: int}"), "array");
    assert_eq!(base_class_name("array{name: string}|null"), "array");
}

// ─── Array Shape Key Completion in Class Method Context ─────────────────────

#[tokio::test]
async fn test_array_shape_key_completion_inside_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_method_ctx.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class UserService {\n",
        "    /**\n",
        "     * @param array{username: string, password: string, remember?: bool} $credentials\n",
        "     */\n",
        "    public function login(array $credentials): void {\n",
        "        $credentials['\n",
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
                character: 21,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should return completions inside method");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(labels.contains(&"username"), "Should suggest 'username'");
            assert!(labels.contains(&"password"), "Should suggest 'password'");
            assert!(
                labels.contains(&"remember"),
                "Should suggest 'remember' (optional)"
            );
            assert_eq!(items.len(), 3);
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_sort_order_preserved() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_sort.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{zebra: string, apple: string, mango: string} $fruits */\n",
        "$fruits = getFruits();\n",
        "$fruits['\n",
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
                line: 3,
                character: 9,
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
            // Sort texts should preserve declaration order, not alphabetical
            let sort_texts: Vec<&str> = items
                .iter()
                .map(|i| i.sort_text.as_deref().unwrap())
                .collect();
            assert_eq!(sort_texts, vec!["0000", "0001", "0002"]);

            // First item should be "zebra" (declared first)
            assert_eq!(items[0].label, "zebra");
            assert_eq!(items[1].label, "apple");
            assert_eq!(items[2].label, "mango");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Text Edit Range Tests (auto-close handling) ────────────────────────────

/// When the IDE auto-inserts `]` after `[`, the text_edit range must
/// cover that trailing `]` so we don't end up with `$config['host']]`.
#[tokio::test]
async fn test_array_shape_text_edit_range_bracket_autoclose() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_autoclose_bracket.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{host: string, port: int} $config */\n",
        "$config = getConfig();\n",
        "$config[]\n",
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

    // Cursor between [ and ] — column 8
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 3,
                character: 8,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should return completions");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let host_item = items.iter().find(|i| i.label == "host").unwrap();
            let te = match &host_item.text_edit {
                Some(CompletionTextEdit::Edit(te)) => te,
                _ => panic!("Expected text_edit"),
            };
            // Range should cover from col 8 (after [) to col 9 (past the auto-inserted ])
            assert_eq!(
                te.range.start.character, 8,
                "Range start should be at key_start_col"
            );
            assert_eq!(
                te.range.end.character, 9,
                "Range end should cover the trailing ]"
            );
            assert_eq!(
                te.new_text, "'host']",
                "new_text should include quote + key + quote + ]"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// When the IDE auto-inserts `']` after `['`, the text_edit range must
/// cover those trailing chars so we don't end up with `$config['host']']`.
#[tokio::test]
async fn test_array_shape_text_edit_range_quote_autoclose() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_autoclose_quote.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{host: string, port: int} $config */\n",
        "$config = getConfig();\n",
        "$config['']\n",
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

    // Cursor between the two single quotes — column 9
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 3,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should return completions");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let host_item = items.iter().find(|i| i.label == "host").unwrap();
            let te = match &host_item.text_edit {
                Some(CompletionTextEdit::Edit(te)) => te,
                _ => panic!("Expected text_edit"),
            };
            // Range should cover from col 9 (after opening ') to col 11 (past '])
            assert_eq!(
                te.range.start.character, 9,
                "Range start should be at key_start_col"
            );
            assert_eq!(
                te.range.end.character, 11,
                "Range end should cover trailing ']"
            );
            assert_eq!(te.new_text, "host']", "new_text should be key + quote + ]");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Method Return Type Key Completion ──────────────────────────────────────

/// `$data = $this->getUserData(); $data['` should suggest keys from the
/// method's `@return` array shape.
#[tokio::test]
async fn test_array_shape_key_from_method_return_type() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_method_key.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class UserService {\n",
        "    /** @return array{name: string, email: string, age: int} */\n",
        "    public function getUserData(): array { return []; }\n",
        "    public function test(): void {\n",
        "        $data = $this->getUserData();\n",
        "        $data['\n",
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
        "Should return key completions from method return type"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"name"),
                "Should suggest 'name' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"email"),
                "Should suggest 'email' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"age"),
                "Should suggest 'age' key, got {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Nested Array Shape Key Completion ──────────────────────────────────────

/// `$response['meta']['` should suggest keys from the nested shape.
#[tokio::test]
async fn test_array_shape_nested_key_completion() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_nested_keys.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{meta: array{page: int, total: int}, items: list<string>} $response */\n",
        "$response = getResponse();\n",
        "$response['meta']['\n",
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
                line: 3,
                character: 19,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for nested array shape"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"page"),
                "Should suggest 'page' from nested shape, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"total"),
                "Should suggest 'total' from nested shape, got {:?}",
                labels
            );
            assert_eq!(labels.len(), 2, "Should only suggest nested keys");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── $_SERVER Superglobal Key Completion ────────────────────────────────────

#[tokio::test]
async fn test_server_superglobal_key_completion() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///server_keys.php").unwrap();
    let text = concat!("<?php\n", "$_SERVER['\n",);

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
                line: 1,
                character: 10,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should return $_SERVER key completions");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"REQUEST_METHOD"),
                "Should suggest REQUEST_METHOD, got {:?}",
                labels
            );
            assert!(labels.contains(&"HTTP_HOST"), "Should suggest HTTP_HOST");
            assert!(
                labels.contains(&"SERVER_NAME"),
                "Should suggest SERVER_NAME"
            );
            assert!(
                labels.contains(&"REMOTE_ADDR"),
                "Should suggest REMOTE_ADDR"
            );
            assert!(
                labels.contains(&"REQUEST_URI"),
                "Should suggest REQUEST_URI"
            );
            // Should have all 40 known keys
            assert_eq!(labels.len(), 40, "Should suggest all known $_SERVER keys");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_server_superglobal_key_partial_filter() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///server_keys_filter.php").unwrap();
    let text = concat!("<?php\n", "$_SERVER['REQ\n",);

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
                line: 1,
                character: 13,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(result.is_some(), "Should return filtered $_SERVER keys");

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(labels.contains(&"REQUEST_METHOD"));
            assert!(labels.contains(&"REQUEST_TIME"));
            assert!(labels.contains(&"REQUEST_TIME_FLOAT"));
            assert!(labels.contains(&"REQUEST_URI"));
            // Should NOT contain non-matching keys
            assert!(
                !labels.contains(&"SERVER_NAME"),
                "Should not suggest SERVER_NAME when filtering by REQ"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_server_superglobal_bracket_only() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///server_bracket.php").unwrap();
    let text = concat!("<?php\n", "$_SERVER[\n",);

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
                line: 1,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return $_SERVER key completions with just ["
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            assert!(!items.is_empty(), "Should have $_SERVER key suggestions");
            // All items should be FIELD kind
            assert!(
                items
                    .iter()
                    .all(|i| i.kind == Some(CompletionItemKind::FIELD)),
                "All items should be FIELD kind"
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_server_superglobal_text_edit_autoclose() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///server_autoclose.php").unwrap();
    let text = concat!("<?php\n", "$_SERVER['']\n",);

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: text.to_string(),
        },
    };
    backend.did_open(open_params).await;

    // Cursor between the two quotes — column 10
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 1,
                character: 10,
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
            let item = items.iter().find(|i| i.label == "HTTP_HOST").unwrap();
            let te = match &item.text_edit {
                Some(CompletionTextEdit::Edit(te)) => te,
                _ => panic!("Expected text_edit"),
            };
            // Range should cover from col 10 to col 12 (past '])
            assert_eq!(te.range.start.character, 10);
            assert_eq!(te.range.end.character, 12);
            assert_eq!(te.new_text, "HTTP_HOST']");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Method Return Type + Auto-Close Bracket ────────────────────────────────

/// `$data = $this->getUserData(); $data[]` (bracket auto-closed by IDE,
/// cursor between `[` and `]`) should suggest keys from the method's
/// `@return` array shape.
#[tokio::test]
async fn test_array_shape_key_from_method_return_type_bracket_autoclose() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_method_key_autoclose.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class UserService {\n",
        "    /** @return array{name: string, email: string, age: int} */\n",
        "    public function getUserData(): array { return []; }\n",
        "    public function test(): void {\n",
        "        $data = $this->getUserData();\n",
        "        $data[]\n",
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

    // Cursor between [ and ] — column 14
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from method return type with auto-closed bracket"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"name"),
                "Should suggest 'name' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"email"),
                "Should suggest 'email' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"age"),
                "Should suggest 'age' key, got {:?}",
                labels
            );

            // Verify text_edit covers the trailing ]
            let name_item = items.iter().find(|i| i.label == "name").unwrap();
            let te = match &name_item.text_edit {
                Some(CompletionTextEdit::Edit(te)) => te,
                _ => panic!("Expected text_edit"),
            };
            assert_eq!(te.range.start.character, 14, "Start at key_start_col");
            assert_eq!(te.range.end.character, 15, "End past the auto-inserted ]");
            assert_eq!(te.new_text, "'name']");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Chained Array Shape + List Element Access ──────────────────────────────

/// `$response['items'][0]->` should resolve the list element type and
/// offer class member completions.
#[tokio::test]
async fn test_array_shape_list_element_member_access() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_list_elem.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "/** @var array{meta: array{page: int, total: int}, items: list<User>} $response */\n",
        "$response = getResponse();\n",
        "$response['items'][0]->\n",
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

    // Cursor right after `->`
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
    assert!(
        result.is_some(),
        "Should return User member completions from list element access"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let names: Vec<&str> = items
                .iter()
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                names.contains(&"name"),
                "Should suggest User::$name, got {:?}",
                names
            );
            assert!(
                names.contains(&"getEmail"),
                "Should suggest User::getEmail(), got {:?}",
                names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Quoted Key Parsing Tests ───────────────────────────────────────────────

#[test]
fn test_parse_array_shape_single_quoted_keys() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("array{'foo': int, 'bar': string}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "foo");
    assert_eq!(entries[0].value_type, "int");
    assert!(!entries[0].optional);
    assert_eq!(entries[1].key, "bar");
    assert_eq!(entries[1].value_type, "string");
    assert!(!entries[1].optional);
}

#[test]
fn test_parse_array_shape_double_quoted_keys() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape(r#"array{"foo": int, "bar": string}"#).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "foo");
    assert_eq!(entries[0].value_type, "int");
    assert_eq!(entries[1].key, "bar");
    assert_eq!(entries[1].value_type, "string");
}

#[test]
fn test_parse_array_shape_mixed_quoted_and_unquoted_keys() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape(r#"array{'foo': int, "bar"?: string, baz: bool}"#).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].key, "foo");
    assert_eq!(entries[0].value_type, "int");
    assert!(!entries[0].optional);
    assert_eq!(entries[1].key, "bar");
    assert_eq!(entries[1].value_type, "string");
    assert!(entries[1].optional);
    assert_eq!(entries[2].key, "baz");
    assert_eq!(entries[2].value_type, "bool");
    assert!(!entries[2].optional);
}

#[test]
fn test_parse_array_shape_quoted_key_with_spaces() {
    use phpantom_lsp::docblock::parse_array_shape;

    let entries = parse_array_shape("array{'po rt': int, 'my key': string}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "po rt");
    assert_eq!(entries[0].value_type, "int");
    assert_eq!(entries[1].key, "my key");
    assert_eq!(entries[1].value_type, "string");
}

#[test]
fn test_parse_array_shape_quoted_key_with_special_chars() {
    use phpantom_lsp::docblock::parse_array_shape;

    // Key contains comma, colon, braces, question mark — all should be
    // treated as literal characters inside quotes.
    let entries = parse_array_shape(
        r#"array{",host?:}"?: string, 'po rt': int, credentials: User|AdminUser}"#,
    )
    .unwrap();
    assert_eq!(entries.len(), 3, "entries: {:?}", entries);
    assert_eq!(entries[0].key, ",host?:}");
    assert_eq!(entries[0].value_type, "string");
    assert!(entries[0].optional);
    assert_eq!(entries[1].key, "po rt");
    assert_eq!(entries[1].value_type, "int");
    assert!(!entries[1].optional);
    assert_eq!(entries[2].key, "credentials");
    assert_eq!(entries[2].value_type, "User|AdminUser");
    assert!(!entries[2].optional);
}

#[test]
fn test_parse_array_shape_optional_quoted_key() {
    use phpantom_lsp::docblock::parse_array_shape;

    // Optional marker `?` after closing quote: `"bar"?: string`
    let entries = parse_array_shape(r#"array{"bar"?: string, 'baz'?: int}"#).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "bar");
    assert!(entries[0].optional);
    assert_eq!(entries[0].value_type, "string");
    assert_eq!(entries[1].key, "baz");
    assert!(entries[1].optional);
    assert_eq!(entries[1].value_type, "int");
}

#[test]
fn test_parse_array_shape_quoted_key_with_colon() {
    use phpantom_lsp::docblock::parse_array_shape;

    // Colon inside a quoted key must not be treated as the key:value separator.
    let entries = parse_array_shape(r#"array{"host:port": string, name: string}"#).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "host:port");
    assert_eq!(entries[0].value_type, "string");
    assert_eq!(entries[1].key, "name");
    assert_eq!(entries[1].value_type, "string");
}

#[test]
fn test_parse_array_shape_quoted_key_with_comma() {
    use phpantom_lsp::docblock::parse_array_shape;

    // Comma inside a quoted key must not split entries.
    let entries = parse_array_shape(r#"array{"first,last": string, age: int}"#).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "first,last");
    assert_eq!(entries[0].value_type, "string");
    assert_eq!(entries[1].key, "age");
    assert_eq!(entries[1].value_type, "int");
}

#[test]
fn test_parse_array_shape_quoted_key_with_braces() {
    use phpantom_lsp::docblock::parse_array_shape;

    // Braces inside a quoted key must not break brace matching.
    let entries = parse_array_shape(r#"array{"{key}": string, normal: int}"#).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "{key}");
    assert_eq!(entries[0].value_type, "string");
    assert_eq!(entries[1].key, "normal");
    assert_eq!(entries[1].value_type, "int");
}

#[test]
fn test_extract_array_shape_value_type_quoted_key() {
    use phpantom_lsp::docblock::extract_array_shape_value_type;

    // Lookup by unquoted key name should match a quoted key in the shape.
    let t = r#"array{"host": string, 'port': int, ssl: bool}"#;
    assert_eq!(
        extract_array_shape_value_type(t, "host"),
        Some("string".to_string())
    );
    assert_eq!(
        extract_array_shape_value_type(t, "port"),
        Some("int".to_string())
    );
    assert_eq!(
        extract_array_shape_value_type(t, "ssl"),
        Some("bool".to_string())
    );
}

#[test]
fn test_parse_array_shape_phpstan_spec_examples() {
    use phpantom_lsp::docblock::parse_array_shape;

    // From the PHPStan documentation:
    // array{'foo': int, "bar": string}
    let entries = parse_array_shape(r#"array{'foo': int, "bar": string}"#).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "foo");
    assert_eq!(entries[0].value_type, "int");
    assert_eq!(entries[1].key, "bar");
    assert_eq!(entries[1].value_type, "string");

    // array{'foo': int, "bar"?: string}
    let entries = parse_array_shape(r#"array{'foo': int, "bar"?: string}"#).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "foo");
    assert!(!entries[0].optional);
    assert_eq!(entries[1].key, "bar");
    assert!(entries[1].optional);

    // array{int, int} (tuple)
    let entries = parse_array_shape("array{int, int}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "0");
    assert_eq!(entries[0].value_type, "int");
    assert_eq!(entries[1].key, "1");
    assert_eq!(entries[1].value_type, "int");

    // array{0: int, 1?: int}
    let entries = parse_array_shape("array{0: int, 1?: int}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "0");
    assert!(!entries[0].optional);
    assert_eq!(entries[1].key, "1");
    assert!(entries[1].optional);

    // array{foo: int, bar: string}
    let entries = parse_array_shape("array{foo: int, bar: string}").unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].key, "foo");
    assert_eq!(entries[1].key, "bar");
}

#[tokio::test]
async fn test_array_shape_key_completion_quoted_keys_in_annotation() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_quoted_keys.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{'first name': string, \"last-name\": string, age: int} $person */\n",
        "$person = getData();\n",
        "$person['\n",
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
                line: 3,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for quoted-key array shape"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.contains(&"first name"),
                "Should suggest 'first name' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"last-name"),
                "Should suggest 'last-name' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"age"),
                "Should suggest 'age' key, got {:?}",
                labels
            );
            assert_eq!(items.len(), 3, "Should have exactly 3 key suggestions");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_key_completion_method_return_bracket_only_no_semicolon() {
    // Reproduce the user's exact scenario: `$data[]` inside a method
    // where `$data` was assigned from `$this->getUserData()`, but
    // WITHOUT a trailing semicolon (as during active typing).
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_method_nosemi.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ArrayShapeDemo {\n",
        "    /**\n",
        "     * @return array{user: string, profile: string, active: bool}\n",
        "     */\n",
        "    public function getUserData(): array {\n",
        "        return [];\n",
        "    }\n",
        "    public function test(): void {\n",
        "        $data = $this->getUserData();\n",
        "        $data[]\n",
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

    // Cursor between [ and ] — 8 spaces indent + "$data[" = col 14
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 10,
                character: 14,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from method return type (bracket-only, no semicolon)"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"user"),
                "Should suggest 'user' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"profile"),
                "Should suggest 'profile' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"active"),
                "Should suggest 'active' key, got {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Systematic Method-Return Array Key Context Tests ───────────────────────
//
// Each test uses a single broken line inside a method where $data was
// assigned from $this->getUserData() which has @return array{…}.
// This covers every cursor pattern the user might encounter.

/// Helper: build the PHP source for a method-return-type array key test.
/// `cursor_line_content` is the literal line (WITHOUT leading spaces) that
/// contains the cursor, e.g. `$data['` or `$data[]`.
fn method_return_array_key_php(cursor_line_content: &str) -> String {
    format!(
        "<?php\n\
         class ArrayShapeDemo {{\n\
         \x20\x20\x20\x20/**\n\
         \x20\x20\x20\x20 * @return array{{user: string, profile: string, active: bool}}\n\
         \x20\x20\x20\x20 */\n\
         \x20\x20\x20\x20public function getUserData(): array {{\n\
         \x20\x20\x20\x20\x20\x20\x20\x20return [];\n\
         \x20\x20\x20\x20}}\n\
         \x20\x20\x20\x20public function test(): void {{\n\
         \x20\x20\x20\x20\x20\x20\x20\x20$data = $this->getUserData();\n\
         \x20\x20\x20\x20\x20\x20\x20\x20{}\n\
         \x20\x20\x20\x20}}\n\
         }}\n",
        cursor_line_content
    )
}

/// Helper: run array key completion on line 10 at the given column and
/// return the labels of FIELD-kind items (or an empty vec on no results).
async fn run_method_return_key_completion(cursor_line: &str, cursor_col: u32) -> Vec<String> {
    let backend = create_test_backend();
    let uri = Url::parse("file:///method_return_key_test.php").unwrap();
    let text = method_return_array_key_php(cursor_line);

    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text,
        },
    };
    backend.did_open(open_params).await;

    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 10,
                character: cursor_col,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    match result {
        Some(CompletionResponse::Array(items)) => items
            .iter()
            .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
            .map(|i| i.label.clone())
            .collect(),
        _ => vec![],
    }
}

fn assert_has_shape_keys(labels: &[String], scenario: &str) {
    assert!(
        labels.contains(&"user".to_string()),
        "{}: should suggest 'user', got {:?}",
        scenario,
        labels
    );
    assert!(
        labels.contains(&"profile".to_string()),
        "{}: should suggest 'profile', got {:?}",
        scenario,
        labels
    );
    assert!(
        labels.contains(&"active".to_string()),
        "{}: should suggest 'active', got {:?}",
        scenario,
        labels
    );
}

// ── Pattern: $data[  (bracket only, no close) ───────────────────────────────
#[tokio::test]
async fn test_method_return_key_bracket_open() {
    //         $data[
    // cols:   01234567891011121314
    // 8 spaces indent + "$data[" → cursor at col 14
    let labels = run_method_return_key_completion("$data[", 14).await;
    assert_has_shape_keys(&labels, "$data[");
}

// ── Pattern: $data['  (single quote, no close) ──────────────────────────────
#[tokio::test]
async fn test_method_return_key_single_quote_open() {
    // cursor after the single quote → col 15
    let labels = run_method_return_key_completion("$data['", 15).await;
    assert_has_shape_keys(&labels, "$data['");
}

// ── Pattern: $data["  (double quote, no close) ──────────────────────────────
#[tokio::test]
async fn test_method_return_key_double_quote_open() {
    let labels = run_method_return_key_completion("$data[\"", 15).await;
    assert_has_shape_keys(&labels, "$data[\"");
}

// ── Pattern: $data[]  (auto-closed bracket) ─────────────────────────────────
#[tokio::test]
async fn test_method_return_key_bracket_autoclosed() {
    // cursor between [ and ] → col 14
    let labels = run_method_return_key_completion("$data[]", 14).await;
    assert_has_shape_keys(&labels, "$data[]");
}

// ── Pattern: $data['']  (auto-closed single quote inside auto-closed bracket)
#[tokio::test]
async fn test_method_return_key_single_quote_autoclosed() {
    // cursor between the two single quotes → col 15
    let labels = run_method_return_key_completion("$data['']", 15).await;
    assert_has_shape_keys(&labels, "$data['']");
}

// ── Pattern: $data[""]  (auto-closed double quote inside auto-closed bracket)
#[tokio::test]
async fn test_method_return_key_double_quote_autoclosed() {
    let labels = run_method_return_key_completion("$data[\"\"]", 15).await;
    assert_has_shape_keys(&labels, "$data[\"\"]");
}

// ── Pattern: $data[']  (auto-closed bracket after single quote) ─────────────
#[tokio::test]
async fn test_method_return_key_single_quote_bracket_autoclosed() {
    // cursor after quote, before ] → col 15
    let labels = run_method_return_key_completion("$data[']", 15).await;
    assert_has_shape_keys(&labels, "$data[']");
}

// ── Pattern: $data["]  (auto-closed bracket after double quote) ─────────────
#[tokio::test]
async fn test_method_return_key_double_quote_bracket_autoclosed() {
    let labels = run_method_return_key_completion("$data[\"]", 15).await;
    assert_has_shape_keys(&labels, "$data[\"]");
}

// ── Pattern: $data[''  (auto-closed single quote, no bracket close) ─────────
#[tokio::test]
async fn test_method_return_key_single_quote_pair_no_bracket() {
    // cursor between the two quotes → col 15
    let labels = run_method_return_key_completion("$data[''", 15).await;
    assert_has_shape_keys(&labels, "$data[''");
}

// ── Pattern: $data[""  (auto-closed double quote, no bracket close) ─────────
#[tokio::test]
async fn test_method_return_key_double_quote_pair_no_bracket() {
    let labels = run_method_return_key_completion("$data[\"\"", 15).await;
    assert_has_shape_keys(&labels, "$data[\"\"");
}

// ─── Preceding class/interface in the same file ─────────────────────────────
//
// When another class or interface is defined before the target class,
// `find_current_class_from_content` must pick the class that actually
// contains the cursor — not just `classes.first()`.

#[tokio::test]
async fn test_method_return_key_with_preceding_interface() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_preceding_iface.php").unwrap();
    let text = concat!(
        "<?php\n",
        "interface Renderable {}\n",
        "\n",
        "class ArrayShapeDemo {\n",
        "    /**\n",
        "     * @return array{user: string, profile: string, active: bool}\n",
        "     */\n",
        "    public function getUserData(): array {\n",
        "        return [];\n",
        "    }\n",
        "    public function methodReturnShapeKeys(): void {\n",
        "        $data = $this->getUserData();\n",
        "        $data[]\n",
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

    // Cursor between [ and ] on line 12: 8 spaces + "$data[" = col 14
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
    assert!(
        result.is_some(),
        "Should return key completions even with a preceding interface"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"user"),
                "Should suggest 'user' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"profile"),
                "Should suggest 'profile' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"active"),
                "Should suggest 'active' key, got {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_method_return_key_with_preceding_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_preceding_class.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class SomeHelper {\n",
        "    public function help(): void {}\n",
        "}\n",
        "\n",
        "class ArrayShapeDemo {\n",
        "    /**\n",
        "     * @return array{user: string, profile: string, active: bool}\n",
        "     */\n",
        "    public function getUserData(): array {\n",
        "        return [];\n",
        "    }\n",
        "    public function test(): void {\n",
        "        $data = $this->getUserData();\n",
        "        $data['\n",
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

    // Cursor after the single quote on line 14: 8 spaces + "$data['" = col 15
    let completion_params = CompletionParams {
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
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions even with a preceding class"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"user"),
                "Should suggest 'user' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"profile"),
                "Should suggest 'profile' key, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"active"),
                "Should suggest 'active' key, got {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Array Shape Inference from Literal Arrays ──────────────────────────────

#[tokio::test]
async fn test_array_shape_inferred_from_literal_array() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_literal.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$var = ['key1' => 1, 'key2' => 'hello'];\n",
        "$var['\n",
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
                line: 2,
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from literal array"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"key1"),
                "Should suggest 'key1', got {:?}",
                labels
            );
            assert!(
                labels.contains(&"key2"),
                "Should suggest 'key2', got {:?}",
                labels
            );
            assert_eq!(labels.len(), 2, "Should have exactly 2 key suggestions");

            // Verify inferred types in detail
            let k1 = items.iter().find(|i| i.label == "key1").unwrap();
            assert_eq!(k1.detail.as_deref(), Some("key1: int"));
            let k2 = items.iter().find(|i| i.label == "key2").unwrap();
            assert_eq!(k2.detail.as_deref(), Some("key2: string"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_inferred_from_literal_with_various_types() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_literal_types.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {}\n",
        "$var = [\n",
        "    'name' => 'Alice',\n",
        "    'age' => 42,\n",
        "    'score' => 3.14,\n",
        "    'active' => true,\n",
        "    'deleted' => null,\n",
        "    'user' => new User(),\n",
        "    'tags' => ['a', 'b'],\n",
        "];\n",
        "$var['\n",
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
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from multi-type literal array"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert_eq!(labels.len(), 7, "Should have 7 keys, got {:?}", labels);

            let find = |name: &str| -> String {
                items
                    .iter()
                    .find(|i| i.label == name)
                    .and_then(|i| i.detail.clone())
                    .unwrap_or_default()
            };
            assert_eq!(find("name"), "name: string");
            assert_eq!(find("age"), "age: int");
            assert_eq!(find("score"), "score: float");
            assert_eq!(find("active"), "active: bool");
            assert_eq!(find("deleted"), "deleted: null");
            assert_eq!(find("user"), "user: User");
            assert_eq!(find("tags"), "tags: array");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_incremental_key_assignments() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_incremental.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$var = ['key1' => 1];\n",
        "$var['key2'] = 'hello';\n",
        "$var['key3'] = true;\n",
        "$var['\n",
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
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from incremental assignments"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"key1"),
                "Should suggest 'key1' from initial literal, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"key2"),
                "Should suggest 'key2' from incremental assignment, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"key3"),
                "Should suggest 'key3' from incremental assignment, got {:?}",
                labels
            );
            assert_eq!(labels.len(), 3, "Should have exactly 3 keys");

            let find = |name: &str| -> String {
                items
                    .iter()
                    .find(|i| i.label == name)
                    .and_then(|i| i.detail.clone())
                    .unwrap_or_default()
            };
            assert_eq!(find("key1"), "key1: int");
            assert_eq!(find("key2"), "key2: string");
            assert_eq!(find("key3"), "key3: bool");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_empty_array_with_incremental_assignments() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_empty_incr.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$bar = [];\n",
        "$bar['name'] = 'Alice';\n",
        "$bar['age'] = 30;\n",
        "$bar['\n",
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
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from empty array + incremental assignments"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(
                labels.contains(&"name"),
                "Should suggest 'name', got {:?}",
                labels
            );
            assert!(
                labels.contains(&"age"),
                "Should suggest 'age', got {:?}",
                labels
            );
            assert_eq!(labels.len(), 2, "Should have exactly 2 keys");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_incremental_override_type() {
    // When the same key is assigned twice, the later type wins.
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_override.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$var = ['status' => 'pending'];\n",
        "$var['status'] = 42;\n",
        "$var['\n",
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
                line: 3,
                character: 6,
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
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert_eq!(labels.len(), 1);
            assert_eq!(labels[0], "status");
            // The incremental assignment overrides the initial type
            let detail = items[0].detail.as_deref().unwrap();
            assert_eq!(detail, "status: int");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_literal_array_syntax() {
    // Test `array(…)` syntax (older PHP style)
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_old_syntax.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$var = array('host' => 'localhost', 'port' => 3306);\n",
        "$var['\n",
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
                line: 2,
                character: 6,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from array() syntax"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(labels.contains(&"host"), "got {:?}", labels);
            assert!(labels.contains(&"port"), "got {:?}", labels);
            assert_eq!(labels.len(), 2);
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_literal_inside_class_method() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_literal_method.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Config {\n",
        "    public function build(): void {\n",
        "        $opts = ['driver' => 'mysql', 'port' => 3306];\n",
        "        $opts['charset'] = 'utf8';\n",
        "        $opts['\n",
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
        "Should return key completions from literal + incremental inside a method"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(labels.contains(&"driver"), "got {:?}", labels);
            assert!(labels.contains(&"port"), "got {:?}", labels);
            assert!(labels.contains(&"charset"), "got {:?}", labels);
            assert_eq!(labels.len(), 3);
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_literal_double_quoted_keys() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_literal_dq.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$cfg = [\"host\" => 'localhost', \"port\" => 8080];\n",
        "$cfg['\n",
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
                line: 2,
                character: 6,
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
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(labels.contains(&"host"), "got {:?}", labels);
            assert!(labels.contains(&"port"), "got {:?}", labels);
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_annotation_takes_priority_over_literal() {
    // When both a @var annotation and a literal exist, the annotation wins.
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_annotation_priority.php").unwrap();
    let text = concat!(
        "<?php\n",
        "/** @var array{x: int, y: int} $point */\n",
        "$point = ['x' => 1, 'y' => 2, 'z' => 3];\n",
        "$point['\n",
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
                line: 3,
                character: 8,
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
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            // @var annotation says only x and y — z from the literal
            // should NOT appear because the annotation takes priority.
            assert_eq!(
                labels.len(),
                2,
                "Annotation should take priority, got {:?}",
                labels
            );
            assert!(labels.contains(&"x"));
            assert!(labels.contains(&"y"));
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_array_shape_incremental_with_new_object() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_incr_object.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {}\n",
        "class Address {}\n",
        "$data = [];\n",
        "$data['user'] = new User();\n",
        "$data['address'] = new Address();\n",
        "$data['\n",
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
                character: 7,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return key completions from incremental new-object assignments"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::FIELD))
                .map(|i| i.label.as_str())
                .collect();
            assert!(labels.contains(&"user"), "got {:?}", labels);
            assert!(labels.contains(&"address"), "got {:?}", labels);
            assert_eq!(labels.len(), 2);

            let find = |name: &str| -> String {
                items
                    .iter()
                    .find(|i| i.label == name)
                    .and_then(|i| i.detail.clone())
                    .unwrap_or_default()
            };
            assert_eq!(find("user"), "user: User");
            assert_eq!(find("address"), "address: Address");
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Scope-Aware Annotation Tests ───────────────────────────────────────────

/// When a class method has `@param array{…} $config` and file-scope code
/// also uses `$config`, the annotation from inside the class must NOT
/// leak to the outer scope.  Completions at file scope should come from
/// the literal array assignment, not the method parameter.
#[tokio::test]
async fn test_array_shape_key_completion_does_not_leak_from_class_scope() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_scope.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class ArrayShapeDemo {\n",
        "    /**\n",
        "     * @param array{host: string, port: int, credentials: string} $config\n",
        "     */\n",
        "    public function connect(array $config): void {\n",
        "        $config['host'];\n",
        "    }\n",
        "}\n",
        "\n",
        "$config = ['host' => 'localhost', 'port' => 3306, 'ssl' => true, 'author' => 'me'];\n",
        "$config['\n",
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

    // Cursor at $config[' on the last line (line 11, char 9)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 11,
                character: 9,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for file-scope $config"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            // Should have keys from the literal array, NOT from the @param annotation
            assert!(
                labels.contains(&"ssl"),
                "Should suggest 'ssl' from literal array, got {:?}",
                labels
            );
            assert!(
                labels.contains(&"author"),
                "Should suggest 'author' from literal array, got {:?}",
                labels
            );
            // 'host' and 'port' appear in both, so they should be present
            assert!(
                labels.contains(&"host"),
                "Should suggest 'host', got {:?}",
                labels
            );
            assert!(
                labels.contains(&"port"),
                "Should suggest 'port', got {:?}",
                labels
            );
            // 'credentials' is ONLY in the @param — it must NOT appear
            assert!(
                !labels.contains(&"credentials"),
                "Must NOT suggest 'credentials' from inner-scope @param, got {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// The @param annotation inside a class should still work when the cursor
/// is inside that class method — scope-aware filtering must not break
/// the normal case.
#[tokio::test]
async fn test_array_shape_key_completion_inside_class_still_works() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_shape_scope_inner.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Demo {\n",
        "    /**\n",
        "     * @param array{host: string, port: int, ssl: bool} $options\n",
        "     */\n",
        "    public function connect(array $options): void {\n",
        "        $options['\n",
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

    // Cursor at $options[' inside the method (line 6, char 18)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 6,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for @param inside method"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
            assert!(
                labels.contains(&"host"),
                "Should suggest 'host', got {:?}",
                labels
            );
            assert!(
                labels.contains(&"port"),
                "Should suggest 'port', got {:?}",
                labels
            );
            assert!(
                labels.contains(&"ssl"),
                "Should suggest 'ssl', got {:?}",
                labels
            );
            assert_eq!(
                labels.len(),
                3,
                "Should have exactly 3 keys, got {:?}",
                labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ─── Literal Array Value Type → Member Access Tests ─────────────────────────

/// When a variable is assigned from a literal array and then a key is
/// accessed with `->`, the value type should resolve to the correct class.
/// e.g. `$result['user'] = new User(); $result['user']->` should complete
/// with User members.
#[tokio::test]
async fn test_array_shape_literal_value_type_member_access() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_literal_member.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public string $name;\n",
        "    public function getEmail(): string {}\n",
        "}\n",
        "\n",
        "$result = ['status' => 'ok'];\n",
        "$result['user'] = new User();\n",
        "$result['user']->\n",
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

    // Cursor right after `$result['user']->`  (line 8, char 18)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 8,
                character: 18,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    assert!(
        result.is_some(),
        "Should return completions for $result['user']->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                method_names.contains(&"getEmail"),
                "Should suggest User::getEmail(), got {:?}",
                method_names
            );
            let prop_labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                prop_labels.contains(&"name"),
                "Should suggest User::$name property, got {:?}",
                prop_labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Member access on a value type from an inline literal array (not
/// incremental assignment).
/// e.g. `$data = ['user' => new User()]; $data['user']->` should complete.
#[tokio::test]
async fn test_array_shape_inline_literal_value_type_member_access() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_inline_member.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Product {\n",
        "    public string $title;\n",
        "    public float $price;\n",
        "    public function getDescription(): string {}\n",
        "}\n",
        "\n",
        "$data = ['item' => new Product(), 'qty' => 5];\n",
        "$data['item']->\n",
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

    // Cursor right after `$data['item']->`  (line 8, char 15)
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
    assert!(
        result.is_some(),
        "Should return completions for $data['item']->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                method_names.contains(&"getDescription"),
                "Should suggest Product::getDescription(), got {:?}",
                method_names
            );
            let prop_labels: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                prop_labels.contains(&"title"),
                "Should suggest Product::$title, got {:?}",
                prop_labels
            );
            assert!(
                prop_labels.contains(&"price"),
                "Should suggest Product::$price, got {:?}",
                prop_labels
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

/// Scalar value types from literal arrays should NOT produce member
/// access completions (e.g. `$data['count']->` where count is int).
#[tokio::test]
async fn test_array_shape_literal_scalar_value_no_member_access() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_literal_scalar.php").unwrap();
    let text = concat!(
        "<?php\n",
        "$data = ['count' => 42, 'name' => 'hello'];\n",
        "$data['count']->\n",
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

    // Cursor right after `$data['count']->`  (line 2, char 17)
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line: 2,
                character: 17,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let result = backend.completion(completion_params).await.unwrap();
    // Should return no class members (int has no methods/properties).
    // The result may be None or contain only the fallback item.
    if let Some(CompletionResponse::Array(items)) = result {
        let class_members: Vec<&str> = items
            .iter()
            .filter(|i| {
                i.kind == Some(CompletionItemKind::METHOD)
                    || i.kind == Some(CompletionItemKind::PROPERTY)
            })
            .map(|i| i.label.as_str())
            .collect();
        assert!(
            class_members.is_empty(),
            "Scalar types should not produce member completions, got {:?}",
            class_members
        );
    }
}

/// Member access on a value type from an array inside a class method
/// using `$this->` return type.
#[tokio::test]
async fn test_array_shape_literal_value_type_inside_class() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///array_literal_class.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Logger {\n",
        "    public function info(): void {}\n",
        "    public function error(): void {}\n",
        "}\n",
        "class App {\n",
        "    public function run(): void {\n",
        "        $services = ['logger' => new Logger()];\n",
        "        $services['logger']->\n",
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

    // Cursor right after `$services['logger']->`  (line 8, char 29)
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
    assert!(
        result.is_some(),
        "Should return completions for $services['logger']->"
    );

    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();
            assert!(
                method_names.contains(&"info"),
                "Should suggest Logger::info(), got {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"error"),
                "Should suggest Logger::error(), got {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
