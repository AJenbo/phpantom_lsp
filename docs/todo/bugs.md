# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Effort** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

## B2. A class declared in two files disappears when the winning file drops it

**Impact: Low-Medium · Effort: Medium**

A class name two files declare, which is how a package ships a variant
behind a `class_exists` guard, settles on the lowest-sorting URI. Only
that declaration is kept, so when the winning file stops declaring the
name (the class is renamed, deleted, or the file is removed) the name
becomes unresolvable even though the other file still declares it. The
user sees "class not found" on a class that exists, until the surviving
file happens to be re-parsed.

Duplicate *functions* keep their runners-up in
`SymbolIndex::duplicate_functions` and promote the next-lowest declarant
when the winner withdraws, which is what classes need too. Classes are
harder because two paths index them: `apply_ast_index_updates_batch` in
`parser/ast_update.rs` (editor files) and `parse_and_cache_content` in
`resolution.rs` (the lazily loaded vendor/stub path). Both would have to
go through the same declare/withdraw helpers, or the runner-up record
goes stale.

While in there: the two paths already disagree about duplicates.
`parse_and_cache_content` fills `fqn_uri_index` with `or_insert_with`
(first write wins) but `fqn_class_index` with `insert` (last write
wins), so for a duplicated name the URI go-to-definition jumps to and
the `ClassInfo` its members are read from can describe different files.

**Fix:** give classes the same declare/withdraw treatment functions
have, and route both indexing paths through it so the two class indexes
cannot disagree.
