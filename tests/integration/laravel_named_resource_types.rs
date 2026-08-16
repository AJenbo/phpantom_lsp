//! End-to-end coverage for source-defined and type-dependent Laravel names.
//!
//! Rate limiter and queue names are application vocabulary rather than config
//! keys. Connection strings are more subtle: the same `connection()` method
//! can name a database, queue, or broadcast connection, while `$connection`
//! means a database on an Eloquent model and a queue backend on a queued job.
//! These tests keep the receiver types explicit and include ordinary classes
//! with identical method/property names to guard against lexical matching.

use crate::common::create_psr4_workspace;
use phpantom_lsp::Backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON: &str = r#"{
    "require": { "laravel/framework": "^12.0" },
    "autoload": {
        "psr-4": {
            "App\\": "app/",
            "Illuminate\\": "vendor/illuminate/"
        }
    }
}"#;

const DATABASE_CONFIG: &str = r#"<?php
return [
    'default' => 'mysql',
    'connections' => [
        'mysql' => ['driver' => 'mysql'],
        'analytics-db' => ['driver' => 'pgsql'],
    ],
];
"#;

const QUEUE_CONFIG: &str = r#"<?php
return [
    'default' => 'sync',
    'connections' => [
        'sync' => ['driver' => 'sync'],
        'redis-queue' => ['driver' => 'redis'],
    ],
];
"#;

const BROADCASTING_CONFIG: &str = r#"<?php
return [
    'default' => 'reverb',
    'connections' => [
        'reverb' => ['driver' => 'reverb'],
        'pusher-broadcast' => ['driver' => 'pusher'],
    ],
];
"#;

const RATE_LIMITER_FACADE: &str = r#"<?php
namespace Illuminate\Support\Facades;
class RateLimiter
{
    public static function for(string $name, callable $callback): void {}
}
"#;

const ROUTE_FACADE: &str = r#"<?php
namespace Illuminate\Support\Facades;
class Route
{
    public static function middleware(string|array $middleware): void {}
}
"#;

const RATE_LIMITED: &str = r#"<?php
namespace Illuminate\Queue\Middleware;
class RateLimited
{
    public function __construct(string $limiterName) {}
}
"#;

const RATE_LIMITED_WITH_REDIS: &str = r#"<?php
namespace Illuminate\Queue\Middleware;
class RateLimitedWithRedis extends RateLimited {}
"#;

const DATABASE_RESOLVER: &str = r#"<?php
namespace Illuminate\Database;
interface ConnectionResolverInterface
{
    public function connection(?string $name = null): mixed;
}
"#;

const QUEUE_FACTORY: &str = r#"<?php
namespace Illuminate\Contracts\Queue;
interface Factory
{
    public function connection(?string $name = null): mixed;
}
"#;

const BROADCAST_FACTORY: &str = r#"<?php
namespace Illuminate\Contracts\Broadcasting;
interface Factory
{
    public function connection(?string $name = null): mixed;
}
"#;

const SHOULD_QUEUE: &str = r#"<?php
namespace Illuminate\Contracts\Queue;
interface ShouldQueue {}
"#;

const QUEUEABLE: &str = r#"<?php
namespace Illuminate\Bus;
trait Queueable
{
    public function onConnection(?string $connection): static
    {
        return $this;
    }

    public function onQueue(?string $queue): static
    {
        return $this;
    }
}
"#;

const ELOQUENT_MODEL: &str = r#"<?php
namespace Illuminate\Database\Eloquent;
abstract class Model {}
"#;

const RATE_LIMITER_PROVIDER: &str = r#"<?php
namespace App\Providers;

use Illuminate\Support\Facades\RateLimiter;

class RouteServiceProvider
{
    public function boot(): void
    {
        RateLimiter::for('api', static fn () => null);
        RateLimiter::for('uploads', static fn () => null);
        RateLimiter::for('2fa', static fn () => null);
    }
}
"#;

const ORDINARY_CONNECTOR: &str = r#"<?php
namespace App\Support;
class OrdinaryConnector
{
    public string $connection = '';

    public function connection(?string $name = null): mixed
    {
        return null;
    }

    public function onConnection(?string $name = null): static
    {
        $this->connection = $name ?? '';
        return $this;
    }

    public function onQueue(?string $name = null): static
    {
        return $this;
    }
}
"#;

fn base_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("config/database.php", DATABASE_CONFIG),
        ("config/queue.php", QUEUE_CONFIG),
        ("config/broadcasting.php", BROADCASTING_CONFIG),
        (
            "vendor/illuminate/Support/Facades/RateLimiter.php",
            RATE_LIMITER_FACADE,
        ),
        ("vendor/illuminate/Support/Facades/Route.php", ROUTE_FACADE),
        (
            "vendor/illuminate/Queue/Middleware/RateLimited.php",
            RATE_LIMITED,
        ),
        (
            "vendor/illuminate/Queue/Middleware/RateLimitedWithRedis.php",
            RATE_LIMITED_WITH_REDIS,
        ),
        (
            "vendor/illuminate/Database/ConnectionResolverInterface.php",
            DATABASE_RESOLVER,
        ),
        (
            "vendor/illuminate/Contracts/Queue/Factory.php",
            QUEUE_FACTORY,
        ),
        (
            "vendor/illuminate/Contracts/Broadcasting/Factory.php",
            BROADCAST_FACTORY,
        ),
        (
            "vendor/illuminate/Contracts/Queue/ShouldQueue.php",
            SHOULD_QUEUE,
        ),
        ("vendor/illuminate/Bus/Queueable.php", QUEUEABLE),
        (
            "vendor/illuminate/Database/Eloquent/Model.php",
            ELOQUENT_MODEL,
        ),
        (
            "app/Providers/RouteServiceProvider.php",
            RATE_LIMITER_PROVIDER,
        ),
        ("app/Support/OrdinaryConnector.php", ORDINARY_CONNECTOR),
    ]
}

async fn workspace(extra: &[(&str, &str)], focus_path: &str) -> (Backend, tempfile::TempDir, Url) {
    let mut files = extra.to_vec();
    files.extend(base_files());
    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, &files);
    backend.initialized(InitializedParams {}).await;

    // Explicitly opening each fixture makes source-defined name discovery
    // deterministic without relying on a background workspace scan winning a
    // race with the first completion request.
    for (path, content) in &files {
        let uri = Url::from_file_path(dir.path().join(path)).unwrap();
        backend
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "php".to_string(),
                    version: 1,
                    text: (*content).to_string(),
                },
            })
            .await;
    }

    let uri = Url::from_file_path(dir.path().join(focus_path)).unwrap();
    (backend, dir, uri)
}

fn position_after(content: &str, needle: &str) -> Position {
    let offset = content
        .find(needle)
        .unwrap_or_else(|| panic!("missing `{needle}`"))
        + needle.len();
    let before = &content[..offset];
    Position::new(
        before.bytes().filter(|byte| *byte == b'\n').count() as u32,
        before
            .rsplit_once('\n')
            .map_or(before.len(), |(_, tail)| tail.len()) as u32,
    )
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

async fn definition_locations(backend: &Backend, uri: &Url, position: Position) -> Vec<Location> {
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
        .expect("definition request should succeed");

    match response {
        Some(GotoDefinitionResponse::Scalar(location)) => vec![location],
        Some(GotoDefinitionResponse::Array(locations)) => locations,
        Some(GotoDefinitionResponse::Link(links)) => links
            .into_iter()
            .map(|link| Location::new(link.target_uri, link.target_selection_range))
            .collect(),
        None => Vec::new(),
    }
}

fn hover_text(backend: &Backend, uri: &Url, content: &str, position: Position) -> Option<String> {
    let hover = backend.handle_hover(uri.as_str(), content, position)?;
    match hover.contents {
        HoverContents::Markup(markup) => Some(markup.value),
        HoverContents::Scalar(MarkedString::String(value)) => Some(value),
        HoverContents::Scalar(MarkedString::LanguageString(value)) => Some(value.value),
        HoverContents::Array(values) => Some(
            values
                .into_iter()
                .map(|value| match value {
                    MarkedString::String(value) => value,
                    MarkedString::LanguageString(value) => value.value,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    }
}

fn diagnostics_with_code<'a>(diagnostics: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                &diagnostic.code,
                Some(NumberOrString::String(found)) if found == code
            )
        })
        .collect()
}

#[tokio::test]
async fn registered_rate_limiters_complete_in_every_read_and_write_context() {
    let consumer = r#"<?php
namespace App;

use Illuminate\Queue\Middleware\RateLimited;
use Illuminate\Queue\Middleware\RateLimitedWithRedis;
use Illuminate\Support\Facades\RateLimiter;
use Illuminate\Support\Facades\Route;

class Consumer
{
    public function run(): void
    {
        RateLimiter::for('', static fn () => null);
        Route::middleware('throttle:a');
        Route::middleware('throttle:2');
        new RateLimited('u');
        new RateLimitedWithRedis('a');
    }
}
"#;
    let (backend, _dir, uri) =
        workspace(&[("app/Consumer.php", consumer)], "app/Consumer.php").await;

    let registration = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "RateLimiter::for('"),
    )
    .await;
    assert_eq!(registration, ["2fa", "api", "uploads"]);

    let middleware = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "Route::middleware('throttle:a"),
    )
    .await;
    assert_eq!(middleware, ["api"]);

    let digit_prefixed = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "Route::middleware('throttle:2"),
    )
    .await;
    assert_eq!(digit_prefixed, ["2fa"]);

    let object = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "new RateLimited('u"),
    )
    .await;
    assert_eq!(object, ["uploads"]);

    let redis_object = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "new RateLimitedWithRedis('a"),
    )
    .await;
    assert_eq!(redis_object, ["api"]);
}

#[tokio::test]
async fn rate_limiter_navigation_hover_and_references_share_the_registration() {
    let consumer = r#"<?php
namespace App;

use Illuminate\Queue\Middleware\RateLimited;
use Illuminate\Queue\Middleware\RateLimitedWithRedis;
use Illuminate\Support\Facades\Route;

class Consumer
{
    public function run(): void
    {
        Route::middleware('throttle:uploads');
        Route::middleware('throttle:2fa');
        new RateLimited('uploads');
        new RateLimitedWithRedis('uploads');
    }
}
"#;
    let (backend, _dir, uri) =
        workspace(&[("app/Consumer.php", consumer)], "app/Consumer.php").await;

    for needle in [
        "throttle:upl",
        "RateLimited('upl",
        "RateLimitedWithRedis('upl",
    ] {
        let position = position_after(consumer, needle);
        let definitions = definition_locations(&backend, &uri, position).await;
        assert!(
            definitions.iter().any(|location| {
                location
                    .uri
                    .path()
                    .ends_with("/app/Providers/RouteServiceProvider.php")
            }),
            "`{needle}` should resolve to RateLimiter::for(), got {definitions:#?}"
        );

        let hover = hover_text(&backend, &uri, consumer, position)
            .unwrap_or_else(|| panic!("`{needle}` should have Laravel hover"));
        assert!(hover.contains("**Rate limiter** `uploads`"), "got: {hover}");
        assert!(
            hover.contains("app/Providers/RouteServiceProvider.php"),
            "got: {hover}"
        );
    }

    let digit_prefixed = position_after(consumer, "throttle:2f");
    let definitions = definition_locations(&backend, &uri, digit_prefixed).await;
    assert!(definitions.iter().any(|location| {
        location
            .uri
            .path()
            .ends_with("/app/Providers/RouteServiceProvider.php")
    }));
    let hover = hover_text(&backend, &uri, consumer, digit_prefixed)
        .expect("digit-prefixed limiter should have Laravel hover");
    assert!(hover.contains("**Rate limiter** `2fa`"), "got: {hover}");

    let references = backend
        .find_references(
            uri.as_str(),
            consumer,
            position_after(consumer, "RateLimited('upl"),
            true,
        )
        .expect("registered limiter should have references");
    assert_eq!(
        references.len(),
        4,
        "one registration and three matching consumers should be linked: {references:#?}"
    );
    assert_eq!(
        references
            .iter()
            .filter(|location| location.uri == uri)
            .count(),
        3
    );
}

#[tokio::test]
async fn unknown_rate_limiters_are_diagnosed_but_numeric_throttles_are_not_names() {
    let consumer = r#"<?php
namespace App;

use Illuminate\Queue\Middleware\RateLimited;
use Illuminate\Queue\Middleware\RateLimitedWithRedis;
use Illuminate\Support\Facades\Route;

class Consumer
{
    public function run(): void
    {
        Route::middleware('throttle:api');
        Route::middleware('throttle:2fa');
        Route::middleware('throttle:missing-middleware');
        Route::middleware('throttle:60,1');
        new RateLimited('missing-object');
        new RateLimitedWithRedis('missing-redis-object');
    }
}
"#;
    let (backend, _dir, uri) =
        workspace(&[("app/Consumer.php", consumer)], "app/Consumer.php").await;
    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), consumer, &mut diagnostics);

    let invalid = diagnostics_with_code(&diagnostics, "invalid_laravel_rate_limiter");
    assert_eq!(invalid.len(), 3, "got: {invalid:#?}");
    for missing in [
        "missing-middleware",
        "missing-object",
        "missing-redis-object",
    ] {
        assert!(
            invalid
                .iter()
                .any(|diagnostic| diagnostic.message.contains(missing)),
            "missing `{missing}` in {invalid:#?}"
        );
    }
    assert!(
        invalid.iter().all(|diagnostic| {
            !diagnostic.message.contains("60") && !diagnostic.message.contains("2fa")
        }),
        "an inline numeric throttle is not a registered name: {invalid:#?}"
    );

    let numeric_completion = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "Route::middleware('throttle:6"),
    )
    .await;
    assert!(
        !numeric_completion.contains(&"api".to_string())
            && !numeric_completion.contains(&"uploads".to_string()),
        "numeric middleware must not enter limiter completion: {numeric_completion:?}"
    );
    assert!(
        definition_locations(
            &backend,
            &uri,
            position_after(consumer, "Route::middleware('throttle:6"),
        )
        .await
        .is_empty(),
        "numeric middleware must not resolve to RateLimiter::for()"
    );
}

#[tokio::test]
async fn a_dynamic_rate_limiter_registration_keeps_diagnostics_open_world() {
    let dynamic_provider = r#"<?php
namespace App\Providers;

use Illuminate\Support\Facades\RateLimiter;

class DynamicRateLimiterProvider
{
    public function register(string $name): void
    {
        RateLimiter::for($name, static fn () => null);
    }
}
"#;
    let consumer = r#"<?php
namespace App;

use Illuminate\Queue\Middleware\RateLimited;
use Illuminate\Support\Facades\Route;

class Consumer
{
    public function run(): void
    {
        Route::middleware('throttle:package-defined');
        new RateLimited('tenant-defined');
    }
}
"#;
    let (backend, _dir, uri) = workspace(
        &[
            (
                "app/Providers/DynamicRateLimiterProvider.php",
                dynamic_provider,
            ),
            ("app/Consumer.php", consumer),
        ],
        "app/Consumer.php",
    )
    .await;
    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), consumer, &mut diagnostics);

    assert!(
        diagnostics_with_code(&diagnostics, "invalid_laravel_rate_limiter").is_empty(),
        "a non-literal registration means packages may define any limiter: {diagnostics:#?}"
    );
}

const NAMED_JOB: &str = r#"<?php
namespace App\Jobs;

use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;

class NamedJob implements ShouldQueue
{
    use Queueable;

    public function configure(): void
    {
        $this->onQueue('critical');
        $this->onQueue('emails');
    }
}
"#;

const ALTERNATE_JOB: &str = r#"<?php
namespace App\Jobs;

use Illuminate\Bus\Queueable;
use Illuminate\Contracts\Queue\ShouldQueue;

class AlternateJob implements ShouldQueue
{
    use Queueable;

    public function configure(): void
    {
        $this->onQueue('bulk');
    }
}
"#;

#[tokio::test]
async fn queue_names_complete_and_reference_only_on_queueable_receivers() {
    let consumer = r#"<?php
namespace App;

use App\Jobs\NamedJob;
use App\Support\OrdinaryConnector;

class Consumer
{
    public function run(NamedJob $job, OrdinaryConnector $ordinary): void
    {
        $job->onQueue('');
        $job->onQueue('emails');
        $job->onQueue('brand-new');
        $ordinary->onQueue('');
        $ordinary->onQueue('emails');
        $ordinary->onQueue('ordinary-only');
    }
}
"#;
    let (backend, _dir, uri) = workspace(
        &[
            ("app/Jobs/NamedJob.php", NAMED_JOB),
            ("app/Consumer.php", consumer),
        ],
        "app/Consumer.php",
    )
    .await;

    let queueable =
        completion_labels(&backend, &uri, position_after(consumer, "$job->onQueue('")).await;
    assert!(
        queueable.contains(&"critical".to_string()),
        "got: {queueable:?}"
    );
    assert!(
        queueable.contains(&"emails".to_string()),
        "got: {queueable:?}"
    );
    assert!(
        !queueable.contains(&"ordinary-only".to_string()),
        "an ordinary same-named method cannot declare queue vocabulary: {queueable:?}"
    );

    let ordinary = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$ordinary->onQueue('"),
    )
    .await;
    assert!(
        !ordinary.contains(&"critical".to_string()) && !ordinary.contains(&"emails".to_string()),
        "an ordinary receiver must not get Laravel queue completion: {ordinary:?}"
    );

    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), consumer, &mut diagnostics);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("brand-new")),
        "queue names are intentionally open and should never be diagnosed: {diagnostics:#?}"
    );

    let references = backend
        .find_references(
            uri.as_str(),
            consumer,
            position_after(consumer, "$job->onQueue('ema"),
            true,
        )
        .expect("a queue name should have references");
    assert_eq!(
        references.len(),
        2,
        "the job declaration and typed consumer should link, ordinary call should not: {references:#?}"
    );
    assert_eq!(
        references
            .iter()
            .filter(|location| location.uri == uri)
            .count(),
        1
    );

    let hover = hover_text(
        &backend,
        &uri,
        consumer,
        position_after(consumer, "$job->onQueue('ema"),
    )
    .expect("a confirmed queue name should hover");
    assert!(hover.contains("**Queue** `emails`"), "got: {hover}");
}

#[tokio::test]
async fn on_connection_is_a_queue_resource_only_for_queueable_receivers() {
    let consumer = r#"<?php
namespace App;

use App\Jobs\NamedJob;
use App\Support\OrdinaryConnector;

class Consumer
{
    public function run(NamedJob $job, OrdinaryConnector $ordinary): void
    {
        $job->onConnection('');
        $job->onConnection('redis-queue');
        $job->onConnection('missing-job-connection');
        $ordinary->onConnection('');
        $ordinary->onConnection('missing-job-connection');
    }
}
"#;
    let (backend, _dir, uri) = workspace(
        &[
            ("app/Jobs/NamedJob.php", NAMED_JOB),
            ("app/Consumer.php", consumer),
        ],
        "app/Consumer.php",
    )
    .await;

    let queueable = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$job->onConnection('"),
    )
    .await;
    assert_eq!(queueable, ["redis-queue", "sync"]);

    let ordinary = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$ordinary->onConnection('"),
    )
    .await;
    assert!(
        !ordinary.contains(&"redis-queue".to_string()) && !ordinary.contains(&"sync".to_string()),
        "an ordinary same-named method must not get queue connections: {ordinary:?}"
    );

    let definitions = definition_locations(
        &backend,
        &uri,
        position_after(consumer, "$job->onConnection('redis"),
    )
    .await;
    assert!(
        definitions
            .iter()
            .any(|location| location.uri.path().ends_with("/config/queue.php")),
        "typed onConnection() should resolve through queue config: {definitions:#?}"
    );

    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), consumer, &mut diagnostics);
    let invalid = diagnostics_with_code(&diagnostics, "invalid_laravel_queue_connection");
    assert_eq!(
        invalid.len(),
        1,
        "only the queueable receiver should be checked: {invalid:#?}"
    );
    assert!(invalid[0].message.contains("missing-job-connection"));
}

#[tokio::test]
async fn nullable_and_union_queueable_receivers_keep_every_resource_feature_in_sync() {
    let consumer = r#"<?php
namespace App;

use App\Jobs\AlternateJob;
use App\Jobs\NamedJob;
use App\Support\OrdinaryConnector;

class Consumer
{
    public function run(
        ?NamedJob $nullable,
        NamedJob|null $nullableUnion,
        NamedJob|AlternateJob $queueableUnion,
        NamedJob|OrdinaryConnector $mixedUnion,
    ): void {
        $nullable?->onQueue('');
        $nullable?->onQueue('emails');
        $nullable?->onConnection('');
        $nullable?->onConnection('redis-queue');
        $nullable?->onConnection('missing-nullable-connection');
        $nullableUnion?->onConnection('');
        $queueableUnion->onQueue('');
        $mixedUnion->onQueue('');
        $mixedUnion->onConnection('missing-mixed-connection');
    }
}
"#;
    let (backend, _dir, uri) = workspace(
        &[
            ("app/Jobs/NamedJob.php", NAMED_JOB),
            ("app/Jobs/AlternateJob.php", ALTERNATE_JOB),
            ("app/Consumer.php", consumer),
        ],
        "app/Consumer.php",
    )
    .await;

    let nullable_queue = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$nullable?->onQueue('"),
    )
    .await;
    assert!(nullable_queue.contains(&"critical".to_string()));
    assert!(nullable_queue.contains(&"emails".to_string()));

    let nullable_connection = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$nullable?->onConnection('"),
    )
    .await;
    assert_eq!(nullable_connection, ["redis-queue", "sync"]);

    let nullable_union_connection = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$nullableUnion?->onConnection('"),
    )
    .await;
    assert_eq!(nullable_union_connection, ["redis-queue", "sync"]);

    let queueable_union = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$queueableUnion->onQueue('"),
    )
    .await;
    assert!(queueable_union.contains(&"critical".to_string()));
    assert!(queueable_union.contains(&"bulk".to_string()));

    let mixed_union = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$mixedUnion->onQueue('"),
    )
    .await;
    assert!(
        !mixed_union.contains(&"critical".to_string())
            && !mixed_union.contains(&"bulk".to_string()),
        "a partly ordinary union must not be guessed as queueable: {mixed_union:?}"
    );

    let queue_position = position_after(consumer, "$nullable?->onQueue('ema");
    let hover = hover_text(&backend, &uri, consumer, queue_position)
        .expect("a nullable queueable receiver should produce queue-name hover");
    assert!(hover.contains("**Queue** `emails`"), "got: {hover}");
    let references = backend
        .find_references(uri.as_str(), consumer, queue_position, true)
        .expect("a nullable queueable receiver should produce queue-name references");
    assert_eq!(
        references.len(),
        2,
        "the job declaration and nullable use should link: {references:#?}"
    );

    let definitions = definition_locations(
        &backend,
        &uri,
        position_after(consumer, "$nullable?->onConnection('redis"),
    )
    .await;
    assert!(
        definitions
            .iter()
            .any(|location| location.uri.path().ends_with("/config/queue.php")),
        "a nullable queueable receiver should navigate to queue config: {definitions:#?}"
    );

    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), consumer, &mut diagnostics);
    let invalid = diagnostics_with_code(&diagnostics, "invalid_laravel_queue_connection");
    assert_eq!(
        invalid.len(),
        1,
        "the mixed union must stay unclassified: {invalid:#?}"
    );
    assert!(invalid[0].message.contains("missing-nullable-connection"));
}

#[tokio::test]
async fn connection_method_uses_the_receivers_database_queue_or_broadcast_family() {
    let consumer = r#"<?php
namespace App;

use App\Support\OrdinaryConnector;
use Illuminate\Contracts\Broadcasting\Factory as BroadcastFactory;
use Illuminate\Contracts\Queue\Factory as QueueFactory;
use Illuminate\Database\ConnectionResolverInterface;

class Consumer
{
    public function run(
        ConnectionResolverInterface $database,
        ?ConnectionResolverInterface $nullableDatabase,
        QueueFactory $queue,
        BroadcastFactory $broadcast,
        OrdinaryConnector $ordinary,
    ): void {
        $database->connection('');
        $database->connection('analytics-db');
        $database->connection('missing-database');
        $nullableDatabase?->connection('');
        $nullableDatabase?->connection('analytics-db');
        $queue->connection('');
        $queue->connection('redis-queue');
        $queue->connection('missing-queue');
        $broadcast->connection('');
        $broadcast->connection('pusher-broadcast');
        $broadcast->connection('missing-broadcast');
        $ordinary->connection('');
        $ordinary->connection('missing-database');
    }
}
"#;
    let (backend, _dir, uri) =
        workspace(&[("app/Consumer.php", consumer)], "app/Consumer.php").await;

    let database = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$database->connection('"),
    )
    .await;
    assert_eq!(database, ["analytics-db", "mysql"]);

    let nullable_database = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$nullableDatabase?->connection('"),
    )
    .await;
    assert_eq!(nullable_database, ["analytics-db", "mysql"]);

    let queue = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$queue->connection('"),
    )
    .await;
    assert_eq!(queue, ["redis-queue", "sync"]);

    let broadcast = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$broadcast->connection('"),
    )
    .await;
    assert_eq!(broadcast, ["pusher-broadcast", "reverb"]);

    let ordinary = completion_labels(
        &backend,
        &uri,
        position_after(consumer, "$ordinary->connection('"),
    )
    .await;
    assert!(
        !ordinary.contains(&"analytics-db".to_string())
            && !ordinary.contains(&"redis-queue".to_string())
            && !ordinary.contains(&"pusher-broadcast".to_string()),
        "an ordinary connection() method must stay ordinary: {ordinary:?}"
    );

    for (needle, config_file, hover_label) in [
        (
            "$database->connection('analytics",
            "database.php",
            "Database connection",
        ),
        (
            "$nullableDatabase?->connection('analytics",
            "database.php",
            "Database connection",
        ),
        ("$queue->connection('redis", "queue.php", "Queue connection"),
        (
            "$broadcast->connection('pusher",
            "broadcasting.php",
            "Broadcast connection",
        ),
    ] {
        let position = position_after(consumer, needle);
        let definitions = definition_locations(&backend, &uri, position).await;
        assert!(
            definitions.iter().any(|location| {
                location
                    .uri
                    .path()
                    .ends_with(&format!("/config/{config_file}"))
            }),
            "`{needle}` should resolve through {config_file}: {definitions:#?}"
        );
        let hover = hover_text(&backend, &uri, consumer, position)
            .unwrap_or_else(|| panic!("`{needle}` should have resource hover"));
        assert!(hover.contains(hover_label), "got: {hover}");
    }

    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), consumer, &mut diagnostics);
    for (code, missing) in [
        ("invalid_laravel_database_connection", "missing-database"),
        ("invalid_laravel_queue_connection", "missing-queue"),
        ("invalid_laravel_broadcast_connection", "missing-broadcast"),
    ] {
        let invalid = diagnostics_with_code(&diagnostics, code);
        assert_eq!(invalid.len(), 1, "{code}: {invalid:#?}");
        assert!(invalid[0].message.contains(missing), "{code}: {invalid:#?}");
    }
}

const BASE_RECORD: &str = r#"<?php
namespace App\Models;

use Illuminate\Database\Eloquent\Model;

abstract class BaseRecord extends Model {}
"#;

const NESTED_QUEUEABLE_TRAIT: &str = r#"<?php
namespace App\Jobs;

use Illuminate\Bus\Queueable;

trait DispatchesOnQueue
{
    use Queueable;
}
"#;

const BASE_JOB: &str = r#"<?php
namespace App\Jobs;

use Illuminate\Contracts\Queue\ShouldQueue;

abstract class BaseJob implements ShouldQueue
{
    use DispatchesOnQueue;
}
"#;

#[tokio::test]
async fn connection_property_completion_distinguishes_models_jobs_and_plain_classes() {
    let report = r#"<?php
namespace App\Models;
class Report extends BaseRecord
{
    protected $connection = 'a';

    public function run(string $connection = 'a'): void {}
}
"#;
    let export_job = r#"<?php
namespace App\Jobs;
class ExportJob extends BaseJob
{
    public $connection = 'r';
}
"#;
    let direct_job = r#"<?php
namespace App\Jobs;
use Illuminate\Contracts\Queue\ShouldQueue;
class DirectJob implements ShouldQueue
{
    public $connection = 'r';
}
"#;
    let plain = r#"<?php
namespace App;
class Plain
{
    private $connection = 'a';
}
"#;
    let files = [
        ("app/Models/BaseRecord.php", BASE_RECORD),
        ("app/Models/Report.php", report),
        ("app/Jobs/DispatchesOnQueue.php", NESTED_QUEUEABLE_TRAIT),
        ("app/Jobs/BaseJob.php", BASE_JOB),
        ("app/Jobs/ExportJob.php", export_job),
        ("app/Jobs/DirectJob.php", direct_job),
        ("app/Plain.php", plain),
    ];
    let (backend, dir, _) = workspace(&files, "app/Models/Report.php").await;

    let report_uri = Url::from_file_path(dir.path().join("app/Models/Report.php")).unwrap();
    let model_labels = completion_labels(
        &backend,
        &report_uri,
        position_after(report, "$connection = 'a"),
    )
    .await;
    assert_eq!(model_labels, ["analytics-db"]);

    let parameter_labels = completion_labels(
        &backend,
        &report_uri,
        position_after(report, "run(string $connection = 'a"),
    )
    .await;
    assert!(
        parameter_labels.is_empty(),
        "a method's visibility must not promote its ordinary parameter: {parameter_labels:?}"
    );

    let export_uri = Url::from_file_path(dir.path().join("app/Jobs/ExportJob.php")).unwrap();
    let inherited_job_labels = completion_labels(
        &backend,
        &export_uri,
        position_after(export_job, "$connection = 'r"),
    )
    .await;
    assert_eq!(inherited_job_labels, ["redis-queue"]);

    let direct_uri = Url::from_file_path(dir.path().join("app/Jobs/DirectJob.php")).unwrap();
    let direct_job_labels = completion_labels(
        &backend,
        &direct_uri,
        position_after(direct_job, "$connection = 'r"),
    )
    .await;
    assert_eq!(
        direct_job_labels,
        ["redis-queue"],
        "ShouldQueue alone makes the class a queued job"
    );

    let plain_uri = Url::from_file_path(dir.path().join("app/Plain.php")).unwrap();
    let plain_labels = completion_labels(
        &backend,
        &plain_uri,
        position_after(plain, "$connection = 'a"),
    )
    .await;
    assert!(
        !plain_labels.contains(&"analytics-db".to_string())
            && !plain_labels.contains(&"redis-queue".to_string()),
        "an ordinary property must not receive Laravel config values: {plain_labels:?}"
    );
}

#[tokio::test]
async fn connection_property_diagnostics_follow_indirect_model_and_job_types() {
    let report = r#"<?php
namespace App\Models;
class BrokenReport extends BaseRecord
{
    protected $connection = 'missing-model-connection';
}
"#;
    let export_job = r#"<?php
namespace App\Jobs;
class BrokenExportJob extends BaseJob
{
    public $connection = 'missing-job-connection';
}
"#;
    let direct_job = r#"<?php
namespace App\Jobs;
use Illuminate\Contracts\Queue\ShouldQueue;
class BrokenDirectJob implements ShouldQueue
{
    public $connection = 'missing-direct-job-connection';
}
"#;
    let plain = r#"<?php
namespace App;
class PlainBroken
{
    private $connection = 'missing-model-connection';
}
"#;
    let files = [
        ("app/Models/BaseRecord.php", BASE_RECORD),
        ("app/Models/BrokenReport.php", report),
        ("app/Jobs/DispatchesOnQueue.php", NESTED_QUEUEABLE_TRAIT),
        ("app/Jobs/BaseJob.php", BASE_JOB),
        ("app/Jobs/BrokenExportJob.php", export_job),
        ("app/Jobs/BrokenDirectJob.php", direct_job),
        ("app/PlainBroken.php", plain),
    ];
    let (backend, dir, _) = workspace(&files, "app/Models/BrokenReport.php").await;

    let cases = [
        (
            "app/Models/BrokenReport.php",
            report,
            "invalid_laravel_database_connection",
            "missing-model-connection",
        ),
        (
            "app/Jobs/BrokenExportJob.php",
            export_job,
            "invalid_laravel_queue_connection",
            "missing-job-connection",
        ),
        (
            "app/Jobs/BrokenDirectJob.php",
            direct_job,
            "invalid_laravel_queue_connection",
            "missing-direct-job-connection",
        ),
    ];
    for (path, content, code, missing) in cases {
        let uri = Url::from_file_path(dir.path().join(path)).unwrap();
        let mut diagnostics = Vec::new();
        backend.collect_slow_diagnostics(uri.as_str(), content, &mut diagnostics);
        let invalid = diagnostics_with_code(&diagnostics, code);
        assert_eq!(invalid.len(), 1, "{path}: {invalid:#?}");
        assert!(invalid[0].message.contains(missing), "{path}: {invalid:#?}");
    }

    let plain_uri = Url::from_file_path(dir.path().join("app/PlainBroken.php")).unwrap();
    let mut plain_diagnostics = Vec::new();
    backend.collect_slow_diagnostics(plain_uri.as_str(), plain, &mut plain_diagnostics);
    assert!(
        diagnostics_with_code(&plain_diagnostics, "invalid_laravel_database_connection").is_empty()
            && diagnostics_with_code(&plain_diagnostics, "invalid_laravel_queue_connection")
                .is_empty(),
        "an ordinary `$connection` property is not Laravel configuration: {plain_diagnostics:#?}"
    );
}

#[tokio::test]
async fn connection_properties_navigate_hover_and_reference_their_config_entries() {
    let report = r#"<?php
namespace App\Models;
class LinkedReport extends BaseRecord
{
    protected $connection = 'analytics-db';

    public function configValue(): mixed
    {
        return config('database.connections.analytics-db');
    }
}
"#;
    let job = r#"<?php
namespace App\Jobs;
class LinkedJob extends BaseJob
{
    public $connection = 'redis-queue';

    public function configValue(): mixed
    {
        return config('queue.connections.redis-queue');
    }
}
"#;
    let files = [
        ("app/Models/BaseRecord.php", BASE_RECORD),
        ("app/Models/LinkedReport.php", report),
        ("app/Jobs/DispatchesOnQueue.php", NESTED_QUEUEABLE_TRAIT),
        ("app/Jobs/BaseJob.php", BASE_JOB),
        ("app/Jobs/LinkedJob.php", job),
    ];
    let (backend, dir, _) = workspace(&files, "app/Models/LinkedReport.php").await;

    for (path, content, literal, config_file, hover_label) in [
        (
            "app/Models/LinkedReport.php",
            report,
            "$connection = 'analytics",
            "database.php",
            "Database connection",
        ),
        (
            "app/Jobs/LinkedJob.php",
            job,
            "$connection = 'redis",
            "queue.php",
            "Queue connection",
        ),
    ] {
        let uri = Url::from_file_path(dir.path().join(path)).unwrap();
        let position = position_after(content, literal);
        let definitions = definition_locations(&backend, &uri, position).await;
        assert!(
            definitions.iter().any(|location| location
                .uri
                .path()
                .ends_with(&format!("/config/{config_file}"))),
            "{path}: {definitions:#?}"
        );

        let hover = hover_text(&backend, &uri, content, position)
            .unwrap_or_else(|| panic!("{path} should have resource hover"));
        assert!(hover.contains(hover_label), "{path}: {hover}");

        let references = backend
            .find_references(uri.as_str(), content, position, true)
            .unwrap_or_else(|| panic!("{path} should have config references"));
        assert_eq!(
            references.len(),
            3,
            "property, generic config() use, and declaration for {path}: {references:#?}"
        );
    }
}

#[tokio::test]
async fn promoted_connection_defaults_have_completion_diagnostics_and_symbol_navigation() {
    let report = r#"<?php
namespace App\Models;
class PromotedReport extends BaseRecord
{
    public function __construct(protected string $connection = 'analytics-db') {}

    public function configValue(): mixed
    {
        return config('database.connections.analytics-db');
    }
}
"#;
    let job = r#"<?php
namespace App\Jobs;
class PromotedJob extends BaseJob
{
    public function __construct(public string $connection = 'redis-queue') {}

    public function configValue(): mixed
    {
        return config('queue.connections.redis-queue');
    }
}
"#;
    let broken_report = r#"<?php
namespace App\Models;
class BrokenPromotedReport extends BaseRecord
{
    public function __construct(protected string $connection = 'missing-model-connection') {}
}
"#;
    let broken_job = r#"<?php
namespace App\Jobs;
class BrokenPromotedJob extends BaseJob
{
    public function __construct(public string $connection = 'missing-job-connection') {}
}
"#;
    let plain = r#"<?php
namespace App;
class PlainPromotedConnection
{
    public function __construct(private string $connection = 'missing-model-connection') {}
}
"#;
    let files = [
        ("app/Models/BaseRecord.php", BASE_RECORD),
        ("app/Models/PromotedReport.php", report),
        ("app/Models/BrokenPromotedReport.php", broken_report),
        ("app/Jobs/DispatchesOnQueue.php", NESTED_QUEUEABLE_TRAIT),
        ("app/Jobs/BaseJob.php", BASE_JOB),
        ("app/Jobs/PromotedJob.php", job),
        ("app/Jobs/BrokenPromotedJob.php", broken_job),
        ("app/PlainPromotedConnection.php", plain),
    ];
    let (backend, dir, _) = workspace(&files, "app/Models/PromotedReport.php").await;

    for (path, content, literal, expected, config_file, hover_label) in [
        (
            "app/Models/PromotedReport.php",
            report,
            "$connection = 'a",
            "analytics-db",
            "database.php",
            "Database connection",
        ),
        (
            "app/Jobs/PromotedJob.php",
            job,
            "$connection = 'r",
            "redis-queue",
            "queue.php",
            "Queue connection",
        ),
    ] {
        let uri = Url::from_file_path(dir.path().join(path)).unwrap();
        let position = position_after(content, literal);
        let labels = completion_labels(&backend, &uri, position).await;
        assert_eq!(labels, [expected], "{path}: {labels:?}");

        let definitions = definition_locations(&backend, &uri, position).await;
        assert!(
            definitions.iter().any(|location| location
                .uri
                .path()
                .ends_with(&format!("/config/{config_file}"))),
            "{path}: {definitions:#?}"
        );

        let hover = hover_text(&backend, &uri, content, position)
            .unwrap_or_else(|| panic!("{path} should have resource hover"));
        assert!(hover.contains(hover_label), "{path}: {hover}");

        let references = backend
            .find_references(uri.as_str(), content, position, true)
            .unwrap_or_else(|| panic!("{path} should have config references"));
        assert_eq!(
            references.len(),
            3,
            "promoted default, generic config() use, and declaration for {path}: {references:#?}"
        );
    }

    for (path, content, code, missing) in [
        (
            "app/Models/BrokenPromotedReport.php",
            broken_report,
            "invalid_laravel_database_connection",
            "missing-model-connection",
        ),
        (
            "app/Jobs/BrokenPromotedJob.php",
            broken_job,
            "invalid_laravel_queue_connection",
            "missing-job-connection",
        ),
    ] {
        let uri = Url::from_file_path(dir.path().join(path)).unwrap();
        let mut diagnostics = Vec::new();
        backend.collect_slow_diagnostics(uri.as_str(), content, &mut diagnostics);
        let invalid = diagnostics_with_code(&diagnostics, code);
        assert_eq!(invalid.len(), 1, "{path}: {invalid:#?}");
        assert!(invalid[0].message.contains(missing), "{path}: {invalid:#?}");
    }

    let plain_uri =
        Url::from_file_path(dir.path().join("app/PlainPromotedConnection.php")).unwrap();
    let plain_position = position_after(plain, "$connection = 'missing");
    let plain_labels = completion_labels(&backend, &plain_uri, plain_position).await;
    assert!(
        !plain_labels.contains(&"analytics-db".to_string())
            && !plain_labels.contains(&"redis-queue".to_string()),
        "an ordinary promoted property must stay ordinary: {plain_labels:?}"
    );
    let mut plain_diagnostics = Vec::new();
    backend.collect_slow_diagnostics(plain_uri.as_str(), plain, &mut plain_diagnostics);
    assert!(
        diagnostics_with_code(&plain_diagnostics, "invalid_laravel_database_connection").is_empty()
            && diagnostics_with_code(&plain_diagnostics, "invalid_laravel_queue_connection")
                .is_empty(),
        "an ordinary promoted property must stay unclassified: {plain_diagnostics:#?}"
    );
}

#[tokio::test]
async fn closing_queue_sources_restores_saved_typed_names() {
    let consumer = r#"<?php
namespace App;

use App\Jobs\NamedJob;

class QueueConsumer
{
    public function run(NamedJob $job): void
    {
        $job->onQueue('');
    }
}
"#;
    let (backend, dir, consumer_uri) = workspace(
        &[
            ("app/Jobs/NamedJob.php", NAMED_JOB),
            ("app/Jobs/AlternateJob.php", ALTERNATE_JOB),
            ("app/QueueConsumer.php", consumer),
        ],
        "app/QueueConsumer.php",
    )
    .await;
    let completion_position = position_after(consumer, "$job->onQueue('");

    let initial = completion_labels(&backend, &consumer_uri, completion_position).await;
    assert!(initial.contains(&"emails".to_string()), "got: {initial:?}");
    assert!(initial.contains(&"bulk".to_string()), "got: {initial:?}");

    let alternate_uri = Url::from_file_path(dir.path().join("app/Jobs/AlternateJob.php")).unwrap();
    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: alternate_uri },
        })
        .await;
    let after_clean_close = completion_labels(&backend, &consumer_uri, completion_position).await;
    assert!(
        after_clean_close.contains(&"bulk".to_string()),
        "an unchanged saved queue name must survive didClose: {after_clean_close:?}"
    );

    let named_uri = Url::from_file_path(dir.path().join("app/Jobs/NamedJob.php")).unwrap();
    let unsaved = NAMED_JOB.replace("'emails'", "'unsaved-queue-with-a-longer-name'");
    backend
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: named_uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: unsaved,
            }],
        })
        .await;
    let dirty = completion_labels(&backend, &consumer_uri, completion_position).await;
    assert!(
        dirty.contains(&"unsaved-queue-with-a-longer-name".to_string()),
        "got: {dirty:?}"
    );
    assert!(!dirty.contains(&"emails".to_string()), "got: {dirty:?}");

    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: named_uri },
        })
        .await;
    let restored = completion_labels(&backend, &consumer_uri, completion_position).await;
    assert!(
        restored.contains(&"emails".to_string()),
        "got: {restored:?}"
    );
    assert!(
        !restored.contains(&"unsaved-queue-with-a-longer-name".to_string()),
        "an unsaved queue name must not survive didClose: {restored:?}"
    );
}
