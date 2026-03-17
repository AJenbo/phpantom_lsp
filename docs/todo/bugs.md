# PHPantom — Bug Fixes

Known bugs and incorrect behaviour. These are distinct from feature
requests — they represent cases where existing functionality produces
wrong results. Bugs should generally be fixed before new features at
the same impact tier.

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## B2. Orphan PHPStan processes on server shutdown

**Impact: High · Effort: Low**

The `shutdown()` handler in `server.rs` is a no-op. The PHPStan and
diagnostic background workers are spawned with `tokio::spawn` and
their `JoinHandle`s are dropped. If PHPStan is running when the user
closes their editor, the child process continues consuming CPU and
memory until it hits its timeout (up to 60 seconds).

The `run_command_with_timeout` busy-wait loop inside `spawn_blocking`
has no cancellation check. There is no `CancellationToken` or similar
mechanism to signal the workers to stop.

**Fix:** Store the `JoinHandle`s (or a shared `CancellationToken`).
In `shutdown`, signal cancellation and abort the tasks. In
`run_command_with_timeout`, check a cancellation flag in the poll
loop so the child process is killed promptly.

**Files:** `src/server.rs` (shutdown, worker spawns),
`src/phpstan.rs` (`run_command_with_timeout`),
`src/diagnostics/mod.rs` (worker loops).

---

## B4. Diagnostic dedup only removes adjacent duplicates and uses wrong key

**Impact: Medium · Effort: Low**

`suppress_redundant_diagnostics` in `src/diagnostics/mod.rs` uses
`Vec::dedup_by`, which only removes **consecutive** duplicates.
Diagnostics from Phase 1 (fast), Phase 2 (slow), and Phase 3
(PHPStan) are appended sequentially without sorting, so identical
diagnostics from different phases survive if anything sits between
them.

The dedup key also checks `a.message == b.message`, which is too
strict. Two diagnostics covering the same range are redundant
regardless of wording. The correct dedup key is the range alone.

The current logic should prefer diagnostics with an exact range
(specific start and end character) over diagnostics that only have a
line number (full-line range with `character 0..MAX`). When two
diagnostics overlap on the same line, keep the one with the more
precise range.

**Fix:** Sort diagnostics by range before deduplicating. Change the
dedup key to range only (drop the message comparison). When a
full-line diagnostic and a precise diagnostic cover the same line,
keep the precise one (this is partially handled by the existing
`is_full_line_range` suppression, but the dedup pass should also
prefer precise ranges).

**File:** `src/diagnostics/mod.rs` L724-726.

---

## B5. Implementation error diagnostic skips enums

**Impact: Medium · Effort: Low**

PHP enums can implement interfaces and must provide all required
methods. The filter in `collect_implementation_error_diagnostics`
skips everything that is not `ClassLikeKind::Class`, so missing
interface methods on enums are never flagged.

The fix also needs to account for the methods that the engine
provides automatically on enums (`UnitEnum::cases`,
`BackedEnum::from`, `BackedEnum::tryFrom`). These should be excluded
from the "missing" set.

**File:** `src/diagnostics/implementation_errors.rs` L73-76.

---

## B7. Inlay hints: wrong parameter name with mixed named and positional arguments

**Impact: Medium · Effort: Medium**

`emit_parameter_hints` in `src/inlay_hints.rs` maps each argument to
a parameter by its positional index (`arg_idx`). This is wrong when
named arguments appear before positional arguments or consume
parameters out of order. For example, `greet(city: 'NYC', 'Alice')`
would label `'Alice'` as `age:` instead of `name:`.

**Fix:** Track which parameters are consumed by named arguments
first, then assign remaining positional arguments to the remaining
parameters in order.

**File:** `src/inlay_hints.rs` L115-121.

---

## B10. PHPStan cache written after file close causes stale diagnostics on reopen

**Impact: Low-Medium · Effort: Low**

If PHPStan is running for a file when the user closes it,
`clear_diagnostics_for_file` clears the cache first, then the
PHPStan worker finishes and writes its results back into
`phpstan_last_diags`. The next `did_open` merges these stale
diagnostics.

**Fix:** After the PHPStan worker writes results, check that the
file is still in `open_files` before caching. Alternatively, tag
cached results with a version counter and discard stale entries.

**File:** `src/diagnostics/mod.rs` (PHPStan worker, around L593-596).

---

## B1. Native type hints not considered in virtual property specificity ranking

**Impact: Low-Medium · Effort: Medium**

The `type_specificity` function used during virtual member merging only
scores the `type_hint` field (the effective/docblock type). It does not
consider `native_type_hint` (the PHP-declared type on the property).

For example, a real property declared as `public string $name;` has
`native_type_hint = Some("string")` and `type_hint = Some("string")`.
If a docblock or virtual provider contributes `@property array<int> $name`,
the specificity comparison works correctly today because both values flow
through `type_hint`.

However, the broader issue is in `resolve_effective_type`: when a native
hint says `string` and a docblock says `array<int>`, the effective type
should be the docblock's version (it is more specific and deliberately
overrides the native hint). This is not specific to virtual member merging
but to the general type resolution pipeline. Fixing it here would not help
because the native vs docblock decision happens upstream in the parser.

This is out of scope for the virtual member specificity work but worth
tracking as a separate improvement to `resolve_effective_type`.