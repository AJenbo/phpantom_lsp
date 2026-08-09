# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

### B67. An inline `@php(…)` masks the rest of the template

`mask_inert_regions` (`src/blade/signature.rs`) treats every `@php` as the
opening of a block and runs `end_of_region(bytes, i + 4, b"@endphp")` on
it. Blade has two spellings: the block form, which does close with
`@endphp`, and the inline directive `@php($featured = $posts->first())`,
which closes with its own parenthesis and never writes `@endphp` at all.
An inline one therefore masks everything from itself to the *next*
`@endphp` anywhere in the file, or to EOF when there is none.

Every consumer of the masked text loses that span: `has_declared_signature`,
`extract_extends`, `extract_props`/`extract_aware`,
`referenced_component_tags`, and `scan_component_tag_calls`. The last is
the one that shows: a `<x-…>` tag written after an inline `@php(…)` is
invisible, so the component it names never sees the attributes that call
site passes and reports the variables they supply as
`unknown_variable`. `examples/laravel/resources/views/welcome.blade.php`
has a live instance — `@php($bakery = …)` near the bottom masks every tag
after it.

Fix: only open a block when the `@php` is *not* the inline form. Blade's
own `compileStatements` regex admits `[ \t]*` between the directive name
and an opening `(`, so `@php` followed by optional spaces or tabs and then
`(` is inline and masks nothing. The same shape applies to any other
directive that has both a block and an inline form.

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
