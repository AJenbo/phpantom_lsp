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

The entries below come from the 2026-08-15 evening sample-project sweep
(11 diagnostics across ten projects, run after the B49–B74 fixes
landed). 10 were false positives, filed here as **B75–B82**; the
eleventh is the already-tracked `abort_unless()` narrowing gap (**L5**
in `docs/todo/laravel.md`). No genuine findings surfaced, so nothing
was patched in the sample sources. Every entry was isolated in a
scratch repro and bisected to the minimal trigger shown in its code
block; several sit at the same source sites as fixed bugs from the
previous sweep (B50, B54, B59, B62), where the coarse defect was fixed
and a finer one behind it became visible. **B83** was filed the same
day from the follow-up re-run at `c618c8aa`, whose compatibility
tightening surfaced one previously-swallowed site, and **B84** from a
probe at `66a524bc` showing the return-position side of that
tightening is still missing.

## Crashes

No outstanding items.

## Type comparison

### B84. Return-position compatibility ignores an array shape's value types

**Impact: Medium · Complexity: Medium**

```php
/** @return array<string, int> */
function bad(): array {
    return ['a' => 'x'];   // not reported
}

/** @param array<string, int> $m */
function takesIntMap(array $m): void {}

function alsoBad(): void {
    takesIntMap(['a' => 'x']);   // reported, as expected
}
```

Needs investigation: `type_mismatch_argument` correctly reports an
array shape whose values do not satisfy the declared map or list value
type (`array{a: 'x'}` vs `array<string, int>`, `array{'x', 'y'}` vs
`list<int>`), but `type_mismatch_return` accepts the identical
mismatch silently. The return side is not skipping shapes entirely —
a nullability mismatch in a shape value (`array{a: ?bool}` vs
`array<string, int>`) is reported in return position — so the two
diagnostics are reaching different verdicts for the same shape-vs-map
comparison somewhere below the nullability check. `array<string,
never>` as the declared type is the extreme case: any all-optional-keys
shape (e.g. an `array_filter()` result) passes against it, which is
what kept masking scratch probes during the 2026-08-15 sweeps.

Found by probe at `66a524bc`, after the argument-side tightening
landed; no sample-project site currently hits it.

**Fix:** find where the return-position compatibility path diverges
from the argument-position path for shape-to-map value checks and
unify them; the recently tightened argument behaviour is the correct
one.

## Standard-library return types

### B80. `max()`/`min()` argument-dependent return type is a malformed union

**Impact: Medium · Complexity: Medium**

```php
function track(int $t): void
{
    $m = max($t, filemtime('a'));
    assert($m !== false);
    takesInt($m);   // reported: got int|int|bool
}

final class Excerpt
{
    /** @return array<int, string> */
    public function lines(int $a, int $b = 0): array
    {
        $line = max($a - 1 - $b, 0);
        $result = [];
        $result[$line] = 'x';
        return $result;   // reported: array<int|string, string>
    }
}
```

Two defects in the computed return type:

1. `max(int, int|false)` produces `int|int|bool` — the argument union
   is not deduplicated, and the `false` arm degrades to `bool`. The
   degraded `bool` then survives everything that would have stripped a
   `false`: `max(...) ?: 0` keeps a `bool` arm (`filemtime($f) ?: 0`
   alone is correctly `int`), and an `assert($m !== false)` leaves
   `int|true`.
2. Inside a class method (static or instance), a `max()` result whose
   argument is an arithmetic expression poisons array-key inference:
   using it as a write key widens the array's key type to
   `int|string`. The same statements in a top-level function infer
   `array<int, string>`, as does `max($a, 0)` without the arithmetic.
   The variable itself still passes a `takesInt()` check, so the bad
   type only surfaces through the key-type path.

Sample sites: `agcms inc/Http/Controllers/AbstractController.php:72`
(`max($updateTime, filemtime($filename)) ?: 0` summed into an `int`
return reported as `int|bool`), `phpmd src/Renderer/HTMLRenderer.php:463`
(`$result[++$line]` keyed by a `max($lineNumber - 1 - $extra, 0)`
result reported as `array<int|string, string>`).

**Fix:** normalize the argument-dependent return union for `max()`/
`min()` (dedupe, preserve literal `false`), and find why method-scope
resolution of the arithmetic argument produces a stringy arm that
plain function scope does not.

### B79. `array_filter()` without a callback keeps `null` on values that share the array with a `?bool`

**Impact: Low-Medium · Complexity: Medium**

```php
final readonly class Dto
{
    public function __construct(public ?int $w = null, public ?bool $a = null) {}

    /** @return array<string, int|true> */
    public function toArray(): array
    {
        return array_filter(['w' => $this->w, 'a' => $this->a]);
        // inferred: array<string, ?int|true> — null survives on the int
    }
}
```

No-callback `array_filter()` removes every falsy value, so `null` and
`false` arms must be stripped and `?bool` collapses to `true`. Each
piece works in isolation: an array of only `?int` properties strips its
nulls, and a lone `?bool` correctly collapses to `true`. But as soon as
a `?bool` value is present in the array, its *sibling* nullable values
(`?int`, `?string`, `?DateTime`) keep their `null` arms. The vytrvalec
sample site (`src/Dto/Season/Request/SeasonQueryFilterRequestDto.php:26`)
builds exactly this shape from readonly promoted properties — two
`?bool` filters among six other nullables — and is reported as
returning `array<string, ?DateTime|?int|true|?string>`.

**Fix:** strip falsy arms per element independently; the `?bool`
handling is somehow clobbering or short-circuiting the falsy-stripping
of the other elements in the same shape.

## Narrowing

### B75. A dim-write to the foreach value variable leaks through the loop back-edge

**Impact: Medium · Complexity: High**

```php
function normalize(mixed $steps): void
{
    if (is_array($steps)) {
        foreach ($steps as $step) {
            if (!is_array($step)) {
                throw new RuntimeException('not array');
            }
            $raw = isset($step['fo']) && is_string($step['fo']) ? $step['fo'] : '{}';
            json_decode($raw, true);   // reported: got array|'{}'
            $decoded = json_decode($raw, true);
            if (!is_array($decoded)) {
                throw new RuntimeException('bad json');
            }
            $step['fo'] = $decoded;    // <-- remove this line and the FP disappears
        }
    }
}
```

The foreach rebinds `$step` to a fresh element at the top of every
iteration, so the `$step['fo'] = $decoded` write at the bottom of the
body must not influence the next iteration's `$step`. It does: the
loop back-edge merges the written element type (`array`) into the
rebound variable, and the `isset(...) && is_string(...)` ternary guard
then fails to filter it — the true arm keeps the leaked `array` type
instead of narrowing to `string` (an `is_string` proof should eliminate
a pure-`array` arm entirely). Without the write-back (or without the
loop) the same guard narrows correctly.

Sample sites: `luxplus-backoffice .../ProductRoutineTemplatesController.php:120`
and `:204` (identical normalize-steps loops).

**Fix:** foreach rebinding must reset the value variable to the
iterated element type on every back-edge merge; dim-writes to it are
dead state once the variable is rebound.

### B76. Blade variables typed from a component class are immune to condition narrowing

**Impact: Medium · Complexity: Medium-High**

```blade
{{-- component view, backed by a component class that declares
     public ?string $countryName / public readonly ?array $rules --}}
<small>NEW {{ $countryName ? strtoupper($countryName) . ' ' : '' }}ORDERS</small>
{{-- reported: strtoupper() got string|null --}}

@foreach ($rules as $rule)
    {{ is_string($rule) ? $rule : $rule->getDescription() }}
    {{-- reported: e() got IFileValidationRule|string --}}
@endforeach
```

Both expressions narrow correctly when the template's variable types
come only from its own `@bladestan-signature` `@var` block: copying
either file byte-for-byte to a path no component renders produces zero
diagnostics. In place — where the variables' types are supplied by the
component class (`Dashboard\NewOrdersOverview::$countryName`,
`File\Upload::$rules`) — the ternary condition's proof is discarded
and the un-narrowed union flows into the arm. The render-site/
component-supplied type appears to be re-pinned at every read instead
of participating in narrowing like a normal scope entry.

Sample sites: `luxplus-backoffice resources/backoffice/views/components/dashboard/new-orders-overview.blade.php:23`,
`.../components/file/upload.blade.php:35`.

**Fix:** inject component-class/render-site variable types as ordinary
initial scope state (the way the signature block's types are), so the
forward walker narrows them like any other variable.

### B77. A foreach over a proven non-empty array still merges the zero-iteration path

**Impact: Medium · Complexity: Medium-High**

```php
/** @param array<int, int> $qtys */
function floorStock(array $qtys): int
{
    if (!$qtys) {
        return 0;
    }
    $max = null;
    foreach ($qtys as $qty) {
        if ($max === null) {
            $max = $qty;
            continue;
        }
        $max = min($max, $qty);
    }
    return takesInt($max);   // reported: got null|int
}
```

`if (!$qtys) return;` proves the array non-empty, so the loop body runs
at least once, and every path through the body assigns `$max` an `int`.
After the loop `$max` cannot be `null`, but the pre-loop `null` is
still merged in. PHPStan eliminates the zero-iteration state when the
iterated subject is a non-empty array.

Sample site: `luxplus-shared src/core/PCN/PCNService.php:845` (bundle
stock floor tracked across `$bundleItemQty`, guarded non-empty).

**Fix:** when the iterated expression's type is proven non-empty at
the loop head, exclude the "body never ran" branch from the post-loop
merge.

### B82. A nullsafe chain compared `===` to a non-nullable value does not narrow the receiver

**Impact: Medium · Complexity: Medium**

```php
final class NodeX
{
    public function getParent(): ?NodeX { /* ... */ }
    public function getChild(): NodeX { /* ... */ }
    public function getInner(): object { /* ... */ }
}

function strip(NodeX $n): NodeX
{
    $parent = $n->getParent();
    if ($parent?->getChild()->getInner() === $n->getInner()) {
        return $parent;   // reported: ?NodeX incompatible with NodeX
    }
    return $n;
}
```

If `$parent` is `null`, the nullsafe chain short-circuits to `null`,
and `null === $n->getInner()` is `false` because the right-hand side's
type excludes `null`. So the `===` succeeding proves `$parent` is
non-null inside the branch. PHPStan applies this narrowing.

Sample site: `phpmd src/Rule/AbstractLocalVariable.php:130`
(`stripWrappedIndexExpression`) — the same site whose plain-dereference
half was B54 in the previous sweep.

**Fix:** in the true branch of `$x?->chain() === $rhs`, when `$rhs`'s
type excludes `null`, narrow `$x` to non-null (and mirror for `!==` in
the false branch).

### B83. A `match (true)` arm's condition does not narrow inside the arm's result

**Impact: Medium · Complexity: Medium**

```php
/** @param list<int|string> $args */
function takesList(array $args): void {}

function label(?int $buy, ?int $pay, string $kind): void {
    $textArgs = match (true) {
        $kind === 'xy' && $buy !== null && $pay !== null => [$buy, $pay],
        default => [],
    };
    takesList($textArgs);   // reported: array{?int, ?int} does not satisfy list<int|string>
}
```

The `!== null` conjuncts of a `match (true)` arm's condition prove the
values non-null within that arm's result expression, exactly as the
equivalent `if` statement does — and the `if` form narrows correctly.
Inside a match arm nothing narrows: plain variables and property reads
both keep their `null` arms.

Filed 2026-08-15, after the evening sweep: the argument-compatibility
tightening in `c618c8aa` surfaced it — the resulting shape mismatch
was previously swallowed by the compatibility leniency that T32 tracks
(the argument side has since been tightened further; the return-side
remainder is B84).

Sample site: `luxplus-website app/Contexts/Api/Resources/ProductResource.php:210`
(discount label `$textArgs` built from `?int` properties the arm
condition null-checks).

**Fix:** evaluate each `match (true)` arm's result expression under
the same condition-derived scope state the `if` evaluator would build
from that arm's condition.

## Symbol resolution

No outstanding items.

## Array types

### B81. Foreach element extraction widens `false` to `bool`

**Impact: Low-Medium · Complexity: Medium**

```php
function collect(string $p): void
{
    $files = [];
    $files[] = realpath($p);   // string|false — stored correctly ($files[0] reads back string|false)
    foreach ($files as $file) {
        assertNotFalse($file);       // @phpstan-assert !false
        takesString($file);          // reported: got string|true
    }
}
```

The element union produced for a `foreach` over the array degrades the
stored `false` literal to `bool`: a `@phpstan-assert !false` (PHPUnit's
`assertNotFalse()`) then strips only the `false` half and leaves a
phantom `string|true`. A dim read (`$files[0]`) of the same array
preserves `string|false`, and the identical assert on a plain variable
holding `realpath()`'s result narrows cleanly to `string` — the
widening is specific to the foreach element-type computation.

Sample site: `pdepend tests/php/PDepend/AbstractTestCase.php:765` — the
same site as the previous sweep's B62; the assertion is now recognized
(B62 fixed) and this widening is what it uncovered.

**Fix:** flatten array-shape values into the foreach element union
without literal-to-general widening, matching what the dim-read path
already does.

## Docblock handling

### B78. A standalone `@var` cast above `return` is ignored

**Impact: Medium · Complexity: Medium**

```php
function giveString(): string { return 'x'; }

function cast(): int
{
    /** @var int */
    return giveString();   // reported: string incompatible with int — the cast was ignored
}
```

A nameless `/** @var T */` docblock directly above a `return` statement
casts the returned expression to `T` (PHPStan semantics). PHPantom
never applies it: the diagnostic judges the raw inferred type of the
expression as if the cast were absent, whether the expression is a
call, a variable, or an array literal. The named form above an
assignment (`/** @var int $x */` then `$x = ...`) works; only the
nameless return-position cast is missing. Beware vacuously-clean
repros when testing a fix: an inferred optional-keys array shape
currently satisfies almost any declared array type, so the cast's
absence only shows against scalar or definite types.

Sample site: `vytrvalec-server src/Dto/Season/Request/SeasonQueryFilterRequestDto.php:26`
(same site as B79; either fix alone clears the diagnostic, both are
real defects).

**Fix:** when resolving a `return` statement's expression, look up a
preceding standalone `@var` docblock and use its type as the expression
type, the same way the named assignment form already does.
