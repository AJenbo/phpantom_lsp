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

## 1. FQN secondary index for `find_class_in_ast_map`
**Impact: High · Effort: Low**

`find_class_in_ast_map` is the Phase 1 lookup in `find_or_load_class`.
It iterates **every file's classes** in the `ast_map` to find a class
by short name + namespace match. In a project with hundreds of parsed
files, this O(files × classes_per_file) scan runs for every class
lookup in every resolution chain. A single completion request that
resolves `$this->` can invoke this dozens of times as it walks the
inheritance chain, loads traits, resolves interfaces, and processes
mixins.

The `class_index` (`HashMap<String, String>`) already maps FQN → URI
but stops short: after finding the URI, the code still iterates all
classes in that file to find the right one.

Under full background indexing (indexing.md Phase 5), "all files"
means every PHP file in the project. The linear scan becomes the
dominant bottleneck.

### Fix

Add a secondary index `HashMap<String, ClassInfo>` (or
`HashMap<String, Arc<ClassInfo>>` if §2 lands first) that maps
fully-qualified class names directly to their parsed `ClassInfo`.
This turns every Phase 1 lookup into an O(1) hash lookup.

### Maintenance

The index must be updated in `update_ast_inner` (when files are
opened/changed) and in `parse_and_cache_content_versioned` (when
files are loaded on demand via classmap, PSR-4, or stubs). Both
code paths already maintain `ast_map` and `class_index`, so adding
a third insertion is straightforward.

When a file is re-parsed, remove all old entries for that file's
classes (snapshot FQNs before overwriting `ast_map`) and insert
the new ones. This mirrors the existing `class_index` maintenance
in `update_ast_inner`.

### Migration path

Once the FQN index is in place, `find_class_in_ast_map` becomes a
single hash lookup. The linear scan can be kept as a fallback for
edge cases (e.g. anonymous classes that don't have stable FQNs) but
should never be the primary path.

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

## 3. `RwLock` for read-heavy maps
**Impact: Medium · Effort: Low (fixed)**

**Status:** Fixed. All read-heavy `Arc<Mutex<…>>` fields on `Backend`
have been replaced with `Arc<parking_lot::RwLock<…>>`:

- `ast_map`, `symbol_maps`, `use_map`, `namespace_map`
- `class_index`, `classmap`
- `global_functions`, `global_defines`
- `open_files`
- `workspace_root`, `psr4_mappings`

Fields that are rarely accessed or always written use
`parking_lot::Mutex` (no poisoning, no `Result` unwrapping):

- `resolved_class_cache` — frequently written (cache stores)
- `php_version`, `vendor_uri_prefix`, `vendor_dir_name`, `config` —
  written once during init, read rarely
- `diag_pending_uri` — tiny critical section

All `.lock().ok()?` / `.lock().map(…)` patterns were simplified to
direct `.read()` or `.write()` calls. `parking_lot` v0.12 added as a
dependency.

---

## 4. `HashSet` dedup in inheritance merging
**Impact: Medium · Effort: Low**

Throughout `inheritance.rs` and `virtual_members/`, member
deduplication uses linear scans:

```rust
if merged.methods.iter().any(|m| m.name == method.name) {
    continue;
}
```

This pattern appears 19+ times across the codebase. For a class with
deep inheritance (Eloquent models with traits, parent chain,
interfaces, and mixins can easily accumulate 100+ methods), each
method merge checks against all previously-merged methods.

With M methods across D inheritance levels, each dedup check is O(M),
giving O(M x D) per method and O(M² x D) total. For an Eloquent
model with ~150 inherited methods across ~8 inheritance levels, this
is ~180,000 string comparisons per resolution.

### Fix

At the start of `resolve_class_with_inheritance` and
`merge_traits_into`, build a `HashSet<String>` (or
`HashSet<&str>` borrowing from `merged.methods`) containing the
names of already-present members. Check the set instead of
scanning the vec. Update the set when a new member is pushed.

Do the same for properties (`merged.properties`) and constants
(`merged.constants`).

### Scope

The fix touches `resolve_class_with_inheritance`,
`merge_traits_into`, `merge_interface_members_into`, and
`collect_mixin_members` in the virtual members provider. All use
the same pattern and can share the approach.

The `merged.methods.iter().any(...)` calls in
`definition/implementation.rs` (member existence checks, not
merging loops) do not need this fix because they run once per
lookup, not per-member-per-level.

---

## 5. `Arc<String>` for file content in `open_files`
**Impact: Low-Medium · Effort: Low**

`open_files` stores `HashMap<String, String>`. Every call to
`get_file_content` clones the entire file content:

```rust
files.get(uri).cloned()  // clones a 50-200 KB String
```

Every completion, hover, go-to-definition, and diagnostic request
clones the full file content. For a 200 KB PHP file, that is a
200 KB allocation per request.

### Fix

Change `open_files` to `HashMap<String, Arc<String>>`. Retrieval
becomes a reference-count increment. The `did_open` and `did_change`
handlers construct the `Arc<String>` once on insertion.

### Callers

All callers receive `Arc<String>` instead of `String`. Most pass
the content by reference (`&str` via `Arc::as_str()` or `&*content`),
so the change propagates minimally. A few places that consume the
string (e.g. storing it in a local variable for the duration of a
request) just hold the `Arc` instead.

---

## 6. `Arc<SymbolMap>` to avoid snapshot cloning
**Impact: Low-Medium · Effort: Low**

`user_file_symbol_maps()` clones every user-file `SymbolMap` into a
`Vec<(String, SymbolMap)>` snapshot. Each `SymbolMap` contains a
sorted vec of `SymbolSpan`s (100-400 per file), variable definition
sites, and call sites. For a workspace with 500 PHP files, this
clones all 500 symbol maps, allocating potentially megabytes of data.

### Fix

Store `Arc<SymbolMap>` in `symbol_maps` instead of owned `SymbolMap`.
The snapshot becomes `Vec<(String, Arc<SymbolMap>)>` where each entry
is a cheap `Arc::clone`.

### Alternative

If §3 (`RwLock`) lands first, `user_file_symbol_maps` could hold a
read lock for the duration of the scan instead of cloning. This
avoids even the `Arc` overhead but holds the lock longer. For Find
References (which can take seconds), the `Arc` snapshot approach is
safer because it releases the lock immediately.

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

This item is already tracked in [lsp-features.md §17](lsp-features.md#17-incremental-text-sync)
and is included here for completeness. The effort and implementation
plan are unchanged. It is the lowest-priority performance item
because full-file sync is rarely the bottleneck in practice.