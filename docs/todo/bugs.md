# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

## B2 Namespace rename writes an FQN into every name of a group `use`

Renaming namespace `App\Old` to `App\New` turns

```php
use App\Old\{Foo, Bar};
```

into

```php
use App\New\{App\New\Foo, App\New\Bar};
```

which is a parse error. This is covered by
`rename_namespace_updates_group_use` in `src/rename/tests.rs`, but the
assertion only checks that `App\New` appears somewhere in the result, so
it passes on the broken output.

Two paths in `rename/namespace.rs` edit the same line:

- `collect_use_statement_edits` scans the source and rewrites the group
  prefix (`App\Old` → `App\New`), which is the correct and complete edit.
- `collect_fqn_reference_edits` walks the symbol map, where each name
  inside the braces is a `ClassReference` span whose recorded `name` is
  the *composed* FQN (`App\Old\Foo`) while the span itself covers only
  the trailing `Foo`. It therefore emits a second edit replacing `Foo`
  with the full `App\New\Foo`.

The two edits do not overlap, so the `dedup_by(ranges_overlap)` pass that
resolves the ordinary `use App\Old\Foo;` case does not catch them.

The fix belongs in `collect_fqn_reference_edits`: a span whose source
text is only a segment-aligned tail of its recorded name is a group-use
member, and the group prefix edit already covers it, so it should be
skipped. `source_spells_reference` in the same file already
distinguishes the two shapes. Extend the test to assert the full
expected line rather than a substring.
