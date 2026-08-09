# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

### B67. `Blade::anonymousComponentNamespace()` registrations are invisible

`Blade::anonymousComponentNamespace('components', 'webshop')` makes
`<x-webshop::pages.boxes>` resolve to the plain view `components.pages.boxes`
against the configured view roots
(`ComponentTagCompiler::guessAnonymousComponentUsingNamespaces` runs
`guessViewName` with the registered directory as the prefix).
`Blade::anonymousComponentPath()` is the same mechanism keyed by directory.

PHPantom models neither registration. `component_tag_names` /
`view_name_for_component_tag` (`src/blade/component_tags.rs`) only know the
un-registered fallback (`ns::X` ↔ `ns::components.X`), so a component
addressed through a registered prefix matches none of its `<x-…>` call
sites: the attributes those tags pass are never inferred, and a variable
the component reads from them is reported `unknown_variable` even though
`AnonymousComponent::data()` supplies it at runtime. One live instance in
the Website sample (`$kerastaseHairAnalysis` in
`components/pages/brand/pro-hair-care/kerastase-boxes.blade.php`, called as
`<x-webshop::pages.brand.pro-hair-care.kerastase-boxes>` from two
templates).

Fix: extract `anonymousComponentNamespace()` / `anonymousComponentPath()`
registrations in `provider_resources.rs` the way `componentNamespace()`
already is (`component_namespace_args`), and consult them in the tag ↔
view-name mapping, which today is a pure function of the view name and
needs the `Backend`'s registrations to do this.

### B2. A layout's contract is enforced without the layout's own suppliers

`blade_template_contract` (`src/blade/contract.rs`) merges the layouts a
template `@extends` into the contract call sites are judged against, so a
variable only `layouts/app.blade.php` declares is one every caller of
`pages.home` has to pass. The `supplied` set that exempts a variable from
the missing check is built from the *child* alone:
`declares_component_directive`, `extract_props`, `extract_aware`,
`blade_backing_class_vars`, and `blade_provider_vars` all run against the
child's source and the child's view names.

A view composer is normally registered on the template that reads the
variable, which is the layout. `View::composer('layouts.app', …)` sharing
`$title` types the layout's `$title` correctly, because
`blade_provider_vars` matches the registered pattern against
`layouts.app`, but nothing matches it against `pages.home`, so every page
extending that layout reports "View 'pages.home' expects $title of type
string, which is not passed" at every one of its call sites. Nothing the
caller can write clears it. The same hole drops a layout's own `@props`
defaults and, for a layout that is itself a component, its backing class
members.

Fix: build `supplied` over the whole chain the way `vars` already is. For
each level the layout walk yields, add that template's props and aware
defaults, its component scope, its backing class members, and its
provider vars. `blade_layout_chain` returns the view name alongside the
source, so the names each level needs are already in hand.
`blade_rendering_scope` has the same shape and needs the same fix.

### B3. Directive coverage gaps in `match_directive`/`translate_directive`

Discovered while building directive-name completion (`DIRECTIVE_COMPLETIONS`
in `src/blade/directives.rs`) and cross-checking it against Laravel's own
compiler source (`Illuminate\View\Compilers\Concerns\*`, vendored under
`examples/laravel/vendor/laravel/framework/`).

**Directives `match_directive` (`src/blade/directives.rs`) does not
recognise at all**, so a template that writes them is silently left
unprocessed (masked as inert HTML) rather than mistranslated:
`@can`/`@cannot`/`@canany`/`@elsecan`/`@elsecannot`/`@elsecanany`/
`@endcan`/`@endcannot`/`@endcanany` (`CompilesAuthorizations.php`),
`@lang`/`@endlang`/`@choice` (`CompilesTranslations.php`), `@unset`
(`CompilesRawPhp.php`), and `@js`/`@vite`/`@viteReactRefresh`/`@fonts`/`@dd`
(`CompilesJs.php`/`CompilesHelpers.php`).

**Directives that are recognised but whose arguments `translate_directive`
still degrades to a bare comment** (`_ => format!("/* @{directive} */")`)
instead of an expression-preserving `blade_directive(...)` call, so their
arguments go completely untyped: `pushIf`, `pushOnce`, `prependOnce`,
`endPushIf`, `endPushOnce`, `endPrependOnce`, `hasStack`, `csrf`, `parent`,
`continue`. The last three take no arguments in real Blade, so a comment is
correct for them; the rest do take arguments that should be type-checked
the way `@error`/`@session`/`@context` already are.

Fix: add the missing directives to `match_directive`'s list with real
translations (each has a `compile*` method in the vendored source above to
copy the shape from), and give `pushIf`/`pushOnce`/`prependOnce`/`hasStack`
a `blade_directive(...)`-style translation instead of falling through to
the default comment.
