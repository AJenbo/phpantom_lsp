//! End-to-end coverage for Laravel's config-backed named resources.

use std::fs;

use crate::common::create_psr4_workspace;
use phpantom_lsp::Backend;
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

const COMPOSER_JSON: &str = r#"{
    "require": { "laravel/framework": "^12.0" },
    "autoload": { "psr-4": { "App\\": "app/" } }
}"#;

const AUTH_CONFIG: &str = r#"<?php
return [
    'guards' => [
        'web' => ['driver' => 'session'],
        'admin' => ['driver' => 'session'],
    ],
];
"#;

const CACHE_CONFIG: &str = r#"<?php
return [
    'stores' => [
        'array' => ['driver' => 'array'],
        'redis' => ['driver' => 'redis'],
    ],
];
"#;

const LOGGING_CONFIG: &str = r#"<?php
return [
    'channels' => [
        'daily' => ['driver' => 'daily'],
        'slack' => ['driver' => 'slack'],
    ],
];
"#;

const FILESYSTEMS_CONFIG: &str = r#"<?php
return [
    'disks' => [
        'local' => ['driver' => 'local'],
        'archive' => ['driver' => 'local'],
    ],
];
"#;

const DATABASE_CONFIG: &str = r#"<?php
return [
    'connections' => [
        'mysql' => ['driver' => 'mysql'],
        'sqlite' => ['driver' => 'sqlite'],
    ],
];
"#;

const QUEUE_CONFIG: &str = r#"<?php
return [
    'connections' => [
        'sync' => ['driver' => 'sync'],
        'redis' => ['driver' => 'redis'],
    ],
];
"#;

const MAIL_CONFIG: &str = r#"<?php
return [
    'mailers' => [
        'smtp' => ['transport' => 'smtp'],
        'log' => ['transport' => 'log'],
    ],
];
"#;

const BROADCASTING_CONFIG: &str = r#"<?php
return [
    'connections' => [
        'reverb' => ['driver' => 'reverb'],
        'log' => ['driver' => 'log'],
    ],
];
"#;

fn workspace_files(source: &str) -> Vec<(&str, &str)> {
    vec![
        ("config/auth.php", AUTH_CONFIG),
        ("config/cache.php", CACHE_CONFIG),
        ("config/logging.php", LOGGING_CONFIG),
        ("config/filesystems.php", FILESYSTEMS_CONFIG),
        ("config/database.php", DATABASE_CONFIG),
        ("config/queue.php", QUEUE_CONFIG),
        ("config/mail.php", MAIL_CONFIG),
        ("config/broadcasting.php", BROADCASTING_CONFIG),
        ("app/NamedResourceConsumer.php", source),
    ]
}

fn position_at_offset(content: &str, offset: usize) -> Position {
    let before = &content[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = before
        .rsplit_once('\n')
        .map_or(before.len(), |(_, tail)| tail.len()) as u32;
    Position::new(line, character)
}

fn position_after(content: &str, unique_prefix: &str) -> Position {
    let offset = content
        .find(unique_prefix)
        .unwrap_or_else(|| panic!("missing `{unique_prefix}`"))
        + unique_prefix.len();
    position_at_offset(content, offset)
}

fn position_of_config_key(content: &str, key: &str) -> Position {
    let marker = format!("'{key}' =>");
    let offset = content
        .find(&marker)
        .unwrap_or_else(|| panic!("missing config declaration `{marker}`"))
        + 1;
    position_at_offset(content, offset)
}

fn text_in_range(content: &str, range: Range) -> &str {
    let start_line = content
        .split_inclusive('\n')
        .take(range.start.line as usize)
        .map(str::len)
        .sum::<usize>();
    let end_line = content
        .split_inclusive('\n')
        .take(range.end.line as usize)
        .map(str::len)
        .sum::<usize>();
    &content[start_line + range.start.character as usize..end_line + range.end.character as usize]
}

async fn open_workspace(source: &str) -> (Backend, tempfile::TempDir, Url) {
    let files = workspace_files(source);
    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, &files);
    backend.initialized(InitializedParams {}).await;

    let uri = Url::from_file_path(dir.path().join("app/NamedResourceConsumer.php")).unwrap();
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

async fn completion_items(backend: &Backend, uri: &Url, position: Position) -> Vec<CompletionItem> {
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
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
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

fn hover_text(hover: Hover) -> String {
    match hover.contents {
        HoverContents::Scalar(MarkedString::String(text)) => text,
        HoverContents::Scalar(MarkedString::LanguageString(text)) => text.value,
        HoverContents::Array(parts) => parts
            .into_iter()
            .map(|part| match part {
                MarkedString::String(text) => text,
                MarkedString::LanguageString(text) => text.value,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        HoverContents::Markup(markup) => markup.value,
    }
}

#[tokio::test]
async fn every_named_resource_context_completes_direct_config_children() {
    let source = r#"<?php
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Broadcast;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Mail;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Facades\Route;
use Illuminate\Support\Facades\Storage;

auth('');
Auth::guard('w');
Route::middleware('auth:a');
Route::middleware('auth:web, a');
Cache::store('');
Log::channel('');
Log::stack(['daily', 's']);
Log::stack(array('daily', 's'));
Storage::disk('');
DB::connection('');
Queue::connection('');
Mail::mailer('');
Broadcast::connection('');

#[\Illuminate\Container\Attributes\Auth('a')]
class AuthTarget {}
#[\Illuminate\Container\Attributes\Authenticated('w')]
class AuthenticatedTarget {}
#[\Illuminate\Container\Attributes\Cache('a')]
class CacheTarget {}
#[\Illuminate\Container\Attributes\Log('s')]
class LogTarget {}
#[\Illuminate\Container\Attributes\Storage('l')]
class StorageTarget {}
#[\Illuminate\Container\Attributes\Database('s')]
class DatabaseTarget {}
#[\Illuminate\Container\Attributes\DB('m')]
class DatabaseAliasTarget {}

Log::stack('');
Log::stack(['s' => 'daily']);
\Vendor\Cache::store('');
Cache::store('array', '');
"#;
    let (backend, _dir, uri) = open_workspace(source).await;

    let cases: &[(&str, &[&str])] = &[
        ("auth('", &["admin", "web"]),
        ("Auth::guard('w", &["web"]),
        ("Route::middleware('auth:a", &["admin"]),
        ("Route::middleware('auth:web, a", &["admin"]),
        ("Cache::store('", &["array", "redis"]),
        ("Log::channel('", &["daily", "slack"]),
        ("Log::stack(['daily', 's", &["slack"]),
        ("Log::stack(array('daily', 's", &["slack"]),
        ("Storage::disk('", &["archive", "local"]),
        ("DB::connection('", &["mysql", "sqlite"]),
        ("Queue::connection('", &["redis", "sync"]),
        ("Mail::mailer('", &["log", "smtp"]),
        ("Broadcast::connection('", &["log", "reverb"]),
        ("Attributes\\Auth('a", &["admin"]),
        ("Attributes\\Authenticated('w", &["web"]),
        ("Attributes\\Cache('a", &["array"]),
        ("Attributes\\Log('s", &["slack"]),
        ("Attributes\\Storage('l", &["local"]),
        ("Attributes\\Database('s", &["sqlite"]),
        ("Attributes\\DB('m", &["mysql"]),
    ];

    for (prefix, expected) in cases {
        let items = completion_items(&backend, &uri, position_after(source, prefix)).await;
        let labels = items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>();
        assert_eq!(labels, *expected, "completion at `{prefix}`");
    }

    let middleware_position = position_after(source, "Route::middleware('auth:web, a");
    let middleware_items = completion_items(&backend, &uri, middleware_position).await;
    let Some(CompletionTextEdit::Edit(edit)) = &middleware_items[0].text_edit else {
        panic!("middleware completion should replace only its guard payload");
    };
    assert_eq!(text_in_range(source, edit.range), " a");
    assert_eq!(edit.new_text, "admin");

    for prefix in [
        "Log::stack('",
        "Log::stack(['s",
        "\\Vendor\\Cache::store('",
        "Cache::store('array', '",
    ] {
        assert!(
            completion_items(&backend, &uri, position_after(source, prefix))
                .await
                .is_empty(),
            "`{prefix}` is not a named-resource argument"
        );
    }
}

#[tokio::test]
async fn completion_resolves_aliases_named_arguments_and_local_homonyms() {
    let source = r#"<?php
namespace App;

use Illuminate\Container\Attributes\Cache as CacheAttribute;
use Illuminate\Support\Facades\Cache as LaravelCache;
use Illuminate\Support\Facades\Route as LaravelRoute;
use Illuminate\Support\Facades\Storage as LaravelStorage;

class Cache { public static function store(string $name): void {} }
class Route { public static function middleware(string $name): void {} }

LaravelCache::store(name: '');
LaravelStorage::fake(config: [], disk: 'a');
LaravelRoute::middleware(middleware: 'auth:a');
\Illuminate\Support\Facades\Route::middleware('auth:w');
LaravelRoute::get('/aliased', fn () => null)->middleware('auth:a');
\Illuminate\Support\Facades\Route::get('/fqn', fn () => null)->middleware('auth:w');

#[CacheAttribute(memo: true, store: 'r')]
class Target {}

Cache::store('');
Route::middleware('auth:a');
Route::get('/local', fn () => null)->middleware('auth:a');
#[Cache('')]
class LocalAttributeTarget {}
LaravelCache::store(store: '');

class Controller
{
    public function boot(): void
    {
        $this->middleware(options: [], middleware: 'auth:a');
    }
}
"#;
    let (backend, _dir, uri) = open_workspace(source).await;

    for (prefix, expected) in [
        ("LaravelCache::store(name: '", vec!["array", "redis"]),
        ("disk: 'a", vec!["archive"]),
        ("middleware: 'auth:a", vec!["admin"]),
        ("Facades\\Route::middleware('auth:w", vec!["web"]),
        (
            "'/aliased', fn () => null)->middleware('auth:a",
            vec!["admin"],
        ),
        ("'/fqn', fn () => null)->middleware('auth:w", vec!["web"]),
        ("options: [], middleware: 'auth:a", vec!["admin"]),
        ("store: 'r", vec!["redis"]),
    ] {
        let labels = completion_items(&backend, &uri, position_after(source, prefix))
            .await
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();
        assert_eq!(labels, expected, "completion at `{prefix}`");
    }

    for prefix in [
        "\nCache::store('",
        "\nRoute::middleware('auth:a",
        "'/local', fn () => null)->middleware('auth:a",
        "#[Cache('",
        "LaravelCache::store(store: '",
    ] {
        assert!(
            completion_items(&backend, &uri, position_after(source, prefix))
                .await
                .is_empty(),
            "local homonym or wrong named parameter at `{prefix}` must stay ordinary"
        );
    }
}

#[tokio::test]
async fn every_resource_spelling_navigates_and_hovers_as_its_family() {
    let source = r#"<?php
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Broadcast;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Mail;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Facades\Route;
use Illuminate\Support\Facades\Storage;

auth('web');
Auth::guard('admin');
Route::middleware('auth:web,admin');
Cache::store('redis');
Log::channel('daily');
Log::stack(['daily', 'slack']);
Storage::disk('archive');
DB::connection('mysql');
Queue::connection('sync');
Mail::mailer('smtp');
Broadcast::connection('reverb');

#[\Illuminate\Container\Attributes\Auth('web')]
class AuthTarget {}
#[\Illuminate\Container\Attributes\Authenticated('admin')]
class AuthenticatedTarget {}
#[\Illuminate\Container\Attributes\Cache('array')]
class CacheTarget {}
#[\Illuminate\Container\Attributes\Log('slack')]
class LogTarget {}
#[\Illuminate\Container\Attributes\Storage('local')]
class StorageTarget {}
#[\Illuminate\Container\Attributes\Database('sqlite')]
class DatabaseTarget {}
#[\Illuminate\Container\Attributes\DB('mysql')]
class DatabaseAliasTarget {}
"#;
    let (backend, _dir, uri) = open_workspace(source).await;

    let cases = [
        ("auth('we", "auth.php", AUTH_CONFIG, "web", "Auth guard"),
        (
            "Auth::guard('adm",
            "auth.php",
            AUTH_CONFIG,
            "admin",
            "Auth guard",
        ),
        (
            "middleware('auth:web,adm",
            "auth.php",
            AUTH_CONFIG,
            "admin",
            "Auth guard",
        ),
        (
            "Cache::store('red",
            "cache.php",
            CACHE_CONFIG,
            "redis",
            "Cache store",
        ),
        (
            "Log::channel('dai",
            "logging.php",
            LOGGING_CONFIG,
            "daily",
            "Log channel",
        ),
        (
            "Log::stack(['daily', 'sla",
            "logging.php",
            LOGGING_CONFIG,
            "slack",
            "Log channel",
        ),
        (
            "Storage::disk('arch",
            "filesystems.php",
            FILESYSTEMS_CONFIG,
            "archive",
            "Storage disk",
        ),
        (
            "DB::connection('mys",
            "database.php",
            DATABASE_CONFIG,
            "mysql",
            "Database connection",
        ),
        (
            "Queue::connection('sy",
            "queue.php",
            QUEUE_CONFIG,
            "sync",
            "Queue connection",
        ),
        (
            "Mail::mailer('sm",
            "mail.php",
            MAIL_CONFIG,
            "smtp",
            "Mailer",
        ),
        (
            "Broadcast::connection('rev",
            "broadcasting.php",
            BROADCASTING_CONFIG,
            "reverb",
            "Broadcast connection",
        ),
        (
            "Attributes\\Auth('we",
            "auth.php",
            AUTH_CONFIG,
            "web",
            "Auth guard",
        ),
        (
            "Attributes\\Authenticated('adm",
            "auth.php",
            AUTH_CONFIG,
            "admin",
            "Auth guard",
        ),
        (
            "Attributes\\Cache('arr",
            "cache.php",
            CACHE_CONFIG,
            "array",
            "Cache store",
        ),
        (
            "Attributes\\Log('sla",
            "logging.php",
            LOGGING_CONFIG,
            "slack",
            "Log channel",
        ),
        (
            "Attributes\\Storage('loc",
            "filesystems.php",
            FILESYSTEMS_CONFIG,
            "local",
            "Storage disk",
        ),
        (
            "Attributes\\Database('sql",
            "database.php",
            DATABASE_CONFIG,
            "sqlite",
            "Database connection",
        ),
        (
            "Attributes\\DB('mys",
            "database.php",
            DATABASE_CONFIG,
            "mysql",
            "Database connection",
        ),
    ];

    for (prefix, config_file, config_content, key, label) in cases {
        let position = position_after(source, prefix);
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
            .unwrap_or_else(|| panic!("`{prefix}` should have a definition"));
        let location = definition_location(response);
        assert!(
            location
                .uri
                .path()
                .ends_with(&format!("/config/{config_file}")),
            "definition for `{prefix}`: {location:?}"
        );
        assert_eq!(
            location.range.start,
            position_of_config_key(config_content, key),
            "definition for `{prefix}` should select `{key}`"
        );

        let hover = backend
            .handle_hover(uri.as_str(), source, position)
            .unwrap_or_else(|| panic!("`{prefix}` should have hover"));
        let text = hover_text(hover);
        assert!(
            text.contains(&format!("**{label}** `{key}`")),
            "family hover for `{prefix}`: {text}"
        );
        assert!(
            text.contains(&format!("config/{config_file}")),
            "config source hover for `{prefix}`: {text}"
        );
    }
}

#[tokio::test]
async fn unknown_rate_limiter_hover_describes_the_application_vocabulary() {
    let source = r#"<?php
use Illuminate\Support\Facades\Route;

Route::middleware('throttle:not-registered');
"#;
    let (backend, _dir, uri) = open_workspace(source).await;
    let hover = backend
        .handle_hover(
            uri.as_str(),
            source,
            position_after(source, "throttle:not-reg"),
        )
        .expect("an unresolved limiter should retain its family hover");
    let text = hover_text(hover);
    assert!(text.contains("**Rate limiter** `not-registered`"), "{text}");
    assert!(text.contains("Application rate limiter"), "{text}");
}

#[tokio::test]
async fn diagnostics_validate_each_family_and_cover_only_the_resource_payload() {
    let source = r#"<?php
use Illuminate\Support\Facades\Auth;
use Illuminate\Support\Facades\Broadcast;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Mail;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Facades\Route;
use Illuminate\Support\Facades\Storage;

Auth::guard('web');
Route::middleware('auth:web,missing-guard');
Cache::store('array');
Cache::store('missing-cache');
Log::channel('daily');
Log::stack(['not-a-channel-key' => 'missing-log']);
Storage::disk('local');
Storage::disk('missing-disk');
Storage::fake('testing-only');
Storage::persistentFake('persistent-testing-only');
Storage::forgetDisk(['already-forgotten']);
DB::connection('mysql');
DB::connection('missing-database');
Queue::connection('sync');
Queue::connection('missing-queue');
Mail::mailer('smtp');
Mail::mailer('missing-mailer');
Broadcast::connection('reverb');
Broadcast::connection('missing-broadcast');

Log::stack('missing-scalar-shape');
\Vendor\Cache::store('missing-vendor-cache');
Cache::store('array', 'missing-second-argument');
"#;
    let (backend, _dir, uri) = open_workspace(source).await;
    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), source, &mut diagnostics);

    let mut actual = diagnostics
        .iter()
        .filter_map(|diagnostic| {
            let Some(NumberOrString::String(code)) = &diagnostic.code else {
                return None;
            };
            if code.starts_with("invalid_laravel_") {
                Some((
                    code.as_str(),
                    text_in_range(source, diagnostic.range),
                    diagnostic.message.as_str(),
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    actual.sort_unstable_by(|left, right| left.0.cmp(right.0));

    let mut expected = vec![
        ("invalid_laravel_auth_guard", "missing-guard"),
        ("invalid_laravel_broadcast_connection", "missing-broadcast"),
        ("invalid_laravel_cache_store", "missing-cache"),
        ("invalid_laravel_database_connection", "missing-database"),
        ("invalid_laravel_log_channel", "missing-log"),
        ("invalid_laravel_mailer", "missing-mailer"),
        ("invalid_laravel_queue_connection", "missing-queue"),
        ("invalid_laravel_storage_disk", "missing-disk"),
    ];
    expected.sort_unstable_by(|left, right| left.0.cmp(right.0));

    assert_eq!(actual.len(), expected.len(), "diagnostics: {actual:#?}");
    for ((code, range_text, message), (expected_code, expected_key)) in
        actual.into_iter().zip(expected)
    {
        assert_eq!(code, expected_code);
        assert_eq!(range_text, expected_key, "range for `{code}`");
        assert!(
            message.contains(expected_key),
            "message for `{code}` should name `{expected_key}`: {message}"
        );
    }
}

#[tokio::test]
async fn generic_config_and_resource_spellings_share_symmetric_references() {
    let source = r#"<?php
use Illuminate\Support\Facades\Broadcast;
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Log;
use Illuminate\Support\Facades\Mail;
use Illuminate\Support\Facades\Queue;
use Illuminate\Support\Facades\Route;
use Illuminate\Support\Facades\Storage;

Route::middleware('auth:web,admin');
Cache::store('redis');
Log::stack(['daily', 'slack']);
Storage::disk('archive');
DB::connection('mysql');
Queue::connection('sync');
Mail::mailer('smtp');
Broadcast::connection('reverb');

config('auth.guards.admin');
config('cache.stores.redis');
config('logging.channels.slack');
config('filesystems.disks.archive');
config('database.connections.mysql');
config('queue.connections.sync');
config('mail.mailers.smtp');
config('broadcasting.connections.reverb');
"#;
    let (backend, _dir, uri) = open_workspace(source).await;

    let cases = [
        (
            "middleware('auth:web,adm",
            "config('auth.guards.adm",
            "admin",
            "auth.guards.admin",
        ),
        (
            "Cache::store('red",
            "config('cache.stores.red",
            "redis",
            "cache.stores.redis",
        ),
        (
            "Log::stack(['daily', 'sla",
            "config('logging.channels.sla",
            "slack",
            "logging.channels.slack",
        ),
        (
            "Storage::disk('arch",
            "config('filesystems.disks.arch",
            "archive",
            "filesystems.disks.archive",
        ),
        (
            "DB::connection('mys",
            "config('database.connections.mys",
            "mysql",
            "database.connections.mysql",
        ),
        (
            "Queue::connection('sy",
            "config('queue.connections.sy",
            "sync",
            "queue.connections.sync",
        ),
        (
            "Mail::mailer('sm",
            "config('mail.mailers.sm",
            "smtp",
            "mail.mailers.smtp",
        ),
        (
            "Broadcast::connection('rev",
            "config('broadcasting.connections.rev",
            "reverb",
            "broadcasting.connections.reverb",
        ),
    ];

    for (resource_prefix, config_prefix, short, full) in cases {
        let from_resource = backend
            .find_references(
                uri.as_str(),
                source,
                position_after(source, resource_prefix),
                true,
            )
            .unwrap_or_else(|| panic!("resource `{resource_prefix}` should have references"));
        let from_config = backend
            .find_references(
                uri.as_str(),
                source,
                position_after(source, config_prefix),
                true,
            )
            .unwrap_or_else(|| panic!("config `{config_prefix}` should have references"));

        assert_eq!(
            from_resource, from_config,
            "reference direction should not matter for `{full}`"
        );
        assert_eq!(
            from_resource.len(),
            3,
            "resource usage, generic config usage, and declaration for `{full}`: {from_resource:#?}"
        );
        assert_eq!(
            from_resource
                .iter()
                .filter(|location| location.uri == uri)
                .count(),
            2,
            "both source spellings should be references for `{full}`"
        );

        let mut referenced_text = from_resource
            .iter()
            .map(|location| {
                if location.uri == uri {
                    text_in_range(source, location.range).to_string()
                } else {
                    let path = location.uri.to_file_path().unwrap();
                    let content = fs::read_to_string(path).unwrap();
                    text_in_range(&content, location.range).to_string()
                }
            })
            .collect::<Vec<_>>();
        referenced_text.sort();
        let mut expected = vec![short.to_string(), short.to_string(), full.to_string()];
        expected.sort();
        assert_eq!(
            referenced_text, expected,
            "references should cover exact literal payloads for `{full}`"
        );
    }
}

#[tokio::test]
async fn runtime_config_merges_open_only_their_own_diagnostic_subtree() {
    let cache_config = r#"<?php
return [
    'stores' => array_merge([
        'array' => ['driver' => 'array'],
    ], $packageStores),
    'default' => 'array',
];
"#;
    let source = r#"<?php
use Illuminate\Support\Facades\Cache;

Cache::store('a');
Cache::store('package-store');
config('cache.stores.package-store');
config('cache.missing');
"#;
    let files = [
        ("config/cache.php", cache_config),
        ("app/NamedResourceConsumer.php", source),
    ];
    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, &files);
    backend.initialized(InitializedParams {}).await;
    let uri = Url::from_file_path(dir.path().join("app/NamedResourceConsumer.php")).unwrap();
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

    let labels = completion_items(&backend, &uri, position_after(source, "Cache::store('a"))
        .await
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert_eq!(labels, ["array"]);

    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), source, &mut diagnostics);
    let invalid = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(&diagnostic.code, Some(NumberOrString::String(code))
                if code.starts_with("invalid_laravel_"))
        })
        .collect::<Vec<_>>();

    assert_eq!(invalid.len(), 1, "diagnostics: {invalid:#?}");
    assert_eq!(
        invalid[0].code,
        Some(NumberOrString::String("invalid_laravel_config".to_string()))
    );
    assert_eq!(text_in_range(source, invalid[0].range), "cache.missing");
}

#[tokio::test]
async fn literal_runtime_config_writes_declare_named_resources() {
    let source = r#"<?php
use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Config;

Config::set('cache.stores.tenant', ['driver' => 'array']);
Cache::store('tenant');
config('cache.stores.tenant');
Cache::store('missing-runtime-store');
"#;
    let (backend, _dir, uri) = open_workspace(source).await;

    let labels = completion_items(&backend, &uri, position_after(source, "Cache::store('t"))
        .await
        .into_iter()
        .map(|item| item.label)
        .collect::<Vec<_>>();
    assert_eq!(labels, ["tenant"]);

    let position = position_after(source, "Cache::store('ten");
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
        .expect("runtime resource should have a definition");
    let location = definition_location(response);
    assert_eq!(location.uri, uri);
    assert_eq!(text_in_range(source, location.range), "cache.stores.tenant");

    let references_without_declaration = backend
        .find_references(uri.as_str(), source, position, false)
        .expect("runtime resource should have usage references");
    assert_eq!(
        references_without_declaration.len(),
        2,
        "references: {references_without_declaration:#?}"
    );
    assert!(
        references_without_declaration
            .iter()
            .all(|location| location.range.start.line != 4)
    );

    let references = backend
        .find_references(uri.as_str(), source, position, true)
        .expect("runtime resource should have references");
    assert_eq!(references.len(), 3, "references: {references:#?}");

    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(uri.as_str(), source, &mut diagnostics);
    let invalid = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(&diagnostic.code, Some(NumberOrString::String(code))
                if code == "invalid_laravel_cache_store")
        })
        .collect::<Vec<_>>();
    assert_eq!(invalid.len(), 1, "diagnostics: {invalid:#?}");
    assert_eq!(
        text_in_range(source, invalid[0].range),
        "missing-runtime-store"
    );
}

#[tokio::test]
async fn closing_a_dirty_runtime_config_file_restores_saved_declarations() {
    let saved = r#"<?php
use Illuminate\Support\Facades\Config;

Config::set('cache.stores.saved', ['driver' => 'array']);
"#;
    let unsaved = r#"<?php
use Illuminate\Support\Facades\Config;

Config::set('cache.stores.unsaved-with-a-different-offset', ['driver' => 'array']);
"#;
    let consumer = r#"<?php
use Illuminate\Support\Facades\Cache;

Cache::store('saved');
config('cache.stores.saved');
"#;
    let mut files = workspace_files(consumer);
    files.push(("app/RuntimeConfig.php", saved));
    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, &files);
    backend.initialized(InitializedParams {}).await;

    let declaration_uri = Url::from_file_path(dir.path().join("app/RuntimeConfig.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: declaration_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: saved.to_string(),
            },
        })
        .await;
    backend
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: declaration_uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: unsaved.to_string(),
            }],
        })
        .await;

    let consumer_uri =
        Url::from_file_path(dir.path().join("app/NamedResourceConsumer.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: consumer_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: consumer.to_string(),
            },
        })
        .await;

    let dirty_labels = completion_items(
        &backend,
        &consumer_uri,
        position_after(consumer, "Cache::store('"),
    )
    .await
    .into_iter()
    .map(|item| item.label)
    .collect::<Vec<_>>();
    assert!(
        dirty_labels
            .iter()
            .any(|label| label == "unsaved-with-a-different-offset")
    );
    assert!(!dirty_labels.iter().any(|label| label == "saved"));

    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: declaration_uri.clone(),
            },
        })
        .await;

    let labels = completion_items(
        &backend,
        &consumer_uri,
        position_after(consumer, "Cache::store('"),
    )
    .await
    .into_iter()
    .map(|item| item.label)
    .collect::<Vec<_>>();
    assert!(labels.iter().any(|label| label == "saved"));
    assert!(
        !labels
            .iter()
            .any(|label| label == "unsaved-with-a-different-offset")
    );

    for position in [
        position_after(consumer, "Cache::store('sav"),
        position_after(consumer, "config('cache.stores.sav"),
    ] {
        let usages = backend
            .find_references(consumer_uri.as_str(), consumer, position, false)
            .expect("saved runtime resource should have a usage");
        assert_eq!(usages.len(), 2, "usage references: {usages:#?}");
        assert!(usages.iter().all(|location| location.uri == consumer_uri));

        let references = backend
            .find_references(consumer_uri.as_str(), consumer, position, true)
            .expect("saved runtime resource should have a declaration");
        assert_eq!(references.len(), 3, "references: {references:#?}");
        let declaration = references
            .iter()
            .find(|location| location.uri == declaration_uri)
            .expect("the saved Config::set literal should be the declaration");
        assert_eq!(
            text_in_range(saved, declaration.range),
            "cache.stores.saved"
        );
    }
}

#[tokio::test]
async fn provider_config_features_follow_the_open_buffer_and_restore_disk_on_close() {
    let provider = r#"<?php
namespace App\Providers;

final class PackageServiceProvider
{
    public function register(): void
    {
        $this->mergeConfigFrom(__DIR__ . '/../../resources/settings.php', 'cache');
    }
}
"#;
    let saved = r#"<?php
return [
    'stores' => [
        'saved-store' => ['driver' => 'array'],
    ],
    'value' => 'saved',
];
"#;
    let unsaved = r#"<?php
return [
    'stores' => [
        'buffer-store' => ['driver' => 'array'],
    ],
    'value' => 123,
];
"#;
    let consumer = r#"<?php
use Illuminate\Support\Facades\Cache;

Cache::store('buffer-store');
config('cache.stores.buffer-store');
Cache::store('saved-store');
$value = config('cache.value');
$value;
"#;
    let files = [
        (
            "bootstrap/providers.php",
            "<?php return [App\\Providers\\PackageServiceProvider::class];\n",
        ),
        ("app/Providers/PackageServiceProvider.php", provider),
        ("resources/settings.php", saved),
        ("app/NamedResourceConsumer.php", consumer),
    ];
    let (backend, dir) = create_psr4_workspace(COMPOSER_JSON, &files);
    backend.initialized(InitializedParams {}).await;

    let consumer_uri =
        Url::from_file_path(dir.path().join("app/NamedResourceConsumer.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: consumer_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: consumer.to_string(),
            },
        })
        .await;

    let config_uri = Url::from_file_path(dir.path().join("resources/settings.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: config_uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: unsaved.to_string(),
            },
        })
        .await;

    let labels = completion_items(
        &backend,
        &consumer_uri,
        position_after(consumer, "Cache::store('buffer"),
    )
    .await
    .into_iter()
    .map(|item| item.label)
    .collect::<Vec<_>>();
    assert!(labels.iter().any(|label| label == "buffer-store"));
    assert!(!labels.iter().any(|label| label == "saved-store"));

    let buffer_position = position_after(consumer, "Cache::store('buffer");
    let response = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: consumer_uri.clone(),
                },
                position: buffer_position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .expect("definition request should succeed")
        .expect("the buffered provider declaration should resolve");
    let location = definition_location(response);
    assert_eq!(location.uri, config_uri);
    assert_eq!(
        location.range.start,
        position_of_config_key(unsaved, "buffer-store")
    );
    let hover = backend
        .handle_hover(consumer_uri.as_str(), consumer, buffer_position)
        .expect("a provider-backed resource should have hover");
    let text = hover_text(hover);
    assert!(text.contains("**Cache store** `buffer-store`"), "{text}");
    assert!(text.contains("resources/settings.php"), "{text}");

    let references = backend
        .find_references(consumer_uri.as_str(), consumer, buffer_position, true)
        .expect("the buffered provider key should have references");
    assert_eq!(references.len(), 3, "references: {references:#?}");
    let declaration = references
        .iter()
        .find(|reference| reference.uri == config_uri)
        .expect("references should include the buffered declaration");
    assert_eq!(text_in_range(unsaved, declaration.range), "buffer-store");

    let declaration_position = position_of_config_key(unsaved, "buffer-store");
    let declaration_references = backend
        .find_references(config_uri.as_str(), unsaved, declaration_position, true)
        .expect("references should work from a provider config declaration");
    assert_eq!(declaration_references.len(), 3);

    let response = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: config_uri.clone(),
                },
                position: declaration_position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .expect("definition request should succeed")
        .expect("a provider config declaration should resolve to itself");
    let location = definition_location(response);
    assert_eq!(location.uri, config_uri);
    assert_eq!(location.range.start, declaration_position);

    let mut diagnostics = Vec::new();
    backend.collect_slow_diagnostics(consumer_uri.as_str(), consumer, &mut diagnostics);
    let invalid_cache = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(&diagnostic.code, Some(NumberOrString::String(code))
                if code == "invalid_laravel_cache_store")
        })
        .collect::<Vec<_>>();
    assert_eq!(invalid_cache.len(), 1, "diagnostics: {diagnostics:#?}");
    assert_eq!(
        text_in_range(consumer, invalid_cache[0].range),
        "saved-store"
    );

    let value_offset = consumer.rfind("$value;").expect("value use") + 2;
    let hover = backend
        .hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: consumer_uri.clone(),
                },
                position: position_at_offset(consumer, value_offset),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })
        .await
        .expect("hover request should succeed")
        .expect("config return value should have a type");
    assert!(hover_text(hover).contains("int"));

    backend
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier {
                uri: config_uri.clone(),
            },
        })
        .await;

    let labels = completion_items(
        &backend,
        &consumer_uri,
        position_after(consumer, "Cache::store('saved"),
    )
    .await
    .into_iter()
    .map(|item| item.label)
    .collect::<Vec<_>>();
    assert!(labels.iter().any(|label| label == "saved-store"));
    assert!(!labels.iter().any(|label| label == "buffer-store"));

    let saved_position = position_after(consumer, "Cache::store('saved");
    let response = backend
        .goto_definition(GotoDefinitionParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: consumer_uri.clone(),
                },
                position: saved_position,
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        })
        .await
        .expect("definition request should succeed")
        .expect("the saved provider declaration should be restored");
    let location = definition_location(response);
    assert_eq!(
        location.uri.to_file_path().unwrap().canonicalize().unwrap(),
        config_uri.to_file_path().unwrap().canonicalize().unwrap()
    );
    assert_eq!(
        location.range.start,
        position_of_config_key(saved, "saved-store")
    );

    diagnostics.clear();
    backend.collect_slow_diagnostics(consumer_uri.as_str(), consumer, &mut diagnostics);
    let invalid_cache = diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(&diagnostic.code, Some(NumberOrString::String(code))
                if code == "invalid_laravel_cache_store")
        })
        .collect::<Vec<_>>();
    assert_eq!(invalid_cache.len(), 1, "diagnostics: {diagnostics:#?}");
    assert_eq!(
        text_in_range(consumer, invalid_cache[0].range),
        "buffer-store"
    );

    let hover = backend
        .hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri: consumer_uri },
                position: position_at_offset(consumer, value_offset),
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
        })
        .await
        .expect("hover request should succeed")
        .expect("saved config return value should have a type");
    assert!(hover_text(hover).contains("string"));
}
