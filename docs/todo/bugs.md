# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

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

**Where to look:** the declaration chain in `src/blade/signature.rs`
needs a source below `@props`: the union of the attributes each `<x-…>`
call site passes, the way template variables are already inferred from
`view()` call sites. `call_site_inference.rs` skips Blade files
entirely, so component tags are never read as call sites — that is the
gap. The two sources should merge, with `@props` winning per name rather
than gating the other.

#### B65. A `@var` whose type is a closure signature binds the wrong variable

**Impact: Low-Medium · Effort: Low**

A closure type writes `$`-prefixed names for its own parameters, and the
`@var` scan takes the first `$` it finds after the tag as the annotated
variable:

```php
/** @var \Closure(\App\Models\User $user): string $callback */
```

is read as declaring `$user` of type `\Closure(\App\Models\User`, so
`$callback` stays untyped and a bogus `$user` enters the scope. The same
shape appears in `@param` and in a Blade template's signature docblock,
where it also decides which names the contract declares.

**Where to look:** `parse_var_docblock_pairs` in
`type_engine/variable/forward_walk/assignment.rs` scans for the first
`$` after `@var`. The annotated variable is the `$name` at paren depth 0
and angle depth 0, so the scan has to track both while walking the type
rather than stopping at the first `$`.
