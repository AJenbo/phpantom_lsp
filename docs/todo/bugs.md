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
