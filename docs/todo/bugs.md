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

### B261. A property keeps its narrowing across a write the resolver cannot type

**Impact: Low-Medium · Complexity: Medium**

```php
class Holder
{
    /** @var A|C */
    public $prop;

    public function f($u): void
    {
        if ($this->prop instanceof A) {
            $this->prop = $u->make();
            $this->prop->onlyC();   // false unknown_member, "on class 'A'"
        }
    }
}
```

The property branch of `process_assignment_expr`
(`forward_walk/assignment.rs`) records the written type under the
property-path key, but `ScopeState::set` ignores an empty type list, so
a right-hand side that resolves to nothing leaves the `instanceof`
narrowing from before the write in place. The same defect on a plain
variable was fixed by writing "no type known" over the old entry; a
property cannot use that, because the correct fallback for a property is
its *declared* type, not unknown, so the key has to be dropped rather
than blanked. Narrower than the variable case: it only misreports when
the declared type is wider than what the check narrowed it to, since
otherwise the declared type answers the same way.

## Arithmetic

No outstanding items.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

No outstanding items.
