<?php

/**
 * PHP Showcase
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

// ── Alphabetical Auto-Import ────────────────────────────────────────────────
// Try: type `new DateT` and accept `DateTime`. The `use DateTime;` statement
// will be inserted between `use Exception;` and `use Stringable;` to maintain
// alphabetical order.

// ── Use-Import Conflict Resolution ──────────────────────────────────────────
// The `use Exception;` import above occupies the short name "Exception".
// Try: type `throw new pq\Exception();` below and accept the auto-import for
// pq\Exception. The auto-import should insert `\pq\Exception` at the usage site
// instead of adding `use pq\Exception;` (which would conflict).

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

// ── Parenthesized RHS Assignment ─────────────────────────────────────────────
// Try completion after -> on $parenUser, which is assigned from a parenthesized expression.

$parenUser = (new User('Bob', 'bob@example.com'));
$parenUser->getEmail();      // resolves correctly after fix
$parenUser->getName();       // inherited method

// Parenthesized ternary assignment
$ternaryUser = (rand(0, 1) ? new User('Carol', 'carol@example.com') : new AdminUser('Dave', 'dave@example.com'));
$ternaryUser->getEmail();    // available on both branches
$ternaryUser->grantPermission('edit'); // only on AdminUser branch


// ── Class-String Variable Static Access ─────────────────────────────────────
// When a variable holds a class-string from `Foo::class`, using `$var::`
// resolves to the referenced class and offers its static members.

$cls = User::class;
$cls::findByEmail('a@b.c');  // static method from User
$cls::TYPE_ADMIN;            // class constant
$cls::$defaultRole;          // static property

$ref = self::class;          // also works with self::class / static::class


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


// ── Multi-line Method Chains ────────────────────────────────────────────────
// Try: trigger completion after `->` on continuation lines. The resolver joins
// multi-line chains automatically, so fluent builder patterns work seamlessly.

$user->setName('Bob')
    ->setStatus(Status::Active)
    ->getEmail();                         // resolves through the full chain

User::query()
    ->where('active', true)
    ->where('name', 'Bob');               // static call base + continuations

$maybe?->getProfile()
    ?->getDisplayName();                  // nullsafe continuations


// ── Chained Method Calls in Variable Assignment ─────────────────────────────
// When a variable is assigned from a chained call, the full chain is walked
// to resolve the stored type.

$storedProfile = $user->getProfile();
$storedName = $storedProfile->getUser()->getName(); // $var->method()->method()

$directProfile = $user->getProfile()->getUser(); // chain stored in variable
$directProfile->getEmail();              // resolves to User

$staticBuilt = User::make('test');       // Static::method() in assignment
$staticBuilt->getEmail();               // resolves to User

$fromNew = (new UserProfile($user))->getUser(); // (new Class())->method()
$fromNew->getEmail();                    // resolves to User


// ── Return Type Resolution ──────────────────────────────────────────────────

$made = User::make('Charlie');            // static return type → User
$made->getEmail();

$admin = AdminUser::make('Eve');          // static on subclass → AdminUser
$admin->grantPermission('edit');          // resolves to AdminUser, not User/Model

$fluent = $admin->setName('Eve');         // setName returns static → AdminUser
$fluent->grantPermission('delete');       // chained static stays on the subclass

$created = createUser('Dana', 'dana@example.com');
$created->getName();                      // function return type

$container = new Container();
$resolved = $container->make(User::class);
$resolved->getEmail();                    // conditional return: class-string<T> → T

$appUser = app(User::class);              // conditional on standalone function
$appUser->getEmail();

$found = findOrFail(1);                   // User|AdminUser union
$found->getName();                        // available on both types


// ── Method-level @template (General Case) ───────────────────────────────────
// When a method declares @template T and @param T $param, the resolver infers
// T from the actual argument type — not just class-string<T>, but any object.

$mapper = new ObjectMapper();
$mapped = $mapper->wrap($user);           // wrap(@param T $item): Collection<T>
$mapped->first();                         // → User (T resolved to User)

$identity = $mapper->identity($user);     // identity(@param T): T
$identity->getEmail();                    // → User methods

$mapper->wrap(new Product())->first()->getPrice(); // new expression arg → Product


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


// ── Static Property & Class Constant Go-to-Definition ───────────────────────

User::$defaultRole;                       // Ctrl+Click → jumps to static property declaration
User::TYPE_ADMIN;                         // Ctrl+Click → jumps to class constant declaration
Model::CONNECTION;                        // Ctrl+Click → jumps to inherited constant


// ── Variable Go-to-Definition ───────────────────────────────────────────────

$typed = getUnknownValue();
echo $typed;                              // Ctrl+Click on $typed → jumps to assignment


// ── Type Hint Go-to-Definition ──────────────────────────────────────────────
// Ctrl+Click on any class/interface name used as a type hint — in parameters,
// return types, property types, catch blocks, and even inside docblock
// annotations — to jump to its definition.

// Parameter type hints (simple, nullable, union, intersection):
function typeHintGtdParam(User $u): void {}                       // Ctrl+Click User
function typeHintGtdNullable(?User $u): void {}                   // Ctrl+Click User after ?
function typeHintGtdUnion(User|AdminUser $u): void {}             // Ctrl+Click either type
function typeHintGtdIntersection(Renderable&Loggable $x): void {} // Ctrl+Click either type

// Return type hints:
function typeHintGtdReturn(): Response { return new Response(200); } // Ctrl+Click Response

// Catch block exception types — Ctrl+Click NotFoundException or ValidationException:
try { typeHintGtdParam(new User('', '')); } catch (NotFoundException|ValidationException $e) {}

// Extends / implements — Ctrl+Click User, Renderable, or Loggable in scaffolding below.

// ── Context-Aware Class Name Filtering ──────────────────────────────────────
// Completions are filtered by syntactic context. Only valid suggestions appear.
// Try: erase the class name after each keyword and re-type a prefix to see filtering.
//   extends (class)     → non-final classes only (no interfaces, traits, enums, or final classes)
//   extends (interface) → interfaces only
//   implements          → interfaces only
//   use (inside class)  → traits only
//   instanceof          → classes, interfaces, and enums (no traits)
//   new                 → concrete non-abstract classes only

// ── Go-to-Implementation ────────────────────────────────────────────────────
// Right-click → "Go to Implementations" (or editor shortcut) on an interface
// or abstract class name to jump to all concrete classes that implement it.
// Also works on method calls typed as an interface/abstract class.

// Try: Go-to-Implementation on "Renderable" → jumps to User and HtmlReport
//      Go-to-Implementation on "format" below → jumps to format() in each implementor
function renderDemo(Renderable $item): string {
    return $item->format('<b>{name}</b>');
}


// Docblock type references — Ctrl+Click class names inside these annotations:
/**
 * @param TypedCollection<int, User> $items   Ctrl+Click User or TypedCollection
 * @return Response                           Ctrl+Click Response
 * @throws NotFoundException                  Ctrl+Click NotFoundException
 */
function typeHintGtdDocblock($items) { return new Response(200); }

/** @var User $docblockVar */
$docblockVar = getUnknownValue();
$docblockVar->getEmail();                 // Ctrl+Click User in the @var above


// ── Callable Snippet Insertion ──────────────────────────────────────────────
// Completion inserts snippets with tab-stops for required params:

$user->setName('Bob');                    // → setName(${1:$name})
$user->toArray();                         // → toArray()  (no params)
$user->addRoles();                        // → addRoles() (variadic)
User::findByEmail('a@b.c');               // → findByEmail(${1:$email})
$r = new Response(200);                   // → Response(${1:$statusCode})


// ── Type Hint Completion in Definitions ─────────────────────────────────────
// When typing a type hint inside a function/method definition, return type,
// or property declaration, completion offers PHP native scalar types
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


// ── Callable / Closure Variable Invocation ──────────────────────────────────
// When a variable holds a closure or callable, invoking it resolves the
// return type for completion.

// Closure literal with native return type hint:
$makeUser = function(): User { return new User('test', 'test@example.com'); };
$makeUser()->getEmail();                  // resolves User from closure return type

// Arrow function literal:
$makeProfile = fn(): UserProfile => new UserProfile(new User('x', 'x@x.com'));
$makeProfile()->getDisplayName();         // resolves UserProfile from arrow fn return type

// Closure with `use` clause:
$name = 'Alice';
$factory = function() use ($name): User { return new User($name, 'a@b.com'); };
$factory()->getEmail();                   // `use` clause doesn't interfere

// Docblock callable return type annotation:
/** @var \Closure(): Response $responder */
$responder = getUnknownValue();
$responder()->getBody();                  // resolves Response from @var Closure(): Response

/** @var callable(string): User $loader */
$loader = getUnknownValue();
$loader('test')->getEmail();              // resolves User from callable(string): User

// Chaining after callable invocation:
$builder = function(): User { return new User('x', 'x@x.com'); };
$builder()->setName('Bob')->getEmail();   // chain works after $fn()

// Variable assigned from callable invocation:
$fromClosure = $makeUser();
$fromClosure->getEmail();                 // $result = $fn() resolves return type


// ── Spread Operator Type Tracking ───────────────────────────────────────────
// When array literals contain spread expressions (`...$var`), element types
// are resolved from the spread variable's iterable annotation and merged.

/** @var list<User> $users */
$users = [];
/** @var list<AdminUser> $admins */
$admins = [];

// Single spread — preserves element type:
$allUsers = [...$users];
$allUsers[0]->getEmail();                 // resolves User from list<User>

// Multiple spreads — union of element types:
$everyone = [...$users, ...$admins];
$everyone[0]->getEmail();                 // resolves User|AdminUser

// Works with array<K,V> and Type[] annotations too:
/** @var array<int, User> $indexed */
$indexed = [];
$copy = [...$indexed];
$copy[0]->getName();                      // resolves User from array<int, User>

/** @var User[] $typed */
$typed = [];
$merged = [...$typed];
$merged[0]->getEmail();                   // resolves User from User[]

// Spread combined with push-style assignments:
$mixed = [...$users];
$mixed[] = new AdminUser('root', 'root@example.com');
$mixed[0]->getName();                     // resolves User|AdminUser

// Works with array() syntax too:
$legacy = array(...$users, ...$admins);
$legacy[0]->getEmail();                   // resolves User|AdminUser


// ── Multi-line @return & Broken Docblock Recovery ───────────────────────────

$collection = collect([]);
$collection->groupBy('key');             // multi-line @return resolves correctly
$collection->map(fn($x) => $x);         // map() works despite groupBy's complex @return

$recovered = (new BrokenDocRecovery())->broken();
$recovered->working();                   // recovers `static` from broken @return static<


// ── Foreach over Generic Collection Classes ─────────────────────────────────
// foreach resolves element types from @extends / @implements generic params.

$eloquentUsers = new UserEloquentCollection();
foreach ($eloquentUsers as $eu) {
    $eu->getEmail();                     // resolves to User via @extends generics
}

// Open CollectionForeachDemo methods below for more examples.


// ── Type Aliases (@phpstan-type / @phpstan-import-type) ─────────────────────

$aliasDemo = new TypeAliasDemo();
$userData = $aliasDemo->getUserData();
$userData['name'];                       // @phpstan-type UserData → array shape key completion

$importDemo = new TypeAliasImportDemo();
$imported = $importDemo->fetchUser();
$imported['email'];                      // @phpstan-import-type UserData from TypeAliasDemo


// ── Anonymous Classes ───────────────────────────────────────────────────────
// $this-> inside anonymous class bodies resolves to the anonymous class's
// own members. Supports extends, implements, trait use, and promoted properties.

$anon = new class extends Model {
    public string $label;
    public function tag(): string { return ''; }
    public function demo() {
        $this->tag();                    // own method
        $this->label;                    // own property
        $this->getName();                // inherited from Model
    }
};


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


// ── Switch Statement Type Tracking ──────────────────────────────────────────

class SwitchDemo
{
    public function switchNewInstantiation(string $type): void
    {
        switch ($type) {
            case 'reviews':
                $service = new ElasticProductReviewIndexService();
                break;
            case 'brands':
                $service = new ElasticBrandIndexService();
                break;
        }
        $service->index();                // on both classes
        $service->reindex();              // ElasticProductReviewIndexService only
        $service->bulkDelete([]);         // ElasticBrandIndexService only
    }

    public function switchInsideCase(string $driver): void
    {
        switch ($driver) {
            case 'mysql':
                $conn = new Response(200, 'OK');
                $conn->getStatusCode();   // resolves inside the case body
                break;
        }
    }
}


// ── Match Class-String Forwarding to Conditional Return Types ───────────────
// When a variable holds a ::class value from a match expression and is then
// passed to a function/method with @template T + @param class-string<T> +
// @return T, the resolver traces the class-string back through the match arms.

class MatchClassStringDemo
{
    public function viaMethod(string $typeName): void
    {
        $container = new Container();
        $requestType = match ($typeName) {
            'reviews' => ElasticProductReviewIndexService::class,
            'brands'  => ElasticBrandIndexService::class,
        };
        $requestBody = $container->make($requestType);
        $requestBody->index();            // on both classes
        $requestBody->reindex();          // ElasticProductReviewIndexService only
        $requestBody->bulkDelete([]);     // ElasticBrandIndexService only
    }

    public function viaFunction(string $typeName): void
    {
        $cls = match ($typeName) {
            'reviews' => ElasticProductReviewIndexService::class,
            'brands'  => ElasticBrandIndexService::class,
        };
        $resolved = resolve($cls);
        $resolved->index();               // on both classes
        $resolved->reindex();             // ElasticProductReviewIndexService only
    }

    public function inlineChain(string $typeName): void
    {
        $container = new Container();
        $cls = match ($typeName) {
            'reviews' => ElasticProductReviewIndexService::class,
            'brands'  => ElasticBrandIndexService::class,
        };
        $container->make($cls)->index();  // inline chain also works
    }

    public function simpleClassStringVar(): void
    {
        $container = new Container();
        $cls = User::class;
        $user = $container->make($cls);
        $user->getEmail();                // resolves through simple $cls variable
    }

    public function ternaryClassString(bool $flag): void
    {
        $container = new Container();
        $cls = $flag ? User::class : AdminUser::class;
        $obj = $container->make($cls);
        $obj->getName();                  // shared by both User and AdminUser
        $obj->grantPermission('edit');    // from AdminUser branch
    }
}


// ── Template Parameter Bounds ───────────────────────────────────────────────
// When a property's type is a template parameter (e.g. TNode), the resolver
// falls back to the upper bound declared in @template TNode of SomeClass.

/**
 * @template-covariant TNode of AstNode
 */
class TemplateBoundsDemo
{
    /** @var TNode */
    public $node;

    /**
     * @param TNode $node
     */
    public function __construct(AstNode $node)
    {
        $this->node = $node;
    }

    public function demo(): void
    {
        $this->node->getChildren();       // resolves via TNode's bound: AstNode
        $this->node->getParent();         // AstNode::getParent()
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

// ── Generator / Iterable Yield Type Resolution ─────────────────────────────

class GeneratorDemo
{
    /** @return \Generator<int, User> */
    public function getUsers(): \Generator
    {
        yield new User('Alice', 'alice@example.com');
        yield new User('Bob', 'bob@example.com');
    }

    /** @return \Generator<int, Order, mixed, Response> */
    public function processOrders(): \Generator
    {
        yield new Order(new Customer('Test', new Address('Main St', 'NYC', '10001')));
    }

    /** @return iterable<User> */
    public function iterableUsers(): iterable
    {
        return [];
    }

    public function foreachGeneratorTwoParams(): void
    {
        // Generator<int, User> — value is 2nd param (User)
        foreach ($this->getUsers() as $user) {
            $user->getEmail();            // resolves to User
            $user->getName();             // resolves to User
        }
    }

    public function foreachGeneratorFourParams(): void
    {
        // Generator<int, Order, mixed, Response> — value is still 2nd param (Order)
        foreach ($this->processOrders() as $order) {
            $order->getId();              // resolves to Order (2nd param), not Response (4th)
        }
    }

    public function foreachGeneratorVarAnnotation(): void
    {
        /** @var \Generator<int, User> $gen */
        $gen = $this->getUsers();
        foreach ($gen as $user) {
            $user->getEmail();            // Generator<int, User> → User
        }
    }

    public function foreachGeneratorFourParamsVar(): void
    {
        /** @var \Generator<int, User, mixed, Response> $gen */
        $gen = $this->processOrders();
        foreach ($gen as $item) {
            $item->getEmail();            // 2nd param: User (not Response)
        }
    }

    public function foreachIterableSingleParam(): void
    {
        // iterable<User> — single param is the value type
        foreach ($this->iterableUsers() as $user) {
            $user->getEmail();            // resolves to User
        }
    }

    public function foreachGeneratorPropertyChain(): void
    {
        foreach ($this->getUsers() as $user) {
            $user->getProfile()->getDisplayName();  // User → UserProfile → string
        }
    }

    /**
     * @param \Generator<int, Customer> $customers
     */
    public function foreachGeneratorParam(\Generator $customers): void
    {
        // @param annotation overrides native \Generator type
        foreach ($customers as $customer) {
            $customer->getName();         // resolves to Customer
        }
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

// ── Foreach over Generic Collection Classes ─────────────────────────────────

class CollectionForeachDemo
{
    public UserEloquentCollection $users;

    public function getUsers(): UserEloquentCollection
    {
        return new UserEloquentCollection();
    }

    public function foreachNewCollection(): void
    {
        $items = new UserEloquentCollection();
        foreach ($items as $item) {
            $item->getEmail();            // resolves to User via @extends generics
        }
    }

    public function foreachMethodReturn(): void
    {
        foreach ($this->getUsers() as $user) {
            $user->getName();             // resolves via method return type → collection generics
        }
    }

    public function foreachProperty(): void
    {
        foreach ($this->users as $user) {
            $user->getEmail();            // resolves via property type → collection generics
        }
    }

    public function foreachVariable(): void
    {
        $collection = $this->getUsers();
        foreach ($collection as $user) {
            $user->getName();             // resolves via variable assignment scanning
        }
    }
}

// ── Type Aliases (@phpstan-type / @phpstan-import-type) ─────────────────────

/**
 * @phpstan-type UserData array{name: string, email: string, age: int}
 * @phpstan-type StatusInfo array{code: int, label: string}
 */
class TypeAliasDemo
{
    /** @return UserData */
    public function getUserData(): array
    {
        return ['name' => 'Alice', 'email' => 'alice@example.com', 'age' => 30];
    }

    /** @return StatusInfo */
    public function getStatus(): array
    {
        return ['code' => 200, 'label' => 'OK'];
    }

    public function demo(): void
    {
        // Try: $this->getUserData()['   ← offers name, email, age
        $data = $this->getUserData();
        $data['name'];                    // resolves UserData alias → array shape keys

        // Try: $this->getStatus()['     ← offers code, label
        $status = $this->getStatus();
        $status['label'];                 // resolves StatusInfo alias → array shape keys
    }
}

/**
 * @phpstan-import-type UserData from TypeAliasDemo
 * @phpstan-import-type StatusInfo from TypeAliasDemo as AliasedStatus
 */
class TypeAliasImportDemo
{
    /** @return UserData */
    public function fetchUser(): array
    {
        return ['name' => 'Bob', 'email' => 'bob@example.com', 'age' => 25];
    }

    /** @return AliasedStatus */
    public function fetchStatus(): array
    {
        return ['code' => 404, 'label' => 'Not Found'];
    }

    public function demo(): void
    {
        // Try: $this->fetchUser()['     ← offers name, email, age (imported from TypeAliasDemo)
        $user = $this->fetchUser();
        $user['email'];                   // resolves imported UserData → array shape keys

        // Try: $this->fetchStatus()['   ← offers code, label (imported and renamed)
        $status = $this->fetchStatus();
        $status['code'];                  // resolves AliasedStatus → StatusInfo → array shape keys
    }
}


// ── Named Key Destructuring from Array Shapes ───────────────────────────────

class DestructuringShapeDemo
{
    /**
     * @return array{customer: Customer, order: Order, total: float}
     */
    public function getInvoice(): array { return []; }

    public function namedKeyFromMethodReturn(): void
    {
        // Try: $cust->  ← offers email, address (Customer members)
        ['customer' => $cust, 'order' => $ord] = $this->getInvoice();
        $cust->email;                     // Customer from 'customer' key
        $ord->total;                      // Order from 'order' key
    }

    public function namedKeyFromVariable(): void
    {
        /** @var array{user: User, profile: UserProfile, active: bool} $data */
        $data = getUnknownValue();

        // Try: $person->  ← offers getName(), getEmail() (User members)
        ['user' => $person, 'profile' => $prof] = $data;
        $person->getEmail();              // User from 'user' key
        $prof->getDisplayName();          // UserProfile from 'profile' key
    }

    public function positionalFromShape(): void
    {
        /** @var array{User, Address} $pair */
        $pair = getUnknownValue();

        // Try: $second->  ← offers city, format() (Address members)
        [$first, $second] = $pair;
        $first->getEmail();               // User (positional index 0)
        $second->format();                // Address (positional index 1)
    }

    public function listSyntaxNamedKey(): void
    {
        /** @var array{recipe: Recipe, servings: int} $meal */
        $meal = getUnknownValue();

        // Try: $r->  ← offers ingredients (Recipe members)
        list('recipe' => $r) = $meal;
        $r->ingredients;                  // Recipe from 'recipe' key
    }
}


// ── Array Function Type Preservation ────────────────────────────────────────

class ArrayFuncDemo
{
    /** @var list<User> */
    public array $users;

    /** @return list<User> */
    public function getUsers(): array { return []; }

    public function filterPreservesType(): void
    {
        // Try: $active[0]->  ← offers getName(), getEmail() (User members)
        $active = array_filter($this->users, fn(User $u) => $u->getStatus() === Status::Active);
        $active[0]->getName();            // User preserved through array_filter
    }

    public function valuesPreservesType(): void
    {
        $vals = array_values($this->users);
        $vals[0]->getEmail();             // User preserved through array_values
    }

    public function reversePreservesType(): void
    {
        $reversed = array_reverse($this->users);
        $reversed[0]->getName();          // User preserved through array_reverse
    }

    public function slicePreservesType(): void
    {
        $page = array_slice($this->users, 0, 10);
        $page[0]->getEmail();             // User preserved through array_slice
    }

    public function popExtractsElement(): void
    {
        // Try: $last->  ← offers getName(), getEmail() (User members)
        $users = $this->getUsers();
        $last = array_pop($users);
        $last->getName();                 // single User from array_pop

        $first = array_shift($users);
        $first->getEmail();               // single User from array_shift
    }

    public function currentEndReset(): void
    {
        $cur = current($this->users);
        $cur->getName();                  // User from current()

        $last = end($this->users);
        $last->getEmail();                // User from end()
    }

    public function foreachOverFiltered(): void
    {
        // Try: $u->  ← offers getName(), getEmail() (User members)
        foreach (array_filter($this->users, fn(User $u) => true) as $u) {
            $u->getEmail();               // User preserved in foreach
        }
    }

    public function arrayMapFallback(): void
    {
        // When callback has no return type, falls back to input element type
        $mapped = array_map(fn($u) => $u, $this->users);
        $mapped[0]->getName();            // User from array_map fallback
    }
}


// ── Trait insteadof / as Conflict Resolution ────────────────────────────────

/**
 * Demonstrates trait conflict resolution with `insteadof` and `as`.
 *
 * When multiple traits define the same method, PHP requires explicit
 * `insteadof` to pick a winner and `as` to create aliases.
 */
class TraitConflictDemo
{
    use JsonSerializer, XmlSerializer {
        JsonSerializer::serialize insteadof XmlSerializer;
        XmlSerializer::serialize as serializeXml;
        JsonSerializer::serialize as private internalSerialize;
    }

    public function demo(): void
    {
        // Try: $this->  — offers serialize (from JsonSerializer), serializeXml (alias),
        //                  internalSerialize (alias), toJson, toXml, and demo
        $this->serialize();               // JsonSerializer wins via insteadof
        $this->serializeXml();            // XmlSerializer::serialize aliased
        $this->internalSerialize();       // JsonSerializer::serialize aliased as private
        $this->toJson();                  // non-conflicting method from JsonSerializer
        $this->toXml();                   // non-conflicting method from XmlSerializer
    }
}



// ── unset() Tracking ────────────────────────────────────────────────────────

/**
 * Demonstrates that `unset($var)` removes the variable from scope.
 *
 * After `unset($x)`, completion on `$x->` should produce no results.
 * Re-assigning the variable restores its type.
 */
class UnsetDemo
{
    public function basicUnset(): void
    {
        $user = new User('Alice', 'alice@example.com');
        $user->getEmail();                // resolves to User
        unset($user);
        //$user->
        // Try: $user->  — no completions (variable was unset)
    }

    public function reassignAfterUnset(): void
    {
        $item = new User('Bob', 'bob@example.com');
        unset($item);
        $item = new AdminUser('Admin', 'admin@example.com');
        $item->grantPermission('edit');   // resolves to AdminUser
    }

    public function unsetMultiple(): void
    {
        $user = new User('A', 'a@b.com');
        $profile = new UserProfile($user);
        unset($user, $profile);
        // Try: $user->   — no completions
        // Try: $profile-> — no completions
    }

    public function unsetOnlyAffectsTarget(): void
    {
        $user = new User('A', 'a@b.com');
        $profile = new UserProfile($user);
        unset($user);
        $profile->getDisplayName();       // still resolves to UserProfile
    }
}


// ── First-Class Callable Syntax (PHP 8.1) ───────────────────────────────────

/**
 * PHP 8.1 first-class callable syntax creates a Closure from any
 * function or method reference.  The return type of the underlying
 * callable is resolved so that invoking the Closure gives completion
 * on the result.
 */
class FirstClassCallableDemo
{
    public function makeOrder(): Response
    {
        return new Response(200);
    }

    public function demo(): void
    {
        // Function reference: createUser(...) → Closure that returns User
        $fn = createUser(...);
        $fn()->getEmail();                // resolves to User

        // Instance method: $this->makeOrder(...) → Closure returning Response
        $orderFn = $this->makeOrder(...);
        $orderFn()->getStatusCode();      // resolves to Response

        // Static method returning ?self: User::findByEmail(...)
        $finder = User::findByEmail(...);
        $finder()->getName();             // resolves to User

        // Assigned result
        $make = createUser(...);
        $user = $make();
        $user->getProfile();              // resolves to User
    }
}


// ── Array Element Access from Assignments ───────────────────────────────────

class ArrayAccessDemo
{
    /** @return User[] */
    public function getUsers(): array { return []; }

    public function singleLine(): void
    {
        $users = $this->getUsers();
        $users[0]->getName();             // resolves to User
    }

    public function multiLineChain(): void
    {
        $gifts = (new GiftShop())
            ->getGifts();
        $gifts[0]->open();                // resolves to Gift (element of Gift[])
    }

    public function intermediateVariable(): void
    {
        $users = $this->getUsers();
        $first = $users[0];
        $first->getEmail();               // resolves to User via $first = $users[0]
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// ┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓
// ┃  SCAFFOLDING — Supporting definitions below this line.              ┃
// ┃  Everything below exists to support the playground above.           ┃
// ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛

// ── AST Node (template bounds demo) ────────────────────────────────────────

class AstNode
{
    /** @return AstNode|null */
    public function getParent(): ?AstNode { return null; }

    /** @return AstNode[] */
    public function getChildren(): array { return []; }

    public function getType(): string { return ''; }
}

// ── ObjectMapper (method-level @template demo) ──────────────────────────────

class ObjectMapper
{
    /**
     * @template T
     * @param T $item
     * @return TypedCollection<int, T>
     */
    public function wrap(object $item): TypedCollection
    {
        /** @var TypedCollection<int, T> */
        return new TypedCollection();
    }

    /**
     * @template T
     * @param T $item
     * @return T
     */
    public function identity(mixed $item): mixed
    {
        return $item;
    }
}
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

class HtmlReport implements Renderable
{
    public function format(string $template): string
    {
        return '<div>' . $template . '</div>';
    }

    public function __toString(): string
    {
        return $this->format('report');
    }
}

// ─── Traits ─────────────────────────────────────────────────────────────────

trait JsonSerializer {
    public function serialize(): string { return '{}'; }
    public function toJson(): string { return $this->serialize(); }
}

trait XmlSerializer {
    public function serialize(): string { return '<xml/>'; }
    public function toXml(): string { return $this->serialize(); }
}

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

// ─── Multi-line @return & Broken Docblock Recovery ──────────────────────────

/**
 * @template TKey of array-key
 * @template TValue
 */
class FluentCollection
{
    /**
     * Multi-line @return with conditionals inside generics.
     * The lines are joined and the full type is parsed.
     *
     * @template TGroupKey of array-key
     *
     * @param  (callable(TValue, TKey): TGroupKey)|array|string  $groupBy
     * @param  bool  $preserveKeys
     * @return static<
     *  ($groupBy is (array|string)
     *      ? array-key
     *      : TGroupKey),
     *  static<($preserveKeys is true ? TKey : int), TValue>
     * >
     */
    public function groupBy($groupBy, $preserveKeys = false)
    {
    }

    /**
     * Single-line @return — works as before.
     *
     * @template TMapValue
     *
     * @param  callable(TValue, TKey): TMapValue  $callback
     * @return static<TKey, TMapValue>
     */
    public function map(callable $callback)
    {
    }

    /**
     * Multi-line @return with nested generics spanning lines.
     *
     * @return array<
     *   string,
     *   FluentCollection<int, TValue>
     * >
     */
    public function toGroupedArray()
    {
    }

    /**
     * @return static<TKey, TValue>
     */
    public function values()
    {
    }
}

/** @return FluentCollection */
function collect(mixed $value = []): FluentCollection
{
    return new FluentCollection();
}

class BrokenDocRecovery
{
    /**
     * Broken multi-line @return — base `static` is recovered.
     * @return static<
     */
    public function broken(): static
    {
        return $this;
    }

    public function working(): string
    {
        return 'hello';
    }
}

// ─── Foreach over Generic Collection Classes ────────────────────────────────

/**
 * @template TKey of array-key
 * @template-covariant TValue
 * @implements \IteratorAggregate<TKey, TValue>
 */
class BaseCollection implements \IteratorAggregate
{
    /** @return \ArrayIterator<TKey, TValue> */
    public function getIterator(): \ArrayIterator { return new \ArrayIterator([]); }
}

/**
 * @template TKey of array-key
 * @template TModel of Model
 * @extends BaseCollection<TKey, TModel>
 */
class EloquentCollection extends BaseCollection {}

/**
 * @extends EloquentCollection<int, User>
 */
final class UserEloquentCollection extends EloquentCollection {}
