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

Bugs land here from wherever they surface: found while working on another
task, or sweeps of the sample projects under `projects/`. Entries are
grouped by the mechanism that has to change, not by the symptom that
surfaced: one entry is one root cause, however many shapes it shows up in.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Reachability

No outstanding items.

## Narrowing

No outstanding items.

## Arithmetic

No outstanding items.

## Symbol resolution

### B310. An assignment expression as a call receiver never resolves

**Impact: Medium · Complexity: Medium**

Calling a method directly on a parenthesized assignment loses the
receiver's type entirely — plain assignment, `??=` on a variable, and
`??=` on an array offset all report
`Cannot verify method 'm' — type of '' could not be resolved`:

```php
if (($x = $map[$key])->truthy()) { ... }             // unresolved
if (($cached ??= $map[$key])->truthy()) { ... }      // unresolved
if (($cache[$key] ??= $map[$key])->truthy()) { ... } // unresolved

$r = $cache[$key] ??= $map[$key];
if ($r->truthy()) { ... }                            // resolves fine
```

Two defects share the root: the subject resolver does not compute a type
for `Assign`/`AssignOp` receiver expressions, and the diagnostic prints
an empty subject name for them. Curiously, the shapes resolve in a
small scratch project but fail when the identical file is dropped into
`projects/pdepend` or `projects/phpstan-src` — some small-project
fallback rescues the case, which is worth understanding during the fix
but is not the fix: the resolver should handle assignment receivers
directly. Sweep site: `src/Analyser/ScopeOps.php:513` (PHPStan Source,
2026-08-29; the sibling construct at `:496` uses the assigned-variable
form and resolves).

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

No outstanding items.
