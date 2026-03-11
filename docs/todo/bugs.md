# PHPantom — Bug Fixes

Known bugs and incorrect behaviour. These are distinct from feature
requests — they represent cases where existing functionality produces
wrong results. Bugs should generally be fixed before new features at
the same impact tier.

Items are ordered by **impact** (descending), then **effort** (ascending).

| Label | Scale |
|---|---|
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low** |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## 0. Thread panic in parallel file scanners crashes the server

**Impact: High — Effort: Low**

The three `scan_files_parallel_*` functions in `classmap_scanner.rs`
join spawned threads with `.join().unwrap()`. If any spawned thread
panics (e.g. due to a malformed file encountered during the byte-level
scan), the `unwrap()` propagates the panic to the caller. Because
these functions run during server initialization (outside any
`catch_unwind` guard), a single thread panic kills the entire LSP
process. The user's editor silently loses PHP intelligence with no
error message.

The `scan_content`, `find_classes`, and `find_symbols` functions
process arbitrary user and vendor files from disk. While they are
designed to be robust, a panic in any of them (or in future code
changes) would be fatal.

### Fix

Replace `.join().unwrap()` with `.join().unwrap_or_default()` (or
`.join().ok()` with appropriate flattening) and log an error for
any thread that panicked. The affected scan results are simply
skipped, matching the existing behaviour of `fs::read` failures
which are silently ignored.

Three call sites:
- `scan_files_parallel_classes` (L407)
- `scan_files_parallel_psr4` (L467)
- `scan_files_parallel_full` (L537)

---

## 1. Sequential write-lock acquisition in `parse_and_cache_content_versioned`

**Impact: Low-Medium — Effort: Low**

`parse_and_cache_content_versioned` in `resolution.rs` acquires write
locks on `ast_map`, `use_map`, `namespace_map`, `fqn_index`, and
`resolved_class_cache` in sequence. Each lock is acquired and released
individually (not nested), so there is no deadlock risk with the
current `parking_lot` implementation. However, holding one write lock
while acquiring the next creates brief windows where readers of the
earlier-released lock see updated data but readers of the
not-yet-written lock see stale data.

In practice this is unlikely to cause user-visible issues because
all five writes complete within microseconds and all consumers
re-read the maps on each request. But the pattern is fragile: if a
future change adds a reader that checks two of these maps for
consistency within the same request, it could observe a partial
update.

### Analysis needed

Audit the code paths that read multiple maps in a single request
to confirm no consumer relies on cross-map consistency. If none
does, document the invariant ("these maps are eventually consistent
within a single `update_ast` call but not atomically consistent")
and close the item. If a consumer is found, batch the writes under
a single coordination mechanism.

---

## 2. Native type hints not considered in virtual property specificity ranking

**Impact: Low-Medium — Effort: Medium**

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
