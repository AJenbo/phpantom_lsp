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

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Narrowing

No outstanding items.

## Arithmetic

### B86. `int / int` reaching an `int` position is reported in a non-strict file

**Impact: Medium · Complexity: Low**

```php
<?php   // no declare(strict_types=1)

class Job
{
    public int $timeout = 3600;

    public function __construct(int $max)
    {
        $this->timeout = $max / 300;      // reported: expects int, got int|float
    }

    public function toWholeDays(int $length): int
    {
        return $length / 86400;           // reported: got int|float
    }
}
```

`int / int` resolves to `int|float`, which is correct and deliberate,
but a file without `declare(strict_types=1)` coerces the float half on
the way in, so PHP accepts all three positions (argument, return, and
typed-property assignment) without a TypeError. The union-argument rule
in `is_type_compatible` demands that *every* member satisfy the
parameter, so the `float` half is reported. PHPStan reports none of
these at level max.

Two escape hatches next to it already model exactly this juggling:
`numeric` → `int`/`float`, and `string` → `int`/`float` outside
`strict_types`. `float` → `int` is the missing one.

Sample sites: `luxplus-shared src/common/DateTime/TimePeriod.php:109`
(`toWholeDays()`), `luxplus-backoffice
app/Jobs/Elastic/ReindexCustomers.php:23` (`int $timeout`). Both files
are non-strict, and both were mistaken for genuine findings in the
2026-08-15 sweep before PHPStan was run on them.

**Fix:** accept `float` where `int` is expected when the file is not
under `declare(strict_types=1)`, alongside the `numeric` and `string`
hatches. Behind it sits a wider policy question worth deciding
separately: PHPStan does not report the *strict* spelling of these
either, because a union that partly satisfies the target is a "maybe"
to it, whereas our union-argument rule requires every member to satisfy.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.
