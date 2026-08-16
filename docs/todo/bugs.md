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

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

### B178. Three declaration sites still hover

**Impact: Low · Complexity: Low**

Hover stands down at a definition site, because the signature is on the
line under the cursor and the docblock is on the lines above it, so the
popup would only repeat what is already on screen. Class, interface,
namespace, method, property, class constant, and enum case declarations
all follow that rule. Three do not:

- a global `function helper(): int` at its own name answers with its
  docblock and signature;
- a global `const LIMIT = 5;` (and `define()`) at its own name answers
  with its value;
- a constructor-promoted parameter (`__construct(public string $sku)`)
  answers as a local parameter, which is also the wrong reading of it:
  the name declares a property, not a variable.

All three go through `hover_from_symbol` in `src/hover/mod.rs`: the
first two land in the `FunctionCall` / `ConstantReference` arms, which
never check the `is_definition` flag their spans already carry, and the
third lands in the `Variable` arm, where `VarDefKind::Parameter` is on
the allow-list and no caller distinguishes a promoted parameter
(`is_promoted_property_param` in `src/definition/resolve.rs` is the
existing test for one).

Decide the rule once and apply it to all three rather than case by
case: a hover on a *declaration* is either useful everywhere or nowhere.
