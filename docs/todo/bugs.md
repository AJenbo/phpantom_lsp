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

### B318. Moving a class into the global namespace writes `namespace ;`

**Impact: Medium · Complexity: Low**

`phpantom_lsp move 'App\Old\Widget' 'Widget'` rewrites the declaration
file's `namespace` name in place, and the destination has no name, so the
replacement text is empty and the file is left holding `namespace ;`, a
syntax error. The whole statement has to go instead, from the `namespace`
keyword through the terminating `;` (or, for a brace-style namespace, the
block it opens has to be unwrapped, which is a good reason to refuse that
shape rather than mangle it).

**Where to look:** `src/rename/class.rs`
(`build_class_move_edit`, the `NamespaceDeclaration` span edit).

### B319. A class leaving the global namespace is reported as left behind

**Impact: Low · Complexity: Low**

Moving a class out of the global namespace (`Widget` →
`App\Casts\Widget`) reports `The old name \`Widget\` still appears here`
against the moved file's own `class Widget` line. The old FQN of a global
class is a bare short name, so the residual scan's needle matches the
declaration the move deliberately leaves spelled the same way. A needle
that is a bare name has to skip the declaration site of the class that
moved.

**Where to look:** `src/move_cli/residual.rs` (`build_needles`,
`collect_hits`).
