# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

## B1. A Blade template's inferred union types vary between runs

**Impact: Medium · Effort: Low**

When more than one `view()` call site passes the same variable name,
`compute_blade_injected_vars` unions the types each site contributes, but
the member order of that union changes from run to run. Analysing the
same unmodified project twice produces, for the same template:

```
$analysis: \Acme\Decimal\Decimal|\Acme\Models\CustomBrand|null
$analysis: \Acme\Models\CustomBrand|null|\Acme\Decimal\Decimal
```

The union members are always the same set; only their order moves.

The order comes from the order the call sites are visited, and
`view_caller_snapshot` builds its list by iterating `symbol_maps`, which
is a `HashMap` — so the iteration order is not stable across runs. The
per-name sort at the end of `compute_blade_injected_vars` sorts the
variable *names* only, so it does not settle the type strings.

Two consequences beyond the cosmetic one:

- Hover on such a variable shows `A|B` on one run and `B|A` on the next.
- `reinfer_and_reparse_blade_with` decides whether to re-parse a template
  by comparing the fresh `BladeScope` against the cached one with `==`. A
  reordered union compares unequal, so the template is re-parsed on a
  refresh pass that discovered nothing new.

### Fix

Give the union a deterministic order independent of visit order. Sorting
the deduplicated members before `PhpType::union` is the smallest fix;
sorting `view_caller_snapshot`'s output by URI also makes the whole pass
reproducible, which is worth having on its own for the re-parse
comparison above.
