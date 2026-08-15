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
child lookup) was patched directly in a sample source.

Where several of those turn out to share one area of the engine, they
are merged into a grouped entry that carries every repro as a numbered
sub-case. A grouped entry is still one task and one PR.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Narrowing

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

The narrowing itself is not the missing part. In the same branch,
`takesArray($violationMessage)` is accepted, so the scope does hold the
narrowed `array{...}`; only the dimension fetch reads past it and back
to the declared parameter type. It reproduces the same way for the
guard-clause spelling (`if (!is_array($v)) { return; }`), which rules
out the branch-merge as the cause.

**Fix:** make the array-dimension fetch resolve its base expression
through the scope the fetch is written in, rather than from the
subject's declared type. The union arm the guard removed is what leaks
into the element type.

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

### B76. A member read off a class constant is looked for on the class that declares the constant

**Impact: Medium · Complexity: Medium**

```php
enum Kind: string
{
    case A = 'a';
}

class Matrix
{
    public const Kind TYPED = Kind::A;
    public const UNTYPED = Kind::A;

    public function all(): void
    {
        echo self::TYPED->value;     // reported: Property 'value' not found on class 'Matrix'
        echo self::UNTYPED->value;   // reported: Property 'value' not found on class 'Matrix'
        echo Matrix::TYPED->value;   // same, written through the class name
    }
}
```

`Class::CONST->member` resolves the subject to the class the constant is
declared on rather than to the type the constant holds, so every member
read off it is reported as missing. It fires on the declared type and on
the initialiser alike, so writing PHP 8.3's `const Kind TYPED` does not
help, and `self::`, `static::`, and an explicit class name all reach it.
The everyday shape is an enum case held in a constant, where `->value` is
the whole reason to hold it.

There are two paths for `Class::CONST` and only one of them knows this.
`resolve_static_access_type` (`type_engine/call_resolution/return_types.rs`)
reads the constant's declared type and initialiser and gets `Kind` right;
the `SubjectExpr::StaticAccess` arm of `resolve_target_classes_expr`
(`type_engine/resolver/mod.rs`) special-cases a static property (`self::$p`)
and otherwise returns the owning class. That arm is what the unknown-member
check resolves its subject through.

**Fix:** have the `StaticAccess` arm resolve a non-property member through
the same constant-type path the call resolver uses, so both answer alike,
rather than teaching it a second way to read a constant. An enum case is
its own enum, so `Kind::A` must keep resolving to `Kind`.

## Array types

No outstanding items.
