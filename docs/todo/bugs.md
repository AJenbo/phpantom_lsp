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

### B258. Two variables assigned in the same branch lose their correlated nullability at the merge

**Impact: Medium · Complexity: High**

```php
$acceptor = null;
$reflection = null;
if ($name !== '') {
    $reflection = $this->find($name);
    if ($reflection !== null) {
        $acceptor = $this->select($reflection);   // non-null exactly when $reflection is
    }
}

if ($reflection !== null) {
    $this->useBoth($reflection, $acceptor);       // false type_mismatch_argument on $acceptor
}
```

The merge keeps each variable's own union (`?Reflection`, `?Acceptor`)
and forgets that the two were written on the same path, so the later
`$reflection !== null` check cannot recover what it implies about
`$acceptor`. Real PHPStan is silent on the shape as its own source
writes it, and it is not in its baseline, so it tracks the correlation
somehow — the mechanism is not root-caused here.

Reproduced standalone. Real-world hits are the `$parametersAcceptor`
cluster in `src/Analyser/ExprHandler/` (`FuncCallHandler.php:977,1042`,
`MethodCallHandler.php:169,350`, `StaticCallHandler.php:240,455`), where
the acceptor is built in the same branch that resolves the method
reflection the later check tests.

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

No outstanding items.
