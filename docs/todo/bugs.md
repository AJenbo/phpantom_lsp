# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

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
