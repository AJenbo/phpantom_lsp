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

### B96. A docblock `@param` type narrower than its native nullable type hint is not flagged

**Impact: Low · Effort: Medium**

```php
/**
 * @param string $name
 */
function greet(?string $name): void {}

/**
 * @param list<int> $items
 */
function takesItems(?array $items): void {}
```

The native type hint (`?string`, `?array`) is nullable, but the docblock says
a narrower type (`string`, `list<int>`) that does not admit `null` — a caller
following the native signature can pass `null` and violate the docblock's own
contract. PHPantom has no diagnostic for a docblock parameter/return type
that is narrower than the native type hint it annotates, for either a scalar
or an array type. Only Qodana flags either case in
`php-typing-conformance`'s corpus.

**Fix:** not investigated; would need a new check (there is no existing
"docblock narrower than native hint" diagnostic to extend) comparing each
documented parameter/return type against its native type hint for
compatibility, most likely reusing the existing type-compatibility check
rather than a new one.
