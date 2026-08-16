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

#### B1. Editing a service provider does not re-scan what it registers

**Impact: Medium · Complexity: Medium**

`build_provider_resources` runs once, at `initialized`, and in the
`analyze` CLI. Nothing re-runs it when a provider file changes, so
everything the scan recovers goes stale for the rest of the session:
a container binding written now needs a restart before `app('key')`
resolves, hovers, or navigates, and the same applies to the view
directories, translation directories, route files, config files, and
component namespaces a provider registers.

Every other provider-derived table already has its per-file
counterpart (`refresh_laravel_gates`, `refresh_laravel_morph_map`,
`refresh_laravel_command_index`, `refresh_laravel_macros`,
`refresh_laravel_storage_drivers`); provider resources are the one
table without one. A `refresh_laravel_provider_resources(uri,
content)` on the same didChange/didSave path closes it.

Re-scanning a single file is not enough on its own: the binding table
is merged across every provider and its precedence depends on which
provider outranks which, so the refresh has to rebuild the merged set
rather than patch one file's entries into it. It must also reset
`laravel_aliases` and clear the class-not-found cache, the way
`build_provider_resources` already does, or the keys it just learned
stay unresolvable.
