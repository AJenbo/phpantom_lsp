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

No outstanding items.
