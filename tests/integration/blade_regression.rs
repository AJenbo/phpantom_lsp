#[cfg(test)]
mod tests {
    use crate::common::create_test_backend;
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::*;

    #[tokio::test]
    async fn test_blade_regression_sentry() {
        let backend = create_test_backend();
        let blade_uri = Url::parse("file:///sentry.blade.php").unwrap();
        let blade_text =
            std::fs::read_to_string("tests/fixtures/blade_regression_1.blade.php").unwrap();

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

        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 1,
                    character: 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let _ = backend.goto_definition(params).await.unwrap();

        let (virtual_php, _) = phpantom_lsp::blade::preprocessor::preprocess(&blade_text);
        println!("VIRTUAL PHP SENTRY:\n{}", virtual_php);
    }

    #[tokio::test]
    async fn test_blade_regression_sitemap() {
        let backend = create_test_backend();
        let blade_uri = Url::parse("file:///sitemap.blade.php").unwrap();
        let blade_text =
            std::fs::read_to_string("tests/fixtures/blade_regression_2.blade.php").unwrap();

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

        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 1,
                    character: 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let _ = backend.goto_definition(params).await.unwrap();

        // Print the preprocessed PHP
        let (virtual_php, _) = phpantom_lsp::blade::preprocessor::preprocess(&blade_text);
        println!("VIRTUAL PHP:\n{}", virtual_php);
    }
}
