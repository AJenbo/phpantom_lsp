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

The **B85** entry below comes from the 2026-08-15 evening sample-project
sweep and the re-runs that followed it as the sweep's own findings
(**B75–B84**) were fixed and cleared. The three diagnostics still
standing across the ten projects at the time of writing are this one
plus two genuine findings (`int / int` returned from an `int` function),
which are the sample sources' own to fix. As with every entry here, it
was isolated in a scratch repro and bisected to the minimal trigger
shown in its code block.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Narrowing

No outstanding items.

## Arithmetic

### B85. A magic constant is not recognised as an arithmetic operand

**Impact: Low-Medium · Complexity: Low**

```php
function getEndLineOfThisFile(): int
{
    return __LINE__ + 3;   // reported: got int|float
}
```

`__LINE__` on its own resolves to `int` and returns cleanly, but as an
operand of `+` it is not classified, so the addition falls back to the
conservative `int|float` that a mixed-operand sum produces. Every
`int`-typed magic constant (`__LINE__`) and `string`-typed one
(`__FILE__`, `__DIR__`, `__FUNCTION__`, `__CLASS__`, `__METHOD__`,
`__NAMESPACE__`) should be classified the same way its own type is.

Sample site: `pdepend tests/php/PDepend/Source/AST/ASTCompilationUnitTest.php:293`.

**Fix:** classify a magic constant by its own resolved type in the
binary-operator operand classifier, the way `int` refinements already
are.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.
