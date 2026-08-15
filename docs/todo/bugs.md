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
tightening surfaced one previously-swallowed site.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Narrowing

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
was previously swallowed by the diagnostic layer's array leniency
(the argument side has since been tightened further).

Sample site: `luxplus-website app/Contexts/Api/Resources/ProductResource.php:210`
(discount label `$textArgs` built from `?int` properties the arm
condition null-checks).

**Fix:** evaluate each `match (true)` arm's result expression under
the same condition-derived scope state the `if` evaluator would build
from that arm's condition.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.
