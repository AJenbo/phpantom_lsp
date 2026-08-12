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

### B86. A check on a method call does not reach the argument that repeats it

**Impact: Medium · Effort: Medium**

```php
if ($this->service() instanceof MockInterface) {
    $this->mockMethod($this->service(), 'annul');  // reported: got EpaymentService
}
```

`resolve_target_classes` narrows an argument-less instance call keyed
under its own text (`narrowable_call_key` in
`type_engine/resolver/mod.rs`), so completion and hover after the check
see the narrowed type.  The argument-type diagnostic never gets there:
`resolve_expression_type` walks the argument as an AST expression, and
`rhs_resolution::resolve_rhs_call` resolves the call purely from the
method's declared return type without consulting the narrowing for the
call's key, the way `Expression::Access` does for a property path.  So
the check is ignored and the declared return type is what the parameter
is measured against.

**Fix:** give `resolve_rhs_call` the same narrowing lookup
`Expression::Access` has, keyed with the same text
`narrowable_call_key` builds so both sides agree on the key.

### B87. An `instanceof` on a two-level property path drops the declared class

**Impact: Medium · Effort: Medium**

```php
if ($this->holder->service instanceof MockInterface) {
    $x = $this->holder->service;
    $this->realMethod($x);  // reported: got MockInterface
}
```

`$this->service instanceof MockInterface` on a one-level path resolves to
`EpaymentService&MockInterface`, but adding a level loses the declared
class and leaves only the interface.  `seed_property_keys_into_scope`
(`type_engine/variable/forward_walk/cond_narrowing.rs`) resolves nothing
for the two-level key, so the merge below it takes the "untyped subject,
instanceof provides the type" branch and replaces rather than intersects.
Every consumer that reads the narrowed subject is then measured against
the interface alone, so a member or parameter belonging to the concrete
class is reported.

**Fix:** make the seeding resolve a multi-level property path to the same
type the one-level path resolves to, so the merge sees a declared type to
intersect with.
