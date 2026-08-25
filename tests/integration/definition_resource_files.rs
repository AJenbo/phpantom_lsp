use crate::common::create_psr4_workspace;
use phpantom_lsp::Backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER: &str = r#"{
    "autoload": { "psr-4": { "App\\": "src/" } }
}"#;

async fn open_resource(backend: &Backend, uri: Url, language_id: &str, content: &str) {
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: language_id.to_string(),
                version: 1,
                text: content.to_string(),
            },
        })
        .await;
}

fn position_in(content: &str, needle: &str, inside: usize) -> Position {
    let offset = content.find(needle).expect("needle should exist") + inside;
    let prefix = &content[..offset];
    Position::new(
        prefix.bytes().filter(|byte| *byte == b'\n').count() as u32,
        prefix
            .rsplit_once('\n')
            .map_or(prefix.len(), |(_, line)| line.len()) as u32,
    )
}

async fn definition_at(
    backend: &Backend,
    uri: Url,
    content: &str,
    needle: &str,
    inside: usize,
) -> Option<GotoDefinitionResponse> {
    backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: position_in(content, needle, inside),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .expect("definition request should succeed")
}

#[tokio::test]
async fn navigates_php_classes_from_arbitrary_yaml_keys_and_values() {
    let php = "<?php\nnamespace App\\UseCase;\nclass DeletePlaylist {}\n";
    let yaml = concat!(
        "App\\UseCase\\DeletePlaylist:\n",
        "  arbitrary-name: App\\UseCase\\DeletePlaylist\n",
        "  x-usecase: App\\UseCase\\DeletePlaylist\n",
    );
    let (backend, dir) = create_psr4_workspace(
        COMPOSER,
        &[
            ("src/UseCase/DeletePlaylist.php", php),
            ("schema/paths/playlists.yaml", yaml),
        ],
    );
    let yaml_uri = Url::from_file_path(dir.path().join("schema/paths/playlists.yaml")).unwrap();
    open_resource(&backend, yaml_uri.clone(), "yaml", yaml).await;

    for (needle, inside) in [
        ("App\\UseCase\\DeletePlaylist:", 18),
        ("arbitrary-name: App\\UseCase\\DeletePlaylist", 27),
        ("x-usecase: App\\UseCase\\DeletePlaylist", 22),
    ] {
        let result = definition_at(&backend, yaml_uri.clone(), yaml, needle, inside)
            .await
            .expect("class should resolve from YAML");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected one class definition");
        };
        assert!(
            location
                .uri
                .path()
                .ends_with("/src/UseCase/DeletePlaylist.php")
        );
        assert_eq!(location.range.start.line, 2);
    }
}

#[tokio::test]
async fn navigates_php_classes_from_xml_attributes_and_text() {
    let php = "<?php\nnamespace App\\Handler;\nclass Run {}\n";
    let xml = r#"<root handler="App\Handler\Run"><target>App\Handler\Run</target></root>"#;
    let (backend, dir) = create_psr4_workspace(
        COMPOSER,
        &[("src/Handler/Run.php", php), ("config/arbitrary.xml", xml)],
    );
    let xml_uri = Url::from_file_path(dir.path().join("config/arbitrary.xml")).unwrap();
    open_resource(&backend, xml_uri.clone(), "xml", xml).await;

    for needle in ["handler=\"App\\Handler\\Run", ">App\\Handler\\Run"] {
        let result = definition_at(&backend, xml_uri.clone(), xml, needle, needle.len() - 2)
            .await
            .expect("class should resolve from XML");
        let GotoDefinitionResponse::Scalar(location) = result else {
            panic!("expected one class definition");
        };
        assert!(location.uri.path().ends_with("/src/Handler/Run.php"));
    }
}

#[tokio::test]
async fn navigates_class_members_and_yaml_escaped_class_names() {
    let php = concat!(
        "<?php\n",
        "namespace App\\Handler;\n",
        "class Run {\n",
        "    public function handle(): void {}\n",
        "}\n",
    );
    let yaml = r#"callback: "App\\Handler\\Run::handle""#;
    let (backend, dir) = create_psr4_workspace(
        COMPOSER,
        &[("src/Handler/Run.php", php), ("config/callbacks.yml", yaml)],
    );
    let yaml_uri = Url::from_file_path(dir.path().join("config/callbacks.yml")).unwrap();
    open_resource(&backend, yaml_uri.clone(), "yaml", yaml).await;

    let class_result = definition_at(&backend, yaml_uri.clone(), yaml, "App", 1)
        .await
        .expect("escaped class should resolve");
    let GotoDefinitionResponse::Scalar(class_location) = class_result else {
        panic!("expected one class definition");
    };
    assert_eq!(class_location.range.start.line, 2);

    let member_result = definition_at(&backend, yaml_uri, yaml, "handle", 2)
        .await
        .expect("class member should resolve");
    let GotoDefinitionResponse::Scalar(member_location) = member_result else {
        panic!("expected one member definition");
    };
    assert_eq!(member_location.range.start.line, 3);
}

#[tokio::test]
async fn unknown_and_unqualified_names_do_not_navigate() {
    let yaml = "short: DeletePlaylist\nunknown: App\\Missing\\DeletePlaylist\n";
    let (backend, dir) = create_psr4_workspace(COMPOSER, &[("config/classes.yaml", yaml)]);
    let yaml_uri = Url::from_file_path(dir.path().join("config/classes.yaml")).unwrap();
    open_resource(&backend, yaml_uri.clone(), "yaml", yaml).await;

    assert!(
        definition_at(&backend, yaml_uri.clone(), yaml, "DeletePlaylist", 2)
            .await
            .is_none()
    );
    assert!(
        definition_at(&backend, yaml_uri, yaml, "App\\Missing", 4)
            .await
            .is_none()
    );
}
