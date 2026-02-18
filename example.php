<?php

/**
 * PHPantomLSP — Feature Showcase
 *
 * A single-file demo of every completion and go-to-definition feature.
 * Open this in your editor to try each one interactively.
 */

namespace Demo;

use Exception;
use Stringable;
use Demo\UserProfile as Profile;

// ─── Interfaces ─────────────────────────────────────────────────────────────

/**
 * @method string render()
 * @property-read string $output
 */
interface Renderable extends Stringable
{
    public function format(string $template): string;
}

interface Loggable
{
    public function log(string $message): void;
}

// ─── Traits ─────────────────────────────────────────────────────────────────

trait HasTimestamps
{
    protected ?string $createdAt = null;

    public function getCreatedAt(): ?string
    {
        return $this->createdAt;
    }

    public function setCreatedAt(string $date): static
    {
        $this->createdAt = $date;
        return $this;
    }
}

trait HasSlug
{
    public function generateSlug(string $value): string
    {
        return strtolower(str_replace(' ', '-', $value));
    }
}

// ─── Enums ──────────────────────────────────────────────────────────────────

enum Status: string
{
    case Active = 'active';
    case Inactive = 'inactive';
    case Pending = 'pending';

    public function label(): string
    {
        return match ($this) {
            self::Active => 'Active',
            self::Inactive => 'Inactive',
            self::Pending => 'Pending',
        };
    }

    public function isActive(): bool
    {
        return $this === self::Active;
    }
}

enum Priority: int
{
    case Low = 1;
    case Medium = 2;
    case High = 3;
}

enum Mode
{
    case Automatic;
    case Manual;
}

// ─── Builder (@mixin target) ────────────────────────────────────────────────

class Builder
{
    /** @return static */
    public static function query(): self
    {
        return new static();
    }

    public function where(string $col, mixed $val): self
    {
        return $this;
    }
}

// ─── Abstract Base Class ────────────────────────────────────────────────────

/**
 * @property string $magicName
 * @method static static create(array $attributes)
 * @mixin Builder
 */
abstract class Model
{
    protected int $id;

    public const string CONNECTION = 'default';
    protected const int PER_PAGE = 15;

    public function __construct(
        protected string $name = '',
        public readonly string $uuid = '',
    ) {
        $this->id = rand(1, 99999);
    }

    public function getId(): int
    {
        return $this->id;
    }

    public function getName(): string
    {
        return $this->name;
    }

    /** @return static */
    public function setName(string $name): static
    {
        $this->name = $name;
        return $this;
    }

    /** @deprecated */
    public static function find(int $id): ?static
    {
        return null;
    }

    /** @return static */
    public static function make(string $name = ''): static
    {
        return new static($name, '');
    }

    abstract public function toArray(): array;

    public function __toString(): string
    {
        return $this->name;
    }
}

// ─── Concrete Class ─────────────────────────────────────────────────────────

/**
 * @property string $displayName
 * @property-read bool $isAdmin
 * @method bool hasPermission(string $permission)
 */
class User extends Model implements Renderable
{
    use HasTimestamps;
    use HasSlug;

    public string $email;
    protected Status $status;
    private array $roles = [];

    public static string $defaultRole = 'user';

    public const string TYPE_ADMIN = 'admin';
    public const string TYPE_USER = 'user';

    public function __construct(
        string $name,
        string $email,
        private readonly string $password = '',
        public int $age = 0,
    ) {
        parent::__construct($name);
        $this->email = $email;
        $this->status = Status::Active;
    }

    public function getEmail(): string
    {
        return $this->email;
    }

    public function getStatus(): Status
    {
        return $this->status;
    }

    public function setStatus(Status $status): self
    {
        $this->status = $status;
        return $this;
    }

    public function addRoles(string ...$roles): void
    {
        $this->roles = array_merge($this->roles, $roles);
    }

    public function getRoles(): array
    {
        return $this->roles;
    }

    public function getProfile(): UserProfile
    {
        return new UserProfile($this);
    }

    public function toArray(): array
    {
        return [
            'id' => $this->getId(),
            'name' => $this->getName(),
            'email' => $this->email,
            'status' => $this->status->value,
        ];
    }

    public function format(string $template): string
    {
        return str_replace('{name}', $this->getName(), $template);
    }

    public static function findByEmail(string $email): ?self
    {
        return null;
    }

    protected function hashPassword(string $raw): string
    {
        return password_hash($raw, PASSWORD_BCRYPT);
    }

    private function secretInternalMethod(): void {}
}

// ─── Related Classes ────────────────────────────────────────────────────────

class UserProfile
{
    public string $bio = '';

    public function __construct(private User $user) {}

    public function getUser(): User
    {
        return $this->user;
    }

    public function setBio(string $bio): self
    {
        $this->bio = $bio;
        return $this;
    }

    public function getDisplayName(): string
    {
        return $this->user->getName() . ' (' . $this->user->getEmail() . ')';
    }
}

final class AdminUser extends User
{
    /** @var string[] */
    private array $permissions = [];

    public function __construct(string $name, string $email)
    {
        parent::__construct($name, $email); // parent:: shows inherited methods
    }

    public function toArray(): array
    {
        $base = parent::toArray();          // parent:: resolves overridden method
        $base['connection'] = parent::CONNECTION; // parent:: resolves inherited constant
        $base['permissions'] = $this->permissions;
        return $base;
    }

    public function grantPermission(string $permission): void
    {
        $this->permissions[] = $permission;
    }
}

class Response
{
    public function __construct(
        private string|int $statusCode,
        private string|array|null $body = null,
    ) {}

    public function getStatusCode(): string|int
    {
        return $this->statusCode;
    }

    public function getBody(): string|array|null
    {
        return $this->body;
    }

    public function isSuccess(): bool
    {
        return $this->statusCode >= 200 && $this->statusCode < 300;
    }
}

// ─── Generics (@template / @extends) ───────────────────────────────────────

/**
 * A generic repository — the base class declares template parameters
 * that child classes fill in with concrete types via @extends.
 *
 * @template T
 */
class Repository
{
    /** @var T|null */
    protected $cached = null;

    /** @return T */
    public function find(int $id)
    {
        return $this->cached;
    }

    /** @return T|null */
    public function findOrNull(int $id)
    {
        return $this->cached;
    }

    /** @return T */
    public function first()
    {
        return $this->cached;
    }
}

/**
 * Concrete repository: @extends tells the engine that T = User.
 * All inherited methods now return User instead of the abstract T.
 *
 * @extends Repository<User>
 */
class UserRepository extends Repository
{
    public function findByEmail(string $email): ?User
    {
        return null;
    }
}

/**
 * A generic collection with two template parameters.
 *
 * @template TKey of array-key
 * @template-covariant TValue
 */
class TypedCollection
{
    /** @var array<TKey, TValue> */
    protected array $items = [];

    /** @return TValue */
    public function first() { return reset($this->items); }

    /** @return ?TValue */
    public function last() { return end($this->items) ?: null; }

    /** @return static */
    public function filter(callable $fn): static { return $this; }

    /** @return int */
    public function count(): int { return count($this->items); }

    /** @return array<TKey, TValue> */
    public function all(): array { return $this->items; }
}

/**
 * A user collection — TKey = int, TValue = User.
 *
 * @extends TypedCollection<int, User>
 */
class UserCollection extends TypedCollection
{
    public function adminsOnly(): self
    {
        return $this;
    }
}

/**
 * Chained generics: this extends UserRepository, which extends
 * Repository<User>.  Grandparent methods resolve through the chain.
 */
class CachingUserRepository extends UserRepository
{
    public function clearCache(): void {}
}

/**
 * Demonstrates @phpstan-extends (the PHPStan-prefixed variant).
 *
 * @phpstan-extends TypedCollection<string, Response>
 */
class ResponseCollection extends TypedCollection
{
}

// ─── Container (conditional return types) ───────────────────────────────────

class Container
{
    /** @var array<string, object> */
    private array $bindings = [];

    /**
     * @template TClass
     * @param string|null $abstract
     * @return ($abstract is class-string<TClass> ? TClass : mixed)
     */
    public function make(?string $abstract = null): mixed
    {
        if ($abstract === null) {
            return $this;
        }
        return $this->bindings[$abstract] ?? new Exception();
    }

    public function bind(string $abstract, object $obj): void
    {
        $this->bindings[$abstract] = $obj;
    }

    public function getStatus(): int
    {
        return 200;
    }
}

// ─── Standalone Functions ───────────────────────────────────────────────────

/**
 * @template TClass
 * @param string|null $abstract
 * @return ($abstract is class-string<TClass> ? TClass : Container)
 */
function app(?string $abstract = null): mixed
{
    static $container = null;
    if ($container === null) {
        $container = new Container();
    }
    return $abstract !== null ? $container->make($abstract) : $container;
}

function createUser(string $name, string $email): User
{
    return new User($name, $email);
}

function findOrFail(int $id): User|AdminUser
{
    return new User('test', 'test@example.com');
}

function getUnknownValue(): mixed
{
    return new AdminUser('', '');
}

// ─── Custom Assert Functions ────────────────────────────────────────────────

/** @phpstan-assert User $value */
function assertUser(mixed $value): void
{
    if (!$value instanceof User) {
        throw new \InvalidArgumentException('Expected User');
    }
}

/** @phpstan-assert-if-true AdminUser $value */
function isAdmin(mixed $value): bool
{
    return $value instanceof AdminUser;
}

/** @phpstan-assert-if-false AdminUser $value */
function isRegularUser(mixed $value): bool
{
    return !$value instanceof AdminUser;
}


// ═══════════════════════════════════════════════════════════════════════════
//  Usage Examples — try completion (→) and go-to-definition (⌘-click) here
// ═══════════════════════════════════════════════════════════════════════════


// ── Instance Completion (→) ─────────────────────────────────────────────────

$user = new User('Alice', 'alice@example.com');
$user->getEmail();           // own method
$user->email;                // own property
$user->age;                  // constructor-promoted property
$user->uuid;                 // readonly promoted property from Model
$user->getCreatedAt();       // from HasTimestamps trait
$user->generateSlug('Hi');   // from HasSlug trait
$user->getName();            // inherited from Model
$user->displayName;          // @property magic member
$user->hasPermission('x');   // @method magic member


// ── Static Completion (::) ──────────────────────────────────────────────────

User::$defaultRole;         // static property
User::TYPE_ADMIN;           // class constant
User::findByEmail('a@b.c'); // static method
User::make('Bob');          // inherited static from Model
User::query();              // from @mixin Builder on Model (inherited)


// ── Enum Completion ─────────────────────────────────────────────────────────

Status::Active;              // enum case
Status::Active->label();     // method on backed enum
Priority::High;              // int-backed enum case
Mode::Manual;                // unit enum case


// ── Method Chaining ─────────────────────────────────────────────────────────

$user->setName('Bob')->setStatus(Status::Active)->getEmail();


// ── Property Chain Resolution ───────────────────────────────────────────────

$user->getProfile()->getDisplayName();
$profile = $user->getProfile();
$profile->getUser()->getEmail();


// ── Null-Safe Chaining ──────────────────────────────────────────────────────

$maybe = User::find(1);
$maybe?->getProfile()?->getDisplayName();


// ── Static Return Type Resolution ───────────────────────────────────────────

$made = User::make('Charlie');
$made->getEmail();


// ── Function Return Type Resolution ─────────────────────────────────────────

$u = createUser('Dana', 'dana@example.com');
$u->getName();               // resolves via createUser() return type


// ── Conditional Return Types ────────────────────────────────────────────────

$container = new Container();
$resolved = $container->make(User::class);
$resolved->getEmail();       // conditional return resolves to User

$appContainer = app();               // no arg → returns Container
$appContainer->getStatus();
$appUser = app(User::class);         // class-string arg → returns User
$appUser->getEmail();


// ── Union Return Types ──────────────────────────────────────────────────────

$found = findOrFail(1);     // User|AdminUser
$found->getName();           // available on both types


// ── Intersection Types ──────────────────────────────────────────────────────

function handleIntersection(User&Loggable $entity): void
{
    $entity->getEmail();     // from User
    $entity->log('saved');   // from Loggable
}


// ── use ... as ... (Class Alias) ────────────────────────────────────────────

$p = new Profile(new User('Eve', 'eve@example.com'));
$p->getDisplayName();        // Profile resolves to Demo\UserProfile via alias



// ── Variable Go-To-Definition ───────────────────────────────────────────────

$typed = getUnknownValue();
echo $typed;               // go-to-def on $typed jumps to its assignment above


// ── @var Docblock Type Override ─────────────────────────────────────────────

/** @var User $hinted */
$hinted = getUnknownValue();
$hinted->getEmail();         // type comes from @var docblock

/** @var AdminUser $inlineHinted */
$inlineHinted = getUnknownValue();
$inlineHinted->grantPermission('write');


// ── Ambiguous Variables (Conditional Branches) ──────────────────────────────

if (rand(0, 1)) {
    $ambiguous = new Container();
} else {
    $ambiguous = new AdminUser('Y', 'y@example.com');
}
$ambiguous->getStatus();     // available on both Container and AdminUser


// ═══════════════════════════════════════════════════════════════════════════
//  Type Narrowing — completion adapts to runtime type checks
// ═══════════════════════════════════════════════════════════════════════════


// ── instanceof ──────────────────────────────────────────────────────────────

$a = findOrFail(1);          // User|AdminUser
if ($a instanceof AdminUser) {
    $a->grantPermission('x');    // narrowed to AdminUser
} else {
    $a->getEmail();              // narrowed to User
}

// negated instanceof
$b = findOrFail(1);
if (!$b instanceof AdminUser) {
    $b->getEmail();              // narrowed to User
}


// ── is_a() ──────────────────────────────────────────────────────────────────

$c = getUnknownValue();
if (is_a($c, AdminUser::class)) {
    $c->grantPermission('edit'); // narrowed to AdminUser
}


// ── get_class() / ::class Identity ──────────────────────────────────────────

$d = findOrFail(1);
if (get_class($d) === User::class) {
    $d->getEmail();              // narrowed to exactly User
}

$e = findOrFail(1);
if ($e::class === AdminUser::class) {
    $e->grantPermission('x');    // narrowed to AdminUser
}


// ── assert() ────────────────────────────────────────────────────────────────

$f = getUnknownValue();
assert($f instanceof User);
$f->getEmail();                  // narrowed unconditionally


// ── match(true) ─────────────────────────────────────────────────────────────

$g = getUnknownValue();
$result = match (true) {
    $g instanceof AdminUser => $g->grantPermission('approve'),
    is_a($g, User::class)  => $g->getEmail(),
    default                 => null,
};


// ── while Loop Narrowing ────────────────────────────────────────────────────

$h = getUnknownValue();
while ($h instanceof User) {
    $h->getEmail();              // narrowed inside loop body
    break;
}


// ═══════════════════════════════════════════════════════════════════════════
//  Custom Assert Narrowing (@phpstan-assert / @psalm-assert)
// ═══════════════════════════════════════════════════════════════════════════


// ── Unconditional (@phpstan-assert) ─────────────────────────────────────────

$i = getUnknownValue();
assertUser($i);
$i->getEmail();                  // narrowed to User after assertion


// ── Conditional True (@phpstan-assert-if-true) ──────────────────────────────

$j = findOrFail(1);
if (isAdmin($j)) {
    $j->grantPermission('sudo'); // then-branch: AdminUser
} else {
    $j->getEmail();              // else-branch: User
}


// ── Conditional False (@phpstan-assert-if-false) ────────────────────────────

$k = findOrFail(1);
if (isRegularUser($k)) {
    $k->getEmail();              // then-branch: User (AdminUser excluded)
} else {
    $k->grantPermission('x');    // else-branch: AdminUser
}


// ── Negated Condition ───────────────────────────────────────────────────────

$l = findOrFail(1);
if (!isAdmin($l)) {
    $l->getEmail();              // negated → User
} else {
    $l->grantPermission('y');    // negated else → AdminUser
}


// ── Generics / Foreach Element Types ────────────────────────────────────────

/** @var list<User> $users */
$users = getUnknownValue();
foreach ($users as $user) {
    $user->getEmail();           // $user resolved to User via list<User>
    $user->getName();
}

/** @var User[] $members */
$members = getUnknownValue();
foreach ($members as $member) {
    $member->getStatus();        // $member resolved to User via User[]
}

/** @var array<int, AdminUser> $admins */
$admins = getUnknownValue();
foreach ($admins as $admin) {
    $admin->grantPermission('x'); // $admin resolved to AdminUser via array<int, AdminUser>
}


// ── Array Access Element Types ──────────────────────────────────────────────

/** @var list<User> $users */
$users = getUnknownValue();
$users[0]->getEmail();           // element resolved to User via list<User>

/** @var User[] $members */
$members = getUnknownValue();
$members[0]->getName();          // element resolved to User via User[]

/** @var array<int, AdminUser> $admins */
$admins = getUnknownValue();
$admins[0]->grantPermission('x'); // element resolved to AdminUser via array<int, AdminUser>
$key = 0;
$admins[$key]->grantPermission('y'); // variable key works too

$admin = $admins[0];
$admin->grantPermission('z');    // assigned from array access → AdminUser

$user = $users[0];
$user->getEmail();               // assigned from array access → User


// ═══════════════════════════════════════════════════════════════════════════
//  Generics — @template / @extends type resolution
// ═══════════════════════════════════════════════════════════════════════════
//
//  When a parent class declares @template parameters and a child class
//  provides concrete types via @extends, all inherited methods and
//  properties have their template types replaced with the real types.


// ── Basic @extends — Repository<User> ───────────────────────────────────────

$repo = new UserRepository();
$repo->find(1)->getEmail();      // find() returns T → User
$repo->first()->getName();       // first() returns T → User
$repo->findOrNull(1)?->getEmail(); // findOrNull() returns ?T → ?User
$repo->findByEmail('a@b.c');     // own method still works


// ── Two Template Parameters — TypedCollection<int, User> ────────────────────

$users = new UserCollection();
$users->first()->getEmail();     // first() returns TValue → User
$users->last()?->getName();      // last() returns ?TValue → ?User
$users->adminsOnly();            // own method returns self
$users->filter(fn($u) => true); // filter() returns static → UserCollection
$users->count();                 // count() returns int (non-template, unchanged)


// ── Chained / Grandparent Generics ──────────────────────────────────────────

$cachingRepo = new CachingUserRepository();
$cachingRepo->find(1)->getEmail();    // grandparent Repository<User>::find() → User
$cachingRepo->first()->getName();     // grandparent first() → User
$cachingRepo->clearCache();           // own method


// ── @phpstan-extends Variant ────────────────────────────────────────────────

$responses = new ResponseCollection();
$responses->first()->getStatusCode(); // first() returns TValue → Response
$responses->last()?->getBody();       // last() returns ?TValue → ?Response


// ── Property Type Substitution ──────────────────────────────────────────────
//  Inherited properties with template types are also substituted.

class PropertyDemo extends UserRepository {
    function test() {
        $this->cached->getEmail();   // $cached has type T → User
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// Method-Level @template Support
// ═══════════════════════════════════════════════════════════════════════════
//
// PHPantomLSP resolves method-level @template parameters from call-site
// arguments.  The canonical pattern:
//
//   @template T
//   @param class-string<T> $class
//   @return T
//
// When you call such a method with `SomeClass::class`, the return type
// is resolved to `SomeClass` — enabling full completion on the result.

// ── Service Locator / DI Container Pattern ──────────────────────────────────

class ServiceLocator {
    /**
     * @template T
     * @param class-string<T> $id
     * @return T
     */
    public function get(string $id): object
    {
        // ...
    }
}

$locator = new ServiceLocator();
$locator->get(User::class)->getEmail();          // Resolved: User
$locator->get(UserProfile::class)->setBio('hi'); // Resolved: UserProfile


// ── Entity Manager / Repository Pattern ─────────────────────────────────────

class EntityManager {
    /**
     * @template TEntity
     * @param class-string<TEntity> $entityClass
     * @return TEntity
     */
    public function find(string $entityClass): object
    {
        // ...
    }

    /**
     * @template TEntity
     * @param class-string<TEntity> $entityClass
     * @return TEntity|null
     */
    public function findOrNull(string $entityClass): ?object
    {
        // ...
    }
}

$em = new EntityManager();
$em->find(User::class)->getName();               // Resolved: User
$em->find(AdminUser::class)->grantPermission(''); // Resolved: AdminUser
$em->findOrNull(Response::class)?->getBody();    // Resolved: ?Response

// Inline chain (no intermediate variable):
$em->find(UserProfile::class)->getDisplayName(); // Resolved: UserProfile


// ── Static Method with @template ────────────────────────────────────────────

class Factory {
    /**
     * @template T
     * @param class-string<T> $class
     * @return T
     */
    public static function create(string $class): object
    {
        return new $class();
    }
}

Factory::create(User::class)->getEmail();        // Resolved: User


// ── Standalone Function with @template ──────────────────────────────────────

/**
 * @template T
 * @param class-string<T> $class
 * @return T
 */
function resolve(string $class): object
{
    return new $class();
}

resolve(AdminUser::class)->grantPermission('x'); // Resolved: AdminUser
$user = resolve(User::class);
$user->getEmail();                               // Resolved: User


// ── @template with $this-> context ──────────────────────────────────────────

class AbstractRepository2 {
    /**
     * @template T
     * @param class-string<T> $class
     * @return T
     */
    public function load(string $class): object { return new $class(); }

    public function demo(): void {
        $this->load(User::class)->getEmail();    // Resolved: User
    }
}

// ─── Generic Context Preservation ───────────────────────────────────────────
//
// When a property or method return type carries generic parameters
// (e.g. `Collection<int, User>`), the generic context is preserved so
// that template parameters are substituted on the resolved class.
// This enables full type inference through chained access.

/**
 * A generic wrapper with a single template parameter.
 *
 * @template T
 */
class Box
{
    /** @var T */
    public $value;

    /** @return T */
    public function unwrap() { return $this->value; }
}

class Gift
{
    public function open(): string { return 'surprise!'; }
    public function getTag(): string { return 'birthday'; }
}

class GiftShop
{
    /** @var Box<Gift> */
    public $giftBox;

    /** @return TypedCollection<int, Gift> */
    public function getGifts(): TypedCollection { return new TypedCollection(); }

    public function demo(): void {
        // ── Property with generic @var ──
        // $this->giftBox is Box<Gift>, so unwrap() returns Gift.
        $this->giftBox->unwrap()->open();        // Resolved: Gift::open()
        $this->giftBox->unwrap()->getTag();       // Resolved: Gift::getTag()

        // ── Method with generic @return ──
        // getGifts() returns TypedCollection<int, Gift>, so first() returns Gift.
        $this->getGifts()->first()->open();       // Resolved: Gift::open()
        $this->getGifts()->first()->getTag();     // Resolved: Gift::getTag()

        // ── Property chain: $this->prop->method() ──
        // The subject extraction now captures the full chain so that
        // $this->giftBox->unwrap() correctly resolves through the property.
        $box = $this->giftBox;
        $box->unwrap()->open();                   // Resolved: Gift::open()

        // ── Nullable union with generics ──
        // `Box<Gift>|null` strips |null but preserves <Gift>.
        /** @var Box<Gift>|null $maybeBox */
        $maybeBox = null;
        $maybeBox->unwrap()->getTag();            // Resolved: Gift::getTag()
    }
}

// ═══════════════════════════════════════════════════════════════════
// 18. Match expression type resolution
// ═══════════════════════════════════════════════════════════════════
// When a variable is assigned from a `match` expression, PHPantomLSP
// collects the types from ALL arms, producing a union of possible types.
// The `default => null` arm (or any scalar arm) is gracefully skipped.

class ElasticProductReviewIndexService {
    public function index(array $markets = []): void {}
    public function reindex(): void {}
}

class ElasticBrandIndexService {
    public function index(array $markets = []): void {}
    public function bulkDelete(array $ids): void {}
}

class MatchExpressionDemo {
    private Response $response;
    private Container $container;

    public function matchWithInstantiations(string $indexName): void {
        // ── Match with new instantiations ──
        // $service resolves to ElasticProductReviewIndexService | ElasticBrandIndexService
        $service = match ($indexName) {
            'product-reviews' => new ElasticProductReviewIndexService(),
            'brands'          => new ElasticBrandIndexService(),
            default           => null,
        };
        $service->index();       // Resolved: shows index() from both classes
        $service->reindex();     // Resolved: ElasticProductReviewIndexService::reindex()
        $service->bulkDelete();  // Resolved: ElasticBrandIndexService::bulkDelete()
    }

    public function matchWithMethodCalls(string $type): void {
        // ── Match with $this->method() calls ──
        // Each arm's return type contributes to the union.
        $result = match ($type) {
            'response'  => $this->response,
            'container' => $this->container,
        };
        $result->getStatusCode();  // Resolved: Response::getStatusCode()
        $result->make();           // Resolved: Container::make()
    }

    public function matchWithStaticCalls(string $source): void {
        // ── Match with static method calls ──
        $model = match ($source) {
            'find' => User::find(1),
            'make' => User::make('test'),
        };
        $model->getEmail();  // Resolved: User::getEmail()
    }
}

// ═══════════════════════════════════════════════════════════════════
// 19. Ternary and null-coalescing type resolution
// ═══════════════════════════════════════════════════════════════════
// When a variable is assigned from a ternary (`?:`) or null-coalescing
// (`??`) expression, PHPantomLSP collects types from both branches.
// Short ternary (`$a ?: $b`) and chained coalescing (`$a ?? $b ?? $c`)
// are also supported.  These compose with match expressions too.

class TernaryDemo {
    /** @var Response */
    private Response $response;
    /** @var Container|null */
    private ?Container $container;

    public function ternaryWithInstantiations(bool $useReal): void {
        // ── Ternary with new instantiations ──
        // $mailer resolves to ElasticProductReviewIndexService | ElasticBrandIndexService
        $mailer = $useReal
            ? new ElasticProductReviewIndexService()
            : new ElasticBrandIndexService();
        $mailer->index();       // Resolved: shows index() from both classes
        $mailer->reindex();     // Resolved: ElasticProductReviewIndexService::reindex()
        $mailer->bulkDelete();  // Resolved: ElasticBrandIndexService::bulkDelete()
    }

    public function nullCoalescingWithFallback(): void {
        // ── Null-coalescing with property and fallback ──
        // $svc resolves to Container | Response
        $svc = $this->container ?? $this->response;
        $svc->make();           // Resolved: Container::make()
        $svc->getStatusCode();  // Resolved: Response::getStatusCode()
    }

    public function mixedTernaryAndMatch(bool $simple, int $mode): void {
        // ── Ternary with match in else branch ──
        // All branch types accumulate: Response + Container + ElasticBrandIndexService
        $handler = $simple
            ? $this->response
            : match ($mode) {
                1 => $this->container ?? new ElasticBrandIndexService(),
                2 => new ElasticProductReviewIndexService(),
            };
        $handler->getStatusCode();  // Resolved: Response::getStatusCode()
        $handler->make();           // Resolved: Container::make()
        $handler->reindex();        // Resolved: ElasticProductReviewIndexService::reindex()
    }
}

// ═══════════════════════════════════════════════════════════════════════
// §14  Property Chains on Non-$this Variables
// ═══════════════════════════════════════════════════════════════════════
// Previously only `$this->prop->` chains worked.  Now `$var->prop->`
// also resolves the property type and offers completions.

class Address {
    public string $city;
    public string $zip;
    public string $country;

    public function format(): string {
        return "{$this->city}, {$this->zip}, {$this->country}";
    }
}

class Customer {
    public Address $address;
    public string $email;

    public function __construct(string $email, Address $address) {
        $this->email = $email;
        $this->address = $address;
    }
}

class Order {
    public Customer $customer;
    public float $total;

    public function __construct(Customer $customer, float $total) {
        $this->customer = $customer;
        $this->total = $total;
    }
}

class PropertyChainDemo {
    public Order $order;

    public function __construct(Order $order) {
        $this->order = $order;
    }

    // ── Simple: $var->prop-> ──
    // Variable assigned via `new`, then chain through its property.
    public function simpleChain(): void {
        $customer = new Customer('test@example.com', new Address());
        $customer->address->city;     // Resolved: Address::$city
        $customer->address->format(); // Resolved: Address::format()
    }

    // ── Deep: $var->prop->subprop-> ──
    // Two levels of property chain resolution.
    public function deepChain(): void {
        $order = new Order(new Customer('a@b.com', new Address()), 99.99);
        $order->customer->address->zip;      // Resolved: Address::$zip
        $order->customer->address->format();  // Resolved: Address::format()
        $order->customer->email;              // Resolved: Customer::$email
    }

    // ── Parameter type hint ──
    // Function parameter types drive property chain resolution.
    public function fromParameter(Customer $cust): void {
        $cust->address->country;  // Resolved: Address::$country
        $cust->address->format(); // Resolved: Address::format()
    }

    // ── @var annotation ──
    // Docblock annotations also drive property chain resolution.
    public function fromDocblock(): void {
        /** @var Order $o */
        $o = loadOrder();
        $o->customer->address->city; // Resolved: Address::$city
    }

    // ── Nullsafe operator ──
    // `$var?->prop->` resolves the same as `$var->prop->`.
    public function nullsafeChain(?Customer $cust): void {
        $cust?->address->city; // Resolved: Address::$city
    }

    // ── Method return + property chain ──
    // Method return type feeds into property chain.
    public function methodThenProperty(): void {
        $repo = new UserRepository();
        $user = $repo->findByEmail('test@example.com');
        // $user is User, so $user->... shows User members
        // (deeper chains work when the property type is a class)
    }

    // ── Mixed with $this ──
    // $this->prop chains still work alongside $var->prop chains.
    public function mixedThisAndVar(): void {
        $this->order->customer->email;        // Resolved: Customer::$email via $this
        $local = new Order(new Customer('x@y.com', new Address()), 50.0);
        $local->customer->address->format();  // Resolved: Address::format() via $local
    }
}

// ── Top-level code ──
// Property chains work in top-level code too (outside any class).
$myOrder = new Order(new Customer('hello@world.com', new Address()), 42.0);
$myOrder->customer->address->city;    // Resolved: Address::$city
$myOrder->customer->address->format(); // Resolved: Address::format()

// ═══════════════════════════════════════════════════════════════════════
// §15  Constructor @param → Promoted Property Override
// ═══════════════════════════════════════════════════════════════════════
// Promoted constructor properties now check `@param` docblock annotations
// for a more specific type than the native hint.  For example,
// `@param list<User> $users` overrides native `array $users`.

class Ingredient {
    public string $name;
    public float $quantity;

    public function format(): string {
        return "{$this->quantity}x {$this->name}";
    }
}

class Recipe {
    /**
     * @param list<Ingredient> $ingredients
     * @param Collection<int, string> $tags
     */
    public function __construct(
        public array $ingredients,     // Overridden to list<Ingredient>
        public object $tags,           // Overridden to Collection<int, string>
        public string $title,          // No override — scalar stays scalar
    ) {}

    public function demo(): void {
        // $this->ingredients has type `list<Ingredient>` from @param,
        // not just `array` from the native hint.
        // This means foreach + property chain works:
        foreach ($this->ingredients as $ingredient) {
            $ingredient->name;      // Resolved: Ingredient::$name
            $ingredient->format();  // Resolved: Ingredient::format()
        }
    }
}

class Kitchen {
    /**
     * @param Recipe $recipe
     */
    public function __construct(
        public object $recipe,  // Overridden to Recipe via @param
    ) {}

    public function cook(): void {
        // $this->recipe is Recipe (not object) thanks to @param override.
        $this->recipe->title;           // Resolved: Recipe::$title
        $this->recipe->ingredients;     // Resolved: Recipe::$ingredients
    }
}

// Property chains on non-$this variables also benefit:
function prepareKitchen(): void {
    $kitchen = new Kitchen(new Recipe([], new \stdClass(), 'Pasta'));
    $kitchen->recipe->title;  // Resolved: Recipe::$title via @param override + $var chain
}

// ─── Trait Generic Substitution (@use) ──────────────────────────────────────
//
// When a trait declares @template parameters and a class uses the trait
// with @use TraitName<ConcreteType>, the template parameters are substituted
// with concrete types in the trait's methods and properties.
// This mirrors the same mechanism used for @extends on parent classes.

/**
 * @template TFactory
 */
trait HasFactory {
    /** @return TFactory */
    public static function factory() {}
}

class UserFactory {
    public function create(): User { return new User('', '', new UserProfile(''), Status::Active); }
    public function count(int $n): static { return $this; }
    public function make(): User { return new User('', '', new UserProfile(''), Status::Active); }
}

/**
 * A trait with two template parameters for key/value lookups.
 *
 * @template TKey
 * @template TValue
 */
trait Indexable {
    /** @return TValue */
    public function get() {}
    /** @return TKey */
    public function key() {}
}

/**
 * @use HasFactory<UserFactory>
 */
class Product {
    use HasFactory;

    public function getPrice(): float { return 0.0; }
}

/**
 * @use Indexable<int, User>
 */
class UserIndex {
    use Indexable;
}

// Try these completions:
//
// Product::factory()->         → shows UserFactory methods: create(), count(), make()
// Product::factory()->create()->  → shows User methods (factory returns UserFactory, create returns User)
//
// $idx = new UserIndex();
// $idx->get()->               → shows User methods (TValue resolved to User)
//
// @phpstan-use variant also works:
// /** @phpstan-use HasFactory<UserFactory> */
// class AnotherModel { use HasFactory; }
// AnotherModel::factory()->   → shows UserFactory methods

function traitGenericDemo(): void {
    // Static method on a class using a generic trait
    Product::factory()->create();   // Resolved: UserFactory::create() → User
    Product::factory()->count(5);   // Resolved: UserFactory::count()
    Product::factory()->make();     // Resolved: UserFactory::make() → User

    // Two-param trait substitution
    $idx = new UserIndex();
    $idx->get()->getEmail();        // Resolved: TValue → User → User::getEmail()
}

// ─── Foreach Key Type Resolution ────────────────────────────────────
//
// When iterating over a generic type with two type parameters
// (e.g. SplObjectStorage<K, V>, WeakMap<K, V>, array<K, V>),
// the foreach key variable resolves to the first type parameter
// and the value variable resolves to the second.
//
// This is most useful when the key type is a class (not a scalar
// like int or string), for example with SplObjectStorage or WeakMap.

class Request {
    public string $method;
    public string $path;
    public function getUri(): string { return $this->path; }
}

class HttpResponse {
    public int $statusCode;
    public function getBody(): string { return ''; }
}

class ForeachKeyDemo {
    /**
     * Object keys: SplObjectStorage<Request, HttpResponse>
     *
     * @param \SplObjectStorage<Request, HttpResponse> $storage
     */
    public function objectKeys(\SplObjectStorage $storage): void {
        // $req resolves to Request, $res resolves to HttpResponse
        foreach ($storage as $req => $res) {
            $req->getUri();     // Resolved: Request::getUri()
            $req->method;       // Resolved: Request::$method
            $res->statusCode;   // Resolved: HttpResponse::$statusCode
            $res->getBody();    // Resolved: HttpResponse::getBody()
        }
    }

    public function weakMapKeys(): void {
        /** @var \WeakMap<User, UserProfile> $profiles */
        $profiles = new \WeakMap();
        foreach ($profiles as $user => $profile) {
            $user->getEmail();          // Resolved: User::getEmail()
            $profile->getDisplayName(); // Resolved: UserProfile::getDisplayName()
        }
    }

    /**
     * Scalar keys (int, string) don't produce completions — correct,
     * since you can't call methods on scalars.
     *
     * @param array<int, User> $users
     */
    public function scalarKeys(array $users): void {
        foreach ($users as $key => $user) {
            // $key is int — no completions on $key->
            $user->getEmail();  // Resolved: User::getEmail()
        }
    }
}
