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

#### B29. `Route::auth()` and other route macros register no names

**Impact: Low · Effort: Medium**

`laravel/ui` registers a `Route::auth()` macro whose body declares
`login`, `logout`, `register`, `password.request`, `password.email`,
`password.reset`, and `password.update`. A route file calling
`Route::auth()` therefore has those names, but the route collector only
reads registrations written literally in a route source, so every
`route('password.update')` is flagged `invalid_laravel_route`.

The macro index (`build_laravel_macro_index`) already knows where the
macro body lives. The route collector does not consult it.

**Where to look:** `collect_names_from_expr`
(`src/virtual_members/laravel/route_names.rs`) sees the `Route::auth()`
static call and falls through. Resolving the call against the macro
index and walking the registered closure's body — with the group prefix
in force at the call site — would pick the names up the same way an
inline `Route::group(…, function () { … })` is picked up. Note the macro
body registers on `$this` (the router) rather than the `Route` facade.
