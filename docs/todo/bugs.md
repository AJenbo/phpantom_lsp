# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B48. Blade drops a variable's generic arguments

**Impact: Medium · Effort: Medium**

A collection passed to a template keeps its class but loses its generic
arguments, so every member typed through a class-level `@template` falls
back to that template's bound. With an explicit annotation in the
template:

```blade
@php
/** @var \Illuminate\Database\Eloquent\Collection<string, \App\Models\Loaf> $byName */
@endphp
{{ $byName->get([1]) }}
{{-- Argument 1 ($key) expects array-key|null, got list<int> --}}
```

The identical `@var` in a plain PHP file resolves `$key` to
`string|null`. `array-key` is the bound of `Eloquent\Collection`'s
`@template TKey of array-key`, so the `<string, Loaf>` arguments never
reach parameter substitution on the Blade side.

#### B49. An unannotated callback body that is a method call binds no template

**Impact: Medium · Effort: Medium**

When a callback has no return-type annotation, the template bound from
its return type is inferred from the body expression. A body that is a
*call* resolves to `mixed` (the call-return step is missing from
argument-text resolution), so the template stays unbound and falls back
to its declared bound:

```php
$byRating = $reviews->keyBy(fn (Review $r) => $r->getRating());
foreach ($byRating as $key => $review) { … }
// $key is array-key|\UnitEnum, should be int
```

A body that is a variable, a literal, or a `new` expression binds
correctly, so this is specifically the call case.
