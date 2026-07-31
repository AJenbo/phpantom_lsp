# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

## B117. Per-file callable-target caches are keyed by call-expression text alone

**Impact: Medium · Effort: Low-Medium**

Both argument diagnostic passes memoize `resolve_callable_target`
in a per-file `HashMap<String, Option<ResolvedCallableTarget>>`
keyed only on the call expression, then resolve the entry using
whichever call site happened to be reached first. Callable-target
resolution is position-dependent (it picks the enclosing class via
`find_class_at_offset` and resolves receiver variables against the
scope at the cursor), so any expression whose meaning varies within
the file gets the wrong target at every site after the first:

- `argument_count.rs` caches *every* expression, including
  variable-based calls. Two methods that both call
  `$parser->parse(…)` on different types share one cached
  signature, so the argument count is checked against the wrong
  method.
- `type_errors/mod.rs` deliberately skips variable-based calls
  (`expr.starts_with('$')`), but `self::`, `static::`, and
  `parent::` do not start with `$` and are equally
  position-dependent. In a file declaring two classes that each
  have a zero-argument `self::make()`, the second class resolves
  to the first class's method.

Both are latent today because the wrong target usually has a
compatible signature, and a wrong resolution can produce either a
false positive or a silently missed diagnostic.

The cache key must cover what resolution actually depends on.
Restricting the cache to position-independent expressions (plain
function calls and `Fqn::method` where the class part is not
`self`/`static`/`parent`) is the smallest sound fix; keying on
`(expr, enclosing_class_start_offset)` keeps more hits for the
keyword forms. Note that per-site resolution is no longer
expensive now that the offset round trip is gone, so dropping the
unsound entries costs little.

---

## B118. `phpantom_lsp analyze` on `examples/laravel` reports one error, but CI requires none

**Impact: Low · Effort: Low**

`docs/CONTRIBUTING.md` lists
`phpantom_lsp analyze --project-root examples/laravel --no-colour`
as a required check and states it "must report `[OK] No errors`",
but `examples/laravel/app/Demo.php` intentionally contains
`Artisan::call('does:not-exist')` to demonstrate the
`invalid_laravel_command` diagnostic, so the command reports
`Found 1 error`. Either the demo needs a form that shows the
diagnostic without failing the check (the way `examples/demo.php`
keeps its intentional diagnostics out of the analyze gate), or the
documented expectation needs to name the diagnostics the Laravel
example is expected to produce.
