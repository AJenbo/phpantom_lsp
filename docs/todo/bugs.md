# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B55. `@props` overwrites a `@var` declaration with `null` for every key but the first

**Impact: High · Effort: Low**

A Blade component that declares its contract in a standalone docblock and
then lists the same names in `@props` keeps the declared type for the
first key only. Every later key is bound to `null` instead:

```blade
@php
    /**
     * @var string $poster
     * @var string $video
     */
@endphp
@props(['poster', 'video'])

{{ imgix($poster) }}   {{-- string, correct --}}
{{ imgix($video) }}    {{-- null — "expects …|string, got null" --}}
```

Reversing the order of the two `@var` lines does not move the diagnostic,
so it follows the order of the `@props` list, not the docblock.

Two things are wrong. A `@props` key with no default is a *required*
prop, so it should be typed by whatever the caller passes (or left
unknown), never `null` — a component with no docblock at all currently
types every defaultless prop as `null`. And a declared `@var` must win
over whatever `@props` derives, for every key rather than the first one.

**Where to look:** the `@props` handling added when the directive
started declaring its keys as local variables. It writes one binding per
key; only the first appears to be merged with the standalone `@var`
scope, and the default for a key written as a bare list entry is being
taken as `null` rather than "declared, value supplied by the caller".

#### B56. Attributes passed to an anonymous component are undefined unless `@props` lists them

**Impact: Medium-High · Effort: Low-Medium**

`Illuminate\View\AnonymousComponent::data()` merges
`$this->attributes->getAttributes()` into the view data, so *every*
attribute written on an `<x-…>` tag becomes a variable inside the
template — `@props` only adds defaults and removes the key from
`$attributes`. PHPantom only creates the variable when `@props` names
it, so a component that reads an attribute directly is reported
`unknown_variable`:

```blade
{{-- caller --}}
<x-brand.boxes :hairAnalysis="$model->hairAnalysis" />

{{-- components/brand/boxes.blade.php, no @props --}}
<x-promo-box :href="$hairAnalysis" />   {{-- "Undefined variable '$hairAnalysis'" --}}
```

Adding `@props(['hairAnalysis'])` silences it, which is the tell: the
call-site attributes are already parsed, they just are not turned into
variables unless the directive declares them.

**Where to look:** wherever `@props` keys become scope entries, the same
scope should also receive the union of the attributes each `<x-…>` call
site passes, the way template variables are already inferred from
`view()` call sites. The two sources should merge rather than one
gating the other.

#### B57. A `@props`-declared key is reported as an unused variable

**Impact: Medium · Effort: Low**

```blade
@props(['accordionId', 'headingId', 'contentId'])

<button id="{{ $headingId }}" aria-controls="{{ $contentId }}" {{ $attributes }}></button>
```

`$accordionId` is flagged `unused_variable`. It is not a local
assignment, though: naming a key in `@props` is what *removes* it from
`$attributes`, so deleting the entry changes the rendered output (the
attribute starts leaking into the tag). A declared prop the body never
reads is a component-API observation, not a dead variable, and the
unused-variable check should not claim it.

**Where to look:** the unused-variable collector treats the scope
entries `@props` creates like ordinary assignments. Props need a marker
that exempts them, the same way a function parameter is exempt.

#### B58. Deprecation diagnostics ignore the project's target PHP version

**Impact: Medium · Effort: Low-Medium**

A project pinned to PHP 8.4 (`"php": "^8.4"` plus
`config.platform.php: "8.4"` in `composer.json`) is still told about
deprecations introduced in 8.5:

```
'PDO::sqliteCreateFunction' is deprecated: use Pdo\Sqlite::createFunction
instead (since PHP 8.5)
```

The message already carries the version the deprecation landed in, and
`Backend` already knows the project's target version, so nothing new
needs discovering. A deprecation newer than the target must not be
reported.

**Where to look:** `collect_deprecated_diagnostics_with_context` in
`src/diagnostics/deprecated.rs`. The "since" version is parsed for the
message but never compared against `self.php_version()`. Deprecations
with no recorded version keep firing, as today.

#### B62. An application's container binding loses to the framework default for the same key

**Impact: Medium · Effort: Medium**

An application that replaces a framework binding still resolves to the
framework's class:

```php
// config/app.php registers Acme\Translation\TranslationServiceProvider,
// which extends Illuminate's and re-binds 'translator':
$this->app->singleton('translator', function (Application $app) {
    return new DatabaseTranslator($loader, $locale);
});

// …but:
return $this->app->make('translator');
// "Return type Illuminate\Translation\Translator is incompatible with
//  declared return type Acme\Translation\DatabaseTranslator"
```

Both providers are indexed and the framework's wins. Overriding a
framework binding from an application provider is the normal way to swap
an implementation, so the application's registration has to take
precedence.

**Where to look:** the binding index built by
`virtual_members/laravel/provider_resources.rs` keeps one entry per key
with no notion of precedence. A provider the application registers
explicitly (`config/app.php` `providers`, `bootstrap/providers.php`)
outranks a framework default, and a provider that `extends` another
outranks its parent. Where two bindings genuinely tie, the union of both
classes beats picking one arbitrarily.

#### B64. A dotted container key resolves to a class named after its first segment

**Impact: Medium · Effort: Low-Medium**

`find_or_load_class` normalises through `PhpType::parse`, whose
`base_name()` stops at the first character a PHP identifier cannot
contain. A container key is handed to it verbatim, so `'demo.bakery'`
becomes `demo` and matches any class of that short name before the
binding table is ever consulted:

```php
// app/Demo.php declares class App\Demo
$this->app->singleton('demo.bakery', fn () => new BakeryService());

app('demo.bakery')->bake('croissant');   // resolves to App\Demo, not BakeryService
```

The bound class is indexed correctly; it is simply never reached,
because the ordinary class-lookup phases claim the key first and the
Laravel alias table is only a fallback. Framework keys hit this too
wherever the leading segment collides (`view.engine.resolver` against a
project class named `View`).

**Where to look:** `resolution.rs`, `Backend::find_or_load_class`. A name
holding a character no PHP identifier can contain is not a class name at
all, so the ordinary phases have nothing to say about it and the alias
tables should be consulted first (or exclusively) for it.
