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

### B174. Reference counts stay at zero until the next edit

**Impact: Low-Medium · Effort: Low**

A file opened before the initial project index finishes shows `0
references` above every declaration, because the counts are computed
from an index that is still filling. `inlay_hint_refresh` is sent when
a `didChange` parse commits a new symbol map (`server.rs`) and when
pending member reference counts finish (`reference_counts.rs`), but not
when the initial index completes, so the stale zeros sit there until
something else happens to trigger a refresh. Send the same refresh once
indexing finishes.

### B175. Renaming a global constant renames a class constant of the same name

**Impact: Medium · Effort: Low**

`find_constant_references` matches `MemberDeclaration` spans by short
name, so a global `const BAR = 1;` and an unrelated `class Holder {
public const BAR = 2; }` are treated as one symbol. Renaming the global
constant to `QUX` rewrites `Holder`'s declaration too, and only the
declaration: `Holder::BAR` at every use site is left alone, so a file
that compiled no longer does.

A class constant is reached through `Holder::BAR` and a global one is
not, so the two can never be the same symbol. The match should require
the reference to be a global constant, not merely share the last
segment of its name.

### B176. A reopened file recycles result ids a client may still hold

**Impact: Low-Medium · Effort: Low**

`did_close` drops the file's entry from `result_ids` and `last_full`, so
reopening it starts the counter at zero again while the client may still
hold an id from before the close. Once the reopened file's recomputes
walk the counter back up to that number, a pull carrying the stale
`previousResultId` matches the current one, the handler answers
`Unchanged`, and the editor keeps showing the diagnostics the file had
before it was closed.

The window is narrow but reachable in ordinary use. A captured Zed
session shows a controller reaching `resultId` 3, being closed, and
climbing back through 1, 2, 3 on reopen with a different set at each
step; nothing about the id says which generation it belongs to. The
editor only escaped it there because it had dropped the file's id on
close.

Result ids are compared for equality alone, so they only need to never
repeat within a session. Drawing them from a session-global sequence
(the way `WorkspaceDiagnostics` already does with its `ws{n}` ids)
removes the collision entirely, and keeping the counter across a close
would do as well.

**Where to look:** the `result_ids` removal in
`clear_diagnostics_for_file` (`diagnostics/mod.rs`), and the equality
checks in
`document_pull_diagnostic` / `workspace_pull_diagnostic`
(`diagnostics/pull.rs`).
