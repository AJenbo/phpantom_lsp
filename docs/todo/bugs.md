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

## B11 — Diagnostic deduplication drops distinct diagnostics on the same range

| Impact | Effort |
| ------ | ------ |
| Medium | Low    |

`deduplicate_diagnostics` in `src/diagnostics/mod.rs` calls
`dedup_by(|a, b| a.range == b.range)` after sorting by range. This
removes **all** diagnostics that share the exact same span, regardless
of their diagnostic code, message, or severity. If two genuinely
different native diagnostics land on the same range (e.g. an
`argument_count` error and an `unknown_member` warning on the same
expression), the second one is silently dropped.

**Fix:** Change the dedup key from `a.range == b.range` to
`a.range == b.range && a.code == b.code`. This preserves distinct
diagnostic codes on the same span while still collapsing true
duplicates produced by different analysis phases.

---

## B12 — PHPStan cache pruning uses length-only comparison

| Impact | Effort |
| ------ | ------ |
| Low    | Low    |

In `publish_diagnostics_for_file` (`src/diagnostics/mod.rs`), the
PHPStan cache pruning step only updates the cache when
`pruned.len() != cached.len()`. If deduplication replaces one PHPStan
diagnostic with a different one at the same count (same number of
entries but different content), the cache is not updated. On the next
Phase 1 merge the stale entry would reappear.

In practice this is unlikely because pruning only ever removes entries
(never replaces them), but the check is technically incorrect.

**Fix:** Replace the length comparison with a content comparison, or
unconditionally write the pruned set back into the cache (the extra
write is negligible).
