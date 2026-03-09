# PHPantom — Laravel Support: Remaining Work


This document tracks bugs, known gaps, and missing features in
PHPantom's Laravel Eloquent support. For the general architecture and
virtual member provider design, see `ARCHITECTURE.md`.

---

## Out of scope (and why)

| Item | Reason |
|------|--------|
| Container string aliases | Requires booting the application. Use `::class` references instead. |
| Facade `getFacadeAccessor()` with string aliases | Same problem. `@method` tags provide a workable fallback. |
| Blade templates | Separate project. See `todo-blade.md` for the implementation plan. |
| Model column types from DB/migrations | Unreasonable complexity. Require `@property` annotations (via ide-helper or hand-written). |
| Legacy Laravel versions | We target current Larastan-style annotations. Older code may degrade gracefully. |
| Application provider scanning | Low-value, high-complexity. |
| Macro discovery (`Macroable` trait) | Requires booting the application to inspect runtime `$macros` static property. `@method` tags provide a workable fallback. |
| Auth model from config | Requires reading runtime config (`config/auth.php`). Larastan boots the app for this. |
| Facade → concrete resolution | Requires booting (`getFacadeRoot()`). `@mixin` on facade stubs handles most cases. |
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
- **Facades fall back to `@method`.** Facades whose `getFacadeAccessor()`
  returns a string alias cannot be resolved. `@method` tags on facade
  classes provide completion without template intelligence.

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

Each gap now carries an **Impact** rating (how many users / codebases
benefit) and an **Effort** estimate (implementation complexity):

| Rating | Impact meaning | Effort meaning |
|--------|---------------|----------------|
| ★ | Rare / niche | Trivial — a few lines |
| ★★ | Occasionally useful | Small — one function / field |
| ★★★ | Common pattern | Moderate — touches 2-3 modules |
| ★★★★ | Very common | Significant — new subsystem or cross-cutting |
| ★★★★★ | Nearly every codebase | Large — new infrastructure + plumbing |

---

#### 1. `morphedByMany` missing from relationship method map

| | |
|---|---|
| **Impact** | ★★ — Any model using `morphedByMany` (the inverse of a polymorphic many-to-many) gets no virtual property or `_count` property for that relationship. |
| **Effort** | ★ — One-line addition to `RELATIONSHIP_METHOD_MAP` in `virtual_members/laravel.rs`. |

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

#### 2. `$dates` array (deprecated)

| | |
|---|---|
| **Impact** | ★★ — Only affects legacy codebases that haven't migrated to `$casts`. Decreasing relevance over time. |
| **Effort** | ★★ — New `extract_dates_definitions` function in `parser/classes.rs` + merge logic in the provider at lower priority than `$casts`. |

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

#### 3. Custom Eloquent builders (`HasBuilder` / `#[UseEloquentBuilder]`)

| | |
|---|---|
| **Impact** | ★★★★ — Custom builders are the recommended pattern for complex query scoping in modern Laravel. Without this, users get zero completions for builder-specific methods via static model calls. |
| **Effort** | ★★★ — In `build_builder_forwarded_methods`, detect `@use HasBuilder<X>` / `newEloquentBuilder()` return type, load the custom builder class, and resolve it instead of the standard `Eloquent\Builder`. |

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

#### 4. `abort_if`/`abort_unless` type narrowing

| | |
|---|---|
| **Impact** | ★★★★ — These are the standard guard patterns in Laravel controllers and middleware. Without narrowing, variables keep their wider type, causing false "unknown member" warnings and missing completions. |
| **Effort** | ★★★ — Special-case handling in `type_narrowing.rs` for specific function names; reuses existing guard-clause narrowing logic but triggered differently. |

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

#### 5. `collect()` and other helper functions lose generic type info

| | |
|---|---|
| **Impact** | ★★★★★ — `collect()` alone is used in virtually every Laravel codebase. Loss of element types breaks completion chains on resulting collections. Also affects `value()`, `retry()`, `tap()`, `with()`, `transform()`, `data_get()`, and non-Laravel functions. |
| **Effort** | ★★★★ — Requires adding `template_params` / `template_bindings` to `FunctionInfo`, populating from `@template`/`@param` in `parser/functions.rs`, and building substitution maps in `resolve_rhs_function_call`. New infrastructure. |

Laravel's `collect()` helper is annotated with function-level
`@template` parameters:

```php
/**
 * @template TKey of array-key
 * @template TValue
 * @param array<TKey, TValue> $value
 * @return \Illuminate\Support\Collection<TKey, TValue>
 */
function collect($value = []) { ... }
```

We correctly resolve the return type as `Collection`, but the
generic arguments `TKey` and `TValue` are lost — the result is an
unparameterised `Collection`, so `$users = collect($array)` followed
by `$users->first()->` produces no completions for the element type.

**Root cause:** `FunctionInfo` has no `template_params` or
`template_bindings` fields (unlike `MethodInfo`, which has both).
The `synthesize_template_conditional` function only handles the
narrow pattern `@return T` where `T` is a bare template param bound
via `@param class-string<T>`.  It does **not** handle `@return
Collection<TKey, TValue>` where multiple template params appear
inside a generic return type.

This affects every Laravel helper that uses function-level generics:
`collect()`, `value()`, `retry()`, `tap()`, `with()`, `transform()`,
`data_get()`, plus non-Laravel functions with the same pattern.

**Where to change:** Add `template_params: Vec<String>` and
`template_bindings: Vec<(String, String)>` to `FunctionInfo` (mirror
the existing fields on `MethodInfo`).  Populate them in
`parser/functions.rs` from `@template` and `@param` annotations.
In `resolve_rhs_function_call` (in `variable_resolution.rs`), after
loading the `FunctionInfo`, build a substitution map from template
bindings → call-site argument types and apply it to the return type
before passing it to `type_hint_to_classes`.  See the general TODO
item (§ PHP Language Feature Gaps, "Function-level `@template`
generic resolution") for the full implementation plan.

#### 6. Factory `has*`/`for*` relationship methods

| | |
|---|---|
| **Impact** | ★★ — Convenience for factory-heavy test suites. Without this, no completion after `->has` or `->for` on factory instances. |
| **Effort** | ★★★ — Load associated model in `LaravelFactoryProvider::provide`, iterate relationship methods, synthesize `has{Rel}` / `for{Rel}` virtual methods with correct signatures. |

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

#### 7. `$pivot` property on BelongsToMany related models

| | |
|---|---|
| **Impact** | ★★★ — Pivot access is common in apps with many-to-many relationships. However, Larastan doesn't handle this either, and `@property` on custom Pivot classes covers most needs. |
| **Effort** | ★★★★ — Multi-layered: basic `$pivot` typed as `Pivot` is easy, but `withPivot()` columns and `using()` custom pivot classes require relationship body parsing we don't currently do. |

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

#### 8. `withSum()` / `withAvg()` / `withMin()` / `withMax()` aggregate properties

| | |
|---|---|
| **Impact** | ★★ — Less common than `withCount`; only affects codebases using aggregate eager-loading. |
| **Effort** | ★★★★ — Cannot be inferred declaratively from the model alone; requires tracking call-site string arguments to `withSum()`/etc. |

Similar to `withCount`, these aggregate methods produce virtual
properties named `{relation}_{function}` (e.g.
`Order::withSum('items', 'price')` → `$order->items_sum`). The same
call-site tracking challenge applies, and the type depends on the
aggregate function (`withSum`/`withAvg` → `float`,
`withMin`/`withMax` → `mixed`).

The `@property` workaround applies here too.

#### 9. Higher-order collection proxies

| | |
|---|---|
| **Impact** | ★★ — Convenience syntax; most users prefer closures. Niche usage. |
| **Effort** | ★★★★ — Requires synthesizing virtual properties on collection classes that return a proxy type parameterised with the collection's value type. Complex proxy delegation. |

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

#### 11. `View::withX()` and `RedirectResponse::withX()` dynamic methods

| | |
|---|---|
| **Impact** | ★ — Most code uses `->with('key', $value)` instead of the dynamic `->withKey($value)` form. Explicitly declared methods (`withErrors`, `withInput`, etc.) already work. |
| **Effort** | ★★ — Could hard-code the two known classes or add `@method` tags to bundled stubs. |

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

#### 12. `$appends` array

| | |
|---|---|
| **Impact** | ★ — The accessor method is the real source of truth; `$appends` only helps when the accessor is defined in an unloaded parent class. |
| **Effort** | ★ — Similar to `$fillable`/`$hidden` extraction. |

The `$appends` property lists accessor names that should always be
included in `toArray()` / `toJson()`. These reference existing
accessors, so in most cases the accessor method itself already produces
the virtual property. Parsing `$appends` would only help when the
accessor is defined in an unloaded parent class.

