<?php

/**
 * PHPantomLSP — Feature Showcase
 *
 * A single-file playground for every completion and go-to-definition feature.
 * Trigger completion after -> / :: / $, or Ctrl+Click for go-to-definition.
 *
 * Layout:
 *   1. PLAYGROUND  — try completion and go-to-definition here
 *   2. DEMO CLASSES — features that require class / method context
 *   3. SCAFFOLDING  — supporting definitions (scroll past these)
 */

namespace Demo;

use Exception;
use Stringable;
use Demo\UserProfile as Profile;

// ═══════════════════════════════════════════════════════════════════════════
//  PLAYGROUND — try completion and go-to-definition here
// ═══════════════════════════════════════════════════════════════════════════


// ── Instance Completion ─────────────────────────────────────────────────────

$user = new User('Alice', 'alice@example.com');
$user->getEmail();           // own method
$user->email;                // own property
$user->age;                  // constructor-promoted property
$user->uuid;                 // readonly promoted (from Model)
$user->getCreatedAt();       // trait method (HasTimestamps)
$user->generateSlug('Hi');   // trait method (HasSlug)
$user->getName();            // inherited from Model
$user->displayName;          // @property magic
$user->hasPermission('x');   // @method magic
$user->output;               // @property-read (Renderable interface)
$user->render();             // @method (Renderable interface)


// ── Static & Enum Completion ────────────────────────────────────────────────

User::$defaultRole;          // static property
User::TYPE_ADMIN;            // class constant
User::findByEmail('a@b.c');  // static method
User::make('Bob');           // inherited static (Model)
User::query();               // @mixin Builder (Model)

Status::Active;              // backed enum case
Status::Active->label();     // enum method
Priority::High;              // int-backed enum
Mode::Manual;                // unit enum


// ── Method & Property Chaining ──────────────────────────────────────────────

$user->setName('Bob')->setStatus(Status::Active)->getEmail();
$user->getProfile()->getDisplayName();   // return type chain
$profile = $user->getProfile();
$profile->getUser()->getEmail();         // variable → method chain

$order = new Order(new Customer('a@b.com', new Address()), 42.0);
$order->customer->address->city;         // deep property chain
$order->customer->address->format();

$maybe = User::find(1);                  // null-safe chaining
$maybe?->getProfile()?->getDisplayName();


// ── Return Type Resolution ──────────────────────────────────────────────────

$made = User::make('Charlie');            // static return type
$made->getEmail();

$created = createUser('Dana', 'dana@example.com');
$created->getName();                      // function return type

$container = new Container();
$resolved = $container->make(User::class);
$resolved->getEmail();                    // conditional return: class-string<T> → T

$appUser = app(User::class);              // conditional on standalone function
$appUser->getEmail();

$found = findOrFail(1);                   // User|AdminUser union
$found->getName();                        // available on both types

function handleIntersection(User&Loggable $entity): void {
    $entity->getEmail();                  // from User
    $entity->log('saved');                // from Loggable
}


// ── Class Alias ─────────────────────────────────────────────────────────────

$p = new Profile(new User('Eve', 'eve@example.com'));
$p->getDisplayName();                     // Profile → UserProfile via `use ... as`


// ── @var Docblock Override ──────────────────────────────────────────────────

/** @var User $hinted */
$hinted = getUnknownValue();
$hinted->getEmail();                      // type from @var, not mixed

/** @var AdminUser $inlineHinted */
$inlineHinted = getUnknownValue();
$inlineHinted->grantPermission('write');


// ── Ambiguous Variables ─────────────────────────────────────────────────────

if (rand(0, 1)) {
    $ambiguous = new Container();
} else {
    $ambiguous = new AdminUser('Y', 'y@example.com');
}
$ambiguous->getStatus();                  // available on both branches


// ── Type Narrowing ──────────────────────────────────────────────────────────

$a = findOrFail(1);                       // User|AdminUser
if ($a instanceof AdminUser) {
    $a->grantPermission('x');             // narrowed to AdminUser
} else {
    $a->getEmail();                       // narrowed to User
}

if (!$a instanceof AdminUser) {
    $a->getEmail();                       // negated instanceof
}

$c = getUnknownValue();
if (is_a($c, AdminUser::class)) {
    $c->grantPermission('edit');          // is_a() narrowing
}

$d = findOrFail(1);
if (get_class($d) === User::class) {
    $d->getEmail();                       // get_class() identity
}

$e = findOrFail(1);
if ($e::class === AdminUser::class) {
    $e->grantPermission('x');             // ::class identity
}

$f = getUnknownValue();
assert($f instanceof User);
$f->getEmail();                           // assert() narrowing

$g = getUnknownValue();
$narrowed = match (true) {
    $g instanceof AdminUser => $g->grantPermission('approve'),
    is_a($g, User::class)  => $g->getEmail(),
    default                 => null,
};


// ── Custom Assert Narrowing ─────────────────────────────────────────────────

$i = getUnknownValue();
assertUser($i);                           // @phpstan-assert User $value
$i->getEmail();

$j = findOrFail(1);
if (isAdmin($j)) {                        // @phpstan-assert-if-true AdminUser
    $j->grantPermission('sudo');
} else {
    $j->getEmail();
}

$k = findOrFail(1);
if (isRegularUser($k)) {                  // @phpstan-assert-if-false AdminUser
    $k->getEmail();
} else {
    $k->grantPermission('x');
}


// ── Guard Clause Narrowing (Early Return / Throw) ──────────────────────────

$m = findOrFail(1);                       // User|AdminUser
if (!$m instanceof User) {
    return;                               // early return — guard clause
}
$m->getEmail();                           // narrowed to User after guard

$n = findOrFail(1);
if ($n instanceof AdminUser) {
    throw new Exception('no admins');     // early throw — guard clause
}
$n->getEmail();                           // narrowed to User (AdminUser excluded)

$o = findOrFail(1);
if ($o instanceof User) {
    return;
}
if ($o instanceof AdminUser) {
    return;
}
// $o has been fully narrowed by sequential guards

$q = getUnknownValue();
if (!$q instanceof User) return;          // single-statement guard (no braces)
$q->getEmail();                           // narrowed to User


// ── Ternary Narrowing ──────────────────────────────────────────────────────

$model = findOrFail(1);
$email = $model instanceof User ? $model->getEmail() : 'unknown';


// ── Generics (@template / @extends) ────────────────────────────────────────

$repo = new UserRepository();
$repo->find(1)->getEmail();               // Repository<User>::find() → User
$repo->first()->getName();
$repo->findOrNull(1)?->getEmail();        // ?User

$users = new UserCollection();            // TypedCollection<int, User>
$users->first()->getEmail();
$users->adminsOnly();                     // own method

$cachingRepo = new CachingUserRepository();
$cachingRepo->find(1)->getEmail();        // grandparent generics

$responses = new ResponseCollection();    // @phpstan-extends variant
$responses->first()->getStatusCode();


// ── Method-Level @template ──────────────────────────────────────────────────

$locator = new ServiceLocator();
$locator->get(User::class)->getEmail();           // class-string<T> → T
$locator->get(UserProfile::class)->setBio('hi');

Factory::create(User::class)->getEmail();         // static @template
resolve(AdminUser::class)->grantPermission('x');  // function @template


// ── Trait Generic Substitution ──────────────────────────────────────────────

Product::factory()->create();             // @use HasFactory<UserFactory> → UserFactory
Product::factory()->count(5);

$idx = new UserIndex();                   // @use Indexable<int, User>
$idx->get()->getEmail();                  // TValue → User


// ── Foreach & Array Access ──────────────────────────────────────────────────

/** @var list<User> $members */
$members = getUnknownValue();
foreach ($members as $member) {
    $member->getEmail();                  // element type from list<User>
}
$members[0]->getName();                   // array access element type

/** @var array<int, AdminUser> $admins */
$admins = getUnknownValue();
foreach ($admins as $admin) {
    $admin->grantPermission('x');
}
$admins[0]->grantPermission('y');         // variable key works too


// ── Array Destructuring ────────────────────────────────────────────────────

/** @var list<User> */
[$first, $second] = getUnknownValue();
$first->getEmail();                       // destructured element type


// ── Array Shapes ────────────────────────────────────────────────────────────

$config = ['host' => 'localhost', 'port' => 3306, 'author' => new User('', '')];
$config[''];                              // key completion: host, port, author
$config['author']->getEmail();            // value type → User

$bag = ['status' => 'ok'];
$bag['user'] = new User('', '');          // incremental assignment
$bag[''];                                 // keys: status, user
$bag['user']->getEmail();

/** @var array{first: User, second: AdminUser} $pair */
$pair = getUnknownValue();
$pair['first']->getName();
$pair['second']->grantPermission('admin');

$collected = [];                          // push-style inference
$collected[] = new User('', '');
$collected[] = new AdminUser('', '');
$collected[0]->getName();

$cfg = getAppConfig();
$cfg['logger']->getEmail();               // shape from function return


// ── Object Shapes ───────────────────────────────────────────────────────────

/** @var object{title: string, score: float} $item */
$item = getUnknownValue();
$item->title;                             // object shape property
$item->score;

/** @var object{name: string, value: int}&\stdClass $obj */
$obj = getUnknownValue();
$obj->name;                               // intersected with \stdClass


// ── $_SERVER Superglobal ────────────────────────────────────────────────────

$_SERVER['REQUEST_METHOD'];               // known key completion
$_SERVER['HTTP_HOST'];
$_SERVER['REMOTE_ADDR'];


// ── Clone Expression ────────────────────────────────────────────────────────

$copy = clone $user;
$copy->getEmail();                        // preserves User type

$immutable = new Immutable(42);
$cloned = clone $immutable;
$cloned->getValue();


// ── Constants (Go-to-Definition) ────────────────────────────────────────────

define('APP_VERSION', '1.0.0');
define('MAX_RETRIES', 3);
echo APP_VERSION;                         // Ctrl+Click → jumps to define()
$retries = MAX_RETRIES;


// ── Variable Go-to-Definition ───────────────────────────────────────────────

$typed = getUnknownValue();
echo $typed;                              // Ctrl+Click on $typed → jumps to assignment


// ── Callable Snippet Insertion ──────────────────────────────────────────────
// Completion inserts snippets with tab-stops for required params:

$user->setName('Bob');                    // → setName(${1:$name})
$user->toArray();                         // → toArray()  (no params)
$user->addRoles();                        // → addRoles() (variadic)
User::findByEmail('a@b.c');               // → findByEmail(${1:$email})
$r = new Response(200);                   // → Response(${1:$statusCode})


// ── Type Hint Completion in Definitions ─────────────────────────────────────
// When typing a type hint inside a function/method definition, return type,
// or property declaration, PHPantomLSP offers PHP native scalar types
// (string, int, float, bool, …) alongside class-name completions.
// Constants and standalone functions are excluded since they're invalid
// in type positions.

// Try triggering completion after the `(` or `,` in these signatures:
function typeHintDemo(User $user, string $name): User { return $user; }
//                    ↑ type hint  ↑ scalar      ↑ return type

// Union types, nullable types, and intersection types also work:
function unionDemo(string|int $value, ?User $maybe): User|null { return $maybe; }
//                 ↑ after |   ↑ after ?             ↑ after |

// Property type hints after visibility modifiers:
// (see Model class below — `public readonly string $uuid`)

// Promoted constructor parameters with modifiers:
// (see Customer class below — `private readonly string $email`)

// Closures and arrow functions:
$typedClosure = function(User $u): string { return $u->getName(); };
$typedArrow = fn(int $x): float => $x * 1.5;


// ── parent:: Completion ─────────────────────────────────────────────────────
// Open AdminUser's constructor and toArray() in scaffolding below for
// parent:: examples (inherited methods, overridden methods, constants).


// ═══════════════════════════════════════════════════════════════════════════
//  DEMO CLASSES — features that require class / method context
// ═══════════════════════════════════════════════════════════════════════════
//
//  Open these methods and trigger completion inside them.


// ── Property Chains on $this and Parameters ─────────────────────────────────

class PropertyChainDemo
{
    public Order $order;

    public function __construct(Order $order)
    {
        $this->order = $order;
    }

    public function simpleChain(): void
    {
        $customer = new Customer('test@example.com', new Address());
        $customer->address->city;         // Address::$city
        $customer->address->format();     // Address::format()
    }

    public function deepChain(): void
    {
        $order = new Order(new Customer('a@b.com', new Address()), 99.99);
        $order->customer->address->zip;   // Address::$zip
        $order->customer->email;          // Customer::$email
    }

    public function mixedThisAndVar(): void
    {
        $this->order->customer->email;    // via $this
        $local = new Order(new Customer('x@y.com', new Address()), 50.0);
        $local->customer->address->format(); // via local variable
    }
}


// ── Match / Ternary / Null-Coalescing Type Accumulation ─────────────────────

class ExpressionTypeDemo
{
    private Response $response;
    private ?Container $container;

    public function matchExpr(string $name): void
    {
        $service = match ($name) {
            'reviews' => new ElasticProductReviewIndexService(),
            'brands'  => new ElasticBrandIndexService(),
            default   => null,
        };
        $service->index();                // on both classes
        $service->reindex();              // ElasticProductReviewIndexService only
        $service->bulkDelete([]);         // ElasticBrandIndexService only
    }

    public function ternaryExpr(bool $flag): void
    {
        $svc = $flag
            ? new ElasticProductReviewIndexService()
            : new ElasticBrandIndexService();
        $svc->index();                    // on both
        $svc->reindex();                  // only one branch
    }

    public function nullCoalescing(): void
    {
        $svc = $this->container ?? $this->response;
        $svc->make();                     // Container::make()
        $svc->getStatusCode();            // Response::getStatusCode()
    }
}


// ── Foreach, Key Types, and Destructuring ───────────────────────────────────

class IterationDemo
{
    /** @var list<User> */
    public array $users;

    /** @return list<User> */
    public function getUsers(): array { return []; }

    /** @return array<Request, HttpResponse> */
    public function getMapping(): array { return []; }

    public function foreachFromMethod(): void
    {
        foreach ($this->getUsers() as $user) {
            $user->getEmail();            // list<User> → User
        }
    }

    public function foreachFromProperty(): void
    {
        foreach ($this->users as $user) {
            $user->getEmail();            // list<User> → User
        }
    }

    public function keyTypes(): void
    {
        foreach ($this->getMapping() as $req => $res) {
            $req->getUri();               // Request (key type)
            $res->getBody();              // HttpResponse (value type)
        }
    }

    public function weakMapKeys(): void
    {
        /** @var \WeakMap<User, UserProfile> $profiles */
        $profiles = new \WeakMap();
        foreach ($profiles as $user => $profile) {
            $user->getEmail();            // key: User
            $profile->getDisplayName();   // value: UserProfile
        }
    }

    public function destructuring(): void
    {
        [$a, $b] = $this->getUsers();
        $a->getEmail();                   // destructured element type
        $b->getName();
    }
}


// ── Array & Object Shapes in Methods ────────────────────────────────────────

class ShapeDemo
{
    /**
     * @return array{user: User, profile: UserProfile, active: bool}
     */
    public function getUserData(): array { return []; }

    /**
     * @return object{name: string, age: int, active: bool}
     */
    public function getProfile(): object { return (object) []; }

    /**
     * @return object{user: User, meta: object{page: int, total: int}}
     */
    public function getResult(): object { return (object) []; }

    /**
     * @param array{host: string, port: int, credentials: User} $config
     */
    public function fromParam(array $config): void
    {
        $config['host'];                  // string
        $config['credentials']->getEmail(); // User
    }

    public function fromReturnType(): void
    {
        $data = $this->getUserData();
        $data['user']->getName();         // User
        $data['profile']->setBio('');     // UserProfile
    }

    public function nestedShapes(): void
    {
        /** @var array{meta: array{page: int, total: int}, items: list<User>} $response */
        $response = getUnknownValue();
        $response['meta']['page'];        // nested shape key
        $response['items'][0]->getName(); // list element type
    }

    public function objectShapes(): void
    {
        $profile = $this->getProfile();
        $profile->name;                   // object{name: string, ...}
        $profile->age;

        $result = $this->getResult();
        $result->user->getEmail();        // nested object → User
        $result->meta->page;              // nested object shape
    }
}


// ── Generic Context Preservation ────────────────────────────────────────────

class GiftShop
{
    /** @var Box<Gift> */
    public $giftBox;

    /** @return TypedCollection<int, Gift> */
    public function getGifts(): TypedCollection { return new TypedCollection(); }

    public function demo(): void
    {
        // Property with generic @var — Box<Gift>::unwrap() → Gift
        $this->giftBox->unwrap()->open();
        $this->giftBox->unwrap()->getTag();

        // Method with generic @return — TypedCollection<int, Gift>::first() → Gift
        $this->getGifts()->first()->open();
        $this->getGifts()->first()->getTag();
    }
}


// ── @throws Completion and Catch Variable Types ─────────────────────────────

class ExceptionDemo
{
    /**
     * Typing `@` in this docblock suggests @throws for each uncaught exception.
     *
     * @throws NotFoundException
     * @throws ValidationException
     */
    public function findOrFail(int $id): array
    {
        if ($id < 0) {
            throw new ValidationException('ID must be positive');
        }
        $result = $this->lookup($id);
        if ($result === null) {
            throw new NotFoundException('Record not found');
        }
        return $result;
    }

    /**
     * Caught exceptions are filtered out of @throws suggestions.
     *
     * @throws AuthorizationException
     */
    public function safeOperation(): void
    {
        try {
            throw new \RuntimeException('transient error');
        } catch (\RuntimeException $e) {
            // caught — not suggested
        }
        throw new AuthorizationException('Forbidden');
    }

    /**
     * Called method's @throws propagate to the caller.
     *
     * @throws AuthorizationException
     */
    public function delegatedWork(): void
    {
        $this->safeOperation();
    }

    /**
     * Catch variable resolves to the caught exception type.
     */
    public function catchVariable(): void
    {
        try {
            $this->riskyOperation();
        } catch (ValidationException $e) {
            $e->getMessage();             // ValidationException members
        }
    }

    /**
     * Narrower catch (RuntimeException) doesn't handle broader Exception,
     * so Exception escapes as a propagated @throws.
     *
     * @throws \Exception
     */
    public function propagatedWithCatchFilter(): void
    {
        try {
            $this->throwsException();
        } catch (\RuntimeException $e) {
            // catches RuntimeException, NOT Exception
        }
    }

    private function lookup(int $id): ?array { return null; }
    private function riskyOperation(): void {}

    /** @throws \Exception */
    private function throwsException(): void { throw new \Exception('error'); }
}


// ── Constructor @param → Promoted Property Override ─────────────────────────

class ParamOverrideDemo
{
    /**
     * @param list<Ingredient> $ingredients
     * @param Recipe $recipe
     */
    public function __construct(
        public array $ingredients,          // @param overrides to list<Ingredient>
        public object $recipe,              // @param overrides to Recipe
    ) {}

    public function demo(): void
    {
        // $this->ingredients is list<Ingredient> from @param, not just array
        foreach ($this->ingredients as $ingredient) {
            $ingredient->name;              // Ingredient::$name
            $ingredient->format();          // Ingredient::format()
        }

        // $this->recipe is Recipe from @param, not just object
        $this->recipe->title;               // Recipe::$title
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
// ┃  SCAFFOLDING — Supporting definitions below this line.              ┃
// ┃  Everything below exists to support the playground above.           ┃
// ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛
// ═══════════════════════════════════════════════════════════════════════════


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

/**
 * @template TFactory
 */
trait HasFactory
{
    /** @return TFactory */
    public static function factory() {}
}

/**
 * @template TKey
 * @template TValue
 */
trait Indexable
{
    /** @return TValue */
    public function get() {}

    /** @return TKey */
    public function key() {}
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
            self::Active   => 'Active',
            self::Inactive => 'Inactive',
            self::Pending  => 'Pending',
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

// ─── Concrete Classes ───────────────────────────────────────────────────────

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

/** @extends Repository<User> */
class UserRepository extends Repository
{
    public function findByEmail(string $email): ?User
    {
        return null;
    }
}

class CachingUserRepository extends UserRepository
{
    public function clearCache(): void {}
}

/**
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

/** @extends TypedCollection<int, User> */
class UserCollection extends TypedCollection
{
    public function adminsOnly(): self
    {
        return $this;
    }
}

/** @phpstan-extends TypedCollection<string, Response> */
class ResponseCollection extends TypedCollection {}

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

// ─── Method-Level @template Classes ─────────────────────────────────────────

class ServiceLocator
{
    /**
     * @template T
     * @param class-string<T> $id
     * @return T
     */
    public function get(string $id): object
    {
        return new \stdClass();
    }
}

class Factory
{
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

// ─── Generic Wrapper ────────────────────────────────────────────────────────

/**
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

class Immutable
{
    public function __construct(private int $value) {}

    public function getValue(): int
    {
        return $this->value;
    }

    public function withValue(int $v): self
    {
        $clone = clone $this;
        return $clone;
    }
}

// ─── Expression Type Support Classes ────────────────────────────────────────

class ElasticProductReviewIndexService
{
    public function index(array $markets = []): void {}
    public function reindex(): void {}
}

class ElasticBrandIndexService
{
    public function index(array $markets = []): void {}
    public function bulkDelete(array $ids): void {}
}

// ─── Property Chain Support Classes ─────────────────────────────────────────

class Address
{
    public string $city = '';
    public string $zip = '';
    public string $country = '';

    public function format(): string
    {
        return "{$this->city}, {$this->zip}, {$this->country}";
    }
}

class Customer
{
    public Address $address;
    public string $email;

    public function __construct(string $email, Address $address)
    {
        $this->email = $email;
        $this->address = $address;
    }
}

class Order
{
    public Customer $customer;
    public float $total;

    public function __construct(Customer $customer, float $total)
    {
        $this->customer = $customer;
        $this->total = $total;
    }
}

class Ingredient
{
    public string $name = '';
    public float $quantity = 0.0;

    public function format(): string
    {
        return "{$this->quantity}x {$this->name}";
    }
}

class Recipe
{
    /**
     * @param list<Ingredient> $ingredients
     */
    public function __construct(
        public array $ingredients = [],
        public string $title = '',
    ) {}
}

// ─── Foreach Key Type Support Classes ───────────────────────────────────────

class Request
{
    public string $method = 'GET';
    public string $path = '/';

    public function getUri(): string { return $this->path; }
}

class HttpResponse
{
    public int $statusCode = 200;

    public function getBody(): string { return ''; }
}

// ─── Trait Generic Support Classes ──────────────────────────────────────────

class UserFactory
{
    public function create(): User { return new User('', ''); }
    public function count(int $n): static { return $this; }
    public function make(): User { return new User('', ''); }
}

/** @use HasFactory<UserFactory> */
class Product
{
    use HasFactory;

    public function getPrice(): float { return 0.0; }
}

/** @use Indexable<int, User> */
class UserIndex
{
    use Indexable;
}

// ─── Exception Classes ──────────────────────────────────────────────────────

class NotFoundException extends \RuntimeException {}
class ValidationException extends \RuntimeException {}
class AuthorizationException extends \RuntimeException {}

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

/**
 * @template T
 * @param class-string<T> $class
 * @return T
 */
function resolve(string $class): object
{
    return new $class();
}

/**
 * @return array{logger: User, debug: bool}
 */
function getAppConfig(): array { return []; }

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
