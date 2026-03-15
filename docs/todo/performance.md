# PHPantom — Performance

Internal performance improvements that reduce latency, memory usage,
and lock contention on the hot paths. These items are sequenced so
that structural fixes land before features that would amplify the
underlying costs (parallel file processing, full background indexing).

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## P1. Reference-counted `ClassInfo` (`Arc<ClassInfo>`)
**Impact: High · Effort: Medium**

### Completed

The three main data stores hold `Arc<ClassInfo>`:

- `ast_map`: `HashMap<String, Vec<Arc<ClassInfo>>>`
- `fqn_index`: `HashMap<String, Arc<ClassInfo>>`
- `resolved_class_cache`: `HashMap<ResolvedClassCacheKey, Arc<ClassInfo>>`

Insertion sites (`parse_and_cache_content_versioned`, `update_ast_inner`)
wrap `ClassInfo` in `Arc` before storing.

The retrieval and resolution pipeline now returns `Arc<ClassInfo>`
from the storage layer through to the class-loader boundary:

- `find_class_in_ast_map` returns `Option<Arc<ClassInfo>>`.
- `find_or_load_class` returns `Option<Arc<ClassInfo>>`.
- `FileContext::classes` is `Vec<Arc<ClassInfo>>`.
- `file_context()` clones `Arc`s instead of deep-cloning.
- `ResolutionCtx::all_classes` and `VarResolutionCtx::all_classes`
  are `&[Arc<ClassInfo>]`.
- All downstream functions that accept `all_classes` or
  `local_classes` parameters (`resolve_variable_types`,
  `type_hint_to_classes`, `resolve_type_alias`, helpers in
  `source/helpers.rs`, `hover/variable_type.rs`, diagnostics,
  definition, references, etc.) accept `&[Arc<ClassInfo>]`.
- `find_class_at_offset` and `find_class_by_name` accept
  `&[Arc<ClassInfo>]` and return `&ClassInfo` via `Arc::Deref`.

The `class_loader` closures and `resolve_class_name` now return
`Option<Arc<ClassInfo>>`, eliminating the deep clone at the
class-loader boundary. All ~32 files that accept a `class_loader`
parameter use the updated signature
`&dyn Fn(&str) -> Option<Arc<ClassInfo>>`.

Consumers that only read the `ClassInfo` (checking methods,
properties, parent chains, etc.) access it through `Arc`'s `Deref`
at zero cost. Mutation sites (inheritance merging in
`resolve_class_with_inheritance`, parent chain walks, virtual member
providers) call `Arc::unwrap_or_clone()` to obtain an owned
`ClassInfo` only when they actually need to mutate.

### Remaining work

1. **`resolve_class_fully_cached`** and related functions — return
   `Arc<ClassInfo>`. Cache hits become a cheap `Arc::clone` instead
   of a deep copy. The cache already stores `Arc<ClassInfo>` internally
   but the public API still clones on the way out.
2. **Propagate `Arc<ClassInfo>` through downstream return types** —
   many intermediate functions (e.g. `resolve_static_owner_class`,
   `resolve_lhs_to_class`, `resolve_variable_subject`) currently
   unwrap the `Arc` from the class_loader and return an owned
   `ClassInfo`. Changing these to return `Arc<ClassInfo>` would
   eliminate additional deep clones along the hot path.

---

## P2. Recursive string substitution in `apply_substitution`
**Impact: Medium · Effort: High**

Generic type substitution (`apply_substitution`) does recursive
string parsing and re-building for every type string. It handles
nullable, union, intersection, generic, callable, and array types
by splitting, recursing, and re-joining strings. Each recursion
level allocates new `String` values.

This runs on every inherited method's return type, every parameter's
type hint, and every property's type hint when template substitution
is active. In a deeply-generic framework like Laravel (where
`Collection<TKey, TValue>` flows through multiple inheritance
levels), this function is called hundreds of times per resolution,
each time allocating new strings.

The resolved-class cache mitigates this by
caching the result, so substitution only runs on cache misses. But
cache misses still happen: first access, after edits that trigger
invalidation, and for generic classes with different type arguments.

The short-term mitigations (early-exit check and `Cow` return type)
are implemented. The remaining work is the long-term structural fix.

### Fix

Replace the string-based type representation with a parsed type AST
(an enum of `TypeNode` variants: `Named`, `Union`, `Intersection`,
`Generic`, `Nullable`, `Array`, `Callable`, etc.). Parse the type
string once during class extraction. Substitution becomes a tree
walk that swaps `Named` leaf nodes, avoiding all string allocation
and re-parsing.

This is a significant refactor that touches the parser, docblock
extraction, type resolution, and inheritance merging. It should be
evaluated after the lower-effort items are done and profiling
confirms that substitution remains a measurable cost.

---

## P3. Parallel pre-filter in `find_implementors`
**Impact: Medium · Effort: Medium**

`find_implementors` Phase 3 reads every unloaded classmap file
sequentially: `fs::read_to_string`, string pre-filter for the target
name, then `parse_and_cache_file`. On a project with thousands of
vendor classes, this loop is dominated by I/O latency. The string
pre-filter rejects most files (the target name appears in very few),
so the vast majority of reads are wasted.

### Fix

Split Phase 3 into two sub-phases:

1. **Parallel pre-filter.** Collect the candidate paths into a
   `Vec<PathBuf>`, then use `std::thread::scope` to read files and
   run the `raw.contains(target_short)` check in parallel. Return
   only the paths that pass the filter along with their content.

2. **Sequential parse.** For the (few) files that pass, call
   `parse_and_cache_file` sequentially. This step mutates `ast_map`
   and calls `class_loader`, which may re-lock shared state.

The same pattern applies to Phase 5 (PSR-4 directory walk for files
not in the classmap). The pre-filter I/O is the bottleneck; the
parse step processes very few files and is fast.

### Trade-off

Thread spawning overhead is only worthwhile when the candidate set
is large. Skip parallelism when the candidate count is below a
threshold (e.g. 8 files).

---

## P4. `memmem` for block comment terminator search
**Impact: Low-Medium · Effort: Low**

The current block comment skip in `find_classes` and `find_symbols`
uses `memchr(b'*', ...)` and then checks the next byte for `/`.
This is effective but can false-match on `*` characters inside
docblock annotations (e.g. `@param`, `@return`, starred lines).
Each false match falls through to a single-byte advance, which is
correct but suboptimal for large docblocks.

### Fix

Replace `memchr(b'*', ...)` with `memmem::find(content[i..], b"*/")`.
This searches for the two-byte sequence `*/` directly, skipping all
intermediate `*` characters in a single SIMD pass. The `memmem`
searcher is already imported and used for keyword pre-screening.

For typical PHP files this is a marginal improvement. For files with
very large docblocks (e.g. generated API documentation classes with
hundreds of `@method` tags), it avoids O(n) false `*` matches inside
the comment body.

---

## P5. `memmap2` for file reads during scanning
**Impact: Low-Medium · Effort: Low**

All file-scanning paths (`scan_files_parallel_classes`,
`scan_files_parallel_psr4`, `scan_files_parallel_full`, and the
`find_implementors` pre-filter) use `std::fs::read(path)` which
copies the entire file into a heap-allocated `Vec<u8>`. When the OS
page cache already has the file mapped, `memmap2` can provide a
read-only view of the file's pages without any copy.

### Fix

Add `memmap2` as a dependency. In the parallel scan helpers, replace
`std::fs::read(path)` with `unsafe { Mmap::map(&file) }`. The
`find_classes` and `find_symbols` scanners already accept `&[u8]`,
so the change is confined to the call sites.

### Safety

Memory-mapped reads are `unsafe` because another process could
truncate the file while the map is live, causing a SIGBUS. In
practice this does not happen during LSP initialization (the user is
not deleting PHP files while the editor starts). A fallback to
`fs::read` on map failure handles edge cases.

### When to implement

Profile first. On Linux with a warm page cache the difference
between `read` and `mmap` is small for files under ~100 KB (which
covers most PHP files). The benefit is more pronounced on macOS
where `read` involves an extra kernel-to-userspace copy. If
profiling shows that file I/O is no longer the bottleneck after
parallelisation, this item can be dropped.

---

## P6. O(n²) transitive eviction in `evict_fqn`
**Impact: Low-Medium · Effort: Low**

The `evict_fqn` function in `virtual_members/mod.rs` runs a
fixed-point loop that scans the entire resolved-class cache on each
iteration to find transitive dependents. In a large project with a
deep class hierarchy (common in Laravel codebases with hundreds of
Eloquent models), editing a base class can trigger a cascade of
evictions where each round does a full cache scan.

The `depends_on_any` helper also matches against both the FQN and
the short name of the evicted class, which increases the chance of
false-positive transitive evictions (e.g. two unrelated classes that
share a short name like `Builder`).

### Fix

Build a reverse-dependency index (`HashMap<String, Vec<String>>`)
that maps each FQN to the set of cached FQNs that directly depend
on it. Maintain this index alongside cache insertions and removals.
On eviction, walk the reverse index instead of scanning the entire
cache, turning the O(n²) loop into O(dependents).

If the reverse index is too much bookkeeping, a simpler first step
is to collect all dependents in a single pass (instead of the
current iterative fixed-point loop) by doing a breadth-first walk
of the dependency graph within the cache.

---

## P7. `diag_pending_uris` uses `Vec::contains` for deduplication
**Impact: Low · Effort: Low**

`schedule_diagnostics` and `schedule_diagnostics_for_open_files`
deduplicate pending URIs with `Vec::contains`, which is O(n) per
insertion. When a class signature changes, every open file is
queued, and each insertion scans the entire pending list.

For typical usage (< 50 open files) this is imperceptible. It
becomes measurable only with hundreds of open tabs and rapid
cross-file edits.

### Fix

Replace `Vec<String>` with `IndexSet<String>` (from `indexmap`) or
`HashSet<String>` + a separate `Vec<String>` for ordering. The
worker drains the collection on each wake, so insertion order is
not important and a plain `HashSet` suffices.

---

## P8. `find_class_in_ast_map` linear fallback scan
**Impact: Low · Effort: Low**

The fast O(1) `fqn_index` lookup in `find_class_in_ast_map` covers
the common case. The slow fallback iterates every file in `ast_map`
linearly. The comment says this covers "race conditions during
initial indexing" and anonymous classes.

During initial indexing with many files open, the fallback could
cause micro-stutters if the `fqn_index` has not been populated yet
for a requested class. In steady state the fallback is rarely hit.

### Fix

Audit the code paths that can reach the fallback to determine
whether they are still reachable after the `fqn_index` was added.
If they are not, replace the fallback with a `None` return and a
debug log. If they are, consider populating `fqn_index` earlier in
the pipeline (e.g. during the byte-level scan phase) to close the
window.

---

## P9. Pull diagnostics (`textDocument/diagnostic`)
**Impact: Medium-High · Effort: Medium**

Replace the current push-based diagnostic model
(`textDocument/publishDiagnostics`) with the pull-based protocol from
LSP 3.17 (`textDocument/diagnostic` and `workspace/diagnostic`). In
the pull model the editor requests diagnostics when it needs them,
rather than the server pushing after every edit.

### Current pain points

1. **Wasted passes on cross-file changes.** When a class signature
   changes, `schedule_diagnostics_for_open_files` queues every open
   file. With 30 open tabs the server runs 30 diagnostic passes, even
   though the user can only see 1-2 files. Each pass runs Phase 2
   collectors (unknown class, unknown member, argument count) which
   involve type resolution and class loading.

2. **PHPStan double-publish flicker.** Phase 1 publishes immediately
   with *stale* PHPStan results, then Phase 2 publishes the full set.
   When PHPStan finishes later it publishes a third time. Between
   these publishes diagnostics can briefly disappear and reappear.

3. **Redundant recomputation.** Files that were not affected by an
   edit still get a full diagnostic pass because the push model has
   no way to say "nothing changed."

### How pull diagnostics fix this

- **Visibility-driven.** The editor only requests diagnostics for
  files the user is looking at. The other 28 tabs are refreshed
  lazily when the user switches to them.

- **`workspace/diagnostic/refresh`.** After a class signature change
  or after PHPStan finishes, the server sends a refresh signal. The
  editor decides which files to actually re-request based on
  visibility. This replaces the `schedule_diagnostics_for_open_files`
  blunderbuss.

- **`resultId` cache hits.** Pull diagnostics support a `resultId`
  field. If the server returns the same `resultId` for a file, the
  editor knows nothing changed and skips the UI update. For files
  not affected by an edit the server can return the cached result
  instantly without recomputing.

- **Cleaner PHPStan integration.** Instead of three sequential
  publishes (fast, fast+slow, fast+slow+PHPStan), the server returns
  whatever it has when the editor asks, and sends a refresh signal
  when PHPStan completes. No flicker, no stale merging.

### Implementation plan

1. **Register capabilities.** Set `diagnostic_provider` in
   `ServerCapabilities` with `inter_file_dependencies: true` (class
   signature changes affect other files) and
   `workspace_diagnostics: true`.

2. **`textDocument/diagnostic` handler.** Accept
   `DocumentDiagnosticParams` (contains URI, optional
   `previous_result_id`). If the file has not changed since the last
   request (same `resultId`), return
   `DocumentDiagnosticReportKind::Unchanged`. Otherwise run the
   existing Phase 1 + Phase 2 collectors synchronously and return the
   diagnostics with a new `resultId` (e.g. a monotonic counter or
   content hash).

3. **`workspace/diagnostic` handler.** Return diagnostics for all
   open files, using the same `resultId` logic per file.

4. **Refresh signals.** Replace `schedule_diagnostics_for_open_files`
   with `client.workspace_diagnostic_refresh()`. Replace the PHPStan
   worker's direct `publish_diagnostics` call with a cache update +
   refresh signal.

5. **PHPStan integration.** The PHPStan worker stores its results in
   `phpstan_last_diags` as today. After updating the cache it sends
   `workspace/diagnostic/refresh`. The next pull request for that
   file merges the cached PHPStan results with fresh native results,
   producing a single consistent response.

6. **Backward compatibility.** Keep the push model as a fallback for
   clients that do not advertise pull diagnostic support in their
   `ClientCapabilities`. Check
   `capabilities.text_document.diagnostic` during `initialize`.

### Expected gains

- **30-open-tab scenario:** diagnostic passes drop from 30 to 1-2
  after a cross-file edit (only visible files).
- **PHPStan:** eliminates the double-publish flicker entirely.
- **Idle CPU:** files not being viewed generate zero diagnostic work
  until the user switches to them.