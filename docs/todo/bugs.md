# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Complexity** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

All entries below come from the 2026-08-15 sample-project sweep (46
diagnostics across ten projects). 45 were false positives, filed here as
**B49–B74**; the one genuine finding (an unguarded `SimpleXMLElement`
child lookup) was patched directly in `luxplus-backoffice`'s sample
source.

## Crashes

No outstanding items.

## Type comparison

### B57. A typed class constant loses its literal value

**Impact: Medium · Complexity: Medium**

```php
final class Json
{
    private const int DEFAULT_OPTIONS = JSON_HEX_TAG | JSON_THROW_ON_ERROR;

    public static function encode(mixed $value, int $options = 0): string
    {
        return json_encode($value, $options | self::DEFAULT_OPTIONS);   // reported: got string|false
    }
}
```

`json_encode()`'s return type narrows to plain `string` (dropping
`false`) when the resolved `$flags` argument provably has
`JSON_THROW_ON_ERROR` set — the flag makes a failure throw instead of
returning `false`. That bitwise fold works when `DEFAULT_OPTIONS` is a
plain untyped `const`, but PHP 8.3's typed class constants
(`const int NAME = expr`) resolve to the declared type (`int`) instead
of the constant-folded literal value of `expr`, so the downstream
bitwise-OR reasoning never sees `JSON_THROW_ON_ERROR` and falls back to
the unrefined `string|false`.

**Fix:** when reading a typed class constant's value for constant
folding, evaluate its initializer expression the same way an untyped
constant's is evaluated, rather than substituting the declared type.

### B67. `str_replace()`'s return type ignores the `$subject` argument

**Impact: Low-Medium · Complexity: Medium**

```php
function clean(?string $error): ?string
{
    return str_replace('Error: ', '', $error ?? '');   // reported: got array<array-key, string>|string
}
```

`str_replace($search, $replace, $subject)` only returns an `array` when
`$subject` is an array; for a `string` subject it always returns
`string`. The stub reports the full overloaded union regardless of the
argument actually passed.

**Fix:** add a dynamic-return-type rule (alongside the existing
argument-dependent-return-type mechanism) that resolves `str_replace()`
and `str_ireplace()`'s return type from `$subject`'s resolved type:
`string` in → `string` out, `array` in → `array` out.

## Narrowing

### B49. Short-circuit `&&`/`||` narrowing does not reach the second operand

**Impact: High · Complexity: High**

```php
class Holder
{
    /** @var null|array<string, string> */
    public ?array $data = null;

    public function field(string $key): ?string
    {
        return is_array($this->data) && array_key_exists($key, $this->data)  // reported: got null|array<string,string>
            ? $this->data[$key]
            : null;
    }
}

function useIt(?int $count, Service $s): bool
{
    return null === $count || $s->hasMet($count);   // reported: got ?int
}
```

Guard-clause narrowing (`if ($x === null) { return; }` followed by
later code) already works. The gap is narrower: within a single
short-circuit expression, the left operand's truthy/falsy proof does
not narrow the type used to evaluate the right operand, and the
narrowing that does apply to plain local variables is not applied to a
property access (`$this->data`) at all.

**Fix:** apply the same type-specifier narrowing used for `if`
conditions to the right operand of `&&`/`||` while evaluating it, and
extend the narrowing target from "plain variable" to any narrowable
subject expression (property access included).

### B50. Ternary-branch narrowing is not applied to property/array-element reads

**Impact: Medium · Complexity: Medium-High**

```php
/** @var array<string, string>|'{}' $raw not quite — see real shape below */
function decode(array|string $raw): void
{
    $value = isset($raw['x']) && is_string($raw['x']) ? $raw['x'] : '{}';
    json_decode($value, true);   // fine: $value is string
}

function display(?string $countryName): string
{
    return $countryName ? strtoupper($countryName) : '';   // reported: got ?string
}
```

`is_string($x) ? $x : default` and `$x ? f($x) : default` both narrow
correctly when `$x` is a plain local variable, but not when `$x` is a
property or array-element read — the same subject-expression gap as
B49, manifesting inside a ternary condition instead of `&&`/`||`.

**Fix:** once B49's narrowing-target extension lands, verify the
ternary condition evaluator reuses it for both the `is_string`/`is_array`
guard form and the plain truthy form.

### B51. `instanceof` does not eliminate a class from a union in the guarded branch

**Impact: High · Complexity: High**

```php
function clip(Decimal|float $value): string
{
    if ($value instanceof Decimal) {
        return $value->format();
    }
    return number_format($value, 2);   // reported: got Decimal|float
}

function imgix(Image|string $imgix): string
{
    if (!$imgix instanceof Image) {
        $imgix = new Image($imgix);   // reported: got Image|string
    }
    return $imgix->url();
}
```

An `instanceof` check on a union-typed value should remove the checked
class from the union on the branch where the check fails — whether that
branch is a separate `else` or the body of a negated
`!$x instanceof T` check. Neither form narrows today; both leave the
full pre-check union.

**Fix:** extend the existing `instanceof` narrowing (which already
handles "rule out the array half of a union", per a prior fix) to also
subtract the checked class from a union on the negative branch,
covering both `if/else` and `if (!...)`.

### B52. A variable reassigned inside a guard that catches its "bad" value is not merged as excluding that value

**Impact: Medium · Complexity: High**

```php
function normalize(): string
{
    $value = mb_strrchr('x', '\\');   // string|false
    if (!$value) {
        $value = 'fallback';
    }
    return $value;   // reported: got string|false
}

function toArray(array|Status $status): array
{
    if (!is_array($status)) {
        $status = [$status];
    }
    foreach ($status as $s) {
        acceptsStatus($s);   // reported: got Status|array<Status>
    }
}
```

Both examples reassign a variable inside the branch that detects its
unwanted type (`false`, or "not yet an array"), replacing it with a
value of the wanted type. After the `if`, every path should agree on the
wanted type: the branch was either skipped (meaning the value already
had the wanted type) or taken (meaning it was just reassigned to it).
The merge instead keeps the original union.

**Fix:** when a branch's only effect is reassigning a variable, and the
branch condition is the negation of (or implies) the variable having a
specific type, the post-merge type should be the reassigned type joined
with the type the variable already had on the skipped path — not the
pre-branch union.

### B53. A repeated identical no-argument call is not recognized as consistent within the same truthy-guarded scope

**Impact: Low-Medium · Complexity: Medium-High**

```php
function currentUser(): ?User { /* ... */ }

function show(): void
{
    if (currentUser()) {
        render(currentUser());   // reported: got ?User
    }
}
```

PHPStan tracks expression types by their normalized form, not just by
variable, so a second call to the same pure/no-argument expression
inside a scope already narrowed by the first call's truthy check is
treated as having the same narrowed type. Calling `currentUser()` twice
in a row (once in the `if`, once inside its body) is a common pattern
this relies on.

**Fix:** when narrowing a condition's subject expression, key the
narrowing by the expression's normalized form (as the resolver cache
already does for chain resolution) rather than only by variable name,
so an identical repeated call inside the guarded scope picks up the same
narrowed type.

### B54. A nullsafe-chain guard does not narrow a later plain dereference of the same chain

**Impact: Medium · Complexity: High**

```php
function process(?Agreement $agreement): void
{
    $period = $agreement?->latestPeriod();
    if (!$period instanceof Period) {
        return;
    }
    accept($agreement);   // reported: got ?Agreement — but $agreement can't be null here
}
```

If `$agreement` were `null`, the nullsafe chain `$agreement?->latestPeriod()`
short-circuits to `null`, and the guard above would have returned. So by
the time execution reaches `accept($agreement)`, `$agreement` is
provably non-null — but nothing propagates that proof back from the
chain's result to the chain's own subject.

**Fix:** when a nullsafe chain's result is proven non-null (by a guard,
an `instanceof` check, or a strict-equality comparison against a
non-null value), treat every nullsafe (`?->`) link in that chain as
proven non-null too, and narrow the base expression accordingly.

### B55. A ternary's self-referencing true branch loses truthy narrowing when compiled from a Blade component attribute

**Impact: Low · Complexity: Medium-High**

```blade
<x-img :alt="$article->image_alt_text ? $article->image_alt_text : $article->title" />
```

`$x ? $x : $y` narrows the true branch to a non-falsy `$x` when `$x` is
a plain variable or a real declared property, and this reproduces
correctly in an ordinary function call. It stops narrowing specifically
when the repeated expression is an Eloquent model's virtual
(`@property`-declared) member read inside a Blade component attribute
expression — the same source line, compiled outside a component
attribute, does not trip this.

**Fix:** trace how Blade component attributes compile virtual-member
ternary expressions (`bladeToPhp`/component-attribute lowering) and
confirm the compiled form still carries the same subject expression the
truthy-narrowing pass keys on; a compilation step that rewrites or
re-wraps the expression would explain the mismatch with the
non-Blade case.

### B62. PHPUnit's `assertNotNull`/`assertNotFalse` are not recognized as narrowing assertions

**Impact: Medium · Complexity: Medium**

```php
/** @var null|array{categories: list<array{id: int}>} $section */
$section = collect($data)->firstWhere('id', $id);
self::assertNotNull($section);
self::assertCount(1, $section['categories']);   // reported: got null|list<...>
```

`assert()` and `@phpstan-assert` tags already narrow. `phpstan-phpunit`
maps `Assert::assertNotNull()`/`assertNotFalse()` (and their static
`self::`/`static::` call forms) to the same `!== null` / `!== false`
type-specification `assert()` gets, so real PHPStan narrows the
variable for the rest of the test method. PHPantom doesn't recognize
these two PHPUnit methods as assertion functions at all.

**Fix:** add `PHPUnit\Framework\Assert::assertNotNull` and
`::assertNotFalse` (matched by unqualified name, since tests call them
as `self::`/`static::`/`$this->`) to the assertion-narrowing table
already used for `assert()`, applying `!== null` / `!== false`
narrowing to their first argument for the rest of the enclosing scope.

### B63. `assertInstanceOf`-narrowed return types leak across unrelated call sites of a shared helper

**Impact: Medium · Complexity: High**

```php
private function makeMock(string $class): MockObject { /* ... */ }

protected function getMethodMock(): MethodNode&MockObject
{
    $node = $this->makeMock(MethodNode::class);
    static::assertInstanceOf(MethodNode::class, $node);
    return $node;   // reported: got (FunctionNode|MethodNode)&MockObject
}

protected function createFunctionMock(): FunctionNode&MockObject
{
    $node = $this->makeMock(FunctionNode::class);
    static::assertInstanceOf(FunctionNode::class, $node);
    return $node;   // reported: got (FunctionNode|MethodNode)&MockObject
}
```

`makeMock()`'s own return type is the wide `MockObject`. Each caller
narrows the local it gets back with its own `assertInstanceOf()` before
returning — that narrowing is call-site-local and should stay local. The
resolved type instead looks like a union of every class ever proven at
*any* call site of the shared helper, as though the assertions from
`getMethodMock()` and `createFunctionMock()` were merged into one fact
about `makeMock()` itself.

**Fix:** this looks like the per-call-site narrowed type is being cached
or merged keyed by the *callee* (`makeMock`) rather than by the call
expression's position; confirm the assertion-narrowing cache is keyed
per call site, not per shared-helper declaration.

### B64. A `@template` bound that includes `null` is dropped by the call site

**Impact: Low-Medium · Complexity: Medium-High**

```php
/**
 * @template TDate of \DateTimeInterface|\Carbon\Carbon|string|null
 * @param TDate $date
 */
function travelTo(mixed $date): void {}

travelTo(Carbon::create(2024, 6, 15));   // reported: expects Carbon, got ?Carbon
```

`Carbon::create()` is genuinely nullable (invalid date components can
yield `null`), and the callee's own `@template` bound explicitly
includes `null` in its type list. The argument-compatibility check
against the resolved template bound drops the `|null` arm somewhere
before comparing, reporting a plain `Carbon` requirement that
contradicts the callee's own declared bound.

**Fix:** when resolving a `@template ... of A|B|null` bound for an
argument-compatibility check, confirm the full union (all three
members) is what gets compared against, not a narrowed/truncated
version of it.

### B70. `is_array()` does not narrow a union-typed parameter before an array-dimension fetch

**Impact: Medium · Complexity: High**

```php
/** @param array{args: array<int, string>, message: string}|string $violationMessage */
function __construct(array|string $violationMessage)
{
    if (is_array($violationMessage)) {
        $this->args = $violationMessage['args'];   // reported: got array<int,string>|string
    }
}
```

Inside `if (is_array($violationMessage))`, the parameter should be
narrowed to its `array{...}` shape before the array-dimension fetch
runs, giving `array<int, string>` for `['args']`. The fetch instead
appears to read against the pre-narrowed `array|string` union.

**Fix:** confirm `is_array()` guard narrowing is applied to the subject
of an array-dimension access, not just to later assignments of the bare
variable — this is the same "subject expression, not just variable"
gap as B49/B50, applied to a constructor-promoted/union-typed
parameter instead of a property.

### B71. A `continue`-discarded falsy union member is not carried into a variable the value is copied into

**Impact: Low-Medium · Complexity: High**

```php
/** @return array{0: false|string, 1: Country|false} */
function columnValues(string $key): array { /* ... */ }

foreach ($keys as $key) {
    [$dbCol, $newMarket] = columnValues($key);
    if (!$dbCol || !$newMarket) {
        continue;
    }
    if (!$market) {
        $market = $newMarket;
    }
    // ... later ...
    updatePrice($productId, $market, $row);   // reported: expects Country, got Country|false
}
```

The `continue` on `!$newMarket` rules out `false` for the rest of the
loop body on every path that doesn't skip to the next iteration. The
assignment `$market = $newMarket` two lines later should therefore only
ever store a `Country`, but the `false` variant survives into `$market`'s
tracked type.

**Fix:** when a `continue`/`break`/`return` guard clause rules out a
value for the rest of the current iteration, apply that narrowing to
the guarded variable before any subsequent assignment reads from it
in the same iteration, not just at direct uses of the original variable.

### B72. `!== null` narrowing on a `@var`-annotated variable is not applied before a builtin call

**Impact: Low-Medium · Complexity: Medium-High**

```php
/** @var null|list<array{version: string}> $cached */
$cached = Cache::get(self::CACHE_KEY);
if ($cached !== null) {
    return array_slice($cached, 0, $limit);   // reported: got null|list<array{version: string}>
}
```

The inline `@var` annotation sets `$cached`'s type to
`null|list<array{...}>`; the immediately following `$cached !== null`
guard should strip the `null` arm for the rest of the `if` body, same as
it would for a plain assignment without the annotation. It doesn't, and
the un-narrowed annotated type reaches `array_slice()`.

**Fix:** check whether inline `@var`-sourced types go through the same
narrowing pass as inferred types, or whether the annotation is
re-applied after narrowing runs (which would explain the guard being
overwritten rather than skipped).

### B74. `Scope::isInClass()` paired with `getClassReflection()` is not recognized as an assert-if-true pair

**Impact: Low · Complexity: Medium**

```php
if ($scope->isInClass()) {
    $reflection = $scope->getClassReflection();   // reported: got ?ClassReflection
    use($reflection);
}
```

`getClassReflection(): ?ClassReflection` is genuinely nullable in
general, but PHPStan's own codebase (and any PHPStan extension, like
Bladestan, built against it) treats a preceding `isInClass(): bool ===
true` check as proof the paired call returns non-null — the same
assert-if-true relationship `@phpstan-assert-if-true` tags express, but
hardcoded for this specific pair rather than declared via a tag PHPStan
ships no docblock for.

**Fix:** lowest-effort option is a targeted special case (like the
`abort_if`/`abort_unless` handling tracked in `docs/todo/laravel.md`)
for `PHPStan\Analyser\Scope::isInClass()` narrowing a subsequent
`getClassReflection()` call on the same `$scope`. Low priority: this is
PHPStan-extension-authoring-specific, not general PHP or Laravel code.

## Symbol resolution

### B65. `ReflectionClass<T>`'s generic argument is not carried through `newInstanceArgs()`

**Impact: Low · Complexity: Medium**

```php
/** @var class-string<Node> $class */
$class = substr(static::class, 0, -4);
$reflection = new ReflectionClass($class);
return $reflection->newInstanceArgs([__METHOD__]);   // reported: got Node|null
```

`new ReflectionClass($class)` on a `class-string<Node>` should produce
`ReflectionClass<Node>`, and its generic `newInstanceArgs(): T` stub
should then resolve to plain `Node` here — matching the declared return
type exactly. An extra `|null` is added instead.

**Fix:** confirm the generic type argument captured from the
`class-string<T>` constructor argument is threaded through to
`newInstanceArgs()`'s templated return, the same way it already must be
for `newInstance()` (worth checking both stay in sync).

### B66. A closure parameter passed to `spl_autoload_register()` is not inferred from the callback's declared type

**Impact: Low · Complexity: Medium**

```php
spl_autoload_register(function ($class): void {
    $file = __DIR__ . strtr(str_replace('App\\', '', $class), '\\', '/') . '.php';   // reported: got array<array-key,string>|string
    // ...
});
```

`spl_autoload_register()`'s stub types its callback parameter as
`callable(string): void`. The closure's own `$class` parameter has no
type declaration, so its type should be inferred from that
context — `string` — the same way an inferred parameter type already
works for other typed-callable consumers (e.g. `array_map`). Left
untyped, `$class` falls back to a wider inferred type and `str_replace()`
returns its full union.

**Fix:** confirm the callable-parameter-type inference used for
`usort`/`array_map`/similar higher-order builtins also covers
`spl_autoload_register`.

## Array types

### B56. `array_key_first`/`array_key_last` are not narrowed non-null after a non-empty guard

**Impact: Low-Medium · Complexity: Medium-High**

```php
/** @param array<int, float> $weights */
function pick(array $weights): int
{
    assert($weights !== []);
    foreach ($weights as $key => $weight) {
        if ($weight > 0.5) {
            return $key;
        }
    }
    return array_key_last($weights);   // reported: got int|null
}
```

`array_key_first()`/`array_key_last()` only return `null` for an empty
array. `assert($weights !== [])` proves `$weights` is non-empty for the
rest of the function, so the call should resolve to plain `int` (the
array's key type), not `int|null`.

**Fix:** add a dynamic-return-type rule for `array_key_first`/
`array_key_last` that drops `null` from the return type when the array
argument is provably non-empty (an `assert()`/guard-clause emptiness
check, or a literal with entries).

### B58. The array union operator (`+`) does not preserve a previously narrowed key type

**Impact: Medium · Complexity: High**

```php
/** @return array<string> shorthand for array<int|string, string> */
function convert(string $raw): array { /* ... */ }

/** @param array<string, string> $vars */
function build(array $vars): void {}

function run(string $raw): void
{
    $data = convert($raw);
    $data = array_filter($data, fn(string|int $key): bool => is_string($key), ARRAY_FILTER_USE_KEY);
    build($data);                      // fine on its own

    $data = getShared() + $data + getShared();   // reported: got array<int|string, string>
    build($data);
}
```

`array_filter(..., ARRAY_FILTER_USE_KEY)` with an `is_string($key)`
guard already narrows the key type correctly on its own (a prior fix
landed this). The narrowing is lost specifically when the result then
feeds into a `+` (array union) expression with other
`array<string, string>`-typed arrays: the merge widens the key type back
to `int|string` instead of keeping `string`.

**Fix:** the `+` operator's result-type computation should combine each
operand's *actual* (possibly narrowed) key type, not re-derive a wider
key type from the operands' declared/generic shapes.

### B59. `max()`/`min()` with scalar arguments return `mixed` instead of the arguments' common type

**Impact: Medium · Complexity: Medium**

```php
function nextLine(int $current, int $extra): void
{
    $line = max($current - 1 - $extra, 0);   // resolves to mixed, not int
    $result[++$line] = 'x';                  // array key ends up int|string instead of int
}
```

Calling `max()`/`min()` with two or more scalar arguments (as opposed to
a single iterable argument) resolves to `mixed` rather than the
arguments' common type (`int` when every argument is `int`). Using the
result as an array key then widens the key type, cascading into
unrelated array-shape mismatches downstream.

**Fix:** add a dynamic-return-type rule for the scalar-arguments overload
of `max`/`min` that returns the union of the resolved argument types
(collapsing to a single type when they agree), matching how the
single-iterable-argument overload is presumably already handled.

### B60. Array keys built from loop-index arithmetic or `array_keys()` still widen to `int|string`

**Impact: Medium · Complexity: High**

```php
/** @return list<string> */
function paths(): array
{
    /** @return array<string, string> absoluteFilePath => viewName */
    $templates = discover();
    return array_keys($templates);   // reported: got list<array-key>, not list<string>
}

/** @return array<int, array<string,int>> */
function lineMapping(string $contents): array
{
    $mapping = [];
    foreach (explode("\n", $contents) as $lineIndex => $line) {
        $mapping[$lineIndex + 1] = ['x' => 1];   // reported: got array<int|string,...>
    }
    return $mapping;
}
```

Both examples only ever produce `int` (arithmetic on a sequential
`foreach` index) or `string` (`array_keys()` on an `array<string, ...>`)
keys, yet the resolved type includes the other half of `array-key`.
Unlike B58 and B59, neither a `+` merge nor `max()`/`min()` appears in
either example, so this may be a separate residual cause rather than a
duplicate of those two — worth re-checking once B58 and B59 are fixed,
since either could be an indirect contributor via code paths not shown
in this trimmed repro.

**Fix:** re-run this project's diagnostics once B58/B59 land; if either
site still reports the widened key type, isolate the exact expression
`array_keys()`/the loop-index write goes through and trace where the
`int`-only or `string`-only key type gets joined with its opposite.

### B68. `array_chunk()`'s return element type resolves to the source array's element type

**Impact: Low-Medium · Complexity: Medium**

```php
/** @param array<int, int> $ids */
function dispatch(array $ids): void
{
    foreach (array_chunk($ids, 500) as $chunk) {
        new BatchJob($chunk);   // reported: expects array<int,int>, got int
    }
}
```

`array_chunk()` always returns an array of arrays — each `$chunk` should
be `array<int, int>` here, matching the source array's element type
wrapped in a list. PHPantom instead resolves `$chunk` to plain `int`, as
though it reused the source array's *element* type as the chunk type
rather than wrapping it.

**Fix:** fix `array_chunk()`'s dynamic-return-type rule to return
`list<array<TKey, TValue>>` (or the tightest list-shape it can prove)
given a `array<TKey, TValue>` input, not `list<TValue>`.

### B69. `array_map()` with a builtin function-name string callback does not preserve the source array's key type

**Impact: Low-Medium · Complexity: Medium**

```php
$ids = explode(',', $csv);       // array<int, string>
$ids = array_filter($ids);       // array<int, string>
$ids = array_map('intval', $ids); // reported: got array<int|string, int>

foreach ($ids as $key => $id) {
    setWeight($key);   // expects int, got int|string
}
```

`array_map()` with a single array argument already preserves the
source's key type when the callback is a closure (per the same fix that
landed B58's `array_filter` case). It does not when the callback is
passed as a builtin function *name string* (`'intval'`) instead of a
closure — an easy-to-miss second call shape for the same rule.

**Fix:** confirm the single-array `array_map()` key-preservation rule
matches on "callback is any callable, closure or name-string", not just
on closures.

### B61. `isset()` narrowing is not propagated through a multi-level chained array-dimension fetch

**Impact: Low-Medium · Complexity: High**

```php
/**
 * @param array{files?: array<string, array{violations?: list<array{rule: string}>}>} $state
 */
function violations(array $state, string $path): array
{
    if (!isset($state['files'][$path]['violations'])) {
        return [];
    }
    return $state['files'][$path]['violations'];   // reported: got list<array{rule:string}>|null
}
```

`violations` is an *optional* shape key (`violations?: ...`), not a
nullable one, so `isset()` on the full three-level chain is the
idiomatic presence check. After it passes, re-reading the same chain
should resolve to the non-optional `list<...>` shape. A stray `|null`
survives instead.

**Fix:** confirm `isset()`'s narrowing walks the full depth of a chained
array-dimension expression (not just a single level) when proving an
optional shape key present, matching the depth the array-shape resolver
itself supports for reads.

### B73. `array_sum()` on a `@var`-annotated `list<int>` from a method call still returns `int|float`

**Impact: Low · Complexity: Medium-High**

```php
class Repo
{
    /** @return list<int> */
    public function counts(): array { /* ... */ }

    public function total(): int
    {
        /** @var list<int> */
        $result = $this->counts();
        return \array_sum($result);   // reported: got int|float
    }
}
```

`array_sum()` should narrow to `int` when every element of its argument
is `int`. This resolves correctly for a plain array literal assigned
under the same `@var list<int>` annotation, but not when the annotated
variable is assigned from a method call whose own declared return type
already is `list<int>` — an unnecessary-looking annotation that
shouldn't change the outcome, but does.

**Fix:** compare how the inline `@var` annotation interacts with an
already-typed call expression on the right-hand side versus a literal;
one of the two appears to skip re-deriving the element type that
`array_sum()`'s rule reads.
