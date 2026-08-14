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

Where several of those turned out to share one area of the engine, they
have since been merged into a grouped entry that carries every repro as
a numbered sub-case (**B75** so far). A grouped entry is still one task
and one PR.

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

## Standard-library return types

### B75. A builtin's return type is not derived from the arguments actually passed

**Impact: Medium · Complexity: Medium-High**

Six standard-library functions whose resolved return type is wider than
what the arguments prove. They are grouped as one task because they all
run through the same pair of mechanisms and the fixes have to keep those
two in agreement about which one owns each function:

- **`src/stub_patches.rs`** — templates and conditional returns grafted
  onto the embedded stub (`patch_array_map`,
  `patch_array_key_value_generics`, `patch_replace_family`, …).
- **`src/type_engine/variable/array_func_rules.rs`** — rules driven by
  the *resolved* argument types, with the classification lists in
  `src/type_engine/variable/mod.rs` (`ARRAY_PRESERVING_FUNCS`,
  `ARRAY_ELEMENT_FUNCS`) deciding which rule a function gets.

Each cause below was confirmed against a release build on a minimal
repro rather than inferred from the original report; three of the six
turned out to have a different cause than first assumed, noted inline.

#### 1. A `\`-qualified call misses every array-function rule

```php
/** @param list<int> $r */
function sum(array $r): int { return \array_sum($r); }   // reported: got int|float

/** @param list<User> $us */
function pop(array $us): void { $u = \array_pop($us); $u->name(); }   // $u is mixed
```

Dropping the leading `\` makes both correct. `func_name` in
`type_engine/variable/rhs_resolution/calls.rs` is the raw identifier
text, so `"\\array_sum"` never matches the rule table. The Laravel
container branch a few lines above already normalizes with
`trim_start_matches('\\')`; the array-function branches below it do not.

This is the widest of the six: it silently disables the rules for
*every* entry in `ARRAY_PRESERVING_FUNCS` and `ARRAY_ELEMENT_FUNCS`
whenever the call is written fully qualified, which is the house style
in a fair amount of library code.

**Fix:** normalize the callee name once, where `func_name` is bound, and
use the normalized form for all the rule lookups that follow.

#### 2. `array_chunk()`'s elements are the source array's elements

```php
/** @param array<int, int> $ids */
function dispatch(array $ids): void
{
    foreach (array_chunk($ids, 500) as $chunk) {
        new BatchJob($chunk);   // reported: expects array<int,int>, got int
    }
}
```

`array_chunk` is listed in `ARRAY_PRESERVING_FUNCS`, so it is told to
hand back the input's element type unchanged. It is the one function in
that list that adds a level of nesting rather than rearranging entries.

**Fix:** drop it from `ARRAY_PRESERVING_FUNCS` and give it its own rule
returning `list<array<TKey, TValue>>` (or `list<list<TValue>>` when
`$preserve_keys` is absent or `false`) from an `array<TKey, TValue>`
input.

#### 3. `str_replace()`'s conditional return is not decided for a `??` subject

```php
function clean(?string $error): ?string
{
    return str_replace('Error: ', '', $error ?? '');   // reported: got array<array-key, string>|string
}
```

The stub-patch side is already right: `patch_replace_family` gives
`str_replace`/`str_ireplace` a conditional return keyed on `$subject`,
and it resolves correctly for a plain variable, a literal, a nested call
(`trim($s)`), and a variable pre-assigned from `$s ?? ''`. It fails only
when the argument expression *is* the coalesce. The condition evaluator
gets no type for a `??` expression argument and falls back to the
unrefined union, so this is an argument-resolution gap, not the missing
dynamic-return rule the original report called for.

**Fix:** resolve a coalesce expression's type when collecting argument
types for conditional-return evaluation (the union of the left operand
with `null` stripped and the right operand). Worth checking which other
argument expression shapes come back untyped on the same path.

#### 4. `max()`/`min()` return `mixed` for every call shape

```php
function takesInt(int $n): void {}

takesInt(max("a", "b"));   // accepted: max() resolves to mixed
/** @var list<string> $ss */
takesInt(max($ss));        // accepted for the same reason
```

Neither overload is modelled. The original report assumed the
single-iterable form was already handled and only the scalar-arguments
form was missing; both return `mixed`.

**Fix:** add a rule covering both shapes — the union of the resolved
argument types for the variadic form (collapsing to a single type when
they agree), and the element type for the single-iterable form.

#### 5. `array_key_first`/`array_key_last` keep `null` after a non-empty proof

```php
/** @param array<int, float> $weights */
function pick(array $weights): int
{
    assert($weights !== []);
    return array_key_last($weights);   // reported: got int|null
}
```

`patch_array_key_value_generics` types these as `TKey|null`, which is
correct in general — the `null` only happens for an empty array. Nothing
consults the argument's proven emptiness to drop it.

**Fix:** drop the `null` arm when the array argument is provably
non-empty: an `assert()`/guard-clause emptiness check, a `non-empty-*`
type, or a literal with entries. `key()` carries the same `TKey|null`
patch and should follow.

#### 6. `array_map()` loses the key type for a function-name string callback

```php
$ids = explode(',', $csv);              // array<int, string>
$ids = array_map('intval', $ids);       // reported: got array<int|string, int>

foreach ($ids as $key => $id) {
    setWeight($key);   // expects int, got int|string
}
```

The closure form (`fn (string $s): int => (int) $s`) keeps `int` keys;
only the name-string form widens, with or without an intervening
`array_filter`. The value type is right in both, so the callback's
return is being resolved either way — it is the key that falls back to
the stub's unbound `TKey`.

**Fix:** make the single-array key-preservation path match on "callback
is any callable" rather than on an inline closure. Note that
`array_func_raw_type` currently answers `array_map` with a bare
`PhpType::list(...)` regardless of the input's key type, so the two
mechanisms need to be reconciled here rather than patched independently.

#### Verification

Each sub-case has a self-contained repro above; add a fixture or
integration test per sub-case. Once this lands, re-check
[B60](#b60-array-keys-built-from-loop-index-arithmetic-or-array_keys-still-widen-to-intstring),
whose two examples do not reproduce in isolation and may be downstream
of the key-type handling here or of
[B58](#b58-the-array-union-operator--does-not-preserve-a-previously-narrowed-key-type).

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
Neither example reproduces on its own against a release build, so this
may be a residual symptom of a cause elsewhere rather than a defect of
its own — the two candidates being the `+` merge below and the
key-type handling in
[B75](#b75-a-builtins-return-type-is-not-derived-from-the-arguments-actually-passed),
either of which could contribute via code paths not shown in this
trimmed repro.

**Fix:** re-run this project's diagnostics once B58/B75 land; if either
site still reports the widened key type, isolate the exact expression
`array_keys()`/the loop-index write goes through and trace where the
`int`-only or `string`-only key type gets joined with its opposite.

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
