//! Integration tests for resolving `Storage::disk()` / `FilesystemManager`'s
//! disk-returning methods from the declared `Filesystem`/`Cloud` contract to
//! the concrete `FilesystemAdapter`, based on `config/filesystems.php`.

use crate::common::create_psr4_workspace;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON: &str = r#"{
    "autoload": {
        "psr-4": {
            "App\\": "src/",
            "Illuminate\\Contracts\\Filesystem\\": "vendor/illuminate/Contracts/Filesystem/",
            "Illuminate\\Filesystem\\": "vendor/illuminate/Filesystem/",
            "Illuminate\\Support\\Facades\\": "vendor/illuminate/Support/Facades/"
        }
    }
}"#;

const FILESYSTEM_CONTRACT_PHP: &str = "\
<?php
namespace Illuminate\\Contracts\\Filesystem;
interface Filesystem {
    public function read(string $path);
}
";

const CLOUD_CONTRACT_PHP: &str = "\
<?php
namespace Illuminate\\Contracts\\Filesystem;
interface Cloud extends Filesystem {
    public function url(string $path);
}
";

const FILESYSTEM_ADAPTER_PHP: &str = "\
<?php
namespace Illuminate\\Filesystem;
use Illuminate\\Contracts\\Filesystem\\Cloud;
class FilesystemAdapter implements Cloud {
    public function read(string $path) { return null; }
    public function url(string $path) { return ''; }
    public function assertExists($path, $content = null) { return $this; }
    public function download($path, $name = null) { return null; }
}
";

const FILESYSTEM_MANAGER_PHP: &str = "\
<?php
namespace Illuminate\\Filesystem;
class FilesystemManager {
    /** @return \\Illuminate\\Contracts\\Filesystem\\Filesystem */
    public function drive($name = null) { return $this->disk($name); }
    /** @return \\Illuminate\\Contracts\\Filesystem\\Filesystem */
    public function disk($name = null) { return null; }
    /** @return \\Illuminate\\Contracts\\Filesystem\\Cloud */
    public function cloud() { return null; }
    /** @return \\Illuminate\\Contracts\\Filesystem\\Filesystem */
    public function build($config) { return null; }
}
";

const STORAGE_FACADE_PHP: &str = "\
<?php
namespace Illuminate\\Support\\Facades;
/**
 * @method static \\Illuminate\\Contracts\\Filesystem\\Filesystem drive(string|null $name = null)
 * @method static \\Illuminate\\Contracts\\Filesystem\\Filesystem disk(string|null $name = null)
 * @method static \\Illuminate\\Contracts\\Filesystem\\Cloud cloud()
 * @method static \\Illuminate\\Contracts\\Filesystem\\Filesystem build(string|array $config)
 */
class Storage {}
";

fn base_files() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "vendor/illuminate/Contracts/Filesystem/Filesystem.php",
            FILESYSTEM_CONTRACT_PHP,
        ),
        (
            "vendor/illuminate/Contracts/Filesystem/Cloud.php",
            CLOUD_CONTRACT_PHP,
        ),
        (
            "vendor/illuminate/Filesystem/FilesystemAdapter.php",
            FILESYSTEM_ADAPTER_PHP,
        ),
        (
            "vendor/illuminate/Filesystem/FilesystemManager.php",
            FILESYSTEM_MANAGER_PHP,
        ),
        (
            "vendor/illuminate/Support/Facades/Storage.php",
            STORAGE_FACADE_PHP,
        ),
    ]
}

async fn complete_labels(
    files: &[(&str, &str)],
    open_path: &str,
    content: &str,
    line: u32,
    character: u32,
) -> Vec<String> {
    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, files);
    let uri = Url::from_file_path(dir.path().join(open_path)).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: content.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position { line, character },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    let items = match result {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        _ => Vec::new(),
    };
    items.into_iter().map(|i| i.label).collect()
}

const CONTROLLER_PHP: &str = "\
<?php
namespace App;
use Illuminate\\Support\\Facades\\Storage;
class C {
    public function show() {
        Storage::disk('s3')->
    }
}
";

/// When every configured disk uses a driver the framework ships,
/// `Storage::disk()` resolves to the concrete `FilesystemAdapter`, so
/// adapter-only members like `assertExists()` complete.
#[tokio::test]
async fn disk_with_only_builtin_drivers_resolves_to_adapter() {
    let mut files = base_files();
    files.push((
        "config/filesystems.php",
        "<?php return [
            'disks' => [
                'local' => ['driver' => 'local'],
                's3' => ['driver' => 's3'],
            ],
        ];",
    ));

    let labels = complete_labels(&files, "src/C.php", CONTROLLER_PHP, 5, 29).await;
    assert!(
        labels.iter().any(|l| l.starts_with("assertExists")),
        "expected FilesystemAdapter::assertExists in completions, got: {labels:?}"
    );
}

/// When a disk is built by a driver not shipped by the framework (registered
/// via `Storage::extend()`, whose return type cannot be read statically),
/// the correction does not fire and `disk()` keeps returning the bare
/// contract.
#[tokio::test]
async fn disk_with_custom_driver_keeps_contract() {
    let mut files = base_files();
    files.push((
        "config/filesystems.php",
        "<?php return [
            'disks' => [
                'local' => ['driver' => 'local'],
                'dropbox' => ['driver' => 'dropbox'],
            ],
        ];",
    ));

    let labels = complete_labels(&files, "src/C.php", CONTROLLER_PHP, 5, 29).await;
    assert!(
        !labels.iter().any(|l| l.starts_with("assertExists")),
        "a custom driver disk must not be widened to FilesystemAdapter, got: {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l.starts_with("read")),
        "the declared Filesystem contract's own members should still complete, got: {labels:?}"
    );
}

/// `cloud()` declares the separate `Cloud` contract, which is also
/// corrected to the concrete adapter (which itself implements `Cloud`).
#[tokio::test]
async fn cloud_resolves_to_adapter() {
    let mut files = base_files();
    files.push((
        "config/filesystems.php",
        "<?php return [
            'disks' => [
                'local' => ['driver' => 'local'],
                's3' => ['driver' => 's3'],
            ],
        ];",
    ));

    let controller = "\
<?php
namespace App;
use Illuminate\\Support\\Facades\\Storage;
class C {
    public function show() {
        Storage::cloud()->
    }
}
";
    let labels = complete_labels(&files, "src/C.php", controller, 5, 26).await;
    assert!(
        labels.iter().any(|l| l.starts_with("assertExists")),
        "expected FilesystemAdapter::assertExists in completions, got: {labels:?}"
    );
}
