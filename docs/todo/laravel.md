# PHPantom — Laravel

Known gaps and missing features in PHPantom's Laravel Eloquent support.
For the general architecture and virtual member provider design, see
`ARCHITECTURE.md`.

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## Out of scope (and why)

| Item | Reason |
|------|--------|
| Container string aliases | Requires booting the application. Use `::class` references instead. |
| Facade `getFacadeAccessor()` with string aliases | Requires booting the application. `@method static` tags provide a workable fallback. |
| Blade templates | Separate project. See `blade.md` for the implementation plan. |
| Model column types from DB/migrations | Unreasonable complexity. Require `@property` annotations (via ide-helper or hand-written). |
| Legacy Laravel versions | We target current Larastan-style annotations. Older code may degrade gracefully. |
| Application provider scanning | Low-value, high-complexity. |
| Macro discovery (`Macroable` trait) | Requires booting the application to inspect runtime `$macros` static property. `@method` tags provide a workable fallback. |
| Auth model from config | Requires reading runtime config (`config/auth.php`). Larastan boots the app for this. |
| Facade → concrete resolution via booting | Requires booting (`getFacadeRoot()`). When `getFacadeAccessor()` returns a `::class` reference, static resolution is possible without booting. See "Facade completion" section below. |
| Contract → concrete resolution | Requires container bindings at runtime. |
| Manager → driver resolution | Requires instantiating the manager at runtime. |

---

## Philosophy (unchanged)

- **No application booting.** We never boot a Laravel application to
  resolve types.
- **No SQL/migration parsing.** Model column types are not inferred from
  database schemas or migration files.
- **Larastan-style hints preferred.** We expect relationship methods to be
  annotated in the style that Larastan expects. Fallback heuristics
  are best-effort.
- **Facades prefer `getFacadeAccessor` over `@method`.** When a facade's
  `getFacadeAccessor()` returns a `::class` reference, we can resolve
  the concrete service class and present its instance methods as static
  methods on the facade with full type information (`@template`,
  conditional return types, etc.). `@method static` tags are a lossy
  fallback for IDEs that cannot perform this resolution.

---

## L1. Facade completion

Facades are the primary way Laravel developers interact with framework
services (`Cache::get(...)`, `DB::table(...)`, `Route::get(...)`, etc.).
The facade pattern works by forwarding static calls on the facade class
to instance methods on a concrete service class via `__callStatic()`.

Every facade stub ships with `@method static` tags that duplicate the
concrete class's public API in simplified form:

```php
/**
 * @method static \Illuminate\Contracts\Process\ProcessResult run(array|string|null $command = null, callable|null $output = null)
 * @method static \Illuminate\Process\InvokedProcess start(array|string|null $command = null, callable|null $output = null)
 * ...
 */
class Process extends Facade {
    protected static function getFacadeAccessor() { return \Illuminate\Process\Factory::class; }
}
```

Some facades return a `::class` reference (like `Process` above), while
others return a string alias (e.g. `Cache` returns `'cache'`). Only the
`::class` form is statically resolvable without booting the application.

### Current behaviour

Facade completion relies entirely on `@method static` tags from the
`PHPDocProvider`. These tags are simplified summaries that lose:

- `@template` parameters on the concrete class's methods.
- Conditional return types (`@return ($key is class-string ? T : mixed)`)
  flattened to a single type.
- Parameter defaults, variadic markers, and docblock descriptions from
  the real method signatures.

### Desired behaviour

When a facade's `getFacadeAccessor()` returns a `::class` reference
(statically resolvable without booting the application), we should
resolve the concrete service class and present its instance methods as
static methods on the facade. This preserves the full type information
from the concrete class. `@method static` tags should only be used as a
fallback when `getFacadeAccessor()` returns a string alias (which
requires the container and is out of scope).

The key transformation: the concrete class has **instance** methods, but
facades expose them as **static** calls. The provider must flip
`is_static = true` on the forwarded methods.

### Implementation notes

**Impact: High · Effort: Medium**

This would be a new virtual member provider (or an extension of the
Laravel provider) that:

1. Detects facade classes (extends `Illuminate\Support\Facades\Facade`
   or has a `getFacadeAccessor()` method).
2. Parses `getFacadeAccessor()` for a `::class` return value. String
   alias returns are ignored (require container resolution).
3. Loads the concrete service class via the class loader.
4. Collects its public instance methods and re-emits them as static
   virtual methods on the facade, preserving full signatures.
5. Runs at higher priority than the `PHPDocProvider` so that the rich
   resolved methods shadow the simplified `@method static` tags.

Edge cases to handle:
- Some facades override specific methods (e.g. `DB::connection()` has
  a different return type than the underlying manager). Real methods on
  the facade class should still take priority over forwarded ones.
- The concrete class may use `@template` at the class level. Generic
  substitution is not needed here since facades are singletons, but
  method-level `@template` should pass through unchanged.
- `getFacadeAccessor()` may return `$this->app->make(...)` or other
  dynamic expressions. Only the `return SomeClass::class;` pattern is
  statically resolvable.

---

## Model property source gaps

The `LaravelModelProvider` synthesizes virtual properties from several
sources on Eloquent models. The table below summarises what we handle
today and what is still missing.

### What we cover

| Source | Type info | Notes |
|--------|-----------|-------|
| `$casts` / `casts()` | Rich (built-in map, custom cast `get()` return type, enum, `Castable`, `CastsAttributes<TGet>` generics fallback) | |
| `$attributes` defaults | Literal type inference (string, bool, int, float, null, array) | Fallback when no `$casts` entry |
| `$fillable`, `$guarded`, `$hidden`, `$visible` | `mixed` | Last-resort column name fallback |
| Legacy accessors (`getXAttribute()`) | Method's return type | |
| Modern accessors (returns `Attribute`) | First generic arg of `Attribute<TGet>`, or `mixed` when unparameterised | |
| Relationship methods | Generic params or body inference | |
| Relationship `*_count` properties | `int` | `{snake_name}_count` for each relationship method |

### Gaps (ranked by impact ÷ effort)

---

#### L2. `morphedByMany` missing from relationship method map

**Impact: Low-Medium · Effort: Low**

Any model using `morphedByMany` (the inverse of a polymorphic
many-to-many) gets no virtual property or `_count` property for that
relationship. One-line addition to `RELATIONSHIP_METHOD_MAP`.

`morphedByMany` is the inverse side of a polymorphic many-to-many
relationship. It returns a `MorphToMany` instance (the same class as
`morphToMany`), but the method name is not listed in
`RELATIONSHIP_METHOD_MAP`. This means body inference
(`infer_relationship_from_body`) does not recognise
`$this->morphedByMany(Tag::class)` calls, so no virtual property or
`_count` property is synthesized.

**Where to change:** Add `("morphedByMany", "MorphToMany")` to
`RELATIONSHIP_METHOD_MAP` in `src/virtual_members/laravel.rs`.
No other changes needed since `MorphToMany` is already in
`COLLECTION_RELATIONSHIPS`.

#### L3. `$dates` array (deprecated)

**Impact: Low-Medium · Effort: Low**

Only affects legacy codebases that haven't migrated to `$casts`.
Decreasing relevance over time. New `extract_dates_definitions`
function in `parser/classes.rs` + merge logic in the provider at
lower priority than `$casts`.

Before `$casts`, Laravel used `protected $dates = [...]` to mark
columns as Carbon instances. This was deprecated in favour of
`casts()` with a `datetime` type, but older codebases still use it.
Columns listed in `$dates` should be typed as `\Carbon\Carbon`.

**Where to change:** Add a new `extract_dates_definitions` function in
`parser/classes.rs` (similar to `extract_column_names` but returning
`Vec<(String, String)>` with each column mapped to `\Carbon\Carbon`).
Merge these into `casts_definitions` at a lower priority than explicit
`$casts` entries, or add a separate field on `ClassInfo` and handle
priority in the provider.

#### L4. Custom Eloquent builders (`HasBuilder` / `#[UseEloquentBuilder]`)

**Impact: High · Effort: Medium**

Custom builders are the recommended pattern for complex query scoping
in modern Laravel. Without this, users get zero completions for
builder-specific methods via static model calls.

Laravel 11+ introduced the `HasBuilder` trait and
`#[UseEloquentBuilder(UserBuilder::class)]` attribute to let models
declare a custom builder class. When present, `User::query()` and
all static builder-forwarded calls should resolve to the custom
builder instead of the base `Illuminate\Database\Eloquent\Builder`.

```php
/** @extends Builder<User> */
class UserBuilder extends Builder {
    /** @return $this */
    public function active(): static { ... }
}

class User extends Model {
    /** @use HasBuilder<UserBuilder> */
    use HasBuilder;
}

User::query()->active()->get(); // active() should resolve on UserBuilder
```

Larastan handles this via `BuilderHelper::determineBuilderName()`,
which inspects `newEloquentBuilder()`'s return type or the
`#[UseEloquentBuilder]` attribute to find the custom builder class.

**Where to change:** In `build_builder_forwarded_methods`, before
loading the standard `Eloquent\Builder`, check whether the model
declares a custom builder via `@use HasBuilder<X>` in `use_generics`
or a `newEloquentBuilder()` method with a non-default return type.
If found, load and resolve that builder class instead.

#### L5. `abort_if`/`abort_unless` type narrowing

**Impact: High · Effort: Medium**

These are the standard guard patterns in Laravel controllers and
middleware. Without narrowing, variables keep their wider type,
causing false "unknown member" warnings and missing completions.

After `abort_if($user === null, 404)`, the type of `$user` should
be narrowed to exclude `null` in subsequent code.  Similarly,
`abort_unless($user instanceof Admin, 403)` should narrow `$user`
to `Admin`.

```php
abort_if($user === null, 404);
$user->email;  // $user should be non-null here

abort_unless($user instanceof Admin, 403);
$user->grantPermission('edit');  // $user should be Admin here
```

Larastan handles this via `AbortIfFunctionTypeSpecifyingExtension`,
a PHPStan-specific `TypeSpecifyingExtension` mechanism.  The
framework does **not** annotate these functions with
`@phpstan-assert` — there are no stubs for this either.

Our guard clause narrowing already handles the pattern
`if ($x === null) { return; }` + subsequent code, and we support
`@phpstan-assert-if-true/false`.  However, `abort_if` / `abort_unless`
/ `throw_if` / `throw_unless` don't follow either pattern: they are
standalone function calls (not if-conditions) that conditionally
throw.

**Where to change:** In `type_narrowing.rs`, add special-case
handling for standalone `abort_if()`, `abort_unless()`, `throw_if()`,
and `throw_unless()` calls.  When the first argument is a type check
expression (instanceof, `=== null`, etc.), apply the inverse narrowing
to subsequent code:
- `abort_if($x === null, ...)` → narrow `$x` to non-null after
- `abort_unless($x instanceof Foo, ...)` → narrow `$x` to `Foo` after
- `throw_if(...)` / `throw_unless(...)` → same logic

This is similar to the existing guard clause narrowing but triggered
by specific function names rather than `if` + early return.

#### L6. Factory `has*`/`for*` relationship methods

**Impact: Low-Medium · Effort: Medium**

Convenience for factory-heavy test suites. Without this, no completion
after `->has` or `->for` on factory instances.

Laravel's `Factory` class supports dynamic `has{Relationship}()` and
`for{Relationship}()` calls via `__call()`.  For example,
`UserFactory::new()->hasPosts(3)` checks that `posts` is a valid
relationship on the `User` model, and
`UserFactory::new()->forAuthor($state)` delegates to the `for()`
method.

```php
UserFactory::new()->hasPosts(3)->create();     // works at runtime
UserFactory::new()->forAuthor(['name' => 'J'])->create(); // works at runtime
```

The framework has no `@method` annotations for these — they are
purely `__call` magic.  Larastan handles this in
`ModelFactoryMethodsClassReflectionExtension`, which inspects the
factory's `TModel` template type, checks whether the camelCase
remainder (after stripping `has`/`for`) is a valid relationship
method, and synthesizes the method reflection dynamically.

Our `LaravelFactoryProvider` currently only synthesizes `create()`
and `make()` methods.

**Where to change:** In `LaravelFactoryProvider::provide`, after
synthesizing `create()`/`make()`, load the associated model class.
For each relationship method on the model, push a `has{Relationship}`
and `for{Relationship}` virtual method (PascalCase of the method
name) that returns `static` (i.e. the factory class itself).
The `has*` variant should accept optional `int $count` and
`array|callable $state` parameters; `for*` should accept
`array|callable $state`.

#### L7. `$pivot` property on BelongsToMany related models

**Impact: Medium · Effort: Medium-High**

Pivot access is common in apps with many-to-many relationships.
However, Larastan doesn't handle this either, and `@property` on
custom Pivot classes covers most needs.

When a model is accessed through a `BelongsToMany` (or `MorphToMany`)
relationship, each related model instance gains a `$pivot` property at
runtime that provides access to intermediate table columns.

```php
/** @return BelongsToMany<Role, $this> */
public function roles(): BelongsToMany {
    return $this->belongsToMany(Role::class)->withPivot('expires_at');
}

$user->roles->first()->pivot;           // Pivot instance — we know nothing about it
$user->roles->first()->pivot->expires_at; // accessible at runtime, invisible to us
```

There are several layers of complexity here:

1. **Basic `$pivot` property.** Related models accessed through a
   `BelongsToMany` or `MorphToMany` relationship should have a `$pivot`
   property typed as `\Illuminate\Database\Eloquent\Relations\Pivot`
   (or the custom pivot class when `->using(CustomPivot::class)` is
   used). We don't currently synthesize this property at all.

2. **`withPivot()` columns.** The `withPivot('col1', 'col2')` call
   declares which extra columns are available on the pivot object.
   Tracking these requires parsing the relationship method body for
   chained `withPivot` calls — similar in difficulty to the
   `withCount` call-site problem (gap 5).

3. **Custom pivot models (`using()`).** When `->using(OrderItem::class)`
   is declared, the pivot is an instance of that custom class, which
   may have its own properties, casts, and accessors. Detecting this
   requires parsing the `->using()` call in the relationship body.

Note: Larastan does **not** handle pivot properties either — the
`$pivot` property comes from Laravel's own `@property` annotations on
the `BelongsToMany` relationship stubs. If the user's stub set
includes these annotations, it already works through our PHPDoc
provider.

#### L8. `withSum()` / `withAvg()` / `withMin()` / `withMax()` aggregate properties

**Impact: Low-Medium · Effort: Medium-High**

Less common than `withCount`; only affects codebases using aggregate
eager-loading. Cannot be inferred declaratively from the model alone;
requires tracking call-site string arguments.

Similar to `withCount`, these aggregate methods produce virtual
properties named `{relation}_{function}` (e.g.
`Order::withSum('items', 'price')` → `$order->items_sum`). The same
call-site tracking challenge applies, and the type depends on the
aggregate function (`withSum`/`withAvg` → `float`,
`withMin`/`withMax` → `mixed`).

The `@property` workaround applies here too.

#### L9. Higher-order collection proxies

**Impact: Low-Medium · Effort: Medium-High**

Convenience syntax; most users prefer closures. Niche usage. Requires
synthesizing virtual properties on collection classes that return a
proxy type parameterised with the collection's value type.

Laravel collections support higher-order proxies via magic properties
like `$users->map->name` or `$users->filter->isActive()`. These
produce a `HigherOrderCollectionProxy` that delegates property
access / method calls to each item in the collection.

```php
$users->map->email;           // Collection<int, string>
$users->filter->isVerified(); // Collection<int, User>
$users->each->notify();       // void (side-effect)
```

Larastan handles this with `HigherOrderCollectionProxyPropertyExtension`
and `HigherOrderCollectionProxyExtension`, which resolve the proxy's
template types and delegate property/method lookups to the collection's
value type.

#### L10. `View::withX()` and `RedirectResponse::withX()` dynamic methods

**Impact: Low · Effort: Low**

Most code uses `->with('key', $value)` instead of the dynamic
`->withKey($value)` form. Explicitly declared methods (`withErrors`,
`withInput`, etc.) already work.

Both `Illuminate\View\View` and `Illuminate\Http\RedirectResponse`
support dynamic `with*()` calls via `__call()`.  For example,
`view('home')->withUser($user)` is equivalent to
`->with('user', $user)`.

```php
view('home')->withUser($user);         // dynamic, no @method annotation
redirect('/')->withErrors($errors);    // has explicit withErrors(), but withFoo() is dynamic
```

The framework provides no `@method` annotations for arbitrary
`with*` calls — only specific ones like `withErrors()`,
`withInput()`, `withCookies()` etc. are declared as real methods.
Larastan handles the dynamic case in
`ViewWithMethodsClassReflectionExtension` and
`RedirectResponseMethodsClassReflectionExtension`, which treat any
`with*` call as valid and returning `$this`.

**Where to change:** This could be handled with a lightweight
virtual member provider that detects classes with a `__call` method
whose body checks `str_starts_with($method, 'with')`, or by
hard-coding the two known classes.  A simpler approach: add
`@method` tags to bundled stubs for the most common dynamic `with*`
methods, or document this as a known limitation.

#### L11. `$appends` array

**Impact: Low · Effort: Low**

The accessor method is the real source of truth; `$appends` only
helps when the accessor is defined in an unloaded parent class.
Similar to `$fillable`/`$hidden` extraction.

The `$appends` property lists accessor names that should always be
included in `toArray()` / `toJson()`. These reference existing
accessors, so in most cases the accessor method itself already produces
the virtual property. Parsing `$appends` would only help when the
accessor is defined in an unloaded parent class.

