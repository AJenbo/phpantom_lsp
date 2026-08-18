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

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

### B180. A deprecation is reported against a same-named variable from another method

**Impact: Low-Medium · Complexity: Medium**

The `deprecated_usage` diagnostic types its subject from a scope that is
not the one the call sits in, so a parameter name reused across two
methods of the same class picks up whichever type was bound first:

```php
class Probe
{
    public function fromHttpRequest(\Illuminate\Http\Request $request): void
    {
        $request->input('name');
    }

    public function fromHttpClient(\Illuminate\Http\Client\PendingRequest $request): void
    {
        // Reported as `Illuminate\Http\Request::get is deprecated`, though
        // `$request` here is a PendingRequest and its `get()` is not
        // deprecated at all.
        $request->get('https://example.com');
    }
}
```

Putting the `PendingRequest` method first makes the report disappear,
and so does renaming either parameter, which is what identifies the leak
as name-keyed and order-dependent rather than a mis-resolution of
`PendingRequest`. Every other diagnostic on the same line resolves
`$request` correctly, so the shared forward walker is right and the
deprecation check is reading a stale or class-wide variable map instead
of the per-method scope. The fix is to resolve the subject through the
same scope the surrounding diagnostics use.
