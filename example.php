<?php

/**
 * PHPantomLSP — Comprehensive Feature Demo
 *
 * This file showcases most of the features supported by PHPantomLSP.
 * Use it to test completion, go-to-definition, type resolution, and more.
 */

namespace Demo;

use Demo\Contracts\Renderable;
use Demo\Concerns\HasTimestamps;
use Demo\Enums\Status;

// ─── Interfaces ─────────────────────────────────────────────────────────────

interface Stringable
{
    public function toString(): string;
}

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
        $this->updatedAt = date("Y-m-d H:i:s");
    }
}

/**
 * @property string $slug
 */
trait HasSlug
{
    public function generateSlug(string $value): string
    {
        return strtolower(str_replace(" ", "-", $value));
    }
}

// ─── Enums ──────────────────────────────────────────────────────────────────

enum Status: string
{
    case Active = "active";
    case Inactive = "inactive";
    case Pending = "pending";

    public function label(): string
    {
        return match ($this) {
            self::Active => "Active",
            self::Inactive => "Inactive",
            self::Pending => "Pending",
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

// ─── Base / Parent Class ────────────────────────────────────────────────────

/**
 * @property string $magicName
 * @method static static create(array $attributes)
 * @mixin HasTimestamps
 */
abstract class Model
{
    protected int $id;

    /** @var string */
    protected string $table = "";

    public const string CONNECTION = "default";
    protected const int PER_PAGE = 15;
    private const string INTERNAL_KEY = "__model__";

    public function __construct(
        protected string $name = "",
        public readonly string $uuid = "",
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
    public static function make(string $name = ""): static
    {
        return new static($name);
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
class User extends Model implements Renderable
{
    use HasTimestamps;
    use HasSlug;

    public string $email;
    protected Status $status;
    private array $roles = [];

    public static string $defaultRole = "user";
    public static int $count = 0;

    public const string TYPE_ADMIN = "admin";
    public const string TYPE_USER = "user";

    public function __construct(
        string $name,
        string $email,
        private readonly string $password = "",
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
            "id" => $this->getId(),
            "name" => $this->getName(),
            "email" => $this->email,
            "status" => $this->status->value,
        ];
    }

    public function toString(): string
    {
        return $this->getName();
    }

    public function format(string $template): string
    {
        return str_replace("{name}", $this->getName(), $template);
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
    public string $bio = "";
    public string $avatarUrl = "";

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
        return $this->user->getName() . " (" . $this->user->getEmail() . ")";
    }
}

// ─── Child Class (parent:: resolution) ──────────────────────────────────────

class AdminUser extends User
{
    /** @var string[] */
    private array $permissions = [];

    public function __construct(string $name, string $email)
    {
        // parent:: completion shows parent's non-private members
        parent::__construct($name, $email);
    }

    public function toArray(): array
    {
        $base = parent::toArray();
        $base["permissions"] = $this->permissions;
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
     */
    public function make(?string $abstract = null): mixed
    {
        if ($abstract === null) {
            return $this;
        }
        return $this->bindings[$abstract] ?? null;
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
    return new User("test", "test@example.com");
}

// ─── Usage Examples ─────────────────────────────────────────────────────────

// Instance member completion via ->
$user = new User("Alice", "alice@example.com");
$user->getEmail(); // Completion: methods on User
$user->email; // Completion: properties on User
$user->getCreatedAt(); // Completion: methods from HasTimestamps trait
$user->generateSlug("Test"); // Completion: methods from HasSlug trait

// Static member completion via ::
User::$defaultRole; // Completion: static properties
User::TYPE_ADMIN; // Completion: class constants
User::findByEmail("a@b.c"); // Completion: static methods
User::make("Bob"); // Completion: inherited static methods from Model

// Enum case completion via ::
Status::Active; // Completion: enum cases
Status::Active->label(); // Completion: methods on enum

// parent:: completion (in AdminUser context)
// parent::toArray()         — shows inherited non-private methods and constants

// self / static / $this resolution
// Inside User class:
//   $this->getName()        — resolves User->getName()
//   self::TYPE_ADMIN        — resolves User::TYPE_ADMIN
//   static::find(1)         — resolves to the calling class

// Method call chaining
$user->setName("Bob")->setStatus(Status::Active)->getEmail();

// Property chain resolution: $this->prop->method()
$profile = $user->getProfile();
$profile->getUser()->getEmail(); // Chain through UserProfile->User

// Chaining directly
$user->getProfile()->getDisplayName();

// Static method return type -> chaining
$made = User::make("Charlie");
$made->getEmail(); // Resolves static return type

// Function return type resolution
$u = createUser("Dana", "dana@example.com");
$u->getName(); // Resolves createUser() return type -> User

// Constructor promoted properties (readonly)
$user->age; // public promoted property
$user->uuid; // readonly promoted property from Model

// new expression -> chaining (PHP 8.4+ / parenthesized)
new User("Eve", "eve@example.com")->getEmail();

// Variable type inference from assignments
$admin = new AdminUser("Frank", "frank@example.com");
$admin->grantPermission("delete");
$admin->getCreatedAt(); // Inherited via trait from User

// Union types — ambiguous variable across conditional branches
if (rand(0, 1)) {
    $ambiguous = new User("X", "x@example.com");
} else {
    $ambiguous = new AdminUser("Y", "y@example.com");
}
$ambiguous->getName(); // Both User and AdminUser have getName()

// Union return type resolution
$found = findOrFail(1);
$found->getName(); // User|AdminUser — union completion

// Go-to-definition targets:
// - Hover over `User` to jump to its class definition
// - Hover over `getEmail` to jump to its method definition
// - Hover over `$email` property to jump to its definition
// - Hover over `TYPE_ADMIN` to jump to its constant definition
// - Hover over `Renderable` to jump to the interface definition
// - Hover over `HasTimestamps` to jump to the trait definition
// - Hover over `Status` to jump to the enum definition
// - Hover over `Model` to jump to the parent class definition
// - Hover over `createUser` to jump to the standalone function definition

// PHPDoc @property and @method (magic members)
// $user->displayName           — from @property tag on User
// $user->hasPermission('edit') — from @method tag on User

// @mixin resolution
// Model has @mixin HasTimestamps — mixin members appear in completion

// @var type override
/** @var User $typed */
$typed = getUnknownValue();
$typed->getEmail(); // Type comes from @var docblock

// Inline @var docblock for variable type hints
/** @var AdminUser $inlineTyped */
$inlineTyped = someFactory();
$inlineTyped->grantPermission("write");

// Null-safe operator chaining
$maybeUser = User::find(1);
$maybeUser?->getProfile()?->getDisplayName();

// Visibility filtering:
// - Arrow completion shows only accessible members
// - parent:: excludes private members
// - Static :: shows only static members, constants, and enum cases

// Namespace and use statement resolution:
// - Fully qualified: \Demo\User
// - Imported via use: User (resolved from `use Demo\User`)
// - Relative in same namespace: User (resolved from current namespace Demo)

// Response with union type properties
$response = new Response(200, ["data" => "ok"]);
$response->getStatusCode(); // Returns string|int
$response->getBody(); // Returns string|array|null

// Container with conditional return type
$container = new Container();
$resolvedUser = $container->make(User::class);
$resolvedUser->getEmail(); // Conditional return resolves User from class-string

// Standalone function with conditional return
$appContainer = app(); // Returns Container (no argument = else branch)
$appUser = app(User::class); // Returns User (class-string argument = then branch)
