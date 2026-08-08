//! Tests that listing members on a Laravel facade offers the concrete
//! class's methods, not just the facade's own declarations.
//!
//! `Facade::__callStatic()` forwards every static call to the container
//! instance named by `getFacadeAccessor()`. Laravel's own facades spell
//! that out in a generated `@method static` docblock, but an app-defined
//! facade that never ran `facade-documenter` has nothing to list, so the
//! members have to come from the class the accessor names.

use crate::common::create_psr4_workspace;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON: &str = r#"{
    "require": { "laravel/framework": "^11.0" },
    "autoload": {
        "psr-4": {
            "App\\": "src/",
            "Illuminate\\Support\\Facades\\": "vendor/illuminate/Support/Facades/"
        }
    }
}"#;

const FACADE_PHP: &str = r#"<?php
namespace Illuminate\Support\Facades;
abstract class Facade
{
    public static function __callStatic($method, $args)
    {
        return static::resolveFacadeInstance()->$method(...$args);
    }
}
"#;

const CONTAINER_PHP: &str = r#"<?php
namespace App\Services;
class Container
{
    public function resolveThing(string $id): object { return new \stdClass(); }
    public function configure(array $options): static { return $this; }
    protected function internals(): void {}
    public static function boot(): void {}
}
"#;

/// An app-defined facade with no generated `@method static` docblock.
const MY_FACADE_PHP: &str = r#"<?php
namespace App\Facades;
use App\Services\Container;
use Illuminate\Support\Facades\Facade;
class MyFacade extends Facade
{
    protected static function getFacadeAccessor(): string
    {
        return Container::class;
    }
}
"#;

/// A facade whose generated docblock already documents the forwarded
/// method, with the flattened return Laravel's generator produces.
const DOCUMENTED_FACADE_PHP: &str = r#"<?php
namespace App\Facades;
use App\Services\Container;
use Illuminate\Support\Facades\Facade;

/**
 * @method static mixed resolveThing(string $id)
 */
class DocumentedFacade extends Facade
{
    protected static function getFacadeAccessor(): string
    {
        return Container::class;
    }
}
"#;

fn base_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vendor/illuminate/Support/Facades/Facade.php", FACADE_PHP),
        ("src/Services/Container.php", CONTAINER_PHP),
        ("src/Facades/MyFacade.php", MY_FACADE_PHP),
        ("src/Facades/DocumentedFacade.php", DOCUMENTED_FACADE_PHP),
    ]
}

async fn complete_labels(consumer: &str, line: u32, character: u32) -> Vec<String> {
    let mut files = base_files();
    files.push(("src/Consumer.php", consumer));
    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, &files);

    let uri = Url::from_file_path(dir.path().join("src/Consumer.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: consumer.to_string(),
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

    match result {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        _ => Vec::new(),
    }
    .into_iter()
    .map(|i| i.label)
    .collect()
}

const UNDOCUMENTED_CONSUMER: &str = "\
<?php
namespace App;
use App\\Facades\\MyFacade;
class Consumer {
    public function go(): void {
        MyFacade::
    }
}
";

#[tokio::test]
async fn undocumented_facade_offers_the_concrete_class_methods() {
    let labels = complete_labels(UNDOCUMENTED_CONSUMER, 5, 18).await;
    assert!(
        labels.iter().any(|l| l.starts_with("resolveThing")),
        "expected Container::resolveThing on the facade, got: {labels:?}"
    );
    assert!(
        labels.iter().any(|l| l.starts_with("configure")),
        "expected Container::configure on the facade, got: {labels:?}"
    );
}

#[tokio::test]
async fn non_public_and_static_concrete_methods_stay_off_the_facade() {
    let labels = complete_labels(UNDOCUMENTED_CONSUMER, 5, 18).await;
    assert!(
        !labels.iter().any(|l| l.starts_with("internals")),
        "a protected method is not reachable through __callStatic, got: {labels:?}"
    );
    // `__callStatic` forwards to an instance, so a static method on the
    // concrete class is never reached through it.
    assert!(
        !labels.iter().any(|l| l.starts_with("boot")),
        "a static concrete method is not forwarded, got: {labels:?}"
    );
}

#[tokio::test]
async fn a_generated_method_tag_keeps_precedence_over_the_forwarded_method() {
    let consumer = "\
<?php
namespace App;
use App\\Facades\\DocumentedFacade;
class Consumer {
    public function go(): void {
        DocumentedFacade::
    }
}
";
    let labels = complete_labels(consumer, 5, 26).await;
    let matches: Vec<&String> = labels
        .iter()
        .filter(|l| l.starts_with("resolveThing"))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "the `@method static` tag should be the only resolveThing, got: {labels:?}"
    );
}

#[tokio::test]
async fn a_forwarded_call_types_to_the_concrete_return() {
    let consumer = "\
<?php
namespace App;
use App\\Facades\\MyFacade;
class Consumer {
    public function go(): void {
        $x = MyFacade::configure([]);
        $x->
    }
}
";
    let labels = complete_labels(consumer, 6, 12).await;
    assert!(
        labels.iter().any(|l| l.starts_with("resolveThing")),
        "a `static` return should chain on the concrete class, got: {labels:?}"
    );
}
