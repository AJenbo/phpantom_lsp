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

No outstanding items.

### B77. A one-line function body lets the preceding docblock's `@param` reach the next function

**Impact: Low · Complexity: Medium**

```php
/** @param array<Status> $s */
function g3(array $s): void { foreach ($s as $x) { doThing($x); } }

function g4(Status $s): void { doThing($s); }   // reported: got array<Status>
```

`g4` declares no docblock of its own, so the backward `@param` scan in
`find_iterable_raw_type_in_source` keeps going and finds `g3`'s, matching
on the shared parameter name `$s`. The scan does have a sibling-function
boundary check, but it only fires once it has seen the brace depth rise
above zero and come back down. A body written entirely on the signature
line opens and closes on the same line, so the net depth never leaves
zero and the boundary is never detected.

Reformatting `g3` across multiple lines makes it go away, which is why
this stays out of sight in PSR-12 code and shows up in fixtures and
tests, where one-line bodies are the norm.

**Fix:** detect the sibling boundary from the `function` keyword at the
scan's own depth rather than from a depth excursion, so a body that
opens and closes on one line still ends the search. The same scan is
what `resolve_param_type` and the array-literal element inference both
call, so both inherit the leak.

## Array types

No outstanding items.
