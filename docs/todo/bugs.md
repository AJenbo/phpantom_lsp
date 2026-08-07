# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B33. Included route file paths behind a local variable are not resolved

**Impact: Low · Effort: Medium**

`resolve_path_arg` (`src/virtual_members/laravel/provider_resources.rs`)
now follows a `Variable` argument back to its most recent assignment in
the enclosing method when resolving `mergeConfigFrom`/`loadViewsFrom`/
`loadTranslationsFrom`/`loadRoutesFrom` paths, but `route_names.rs`'s
`open_included_file` (which resolves `require __DIR__ . '/x.php'` and
`Route::group([], base_path('x.php'))` targets while following include
chains during route scanning) calls `resolve_path_arg` with `program:
None`, so the same local-variable indirection is still unresolved there:

```php
$routes = __DIR__.'/../routes/api.php';
Route::group(['prefix' => 'v1'], $routes);
```

`open_included_file` never opens `routes/api.php`, so any route defined
in it is invisible to `route()` / route-name completion.

**Where to look:** `open_included_file` would need the already-parsed
`Program` threaded through `scan_route_file` → `scan_stmt` → `scan_expr`
→ `scan_included_file` (each currently receives only `content` and
`ScanPaths`), which is a larger, more invasive change than the
provider-resources fix since `Program` does not currently travel through
that call chain.
