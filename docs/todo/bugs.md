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
and a finer one behind it became visible.

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

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.
