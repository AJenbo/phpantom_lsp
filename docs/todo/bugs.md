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

### B261. A property keeps its narrowing across a write the resolver cannot type

**Impact: Low-Medium · Complexity: Medium**

```php
class Holder
{
    /** @var A|C */
    public $prop;

    public function f($u): void
    {
        if ($this->prop instanceof A) {
            $this->prop = $u->make();
            $this->prop->onlyC();   // false unknown_member, "on class 'A'"
        }
    }
}
```

The property branch of `process_assignment_expr`
(`forward_walk/assignment.rs`) records the written type under the
property-path key, but `ScopeState::set` ignores an empty type list, so
a right-hand side that resolves to nothing leaves the `instanceof`
narrowing from before the write in place. The same defect on a plain
variable was fixed by writing "no type known" over the old entry; a
property cannot use that, because the correct fallback for a property is
its *declared* type, not unknown, so the key has to be dropped rather
than blanked. Narrower than the variable case: it only misreports when
the declared type is wider than what the check narrowed it to, since
otherwise the declared type answers the same way.

## Arithmetic

No outstanding items.

## Symbol resolution

### B243. `PHPStan\Analyser\Scope::mergeWith()` is a confirmed false positive with no explanation on the PHPStan side

**Impact: Low-Medium · Complexity: Unknown**

`Scope` genuinely has no `mergeWith` (only `MutatingScope` does), and the
expression `NodeScopeResolver.php:5404,5412` calls it on is
`StatementResult::getScope()`, declared `: Scope`. PHPantom's own
`dumpType`-equivalent agrees, and the same file spells the conversion out
explicitly 4,000 lines earlier
(`$statement->getScope()->toMutatingScope()` at line 1112), so reading the
source alone says the call is an error and the diagnostic is right. Yet
real PHPStan raises nothing, and no entry for it exists in
`phpstan-baseline.neon`. No stub, mixin, or reflection extension
explaining the silence was found, and running PHPStan on itself to settle
it needs a `composer install` in the checkout. Filed as a confirmed false
positive whose cause is still unaccounted for; the eight call sites are
the only ones seen anywhere.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

### B262. `analyze` orders two diagnostics on the same line differently from run to run

**Impact: Low · Complexity: Low**

Running `phpantom_lsp analyze` twice over an unchanged checkout produces
reports that differ in the order of diagnostics sharing a line. On
`references/phpactor` one run prints

```
    99   Unused variable '$offset'
         🪪  unused_variable
```

and the next prints the two lines the other way round relative to the
neighbouring diagnostic on the same line. The counts and the diagnostics
themselves are identical, so nothing is lost, but a report that is not
byte-identical between runs cannot be diffed, which is the natural way to
check a change introduced no new findings. Reproduced on `phpactor`,
`laravel-framework`, and `php-lsp`; the sort that emits the report needs
a total order (line, then column, then code) rather than one that leaves
same-line entries to whatever order the parallel walk finished in.
