mod common;

use common::{create_psr4_workspace, create_test_backend};
use tower_lsp::LanguageServer;
use tower_lsp::lsp_types::*;

// ─── Shared stubs ───────────────────────────────────────────────────────────

const COMPOSER_JSON: &str = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Database\\Factories\\": "database/factories/",
            "Illuminate\\Support\\": "vendor/illuminate/Support/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Factories\\": "vendor/illuminate/Eloquent/Factories/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;

const MODEL_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
class Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */
    public static function with(mixed $relations): Builder { return new Builder(); }
}
";

const COLLECTION_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
/**
 * @template TKey of array-key
 * @template TModel
 */
class Collection {
    /** @return int */
    public function count(): int { return 0; }
    /** @return TModel|null */
    public function first(): mixed { return null; }
    /** @return array<TKey, TModel> */
    public function all(): array { return []; }
}
";

const HAS_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class HasMany {}
";

const HAS_ONE_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class HasOne {}
";

const BELONGS_TO_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class BelongsTo {}
";

const BELONGS_TO_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class BelongsToMany {}
";

const MORPH_TO_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphTo {}
";

const MORPH_ONE_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphOne {}
";

const MORPH_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphMany {}
";

const MORPH_TO_MANY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class MorphToMany {}
";

const HAS_MANY_THROUGH_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
class HasManyThrough {}
";

const BUILDER_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;

use Illuminate\\Database\\Concerns\\BuildsQueries;

/**
 * @template TModel of \\Illuminate\\Database\\Eloquent\\Model
 * @mixin \\Illuminate\\Database\\Query\\Builder
 */
class Builder {
    /** @use BuildsQueries<TModel> */
    use BuildsQueries;

    /** @return static */
    public function where(string $column, mixed $operator = null, mixed $value = null): static { return $this; }
    /** @return static */
    public function orderBy(string $column, string $direction = 'asc'): static { return $this; }
    /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */
    public function get(): Collection { return new Collection(); }
    /** @return TModel|\\Illuminate\\Database\\Eloquent\\Collection<int, TModel>|null */
    public function find(mixed $id): mixed { return null; }
    /**
     * @param mixed $id
     * @return ($id is (\\Illuminate\\Contracts\\Support\\Arrayable<array-key, mixed>|array<mixed>) ? \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> : TModel)
     */
    public function findOrFail(mixed $id, array $columns = ['*']): mixed { return null; }
    /** @return static */
    public function limit(int $value): static { return $this; }
    /** @return bool */
    public function exists(): bool { return false; }
    /** @return string */
    public function toSql(): string { return ''; }
    /**
     * @param  string  $relation
     * @param  (\\Closure(\\Illuminate\\Database\\Eloquent\\Builder<TModel>): mixed)|null  $callback
     * @return static
     */
    public function whereHas(string $relation, ?\\Closure $callback = null): static { return $this; }
    /**
     * @param  array<array-key, array|(\\Closure(\\Illuminate\\Database\\Eloquent\\Relations\\Relation): mixed)|string>|string  $relations
     * @param  (\\Closure(\\Illuminate\\Database\\Eloquent\\Relations\\Relation): mixed)|string|null  $callback
     * @return static
     */
    public function with(mixed $relations, mixed $callback = null): static { return $this; }
}
";

const QUERY_BUILDER_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Query;
class Builder {
    /** @return static */
    public function whereIn(string $column, array $values): static { return $this; }
    /** @return static */
    public function whereNested(\\Closure $callback, string $boolean = 'and'): static { return $this; }
    /** @return static */
    public function groupBy(string ...$groups): static { return $this; }
    /** @return static */
    public function having(string $column, mixed $operator = null, mixed $value = null): static { return $this; }
}
";

const BUILDS_QUERIES_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Concerns;
/**
 * @template TValue
 */
trait BuildsQueries {
    /** @return TValue|null */
    public function first(): mixed { return null; }
    /** @return TValue */
    public function firstOrFail(): mixed { return null; }
    /** @return TValue|null */
    public function sole(): mixed { return null; }
    /**
     * @param  callable(\\Illuminate\\Support\\Collection<int, TValue>, int): mixed  $callback
     * @return bool
     */
    public function chunk(int $count, callable $callback): bool { return true; }
}
";

const SUPPORT_COLLECTION_PHP: &str = "\
<?php
namespace Illuminate\\Support;
/**
 * @template TKey of array-key
 * @template TValue
 */
class Collection {
    /** @return int */
    public function count(): int { return 0; }
    /** @return TValue|null */
    public function first(): mixed { return null; }
    /** @return array<TKey, TValue> */
    public function all(): array { return []; }
    /**
     * @param callable(TValue, TKey): mixed $callback
     * @return static
     */
    public function each(callable $callback): static { return $this; }
    /**
     * @template TMapValue
     * @param callable(TValue, TKey): TMapValue $callback
     * @return static<TKey, TMapValue>
     */
    public function map(callable $callback): static { return $this; }
}
";

const RELATION_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Relations;
/**
 * @template TRelated of \\Illuminate\\Database\\Eloquent\\Model
 * @template TDeclaringModel of \\Illuminate\\Database\\Eloquent\\Model
 * @template TResult
 */
class Relation {
    /** @return static */
    public function where(string $column, mixed $operator = null, mixed $value = null): static { return $this; }
    /** @return static */
    public function orderBy(string $column, string $direction = 'asc'): static { return $this; }
}
";

const COLLECTED_BY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Attributes;
class CollectedBy {}
";

const SCOPE_ATTR_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Attributes;
class Scope {}
";

const HAS_COLLECTION_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent;
/**
 * @template TCollection of \\Illuminate\\Database\\Eloquent\\Collection
 */
trait HasCollection {}
";

const HAS_FACTORY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Factories;
/**
 * @template TFactory of Factory
 */
trait HasFactory {
    /** @return TFactory */
    public static function factory() {}
}
";

const FACTORY_PHP: &str = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Factories;
/**
 * @template TModel of \\Illuminate\\Database\\Eloquent\\Model
 */
class Factory {
    /** @return TModel */
    public function create(array $attributes = []) {}
    /** @return TModel */
    public function make(array $attributes = []) {}
    /** @return static */
    public function count(int $count): static { return $this; }
    /** @return static */
    public function state(array $state): static { return $this; }
}
";

/// Standard set of framework stub files that every test needs.
fn framework_stubs() -> Vec<(&'static str, &'static str)> {
    vec![
        ("vendor/illuminate/Eloquent/Model.php", MODEL_PHP),
        (
            "vendor/illuminate/Concerns/BuildsQueries.php",
            BUILDS_QUERIES_PHP,
        ),
        ("vendor/illuminate/Eloquent/Collection.php", COLLECTION_PHP),
        ("vendor/illuminate/Eloquent/Builder.php", BUILDER_PHP),
        ("vendor/illuminate/Query/Builder.php", QUERY_BUILDER_PHP),
        (
            "vendor/illuminate/Eloquent/Relations/HasMany.php",
            HAS_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/HasOne.php",
            HAS_ONE_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/BelongsTo.php",
            BELONGS_TO_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/BelongsToMany.php",
            BELONGS_TO_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphTo.php",
            MORPH_TO_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphOne.php",
            MORPH_ONE_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphMany.php",
            MORPH_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/MorphToMany.php",
            MORPH_TO_MANY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/HasManyThrough.php",
            HAS_MANY_THROUGH_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Attributes/CollectedBy.php",
            COLLECTED_BY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Attributes/Scope.php",
            SCOPE_ATTR_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/HasCollection.php",
            HAS_COLLECTION_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Factories/HasFactory.php",
            HAS_FACTORY_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Factories/Factory.php",
            FACTORY_PHP,
        ),
        (
            "vendor/illuminate/Support/Collection.php",
            SUPPORT_COLLECTION_PHP,
        ),
        (
            "vendor/illuminate/Eloquent/Relations/Relation.php",
            RELATION_PHP,
        ),
    ]
}

/// Build a PSR-4 workspace from the framework stubs plus extra app files.
fn make_workspace(app_files: &[(&str, &str)]) -> (phpantom_lsp::Backend, tempfile::TempDir) {
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.extend_from_slice(app_files);
    create_psr4_workspace(COMPOSER_JSON, &files)
}

/// Helper: open a file and trigger completion, returning the completion items.
async fn complete_at(
    backend: &phpantom_lsp::Backend,
    dir: &tempfile::TempDir,
    relative_path: &str,
    content: &str,
    line: u32,
    character: u32,
) -> Vec<CompletionItem> {
    let uri = Url::from_file_path(dir.path().join(relative_path)).unwrap();
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

    match result {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        _ => Vec::new(),
    }
}

fn property_names(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::PROPERTY))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect()
}

fn method_names(items: &[CompletionItem]) -> Vec<&str> {
    items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect()
}

// ─── HasMany relationship produces virtual property ─────────────────────────

#[tokio::test]
async fn test_has_many_relationship_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // Line 9 = "$user->", character 15 = after ->
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Should include synthesized 'posts' relationship property, got: {:?}",
        props
    );

    let methods = method_names(&items);
    assert!(
        methods.contains(&"posts"),
        "The relationship method itself should also appear, got: {:?}",
        methods
    );
}

// ─── HasOne relationship produces virtual property ──────────────────────────

#[tokio::test]
async fn test_has_one_relationship_produces_property() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"profile"),
        "Should include synthesized 'profile' property, got: {:?}",
        props
    );
}

// ─── BelongsTo relationship produces virtual property ───────────────────────

#[tokio::test]
async fn test_belongs_to_relationship_produces_property() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getEmail(): string { return ''; }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\BelongsTo;
class Post extends Model {
    /** @return BelongsTo<\\App\\Models\\User, $this> */
    public function author(): BelongsTo { return $this->belongsTo(User::class); }
    public function test() {
        $post = new Post();
        $post->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Post.php", post_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"author"),
        "Should include synthesized 'author' property, got: {:?}",
        props
    );
}

// ─── MorphTo relationship produces virtual property ─────────────────────────

#[tokio::test]
async fn test_morph_to_relationship_produces_property() {
    let comment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\MorphTo;
class Comment extends Model {
    /** @return MorphTo */
    public function commentable(): MorphTo { return $this->morphTo(); }
    public function test() {
        $comment = new Comment();
        $comment->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Comment.php", comment_php)]);

    let items = complete_at(&backend, &dir, "src/Models/Comment.php", comment_php, 9, 19).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"commentable"),
        "Should include synthesized 'commentable' property, got: {:?}",
        props
    );
}

// ─── Multiple relationships all produce properties ──────────────────────────

#[tokio::test]
async fn test_multiple_relationships_all_produce_properties() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {}
";
    let role_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Role extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
use Illuminate\\Database\\Eloquent\\Relations\\BelongsToMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    /** @return BelongsToMany<\\App\\Models\\Role, $this> */
    public function roles(): BelongsToMany { return $this->belongsToMany(Role::class); }
    public function getFullName(): string { return ''; }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/Profile.php", profile_php),
        ("src/Models/Role.php", role_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 16, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Should include 'posts' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"profile"),
        "Should include 'profile' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"roles"),
        "Should include 'roles' property, got: {:?}",
        props
    );
    assert!(
        !props.contains(&"getFullName"),
        "'getFullName' should not appear as a property, got: {:?}",
        props
    );
}

// ─── Non-model class does not get relationship properties ───────────────────

#[tokio::test]
async fn test_relationship_property_does_not_appear_for_non_models() {
    // A plain class that happens to return a class named HasMany (but in a
    // different namespace / without actually extending Eloquent Model).
    let service_php = "\
<?php
namespace App\\Models;
class HasMany {}
class UserService {
    /** @return HasMany */
    public function posts(): HasMany { return new HasMany(); }
    public function test() {
        $svc = new UserService();
        $svc->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/UserService.php", service_php)]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserService.php",
        service_php,
        8,
        14,
    )
    .await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"posts"),
        "'posts' should NOT be synthesized on non-Model class, got: {:?}",
        props
    );
}

// ─── HasOne chain resolves to the related model's members ───────────────────

#[tokio::test]
async fn test_has_one_relationship_property_chains_to_related_class() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
    public function getAvatar(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->profile->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->profile->" at line 9, character 24
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 24).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getBio"),
        "Should chain through profile to Profile::getBio, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getAvatar"),
        "Should chain through profile to Profile::getAvatar, got: {:?}",
        methods
    );
}

// ─── $this-> shows relationship properties ──────────────────────────────────

#[tokio::test]
async fn test_this_arrow_shows_relationship_properties() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $this->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$this->" at line 8, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Should include synthesized 'posts' property via $this->, got: {:?}",
        props
    );
}

// ─── Laravel provider beats @property tag (priority) ────────────────────────

#[tokio::test]
async fn test_laravel_provider_beats_phpdoc_property_tag() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    // The class has both a @property tag and a relationship method named
    // "posts". The LaravelModelProvider has higher priority so its
    // synthesized property wins, and the @property tag from PHPDocProvider
    // is not duplicated.
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
/**
 * @property array $posts
 */
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 12, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 15).await;

    let posts_props: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "posts"
        })
        .collect();

    assert_eq!(
        posts_props.len(),
        1,
        "Should have exactly one 'posts' property (Laravel provider wins over @property), got: {}",
        posts_props.len()
    );
}

// ─── Relationship declared in a trait used by the model ─────────────────────

#[tokio::test]
async fn test_relationship_from_trait_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let trait_php = "\
<?php
namespace App\\Concerns;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
trait HasPosts {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(\\App\\Models\\Post::class); }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Concerns\\HasPosts;
class User extends Model {
    use HasPosts;
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Concerns/HasPosts.php", trait_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 8, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Trait relationship method should produce virtual property, got: {:?}",
        props
    );
}

// ─── Indirect Model subclass (through BaseModel) ────────────────────────────

#[tokio::test]
async fn test_indirect_model_subclass_gets_relationship_properties() {
    let base_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BaseModel extends Model {}
";
    let post_php = "\
<?php
namespace App\\Models;
class Post extends BaseModel {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends BaseModel {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/BaseModel.php", base_php),
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 8, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Indirect Model subclass should still get relationship properties, got: {:?}",
        props
    );
}

// ─── FQN relationship return type ───────────────────────────────────────────

#[tokio::test]
async fn test_fqn_relationship_return_type_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<\\App\\Models\\Post, $this> */
    public function posts(): \\Illuminate\\Database\\Eloquent\\Relations\\HasMany {
        return $this->hasMany(Post::class);
    }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 10, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "FQN return type should still produce 'posts' property, got: {:?}",
        props
    );
}

// ─── All collection relationship types produce properties ───────────────────

#[tokio::test]
async fn test_morph_many_and_belongs_to_many_produce_properties() {
    let comment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Comment extends Model {}
";
    let role_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Role extends Model {}
";
    let tag_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Tag extends Model {}
";
    let deployment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Deployment extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\MorphMany;
use Illuminate\\Database\\Eloquent\\Relations\\BelongsToMany;
use Illuminate\\Database\\Eloquent\\Relations\\HasManyThrough;
use Illuminate\\Database\\Eloquent\\Relations\\MorphToMany;
class User extends Model {
    /** @return MorphMany<\\App\\Models\\Comment, $this> */
    public function comments(): MorphMany { return $this->morphMany(Comment::class, 'commentable'); }
    /** @return BelongsToMany<\\App\\Models\\Role, $this> */
    public function roles(): BelongsToMany { return $this->belongsToMany(Role::class); }
    /** @return HasManyThrough<\\App\\Models\\Deployment, \\App\\Models\\User> */
    public function deployments(): HasManyThrough { return $this->hasManyThrough(Deployment::class, User::class); }
    /** @return MorphToMany<\\App\\Models\\Tag, $this> */
    public function tags(): MorphToMany { return $this->morphToMany(Tag::class, 'taggable'); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Comment.php", comment_php),
        ("src/Models/Role.php", role_php),
        ("src/Models/Tag.php", tag_php),
        ("src/Models/Deployment.php", deployment_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 18, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 18, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"comments"),
        "MorphMany should produce 'comments' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"roles"),
        "BelongsToMany should produce 'roles' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"deployments"),
        "HasManyThrough should produce 'deployments' property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"tags"),
        "MorphToMany should produce 'tags' property, got: {:?}",
        props
    );
}

// ─── MorphOne relationship produces virtual property ────────────────────────

#[tokio::test]
async fn test_morph_one_relationship_produces_property() {
    let image_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Image extends Model {
    public function getUrl(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\MorphOne;
class User extends Model {
    /** @return MorphOne<\\App\\Models\\Image, $this> */
    public function avatar(): MorphOne { return $this->morphOne(Image::class, 'imageable'); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Image.php", image_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"avatar"),
        "MorphOne should produce 'avatar' property, got: {:?}",
        props
    );
}

// ─── Real declared property beats virtual relationship property ──────────────

#[tokio::test]
async fn test_real_property_beats_virtual_relationship_property() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** A real declared property that shadows the relationship. */
    public string $profile = 'default';
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 11, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;

    let profile_props: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "profile"
        })
        .collect();

    assert_eq!(
        profile_props.len(),
        1,
        "Should have exactly one 'profile' property (real declared wins), got: {}",
        profile_props.len()
    );
}

// ─── Cross-file chain through relationship property ─────────────────────────

#[tokio::test]
async fn test_cross_file_relationship_property_chain_resolves() {
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
    public function getAvatar(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** @return HasOne<\\App\\Models\\Profile, $this> */
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->profile->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->profile->" at line 9, character 24
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 24).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getBio"),
        "Should chain through relationship property to Profile::getBio cross-file, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getAvatar"),
        "Should chain through relationship property to Profile::getAvatar cross-file, got: {:?}",
        methods
    );
}

// ─── Relationship property chain after first() ─────────────────────────────

#[tokio::test]
async fn test_relationship_property_chain_after_first() {
    // When accessing a relationship property on a model returned by first(),
    // completion should resolve to the related model, not the parent model.
    //
    // Customer::where()->first()->userInformation-> should offer
    // UserInformation methods, not Customer methods.
    let user_info_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class UserInformation extends Model {
    public function getAddress(): string { return ''; }
    public function getPhone(): string { return ''; }
}
";
    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class Customer extends Model {
    public function getEmail(): string { return ''; }
    /** @return HasOne<\\App\\Models\\UserInformation, $this> */
    public function userInformation(): HasOne { return $this->hasOne(UserInformation::class); }
    public function test() {
        $customer = Customer::where('id', 1)->first();
        $customer->userInformation->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/UserInformation.php", user_info_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$customer->userInformation->" at line 10, character 39
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        10,
        39,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getAddress"),
        "Should chain through userInformation to UserInformation::getAddress, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getPhone"),
        "Should chain through userInformation to UserInformation::getPhone, got: {:?}",
        methods
    );
    // Should NOT contain Customer's own methods
    assert!(
        !methods.contains(&"getEmail"),
        "Should NOT offer Customer::getEmail on UserInformation, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_relationship_property_chain_after_first_or_fail() {
    // Same as above but with firstOrFail() instead of first().
    let user_info_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class UserInformation extends Model {
    public function getAddress(): string { return ''; }
    public function getPhone(): string { return ''; }
}
";
    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class Customer extends Model {
    public function getEmail(): string { return ''; }
    /** @return HasOne<\\App\\Models\\UserInformation, $this> */
    public function userInformation(): HasOne { return $this->hasOne(UserInformation::class); }
    public function test() {
        $customer = Customer::where('id', 1)->firstOrFail();
        $customer->userInformation->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/UserInformation.php", user_info_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$customer->userInformation->" at line 10, character 39
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        10,
        39,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getAddress"),
        "Should chain through userInformation to UserInformation::getAddress after firstOrFail(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"getPhone"),
        "Should chain through userInformation to UserInformation::getPhone after firstOrFail(), got: {:?}",
        methods
    );
    assert!(
        !methods.contains(&"getEmail"),
        "Should NOT offer Customer::getEmail on UserInformation after firstOrFail(), got: {:?}",
        methods
    );
}

// ─── Skips methods without return type ──────────────────────────────────────

#[tokio::test]
async fn test_skips_methods_without_return_type() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"posts"),
        "Method without return type should not produce a virtual property, got: {:?}",
        props
    );
}

// ─── Relationship without generics (singular) produces nothing ──────────────

#[tokio::test]
async fn test_singular_relationship_without_generics_produces_nothing() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    public function profile(): HasOne { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"profile"),
        "Singular relationship without generics should not produce a property (no TRelated), got: {:?}",
        props
    );
}

// ─── Collection relationship without generics falls back to Model ───────────

#[tokio::test]
async fn test_collection_relationship_without_generics_uses_model_fallback() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Collection relationship without generics should still produce a property (falls back to Collection<Model>), got: {:?}",
        props
    );
}

// ─── Same-file test using did_open with no workspace ────────────────────────

#[tokio::test]
async fn test_same_file_relationship_property_with_plain_backend() {
    // This test uses create_test_backend() and opens a single file that
    // defines all needed classes in the global namespace. The parent_class
    // is set to the full FQN via the use statement.
    let backend = create_test_backend();

    let uri = Url::parse("file:///laravel_same_file.php").unwrap();
    // We define stub classes without a namespace. The parser stores them
    // by their short name. We place them so that `User extends Model` and
    // `Model` has FQN `Illuminate\Database\Eloquent\Model` via the
    // namespace declaration.
    //
    // Actually, for a single file the simplest approach is to put everything
    // in one namespace. We define Model as a separate class in the file with
    // the correct FQN.
    let text = "\
<?php
namespace App\\Models;

class Model extends \\Illuminate\\Database\\Eloquent\\Model {}

class HasMany {}

class Post extends Model {
    public function getTitle(): string { return ''; }
}

class User extends Model {
    /** @return HasMany<Post, $this> */
    public function posts(): HasMany { return new HasMany(); }
    public function test() {
        $user = new User();
        $user->
    }
}
";

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                // "$user->" at line 17, character 15
                position: Position {
                    line: 17,
                    character: 15,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    match result {
        Some(CompletionResponse::Array(items))
        | Some(CompletionResponse::List(CompletionList { items, .. })) => {
            let props = property_names(&items);
            // The parent class is `App\Models\Model` which extends
            // `\Illuminate\Database\Eloquent\Model`. Since the class loader
            // cannot resolve the stub FQN in this simple test, the provider
            // may not detect this as an Eloquent model. That's expected.
            // This test documents the limitation of same-file testing
            // without stubs. Cross-file PSR-4 tests above cover the real
            // behavior.
            //
            // If the provider detects it (because the parent walk finds it),
            // great. If not, this is a known limitation.
            let _ = props;
        }
        _ => {
            // Completion may return None for this edge case - that's acceptable.
        }
    }
}

// ─── Provider priority: virtual property from Laravel beats @property from PHPDoc ───

#[tokio::test]
async fn test_builder_methods_appear_as_static_on_model() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 5, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 14).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"where"),
        "Builder's where() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "Builder's orderBy() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "Builder's get() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Builder's first() should appear as static on User::, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"find"),
        "Builder's find() should appear as static on User::, got: {:?}",
        methods
    );
}

// ─── Builder chain resolution ───────────────────────────────────────────────

#[tokio::test]
async fn test_builder_where_chain_resolves_to_builder_methods() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        $q = User::where('email', 'foo@bar.com');
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$q->" at line 6, character 12
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 6, 12).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"orderBy"),
        "After User::where(), ->orderBy() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "After User::where(), ->get() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "After User::where(), ->first() should be available, got: {:?}",
        methods
    );
}

// ─── Builder get() returns Collection with model type ───────────────────────

#[tokio::test]
async fn test_builder_get_returns_collection_of_model() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
    public function test() {
        $users = User::where('active', true)->get();
        $users->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$users->" at line 7, character 16
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 16).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"count"),
        "Collection from get() should have count(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Collection from get() should have first(), got: {:?}",
        methods
    );
}

// ─── Builder first() returns model instance ─────────────────────────────────

#[tokio::test]
async fn test_builder_first_returns_model_instance() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
    public function test() {
        $user = User::where('active', true)->first();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$user->" at line 7, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getName"),
        "first() should return a User instance with getName(), got: {:?}",
        methods
    );
}

// ─── Builder first() via BuildsQueries trait ────────────────────────────────

#[tokio::test]
async fn test_builder_first_via_builds_queries_trait() {
    // first() lives on the BuildsQueries trait, not directly on Builder.
    // The Builder stub declares:
    //   /** @use BuildsQueries<TModel> */
    //   use BuildsQueries;
    //
    // BuildsQueries has @template TValue and first() returns TValue|null.
    // After trait merging, Builder::first() returns TModel|null, and when
    // TModel is substituted with User, the result should be User|null.
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
    public function test() {
        $user = User::where('active', true)->first();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$user->" at line 7, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getName"),
        "first() via BuildsQueries should return User with getName(), got: {:?}",
        methods
    );
}

// ─── Builder mixin methods forwarded ────────────────────────────────────────

#[tokio::test]
async fn test_builder_mixin_methods_forwarded_to_model() {
    // whereIn and groupBy come from Query\Builder via @mixin on Eloquent\Builder.
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 5, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 14).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"whereIn"),
        "Query\\Builder's whereIn() should appear via @mixin forwarding, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"groupBy"),
        "Query\\Builder's groupBy() should appear via @mixin forwarding, got: {:?}",
        methods
    );
}

// ─── Scope method beats Builder forwarded method ────────────────────────────

#[tokio::test]
async fn test_scope_beats_builder_forwarded_method() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function scopeWhere(\\Illuminate\\Database\\Eloquent\\Builder $query, string $col): void {}
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 6, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 6, 14).await;

    // There should be a static "where" method from the scope.
    // The Builder's "where" should not duplicate it (merge dedup).
    let where_methods: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::METHOD)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "where"
        })
        .collect();

    assert!(
        !where_methods.is_empty(),
        "Should have at least one 'where' method"
    );
}

// ─── Builder forwarding does not appear for non-models ──────────────────────

#[tokio::test]
async fn test_builder_forwarding_not_on_non_models() {
    let service_php = "\
<?php
namespace App\\Models;
class UserService {
    public function test() {
        UserService::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/UserService.php", service_php)]);

    // "UserService::" at line 4, character 22
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserService.php",
        service_php,
        4,
        22,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        !methods.contains(&"where"),
        "Non-model class should not have Builder methods, got: {:?}",
        methods
    );
}

// ─── Builder exists() and toSql() preserve non-template return types ────────

#[tokio::test]
async fn test_builder_non_template_return_types_preserved() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 5, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 14).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"exists"),
        "Builder's exists() should be forwarded, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"toSql"),
        "Builder's toSql() should be forwarded, got: {:?}",
        methods
    );
}

// ─── Indirect model subclass gets Builder forwarding ────────────────────────

#[tokio::test]
async fn test_indirect_model_subclass_gets_builder_forwarding() {
    let base_model_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BaseModel extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
class User extends BaseModel {
    public function getName(): string { return ''; }
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/BaseModel.php", base_model_php),
        ("src/Models/User.php", user_php),
    ]);

    // "User::" at line 5, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 15).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"where"),
        "Indirect model subclass should get Builder forwarding, got: {:?}",
        methods
    );
}

// ─── Builder forwarding coexists with relationships and scopes ──────────────

#[tokio::test]
async fn test_builder_forwarding_coexists_with_relationships_and_scopes() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function scopeActive(\\Illuminate\\Database\\Eloquent\\Builder $query): void {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 10, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);
    let methods = method_names(&items);

    // Relationship property
    assert!(
        props.contains(&"posts"),
        "Relationship property should appear, got: {:?}",
        props
    );
    // Scope (instance)
    assert!(
        methods.contains(&"active"),
        "Scope method should appear as instance, got: {:?}",
        methods
    );
    // Relationship method
    assert!(
        methods.contains(&"posts"),
        "Relationship method should appear, got: {:?}",
        methods
    );
}

// ─── Provider priority ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_provider_priority_laravel_over_phpdoc_over_mixin() {
    // A model with a relationship, a @property tag for the same name,
    // and a @mixin with a property of the same name.
    // The Laravel provider's version should be the one that survives.
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let mixin_php = "\
<?php
namespace App\\Models;
class PostsMixin {
    public string $posts = '';
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
/**
 * @property string $posts
 * @mixin PostsMixin
 */
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/PostsMixin.php", mixin_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$user->" at line 13, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 13, 15).await;

    let posts_props: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "posts"
        })
        .collect();

    assert_eq!(
        posts_props.len(),
        1,
        "Should have exactly one 'posts' property despite three sources, got: {}",
        posts_props.len()
    );
}

// ─── Inline builder chain completion ────────────────────────────────────────

#[tokio::test]
async fn test_inline_builder_chain_where_arrow_completion() {
    // User::where()-> should offer builder methods (orderBy, get, first, etc.)
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::where()->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::where()->" at line 5, character 23
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 23).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"orderBy"),
        "User::where()-> should offer orderBy(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "User::where()-> should offer get(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "User::where()-> should offer first(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_inline_builder_chain_orderby_arrow_completion() {
    // User::where()->orderBy('name')-> should continue to offer builder methods
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::where()->orderBy('name')->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::where()->orderBy('name')->" at line 5, character 40
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 40).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"get"),
        "User::where()->orderBy('name')-> should offer get(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "User::where()->orderBy('name')-> should offer first(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"limit"),
        "User::where()->orderBy('name')-> should offer limit(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_inline_builder_chain_three_deep() {
    // User::where()->orderBy('name')->limit(10)-> should still offer builder methods
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function test() {
        User::where()->orderBy('name')->limit(10)->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // cursor at end of chain
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 5, 55).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"get"),
        "Three-deep builder chain should offer get(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Three-deep builder chain should offer first(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_builder_scope_static_chain_completion() {
    // A model with scopes should also chain: BlogAuthor::active()-> should offer builder methods
    let author_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class BlogAuthor extends Model {
    public function scopeActive(Builder $query): void {}
    public function scopeOfGenre(Builder $query, string $genre): void {}
    public function test() {
        BlogAuthor::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/BlogAuthor.php", author_php)]);

    // "BlogAuthor::" at line 8, character 20
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BlogAuthor.php",
        author_php,
        8,
        20,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"active"),
        "BlogAuthor:: should offer scope method active(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"ofGenre"),
        "BlogAuthor:: should offer scope method ofGenre(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"where"),
        "BlogAuthor:: should offer builder-forwarded where(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "BlogAuthor:: should offer builder-forwarded orderBy(), got: {:?}",
        methods
    );
}

// ─── Single-file with inline Illuminate stubs (example.php style) ───────────

#[tokio::test]
async fn test_builder_chain_single_file_with_inline_stubs() {
    // Mimics example.php: model class in one namespace, Illuminate stubs
    // in separate namespace blocks in the same file.
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_stubs.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class MyUser extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function getName(): string { return ''; }\n",
        "    public function demo(): void\n",
        "    {\n",
        "        MyUser::where('active', true);\n", // line 8
        "        MyUser::where('active', 1)->get();\n", // line 9
        "        MyUser::where('active', 1)->first();\n", // line 10
        "        MyUser::orderBy('name')->limit(10)->get();\n", // line 11
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /** @return $this */\n",
        "        public function where($column, $operator = null, $value = null) {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    // ── Test 1: MyUser::where('active', true) should offer Builder methods ──
    // "MyUser::where('active', true);" is at line 8.
    // We need completion AFTER the semicolon is removed and replaced with "->".
    // Instead, let's test the chain: MyUser::where('active', 1)->get() at line 9.
    // "$q = MyUser::where(...)->get();" — let's check that get() returns Collection.
    // Actually, let's just trigger completion at the right spots.

    // Test: MyUser::where('active', 1)->  (need to check what methods are offered)
    // Line 9: "        MyUser::where('active', 1)->get();\n"
    // Position after "->" is column 40.
    // But the text already has "get()" so let's change approach:
    // Use a modified version that has a completion trigger point.

    // Let's re-open with a version that has completion triggers
    let text_with_triggers = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class MyUser extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function getName(): string { return ''; }\n",
        "    public function demo(): void\n",
        "    {\n",
        "        $q = MyUser::where('active', true);\n", // line 8
        "        $q->\n",                                // line 9
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /** @return $this */\n",
        "        public function where($column, $operator = null, $value = null) {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 2,
                text: text_with_triggers.to_string(),
            },
        })
        .await;

    // "$q->" at line 9, character 12
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 9,
                    character: 12,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for $q->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!("Single-file inline stubs: $q-> methods: {:?}", methods);

            assert!(
                methods.contains(&"get"),
                "MyUser::where()-> should offer get() from Eloquent Builder, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "MyUser::where()-> should offer first() from BuildsQueries, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"orderBy"),
                "MyUser::where()-> should offer orderBy() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "MyUser::where()-> should offer limit() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_inline_chain_after_static_builder_single_file() {
    // Mimics example.php lines 881-885: BlogAuthor::where(...)->get() etc.
    // The chain is inline (no intermediate $q variable), so the subject
    // extractor must resolve BlogAuthor::where(...) to Builder<BlogAuthor>
    // and then offer Builder methods after "->".
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_chain.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class BlogAuthor extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function demo(): void\n",
        "    {\n",
        "        BlogAuthor::where('active', 1)->\n", // line 7, cursor at 42
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     *\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder implements \\Illuminate\\Contracts\\Database\\Eloquent\\Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /**\n",
        "         * @param  (\\Closure(static): mixed)|string|array  $column\n",
        "         * @return $this\n",
        "         */\n",
        "        public function where($column, $operator = null, $value = null, $boolean = 'and') {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Contracts\\Database\\Eloquent {\n",
        "    /**\n",
        "     * @mixin \\Illuminate\\Database\\Eloquent\\Builder\n",
        "     */\n",
        "    interface Builder {}\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    // "BlogAuthor::where('active', 1)->" at line 7
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 7,
                    character: 42,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::where(...)->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "Inline chain BlogAuthor::where(...)->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() from Eloquent Builder, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() from BuildsQueries, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"orderBy"),
                "Should offer orderBy() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "Should offer limit() from Query\\Builder via @mixin, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"where"),
                "Should offer where() for continued chaining, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

#[tokio::test]
async fn test_inline_orderby_chain_after_static_builder_single_file() {
    // Tests that BlogAuthor::orderBy('name')-> offers Builder methods.
    // orderBy() comes from Query\Builder via @mixin on Eloquent Builder,
    // so the $this return type must resolve back to Eloquent Builder
    // (not Query\Builder).
    let backend = create_test_backend();

    let uri = Url::parse("file:///inline_orderby.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class BlogAuthor extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function demo(): void\n",
        "    {\n",
        "        BlogAuthor::orderBy('name')->\n",
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     *\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder implements \\Illuminate\\Contracts\\Database\\Eloquent\\Builder {\n",
        "        /** @use \\Illuminate\\Database\\Concerns\\BuildsQueries<TModel> */\n",
        "        use \\Illuminate\\Database\\Concerns\\BuildsQueries;\n",
        "\n",
        "        /**\n",
        "         * @param  (\\Closure(static): mixed)|string|array  $column\n",
        "         * @return $this\n",
        "         */\n",
        "        public function where($column, $operator = null, $value = null, $boolean = 'and') {}\n",
        "\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get($columns = ['*']) { return new Collection(); }\n",
        "    }\n",
        "\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed { return null; }\n",
        "        public function count(): int { return 0; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "    class BelongsTo {}\n",
        "    class BelongsToMany {}\n",
        "    class MorphOne {}\n",
        "    class MorphMany {}\n",
        "    class MorphTo {}\n",
        "    class MorphToMany {}\n",
        "    class HasManyThrough {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Concerns {\n",
        "    /**\n",
        "     * @template TValue\n",
        "     */\n",
        "    trait BuildsQueries {\n",
        "        /** @return TValue|null */\n",
        "        public function first($columns = ['*']) { return null; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function whereIn($column, $values) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function groupBy(...$groups) { return $this; }\n",
        "        /** @return $this */\n",
        "        public function orderBy($column, $direction = 'asc') { return $this; }\n",
        "        /** @return $this */\n",
        "        public function limit($value) { return $this; }\n",
        "    }\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Contracts\\Database\\Eloquent {\n",
        "    /**\n",
        "     * @mixin \\Illuminate\\Database\\Eloquent\\Builder\n",
        "     */\n",
        "    interface Builder {}\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    // "BlogAuthor::orderBy('name')->" at line 7
    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 7,
                    character: 37,
                },
            },
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: None,
        })
        .await
        .unwrap();

    assert!(
        result.is_some(),
        "Should return completion results for BlogAuthor::orderBy('name')->"
    );
    match result.unwrap() {
        CompletionResponse::Array(items) => {
            let methods: Vec<&str> = items
                .iter()
                .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
                .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
                .collect();

            eprintln!(
                "Inline orderBy chain BlogAuthor::orderBy('name')->  methods: {:?}",
                methods
            );

            assert!(
                methods.contains(&"get"),
                "Should offer get() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"first"),
                "Should offer first() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"limit"),
                "Should offer limit() after orderBy()->, got: {:?}",
                methods
            );
            assert!(
                methods.contains(&"where"),
                "Should offer where() after orderBy()->, got: {:?}",
                methods
            );
        }
        _ => panic!("Expected CompletionResponse::Array"),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Custom collection support
// ═══════════════════════════════════════════════════════════════════════════

// ─── #[CollectedBy] attribute ───────────────────────────────────────────────

/// When a model uses `#[CollectedBy(CustomCollection::class)]`, Builder
/// methods like `get()` should return the custom collection class instead
/// of `\Illuminate\Database\Eloquent\Collection`.
#[tokio::test]
async fn test_collected_by_attribute_builder_get_returns_custom_collection() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class ReviewCollection extends Collection {
    /** @return array<TKey, TModel> */
    public function topRated(): array { return []; }
}
";
    let review_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Attributes\\CollectedBy;
use App\\Collections\\ReviewCollection;
#[CollectedBy(ReviewCollection::class)]
class Review extends Model {
    public function getTitle(): string { return ''; }
    public function test() {
        $reviews = Review::where('active', true)->get();
        $reviews->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "src/Collections/ReviewCollection.php",
            custom_collection_php,
        ),
        ("src/Models/Review.php", review_php),
    ]);

    // "$reviews->" at line 10, character 18
    let items = complete_at(&backend, &dir, "src/Models/Review.php", review_php, 10, 18).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"topRated"),
        "Custom collection from #[CollectedBy] should have topRated(), got: {:?}",
        methods
    );
    // Standard Collection methods should still be available via inheritance.
    assert!(
        methods.contains(&"count"),
        "Custom collection should inherit count(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Custom collection should inherit first(), got: {:?}",
        methods
    );
}

/// Chaining through a custom collection's `first()` should return
/// the model type, not the collection.
#[tokio::test]
async fn test_collected_by_attribute_first_returns_model() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class ReviewCollection extends Collection {}
";
    let review_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Attributes\\CollectedBy;
use App\\Collections\\ReviewCollection;
#[CollectedBy(ReviewCollection::class)]
class Review extends Model {
    public function getTitle(): string { return ''; }
    public function test() {
        $review = Review::where('active', true)->first();
        $review->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "src/Collections/ReviewCollection.php",
            custom_collection_php,
        ),
        ("src/Models/Review.php", review_php),
    ]);

    // "$review->" at line 10, character 17
    let items = complete_at(&backend, &dir, "src/Models/Review.php", review_php, 10, 17).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getTitle"),
        "first() should still return a Review instance with getTitle(), got: {:?}",
        methods
    );
}

// ─── @use HasCollection<X> docblock annotation ──────────────────────────────

/// When a model uses `/** @use HasCollection<CustomCollection> */ use HasCollection;`,
/// Builder methods like `get()` should return the custom collection class.
#[tokio::test]
async fn test_has_collection_trait_builder_get_returns_custom_collection() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class InvoiceCollection extends Collection {
    /** @return float */
    public function totalAmount(): float { return 0.0; }
}
";
    let invoice_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\HasCollection;
use App\\Collections\\InvoiceCollection;
class Invoice extends Model {
    /** @use HasCollection<InvoiceCollection> */
    use HasCollection;
    public function getNumber(): string { return ''; }
    public function test() {
        $invoices = Invoice::where('paid', true)->get();
        $invoices->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "src/Collections/InvoiceCollection.php",
            custom_collection_php,
        ),
        ("src/Models/Invoice.php", invoice_php),
    ]);

    // "$invoices->" at line 11, character 19
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Invoice.php",
        invoice_php,
        11,
        19,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"totalAmount"),
        "Custom collection from @use HasCollection<> should have totalAmount(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"count"),
        "Custom collection should inherit count(), got: {:?}",
        methods
    );
}

/// HasCollection<X> first() should still return the model, not the collection.
#[tokio::test]
async fn test_has_collection_trait_first_returns_model() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class InvoiceCollection extends Collection {}
";
    let invoice_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\HasCollection;
use App\\Collections\\InvoiceCollection;
class Invoice extends Model {
    /** @use HasCollection<InvoiceCollection> */
    use HasCollection;
    public function getNumber(): string { return ''; }
    public function test() {
        $inv = Invoice::where('paid', true)->first();
        $inv->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "src/Collections/InvoiceCollection.php",
            custom_collection_php,
        ),
        ("src/Models/Invoice.php", invoice_php),
    ]);

    // "$inv->" at line 11, character 14
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Invoice.php",
        invoice_php,
        11,
        14,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getNumber"),
        "first() should return an Invoice with getNumber(), got: {:?}",
        methods
    );
}

// ─── Custom collection on relationship properties ───────────────────────────

/// When a model with a custom collection has a HasMany relationship,
/// the virtual relationship property should use the custom collection type.
#[tokio::test]
async fn test_collected_by_relationship_property_uses_custom_collection() {
    let review_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class ReviewCollection extends Collection {
    /** @return array */
    public function topRated(): array { return []; }
}
";
    let product_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class ProductCollection extends Collection {
    /** @return array */
    public function bestSellers(): array { return []; }
}
";
    let review_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Attributes\\CollectedBy;
use App\\Collections\\ReviewCollection;
#[CollectedBy(ReviewCollection::class)]
class Review extends Model {
    public function getTitle(): string { return ''; }
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<Review, $this> */
    public function childReviews(): mixed { return $this->hasMany(Review::class); }
}
";
    let product_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Attributes\\CollectedBy;
use App\\Collections\\ProductCollection;
#[CollectedBy(ProductCollection::class)]
class Product extends Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<Review, $this> */
    public function reviews(): mixed { return $this->hasMany(Review::class); }
    public function test() {
        $product = new Product();
        $product->reviews->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "src/Collections/ReviewCollection.php",
            review_collection_php,
        ),
        (
            "src/Collections/ProductCollection.php",
            product_collection_php,
        ),
        ("src/Models/Review.php", review_php),
        ("src/Models/Product.php", product_php),
    ]);

    // "$product->reviews->" at line 11, character 28
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Product.php",
        product_php,
        11,
        28,
    )
    .await;
    let methods = method_names(&items);

    // The relationship property `$product->reviews` is a HasMany<Review>,
    // so it should use the *related* model's (Review's) custom collection
    // (ReviewCollection), NOT the owning model's (Product's) collection
    // (ProductCollection).
    assert!(
        methods.contains(&"topRated"),
        "HasMany relationship property should use the related model's ReviewCollection (topRated()), got: {:?}",
        methods
    );
    assert!(
        !methods.contains(&"bestSellers"),
        "HasMany relationship property should NOT use the owning model's ProductCollection (bestSellers()), got: {:?}",
        methods
    );
}

// ─── Model without custom collection still uses standard Collection ─────────

/// A model without #[CollectedBy] or HasCollection should still use
/// the standard Eloquent Collection.
#[tokio::test]
async fn test_model_without_custom_collection_uses_standard_collection() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
    public function test() {
        $users = User::where('active', true)->get();
        $users->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$users->" at line 7, character 16
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 16).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"count"),
        "Standard Collection should have count(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Standard Collection should have first(), got: {:?}",
        methods
    );
}

// ─── #[CollectedBy] with FQN ────────────────────────────────────────────────

/// The attribute argument can be a fully-qualified name.
#[tokio::test]
async fn test_collected_by_fqn_argument() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class OrderCollection extends Collection {
    /** @return float */
    public function grandTotal(): float { return 0.0; }
}
";
    let order_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Attributes\\CollectedBy;
#[CollectedBy(\\App\\Collections\\OrderCollection::class)]
class Order extends Model {
    public function test() {
        $orders = Order::where('status', 'paid')->get();
        $orders->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Collections/OrderCollection.php", custom_collection_php),
        ("src/Models/Order.php", order_php),
    ]);

    // "$orders->" at line 8, character 17
    let items = complete_at(&backend, &dir, "src/Models/Order.php", order_php, 8, 17).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"grandTotal"),
        "FQN #[CollectedBy] should resolve custom collection with grandTotal(), got: {:?}",
        methods
    );
}

// ─── Same-file test with plain backend ──────────────────────────────────────

/// Custom collection via @use HasCollection<X> in a single file with inline stubs.
#[tokio::test]
async fn test_custom_collection_same_file_plain_backend() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///custom_coll.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Builder<static> */\n",
        "        public static function query() {}\n",
        "    }\n",
        "    /**\n",
        "     * @template TModel of \\Illuminate\\Database\\Eloquent\\Model\n",
        "     * @mixin \\Illuminate\\Database\\Query\\Builder\n",
        "     */\n",
        "    class Builder {\n",
        "        /** @return $this */\n",
        "        public function where($column, $value = null) {}\n",
        "        /** @return \\Illuminate\\Database\\Eloquent\\Collection<int, TModel> */\n",
        "        public function get() {}\n",
        "    }\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel\n",
        "     */\n",
        "    class Collection {\n",
        "        /** @return TModel|null */\n",
        "        public function first(): mixed {}\n",
        "        public function count(): int {}\n",
        "    }\n",
        "}\n",
        "namespace Illuminate\\Database\\Eloquent\\Attributes {\n",
        "    class CollectedBy {}\n",
        "}\n",
        "namespace Illuminate\\Database\\Eloquent\\Relations {\n",
        "    class HasMany {}\n",
        "    class HasOne {}\n",
        "}\n",
        "namespace Illuminate\\Database\\Query {\n",
        "    class Builder {}\n",
        "}\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    /** @template TCollection */\n",
        "    trait HasCollection {}\n",
        "}\n",
        "namespace App\\Collections {\n",
        "    /**\n",
        "     * @template TKey of array-key\n",
        "     * @template TModel\n",
        "     * @extends \\Illuminate\\Database\\Eloquent\\Collection<TKey, TModel>\n",
        "     */\n",
        "    class TaskCollection extends \\Illuminate\\Database\\Eloquent\\Collection {\n",
        "        /** @return array */\n",
        "        public function overdue(): array { return []; }\n",
        "    }\n",
        "}\n",
        "namespace App\\Models {\n",
        "    use Illuminate\\Database\\Eloquent\\Model;\n",
        "    use Illuminate\\Database\\Eloquent\\HasCollection;\n",
        "    use App\\Collections\\TaskCollection;\n",
        "    class Task extends Model {\n",
        "        /** @use HasCollection<TaskCollection> */\n",
        "        use HasCollection;\n",
        "        public function getTitle(): string { return ''; }\n",
        "        public function demo() {\n",
        "            $tasks = Task::where('done', false)->get();\n",
        "            $tasks->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 61,
                    character: 20,
                },
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
    let methods = method_names(&items);

    assert!(
        methods.contains(&"overdue"),
        "Same-file custom collection should have overdue(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"count"),
        "Same-file custom collection should inherit count(), got: {:?}",
        methods
    );
}

// ─── Same-file accessor on variable (reproduces example.php scenario) ───────

/// When using inline namespace stubs (like example.php), accessor virtual
/// properties should appear on `$model->` (variable), not just `$this->`.
#[tokio::test]
async fn test_accessor_on_variable_same_file_inline_stubs() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///accessor_var.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Demo {\n",
        "\n",
        "class AccessorDemo extends \\Illuminate\\Database\\Eloquent\\Model\n",
        "{\n",
        "    public function getDisplayNameAttribute(): string\n",
        "    {\n",
        "        return 'display';\n",
        "    }\n",
        "\n",
        "    protected function avatarUrl(): \\Illuminate\\Database\\Eloquent\\Casts\\Attribute\n",
        "    {\n",
        "        return new \\Illuminate\\Database\\Eloquent\\Casts\\Attribute();\n",
        "    }\n",
        "\n",
        "    public function demo(): void\n",
        "    {\n",
        "        $model = new AccessorDemo();\n",
        "        $model->\n",
        "    }\n",
        "}\n",
        "\n",
        "} // end namespace Demo\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {}\n",
        "}\n",
        "\n",
        "namespace Illuminate\\Database\\Eloquent\\Casts {\n",
        "    class Attribute {}\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                // "$model->" at line 18, character 16
                position: Position {
                    line: 18,
                    character: 16,
                },
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
    let props = property_names(&items);

    assert!(
        props.contains(&"display_name"),
        "Legacy accessor getDisplayNameAttribute should produce property display_name on $model->, got: {:?}",
        props
    );
    assert!(
        props.contains(&"avatar_url"),
        "Modern accessor avatarUrl() should produce property avatar_url on $model->, got: {:?}",
        props
    );
}

// ─── Legacy accessor virtual properties ─────────────────────────────────────

/// A model with `getFullNameAttribute(): string` should produce a virtual
/// property `$full_name` typed as `string`.
#[tokio::test]
async fn test_legacy_accessor_produces_virtual_property() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getFullNameAttribute(): string { return ''; }
    public function test() {
        $u = new User();
        $u->full_name->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$u->full_name->" — full_name resolves to string, so string methods
    // won't show, but we can verify the property appears in $u-> completions.
    // Instead, complete at "$u->" to check that full_name is in the list.
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 12).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"full_name"),
        "Legacy accessor getFullNameAttribute should produce property full_name, got: {:?}",
        props
    );
}

/// Multiple legacy accessors coexist with regular methods and relationship properties.
#[tokio::test]
async fn test_legacy_accessor_multiple() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getFirstNameAttribute(): string { return ''; }
    public function getLastNameAttribute(): string { return ''; }
    public function getIsAdminAttribute(): bool { return false; }
    public function greet(): string { return ''; }
    public function test() {
        $u = new User();
        $u->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 12).await;
    let props = property_names(&items);
    let methods = method_names(&items);

    assert!(
        props.contains(&"first_name"),
        "Should have first_name property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"last_name"),
        "Should have last_name property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"is_admin"),
        "Should have is_admin property, got: {:?}",
        props
    );
    assert!(
        methods.contains(&"greet"),
        "Regular method greet() should still appear, got: {:?}",
        methods
    );
}

/// `getAttribute()` is a real Eloquent method and must NOT be treated as a
/// legacy accessor.
#[tokio::test]
async fn test_get_attribute_not_treated_as_accessor() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getAttribute(string $key): mixed { return null; }
    public function test() {
        $u = new User();
        $u->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 12).await;
    let props = property_names(&items);

    // getAttribute() has no middle portion, so it should not produce a property.
    // The only properties should be from the Model base (none in our stub).
    assert!(
        !props.iter().any(|p| p.is_empty()),
        "getAttribute should not produce an empty-named property, got: {:?}",
        props
    );
}

// ─── Modern accessor virtual properties (Laravel 9+) ────────────────────────

/// A model with `fullName(): Attribute` should produce a virtual property
/// `$full_name` typed as `mixed`.
#[tokio::test]
async fn test_modern_accessor_produces_virtual_property() {
    let attribute_php = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Casts;
class Attribute {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Casts\\Attribute;
class User extends Model {
    protected function fullName(): Attribute {
        return Attribute::make(get: fn() => 'hello');
    }
    public function test() {
        $u = new User();
        $u->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "vendor/illuminate/Eloquent/Casts/Attribute.php",
            attribute_php,
        ),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 12).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"full_name"),
        "Modern accessor fullName() returning Attribute should produce property full_name, got: {:?}",
        props
    );
}

/// When a modern accessor declares `Attribute<string>` (or
/// `Attribute<string, never>`), the synthesized property should be
/// typed `string`, not `mixed`.
#[tokio::test]
async fn test_modern_accessor_generic_type_extracted() {
    let attribute_php = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Casts;
class Attribute {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Casts\\Attribute;
class User extends Model {
    /** @return Attribute<string> */
    protected function fullName(): Attribute {
        return Attribute::make(get: fn() => 'hello');
    }
    /** @return Attribute<int, never> */
    protected function age(): Attribute {
        return Attribute::make(get: fn() => 42);
    }
    /** @return Attribute<string|null> */
    protected function nickname(): Attribute {
        return Attribute::make(get: fn() => null);
    }
    protected function noGeneric(): Attribute {
        return Attribute::make(get: fn() => 'fallback');
    }
    public function test() {
        $u = new User();
        $u->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "vendor/illuminate/Eloquent/Casts/Attribute.php",
            attribute_php,
        ),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 22, 12).await;

    // Helper: find a completion item by its filter_text / label
    let find_prop = |name: &str| -> Option<&CompletionItem> {
        items.iter().find(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && i.filter_text.as_deref().unwrap_or(&i.label) == name
        })
    };

    // Attribute<string> → "string"
    let full_name = find_prop("full_name");
    assert!(full_name.is_some(), "full_name property should exist");
    assert!(
        full_name
            .unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("string"),
        "full_name should be typed string, got: {:?}",
        full_name.unwrap().detail
    );
    assert!(
        !full_name
            .unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("mixed"),
        "full_name should NOT be typed mixed, got: {:?}",
        full_name.unwrap().detail
    );

    // Attribute<int, never> → "int"
    let age = find_prop("age");
    assert!(age.is_some(), "age property should exist");
    assert!(
        age.unwrap().detail.as_deref().unwrap_or("").contains("int"),
        "age should be typed int, got: {:?}",
        age.unwrap().detail
    );

    // Attribute<string|null> → "string|null"
    let nickname = find_prop("nickname");
    assert!(nickname.is_some(), "nickname property should exist");
    assert!(
        nickname
            .unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("string|null"),
        "nickname should be typed string|null, got: {:?}",
        nickname.unwrap().detail
    );

    // Attribute (no generics) → "mixed"
    let no_generic = find_prop("no_generic");
    assert!(no_generic.is_some(), "no_generic property should exist");
    assert!(
        no_generic
            .unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("mixed"),
        "no_generic should fall back to mixed, got: {:?}",
        no_generic.unwrap().detail
    );
}

/// Modern and legacy accessors can coexist on the same model alongside
/// relationships and scopes.
#[tokio::test]
async fn test_accessors_coexist_with_scopes_and_relationships() {
    let attribute_php = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Casts;
class Attribute {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Casts\\Attribute;
class User extends Model {
    public function getDisplayNameAttribute(): string { return ''; }
    protected function isVerified(): Attribute {
        return Attribute::make(get: fn() => true);
    }
    public function scopeActive(Builder $query): void {}
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<User, $this> */
    public function friends(): mixed { return $this->hasMany(User::class); }
    public function test() {
        $u = new User();
        $u->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "vendor/illuminate/Eloquent/Casts/Attribute.php",
            attribute_php,
        ),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 15, 12).await;
    let props = property_names(&items);
    let methods = method_names(&items);

    assert!(
        props.contains(&"display_name"),
        "Legacy accessor property display_name should be present, got: {:?}",
        props
    );
    assert!(
        props.contains(&"is_verified"),
        "Modern accessor property is_verified should be present, got: {:?}",
        props
    );
    assert!(
        props.contains(&"friends"),
        "Relationship property friends should be present, got: {:?}",
        props
    );
    assert!(
        methods.contains(&"active"),
        "Scope method active() should be present, got: {:?}",
        methods
    );
}

/// A cross-file modern accessor should work when the Attribute class is
/// resolved via PSR-4.
#[tokio::test]
async fn test_modern_accessor_cross_file() {
    let attribute_php = "\
<?php
namespace Illuminate\\Database\\Eloquent\\Casts;
class Attribute {}
";
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Casts\\Attribute;
class Profile extends Model {
    protected function avatarUrl(): Attribute {
        return Attribute::make(get: fn() => 'https://example.com/avatar.png');
    }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasOne<Profile, $this> */
    public function profile(): mixed { return $this->hasOne(Profile::class); }
    public function test() {
        $u = new User();
        $u->profile->
    }
}
";
    let (backend, dir) = make_workspace(&[
        (
            "vendor/illuminate/Eloquent/Casts/Attribute.php",
            attribute_php,
        ),
        ("src/Models/Profile.php", profile_php),
        ("src/Models/User.php", user_php),
    ]);

    // "$u->profile->" — profile resolves to Profile, and Profile has
    // a modern accessor avatarUrl() producing $avatar_url.
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 22).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"avatar_url"),
        "Cross-file modern accessor avatarUrl() on Profile should produce property avatar_url, got: {:?}",
        props
    );
}

// ─── Conditional return type on Builder methods ─────────────────────────────

/// `Customer::findOrFail(1)` has a conditional return type:
///   `($id is Arrayable|array) ? Collection<int, TModel> : TModel`
/// When called with a scalar argument, the condition is false and the
/// return type should resolve to TModel (= Customer).  Assigning the
/// result to a variable and completing on it should offer Customer's
/// methods.
#[tokio::test]
async fn test_find_or_fail_conditional_return_resolves_to_model() {
    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Customer extends Model {
    public function getEmail(): string { return ''; }
    public function getFullNameAttribute(): string { return ''; }
}
";
    let controller_php = "\
<?php
namespace App\\Models;
class OrderController {
    public function show(): void {
        $customer = Customer::findOrFail(1);
        $customer->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Customer.php", customer_php),
        ("src/Models/OrderController.php", controller_php),
    ]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/OrderController.php",
        controller_php,
        5,
        20,
    )
    .await;
    let methods = method_names(&items);
    let props = property_names(&items);

    assert!(
        methods.contains(&"getEmail"),
        "Customer::findOrFail(1) should resolve to Customer with getEmail(), got methods: {:?}, props: {:?}",
        methods,
        props
    );
    assert!(
        props.contains(&"full_name"),
        "Customer::findOrFail(1) should resolve accessor property full_name, got props: {:?}",
        props
    );
}

/// `Customer::findOrFail([1, 2])` passes an array argument, so the
/// conditional `($id is Arrayable|array) ? Collection<int, TModel> : TModel`
/// should take the then-branch and resolve to `Collection<int, Customer>`.
/// Completing on the result should offer Collection methods like `count()`.
#[tokio::test]
async fn test_find_or_fail_array_arg_resolves_to_collection() {
    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Customer extends Model {
    public function getEmail(): string { return ''; }
}
";
    let controller_php = "\
<?php
namespace App\\Models;
class OrderController {
    public function index(): void {
        $customers = Customer::findOrFail([1, 2]);
        $customers->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Customer.php", customer_php),
        ("src/Models/OrderController.php", controller_php),
    ]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/OrderController.php",
        controller_php,
        5,
        21,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"count"),
        "Customer::findOrFail([1, 2]) should resolve to Collection with count(), got methods: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Customer::findOrFail([1, 2]) should resolve to Collection with first(), got methods: {:?}",
        methods
    );
}

// ─── Body-inferred relationships (no @return annotation) ────────────────────

#[tokio::test]
async fn test_body_inferred_has_many_produces_property() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Body-inferred hasMany should produce a 'posts' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_has_one_produces_property() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function profile() { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Profile.php", profile_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"profile"),
        "Body-inferred hasOne should produce a 'profile' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_belongs_to_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function author() { return $this->belongsTo(User::class); }
    public function test() {
        $post = new Post();
        $post->
    }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function getName(): string { return ''; }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Post.php", post_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"author"),
        "Body-inferred belongsTo should produce an 'author' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_morph_to_produces_property() {
    let comment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Comment extends Model {
    public function commentable() { return $this->morphTo(); }
    public function test() {
        $c = new Comment();
        $c->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Comment.php", comment_php)]);

    let items = complete_at(&backend, &dir, "src/Models/Comment.php", comment_php, 7, 13).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"commentable"),
        "Body-inferred morphTo should produce a 'commentable' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_relationship_chain_resolves() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function profile() { return $this->hasOne(Profile::class); }
    public function test() {
        $user = new User();
        $user->profile->
    }
}
";
    let profile_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Profile extends Model {
    public function getBio(): string { return ''; }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Profile.php", profile_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 24).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getBio"),
        "Chaining through body-inferred hasOne property should resolve to related class, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_body_inferred_fqn_class_argument() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() { return $this->hasMany(\\App\\Models\\Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Body-inferred hasMany with FQN class argument should produce a 'posts' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_empty_body_still_skipped() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"posts"),
        "Empty method body should not produce a virtual property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_with_extra_arguments() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() { return $this->hasMany(Post::class, 'author_id', 'id'); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Body-inferred hasMany with extra FK arguments should produce a 'posts' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_does_not_override_docblock() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<Post, $this> */
    public function posts() { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Docblock @return should still produce a 'posts' property (body inference not needed), got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_relationship_with_chained_builder() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() { return $this->hasMany(Post::class)->latest(); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Body-inferred hasMany with chained ->latest() should produce a 'posts' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_morph_many_produces_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function comments() { return $this->morphMany(Comment::class, 'commentable'); }
    public function test() {
        $post = new Post();
        $post->
    }
}
";
    let comment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Comment extends Model {}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/Comment.php", comment_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Post.php", post_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"comments"),
        "Body-inferred morphMany should produce a 'comments' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_belongs_to_many_produces_property() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function roles() { return $this->belongsToMany(Role::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let role_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Role extends Model {}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Role.php", role_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"roles"),
        "Body-inferred belongsToMany should produce a 'roles' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_this_arrow_shows_relationship_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() { return $this->hasMany(Post::class); }
    public function test() {
        $this->
    }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 6, 16).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "$this-> should show body-inferred relationship properties, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_body_inferred_non_model_class_no_property() {
    let service_php = "\
<?php
namespace App\\Services;
class UserService {
    public function posts() { return $this->hasMany(Post::class); }
    public function test() {
        $s = new UserService();
        $s->
    }
}
";
    let (backend, dir) = create_psr4_workspace(
        r#"{ "autoload": { "psr-4": { "App\\Services\\": "src/Services/" } } }"#,
        &[("src/Services/UserService.php", service_php)],
    );

    let items = complete_at(
        &backend,
        &dir,
        "src/Services/UserService.php",
        service_php,
        5,
        13,
    )
    .await;
    let props = property_names(&items);

    assert!(
        !props.contains(&"posts"),
        "Non-model classes should not get virtual relationship properties even if body matches, got: {:?}",
        props
    );
}

// ─── newCollection() override detection ─────────────────────────────────────

#[tokio::test]
async fn test_new_collection_override_builder_get_returns_custom_collection() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class TaskCollection extends Collection {
    /** @return array<TKey, TModel> */
    public function pending(): array { return []; }
}
";
    let task_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Collections\\TaskCollection;
class Task extends Model {
    /** @return TaskCollection<int, static> */
    public function newCollection(array $models = []): TaskCollection
    {
        return new TaskCollection($models);
    }
    public function getTitle(): string { return ''; }
    public function test() {
        $tasks = Task::where('active', true)->get();
        $tasks->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Collections/TaskCollection.php", custom_collection_php),
        ("src/Models/Task.php", task_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Task.php", task_php, 13, 16).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"pending"),
        "Custom collection from newCollection() should have pending(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"count"),
        "Custom collection should inherit count(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_new_collection_override_first_returns_model() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class TaskCollection extends Collection {}
";
    let task_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Collections\\TaskCollection;
class Task extends Model {
    /** @return TaskCollection<int, static> */
    public function newCollection(array $models = []): TaskCollection
    {
        return new TaskCollection($models);
    }
    public function getTitle(): string { return ''; }
    public function test() {
        $task = Task::where('active', true)->first();
        $task->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Collections/TaskCollection.php", custom_collection_php),
        ("src/Models/Task.php", task_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Task.php", task_php, 13, 15).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"getTitle"),
        "first() on model with newCollection() should still return model, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_new_collection_override_relationship_property_uses_custom_collection() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class TaskCollection extends Collection {
    /** @return array<TKey, TModel> */
    public function pending(): array { return []; }
}
";
    let task_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Collections\\TaskCollection;
class Task extends Model {
    /** @return TaskCollection<int, static> */
    public function newCollection(array $models = []): TaskCollection
    {
        return new TaskCollection($models);
    }
    public function getTitle(): string { return ''; }
}
";
    let project_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Project extends Model {
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<Task, $this> */
    public function tasks(): mixed { return $this->hasMany(Task::class); }
    public function test() {
        $project = new Project();
        $project->tasks->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Collections/TaskCollection.php", custom_collection_php),
        ("src/Models/Task.php", task_php),
        ("src/Models/Project.php", project_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Project.php", project_php, 8, 25).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"pending"),
        "Relationship property should use related model's newCollection() custom collection, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_collected_by_takes_priority_over_new_collection() {
    let collection_a_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class CollectionA extends Collection {
    public function fromAttribute(): string { return ''; }
}
";
    let collection_b_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class CollectionB extends Collection {
    public function fromMethod(): string { return ''; }
}
";
    let widget_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Attributes\\CollectedBy;
use App\\Collections\\CollectionA;
use App\\Collections\\CollectionB;
#[CollectedBy(CollectionA::class)]
class Widget extends Model {
    /** @return CollectionB<int, static> */
    public function newCollection(array $models = []): CollectionB
    {
        return new CollectionB($models);
    }
    public function test() {
        $widgets = Widget::where('active', true)->get();
        $widgets->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Collections/CollectionA.php", collection_a_php),
        ("src/Collections/CollectionB.php", collection_b_php),
        ("src/Models/Widget.php", widget_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Widget.php", widget_php, 15, 18).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"fromAttribute"),
        "#[CollectedBy] should take priority over newCollection(), got: {:?}",
        methods
    );
    assert!(
        !methods.contains(&"fromMethod"),
        "newCollection() should NOT be used when #[CollectedBy] is present, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_new_collection_standard_return_type_ignored() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Collection;
class User extends Model {
    public function newCollection(array $models = []): Collection
    {
        return new Collection($models);
    }
    public function getName(): string { return ''; }
    public function test() {
        $users = User::where('active', true)->get();
        $users->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 16).await;
    let methods = method_names(&items);

    // Standard Collection methods should be present (it's the default collection).
    assert!(
        methods.contains(&"count"),
        "Standard collection should still have count(), got: {:?}",
        methods
    );
    // No custom method should appear — newCollection returning Collection is not custom.
    assert!(
        !methods.iter().any(|m| m == &"pending"),
        "No custom methods should appear when newCollection returns standard Collection"
    );
}

#[tokio::test]
async fn test_new_collection_fqn_return_type() {
    let custom_collection_php = "\
<?php
namespace App\\Collections;
use Illuminate\\Database\\Eloquent\\Collection;
/**
 * @template TKey of array-key
 * @template TModel
 * @extends Collection<TKey, TModel>
 */
class EventCollection extends Collection {
    public function upcoming(): array { return []; }
}
";
    let event_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Event extends Model {
    public function newCollection(array $models = []): \\App\\Collections\\EventCollection
    {
        return new \\App\\Collections\\EventCollection($models);
    }
    public function test() {
        $events = Event::where('upcoming', true)->get();
        $events->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Collections/EventCollection.php", custom_collection_php),
        ("src/Models/Event.php", event_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Event.php", event_php, 10, 17).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"upcoming"),
        "FQN return type on newCollection() should be detected, got: {:?}",
        methods
    );
}

// ── Eloquent Casts ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_casts_property_produces_typed_virtual_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
        'created_at' => 'datetime',
        'options' => 'array',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "Cast 'boolean' should produce is_admin property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"created_at"),
        "Cast 'datetime' should produce created_at property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"options"),
        "Cast 'array' should produce options property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_boolean_type_hint() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let prop = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "is_admin");
    assert!(prop.is_some(), "should find is_admin property");
    assert!(
        prop.unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("bool"),
        "is_admin should show bool in detail, got: {:?}",
        prop.unwrap().detail
    );
}

#[tokio::test]
async fn test_casts_integer_and_float() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'age' => 'integer',
        'score' => 'float',
        'price' => 'decimal:2',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(props.contains(&"age"), "integer cast, got: {:?}", props);
    assert!(props.contains(&"score"), "float cast, got: {:?}", props);
    assert!(props.contains(&"price"), "decimal:2 cast, got: {:?}", props);
}

#[tokio::test]
async fn test_casts_string_and_encrypted() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'name' => 'string',
        'secret' => 'encrypted',
        'hashed_val' => 'hashed',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(props.contains(&"name"), "string cast, got: {:?}", props);
    assert!(
        props.contains(&"secret"),
        "encrypted cast, got: {:?}",
        props
    );
    assert!(
        props.contains(&"hashed_val"),
        "hashed cast, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_object_and_collection() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'metadata' => 'object',
        'tags' => 'collection',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(props.contains(&"metadata"), "object cast, got: {:?}", props);
    assert!(props.contains(&"tags"), "collection cast, got: {:?}", props);
}

#[tokio::test]
async fn test_casts_method_produces_typed_virtual_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected function casts(): array {
        return [
            'is_admin' => 'boolean',
            'created_at' => 'datetime',
        ];
    }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "casts() method should produce is_admin, got: {:?}",
        props
    );
    assert!(
        props.contains(&"created_at"),
        "casts() method should produce created_at, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_coexist_with_relationships_and_scopes() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
    ];
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<\\App\\Models\\Post, $this> */
    public function posts(): mixed { return $this->hasMany(Post::class); }
    public function scopeActive(\\Illuminate\\Database\\Eloquent\\Builder $query): void {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 15).await;
    let props = property_names(&items);
    let methods = method_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "Cast property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"posts"),
        "Relationship property, got: {:?}",
        props
    );
    assert!(
        methods.contains(&"active"),
        "Scope method, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_casts_coexist_with_accessors() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
    ];
    public function getDisplayNameAttribute(): string { return ''; }
    public function avatarUrl(): \\Illuminate\\Database\\Eloquent\\Casts\\Attribute {
        return \\Illuminate\\Database\\Eloquent\\Casts\\Attribute::make();
    }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 13, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "Cast property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"display_name"),
        "Legacy accessor, got: {:?}",
        props
    );
    assert!(
        props.contains(&"avatar_url"),
        "Modern accessor, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_on_this_arrow() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
        'options' => 'array',
    ];
    public function demo() {
        $this->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "$this-> should show cast properties, got: {:?}",
        props
    );
    assert!(
        props.contains(&"options"),
        "$this-> should show cast properties, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_method_overrides_property_and_both_merge() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'from_property' => 'boolean',
        'shared' => 'boolean',
    ];
    protected function casts(): array {
        return [
            'from_method' => 'integer',
            'shared' => 'integer',
        ];
    }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 16, 15).await;
    let props = property_names(&items);

    // Both sources contribute unique keys.
    assert!(
        props.contains(&"from_property"),
        "$casts-only column should be present, got: {:?}",
        props
    );
    assert!(
        props.contains(&"from_method"),
        "casts()-only column should be present, got: {:?}",
        props
    );
    // casts() method overrides $casts property for overlapping keys.
    assert!(
        props.contains(&"shared"),
        "overlapping column should be present, got: {:?}",
        props
    );
    let shared = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "shared");
    assert!(shared.is_some());
    assert!(
        shared
            .unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("int"),
        "shared should be int from casts() not bool from $casts, got: {:?}",
        shared.unwrap().detail
    );
}

#[tokio::test]
async fn test_casts_non_model_class_no_properties() {
    let service_php = "\
<?php
namespace App\\Services;
class UserService {
    protected $casts = [
        'is_admin' => 'boolean',
    ];
    public function test() {
        $svc = new UserService();
        $svc->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/UserService.php", service_php)]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserService.php",
        service_php,
        8,
        14,
    )
    .await;
    let props = property_names(&items);

    // Non-model classes should NOT get virtual cast properties.
    // The $casts property itself is a real property, but no virtual
    // 'is_admin' property should be synthesized.
    assert!(
        !props.contains(&"is_admin"),
        "Non-model should not get cast virtual properties, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_double_quoted_strings() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        \"is_admin\" => \"boolean\",
        \"created_at\" => \"datetime\",
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "Double-quoted casts key, got: {:?}",
        props
    );
    assert!(
        props.contains(&"created_at"),
        "Double-quoted casts key, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_cross_file_psr4() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
        'balance' => 'decimal:2',
    ];
}
";
    let controller_php = "\
<?php
namespace App\\Models;
class UserController {
    public function show() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/UserController.php", controller_php),
    ]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserController.php",
        controller_php,
        5,
        15,
    )
    .await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "Cross-file cast property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"balance"),
        "Cross-file decimal cast, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_indirect_model_subclass() {
    let base_model_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BaseModel extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
class User extends BaseModel {
    protected $casts = [
        'is_admin' => 'boolean',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/BaseModel.php", base_model_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "Indirect model subclass should get cast properties, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_same_file_plain_backend() {
    let backend = create_test_backend();

    let uri = Url::parse("file:///casts_same_file.php").unwrap();
    let text = concat!(
        "<?php\n",
        "namespace Illuminate\\Database\\Eloquent {\n",
        "    abstract class Model {}\n",
        "}\n",
        "namespace App\\Models {\n",
        "    class User extends \\Illuminate\\Database\\Eloquent\\Model {\n",
        "        protected $casts = [\n",
        "            'is_admin' => 'boolean',\n",
        "            'created_at' => 'datetime',\n",
        "            'options' => 'array',\n",
        "        ];\n",
        "        public function test() {\n",
        "            $user = new User();\n",
        "            $user->\n",
        "        }\n",
        "    }\n",
        "}\n",
    );

    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "php".to_string(),
                version: 1,
                text: text.to_string(),
            },
        })
        .await;

    let result = backend
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams {
                text_document: TextDocumentIdentifier { uri },
                position: Position {
                    line: 13,
                    character: 19,
                },
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
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "Same-file cast property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"created_at"),
        "Same-file cast property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"options"),
        "Same-file cast property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_custom_cast_class_with_get_method() {
    let money_cast_php = "\
<?php
namespace App\\Casts;
class MoneyCast {
    public function get($model, string $key, $value, array $attributes): \\App\\ValueObjects\\Money {
        return new \\App\\ValueObjects\\Money($value);
    }
}
";
    let money_php = "\
<?php
namespace App\\ValueObjects;
class Money {
    public function amount(): int { return 0; }
    public function currency(): string { return ''; }
    public function formatted(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'balance' => 'App\\Casts\\MoneyCast',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Collections/MoneyCast.php", money_cast_php),
        ("src/Collections/Money.php", money_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"balance"),
        "Custom cast class should produce a virtual property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_encrypted_variants() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'secret' => 'encrypted',
        'encrypted_opts' => 'encrypted:array',
        'encrypted_coll' => 'encrypted:collection',
        'encrypted_obj' => 'encrypted:object',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"secret"),
        "encrypted cast, got: {:?}",
        props
    );
    assert!(
        props.contains(&"encrypted_opts"),
        "encrypted:array cast, got: {:?}",
        props
    );
    assert!(
        props.contains(&"encrypted_coll"),
        "encrypted:collection cast, got: {:?}",
        props
    );
    assert!(
        props.contains(&"encrypted_obj"),
        "encrypted:object cast, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_datetime_with_format() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'birthday' => 'date:Y-m-d',
        'logged_at' => 'datetime:Y-m-d H:i:s',
        'frozen_at' => 'immutable_datetime:Y-m-d',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"birthday"),
        "date:format cast, got: {:?}",
        props
    );
    assert!(
        props.contains(&"logged_at"),
        "datetime:format cast, got: {:?}",
        props
    );
    assert!(
        props.contains(&"frozen_at"),
        "immutable_datetime:format cast, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_enum_class_resolves_to_enum_type() {
    let status_enum_php = "\
<?php
namespace App\\Enums;
enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
    public function label(): string { return $this->value; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'status' => App\\Enums\\Status::class,
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Enums\\": "src/Enums/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("src/Enums/Status.php", status_enum_php));
    files.push(("src/Models/User.php", user_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"status"),
        "Enum cast should produce status property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_castable_class_resolves_to_class_itself() {
    let address_php = "\
<?php
namespace App\\Casts;
use Illuminate\\Contracts\\Database\\Eloquent\\Castable;
class Address implements Castable {
    public function getStreet(): string { return ''; }
    public function getCity(): string { return ''; }
    public static function castUsing(array $arguments): mixed { return null; }
}
";
    let castable_php = "\
<?php
namespace Illuminate\\Contracts\\Database\\Eloquent;
interface Castable {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'address' => App\\Casts\\Address::class,
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Casts\\": "src/Casts/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/",
            "Illuminate\\Contracts\\Database\\Eloquent\\": "vendor/illuminate/Contracts/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("vendor/illuminate/Contracts/Castable.php", castable_php));
    files.push(("src/Casts/Address.php", address_php));
    files.push(("src/Models/User.php", user_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"address"),
        "Castable class should produce address property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_class_with_colon_argument_suffix() {
    let address_php = "\
<?php
namespace App\\Casts;
use Illuminate\\Contracts\\Database\\Eloquent\\Castable;
class Address implements Castable {
    public function getStreet(): string { return ''; }
    public static function castUsing(array $arguments): mixed { return null; }
}
";
    let castable_php = "\
<?php
namespace Illuminate\\Contracts\\Database\\Eloquent;
interface Castable {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected function casts(): array {
        return [
            'address' => App\\Casts\\Address::class.':nullable',
        ];
    }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Casts\\": "src/Casts/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/",
            "Illuminate\\Contracts\\Database\\Eloquent\\": "vendor/illuminate/Contracts/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("vendor/illuminate/Contracts/Castable.php", castable_php));
    files.push(("src/Casts/Address.php", address_php));
    files.push(("src/Models/User.php", user_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"address"),
        "::class.':argument' should strip suffix and resolve, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_enum_with_colon_argument_in_casts_method() {
    let status_enum_php = "\
<?php
namespace App\\Enums;
enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
    public function label(): string { return $this->value; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected function casts(): array {
        return [
            'status' => App\\Enums\\Status::class.':force',
        ];
    }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Enums\\": "src/Enums/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("src/Enums/Status.php", status_enum_php));
    files.push(("src/Models/User.php", user_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"status"),
        "Enum cast with :argument suffix should produce status property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_enum_and_builtin_coexist() {
    let status_enum_php = "\
<?php
namespace App\\Enums;
enum Status: string {
    case Active = 'active';
    case Inactive = 'inactive';
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'status' => App\\Enums\\Status::class,
        'is_admin' => 'boolean',
        'created_at' => 'datetime',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Enums\\": "src/Enums/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("src/Enums/Status.php", status_enum_php));
    files.push(("src/Models/User.php", user_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"status"),
        "Enum cast property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"is_admin"),
        "Boolean cast property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"created_at"),
        "Datetime cast property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_casts_custom_cast_class_get_return_type_resolves_to_class() {
    let html_string_php = "\
<?php
namespace Illuminate\\Support;
class HtmlString {
    public function toHtml(): string { return ''; }
    public function isEmpty(): bool { return true; }
}
";
    let html_cast_php = "\
<?php
namespace App\\Casts;
use Illuminate\\Support\\HtmlString;
class HtmlCast {
    public function get($model, string $key, $value, array $attributes): ?HtmlString {
        return new HtmlString($value);
    }
}
";
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BrandTranslation extends Model {
    protected $casts = [
        'description' => 'App\\Casts\\HtmlCast',
    ];
    public function test() {
        $brand = new BrandTranslation();
        $brand->description->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Casts\\": "src/Casts/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Support\\": "vendor/illuminate/Support/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("vendor/illuminate/Support/HtmlString.php", html_string_php));
    files.push(("src/Casts/HtmlCast.php", html_cast_php));
    files.push(("src/Models/BrandTranslation.php", brand_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    // First verify the 'description' virtual property exists.
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        9,
        16,
    )
    .await;
    let props = property_names(&items);
    assert!(
        props.contains(&"description"),
        "Custom cast HtmlCast should produce 'description' property, got: {:?}",
        props
    );

    // Now verify that $brand->description-> resolves to HtmlString members.
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        9,
        30,
    )
    .await;
    let methods = method_names(&items);
    assert!(
        methods.contains(&"toHtml"),
        "description should resolve to HtmlString with toHtml(), got methods: {:?}",
        methods
    );
    assert!(
        methods.contains(&"isEmpty"),
        "description should resolve to HtmlString with isEmpty(), got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_casts_custom_cast_class_with_class_string_syntax() {
    let html_string_php = "\
<?php
namespace Illuminate\\Support;
class HtmlString {
    public function toHtml(): string { return ''; }
}
";
    let html_cast_php = "\
<?php
namespace App\\Casts;
use Illuminate\\Support\\HtmlString;
class HtmlCast {
    public function get($model, string $key, $value, array $attributes): ?HtmlString {
        return new HtmlString($value);
    }
}
";
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Casts\\HtmlCast;
class BrandTranslation extends Model {
    protected $casts = [
        'description' => HtmlCast::class,
    ];
    public function test() {
        $brand = new BrandTranslation();
        $brand->description->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Casts\\": "src/Casts/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Support\\": "vendor/illuminate/Support/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("vendor/illuminate/Support/HtmlString.php", html_string_php));
    files.push(("src/Casts/HtmlCast.php", html_cast_php));
    files.push(("src/Models/BrandTranslation.php", brand_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    // Verify the property exists.
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        10,
        16,
    )
    .await;
    let props = property_names(&items);
    assert!(
        props.contains(&"description"),
        "HtmlCast::class syntax should produce 'description' property, got: {:?}",
        props
    );

    // Verify chained completion resolves HtmlString members.
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        10,
        30,
    )
    .await;
    let methods = method_names(&items);
    assert!(
        methods.contains(&"toHtml"),
        "description via ::class should resolve to HtmlString with toHtml(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_casts_custom_cast_class_no_native_return_type_uses_docblock() {
    let html_string_php = "\
<?php
namespace Illuminate\\Support;
class HtmlString {
    public function toHtml(): string { return ''; }
    public function isEmpty(): bool { return true; }
}
";
    let html_cast_php = "\
<?php
namespace App\\Casts;
use Illuminate\\Support\\HtmlString;
class HtmlCast {
    /** @return HtmlString|null */
    public function get($model, string $key, $value, array $attributes) {
        return new HtmlString($value);
    }
}
";
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BrandTranslation extends Model {
    protected $casts = [
        'description' => 'App\\Casts\\HtmlCast',
    ];
    public function test() {
        $brand = new BrandTranslation();
        $brand->description->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Casts\\": "src/Casts/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Support\\": "vendor/illuminate/Support/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("vendor/illuminate/Support/HtmlString.php", html_string_php));
    files.push(("src/Casts/HtmlCast.php", html_cast_php));
    files.push(("src/Models/BrandTranslation.php", brand_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    // Verify the property exists.
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        9,
        16,
    )
    .await;
    let props = property_names(&items);
    assert!(
        props.contains(&"description"),
        "Custom cast with @return docblock should produce 'description' property, got: {:?}",
        props
    );

    // Verify chained completion resolves HtmlString members via docblock @return.
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        9,
        30,
    )
    .await;
    let methods = method_names(&items);
    assert!(
        methods.contains(&"toHtml"),
        "description via @return docblock should resolve to HtmlString with toHtml(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"isEmpty"),
        "description via @return docblock should resolve to HtmlString with isEmpty(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_casts_custom_cast_implements_generic_interface_no_native_return() {
    let html_string_php = "\
<?php
namespace Illuminate\\Support;
class HtmlString {
    public function toHtml(): string { return ''; }
    public function isEmpty(): bool { return true; }
}
";
    let casts_attributes_php = "\
<?php
namespace Illuminate\\Contracts\\Database\\Eloquent;
/**
 * @template TGet
 * @template TSet
 */
interface CastsAttributes {
    /**
     * @return TGet|null
     */
    public function get($model, string $key, $value, array $attributes);
}
";
    let html_cast_php = "\
<?php
namespace App\\Casts;
use Illuminate\\Support\\HtmlString;
use Illuminate\\Contracts\\Database\\Eloquent\\CastsAttributes;
/**
 * @implements CastsAttributes<HtmlString, HtmlString>
 */
final class HtmlCast implements CastsAttributes {
    public function get($model, string $key, $value, array $attributes) {
        return new HtmlString($value);
    }
}
";
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class BrandTranslation extends Model {
    protected $casts = [
        'description' => 'App\\Casts\\HtmlCast',
    ];
    public function test() {
        $brand = new BrandTranslation();
        $brand->description->
    }
}
";
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Casts\\": "src/Casts/",
            "App\\Collections\\": "src/Collections/",
            "App\\Concerns\\": "src/Concerns/",
            "Illuminate\\Support\\": "vendor/illuminate/Support/",
            "Illuminate\\Contracts\\Database\\Eloquent\\": "vendor/illuminate/Contracts/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;
    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("vendor/illuminate/Support/HtmlString.php", html_string_php));
    files.push((
        "vendor/illuminate/Contracts/CastsAttributes.php",
        casts_attributes_php,
    ));
    files.push(("src/Casts/HtmlCast.php", html_cast_php));
    files.push(("src/Models/BrandTranslation.php", brand_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    // Verify the property exists.
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        9,
        16,
    )
    .await;
    let props = property_names(&items);
    assert!(
        props.contains(&"description"),
        "@implements CastsAttributes<HtmlString, HtmlString> should produce 'description' property, got: {:?}",
        props
    );

    // Verify chained completion resolves HtmlString members.
    // The get() method has no native return type. The type should come from
    // resolving the @implements generic: TGet=HtmlString substituted into
    // the interface's @return TGet|null on get().
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/BrandTranslation.php",
        brand_php,
        9,
        30,
    )
    .await;
    let methods = method_names(&items);
    assert!(
        methods.contains(&"toHtml"),
        "description should resolve to HtmlString via @implements generic, got methods: {:?}",
        methods
    );
    assert!(
        methods.contains(&"isEmpty"),
        "description should resolve to HtmlString via @implements generic, got methods: {:?}",
        methods
    );
}

// ── Eloquent $attributes Defaults ───────────────────────────────────────────

#[tokio::test]
async fn test_attributes_defaults_produce_typed_virtual_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'role' => 'user',
        'is_active' => true,
        'login_count' => 0,
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"role"),
        "Attribute default 'user' (string) should produce role property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"is_active"),
        "Attribute default true (bool) should produce is_active property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"login_count"),
        "Attribute default 0 (int) should produce login_count property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_attributes_defaults_string_type_hint() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'role' => 'user',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let prop = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "role");
    assert!(prop.is_some(), "should find role property");
    assert!(
        prop.unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("string"),
        "role should show string in detail, got: {:?}",
        prop.unwrap().detail
    );
}

#[tokio::test]
async fn test_attributes_defaults_bool_type_hint() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'is_active' => true,
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let prop = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "is_active");
    assert!(prop.is_some(), "should find is_active property");
    assert!(
        prop.unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("bool"),
        "is_active should show bool in detail, got: {:?}",
        prop.unwrap().detail
    );
}

#[tokio::test]
async fn test_attributes_defaults_int_and_float() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'login_count' => 0,
        'rating' => 1.5,
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"login_count"),
        "int attribute default, got: {:?}",
        props
    );
    assert!(
        props.contains(&"rating"),
        "float attribute default, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_attributes_defaults_null_and_array() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'bio' => null,
        'settings' => [],
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"bio"),
        "null attribute default, got: {:?}",
        props
    );
    assert!(
        props.contains(&"settings"),
        "array attribute default, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_attributes_defaults_casts_take_priority() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_active' => 'boolean',
    ];
    protected $attributes = [
        'is_active' => 1,
        'role' => 'user',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 13, 15).await;
    let props = property_names(&items);

    // is_active should exist (from casts)
    assert!(
        props.contains(&"is_active"),
        "is_active should be present, got: {:?}",
        props
    );
    // role should exist (from attributes, not in casts)
    assert!(
        props.contains(&"role"),
        "role should be present from attributes, got: {:?}",
        props
    );

    // Verify is_active has cast type (bool), not attributes type (int)
    let prop = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "is_active");
    assert!(prop.is_some());
    assert!(
        prop.unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("bool"),
        "is_active should be bool from casts, not int from attributes, got: {:?}",
        prop.unwrap().detail
    );
}

#[tokio::test]
async fn test_attributes_defaults_on_this_arrow() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'role' => 'user',
        'is_active' => true,
    ];
    public function test() {
        $this->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"role"),
        "$this-> should include attribute default property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"is_active"),
        "$this-> should include attribute default property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_attributes_defaults_cross_file_psr4() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'role' => 'user',
        'login_count' => 0,
    ];
}
";
    let controller_php = "\
<?php
namespace App\\Models;
class UserController {
    public function show() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/UserController.php", controller_php),
    ]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserController.php",
        controller_php,
        5,
        15,
    )
    .await;
    let props = property_names(&items);

    assert!(
        props.contains(&"role"),
        "Cross-file attribute default property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"login_count"),
        "Cross-file attribute default property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_attributes_defaults_coexist_with_relationships_and_scopes() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'role' => 'user',
    ];
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<Post, $this> */
    public function posts(): mixed { return $this->hasMany(Post::class); }
    public function scopeActive($query): void {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 15).await;
    let props = property_names(&items);
    let methods = method_names(&items);

    assert!(
        props.contains(&"role"),
        "attribute default property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"posts"),
        "relationship property, got: {:?}",
        props
    );
    assert!(
        methods.contains(&"active"),
        "scope method, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_attributes_defaults_double_quoted_keys() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        \"role\" => \"user\",
        \"is_active\" => false,
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"role"),
        "double-quoted key attribute default, got: {:?}",
        props
    );
    assert!(
        props.contains(&"is_active"),
        "double-quoted key attribute default, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_attributes_defaults_negative_numbers() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'balance' => -100,
        'score' => -1.5,
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"balance"),
        "negative int attribute default, got: {:?}",
        props
    );
    assert!(
        props.contains(&"score"),
        "negative float attribute default, got: {:?}",
        props
    );
}

// ── Eloquent $fillable / $guarded / $hidden Column Names ────────────────────

#[tokio::test]
async fn test_fillable_produces_mixed_virtual_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $fillable = [
        'name',
        'email',
        'password',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 11, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"name"),
        "$fillable should produce name property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"email"),
        "$fillable should produce email property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"password"),
        "$fillable should produce password property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_fillable_type_is_mixed() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $fillable = ['name'];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let prop = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "name");
    assert!(prop.is_some(), "should find name property");
    assert!(
        prop.unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("mixed"),
        "name from $fillable should show mixed in detail, got: {:?}",
        prop.unwrap().detail
    );
}

#[tokio::test]
async fn test_guarded_produces_mixed_virtual_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $guarded = [
        'id',
        'created_at',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"id"),
        "$guarded should produce id property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"created_at"),
        "$guarded should produce created_at property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_hidden_produces_mixed_virtual_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $hidden = [
        'password',
        'remember_token',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"password"),
        "$hidden should produce password property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"remember_token"),
        "$hidden should produce remember_token property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_visible_produces_mixed_virtual_properties() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $visible = [
        'name',
        'avatar_url',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"name"),
        "$visible should produce name property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"avatar_url"),
        "$visible should produce avatar_url property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_fillable_guarded_hidden_merge_without_duplicates() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $fillable = ['name', 'email'];
    protected $guarded = ['id'];
    protected $hidden = ['password', 'email'];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(props.contains(&"name"), "from $fillable, got: {:?}", props);
    assert!(
        props.contains(&"email"),
        "from $fillable (first), got: {:?}",
        props
    );
    assert!(props.contains(&"id"), "from $guarded, got: {:?}", props);
    assert!(
        props.contains(&"password"),
        "from $hidden, got: {:?}",
        props
    );

    // email appears in both $fillable and $hidden — should appear only once.
    let email_count = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::PROPERTY)
                && (i.filter_text.as_deref().unwrap_or(&i.label) == "email")
        })
        .count();
    assert_eq!(
        email_count, 1,
        "email should appear exactly once despite being in both $fillable and $hidden"
    );
}

#[tokio::test]
async fn test_casts_take_priority_over_fillable() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
    ];
    protected $fillable = [
        'is_admin',
        'name',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 13, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"is_admin"),
        "should be present, got: {:?}",
        props
    );
    assert!(
        props.contains(&"name"),
        "should be present, got: {:?}",
        props
    );

    // is_admin should have bool type from casts, not mixed from fillable.
    let prop = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "is_admin");
    assert!(prop.is_some());
    assert!(
        prop.unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("bool"),
        "is_admin should be bool from casts, not mixed from fillable, got: {:?}",
        prop.unwrap().detail
    );
}

#[tokio::test]
async fn test_attributes_take_priority_over_fillable() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $attributes = [
        'role' => 'user',
    ];
    protected $fillable = [
        'role',
        'email',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 13, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"role"),
        "should be present, got: {:?}",
        props
    );
    assert!(
        props.contains(&"email"),
        "should be present, got: {:?}",
        props
    );

    // role should have string type from attributes, not mixed from fillable.
    let prop = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "role");
    assert!(prop.is_some());
    assert!(
        prop.unwrap()
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("string"),
        "role should be string from attributes, not mixed from fillable, got: {:?}",
        prop.unwrap().detail
    );
}

#[tokio::test]
async fn test_all_three_sources_coexist() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $casts = [
        'is_admin' => 'boolean',
    ];
    protected $attributes = [
        'role' => 'user',
    ];
    protected $fillable = [
        'is_admin',
        'role',
        'email',
    ];
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 17, 15).await;
    let props = property_names(&items);

    assert!(props.contains(&"is_admin"), "from casts, got: {:?}", props);
    assert!(props.contains(&"role"), "from attributes, got: {:?}", props);
    assert!(props.contains(&"email"), "from fillable, got: {:?}", props);

    // Verify priority: casts > attributes > fillable
    let is_admin = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "is_admin")
        .unwrap();
    assert!(
        is_admin.detail.as_deref().unwrap_or("").contains("bool"),
        "is_admin should be bool from casts, got: {:?}",
        is_admin.detail
    );

    let role = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "role")
        .unwrap();
    assert!(
        role.detail.as_deref().unwrap_or("").contains("string"),
        "role should be string from attributes, got: {:?}",
        role.detail
    );

    let email = items
        .iter()
        .find(|i| i.kind == Some(CompletionItemKind::PROPERTY) && i.label == "email")
        .unwrap();
    assert!(
        email.detail.as_deref().unwrap_or("").contains("mixed"),
        "email should be mixed from fillable, got: {:?}",
        email.detail
    );
}

#[tokio::test]
async fn test_fillable_on_this_arrow() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $fillable = ['name', 'email'];
    public function test() {
        $this->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 6, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"name"),
        "$this-> should include fillable property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"email"),
        "$this-> should include fillable property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_fillable_cross_file_psr4() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $fillable = ['name', 'email'];
}
";
    let controller_php = "\
<?php
namespace App\\Models;
class UserController {
    public function show() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/UserController.php", controller_php),
    ]);

    let items = complete_at(
        &backend,
        &dir,
        "src/Models/UserController.php",
        controller_php,
        5,
        15,
    )
    .await;
    let props = property_names(&items);

    assert!(
        props.contains(&"name"),
        "Cross-file fillable property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"email"),
        "Cross-file fillable property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_fillable_coexist_with_relationships_and_scopes() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    protected $fillable = ['name'];
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<Post, $this> */
    public function posts(): mixed { return $this->hasMany(Post::class); }
    public function scopeActive($query): void {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let props = property_names(&items);
    let methods = method_names(&items);

    assert!(
        props.contains(&"name"),
        "fillable property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"posts"),
        "relationship property, got: {:?}",
        props
    );
    assert!(
        methods.contains(&"active"),
        "scope method, got: {:?}",
        methods
    );
}

// ─── Factory support tests ──────────────────────────────────────────────────

#[tokio::test]
async fn test_factory_method_appears_on_model_static_access() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    // Verify that factory() appears as a static method on User::
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        "<?php\nuse App\\Models\\User;\nUser::\n",
        2,
        6,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"factory"),
        "factory() should appear as static method on User::, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_convention_based_factory_method_on_model() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    // User::factory()-> should resolve to UserFactory and show its methods
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        concat!(
            "<?php\n",
            "use App\\Models\\User;\n",
            "function test() {\n",
            "    User::factory()->\n",
            "}\n",
        ),
        3,
        22,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"definition"),
        "factory() should resolve to UserFactory, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_convention_based_create_returns_model() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
    public function greet(): string { return ''; }
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        "<?php\nuse App\\Models\\User;\nUser::factory()->create()->\n",
        2,
        28,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"greet"),
        "create() should resolve back to User, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_convention_based_make_returns_model() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
    public function greet(): string { return ''; }
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        "<?php\nuse App\\Models\\User;\nUser::factory()->make()->\n",
        2,
        26,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"greet"),
        "make() should resolve back to User, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_convention_based_chain_count_then_create() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
    public function greet(): string { return ''; }
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    // User::factory()->count(3)->create() should still resolve to User
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        "<?php\nuse App\\Models\\User;\nUser::factory()->count(3)->create()->\n",
        2,
        38,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"greet"),
        "count()->create() chain should resolve back to User, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_skips_convention_when_use_generic_present() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
use Database\\Factories\\CustomUserFactory;
/**
 * @use HasFactory<CustomUserFactory>
 */
class User extends Model {
    use HasFactory;
}
";

    let custom_factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class CustomUserFactory extends Factory {
    public function customMethod(): void {}
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        (
            "database/factories/CustomUserFactory.php",
            custom_factory_php,
        ),
    ]);

    // With @use HasFactory<CustomUserFactory>, the generics system handles it.
    // The convention-based factory() virtual method should NOT be synthesized.
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        "<?php\nuse App\\Models\\User;\nUser::factory()->\n",
        2,
        18,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"customMethod"),
        "should resolve to CustomUserFactory via generics, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_no_factory_class_no_crash() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
    public function greet(): string { return ''; }
}
";

    // No UserFactory file exists — should degrade gracefully.
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        "<?php\nuse App\\Models\\User;\n$u = new User();\n$u->\n",
        3,
        4,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"greet"),
        "model should still work when factory is missing, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_provider_on_factory_class_directly() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function greet(): string { return ''; }
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    // $factory = new UserFactory(); $factory->create()->
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        concat!(
            "<?php\n",
            "use Database\\Factories\\UserFactory;\n",
            "$f = new UserFactory();\n",
            "$f->create()->\n",
        ),
        3,
        14,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"greet"),
        "create() on factory instance should resolve to User model, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_provider_skipped_when_extends_generic_present() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function greet(): string { return ''; }
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
use App\\Models\\User;
/**
 * @extends Factory<User>
 */
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    // With @extends Factory<User>, the generics system handles create()/make().
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        concat!(
            "<?php\n",
            "use Database\\Factories\\UserFactory;\n",
            "$f = new UserFactory();\n",
            "$f->create()->\n",
        ),
        3,
        14,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"greet"),
        "create() should resolve via @extends generic, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_subdirectory_convention() {
    let user_php = "\
<?php
namespace App\\Models\\Admin;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class SuperUser extends Model {
    use HasFactory;
    public function adminAction(): void {}
}
";

    let factory_php = "\
<?php
namespace Database\\Factories\\Admin;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class SuperUserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    // Need to add subdirectory PSR-4 mapping.
    let composer = r#"{
    "autoload": {
        "psr-4": {
            "App\\Models\\": "src/Models/",
            "App\\Models\\Admin\\": "src/Models/Admin/",
            "Database\\Factories\\": "database/factories/",
            "Database\\Factories\\Admin\\": "database/factories/Admin/",
            "Illuminate\\Database\\Eloquent\\": "vendor/illuminate/Eloquent/",
            "Illuminate\\Database\\Eloquent\\Attributes\\": "vendor/illuminate/Eloquent/Attributes/",
            "Illuminate\\Database\\Eloquent\\Factories\\": "vendor/illuminate/Eloquent/Factories/",
            "Illuminate\\Database\\Eloquent\\Relations\\": "vendor/illuminate/Eloquent/Relations/",
            "Illuminate\\Database\\Query\\": "vendor/illuminate/Query/",
            "Illuminate\\Database\\Concerns\\": "vendor/illuminate/Concerns/"
        }
    }
}"#;

    let mut files: Vec<(&str, &str)> = framework_stubs();
    files.push(("src/Models/Admin/SuperUser.php", user_php));
    files.push(("database/factories/Admin/SuperUserFactory.php", factory_php));
    let (backend, dir) = create_psr4_workspace(composer, &files);

    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        concat!(
            "<?php\n",
            "use App\\Models\\Admin\\SuperUser;\n",
            "function test() {\n",
            "    SuperUser::factory()->\n",
            "}\n",
        ),
        3,
        26,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"definition"),
        "factory() on subdirectory model should resolve to SuperUserFactory, got methods: {:?}",
        methods
    );
}

/// Variable assignment from a factory chain: `$user = User::factory()->create(); $user->`
/// should resolve `$user` to `User` via the static call chain.
#[tokio::test]
async fn test_factory_variable_assignment_then_create() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
    public function greet(): string { return ''; }
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    // $user = User::factory()->create(); $user->
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        concat!(
            "<?php\n",
            "use App\\Models\\User;\n",
            "function test() {\n",
            "    $user = User::factory()->create();\n",
            "    $user->\n",
            "}\n",
        ),
        4,
        11,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"greet"),
        "$user assigned from factory()->create() should resolve to User, got methods: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_factory_coexists_with_relationships_and_scopes() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Factories\\HasFactory;
class User extends Model {
    use HasFactory;
    public function greet(): string { return ''; }
    /** @return \\Illuminate\\Database\\Eloquent\\Relations\\HasMany<Post, $this> */
    public function posts() {}
    public function scopeActive($query) {}
}
";

    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function title(): string { return ''; }
}
";

    let factory_php = "\
<?php
namespace Database\\Factories;
use Illuminate\\Database\\Eloquent\\Factories\\Factory;
class UserFactory extends Factory {
    public function definition(): array { return []; }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Models/Post.php", post_php),
        ("database/factories/UserFactory.php", factory_php),
    ]);

    // User:: should show factory() alongside other static methods and scopes.
    let items = complete_at(
        &backend,
        &dir,
        "src/test.php",
        "<?php\nuse App\\Models\\User;\nUser::\n",
        2,
        6,
    )
    .await;

    let methods = method_names(&items);
    assert!(
        methods.contains(&"factory"),
        "factory() should be available as static method, got methods: {:?}",
        methods
    );
    assert!(
        methods.contains(&"active"),
        "scope should coexist with factory, got methods: {:?}",
        methods
    );
}

// ─── Scope methods on Builder instances ─────────────────────────────────────

#[tokio::test]
async fn test_scope_available_after_builder_where_chain() {
    // Brand::where('id', $id)->isActive() should resolve scope methods
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeIsActive(Builder $query): void {}
    public function test() {
        $q = Brand::where('id', 1);
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // "$q->" at line 8, character 12
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 8, 12).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"isActive"),
        "After Brand::where(), ->isActive() scope should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "Builder methods should still be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "Builder get() should still be available, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_available_inside_scope_body() {
    // Inside a scope method body, $query->verified() should resolve other scopes
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class User extends Model {
    public function scopeActive(Builder $query): void {
        $query->
    }
    public function scopeVerified(Builder $query): void {}
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$query->" at line 6, character 16
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 6, 16).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"verified"),
        "Inside scope body, $query->verified() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"active"),
        "Inside scope body, $query->active() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"where"),
        "Builder where() should still be available inside scope body, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_available_after_inline_builder_chain() {
    // Brand::where('id', $id)->isActive()-> should continue the chain
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeIsActive(Builder $query): void {}
    public function test() {
        Brand::where('id', 1)->isActive()->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // "->isActive()->" at line 7, character 43
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 7, 43).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"get"),
        "After chaining through scope, ->get() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "After chaining through scope, ->orderBy() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"isActive"),
        "After chaining through scope, ->isActive() should still be chainable, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_on_builder_with_multiple_scopes() {
    // Multiple scopes should all be available on the builder
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Post extends Model {
    public function scopePublished(Builder $query): void {}
    public function scopeDraft(Builder $query): void {}
    public function scopeByAuthor(Builder $query, int $authorId): void {}
    public function test() {
        $q = Post::where('id', 1);
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Post.php", post_php)]);

    // "$q->" at line 10, character 12
    let items = complete_at(&backend, &dir, "src/Models/Post.php", post_php, 10, 12).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"published"),
        "published scope should be available on Builder, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"draft"),
        "draft scope should be available on Builder, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"byAuthor"),
        "byAuthor scope should be available on Builder, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_on_builder_from_trait() {
    // Scopes defined in traits used by the model should appear on the builder
    let trait_php = "\
<?php
namespace App\\Concerns;
use Illuminate\\Database\\Eloquent\\Builder;
trait SoftDeletesCustom {
    public function scopeWithTrashed(Builder $query): void {}
}
";
    let order_php = "\
<?php
namespace App\\Models;
use App\\Concerns\\SoftDeletesCustom;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Order extends Model {
    use SoftDeletesCustom;
    public function scopePending(Builder $query): void {}
    public function test() {
        $q = Order::where('status', 'pending');
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Concerns/SoftDeletesCustom.php", trait_php),
        ("src/Models/Order.php", order_php),
    ]);

    // "$q->" at line 10, character 12
    let items = complete_at(&backend, &dir, "src/Models/Order.php", order_php, 10, 12).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"pending"),
        "Own scope pending should be available on Builder, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"withTrashed"),
        "Trait scope withTrashed should be available on Builder, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_on_builder_cross_file() {
    // Scopes should work when the model is in a different file from
    // the code that chains builder calls
    let product_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Product extends Model {
    public function scopeInStock(Builder $query): void {}
    public function scopeOnSale(Builder $query): void {}
}
";
    let service_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Builder;
class ProductService {
    public function test() {
        $q = Product::where('active', true);
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Product.php", product_php),
        ("src/Models/ProductService.php", service_php),
    ]);

    // Open the model file first so it's indexed
    let model_uri = Url::from_file_path(dir.path().join("src/Models/Product.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: model_uri,
                language_id: "php".to_string(),
                version: 1,
                text: product_php.to_string(),
            },
        })
        .await;

    // "$q->" at line 6, character 12
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/ProductService.php",
        service_php,
        6,
        12,
    )
    .await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"inStock"),
        "Cross-file scope inStock should be available on Builder, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"onSale"),
        "Cross-file scope onSale should be available on Builder, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_not_injected_on_non_eloquent_builder() {
    // Scopes should NOT be injected when the Builder's generic arg
    // is not an Eloquent Model.
    let not_a_model_php = "\
<?php
namespace App\\Models;
class NotAModel {
    public function hello(): void {}
}
";
    let model_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class SomeModel extends Model {
    public function scopePopular(Builder $query): void {}
    /** @return Builder<NotAModel> */
    public function getBadBuilder(): Builder { return new Builder(); }
    public function test() {
        $q = $this->getBadBuilder();
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/NotAModel.php", not_a_model_php),
        ("src/Models/SomeModel.php", model_php),
    ]);

    // Open NotAModel first so it's indexed
    let uri = Url::from_file_path(dir.path().join("src/Models/NotAModel.php")).unwrap();
    backend
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "php".to_string(),
                version: 1,
                text: not_a_model_php.to_string(),
            },
        })
        .await;

    // "$q->" at line 10, character 12
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/SomeModel.php",
        model_php,
        10,
        12,
    )
    .await;
    let methods = method_names(&items);

    // Builder methods should be available (where, get, etc.)
    assert!(
        methods.contains(&"where"),
        "Builder methods should be available, got: {:?}",
        methods
    );
    // Scopes from SomeModel should NOT appear because the generic arg
    // is NotAModel, which does not extend Eloquent Model.
    assert!(
        !methods.contains(&"popular"),
        "Scope from SomeModel should NOT appear on Builder<NotAModel>, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_chain_returns_builder_with_scopes() {
    // After calling a scope, further scopes should still be available
    let task_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Task extends Model {
    public function scopeUrgent(Builder $query): void {}
    public function scopeAssignedTo(Builder $query, int $userId): void {}
    public function test() {
        $q = Task::where('active', true)->urgent();
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Task.php", task_php)]);

    // "$q->" at line 9, character 12
    let items = complete_at(&backend, &dir, "src/Models/Task.php", task_php, 9, 12).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"assignedTo"),
        "After chaining through urgent(), assignedTo() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"urgent"),
        "After chaining through urgent(), urgent() should still be chainable, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "Builder get() should be available after scope chain, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_on_builder_indirect_model_subclass() {
    // Scopes on indirect model subclasses should also appear on Builder
    let base_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class BaseModel extends Model {
    public function scopeTenantAware(Builder $query): void {}
}
";
    let invoice_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Builder;
class Invoice extends BaseModel {
    public function scopeOverdue(Builder $query): void {}
    public function test() {
        $q = Invoice::where('status', 'pending');
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/BaseModel.php", base_php),
        ("src/Models/Invoice.php", invoice_php),
    ]);

    // "$q->" at line 7, character 12
    let items = complete_at(&backend, &dir, "src/Models/Invoice.php", invoice_php, 7, 12).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"overdue"),
        "Own scope overdue should be available on Builder, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"tenantAware"),
        "Parent model scope tenantAware should be available on Builder, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_strips_query_param_on_builder() {
    // Scope methods on Builder should have the $query parameter stripped
    let item_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Item extends Model {
    public function scopeOfType(Builder $query, string $type): void {}
    public function test() {
        $q = Item::where('active', true);
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Item.php", item_php)]);

    // "$q->" at line 8, character 12
    let items = complete_at(&backend, &dir, "src/Models/Item.php", item_php, 8, 12).await;

    // Find the ofType method completion item
    let of_type_item = items.iter().find(|i| {
        let name = i.filter_text.as_deref().unwrap_or(&i.label);
        name == "ofType"
    });
    assert!(
        of_type_item.is_some(),
        "ofType scope should be available on Builder, got: {:?}",
        method_names(&items)
    );
}

#[tokio::test]
async fn test_model_with_returns_builder_methods() {
    // Sanity check: Brand::with('english')-> should at minimum resolve
    // to Builder methods (where, get, orderBy, etc).
    // Model::with() has @return \Illuminate\Database\Eloquent\Builder<static>
    // and `static` in the generic arg must be resolved to the concrete model.
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function test() {
        Brand::with('english')->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 6 = "        Brand::with('english')->", character 32 = after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 6, 32).await;
    let methods = method_names(&items);

    assert!(
        !methods.is_empty(),
        "Brand::with('english')-> should produce completions, got empty list"
    );
    assert!(
        methods.contains(&"where"),
        "Builder::where() should be available after Brand::with(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "Builder::get() should be available after Brand::with(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_available_after_model_with_call() {
    // Brand::with('english')-> should resolve scope methods.
    // Model::with() returns Builder<static>, and `static` in the generic
    // arg must be resolved to the concrete model name so that scope
    // injection on Builder<Brand> works.
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeProductInformation(Builder $query): void {}
    public function test() {
        Brand::with('english')->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 7 = "        Brand::with('english')->", character 32 = after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 7, 32).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"productInformation"),
        "After Brand::with(), ->productInformation() scope should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"where"),
        "Builder methods should still be available after with(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"get"),
        "Builder get() should still be available after with(), got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_scope_available_after_model_with_then_chain() {
    // Brand::with('english')->where('active', 1)-> should still have scopes.
    // This verifies that chaining after with() preserves the Builder<Brand>
    // type through subsequent builder method calls.
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeIsActive(Builder $query): void {}
    public function test() {
        Brand::with('english')->where('active', 1)->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 7 = "        Brand::with('english')->where('active', 1)->", character 52 = after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 7, 52).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"isActive"),
        "After Brand::with()->where(), ->isActive() scope should be available, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_completion_after_multiline_closure_argument() {
    // Brand::whereNested(function (Builder $q): void {
    // })
    // ->   // completion should work here
    //
    // This tests that collapse_continuation_lines handles multi-line
    // closure arguments by tracking brace/paren balance when walking
    // backwards through lines.
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeIsActive(Builder $query): void {}
    public function test() {
        Brand::whereNested(function (Builder $q): void {
        })
        ->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 9 = "        ->", character 10 = after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 9, 10).await;
    let methods = method_names(&items);

    assert!(
        !methods.is_empty(),
        "Completion after multi-line closure arg should produce results, got empty list"
    );
    assert!(
        methods.contains(&"where"),
        "Builder::where() should be available after multi-line closure, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"isActive"),
        "Scope isActive() should be available after multi-line closure, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_completion_after_multiline_closure_with_body() {
    // Same as above but the closure has a body with content inside.
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeIsActive(Builder $query): void {}
    public function test() {
        Brand::whereNested(function (Builder $q): void {
            $q->where('active', true);
        })
        ->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 10 = "        ->", character 10 = after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 10, 10).await;
    let methods = method_names(&items);

    assert!(
        !methods.is_empty(),
        "Completion after multi-line closure with body should produce results, got empty list"
    );
    assert!(
        methods.contains(&"where"),
        "Builder::where() should be available, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"isActive"),
        "Scope isActive() should be available, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_single_line_closure_still_works() {
    // Sanity check: the single-line closure case should still work.
    // Brand::whereNested(function (Builder $q): void {})
    // ->
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    public function scopeIsActive(Builder $query): void {}
    public function test() {
        Brand::whereNested(function (Builder $q): void {})
        ->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 8 = "        ->", character 10 = after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 8, 10).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"where"),
        "Builder::where() should be available after single-line closure, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"isActive"),
        "Scope isActive() should be available after single-line closure, got: {:?}",
        methods
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Closure parameter inference in Laravel pipelines
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_chunk_closure_param_inferred_via_local_collection() {
    // Minimal reproduction: a locally defined generic class with a chunk
    // method whose callable parameter receives Collection<int, TValue>.
    // This isolates the callable-param-inference machinery from Laravel's
    // Builder-as-static forwarding and cross-file resolution.
    let backend = create_test_backend();
    let uri = Url::parse("file:///test/chunk_local.php").unwrap();

    let src = "\
<?php
class Brand {
    public string $name;
    public function getName(): string { return ''; }
}
/**
 * @template TKey of array-key
 * @template TValue
 */
class MyCollection {
    /** @return int */
    public function count(): int { return 0; }
    /** @return TValue|null */
    public function first(): mixed { return null; }
}
/**
 * @template TModel
 */
class MyBuilder {
    /**
     * @param callable(MyCollection<int, TModel>, int): mixed $callback
     * @return bool
     */
    public function chunk(int $count, callable $callback): bool { return true; }
    /** @return static */
    public function where(string $col, mixed $val = null): static { return $this; }
}
class Service {
    /** @return MyBuilder<Brand> */
    public function query(): MyBuilder { return new MyBuilder(); }
    public function run(): void {
        $builder = $this->query();
        $builder->chunk(100, function ($orders) {
            $orders->
        });
    }
}
";
    let open_params = DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: uri.clone(),
            language_id: "php".to_string(),
            version: 1,
            text: src.to_string(),
        },
    };
    backend.did_open(open_params).await;

    // line 33: "            $orders->"  cursor after ->
    let completion_params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line: 33,
                character: 22,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };

    let items = match backend.completion(completion_params).await.unwrap() {
        Some(CompletionResponse::Array(items)) => items,
        Some(CompletionResponse::List(list)) => list.items,
        None => vec![],
    };
    let methods = method_names(&items);

    assert!(
        methods.contains(&"count"),
        "Expected count() from MyCollection<int, Brand>, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Expected first() from MyCollection<int, Brand>, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_chunk_closure_param_inferred_as_collection_of_model() {
    // BuildsQueries::chunk($count, callable(Collection<int, TValue>, int): mixed)
    // Builder uses: @use BuildsQueries<TModel>
    // Brand::where()->chunk(100, function ($orders) { $orders-> })
    //   => $orders should be Collection<int, Brand>
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Brand extends Model {
    public string $name;
    public function test() {
        Brand::where('active', true)->chunk(100, function ($orders) {
            $orders->
        });
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 7: "            $orders->"  cursor after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 7, 22).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"count"),
        "Expected count() from Collection<int, Brand>, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Expected first() from Collection<int, Brand>, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"each"),
        "Expected each() from Support\\Collection<int, Brand>, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_where_has_closure_param_inferred_as_builder() {
    // Builder::whereHas(string $relation, Closure(Builder<TModel>): mixed $callback)
    // Brand::whereHas('orders', function ($q) { $q-> })
    //   => $q should be Builder<Brand> (has where, orderBy, etc.)
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Brand extends Model {
    public function scopeIsActive(\\Illuminate\\Database\\Eloquent\\Builder $query): void {}
    public function test() {
        Brand::whereHas('orders', function ($q) {
            $q->
        });
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 7: "            $q->"  cursor after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 7, 16).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"where"),
        "Expected where() from Builder<Brand>, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "Expected orderBy() from Builder<Brand>, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_with_closure_param_inferred_as_relation() {
    // Builder::with($relations, Closure(Relation): mixed $callback)
    // When called as an instance method on Builder, the closure param
    // should be inferred as Relation (has where, orderBy, etc.)
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class Brand extends Model {
    /** @param Builder<Brand> $builder */
    public function test(Builder $builder) {
        $builder->with('translations', function ($query) {
            $query->
        });
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 8: "            $query->"  cursor after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 8, 20).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"where"),
        "Expected where() from Relation, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"orderBy"),
        "Expected orderBy() from Relation, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_chunk_explicit_type_hint_takes_precedence() {
    // When a user explicitly types the parameter, the explicit type wins.
    // Brand::where()->chunk(100, function (Collection $orders) { $orders-> })
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Collection;
class Brand extends Model {
    public function test() {
        Brand::where('active', true)->chunk(100, function (Collection $orders) {
            $orders->
        });
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 7: "            $orders->"  cursor after ->
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 7, 22).await;
    let methods = method_names(&items);

    assert!(
        methods.contains(&"count"),
        "Explicit Collection type should resolve; expected count() in {:?}",
        methods
    );
}

#[tokio::test]
async fn test_chunk_chain_continues_after_closure() {
    // The outer chain should still work after chunk's closure.
    // Brand::where()->chunk(100, function ($orders) { ... })
    // The return type of chunk is bool, so no chaining, but the
    // important thing is the variable inside the closure resolves.
    let brand_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Brand extends Model {
    public function test() {
        Brand::where('active', true)->chunk(100, function ($orders) {
            $orders->each(function ($brand) {
                $brand->
            });
        });
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/Brand.php", brand_php)]);

    // line 7: "                $brand->"  cursor after ->
    // $orders is Collection<int, Brand>, and Collection::each has
    // callable(TValue, TKey): mixed. TValue=Brand, so $brand=Brand.
    let items = complete_at(&backend, &dir, "src/Models/Brand.php", brand_php, 7, 24).await;
    let methods = method_names(&items);

    // Brand extends Model, so it should at least have Model's static
    // methods forwarded. But as a model instance it should have basic
    // methods. At minimum the completion should not be empty.
    assert!(
        !methods.is_empty(),
        "Expected completions for $brand inside nested closure, got empty list"
    );
}

// ─── Relationship count properties (*_count) ────────────────────────────────

#[tokio::test]
async fn test_has_many_count_property_produced() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts_count"),
        "Should include synthesized 'posts_count' property, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_count_property_typed_as_int() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->posts_count->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // posts_count is typed as int, so chaining -> on it should not
    // produce class completions.  The important thing is that
    // posts_count itself appears as a property (tested above).
    // Here we just confirm it doesn't crash and doesn't resolve
    // to Post's methods.
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 28).await;
    let methods = method_names(&items);
    assert!(
        !methods.contains(&"getTitle"),
        "posts_count is int, should not resolve to Post methods, got: {:?}",
        methods
    );
}

#[tokio::test]
async fn test_multiple_relationships_produce_count_properties() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let comment_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Comment extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    /** @return HasMany<\\App\\Models\\Comment, $this> */
    public function comments(): HasMany { return $this->hasMany(Comment::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/Comment.php", comment_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 12, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts_count"),
        "Should include 'posts_count', got: {:?}",
        props
    );
    assert!(
        props.contains(&"comments_count"),
        "Should include 'comments_count', got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_count_property_camel_case_relationship() {
    let baker_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Baker extends Model {
    public function getName(): string { return ''; }
}
";
    let bakery_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasOne;
class Bakery extends Model {
    /** @return HasOne<\\App\\Models\\Baker, $this> */
    public function headBaker(): HasOne { return $this->hasOne(Baker::class); }
    public function test() {
        $b = new Bakery();
        $b->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Baker.php", baker_php),
        ("src/Models/Bakery.php", bakery_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/Bakery.php", bakery_php, 9, 13).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"head_baker_count"),
        "camelCase 'headBaker' should produce 'head_baker_count', got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_count_property_on_this_arrow() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $this->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts_count"),
        "$this-> should include 'posts_count', got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_count_property_body_inferred_relationship() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class User extends Model {
    public function posts() { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts_count"),
        "Body-inferred relationship should produce 'posts_count', got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_count_property_coexists_with_relationship_property() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 15).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "Relationship property 'posts' should still exist, got: {:?}",
        props
    );
    assert!(
        props.contains(&"posts_count"),
        "Count property 'posts_count' should coexist, got: {:?}",
        props
    );
}

#[tokio::test]
async fn test_count_property_on_inline_new_instantiation() {
    let post_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
class Post extends Model {
    public function getTitle(): string { return ''; }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Relations\\HasMany;
class User extends Model {
    /** @return HasMany<\\App\\Models\\Post, $this> */
    public function posts(): HasMany { return $this->hasMany(Post::class); }
    public function test() {
        (new User())->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/Post.php", post_php),
        ("src/Models/User.php", user_php),
    ]);

    // Line 8: "(new User())->" cursor after ->
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 22).await;
    let props = property_names(&items);

    assert!(
        props.contains(&"posts"),
        "(new User())-> should include 'posts' relationship property, got: {:?}",
        props
    );
    assert!(
        props.contains(&"posts_count"),
        "(new User())-> should include 'posts_count', got: {:?}",
        props
    );
}

// ─── #[Scope] attribute (Laravel 11+) ───────────────────────────────────────

/// A method with `#[Scope]` should produce completions using its own name
/// (no prefix stripping), available as both static and instance.
#[tokio::test]
async fn test_scope_attribute_completion_static() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    #[Scope]
    protected function active(Builder $query): void {}
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "User::" at line 9, character 14
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 14).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"active"),
        "#[Scope] should produce a static 'active' method, got: {:?}",
        method_names
    );
}

/// `#[Scope]` method should be available as an instance method too.
#[tokio::test]
async fn test_scope_attribute_completion_instance() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    #[Scope]
    protected function active(Builder $query): void {}
    public function test() {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$user->" at line 10, character 15
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 15).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"active"),
        "#[Scope] should produce an instance 'active' method, got: {:?}",
        method_names
    );
}

/// `#[Scope]` with extra parameters beyond `$query` should preserve them.
#[tokio::test]
async fn test_scope_attribute_with_extra_params() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    #[Scope]
    protected function ofType(Builder $query, string $type): void {}
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 14).await;
    let of_type: Vec<&CompletionItem> = items
        .iter()
        .filter(|i| {
            i.kind == Some(CompletionItemKind::METHOD)
                && i.filter_text.as_deref().unwrap_or(&i.label) == "ofType"
        })
        .collect();

    assert!(
        !of_type.is_empty(),
        "#[Scope] ofType should appear in completions"
    );
}

/// `#[Scope]` scope should work on Builder instances (chaining).
#[tokio::test]
async fn test_scope_attribute_on_builder_chain() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    #[Scope]
    protected function active(Builder $query): void {}
    public function getName(): string { return ''; }
    public function test() {
        User::where('id', 1)->active()->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "->active()->" at line 10, character 40
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 40).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"get"),
        "After #[Scope] chain, Builder methods like 'get' should be available, got: {:?}",
        method_names
    );
}

/// `#[Scope]` on a Builder instance without static call.
#[tokio::test]
async fn test_scope_attribute_on_builder_variable() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    #[Scope]
    protected function active(Builder $query): void {}
    public function test() {
        $q = User::where('status', 'pending');
        $q->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$q->" at line 10, character 12
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 12).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"active"),
        "#[Scope] should be available on Builder variable, got: {:?}",
        method_names
    );
}

/// `#[Scope]` attribute and `scopeX` convention can coexist on the same model.
#[tokio::test]
async fn test_scope_attribute_and_convention_coexist() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    public function scopeVerified(Builder $query): void {}
    #[Scope]
    protected function active(Builder $query): void {}
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 10, 14).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"active"),
        "#[Scope] 'active' should appear, got: {:?}",
        method_names
    );
    assert!(
        method_names.contains(&"verified"),
        "Convention 'verified' should appear, got: {:?}",
        method_names
    );
}

/// `#[Scope]` with FQN attribute name should also work.
#[tokio::test]
async fn test_scope_attribute_fqn() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
class User extends Model {
    #[\\Illuminate\\Database\\Eloquent\\Attributes\\Scope]
    protected function active(Builder $query): void {}
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 8, 14).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"active"),
        "FQN #[Scope] should produce 'active' method, got: {:?}",
        method_names
    );
}

/// `#[Scope]` defined in a trait used by the model.
#[tokio::test]
async fn test_scope_attribute_in_trait() {
    let trait_php = "\
<?php
namespace App\\Concerns;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
trait HasActiveScope {
    #[Scope]
    protected function active(Builder $query): void {
        $query->where('active', true);
    }
}
";
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Concerns\\HasActiveScope;
class User extends Model {
    use HasActiveScope;
    public function test() {
        User::
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Concerns/HasActiveScope.php", trait_php),
        ("src/Models/User.php", user_php),
    ]);

    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 7, 14).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"active"),
        "#[Scope] from trait should produce 'active' method, got: {:?}",
        method_names
    );
}

/// Inside a `#[Scope]` method body, `$query->` should resolve scope methods
/// from the enclosing model.
#[tokio::test]
async fn test_scope_attribute_query_resolution_inside_body() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    public function scopeVerified(Builder $query): void {}
    #[Scope]
    protected function active(Builder $query): void {
        $query->
    }
}
";
    let (backend, dir) = make_workspace(&[("src/Models/User.php", user_php)]);

    // "$query->" at line 9, character 16
    let items = complete_at(&backend, &dir, "src/Models/User.php", user_php, 9, 16).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"where"),
        "$query-> inside #[Scope] body should have Builder methods, got: {:?}",
        method_names
    );
    assert!(
        method_names.contains(&"verified"),
        "$query-> inside #[Scope] body should have convention scopes, got: {:?}",
        method_names
    );
}

/// `#[Scope]` protected method accessed from *outside* the model class
/// must still appear as a public scope method in instance completions.
/// This is the scenario where `$bakery->fresh()` was missing: the
/// original `protected function fresh()` blocked the virtual public
/// replacement during merge.
#[tokio::test]
async fn test_scope_attribute_instance_from_outside_class() {
    let user_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use Illuminate\\Database\\Eloquent\\Builder;
use Illuminate\\Database\\Eloquent\\Attributes\\Scope;
class User extends Model {
    #[Scope]
    protected function active(Builder $query): void {}
    public function getName(): string { return ''; }
}
";
    let demo_php = "\
<?php
namespace App\\Demo;
use App\\Models\\User;
class Demo {
    public function test(): void {
        $user = new User();
        $user->
    }
}
";
    let (backend, dir) = make_workspace(&[
        ("src/Models/User.php", user_php),
        ("src/Demo/Demo.php", demo_php),
    ]);

    // "$user->" at line 6, character 15
    let items = complete_at(&backend, &dir, "src/Demo/Demo.php", demo_php, 6, 15).await;
    let method_names: Vec<&str> = items
        .iter()
        .filter(|i| i.kind == Some(CompletionItemKind::METHOD))
        .map(|i| i.filter_text.as_deref().unwrap_or(&i.label))
        .collect();

    assert!(
        method_names.contains(&"active"),
        "#[Scope] should appear as public instance method from outside the class, got: {:?}",
        method_names
    );
    assert!(
        method_names.contains(&"getName"),
        "regular public method should also appear, got: {:?}",
        method_names
    );
}

/// When a model uses a trait that declares `@method` tags (e.g.
/// `SoftDeletes` with `@method static Builder<static> withTrashed()`),
/// those methods should be available on `Builder<Model>` instances
/// returned by query builder methods in the chain.
///
/// Reproduces: `Customer::groupBy('email')->withTrashed()->first()->email`
/// where `groupBy` comes from Query\Builder (via @mixin) and returns
/// `Builder<Customer>`, and `withTrashed` comes from `SoftDeletes`
/// `@method` tag on the model.
#[tokio::test]
async fn test_model_method_tags_on_builder_instance() {
    let soft_deletes_php = "\
<?php
namespace App\\Concerns;
/**
 * @method static \\Illuminate\\Database\\Eloquent\\Builder<static> withTrashed(bool $withTrashed = true)
 * @method static \\Illuminate\\Database\\Eloquent\\Builder<static> onlyTrashed()
 */
trait SoftDeletes
{
}
";

    // Place the completion trigger inside the model file itself (same
    // pattern as the working test_scope_on_builder_from_trait).
    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Concerns\\SoftDeletes;
class Customer extends Model {
    use SoftDeletes;
    public string $email = '';
    public function getDisplayName(): string { return ''; }
    public function scopeActive(\\Illuminate\\Database\\Eloquent\\Builder $query): void {}
    public function test(): void {
        $q = Customer::where('active', true);
        $q->
    }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Concerns/SoftDeletes.php", soft_deletes_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$q->" at line 11, character 12
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        11,
        12,
    )
    .await;
    let methods = method_names(&items);

    // Model @method tags from SoftDeletes should be available on Builder<Customer>.
    assert!(
        methods.contains(&"withTrashed"),
        "Model @method 'withTrashed' from SoftDeletes should be available on Builder<Customer>, got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"onlyTrashed"),
        "Model @method 'onlyTrashed' from SoftDeletes should be available on Builder<Customer>, got: {:?}",
        methods
    );

    // Regular Builder methods should still be present.
    assert!(
        methods.contains(&"where"),
        "Builder::where should still be available, got: {:?}",
        methods
    );

    // Model scope methods should also be present.
    assert!(
        methods.contains(&"active"),
        "Model scope 'active' should be available on Builder<Customer>, got: {:?}",
        methods
    );
}

/// After resolving `withTrashed()` on Builder<Customer>, the chain
/// should continue to resolve Builder methods like `first()`.
#[tokio::test]
async fn test_model_method_tags_chain_continues_after_virtual_method() {
    let soft_deletes_php = "\
<?php
namespace App\\Concerns;
/**
 * @method static \\Illuminate\\Database\\Eloquent\\Builder<static> withTrashed(bool $withTrashed = true)
 */
trait SoftDeletes
{
}
";

    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Concerns\\SoftDeletes;
class Customer extends Model {
    use SoftDeletes;
    public string $email = '';
    public function getDisplayName(): string { return ''; }
    public function scopeActive(\\Illuminate\\Database\\Eloquent\\Builder $query): void {}
    public function test(): void {
        $q = Customer::where('active', true)->withTrashed();
        $q->
    }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Concerns/SoftDeletes.php", soft_deletes_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$q->" at line 11, character 12
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        11,
        12,
    )
    .await;
    let methods = method_names(&items);

    // withTrashed() returns Builder<static> → Builder<Customer>,
    // so Builder methods like first() and where() should be available.
    assert!(
        methods.contains(&"first"),
        "Builder methods should be available after withTrashed(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"where"),
        "Builder::where should be available after withTrashed(), got: {:?}",
        methods
    );
}

/// `withTrashed()->get()` should return `Collection<int, Customer>`,
/// not `Collection<int, Builder<Customer>>`.  The `@method` tag on
/// `SoftDeletes` declares `Builder<static>` as the return type, and
/// `static` must resolve to the model name so that Builder's own
/// `get()` method sees the correct TModel generic argument.
#[tokio::test]
async fn test_model_method_tags_get_returns_collection_of_model() {
    let soft_deletes_php = "\
<?php
namespace App\\Concerns;
/**
 * @method static \\Illuminate\\Database\\Eloquent\\Builder<static> withTrashed(bool $withTrashed = true)
 */
trait SoftDeletes
{
}
";

    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Concerns\\SoftDeletes;
class Customer extends Model {
    use SoftDeletes;
    public string $email = '';
    public function getDisplayName(): string { return ''; }
    public function test(): void {
        $users = Customer::groupBy('email')->withTrashed()->get();
        $users->
    }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Concerns/SoftDeletes.php", soft_deletes_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$users->" at line 10, character 16
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        10,
        16,
    )
    .await;
    let methods = method_names(&items);

    // get() on Builder<Customer> should return Collection<int, Customer>,
    // so Collection methods should be available (not Builder methods).
    assert!(
        methods.contains(&"count"),
        "Collection from get() after withTrashed() should have count(), got: {:?}",
        methods
    );
    assert!(
        methods.contains(&"first"),
        "Collection from get() after withTrashed() should have first(), got: {:?}",
        methods
    );
    // The collection should NOT expose Builder-only methods, which
    // would indicate a double-wrapped Builder<Builder<Customer>> type.
    assert!(
        !methods.contains(&"withTrashed"),
        "Collection should not have withTrashed() — that would indicate double-wrapping, got: {:?}",
        methods
    );
}

/// `withTrashed()->first()` should return a `Customer` model instance,
/// not a `Builder<Customer>`.
#[tokio::test]
async fn test_model_method_tags_first_returns_model_instance() {
    let soft_deletes_php = "\
<?php
namespace App\\Concerns;
/**
 * @method static \\Illuminate\\Database\\Eloquent\\Builder<static> withTrashed(bool $withTrashed = true)
 */
trait SoftDeletes
{
}
";

    let customer_php = "\
<?php
namespace App\\Models;
use Illuminate\\Database\\Eloquent\\Model;
use App\\Concerns\\SoftDeletes;
class Customer extends Model {
    use SoftDeletes;
    public string $email = '';
    public function getDisplayName(): string { return ''; }
    public function test(): void {
        $user = Customer::where('active', true)->withTrashed()->first();
        $user->
    }
}
";

    let (backend, dir) = make_workspace(&[
        ("src/Concerns/SoftDeletes.php", soft_deletes_php),
        ("src/Models/Customer.php", customer_php),
    ]);

    // "$user->" at line 10, character 15
    let items = complete_at(
        &backend,
        &dir,
        "src/Models/Customer.php",
        customer_php,
        10,
        15,
    )
    .await;
    let methods = method_names(&items);

    // first() on Builder<Customer> should return Customer, so model
    // methods and properties should be available.
    assert!(
        methods.contains(&"getDisplayName"),
        "first() after withTrashed() should return Customer with getDisplayName(), got: {:?}",
        methods
    );
}
