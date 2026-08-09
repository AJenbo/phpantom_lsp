//! BL7: Blade directive-name completion (`docs/todo/blade.md`).

use crate::common::create_test_backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

async fn open_blade(backend: &phpantom_lsp::Backend, uri: &Url, text: &str) {
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "blade".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;
}

async fn complete_at(
    backend: &phpantom_lsp::Backend,
    uri: &Url,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position { line, character },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: Some(CompletionContext {
            trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
            trigger_character: Some("@".to_string()),
        }),
    };

    match backend.completion(params).await.unwrap() {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        None => Vec::new(),
    }
}

#[tokio::test]
async fn at_sign_in_html_position_offers_all_known_directives() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///page.blade.php").unwrap();
    open_blade(&backend, &uri, "<div>@</div>").await;

    let items = complete_at(&backend, &uri, 0, 6).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(
        labels.contains(&"@if") && labels.contains(&"@foreach") && labels.contains(&"@endif"),
        "expected the full known-directive list, got: {:?}",
        labels
    );
}

#[tokio::test]
async fn the_if_completion_inserts_the_documented_snippet() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///page.blade.php").unwrap();
    open_blade(&backend, &uri, "<div>@</div>").await;

    let items = complete_at(&backend, &uri, 0, 6).await;
    let if_item = items
        .iter()
        .find(|i| i.label == "@if")
        .expect("expected an @if completion item");

    assert_eq!(
        if_item.insert_text.as_deref(),
        Some("if ($1)\n\t$0\n@endif")
    );
    assert_eq!(if_item.insert_text_format, Some(InsertTextFormat::SNIPPET));
}

#[tokio::test]
async fn a_partial_directive_name_filters_the_list() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///page.blade.php").unwrap();
    open_blade(&backend, &uri, "<div>@for</div>").await;

    // Cursor right after "@for".
    let items = complete_at(&backend, &uri, 0, 9).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();

    assert!(labels.contains(&"@for"), "got: {:?}", labels);
    assert!(labels.contains(&"@foreach"), "got: {:?}", labels);
    assert!(labels.contains(&"@forelse"), "got: {:?}", labels);
    assert!(
        !labels.contains(&"@if"),
        "'@if' does not start with 'for', got: {:?}",
        labels
    );
}

#[tokio::test]
async fn an_unknown_directive_name_still_short_circuits_with_an_empty_list() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///page.blade.php").unwrap();
    // `zzz` matches no directive, but the position is still an HTML/
    // directive-name position, so the strategy must not fall through to
    // (e.g.) class-name completion.
    open_blade(&backend, &uri, "<div>@zzz</div>").await;

    let items = complete_at(&backend, &uri, 0, 9).await;
    assert!(
        items.is_empty(),
        "expected an empty (short-circuited) list, got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn no_directive_completion_inside_echo_braces() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///page.blade.php").unwrap();
    open_blade(&backend, &uri, "{{ @ }}").await;

    // Cursor right after "@" inside `{{ ... }}`.
    let items = complete_at(&backend, &uri, 0, 4).await;
    assert!(
        items.is_empty(),
        "directive completion must not fire inside {{{{ }}}}, got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn no_directive_completion_inside_a_php_block() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///page.blade.php").unwrap();
    open_blade(&backend, &uri, "@php $x = 1; @\n@endphp").await;

    // Cursor right after the trailing "@" on the first line, still inside
    // the `@php ... @endphp` block.
    let items = complete_at(&backend, &uri, 0, 14).await;
    assert!(
        items.is_empty(),
        "directive completion must not fire inside a @php block, got: {:?}",
        items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn directive_completion_still_fires_inside_an_open_block() {
    let backend = create_test_backend();
    let uri = Url::parse("file:///page.blade.php").unwrap();
    // The body between `@if` and `@endif` is ordinary template markup, so
    // a nested directive must still complete.
    open_blade(&backend, &uri, "@if ($x)\n    @\n@endif").await;

    let items = complete_at(&backend, &uri, 1, 5).await;
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    assert!(
        labels.contains(&"@foreach"),
        "expected nested directive completion inside @if/@endif, got: {:?}",
        labels
    );
}
