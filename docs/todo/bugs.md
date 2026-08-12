# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Effort** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

### B85. Some `instanceof`-to-unrelated-interface narrowings still merge as a union

**Impact: Low · Effort: Medium**

`apply_instanceof_inclusion` (`type_engine/types/narrowing/instanceof.rs`)
narrows a declared class to an unrelated interface it doesn't nominally
implement (e.g. a mock that IS the declared class AND implements the
interface it was narrowed to) by keeping both classes, which the value
satisfies simultaneously — an intersection, not a union. Two of the
narrowing call sites that reach this function now report that correctly:
plain-variable/property narrowing during the forward walk
(`forward_walk/cond_narrowing.rs`) and property-path narrowing reached
through `Expression::Access` in argument/return diagnostics
(`variable/rhs_resolution/mod.rs`, via `resolver::apply_property_narrowing`,
which now returns whether the merge was an intersection).

Two other consumers of the same `apply_property_narrowing` entry point,
in `type_engine/resolver/mod.rs` (~line 526, method-call return narrowing;
~line 644, the generic subject resolver used by hover/completion/go-to-
definition), discard that return value and still join the merged classes
as a plain union via `ResolvedType::from_arc`. A subject narrowed this
way and then passed as an argument would still be wrongly flagged.

Separately, `try_apply_instanceof_narrowing`'s compound-AND branch
(`$x instanceof A && $x instanceof B`) and `try_apply_assert_instanceof_narrowing`
(`assert($this->prop instanceof Foo)`) both call into the same
"keep both" logic but were not wired to report the intersection either,
even where their caller could propagate it.

**Fix:** thread the intersection flag from `apply_property_narrowing`
through the two remaining call sites in `resolver/mod.rs`, building the
same "same `type_string` on every entry, distinct `class_info` each"
`ResolvedType` shape `variable/rhs_resolution/mod.rs` now uses. Decide
whether the compound-AND and assert-based paths should report an
intersection the same way, and wire them up if so.
