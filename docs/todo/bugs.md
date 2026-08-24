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

### B227. The replace function family's conditional return type never collapses

**Impact: Medium-High · Complexity: Low-Medium**

`str_replace`, `str_ireplace`, `substr_replace`, `preg_replace`,
`preg_replace_callback`, `preg_replace_callback_array`, and `preg_filter`
are patched in `stub_patches.rs` (`patch_replace_family`) with a
conditional return type keyed on `$subject`: array in, array out;
string in, string out. In practice the conditional never resolves to a
single branch — it always falls back to the full `string|array<string>`
union, even when `$subject` is a plain, unambiguous `string` local or a
string literal:

```php
function relative(string $filename): string
{
    return str_replace('\\', '/', $filename); // string|array<string>, not string
}
```

By contrast, other functions patched the same way but keyed on an
"is string" discriminant (`range()`, keyed on `$start`) resolve
correctly. The "is array" discriminant is the common thread across
every failing case (`patch_range` uses `PhpType::named(atom("string"))`
as its condition; `patch_replace_family` uses `PhpType::array()`), which
points at `condition_category`/`type_category` in
`type_engine/types/conditional.rs` or the conditional-evaluation
dispatch in `type_engine/types/narrowing/assertions.rs`
(`evaluate_conditional_for`) mishandling an array discriminant
specifically. Confirmed via minimal repro with a string variable, a
string literal, and a plain array variable as `$subject` — all three
return the unresolved union instead of picking a branch.

Real-world hits: any `str_replace()`/`str_ireplace()` call whose result
feeds a `string`-typed parameter or return raises a false
`type_mismatch_argument`/`type_mismatch_return`.

## Narrowing

### B259. `instanceof` on an array element with a computed key never narrows

**Impact: Medium · Complexity: Medium**

```php
/** @param NodeStmt[] $statements */
function probe(array $statements, int $count): void
{
    if (!$statements[$count - 2] instanceof IfStmt) {
        return;
    }
    $if = $statements[$count - 2];
    $if->elseifs;                       // false unknown_member
    $statements[$count - 2]->elseifs;   // same, read directly
}
```

A literal key (`$statements[2]`) narrows correctly, so the subject key
built for a computed index expression is what the proof is missed on.
Real-world hits are `src/Parser/LastConditionVisitor.php:86,91`, where
the unresolved element also breaks the `@template TValue` binding of the
`array_last()` call it is passed to, so the same line additionally
reports "subject type 'TValue' could not be resolved".

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
