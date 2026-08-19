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

### B196. The chain resolution cache key omits file identity

**Impact: Low-Medium · Complexity: Medium**

The chain cache in `type_engine/resolver/mod.rs` keys a variable-free
subject chain by its bare text (`expr.to_subject_text()`), with no file
identity in the key, while some cache activations span multiple files
(e.g. the reference-counts pending-item loop, and the request-level
guard used by find-references/rename while walking other files). Two
files that each `use` a different class under the same alias and spell
the same method-chain text can have the second file's resolution
poisoned by the first file's cached entry:

```php
// file A: use A\Pen;           file B: use B\Pen;
Pen::make()->write();           Pen::make()->write();  // may resolve against A\Pen
```

### B218. `new ReflectionProperty(Foo::class, 'bar')` forgets what it reflects

**Impact: Low · Complexity: Medium**

A reflection value built by `ReflectionClass::getProperty('bar')` carries
the class and the property name, so reading it types as the property
declares. Constructing the same value directly does not:

```php
$viaClass = (new \ReflectionClass(Configuration::class))->getProperty('shell');
$viaClass->getValue($config);   // ?Shell

$direct = new \ReflectionProperty(Configuration::class, 'shell');
$direct->getValue($config);     // mixed
```

The two spellings are interchangeable in real code, so the second should
resolve like the first. The binding cannot come from the constructor's
docblock: `class-string<T>|T $class` would bind the class through the
existing machinery, but the `$property` name is a string literal, and a
literal only binds to a `@template` whose bound is a type operator
(`key-of<…>` and friends). Either the two `new`-expression resolution
paths need the same rule the two call paths got, or literal binding has
to be widened to a `@template TName of string`, which is what PHPStan
does for literal string types and would want measuring against the whole
corpus first.

### B183. A Laravel Folio route is reported as unknown

**Impact: High · Complexity: Medium**

Folio registers routes from the filesystem: a page file under a mounted
directory becomes a route, and `Laravel\Folio\name()` inside the page
names it. The route index only reads registrations it can see in
`routes/`, service providers, and resource declarations, so a Folio
route does not exist as far as any consumer is concerned. The worst
symptom is a false positive:

```php
// resources/views/folio/explore/index.blade.php
use function Laravel\Folio\name;
name('explore');

// anywhere else
route('explore');  // Unknown route: 'explore'
```

Three more follow from the same cause: go-to-definition on the name
answers nothing, completion inside `route('')` omits every Folio route,
and hover falls back to the bare `Route name` label instead of naming
the file that declares it.

The mount points come from `Folio::path(...)` / `Folio::route(...)` calls
in a service provider (`FolioServiceProvider` by convention), and the
name comes from a `name()` call in the page file, imported as
`use function Laravel\Folio\name;`. Both need reading before the route
index can answer for these names. The URI a page maps to is derived from
its path relative to the mount, with `[param]` and `[...param]` segments
becoming route parameters, which is also what route-parameter completion
would need.

Reproduced against a Folio-based Laravel application; `route('home')`
from `routes/web.php` in the same file resolves correctly, so the gap is
specific to filesystem-derived routes.

### B225. A route group whose name spells out nothing still flags its routes

**Impact: Low · Complexity: Medium**

A group whose name is entirely a variable and which sits under no
enclosing literal group (`Route::name($panelId)->group(...)` at the top
of a routes file) records no open prefix, so every `route()` call naming
one of its routes is still reported as unknown.

The obvious fix is the wrong one: an open prefix of `""` is a prefix of
every route name there is, so the diagnostic
(`route_open_prefixes.iter().any(|prefix| key.starts_with(prefix))` in
`diagnostics/mod.rs`) would stand down for the whole project rather than
for the one group. What is needed instead is for the collector
(`virtual_members/laravel/route_names.rs`) to record which *names* fall
under an unknowable group rather than which prefixes, so an unnamed
group opens only the suffixes it registers (`pages.dashboard` under an
unknown prefix means any name *ending* in it is unjudgeable) and every
other name in the project stays checked.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

No outstanding items.
