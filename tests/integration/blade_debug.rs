#[cfg(test)]
mod tests {
    use crate::common::create_test_backend;
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::*;

    #[tokio::test]
    async fn test_blade_hover_range_translated_back() {
        let backend = create_test_backend();

        let php_uri = Url::parse("file:///BlogAuthor.php").unwrap();
        let php_text =
            "<?php namespace App\\Models; class BlogAuthor { public string $name = ''; }";
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: php_uri.clone(),
                    language_id: "php".to_string(),
                    version: 1,
                    text: php_text.to_string(),
                },
            })
            .await;

        let blade_uri = Url::parse("file:///email.blade.php").unwrap();
        // Line 0: @php
        // Line 1: /**
        // Line 2:  * @var \App\Models\BlogAuthor $author
        // Line 3:  */
        // Line 4: @endphp
        // Line 5: (empty)
        // Line 6: <p>{{ $author->name }}</p>
        let blade_text = "@php\n/**\n * @var \\App\\Models\\BlogAuthor $author\n */\n@endphp\n\n<p>{{ $author->name }}</p>";

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: blade_uri.clone(),
                    language_id: "blade".to_string(),
                    version: 1,
                    text: blade_text.to_string(),
                },
            })
            .await;

        // Hover on $author (line 6, character 6 = the '$' of $author)
        let params = HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 6,
                    character: 6,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        };

        let result = backend.hover(params).await.unwrap();
        assert!(result.is_some(), "Should return hover for $author");

        let hover = result.unwrap();
        // The range should be in Blade space (line 6), not PHP virtual space (line 11)
        if let Some(range) = hover.range {
            assert_eq!(
                range.start.line, 6,
                "Hover range start line should be in Blade space (6), got {}",
                range.start.line
            );
        }
    }

    #[tokio::test]
    async fn test_blade_completion_with_var_annotation() {
        let backend = create_test_backend();

        let php_uri = Url::parse("file:///BlogAuthor.php").unwrap();
        let php_text = "<?php namespace App\\Models; class BlogAuthor { public string $name = ''; public int $age = 0; }";
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: php_uri.clone(),
                    language_id: "php".to_string(),
                    version: 1,
                    text: php_text.to_string(),
                },
            })
            .await;

        let blade_uri = Url::parse("file:///email.blade.php").unwrap();
        let blade_text = "@php\n/**\n * @var \\App\\Models\\BlogAuthor $author\n */\n@endphp\n\n<p>{{ $author-> }}</p>";

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: blade_uri.clone(),
                    language_id: "blade".to_string(),
                    version: 1,
                    text: blade_text.to_string(),
                },
            })
            .await;

        // Complete after "$author->" on line 6
        // "<p>{{ $author-> }}</p>"
        //  0123456789012345
        let params = CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 6,
                    character: 15, // after "$author->"
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: Some(">".to_string()),
            }),
        };

        let result = backend.completion(params).await.unwrap();
        assert!(result.is_some(), "Should return completions for $author->");

        let items = match result.unwrap() {
            CompletionResponse::Array(items) => items,
            CompletionResponse::List(list) => list.items,
        };

        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert!(
            labels.contains(&"name"),
            "Should complete 'name' property, got: {:?}",
            labels
        );
        assert!(
            labels.contains(&"age"),
            "Should complete 'age' property, got: {:?}",
            labels
        );
    }

    #[tokio::test]
    async fn test_blade_goto_definition_member() {
        let backend = create_test_backend();

        let php_uri = Url::parse("file:///BlogAuthor.php").unwrap();
        let php_text =
            "<?php namespace App\\Models; class BlogAuthor { public string $name = ''; }";
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: php_uri.clone(),
                    language_id: "php".to_string(),
                    version: 1,
                    text: php_text.to_string(),
                },
            })
            .await;

        let blade_uri = Url::parse("file:///email.blade.php").unwrap();
        let blade_text = "@php\n/**\n * @var \\App\\Models\\BlogAuthor $author\n */\n@endphp\n\n<p>{{ $author->name }}</p>";

        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: blade_uri.clone(),
                    language_id: "blade".to_string(),
                    version: 1,
                    text: blade_text.to_string(),
                },
            })
            .await;

        // GTD on "name" in "$author->name" on line 6
        // "<p>{{ $author->name }}</p>"
        //  0123456789012345678
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 6,
                    character: 16, // "name"
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let result = backend.goto_definition(params).await.unwrap();
        assert!(
            result.is_some(),
            "Should resolve definition for ->name in Blade file"
        );
    }
}
