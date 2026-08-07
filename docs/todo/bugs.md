# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B30. `$this` inside a macro closure resolves to the enclosing class

**Impact: Medium · Effort: Low-Medium**

Laravel binds a macro's closure to the target, so `$this` inside
`Route::macro('auth', function () { … })` is the router, not the service
provider the registration is written in. Completion knows this;
diagnostics do not, so every member call on `$this` in a macro body is
reported as unknown:

```php
class RouteServiceProvider extends ServiceProvider {
    public function boot(): void {
        Route::macro('auth', function (): void {
            // Method 'get' not found on class 'App\Providers\RouteServiceProvider'
            $this->get('login', fn () => view('welcome'))->name('login');
        });
    }
}
```

This is the shape `laravel/ui` writes `Route::auth()` in, so any project
with a macro that registers on `$this` gets a diagnostic per line of the
macro body.

**Where to look:** `ResolutionCtx::laravel_macro_this_resolver` is the
hook that answers this, and `closure_this_from_static_receiver`
(`src/type_engine/variable/closure_resolution.rs`) already consumes it.
Only `member_access.rs` supplies one; every other construction site
passes `None`, including the unknown-member and deprecated-usage
diagnostic passes and the hover and go-to-definition paths. The resolver
is the same three lines in each case (load the written target, map a
facade to its concrete class via `facade_macro_concrete`), so it belongs
on the `Backend` rather than being rebuilt per consumer.

#### B31. Route names built with a string function are reported as unknown

**Impact: Low · Effort: Low-Medium**

The route-name evaluator
(`src/virtual_members/laravel/const_eval.rs`) folds literals,
interpolation, concatenation and the `foreach` bindings around them, but
it stops at any call.  A loop that normalizes its element before naming
the route therefore contributes nothing:

```php
foreach ($subcategories as $subcategory) {
    $slug = preg_replace('#^/xmas/#', '', $subcategory);

    Route::get('/xmas/' . $slug, [EventsController::class, 'page'])
        ->name('events.xmas.' . $slug);
}
```

`$slug` is unknown, so the name is too, and every `route('events.xmas.…')`
naming one is flagged `invalid_laravel_route`.

**Where to look:** `const_value` could fold a short list of pure string
functions whose result is fully determined by constant arguments —
`str_replace`, `preg_replace`, `trim`/`ltrim`/`rtrim`,
`strtolower`/`strtoupper`, `sprintf`, `implode`, `ucfirst`.  Only fold
when every argument is already a `ConstValue::Scalar`; a call with an
unknown argument, or any function not on the list, must stay
`ConstValue::Unknown` so a partial name is never invented.  `preg_replace`
means running a real regex, so it is the one that needs care: PHP's
delimiters and modifiers are not the `regex` crate's syntax, and an
unsupported pattern has to fall back to unknown rather than a wrong
substitution.
