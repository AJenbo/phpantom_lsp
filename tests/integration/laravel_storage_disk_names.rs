//! End-to-end coverage for config-backed Laravel storage disk names.

use crate::common::create_psr4_workspace;
use phpantom_lsp::Backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON: &str = r#"{
    "require": { "laravel/framework": "^12.0" },
    "autoload": { "psr-4": { "App\\": "app/" } }
}"#;

const FILESYSTEMS_CONFIG: &str = r#"<?php
return [
    'default' => 'local',
    'disks' => [
        'local' => ['driver' => 'local'],
        'archive' => ['driver' => 'local'],
        'backup' => ['driver' => 's3'],
    ],
];
"#;

fn position_after(content: &str, unique_prefix: &str) -> Position {
    let offset = content
        .find(unique_prefix)
        .unwrap_or_else(|| panic!("missing `{unique_prefix}`"))
        + unique_prefix.len();
    let before = &content[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = before
        .rsplit_once('\n')
        .map_or(before.len(), |(_, tail)| tail.len()) as u32;
    Position::new(line, character)
}

async fn open_workspace(source: &str) -> (Backend, tempfile::TempDir, Url) {
    let (backend, dir) = create_psr4_workspace(
        COMPOSER_JSON,
        &[
            ("config/filesystems.php", FILESYSTEMS_CONFIG),
            ("app/DiskConsumer.php", source),
        ],
    );
    backend.initialized(InitializedParams {}).await;

    let uri = Url::from_file_path(dir.path().join("app/DiskConsumer.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: source.to_string(),
            },
        })
        .await;

    (backend, dir, uri)
}

async fn completion_labels(backend: &Backend, uri: &Url, position: Position) -> Vec<String> {
    let response = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .expect("completion request should succeed");

    match response {
        Some(CompletionResponse::Array(items)) => {
            items.into_iter().map(|item| item.label).collect()
        }
        Some(CompletionResponse::List(list)) => {
            list.items.into_iter().map(|item| item.label).collect()
        }
        None => Vec::new(),
    }
}

fn definition_location(response: GotoDefinitionResponse) -> Location {
    match response {
        GotoDefinitionResponse::Scalar(location) => location,
        GotoDefinitionResponse::Array(mut locations) => locations.remove(0),
        GotoDefinitionResponse::Link(mut links) => {
            let link = links.remove(0);
            Location::new(link.target_uri, link.target_selection_range)
        }
    }
}

#[tokio::test]
async fn every_storage_disk_context_completes_direct_config_children() {
    let source = r#"<?php
use Illuminate\Support\Facades\Storage;

Storage::disk('');
Storage::fake('a');
Storage::persistentFake('ar');
Storage::forgetDisk('l');
Storage::forgetDisk(['local', 'b']);
Storage::forgetDisk(array('ar'));
#[\Illuminate\Container\Attributes\Storage('a')]
class DiskConsumer {}
"#;
    let (backend, _dir, uri) = open_workspace(source).await;

    let all = completion_labels(&backend, &uri, position_after(source, "Storage::disk('")).await;
    assert_eq!(all, ["archive", "backup", "local"]);

    for prefix in [
        "Storage::fake('a",
        "Storage::persistentFake('ar",
        "Attributes\\Storage('a",
    ] {
        let labels = completion_labels(&backend, &uri, position_after(source, prefix)).await;
        assert_eq!(labels, ["archive"], "completion at `{prefix}`");
    }

    let local = completion_labels(
        &backend,
        &uri,
        position_after(source, "Storage::forgetDisk('l"),
    )
    .await;
    assert_eq!(local, ["local"]);

    let array_value = completion_labels(
        &backend,
        &uri,
        position_after(source, "Storage::forgetDisk(['local', 'b"),
    )
    .await;
    assert_eq!(array_value, ["backup"]);

    let legacy_array_value = completion_labels(
        &backend,
        &uri,
        position_after(source, "Storage::forgetDisk(array('ar"),
    )
    .await;
    assert_eq!(legacy_array_value, ["archive"]);
}

#[tokio::test]
async fn storage_disk_names_navigate_and_hover_through_the_config_index() {
    let source = r#"<?php
use Illuminate\Support\Facades\Storage;

Storage::disk('archive');
Storage::fake('archive');
Storage::persistentFake('archive');
Storage::forgetDisk('archive');
Storage::forgetDisk(['archive', 'backup']);
#[\Illuminate\Container\Attributes\Storage('archive')]
class DiskConsumer {}
"#;
    let (backend, _dir, uri) = open_workspace(source).await;
    let positions = [
        position_after(source, "Storage::disk('arch"),
        position_after(source, "Storage::fake('arch"),
        position_after(source, "Storage::persistentFake('arch"),
        position_after(source, "Storage::forgetDisk('arch"),
        position_after(source, "Storage::forgetDisk(['arch"),
        position_after(source, "Attributes\\Storage('arch"),
    ];

    for position in positions {
        let response = backend
            .goto_definition(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await
            .expect("definition request should succeed")
            .expect("configured disk should have a definition");
        let location = definition_location(response);
        assert!(location.uri.path().ends_with("/config/filesystems.php"));
        assert_eq!(location.range.start.line, 5, "archive config key line");

        let hover = backend
            .handle_hover(uri.as_str(), source, position)
            .expect("configured disk should have hover");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("expected markdown hover");
        };
        assert!(markup.value.contains("**Storage disk** `archive`"));
        assert!(markup.value.contains("config/filesystems.php"));
    }
}

#[tokio::test]
async fn only_disk_lookup_requires_a_preconfigured_name() {
    let source = r#"<?php
use Illuminate\Support\Facades\Storage;

Storage::disk('missing');
Storage::fake('testing');
Storage::persistentFake('persistent-testing');
Storage::forgetDisk('already-forgotten');
"#;
    let (backend, _dir, uri) = open_workspace(source).await;
    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), source, &mut diagnostics);

    let invalid_disks: Vec<_> = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                &diagnostic.code,
                Some(NumberOrString::String(code)) if code == "invalid_laravel_storage_disk"
            )
        })
        .collect();
    assert_eq!(invalid_disks.len(), 1, "got: {invalid_disks:#?}");
    assert!(invalid_disks[0].message.contains("storage disk: 'missing'"));
}

#[tokio::test]
async fn storage_call_shapes_share_references_with_the_config_declaration() {
    let source = r#"<?php
use Illuminate\Support\Facades\Storage;

Storage::disk('archive');
Storage::fake('archive');
Storage::persistentFake('archive');
Storage::forgetDisk('archive');
Storage::forgetDisk(['archive', 'backup']);
config('filesystems.disks.archive');
#[\Illuminate\Container\Attributes\Storage('archive')]
class DiskConsumer {}
"#;
    let (backend, _dir, uri) = open_workspace(source).await;
    let references = backend
        .find_references(
            uri.as_str(),
            source,
            position_after(source, "Storage::fake('arch"),
            true,
        )
        .expect("disk should have references");

    assert_eq!(
        references.len(),
        8,
        "six call usages, one attribute usage, and one declaration: {references:#?}"
    );
    assert_eq!(
        references
            .iter()
            .filter(|location| location.uri == uri)
            .count(),
        7
    );
    assert!(
        references
            .iter()
            .any(|location| location.uri.path().ends_with("/config/filesystems.php"))
    );
}

#[tokio::test]
async fn framework_default_disks_keep_their_exact_definition_and_reference() {
    let framework_config = r#"<?php
return [
    'disks' => [
        'framework-local' => ['driver' => 'local'],
    ],
];
"#;
    let app_config = r#"<?php
return [
    'disks' => [
        'application' => ['driver' => 'local'],
    ],
];
"#;
    let source = r#"<?php
use Illuminate\Support\Facades\Storage;

Storage::disk('framework-local');
Storage::disk('missing');
"#;
    let (backend, dir) = create_psr4_workspace(
        COMPOSER_JSON,
        &[
            ("config/filesystems.php", app_config),
            (
                "vendor/laravel/framework/config/filesystems.php",
                framework_config,
            ),
            ("app/DiskConsumer.php", source),
        ],
    );
    backend.initialized(InitializedParams {}).await;
    let uri = Url::from_file_path(dir.path().join("app/DiskConsumer.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: source.to_string(),
            },
        })
        .await;

    let labels = completion_labels(
        &backend,
        &uri,
        position_after(source, "Storage::disk('framework"),
    )
    .await;
    assert_eq!(labels, ["framework-local"]);

    let definition = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                position: position_after(source, "Storage::disk('framework"),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .expect("definition request should succeed")
        .expect("framework disk should resolve");
    let location = definition_location(definition);
    assert!(
        location
            .uri
            .path()
            .ends_with("/vendor/laravel/framework/config/filesystems.php")
    );
    assert_eq!(location.range.start.line, 3);

    let references = backend
        .find_references(
            uri.as_str(),
            source,
            position_after(source, "Storage::disk('framework"),
            true,
        )
        .expect("framework disk should have references");
    assert_eq!(references.len(), 2, "usage and framework declaration");

    let missing_position = position_after(source, "Storage::disk('miss");
    assert!(
        backend
            .goto_definition(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                    position: missing_position,
                },
                work_done_progress_params: WorkDoneProgressParams::default(),
                partial_result_params: PartialResultParams::default(),
            })
            .await
            .expect("definition request should succeed")
            .is_none(),
        "an unknown disk must not fall back to the config file header"
    );
    let hover = backend
        .handle_hover(uri.as_str(), source, missing_position)
        .expect("unknown disk should retain family hover");
    let HoverContents::Markup(markup) = hover.contents else {
        panic!("expected markdown hover");
    };
    assert!(!markup.value.contains("Defined in"), "{}", markup.value);
}
