# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

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
