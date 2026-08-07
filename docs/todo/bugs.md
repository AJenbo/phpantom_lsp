# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B48. `Collection::keyBy()` does not rebind the key template

**Impact: Medium · Effort: Medium**

`keyBy()` re-keys a collection, so its result is
`Collection<TNewKey, TValue>` where `TNewKey` comes from the callback's
return type (or the type of the named column). PHPantom keeps the
original `int` key from the Eloquent collection, so every subsequent
`get()` with the new key is reported:

```php
$byMarket = ProductPrice::query()->get()
    ->keyBy(fn (ProductPrice $pp): string => $pp->market->value);

$byMarket->get($translation->lang_code->value);
// Argument 1 ($key) expects int|null, got string
```

3 diagnostics across two sample projects (one of them through a Blade
view that receives the collection).
