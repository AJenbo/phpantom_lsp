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

### B83. A partially-compatible union argument is not reported
**Impact: Medium · Effort: Low-Medium**

Passing a union argument where only *some* members satisfy the declared
parameter type is accepted silently. The check appears to ask whether any
member of the argument union is compatible, where it should ask whether
every member is.

```php
/** @return 1|99 */
function gives() { return 1; }

/** @param 1|10 $level */
function acceptsLevel(int $level): void {}

acceptsLevel(gives());   // missed: 99 does not satisfy 1|10
acceptsLevel(98|99);     // reported correctly (no member overlaps)
```

A zero-overlap union (`98|99` against `1|10`) is reported, so the union
handling runs, it just stops at the first compatible member. The same
laxness applies to any union source, not just literal unions: an
`int|string` argument passed to an `int` parameter is a real error PHP
will raise at runtime for the `string` case.

Fix: change the union-argument compatibility check so every member of
the argument type must satisfy the parameter type, and report the
offending members in the message.
