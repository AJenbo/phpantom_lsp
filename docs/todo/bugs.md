# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B65. A `@var` whose type is a closure signature binds the wrong variable

**Impact: Low-Medium · Effort: Low**

A closure type writes `$`-prefixed names for its own parameters, and the
`@var` scan takes the first `$` it finds after the tag as the annotated
variable:

```php
/** @var \Closure(\App\Models\User $user): string $callback */
```

is read as declaring `$user` of type `\Closure(\App\Models\User`, so
`$callback` stays untyped and a bogus `$user` enters the scope. The same
shape appears in `@param` and in a Blade template's signature docblock,
where it also decides which names the contract declares.

**Where to look:** `parse_var_docblock_pairs` in
`type_engine/variable/forward_walk/assignment.rs` scans for the first
`$` after `@var`. The annotated variable is the `$name` at paren depth 0
and angle depth 0, so the scan has to track both while walking the type
rather than stopping at the first `$`.
