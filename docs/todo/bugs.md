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

### B87. Union-argument compatibility requires every member to satisfy, PHPStan requires only one

**Impact: Low · Complexity: High**

`is_type_compatible`'s union-argument rule demands that *every* member
of an argument's union type satisfy the parameter before the call is
accepted; PHPStan treats a union that only *partly* satisfies the
target as a "maybe" and stays silent rather than reporting the members
that don't. This came up while fixing `float` → `int` coercion outside
`strict_types` ([`docs/CHANGELOG.md`](../CHANGELOG.md)): PHPStan does
not report the *strict* spelling of that case either (an `int|float`
value assigned to `int` under `declare(strict_types=1)`), specifically
because the `int` half already satisfies the parameter and the union
rule only needs one member to.

Whether to relax the union-argument rule to "at least one member
satisfies" project-wide is a bigger policy change than the escape
hatch above: it would silence real mismatches where only one union
member happens to be compatible by accident rather than by design.
Needs a decision on the tradeoff (and probably a survey of how much
signal the current "every member" rule catches in `projects/*`) before
changing it.

## Standard-library return types

No outstanding items.

## Narrowing

No outstanding items.

## Arithmetic

No outstanding items.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.
