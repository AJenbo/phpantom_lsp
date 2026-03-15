# PHPantom — LSP Features

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## F1. PHPDoc block generation on `/**`

**Impact: Medium-High · Effort: Medium**

Typing `/**` above a function, class, property, or constant should
generate a complete doc block skeleton — not just offer tag completions
(which we already do via `@`-triggered completion inside existing doc
blocks). This is a distinct feature: the trigger is `/**` outside any
doc block, and the result is a multi-line snippet with all tags
pre-filled.

### What to generate

- **Functions/methods:** `@param` tags for every parameter (with type
  from native hint or inference), `@return` with the declared/inferred
  return type, `@throws` for uncaught exception types in the body.
- **Method overrides:** insert `{@inheritDoc}` instead of repeating the
  parent's documentation, unless the override changes the signature.
- **Classes/interfaces/enums:** `@package` or a blank summary line.
- **Properties:** `@var` with the declared/inferred type.
- **Constants:** `@var` with the value's type.

### Implementation

1. Register `/**` as a completion trigger (it already fires on
   individual characters; the trigger is the `/` after `**`).
2. In the completion handler, detect that the cursor is immediately
   after `/**` with only whitespace before the next declaration.
3. Find the declaration below the cursor (function, class, property,
   constant) by scanning forward in the AST or raw text.
4. Build a `CompletionItem` with `insertTextFormat: Snippet` containing
   the full doc block with tab stops for summary and description fields.
5. For functions, resolve parameter types using the same paths as
   signature help. For `@throws`, reuse the exception-detection logic
   from existing PHPDoc `@throws` completion.

**Note:** This is different from the existing `@`-triggered PHPDoc
completion which suggests individual tags inside an already-open doc
block. This generates the entire block from scratch.

---

## F2. Partial result streaming via `$/progress`

**Impact: Medium · Effort: Medium-High**

The LSP spec (3.17) allows requests that return arrays — such as
`textDocument/implementation`, `textDocument/references`,
`workspace/symbol`, and even `textDocument/completion` — to stream
incremental batches of results via `$/progress` notifications when both
sides negotiate a `partialResultToken`. The final RPC response then
carries `null` (all items were already sent through progress).

This would let PHPantom deliver the _first_ useful results almost
instantly instead of blocking until every source has been scanned.

### Streaming between existing phases

`find_implementors` already runs five sequential phases (see
`docs/ARCHITECTURE.md` § Go-to-Implementation):

1. **Phase 1 — ast_map** (already-parsed classes in memory) — essentially
   free. Flush results immediately.
2. **Phase 2 — class_index** (FQN → URI entries not yet in ast_map) —
   loads individual files. Flush after each batch.
3. **Phase 3 — classmap files** (Composer classmap, user + vendor mixed)
   — iterates unique file paths, applies string pre-filter, parses
   matches. This is the widest phase and the best candidate for
   within-phase streaming (see below).
4. **Phase 4 — embedded stubs** (string pre-filter → lazy parse) — flush
   after stubs are checked.
5. **Phase 5 — PSR-4 directory walk** (user code only, catches files not
   in the classmap) — disk I/O + parse per file, good candidate for
   per-file streaming.

Each phase boundary is a natural point to flush a `$/progress` batch,
so the editor starts populating the results list while heavier phases
are still running.

### Prioritising user code within Phase 3

Phase 3 iterates the Composer classmap, which contains both user and
vendor entries. Currently they are processed in arbitrary order. A
simple optimisation: partition classmap file paths into user paths
(under PSR-4 roots from `composer.json` `autoload` / `autoload-dev`)
and vendor paths (everything else, typically under `vendor/`), then
process user paths first. This way the results most relevant to the
developer arrive before vendor matches, even within a single phase.

### Granularity options

- **Per-phase batches** (simplest) — one `$/progress` notification at
  each of the five phase boundaries listed above.
- **Per-file streaming** — within Phases 3 and 5, emit results as each
  file is parsed from disk instead of waiting for the entire phase to
  finish. Phase 3 can iterate hundreds of classmap files and Phase 5
  recursively walks PSR-4 directories, so per-file flushing would
  significantly improve perceived latency for large projects.
- **Adaptive batching** — collect results for a short window (e.g. 50 ms)
  then flush, balancing notification overhead against latency.

### Applicable requests

| Request                        | Benefit                                                                         |
| ------------------------------ | ------------------------------------------------------------------------------- |
| `textDocument/implementation`  | Already scans five phases; each phase's matches can be streamed                 |
| `textDocument/references`      | Will need full-project scanning; streaming is essential                         |
| `workspace/symbol`             | Searches every known class/function; early batches feel instant                 |
| `textDocument/completion`      | Less critical (usually fast), but long chains through vendor code could benefit |

### Implementation sketch

1. Check whether the client sent a `partialResultToken` in the request
   params.
2. If yes, create a `$/progress` sender. After each scan phase (or
   per-file, depending on granularity), send a
   `ProgressParams { token, value: [items...] }` notification.
3. Return `null` as the final response.
4. If no token was provided, fall back to the current behaviour: collect
   everything, return once.

---

## F3. Incremental text sync

**Impact: Low-Medium · Effort: Medium**

PHPantom uses `TextDocumentSyncKind::FULL`, meaning every
`textDocument/didChange` notification sends the entire file content.
Switching to `TextDocumentSyncKind::INCREMENTAL` means the client sends
only the changed range (line/column start, line/column end, replacement
text), reducing IPC bandwidth for large files.

The practical benefit is bounded: Mago requires a full re-parse of the
file regardless of how the change was received, so the saving is purely
in the data transferred over the IPC channel. For files under ~1000
lines this is negligible. For very large files (5000+ lines, common in
legacy PHP), sending 200KB on every keystroke can become noticeable.

**Implementation:**

1. **Change the capability** — set `text_document_sync` to
   `TextDocumentSyncKind::INCREMENTAL` in `ServerCapabilities`.

2. **Apply diffs** — in the `did_change` handler, apply each
   `TextDocumentContentChangeEvent` to the stored file content string.
   The events contain a `range` (start/end position) and `text`
   (replacement). Convert positions to byte offsets and splice.

3. **Re-parse** — after applying all change events, re-parse the full
   file with Mago as today. No incremental parsing needed initially.

**Relationship with partial result streaming (F2):** These two features
address different performance axes. Incremental text sync reduces the
cost of _inbound_ data (client to server per keystroke). Partial result
streaming (F2) reduces the _perceived latency_ of _outbound_ results
(server to client for large result sets). They are independent and can
be implemented in either order, but if both are planned, incremental
text sync is lower priority because full-file sync is rarely the
bottleneck in practice. Partial result streaming has a more immediate
user-visible impact for go-to-implementation, find references, and
workspace symbols on large codebases.

---

## F4. File rename on class rename

**Impact: Medium · Effort: Medium**

When a class, interface, trait, or enum is renamed and the file follows
PSR-4 naming conventions (filename matches the class name), the file
should be renamed to match the new class name.

### Behaviour

1. During `textDocument/rename` on a `ClassDeclaration`, after
   building the normal text edits, check whether the definition file's
   basename (without `.php`) matches the old class name.
2. If it does, add a `DocumentChange::RenameFile` operation to the
   `WorkspaceEdit` that renames the file to `NewClassName.php` in the
   same directory.
3. If the client's `workspace.workspaceEdit.resourceOperations`
   capability does not include `rename`, fall back to text-only edits
   (no file rename).

### Namespace rename (future extension)

When the user renames a namespace segment, all files under the
corresponding PSR-4 directory could be moved to a new directory
matching the new namespace. This is significantly more complex
(directory creation, moving multiple files, updating all `namespace`
declarations and `use` imports) and should be a separate item. For
now, only single-class file renames are in scope.

### Edge cases

- **Multiple classes in one file.** Do not rename the file if it
  contains more than one class/interface/trait/enum declaration.
- **File doesn't match class name.** Do not rename (the project
  may not follow PSR-4).
- **Vendor files.** Already rejected by the existing vendor check.
- **`DocumentChange` vs `changes`.** The `WorkspaceEdit` must switch
  from the `changes` map to `documentChanges` array when file
  operations are included, since `changes` does not support renames.
  Check the client capability first.

