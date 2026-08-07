# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B28. Route names built in a loop are reported as unknown

**Impact: Medium · Effort: Medium**

A route file that registers one route per entry of a literal array names
each of them by interpolation, so the name is not a plain string literal
and `collect_names_from_file`
(`src/virtual_members/laravel/route_names.rs`) records nothing for it.
Every `route('…')` call naming one is then flagged
`invalid_laravel_route`:

```php
$events = ['black-friday' => ['perfume-her', 'k-beauty'], 'valentines' => ['perfume']];

foreach ($events as $event => $subcategories) {
    Route::get("/{$event}", [EventsController::class, 'landing'])
        ->name("events.{$event}.landing");

    foreach ($subcategories as $subcategory) {
        Route::get("/{$event}/{$subcategory}", [EventsController::class, 'sub'])
            ->name("events.{$event}.{$subcategory}");
    }
}
```

The loop bodies are walked (the registrations are seen), but the names
are not evaluated. The values are all statically known here: the array is
a literal, the loop variable is bound to each element in turn, and the
name is a concatenation of literals and that variable.

**Where to look:** the collector needs a small constant evaluator for
route-name expressions — bind the `foreach` key/value variables to each
literal element of a literal array, then fold interpolated and
concatenated strings against those bindings. Anything not statically
known must still yield no name rather than a partial one. Nested loops
compose, as above. `slug('/xmas/gift-sets')` and other function calls in
the array are not evaluable and should contribute nothing.

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
