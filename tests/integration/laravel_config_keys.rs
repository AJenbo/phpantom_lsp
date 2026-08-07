//! Tests for the config-key diagnostic.
//!
//! A library package ships no `config/` directory of its own: its keys come
//! from the host application's config files, which we never see.  Keys under
//! such a root are unjudgeable, while a typo inside a config file we did read
//! must still be reported.  `Config::set()` is a write: it declares the key it
//! names, so there is nothing to check it against either.

use crate::common::create_psr4_workspace;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON: &str = r#"{
    "require": { "laravel/framework": "^11.0" },
    "autoload": { "psr-4": { "App\\": "src/" } }
}"#;

const APP_CONFIG: &str = "\
<?php
return [
    'name' => 'Acme',
    'timezone' => 'UTC',
];
";

const FILESYSTEMS_CONFIG: &str = "\
<?php
return [
    'disks' => [
        'local' => ['driver' => 'local'],
    ],
];
";

const CONSUMER: &str = "\
<?php
namespace App;

use Illuminate\\Support\\Facades\\Config;

class Settings {
    public function demo(): void {
        config('app.name');
        config('acme.gateway.key');
        Config::set('filesystems.disks.ondemand', ['driver' => 'local']);
        config('app.tzimezone');
    }
}
";

async fn config_diagnostics() -> Vec<String> {
    let (backend, dir) = create_psr4_workspace(
        COMPOSER_JSON,
        &[
            ("config/app.php", APP_CONFIG),
            ("config/filesystems.php", FILESYSTEMS_CONFIG),
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

    let mut diags = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), CONSUMER, &mut diags);

    diags
        .iter()
        .filter(
            |d| matches!(&d.code, Some(NumberOrString::String(s)) if s == "invalid_laravel_config"),
        )
        .map(|d| d.message.clone())
        .collect()
}

#[tokio::test]
async fn only_a_typo_in_a_config_file_we_read_is_reported() {
    let messages = config_diagnostics().await;

    assert_eq!(
        messages.len(),
        1,
        "an unknown root file and a written key are unjudgeable, got: {messages:?}"
    );
    assert!(
        messages[0].contains("app.tzimezone"),
        "the flagged key should be the typo, got: {}",
        messages[0]
    );
}
