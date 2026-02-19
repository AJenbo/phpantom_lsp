mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

#[tokio::test]
async fn test_guard_clause_negated_instanceof_return_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_neg_instanceof.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            return;\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's method 'bark' after guard clause, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's method 'purr' after guard clause, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_positive_instanceof_return_excludes() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_pos_instanceof.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if ($pet instanceof Dog) {\n",
        "            return;\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"purr"),
                "Should include Cat's method 'purr' after guard excludes Dog, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bark"),
                "Should NOT include Dog's method 'bark' after guard excludes Dog, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_throw_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_throw.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getEmail(): string { return ''; }\n",
        "}\n",
        "class Admin {\n",
        "    public function getRole(): string { return ''; }\n",
        "}\n",
        "class Svc {\n",
        "    /** @param User|Admin $u */\n",
        "    public function test($u): void {\n",
        "        if ($u instanceof Admin) {\n",
        "            throw new \\Exception('no admins');\n",
        "        }\n",
        "        $u->\n",
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
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getEmail"),
                "Should include User's 'getEmail' after throw guard, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getRole"),
                "Should NOT include Admin's 'getRole' after throw guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_multiple_sequential_guards() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_multiple.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Alpha {\n",
        "    public function alphaMethod(): void {}\n",
        "}\n",
        "class Beta {\n",
        "    public function betaMethod(): void {}\n",
        "}\n",
        "class Gamma {\n",
        "    public function gammaMethod(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Alpha|Beta|Gamma $obj */\n",
        "    public function test($obj): void {\n",
        "        if ($obj instanceof Alpha) {\n",
        "            return;\n",
        "        }\n",
        "        if ($obj instanceof Beta) {\n",
        "            return;\n",
        "        }\n",
        "        $obj->\n",
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
                    line: 19,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"gammaMethod"),
                "Should include Gamma's method after two guard clauses, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"alphaMethod"),
                "Should NOT include Alpha's method after guard, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"betaMethod"),
                "Should NOT include Beta's method after guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_no_narrowing_when_body_does_not_exit() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_no_exit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            echo 'not a dog';\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Body doesn't exit, so no guard clause narrowing applies
            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' — body doesn't exit, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"purr"),
                "Should include Cat's 'purr' — body doesn't exit, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_no_narrowing_when_else_exists() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_with_else.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            return;\n",
        "        } else {\n",
        "            echo 'is dog';\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Guard clause narrowing should NOT apply when else exists
            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' — else exists so no guard, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"purr"),
                "Should include Cat's 'purr' — else exists so no guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_top_level_function() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_top_level.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getEmail(): string { return ''; }\n",
        "}\n",
        "class Guest {\n",
        "    public function getSession(): string { return ''; }\n",
        "}\n",
        "/** @param User|Guest $person */\n",
        "function process($person): void {\n",
        "    if (!$person instanceof User) {\n",
        "        return;\n",
        "    }\n",
        "    $person->\n",
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
                    character: 13,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getEmail"),
                "Should include User's 'getEmail' after guard in top-level function, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"getSession"),
                "Should NOT include Guest's 'getSession' after guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_cross_file() {
    let (backend, _dir) = create_psr4_workspace(
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
        &[
            (
                "src/Animal.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Animal {\n",
                    "    public function breathe(): void {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Dog.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Dog extends Animal {\n",
                    "    public function bark(): void {}\n",
                    "}\n",
                ),
            ),
            (
                "src/Cat.php",
                concat!(
                    "<?php\n",
                    "namespace App;\n",
                    "class Cat extends Animal {\n",
                    "    public function purr(): void {}\n",
                    "}\n",
                ),
            ),
        ],
    );

    let uri = Url::parse("file:///guard_cross_file.php").unwrap();
    let text = concat!(
        "<?php\n",
        "use App\\Dog;\n",
        "use App\\Cat;\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            return;\n",
        "        }\n",
        "        $pet->\n",
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
                    line: 9,
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after guard (cross-file), got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"breathe"),
                "Should include inherited 'breathe' after guard (cross-file), got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after guard (cross-file), got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_parenthesised_condition() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_parens.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!($pet instanceof Dog)) {\n",
        "            return;\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after parenthesised guard, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after parenthesised guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_is_a_narrows() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_is_a.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!is_a($pet, Dog::class)) {\n",
        "            return;\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after is_a guard clause, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after is_a guard clause, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_no_narrowing_inside_if_body() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_inside_body.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            $pet->\n",
        "        }\n",
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
                    character: 18,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Inside the if body: $pet is NOT Dog, so should see Cat's methods
            assert!(
                method_names.contains(&"purr"),
                "Inside negated instanceof block should include Cat's 'purr', got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"bark"),
                "Inside negated instanceof block should NOT include Dog's 'bark', got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_mixed_type_parameter() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_mixed.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class User {\n",
        "    public function getEmail(): string { return ''; }\n",
        "}\n",
        "class Svc {\n",
        "    public function process(mixed $value): void {\n",
        "        if (!$value instanceof User) {\n",
        "            return;\n",
        "        }\n",
        "        $value->\n",
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
                    line: 9,
                    character: 16,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"getEmail"),
                "Should include User's 'getEmail' after guard on mixed param, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_with_block_body() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_block.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            $x = 1;\n",
        "            $y = 2;\n",
        "            return;\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' — block ends with return, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' — block ends with return, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_no_narrowing_when_block_does_not_end_with_exit() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_block_no_exit.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            $x = 1;\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            // Block doesn't end with exit, so no narrowing
            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' — block doesn't exit, got: {:?}",
                method_names
            );
            assert!(
                method_names.contains(&"purr"),
                "Should include Cat's 'purr' — block doesn't exit, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_single_statement_no_braces() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_no_braces.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) return;\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after braceless guard, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after braceless guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_this_property() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_this_prop.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (!$pet instanceof Dog) {\n",
        "            throw new \\RuntimeException('Expected Dog');\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after throw guard, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after throw guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_guard_clause_get_class_identity_check() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///guard_get_class.php").unwrap();
    let text = concat!(
        "<?php\n",
        "class Dog {\n",
        "    public function bark(): void {}\n",
        "}\n",
        "class Cat {\n",
        "    public function purr(): void {}\n",
        "}\n",
        "class Svc {\n",
        "    /** @param Dog|Cat $pet */\n",
        "    public function test($pet): void {\n",
        "        if (get_class($pet) !== Dog::class) {\n",
        "            return;\n",
        "        }\n",
        "        $pet->\n",
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
                    character: 14,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(result.is_some(), "Should return completions");
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let method_names: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap())
                .collect();

            assert!(
                method_names.contains(&"bark"),
                "Should include Dog's 'bark' after get_class guard, got: {:?}",
                method_names
            );
            assert!(
                !method_names.contains(&"purr"),
                "Should NOT include Cat's 'purr' after get_class guard, got: {:?}",
                method_names
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}
