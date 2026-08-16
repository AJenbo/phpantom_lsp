//! Regression tests for gating Laravel string-key hover, go-to-definition,
//! find-references, and rename on `is_laravel`.
//!
//! `SymbolKind::LaravelStringKey` spans are extracted by pure name match
//! (`config()`, `route()`, `view()`, `__()`, `trans()`, …) with no project
//! context, so a non-Laravel project that happens to declare its own
//! `config()` function (common in home-grown micro-frameworks) must not get
//! fabricated "Config key" hovers, go-to-definition jumps, or find-references
//! results against a `config/*.php` file that happens to sit on disk.

use crate::common::create_psr4_workspace;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON_NON_LARAVEL: &str = r#"{
    "autoload": { "psr-4": { "App\\": "src/" } }
}"#;

const COMPOSER_JSON_LARAVEL: &str = r#"{
    "require": { "laravel/framework": "^11.0" },
    "autoload": { "psr-4": { "App\\": "src/" } }
}"#;

const APP_CONFIG: &str = "\
<?php
return [
    'name' => 'Acme',
];
";

const CONSUMER: &str = "\
<?php
namespace App;

class Settings {
    public function demo(): void {
        config('app.name');
        config('app.name');
    }
}
";

async fn setup(composer_json: &str) -> (phpantom_lsp::Backend, tempfile::TempDir, Url) {
    let (backend, dir) = create_psr4_workspace(
        composer_json,
        &[
            ("config/app.php", APP_CONFIG),
            ("src/Settings.php", CONSUMER),
        ],
    );
    backend.initialized(InitializedParams {}).await;

    let uri = Url::from_file_path(dir.path().join("src/Settings.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: CONSUMER.to_string(),
            },
        })
        .await;

    (backend, dir, uri)
}

/// Lands inside the `'app.name'` string literal on the first `config()`
/// call (line 5, 0-based) in `CONSUMER`.
const KEY_POSITION: Position = Position {
    line: 5,
    character: 20,
};

#[tokio::test]
async fn non_laravel_project_hover_does_not_fabricate_config_key() {
    let (backend, _dir, uri) = setup(COMPOSER_JSON_NON_LARAVEL).await;

    let hover = backend
        .hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: KEY_POSITION,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })
        .await
        .unwrap();

    assert!(
        hover.is_none(),
        "a non-Laravel project's own config() must not hover as a config key, got {hover:?}"
    );
}

#[tokio::test]
async fn laravel_project_hover_still_shows_config_key() {
    let (backend, _dir, uri) = setup(COMPOSER_JSON_LARAVEL).await;

    let hover = backend
        .hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: KEY_POSITION,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })
        .await
        .unwrap();

    assert!(
        hover.is_some(),
        "a real Laravel project should still hover config('app.name')"
    );
}

#[tokio::test]
async fn non_laravel_project_definition_does_not_jump_to_config_file() {
    let (backend, _dir, uri) = setup(COMPOSER_JSON_NON_LARAVEL).await;

    let result = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: KEY_POSITION,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .unwrap();

    assert!(
        result.is_none(),
        "a non-Laravel project's own config() must not jump to config/app.php, got {result:?}"
    );
}

#[tokio::test]
async fn non_laravel_project_references_does_not_fabricate_config_key_refs() {
    let (backend, _dir, uri) = setup(COMPOSER_JSON_NON_LARAVEL).await;

    let result = backend
        .references(ReferenceParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: KEY_POSITION,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: ReferenceContext {
                include_declaration: true,
            },
        })
        .await
        .unwrap()
        .unwrap_or_default();

    assert!(
        result.is_empty(),
        "a non-Laravel project's own config() must not produce Laravel string-key \
         references, got {result:?}"
    );
}

#[tokio::test]
async fn non_laravel_project_prepare_rename_rejects_config_key() {
    let (backend, _dir, uri) = setup(COMPOSER_JSON_NON_LARAVEL).await;

    let result = backend
        .prepare_rename(TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: KEY_POSITION,
        })
        .await
        .unwrap();

    assert!(
        result.is_none(),
        "a non-Laravel project's own config() must not be renameable as a config key, \
         got {result:?}"
    );
}

#[tokio::test]
async fn non_laravel_project_rename_does_not_rewrite_config_key() {
    let (backend, _dir, uri) = setup(COMPOSER_JSON_NON_LARAVEL).await;

    let result = backend
        .rename(RenameParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: KEY_POSITION,
            },
            new_name: "renamed.key".to_string(),
            work_done_progress_params: WorkDoneProgressParams::default(),
        })
        .await
        .unwrap();

    assert!(
        result.is_none(),
        "a non-Laravel project's own config() must not be renamed as a config key, \
         got {result:?}"
    );
}
