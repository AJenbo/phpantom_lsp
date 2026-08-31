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

## B1. Adding an `[indexing] extensions` entry needs a restart to be watched

**Impact: Low · Complexity: Low-Medium**

`initialized` in `src/server.rs` registers one
`workspace/didChangeWatchedFiles` watcher per configured
`[indexing] extensions` entry, and that registration happens once. A
`.phpantom.toml` reload recompiles the filters (so the next workspace
scan sees the new extension) but does not touch the watcher list, so
editing a `.module` file added to the setting mid-session produces no
event and the index keeps serving the version from the last full scan.
The rest of the configuration is reloaded live, and the changelog says
so, which is what makes the exception surprising.

The Laravel-gated `*.sql` and `config/database.php` watchers have the
same shape: they are chosen from `is_laravel()` at startup and never
revisited.

**Fix:** Re-register the watcher capability from `reload_config` when
the extension list (or the Laravel classification) changed since the
last registration, unregistering the previous one by its
`workspace/didChangeWatchedFiles` id first. Guard it on an actual
change so an unrelated config edit does not churn the client's
watchers. A test can assert the registered glob set after a reload
that adds an extension.
