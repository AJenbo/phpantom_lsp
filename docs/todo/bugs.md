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

Bugs land here from wherever they surface: found while working on another
task, or sweeps of the sample projects under `projects/`. Entries are
grouped by the mechanism that has to change, not by the symptom that
surfaced: one entry is one root cause, however many shapes it shows up in.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Reachability

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

### B317. The Blade preprocessor's own declarations reach the global indexes

**Impact: Low · Complexity: Low**

The virtual PHP a template lowers to opens with a prologue declaring
`__blade_template` and the `blade_*_directive` marker functions the
lowering calls. Those are published like any other declaration, so they
appear in workspace-symbol search (ten entries a query for "blade"
matches) and every template in the project contributes a duplicate
declaration of each of them to `duplicate_functions`.

Nothing reports them as a redeclaration and the memory is small (a few
megabytes across a project with hundreds of templates), but they are
boilerplate no file wrote and they should not be reachable as symbols.
Keeping them out of the published indexes has to leave them resolvable
from within a template, or every marker call the lowering emits becomes
an unknown-function diagnostic; registering them once as stubs rather
than per-template is the shape that gives both.

**Where to look:** `src/blade/preprocessor.rs` (the prologue),
`src/parser/ast_update.rs` (`build_ast_index_update`),
`src/workspace_symbols.rs`.
