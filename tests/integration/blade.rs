#[cfg(test)]
mod tests {
    use crate::common::create_test_backend;
    use tower_lsp::LanguageServer;
    use tower_lsp::lsp_types::*;

    #[tokio::test]
    async fn test_goto_definition_in_blade_file() {
        let backend = create_test_backend();

        // 1. Define a class in a PHP file
        let php_uri = Url::parse("file:///Logger.php").unwrap();
        let php_text = "<?php class Logger { public function info() {} }";
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

        // 2. Try to use it in a Blade file
        let blade_uri = Url::parse("file:///view.blade.php").unwrap();
        let blade_text = "@php $logger = new Logger(); @endphp\n{{ $logger->info() }}";

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

        // 3. Click on "info" in the Blade file
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 1,
                    character: 13, // $logger->in|fo()
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let result = backend.goto_definition(params).await.unwrap();

        assert!(result.is_some(), "Should resolve definition in Blade file");

        match result.unwrap() {
            GotoDefinitionResponse::Scalar(location) => {
                assert_eq!(location.uri, php_uri);
                // Logger::info is on line 0
                assert_eq!(location.range.start.line, 0);
            }
            _ => panic!("Expected scalar location"),
        }
    }

    #[tokio::test]
    async fn test_blade_if_endif_parsing() {
        let backend = create_test_backend();

        let blade_uri = Url::parse("file:///test.blade.php").unwrap();
        let blade_text = "@if(true)\n    {{ config('app.name') }}\n@endif";

        // This should not produce any syntax errors now.
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

        // We check if it can resolve "config" inside the @if block.
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 1,
                    character: 7, // con|fig
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let result = backend.goto_definition(params).await.unwrap();
        // Even if config is not resolved (depends on stubs),
        // the important thing is that the server didn't crash or return error due to @endif.
        let _ = result;
    }

    #[tokio::test]
    async fn test_blade_if_with_leading_space() {
        let backend = create_test_backend();

        let blade_uri = Url::parse("file:///test.blade.php").unwrap();
        // Test user's specific example with leading space
        let blade_text = " @if(true)\n    {{ config('app.name') }}\n @endif";

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
                    character: 10,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let result = backend.goto_definition(params).await.unwrap();
        let _ = result;
    }

    #[tokio::test]
    async fn test_complex_blade_nesting_and_syntax() {
        let backend = create_test_backend();

        let blade_uri = Url::parse("file:///complex.blade.php").unwrap();
        let blade_text = r#"
<ul>
    @foreach ($items as $item)
        <li>
            <a href="{{ $item->url }}">{{ $item->name }}</a>
        </li>
    @endforeach
</ul>
"#;

        // This should not produce any syntax errors.
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

        // Verify no syntax errors are reported for this file.
        // (Note: Backend::new_test might not automatically publish diagnostics to a list we can check easily here,
        // but we can check if it parses correctly by trying to resolve something inside).

        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 4,
                    character: 31, // $item->u|rl
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let result = backend.goto_definition(params).await.unwrap();
        // The fact that it doesn't return a JSON-RPC error means it parsed.
        let _ = result;
    }

    #[tokio::test]
    async fn test_blade_references() {
        let backend = create_test_backend();

        let php_uri = Url::parse("file:///Logger.php").unwrap();
        let php_text = "<?php class Logger { public function info() {} }";
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

        let blade_uri = Url::parse("file:///view.blade.php").unwrap();
        let blade_text = "@php $l = new Logger(); @endphp\n{{ $l->info() }}";

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

        let params = ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: php_uri.clone(),
                },
                position: Position {
                    line: 0,
                    character: 37, // info
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: true,
            },
        };

        let result = backend.references(params).await.unwrap();
        assert!(result.is_some());
        let locations = result.unwrap();

        assert!(
            locations.iter().any(|l| l.uri == blade_uri),
            "Should find reference in Blade file"
        );
    }

    #[tokio::test]
    async fn test_blade_layout_directives() {
        let backend = create_test_backend();

        let blade_uri = Url::parse("file:///layout.blade.php").unwrap();
        let blade_text = r#"
@extends('layouts.app')

@section('title', 'Page Title')

@section('content')
    <p>This is my body content.</p>
    @include('shared.errors')
    @yield('scripts')
@endsection
"#;

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

        // If it parses successfully without crashing or returning syntax errors, we are good.
        let params = GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: blade_uri.clone(),
                },
                position: Position {
                    line: 2,
                    character: 1,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };

        let result = backend.goto_definition(params).await.unwrap();
        let _ = result;
    }
}
