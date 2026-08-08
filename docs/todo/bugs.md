# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B49. An unannotated callback body that is a method call binds no template

**Impact: Medium · Effort: Medium**

When a callback has no return-type annotation, the template bound from
its return type is inferred from the body expression. A body that is a
*call* resolves to `mixed` (the call-return step is missing from
argument-text resolution), so the template stays unbound and falls back
to its declared bound:

```php
$byRating = $reviews->keyBy(fn (Review $r) => $r->getRating());
foreach ($byRating as $key => $review) { … }
// $key is array-key|\UnitEnum, should be int
```

A body that is a variable, a literal, or a `new` expression binds
correctly, so this is specifically the call case.
