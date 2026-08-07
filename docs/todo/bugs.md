# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B40. Assignments in the inline `@php(…)` directive are not recorded

**Impact: Medium · Effort: Low**

The block form updates scope correctly; the inline form does not:

```blade
@php($a = $order->orderProducts->whereIn('product_id', [1]))
{{ $a->isNotEmpty() }}   {{-- type of '$a' could not be resolved --}}

@php
    $b = $order->orderProducts->whereIn('product_id', [1]);
@endphp
{{ $b->isNotEmpty() }}   {{-- resolves --}}
```

#### B41. Translation-key diagnostics fire when the app replaces the translation loader

**Impact: Medium-High · Effort: Low-Medium**

The Trans arm skips the check when no translation files were found, but
an application that keeps its strings in the database still has
`vendor/`'s own `lang/` files on disk, so the set is non-empty and every
application key is reported as unknown.

```php
$this->app->singleton('translation.loader', fn ($app) => new DatabaseTranslationLoader(
    new FileLoader($app->make('files'), $app->make('path.lang')),
));
```

28 diagnostics in one sample project, none of them real.

**Fix:** when a service provider rebinds `translator` or
`translation.loader` to something other than Laravel's own `FileLoader`,
the valid-key set is unknowable — skip the check, the same way an
unenforced morph map does.

#### B43. `App::make()` / `App::makeWith()` with a class-string do not resolve

**Impact: Medium-High · Effort: Low**

The `app()` helper resolves a class-string argument to that class; the
`App` facade does not:

```php
app(CurrencyHelper::class)->noSuchMethod();          // resolves ✓
app()->make(CurrencyHelper::class)->noSuchMethod();  // resolves ✓
App::make(CurrencyHelper::class)->noSuchMethod();    // could not be resolved ✗
App::makeWith(CurrencyHelper::class, [])->…          // could not be resolved ✗
```

8 diagnostics in one sample project. Whatever gives the helper its
class-string return needs to apply to the facade's `make`, `makeWith`
and `resolve` too.

#### B44. String container bindings do not resolve

**Impact: Low-Medium · Effort: Medium**

`app()->make('sentry')` resolves to nothing, because nothing indexes the
string keys that service providers bind:

```php
// Sentry\Laravel\ServiceProvider
$this->app->singleton('sentry', fn () => new HubAdapter());
```

Provider scanning already walks `register()` for config and route
resources, so the `bind()` / `singleton()` / `instance()` calls with a
string abstract and a resolvable concrete are within reach.

#### B47. Deprecation diagnostics ignore the project's target PHP version

**Impact: Medium · Effort: Low-Medium**

A project pinned to PHP 8.4 (`"require": {"php": "^8.4"}`,
`"config": {"platform": {"php": "8.4"}}`) is reported for a deprecation
introduced in 8.5:

```
'Pdo\PDO::sqliteCreateFunction' is deprecated: use Pdo\Sqlite::createFunction
instead (since PHP 8.5)
```

6 diagnostics in one sample project. The stub already carries the
version the deprecation landed in; it should be compared against the
project's resolved target version, and the diagnostic suppressed when
the target predates it.

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

#### B49. Static call through a string-typed variable is reported as scalar access

**Impact: Medium · Effort: Low**

`$string::method()` is valid PHP — the string is the class name — but it
is reported as a member access on a scalar:

```php
$job->class_name::dispatch();
// Cannot access method 'dispatch' on type 'string'
```

A `class-string<T>` subject should resolve to `T`; a plain `string`
subject is unresolvable, which is a "cannot verify" at most, never a
scalar-access error.

#### B50. Closure parameter types are not narrowed from the call site

**Impact: Medium · Effort: Medium**

When a closure is passed to `array_map()` (and friends) over an array
with a known element type, that element type should refine a wider
declared parameter type. PHPStan does this at level max; PHPantom keeps
the declared `array`:

```php
/** @return iterable<array{DiscountType, ?CartLabel}> */
private static function yieldCases(): iterable { /* … */ }

array_map(
    static fn (array $case): string => $case[0]->name,   // type of '$case[0]' could not be resolved
    iterator_to_array(self::yieldCases()),
);
```

3 diagnostics in one sample project. Related to
[T25](type-inference.md#t25-call-site-template-argument-inference-for-callable-parameters),
which covers the template side of the same call-site inference.
