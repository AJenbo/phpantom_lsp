<?php
/**
 * Laravel Demo Assertions
 *
 * Run: php examples/laravel/assertions.php
 *
 * These assertions verify that our assumptions about Laravel's runtime
 * behaviour are correct, so the LSP can model them accurately.
 * Uses only reflection (no database or app boot required).
 */

require_once __DIR__ . '/vendor/autoload.php';

// Boot Eloquent with an in-memory SQLite database
$capsule = new \Illuminate\Database\Capsule\Manager();
$capsule->addConnection([
    'driver'   => 'sqlite',
    'database' => ':memory:',
]);
$capsule->setAsGlobal();
$capsule->bootEloquent();

$passed = 0;
$failed = 0;

function check(string $label, bool $condition): void
{
    global $passed, $failed;
    if ($condition) {
        $passed++;
    } else {
        $failed++;
        echo "FAIL: $label\n";
    }
}

function assertMethodVisibility(string $class, string $method, string $expected): void
{
    $ref = new ReflectionMethod($class, $method);
    $actual = $ref->isPublic() ? 'public' : ($ref->isProtected() ? 'protected' : 'private');
    check("$class::$method() is $expected", $actual === $expected);
}

function assertMethodReturnType(string $class, string $method, string $expected): void
{
    $ref = new ReflectionMethod($class, $method);
    $type = $ref->getReturnType();
    $actual = $type ? $type->__toString() : 'mixed';
    check("$class::$method() returns $expected (got $actual)", $actual === $expected);
}

// ─── Scope vs Model method shadowing ────────────────────────────────────────

// Model::fresh() is public — a subclass CANNOT define a #[Scope] named "fresh"
// because PHP forbids changing the signature of an inherited public method.
// Our demo uses "freshlyBaked" instead.
check(
    'Model::fresh() exists',
    method_exists(\Illuminate\Database\Eloquent\Model::class, 'fresh')
);
assertMethodVisibility(\Illuminate\Database\Eloquent\Model::class, 'fresh', 'public');

// Our Bakery uses "freshlyBaked" to avoid the conflict
check(
    'Bakery::freshlyBaked() exists',
    method_exists(\App\Models\Bakery::class, 'freshlyBaked')
);
assertMethodVisibility(\App\Models\Bakery::class, 'freshlyBaked', 'protected');

// Verify #[Scope] attribute is present on freshlyBaked
$ref = new ReflectionMethod(\App\Models\Bakery::class, 'freshlyBaked');
$attrs = $ref->getAttributes(\Illuminate\Database\Eloquent\Attributes\Scope::class);
check('Bakery::freshlyBaked() has #[Scope] attribute', count($attrs) === 1);

// ─── Convention-based scopes ────────────────────────────────────────────────

// scopeXxx methods are public and accessible via __call as xxx()
check(
    'Bakery::scopeUnbaked() exists',
    method_exists(\App\Models\Bakery::class, 'scopeUnbaked')
);
assertMethodVisibility(\App\Models\Bakery::class, 'scopeUnbaked', 'public');

check(
    'Bakery::scopeTopping() exists',
    method_exists(\App\Models\Bakery::class, 'scopeTopping')
);
assertMethodVisibility(\App\Models\Bakery::class, 'scopeTopping', 'public');

// ─── Relationship methods ───────────────────────────────────────────────────

check(
    'Bakery::baguettes() exists',
    method_exists(\App\Models\Bakery::class, 'baguettes')
);
check(
    'Bakery::headBaker() exists',
    method_exists(\App\Models\Bakery::class, 'headBaker')
);
check(
    'Bakery::masterRecipe() exists',
    method_exists(\App\Models\Bakery::class, 'masterRecipe')
);

// ─── Accessor methods ───────────────────────────────────────────────────────

// Legacy accessor
check(
    'Bakery::getLoafNameAttribute() exists (legacy accessor)',
    method_exists(\App\Models\Bakery::class, 'getLoafNameAttribute')
);

// Modern Attribute accessor
check(
    'Bakery::sprinkle() exists (modern accessor)',
    method_exists(\App\Models\Bakery::class, 'sprinkle')
);

// ─── Runtime scope behaviour ────────────────────────────────────────────────

// Convention-based scopes via __call on instance return Builder
$bakery = new \App\Models\Bakery();
$result = $bakery->unbaked();
check(
    '$bakery->unbaked() returns Builder via __call',
    $result instanceof \Illuminate\Database\Eloquent\Builder
);

$result = $bakery->topping('choc');
check(
    '$bakery->topping("choc") returns Builder via __call',
    $result instanceof \Illuminate\Database\Eloquent\Builder
);

// #[Scope] attribute scopes are available on the query builder
$result = \App\Models\Bakery::query()->freshlyBaked();
check(
    'Bakery::query()->freshlyBaked() returns Builder',
    $result instanceof \Illuminate\Database\Eloquent\Builder
);

// Static scope forwarding
$result = \App\Models\Bakery::where('flour', 'rye');
check(
    'Bakery::where() returns Builder',
    $result instanceof \Illuminate\Database\Eloquent\Builder
);

// Model::fresh() on instance (non-existing model returns null)
$result = $bakery->fresh();
check(
    '$bakery->fresh() returns null (Model::fresh on non-persisted)',
    $result === null
);

// ─── Summary ────────────────────────────────────────────────────────────────

echo "\n";
if ($failed === 0) {
    echo "\033[32m✓ All $passed assertions passed.\033[0m\n";
} else {
    echo "\033[31m✗ $failed failed, $passed passed.\033[0m\n";
    exit(1);
}
