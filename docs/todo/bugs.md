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

No outstanding items.

## Symbol resolution

No outstanding items.

## Array types

### B266. A variable-indexed array element doesn't inherit the array's value type even behind an `array_key_exists` guard

**Impact: Medium-High · Complexity: Medium-High**

```php
/** @var array<int, ConstantArrayType> $arraysToProcess */
foreach ($eligibleCombinations as $i => $other) {
    if (!array_key_exists($i, $arraysToProcess)) {
        continue;
    }
    foreach ($other as $j => $count) {
        if (!array_key_exists($j, $arraysToProcess)) {
            continue;
        }
        $arraysToProcess[$i]->getKeyTypes(); // "type of '$arraysToProcess[$i]' could not be resolved"
    }
}
```

(`src/Type/TypeCombinator.php:1339-1471`, 20 occurrences in this loop alone.)
`$arraysToProcess[$i]` should resolve to the array's declared/inferred value type
(`ConstantArrayType`) the moment the key is known to exist, exactly as a literal-index
access like `$arraysToProcess[0]` would. PHPantom instead reports the subject as fully
unresolved for a variable key, even one just proven present by `array_key_exists()`.
Real-world hits: any loop that pairs off elements of an array by index (`$arr[$i]`,
`$arr[$j]`) after checking presence — a common pattern for building combinations,
diffs, or lookup tables.

### B267. An array populated inside a closure via an `instanceof`-narrowed push isn't typed once the closure has run

**Impact: Medium-High · Complexity: High**

```php
$executionEnds = [];
$this->processStmtNodesInternal($stmt, $stmt->stmts, $scope, $storage, new GatheringNodeCallback(
    static function (Node $node, Scope $scope) use (&$executionEnds): void {
        if ($node instanceof ExecutionEndNode) {
            $executionEnds[] = $node; // $node is narrowed to ExecutionEndNode right here
        }
    },
), StatementContext::createTopLevel());

foreach ($executionEnds as $executionEnd) {
    $executionEnd->getStatementResult(); // "type of '$executionEnd' could not be resolved"
}
```

(`src/Analyser/NodeScopeResolver.php:1031-1093`.) `$executionEnds` is captured
`use (&$executionEnds)` and mutated only after the closure's own `$node` parameter is
narrowed via `instanceof ExecutionEndNode`. Once the call that invoked the closure
returns, PHPantom has no record that the array's element type is `ExecutionEndNode`
rather than the closure parameter's declared `Node` — the enclosing `foreach` sees an
unresolved element type. The damage carries onward: `$endScope =
$executionEnd->getStatementResult()->getScope()` is unresolved in turn, so the
`$finalScope` accumulator the loop folds out of it never has anything to resolve
either, and every use of it down to `rememberConstructorScope()` is reported.

## Docblock handling

No outstanding items.

## Miscellaneous

### B268. `deprecated_usage` over-reports two call shapes PHPStan's own deprecation rule exempts

**Impact: Low-Medium · Complexity: Low-Medium**

```php
// (a) implementing a deprecated interface method by delegating to the same deprecated
// method on other instances — PHPStan doesn't flag its own required delegation,
// PHPantom does:
public function hasProperty(string $propertyName): TrinaryLogic
{
    return $this->unionResults(static fn (Type $type): TrinaryLogic => $type->hasProperty($propertyName));
}

// (b) accessing a constant on a deprecated class — PHPStan's deprecation rule doesn't
// fire on this either:
$val[Helpers::PREVENT_MERGING] = true;
```

(`src/Type/UnionType.php:656-659`, `src/Type/ObjectType.php`,
`src/Type/IntersectionType.php`, `src/Analyser/MutatingScope.php`,
`src/Reflection/ClassReflection.php`,
`src/Reflection/RequireExtension/RequireExtendsPropertiesClassReflectionExtension.php`,
`src/Type/Traits/LateResolvableTypeTrait.php` for shape (a);
`src/DependencyInjection/NeonAdapter.php` for shape (b).) All 24 `deprecated_usage`
diagnostics in this sweep match one of these two shapes, and none of the affected files
produce a PHPStan deprecation warning when analysed directly (confirmed by running
`vendor/bin/phpstan analyse` on each file listed above).
