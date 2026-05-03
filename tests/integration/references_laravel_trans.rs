use crate::common::create_psr4_workspace;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON: &str = r#"{
    "autoload": {
        "psr-4": {
            "App\\": "src/"
        }
    }
}"#;

#[tokio::test]
async fn test_find_references_laravel_trans_lists_all_locales() {
    let app_php = r#"<?php
namespace App;

class Test {
    public function demo() {
        return trans('messages.welcome');
    }
}
"#;

    let lang_en_messages = r#"<?php
return [
    'welcome' => 'Welcome',
];
"#;

    let lang_zh_messages = r#"<?php
return [
    'welcome' => '歡迎',
];
"#;

    let files = vec![
        ("src/Test.php", app_php),
        ("lang/en/messages.php", lang_en_messages),
        ("lang/zh_TW/messages.php", lang_zh_messages),
    ];

    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, &files);

    let app_uri = Url::from_file_path(dir.path().join("src/Test.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: app_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: app_php.to_string(),
            },
        })
        .await;

    // Find references for 'messages.welcome' in app.php
    // Line 5, 'messages.welcome' starts at char 22
    let params = ReferenceParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: app_uri },
            position: Position::new(5, 25),
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: ReferenceContext {
            include_declaration: true,
        },
    };

    let results = backend.references(params).await.unwrap().unwrap();

    println!("Found {} references:", results.len());
    for (i, loc) in results.iter().enumerate() {
        println!("  [{}] {}", i, loc.uri);
    }

    // Current behavior (expected to fail if my hypothesis is correct):
    // It should find:
    // 1. Usage in src/Test.php
    // 2. Definition in lang/en/messages.php
    // 3. Definition in lang/zh_TW/messages.php <--- This is what's likely missing

    let uris: Vec<String> = results.iter().map(|l| l.uri.to_string()).collect();

    assert!(
        uris.iter().any(|u| u.contains("Test.php")),
        "Should find usage in Test.php"
    );
    assert!(
        uris.iter().any(|u| u.contains("en/messages.php")),
        "Should find English definition"
    );
    assert!(
        uris.iter().any(|u| u.contains("zh_TW/messages.php")),
        "Should find Traditional Chinese definition"
    );
}
