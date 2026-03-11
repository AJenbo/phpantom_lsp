# PHPantom — Performance

Internal performance improvements that reduce latency, memory usage,
and lock contention on the hot paths. These items are sequenced so
that structural fixes land before features that would amplify the
underlying costs (parallel file processing, full background indexing).

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label | Scale |
|---|---|
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low** |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## 2. Reference-counted `ClassInfo` (`Arc<ClassInfo>`)
**Impact: High · Effort: Medium**

`ClassInfo` is a large struct: 30+ fields including `Vec<MethodInfo>`,
`Vec<PropertyInfo>`, `Vec<ConstantInfo>`, multiple `HashMap`s, and
many `Vec<String>` fields. It is deep-cloned constantly:

- `find_class_in_ast_map` returns `Some(cls.clone())`
- `find_or_load_class` clones the result from the ast_map
- `resolve_class_with_inheritance` starts with `class.clone()` and
  clones every parent method/property during merging
- `resolve_class_fully_inner` calls `resolve_class_with_inheritance`
  (more clones), then caches the result with `.clone()`
- `resolve_target_classes` returns `Vec<ClassInfo>` (each a full clone)
- Cache retrieval clones on read: `map.get(&key) → cached.clone()`

A single completion on `$this->` in a class with a deep inheritance
chain can produce dozens of full `ClassInfo` clones, each involving
deep copies of all method signatures, parameter lists, and docblock
strings.

Under full background indexing (indexing.md Phase 5), the ast_map
holds thousands of `ClassInfo` values. Cloning them out on every
lookup produces significant allocation pressure.

### Fix

Store `Arc<ClassInfo>` in `ast_map` instead of owned `ClassInfo`.
Retrieval becomes a cheap reference-count increment instead of a
deep copy. The `resolved_class_cache` should also store
`Arc<ClassInfo>` so that cache hits are free.

### Mutation

Inheritance merging (`resolve_class_with_inheritance`) mutates the
merged class. Use `Arc::make_mut` (copy-on-write) at the start of
merging: the first mutation clones the inner value, but subsequent
mutations on the same `Arc` are free. Code that only reads a
`ClassInfo` (the majority of call sites) never pays for a clone.

### Scope

This is a pervasive change that touches every function returning or
accepting `ClassInfo`. It can be done incrementally:

1. Change `ast_map` to store `Arc<ClassInfo>`. Update
   `find_class_in_ast_map` and `parse_and_cache_content_versioned`.
2. Change `resolved_class_cache` to store `Arc<ClassInfo>`. Update
   `resolve_class_fully_inner`.
3. Update `resolve_target_classes` and downstream consumers to accept
   `Arc<ClassInfo>` where possible.

Each step compiles and passes tests independently.

---

## 7. Recursive string substitution in `apply_substitution`
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

The resolved-class cache (type-inference.md §31) mitigates this by
caching the result, so substitution only runs on cache misses. But
cache misses still happen: first access, after edits that trigger
invalidation, and for generic classes with different type arguments.

### Fix (long-term)

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

### Fix (short-term)

Two targeted optimisations that reduce allocation without the full
refactor:

1. **Early exit.** Before recursing, check whether the type string
   contains any of the substitution map's keys. If no key appears
   as a substring, return the input unchanged (no allocation). This
   skips the majority of type strings that don't reference template
   parameters.

2. **Cow return type.** Change `apply_substitution` to return
   `Cow<'_, str>` instead of `String`. When no substitution occurs
   (the common case), return the borrowed input. Only allocate a new
   `String` when a replacement actually happens.

---

## 9. Parallel pre-filter in `find_implementors`
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

## 10. `memmem` for block comment terminator search
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

## 11. `memmap2` for file reads during scanning
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

## 12. O(n²) transitive eviction in `evict_fqn`
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

## 13. `diag_pending_uris` uses `Vec::contains` for deduplication
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

## 14. `find_class_in_ast_map` linear fallback scan
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

## 8. Incremental text sync
**Impact: Low-Medium · Effort: Medium**

PHPantom uses `TextDocumentSyncKind::FULL`, meaning every
`textDocument/didChange` notification sends the entire file content.
For large files (5000+ lines, common in legacy PHP), sending 200 KB
on every keystroke adds measurable IPC overhead.

The practical benefit is bounded: Mago requires a full re-parse
regardless of how the change was received. The saving is purely in
the data transferred over the IPC channel. For files under ~1000
lines this is negligible.

### User-visible impact

Most editors (VS Code, Zed, Neovim) handle full sync gracefully and
users will not notice a difference on typical files. The impact
becomes noticeable in two scenarios:

1. **Very large files (5000+ lines).** Legacy PHP files, generated
   code, and test fixtures can exceed 200 KB. On every keystroke the
   editor serializes the entire buffer and sends it over the IPC
   channel. With incremental sync, only the changed range (typically
   a few bytes) is sent.

2. **Remote / high-latency connections.** When the editor and
   language server communicate over a network (e.g. VS Code Remote,
   SSH tunnels), the per-keystroke payload matters more. Full sync
   sends orders of magnitude more data than incremental.

For the common case (local editing, files under 2000 lines), full
sync adds roughly 0.1–0.5 ms per keystroke of serialization overhead.
This is well within the editor's debounce window and imperceptible.

Intelephense uses incremental sync by default. Users switching from
Intelephense are unlikely to notice the difference unless they
regularly edit very large PHP files.

This item is already tracked in [lsp-features.md §17](lsp-features.md#17-incremental-text-sync)
and is included here for completeness. The effort and implementation
plan are unchanged. It is the lowest-priority performance item
because full-file sync is rarely the bottleneck in practice.