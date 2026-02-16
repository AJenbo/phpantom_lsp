<?php

/**
 * PHPantomLSP — Comprehensive Feature Demo
 *
 * This file showcases most of the features supported by PHPantomLSP.
 * Use it to test completion, go-to-definition, type resolution, and more.
 */

namespace Demo;

use Exception;
use Stringable;

// ─── Interfaces ─────────────────────────────────────────────────────────────

/**
 * @method string render()
 * @property-read string $output
 */
interface Renderable extends Stringable
{
    public function format(string $template): string;
}

// ─── Traits ─────────────────────────────────────────────────────────────────

trait HasTimestamps
{
    protected ?string $createdAt = null;
    protected ?string $updatedAt = null;

    public function getCreatedAt(): ?string
    {
        return $this->createdAt;
    }

    public function setCreatedAt(string $date): static
    {
        $this->createdAt = $date;
        return $this;
    }

    public function touch(): void
    {
        $this->updatedAt = date('Y-m-d H:i:s');
    }
}

/**
 * @property string $slug
 */
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

// ─── Base / Parent Class ────────────────────────────────────────────────────

class Builder
{
    /**
     * @return static
     */
    public static function query(): self
    {
        return new static();
    }
}

/**
 * @property string $magicName
 * @method static static create(array $attributes)
 * @mixin Builder
 */
abstract class Model
{
    protected int $id;

    /** @var string */
    protected $table = '';

    public const string CONNECTION = 'default';
    protected const int PER_PAGE = 15;
    private const string INTERNAL_KEY = '__model__';

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

    /**
     * @return static
     */
    public function setName(string $name): static
    {
        $this->name = $name;
        return $this;
    }

    public static function find(int $id): ?static
    {
        return null;
    }

    /**
     * @return static
     */
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

// ─── Concrete Class with Inheritance, Traits, Interfaces ────────────────────

/**
 * Represents a user in the system.
 *
 * @property string $displayName
 * @property-read bool $isAdmin
 * @method bool hasPermission(string $permission)
 */
class User extends Model implements Renderable // Press go-to on `Model` or `Renderable` to jump to there definitions
{
    use HasTimestamps; // Press go-to on `HasTimestamps` to jump to the trait definition
    use HasSlug;

    public string $email;
    protected Status $status; // Press go-to on `Status` to jump to the enum definition
    private array $roles = [];

    public static string $defaultRole = 'user';
    public static int $count = 0;

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
        self::$count++;
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

    /**
     * @param string ...$roles
     */
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

    public function toString(): string
    {
        return $this->getName();
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

    private function secretInternalMethod(): void
    {
        // Not visible via parent:: or from outside
    }
}

// ─── Another Class for Chaining / Cross-Class Resolution ────────────────────

class UserProfile
{
    public string $bio = '';
    public string $avatarUrl = '';

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

// ─── Child Class (parent:: resolution) ──────────────────────────────────────

final class AdminUser extends User
{
    /** @var string[] */
    private array $permissions = [];

    public function __construct(string $name, string $email)
    {
        parent::__construct($name, $email);
        // parent:: shows inherited non-private methods and constants
    }

    public function toArray(): array
    {
        $base = parent::toArray();
        $base['permissions'] = $this->permissions;
        return $base;
    }

    public function grantPermission(string $permission): void
    {
        $this->permissions[] = $permission;
    }
}

// ─── Class with Union Types and Nullable ────────────────────────────────────

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

// ─── Container with PHPStan Conditional Return Types ────────────────────────

class Container
{
    /** @var array<string, object> */
    private array $bindings = [];

    /**
     * Resolve an item from the container.
     *
     * @template TClass
     * @param string|null $abstract
     * @return ($abstract is class-string<TClass> ? TClass : mixed)
     * @throws Exception
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
        return 404;
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

    if ($abstract !== null) {
        return $container->make($abstract);
    }

    return $container;
}

function createUser(string $name, string $email): User
{
    return new User($name, $email);
}

function findOrFail(int $id): User|AdminUser
{
    return new User('test', 'test@example.com');
}

function getUnknownValue()
{
    return new AdminUser('', '');
}

// ─── Custom Assert Functions (@phpstan-assert) ──────────────────────────────

/**
 * @phpstan-assert User $value
 */
function assertUser($value): void
{
    if (!$value instanceof User) {
        throw new \InvalidArgumentException('Expected User');
    }
}

/**
 * @phpstan-assert-if-true AdminUser $value
 */
function isAdmin($value): bool
{
    return $value instanceof AdminUser;
}

/**
 * @phpstan-assert-if-false AdminUser $value
 */
function isRegularUser($value): bool
{
    return !$value instanceof AdminUser;
}

// ─── Usage Examples ─────────────────────────────────────────────────────────

// Instance member completion via ->
$user = new User('Alice', 'alice@example.com');
$user->getEmail(); // Completion: methods on User
$user->email; // Completion: properties on User
$user->getCreatedAt(); // Completion: methods from HasTimestamps trait
$user->generateSlug('Test'); // Completion: methods from HasSlug trait

// Static member completion via ::
User::$defaultRole; // Completion: static properties
// Press go-to on `TYPE_ADMIN` to jump to its constant definition
User::TYPE_ADMIN; // Completion: class constants
User::findByEmail('a@b.c'); // Completion: static methods
User::make('Bob'); // Completion: inherited static methods from Model

// Enum case completion via ::
Status::Active; // Completion: enum cases
Status::Active->label(); // Completion: methods on enum


// self / static / $this resolution
// Inside User class:
//   $this->getName()        — resolves User->getName()
//   self::TYPE_ADMIN        — resolves User::TYPE_ADMIN
//   static::find(1)         — resolves to the calling class

// Method call chaining
$user->setName('Bob')->setStatus(Status::Active)->getEmail();

// Property chain resolution: $this->prop->method()
$profile = $user->getProfile();
$profile->getUser()->getEmail(); // Chain through UserProfile->User

// Chaining directly
$user->getProfile()->getDisplayName();

// Static method return type -> chaining
$made = User::make('Charlie');
$made->getEmail(); // Resolves static return type

// Function return type resolution
$u = createUser('Dana', 'dana@example.com'); // Press go-to on `createUser` to jump to the function definition
$u->getName(); // Resolves createUser() return type -> User

// Constructor promoted properties (readonly)
$user->age; // public promoted property
$user->uuid; // readonly promoted property from Model

// new expression -> chaining (PHP 8.4+ / parenthesized)
new User('Eve', 'eve@example.com')->getEmail();

// Variable type inference from assignments
$admin = new AdminUser('Frank', 'frank@example.com');
$admin->grantPermission('delete');
$admin->getCreatedAt(); // Inherited via trait from User

// Union types — ambiguous variable across conditional branches
if (rand(0, 1)) {
    $ambiguous = new Container();
} else {
    $ambiguous = new AdminUser('Y', 'y@example.com');
}
$ambiguous->getStatus(); // Both Container and AdminUser have getStatus()
if ($ambiguous instanceof AdminUser) {
    $ambiguous->grantPermission('review');
} else {
    $ambiguous->bind($ambiguous::class, $ambiguous);
}

// Union return type resolution
$found = findOrFail(1);
$found->getName(); // User|AdminUser — union completion
if ($found instanceof User) {
    $found->addRoles('triage'); // AdminUser extends User
}

// Narrow using assert()
$asserted = getUnknownValue(1);
assert($asserted instanceof User);
$asserted->addRoles();

// ─── Type Narrowing ────────────────────────────────────────────────────────
//
// PHPantomLSP narrows union types based on runtime checks so that
// completion only shows the relevant members.

// while-loop instanceof narrowing
// Inside the loop body, $found is narrowed to User because the condition
// guarantees it on every iteration.
$found2 = getUnknownValue(1);
while ($found2 instanceof User) {
    $found2->getEmail(); // ✅ User members only
    break;
}

// is_a() — treated the same as instanceof
$pet = getUnknownValue(1);
if (is_a($pet, AdminUser::class)) {
    $pet->grantPermission('edit'); // ✅ narrowed to AdminUser
}

// Negated is_a() — excludes the checked class
$pet2 = findOrFail(1); // User|AdminUser
if (!is_a($pet2, AdminUser::class)) {
    $pet2->getEmail(); // ✅ narrowed to User (AdminUser excluded)
}

// assert() with is_a() — unconditional narrowing
$pet3 = getUnknownValue(1);
assert(is_a($pet3, AdminUser::class));
$pet3->grantPermission('delete'); // ✅ narrowed to AdminUser

// get_class() === ClassName::class — exact class identity
$entity = findOrFail(1); // User|AdminUser
if (get_class($entity) === User::class) {
    $entity->getEmail(); // ✅ narrowed to exactly User
}

// $var::class === ClassName::class — modern exact class identity (PHP 8.0+)
$entity2 = findOrFail(1); // User|AdminUser
if ($entity2::class === AdminUser::class) {
    $entity2->grantPermission('manage'); // ✅ narrowed to AdminUser
}

// Negated class identity — excludes the matched class
$entity3 = findOrFail(1); // User|AdminUser
if (get_class($entity3) !== User::class) {
    $entity3->grantPermission('review'); // ✅ narrowed to AdminUser (User excluded)
}

// Reversed operand order also works
$entity4 = findOrFail(1); // User|AdminUser
if (User::class === $entity4::class) {
    $entity4->getEmail(); // ✅ narrowed to User
}

// match(true) with instanceof — narrowing inside match arm bodies
$value = getUnknownValue(1);
$result = match (true) {
    $value instanceof AdminUser => $value->grantPermission('approve'), // ✅ narrowed to AdminUser
    default => null,
};

// match(true) with is_a() — also works
$value2 = getUnknownValue(1);
$result2 = match (true) {
    is_a($value2, AdminUser::class) => $value2->grantPermission('deploy'), // ✅ narrowed to AdminUser
    default => null,
};

// Else-branch narrowing — the inverse type is used
$check = findOrFail(1); // User|AdminUser
if ($check instanceof AdminUser) {
    $check->grantPermission('sudo'); // ✅ narrowed to AdminUser
} else {
    $check->getEmail(); // ✅ narrowed to User (AdminUser excluded)
}

// ─── Custom Assert Narrowing (@phpstan-assert) ─────────────────────────────
//
// Functions annotated with @phpstan-assert / @psalm-assert act as custom
// type guards — PHPantomLSP reads the annotation and narrows accordingly.

// @phpstan-assert — unconditional narrowing after the call
$unknown = getUnknownValue(1);
assertUser($unknown);
$unknown->getEmail(); // ✅ narrowed to User (assertUser guarantees it)

// @phpstan-assert-if-true — narrows inside the if-body when function returns true
$maybe = findOrFail(1); // User|AdminUser
if (isAdmin($maybe)) {
    $maybe->grantPermission('sudo'); // ✅ narrowed to AdminUser
} else {
    $maybe->getEmail(); // ✅ narrowed to User (AdminUser excluded)
}

// @phpstan-assert-if-false — narrows in the else-body when function returns false
// and excludes in the then-body (function returned true → AdminUser is excluded)
$maybe2 = findOrFail(1); // User|AdminUser
if (isRegularUser($maybe2)) {
    $maybe2->getEmail(); // ✅ narrowed to User (AdminUser excluded)
} else {
    $maybe2->grantPermission('deploy'); // ✅ narrowed to AdminUser
}

// Negated condition flips which branch gets the narrowing
$maybe3 = findOrFail(1); // User|AdminUser
if (!isAdmin($maybe3)) {
    $maybe3->getEmail(); // ✅ narrowed to User (function returned false → AdminUser excluded)
} else {
    $maybe3->grantPermission('edit'); // ✅ narrowed to AdminUser (function returned true)
}

// @phpstan-assert-if-true in a while-loop condition
$cursor = getUnknownValue(1);
while (isAdmin($cursor)) {
    $cursor->grantPermission('loop'); // ✅ narrowed to AdminUser inside loop
    break;
}

// @mixin resolution
// Model has @mixin Builder — mixin members appear in completion
$query = User::query(); // Press go-to on `query()` to jump to its class definition

// @var type override
/** @var User $typed */
$typed = getUnknownValue();
// Press go-to on `getEmail()` to jump to its method definition
$email = $typed->getEmail(); // Type comes from @var docblock
echo $email; // Press go-to on `$email` to jump to its assignment

// Inline @var docblock for variable type hints
/** @var AdminUser $inlineTyped */
$inlineTyped = getUnknownValue(AdminUser::class);
$inlineTyped->grantPermission('write');

// Null-safe operator chaining
$maybeUser = User::find(1); // Press go-to on `User` to jump to its class definition
$maybeUser?->getProfile()?->getDisplayName();

// PHPDoc @property and @method (magic members)
echo $maybeUser->displayName; // from @property tag on User
$maybeUser->hasPermission('edit'); // from @method tag on User

// Visibility filtering:
// - $obj-> shows only accessible dynamic members
// - MyClass:: shows only accessible static members

// Namespace and use statement resolution:
// - Fully qualified: \Demo\User
// - Imported via use: User (resolved from `use Demo\User`)
// - Relative in same namespace: User (resolved from current namespace Demo)

// Response with union type properties
$response = new Response(200, ['data' => 'ok']);
$response->getStatusCode(); // Returns string|int
$response->getBody(); // Returns string|array|null

// Container with conditional return type
$container = new Container();
$container->bind(User::class, new User('', ''));
$resolvedUser = $container->make(User::class);
$resolvedUser->getEmail(); // Conditional return resolves User from class-string

// Standalone function with conditional return
$appContainer = app(); // Returns Container (no argument = else branch)
$appUser = app(User::class); // Returns User (class-string argument = then branch)
