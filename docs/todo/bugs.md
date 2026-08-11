# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Effort** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

## B1. A function declared in two files resolves to a different one on every run

**Impact: Medium · Effort: Low-Medium**

When two indexed files declare the same fully-qualified function, which
declaration wins is decided by whichever indexing worker finishes last,
so repeated analyses of an unchanged project disagree with each other.
PHPStan's own test corpus has a minimal case:
`tests/PHPStan/Levels/data/stubs-functions.php` declares
`StubsIntegrationTest\foo($i)` with no annotations, and
`tests/PHPStan/Levels/data/stubs/function.php` declares the same
function with `@param int $i` / `@return string`. Running `analyze` on
that directory four times in a row reports the argument type mismatch on
`foo('test')` in some runs and nothing in others.

The user-visible effect is diagnostics that appear and disappear between
identical runs, which reads as a flaky analyzer, and (in the editor) a
go-to-definition target that changes between sessions.

**Fix:** make the winner deterministic rather than last-writer-wins.
Break the tie on a stable key (scan order, then path) when a duplicate
function declaration is inserted, so the same project always produces
the same symbol table. Duplicate class declarations go through the same
index and should be checked for the same race while in there.

---

## B2. A template bound from several parameters is checked one argument against another

**Impact: Low-Medium · Effort: Medium**

`self_bound_template_params` deliberately skips the argument check only
for a `@template` parameter with exactly one binding site, on the
grounds that a template bound from several parameters "still carries a
real independent check". It does not: the substitution keeps the type
from one binding site (unioning it with the others where the binding
mode remembers to, and overwriting it where it does not), so every other
argument is then measured against a type taken from one of its
siblings.

```php
/**
 * @template T
 * @param T[] $first
 * @param T[] $second
 */
function combine(array $first, array $second): void {}

// Argument 1 ($first) expects array<string>, got list<int>
combine([1, 2], ['a', 'b']);
```

`T` is bound to `string` from the second argument, and the first is then
reported for not being one. PHPStan infers `T` as the union of every
binding site and reports nothing here.

**Fix:** union at every binding site in `build_function_template_subs`.
The direct, callable and class-string modes already do; the array
literal and array-position branches of `GenericWrapper` and
`ArrayElement` still overwrite, so a multi-bound template resolves to
whichever argument was resolved last rather than to what all of them
have in common.
