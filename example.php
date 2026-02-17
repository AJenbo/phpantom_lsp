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
