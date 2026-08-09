# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

### B68. An unresolvable branch drops out of a union instead of widening it

Indexing a union-typed array resolves to nothing rather than to `mixed`,
and a branch that resolves to nothing contributes nothing to the union
built from it. The result is not a wider type but a *narrower* one: the
surviving branch alone, stated with the confidence of a complete answer.

`YamlFileLoader::parseDefinition()` in `symfony/dependency-injection`
holds a live instance. Its `$service` parameter is `array|string|null`, so

```php
$decorationOnInvalid = \array_key_exists('decoration_on_invalid', $service)
    ? $service['decoration_on_invalid']
    : 'exception';
```

should give `mixed`, since one branch is an index into a union PHPantom
cannot index. Instead `$service['decoration_on_invalid']` resolves to
nothing, the ternary keeps only its else branch, and `$decorationOnInvalid`
comes out as the literal type `'exception'` — a claim that the variable
holds exactly that one string. Declaring the parameter plain `array` gives
the correct `mixed`, so the trigger is the union, not the ternary.

Every consumer inherits the over-claim. Hover names one value the variable
often does not hold, completion offers that value's members, and a
diagnostic that reads the type as the complete set of what a subject can
hold will contradict the code: `unreachable_match_arm` had to stop trusting
literal types at all (`scalar_type_label` in
`src/diagnostics/match_type_errors.rs`) to avoid calling the `null` arm of
that same `match ($decorationOnInvalid)` unreachable.

Fix: index a union member-wise and union the results, so `array` yields its
element type, `string` yields `string`, and `null` yields `null`. Then, in
the resolver generally, distinguish "this expression is `mixed`" from "this
expression did not resolve": a branch that fails to resolve must widen the
union it belongs to rather than silently vanish from it.
