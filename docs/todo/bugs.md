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

### B181. `array_filter()` reports a `list` the filter cannot preserve

**Impact: Low-Medium · Complexity: Medium**

`array_filter()` keeps the key of every entry it keeps, so filtering a
`list` leaves gaps in the numbering and the result is `array<int, T>`
rather than `list<T>`. PHPantom hands back the container it was given:

```php
/** @param list<int> $values */
function probe(array $values): void {
    $kept = array_filter($values, fn ($v) => $v > 3);  // reported as list<int>
    $kept[0];  // [3, 4] filtered this way starts at key 1, so this is unset
}
```

The over-claim runs both ways: reading `$kept[0]` looks safe when it is
not, and a function declared `@return list<int>` that hands back an
unwrapped `array_filter()` result is accepted where PHPStan reports it.
The rule that rebuilds the container for the preserving family lives in
`type_engine/variable/array_func_rules.rs`; `array_filter` needs to drop
to `array<int, T>` there while the renumbering functions around it
(`array_values`, `array_merge`) keep answering `list<T>`, and the demo
files and tests that currently assert `list<…>` for a filtered list need
updating with it.

## Docblock handling

No outstanding items.

## Miscellaneous

No outstanding items.
