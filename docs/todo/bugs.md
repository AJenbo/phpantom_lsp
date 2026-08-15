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

Most entries below come from the 2026-08-15 sample-project sweep (46
diagnostics across ten projects). 45 were false positives, filed here as
**B49–B74**; the one genuine finding (an unguarded `SimpleXMLElement`
child lookup) was patched directly in a sample source.

Where several of those turn out to share one area of the engine, they
are merged into a grouped entry that carries every repro as a numbered
sub-case. A grouped entry is still one task and one PR.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Narrowing

No outstanding items.

## Symbol resolution

### B78. A call argument binds a `@template` without the `null` its return type declares

**Impact: Low-Medium · Complexity: Medium-High**

```php
/**
 * @template T of Carbon|string|null
 * @param T $date
 * @return T
 */
function passthrough(mixed $date): mixed { return $date; }

takesCarbon(passthrough(Carbon::create(2024)));   // not reported — but can be null
```

`Carbon::create()` returns `?Carbon`, and the diagnostic's own argument
resolution says so. The template-binding pass reads the argument from
its *text* instead, and that path resolves an expression to the classes
it can be. `null` is not a class, so the arm is dropped and `T` binds to
a bare `Carbon`. Every use of the substituted template then claims the
value can never be null: a missed diagnostic wherever the substituted
return is consumed, and a false positive wherever a parameter is checked
against it.

Only the inline-call form loses it. `$d = Carbon::create(2024);
passthrough($d);` binds `?Carbon` correctly, because a variable's type
is read from the scope rather than re-resolved from text.

**Fix:** the text-based argument resolver reaches
`resolve_expression_to_type` (class-backed results only) before it
reaches `resolve_call_return_hint` (the declared return type, which
still has the arm). Either restore the nullability the class walk drops
for a call expression, or resolve a call argument through its declared
return type first. Widening the order is the wider blast radius of the
two: the hint preserves `static`/`$this` and generics that the class
walk deliberately flattens.

## Array types

No outstanding items.
