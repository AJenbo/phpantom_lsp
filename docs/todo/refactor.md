# PHPantom — Refactoring

Technical debt and internal cleanup tasks. This document is the first
item in every sprint. The sprint cannot begin feature work until this
gate is clear.

> **Housekeeping:** When a task is completed, remove it from this
> document entirely. Do not strike through or mark as done.

## Sprint-opening gate process

Every sprint lists "Clear refactoring gate" as its first item,
linking here. When an agent starts a sprint, follow these steps
**in order**. No step may be skipped.

### Step 1. Resolve outstanding items

Read this document top to bottom. If there are any tasks listed in the
"Outstanding items" section at the bottom, complete every one of them.
Remove each task from this document as it is completed. After all tasks
are resolved, go to step 2.

If the "Outstanding items" section says "No outstanding items", go
directly to step 3.

### Step 2. Request a fresh session

After completing refactoring work, **stop and ask the user to start a
new session**. The analysis in step 3 must happen in a session where
no refactoring edits have been made. This prevents the analyst from
rubber-stamping work it just performed. Do not proceed to step 3 in
the same session where you completed step 1.

### Step 3. Analyze the codebase

This step produces a written analysis report. The report must be shown
to the user before any decision is made about the gate.

**Prerequisite:** You must be in a session where no refactoring edits
have been made (either a fresh session, or one where step 1 had no
work to do).

Run through **every section** of the analysis checklist below. For
each section, **actually read the relevant source files** using tools.
Do not rely on memory, summaries, or prior context. Open the files,
look at the code, and report what you find.

**Required output format.** For each checklist section, write:

1. **Which files you read** (list them by path).
2. **What you found** (specific observations with line numbers).
3. **Verdict: PASS or FAIL** with justification.

A section FAILs if it identifies work that should be done before the
sprint's feature tasks begin. A section PASSes only if you can point
to specific evidence (file sizes, grep results, code you read) that
confirms there is no problem.

"I didn't find anything" is not a PASS. "I read X, Y, and Z, checked
for A and B, and found no instances because [concrete reason]" is a
PASS.

After completing the full checklist:

- If **any section FAILed**: add concrete, actionable tasks to the
  "Outstanding items" section of this document. Each task must name
  the file(s) to change and describe what to do. Then go to step 1.
- If **all sections PASSed**: go to step 4.

### Step 4. Declare the gate clear

Remove the "Clear refactoring gate" row from the current sprint's
table in `docs/todo.md`. The sprint is now open for feature work.

This step may only be reached after step 3 produces an all-PASS
report. There is no shortcut.

---

## Analysis checklist

The checklist is scoped to the **current sprint's tasks**. Before
starting, read the sprint table in `docs/todo.md` and the linked
domain documents to understand which modules will be touched.

### 1. File size and module boundaries

- Identify the source files most likely to be touched by this
  sprint's tasks. Read each one. Report its line count.
- Any file over ~600 lines is a candidate for splitting. Look for
  natural seams: logically distinct groups of functions, multiple
  unrelated `impl` blocks, or a section that is already commented
  as a separate concern.
- Check whether any module is doing two jobs (e.g. parsing _and_
  resolution, or building _and_ formatting). If the sprint will add
  a third job to the same file, that file must be split now.
- Look for `mod.rs` files that have grown beyond a thin re-export
  layer. Logic that lives in `mod.rs` is harder to find and test.

**FAIL criteria:** A file that will be heavily modified during the
sprint exceeds 600 lines, or a module mixes unrelated concerns that
the sprint will make worse.

### 2. Test placement

- Check whether any `#[cfg(test)]` blocks exist inside `src/` files
  for the modules this sprint will touch. Inline tests are fine for
  pure unit tests on private helpers, but integration tests and
  anything that touches the `Backend` or multi-file resolution should
  live in `tests/`.
- Check whether the existing `tests/` files cover the modules the
  sprint will modify. List what coverage exists and what is missing.
- Look for test helper code duplicated across multiple test files.
  If the same fixture setup or assertion pattern appears more than
  twice, it belongs in `tests/common/mod.rs`.

**FAIL criteria:** Integration-level tests live in `src/`, or the
sprint will modify modules that have no test coverage at all, or the
same test helper is copy-pasted in three or more files.

### 3. Code duplication

- Grep for structurally similar functions across the modules the
  sprint will touch. Report what you searched for and what you found.
- Pay particular attention to: type string manipulation, AST node
  offset extraction, docblock text extraction, and `WorkspaceEdit`
  construction. These patterns tend to proliferate.
- If two code action handlers share a non-trivial pattern (e.g. "find
  the token at the cursor, determine its span, build an edit"), check
  whether a shared helper already exists or should be created before
  the sprint adds a third copy.

**FAIL criteria:** Two or more places implement the same non-trivial
logic (>10 lines of structurally similar code), and the sprint will
add another copy or modify one of the existing copies.

### 4. Performance and memory

- Look for any place where the full file AST is re-parsed inside a
  hot path (completion, hover, diagnostics) in the modules the sprint
  will touch. Re-parsing should happen at most once per request.
- Look for unbounded clones of `ClassInfo`, `MethodInfo`, or other
  large structs inside loops. These should be references or
  `Arc`-wrapped.
- Check whether any new data structures added in the previous sprint
  are stored per-file but never evicted. Unbounded growth in
  `DashMap` entries is a memory leak.
- Look for `Vec::contains` or `Vec::iter().find()` used as a set
  membership check on collections that could grow with the number of
  files. These should be `HashSet` or `DashSet`.

**FAIL criteria:** A hot path re-parses when it does not need to,
large structs are cloned in a loop, or a per-file data structure has
no eviction path.

### 5. Fragility and error handling

- Look for `unwrap()` and `expect()` calls in request-handling code
  paths (anything reachable from `server.rs`) in the modules the
  sprint will touch. A panic in a request handler crashes the language
  server. These should be `?` or explicit early returns.
- Check whether the sprint's target modules propagate errors up or
  silently swallow them with `let _ = ...` or empty `Err(_) => {}`
  arms. Silent failures produce confusing user-visible behaviour.
- Look for code that assumes a particular UTF-8 byte offset is a
  valid char boundary without checking. This is a common source of
  panics when files contain multibyte characters.
- Check whether any `Arc<RwLock<...>>` or `Arc<Mutex<...>>` is held
  across an `await` point or across a call that re-acquires the same
  lock. These cause deadlocks or unnecessary blocking.

**FAIL criteria:** `unwrap()`/`expect()` in a request handler, errors
silently swallowed in code the sprint will build on, or a lock held
across an await point.

### 6. Sprint-specific concerns

Read each feature task in the sprint and ask these questions. Answer
each one explicitly in the report:

- Will any task require touching a module that is already large or
  doing too many things? If so, it must be split now.
- Will any task duplicate logic that already exists elsewhere? If so,
  the shared helper must be extracted first.
- Will any task add a new data structure that needs an eviction path?
  The eviction must be planned before writing the feature.
- Will any task generate `WorkspaceEdit` responses? Check that the
  existing edit-building helpers (if any) are adequate, or that a new
  shared helper should be written before the first action is
  implemented.

**FAIL criteria:** Any "yes" answer to the above questions where the
prerequisite work has not already been done.

---

## What belongs here

Only add items that would actively hinder the upcoming sprint's work
or that have accumulated enough friction to justify a focused cleanup
pass. Small fixes that can be done inline during feature work should
just be done inline. Items do not need to be scoped to the sprint's
feature area, but they should be completable in reasonable time (not
multi-week rewrites that would stall the sprint indefinitely).

Each item must include:

- **What to do** (concrete action, not "consider refactoring X").
- **Which files to change** (list specific paths).
- **Why it matters for the sprint** (which task it unblocks or
  de-risks).

---

# Outstanding items

Recommended order: 3 → 4 → 5. Tasks 4 and 5 both churn the whole crate
(4 renames `Backend` fields, 5 moves the type-engine modules), so they
go last to avoid re-touching code that 3 moves; do 4 before 5 so 5's
module moves don't collide with 4's field renames.

All three remaining tasks are pure internal refactors. None gets a
changelog entry unless the work uncovers and fixes a concrete
user-visible bug. Run the Rust CI checks (`cargo nextest run`, `cargo
clippy -- -D warnings`, `cargo clippy --tests -- -D warnings`, `cargo
fmt`) after each task.

## 3. Move workspace init/indexing out of `server.rs` and `references/mod.rs`

**What to do.** `src/server.rs` (4,007 lines) ends with an
`impl Backend` block of workspace initialization and indexing
(~lines 2696–4007, ~1,300 lines) that has nothing to do with LSP
dispatch, and `src/references/mod.rs` hosts workspace-indexing
functions that have nothing to do with reference finding. Consolidate
both into a new `src/indexing/` module (declare `mod indexing;` in
`lib.rs`). Pure code motion via `impl Backend` blocks in the new
files; approximate current line numbers given for finding things.

- `indexing/init.rs` — `init_single_project` (~2696),
  `init_monorepo` (~2933), `init_no_composer` (~3102).
- `indexing/scan.rs` — `add_vendor_dir` (~3153),
  `rescan_composer_indexes` (~3374), `scan_autoload_files` (~3469),
  `scan_phar_archive` (~3686), `build_self_scan_composer` (~3776),
  `populate_autoload_indices` (~3877).
- `indexing/preload.rs` — `preload_autoload_files` (~3608) and
  `preload_autoload_files_with_progress` (~3614) from `server.rs`,
  plus from `src/references/mod.rs`: `ensure_workspace_indexed`
  (~125), `ensure_workspace_indexed_for_request` (~137),
  `ensure_workspace_indexed_with_progress` (~166),
  `parse_files_parallel_with_progress` (~314), and
  `parse_paths_parallel_with_progress` (~430).
- `indexing/watch.rs` — `apply_watched_file_changes` (~3194).

Two more relocations while in there:

- `warm_laravel_completion_cache` (`server.rs` ~2081) belongs in
  `src/virtual_members/laravel/`.
- The pull-diagnostics resultId-cache logic embedded in the
  `diagnostic` (~1589) and `workspace_diagnostic` (~1651) handlers
  belongs in a new `src/diagnostics/pull.rs`, leaving both handlers
  as thin delegations like the rest of `server.rs`.

**Constraint.** Several of these functions spawn threads sized with
`PARSE_WORKER_STACK_SIZE` (see the "Performance Anti-Patterns" §3
note in `CLAUDE.md`). Move that code verbatim — do not "simplify"
thread spawning or stack sizing while relocating it.

**Acceptance.** `cargo nextest run` passes with no test edits (except
import paths); `server.rs` is reduced to protocol handlers and
dispatch (~1,700 lines); behavior is unchanged.

**Why it matters.** Full background indexing has already shipped on
top of init logic scattered across `server.rs` and
`references/mod.rs`; leaving that logic unconsolidated guarantees more
sprawl as the feature continues to grow.

---

## 4. Group `Backend`'s remaining fields into sub-systems

**What to do.** `struct Backend` in `src/lib.rs` (starts at ~line 377)
still has fields that cluster into implicit sub-systems. The four
external-tool fields are already grouped (`ExternalToolWorker`); this
task introduces three more groups. It is a mechanical, crate-wide
field rename — do it as three separate passes (one per group), running
`cargo check` between passes, and run it as the **sole active agent**
(no parallel sub-agents; see "Never run project-wide rewrites in
parallel" in `CLAUDE.md`).

For each group: define the struct with a `fn new()` used by both
`Backend` constructors (~lines 1096 and 1206) and include the group in
the per-request clone (~line 1836); the `Arc`/`Mutex` wrappers move
into the new struct unchanged (clone semantics of a `Backend` clone
must not change).

**Group 1 — `DiagnosticState`** (define it in `src/diagnostics/`,
field name e.g. `diag`): `diag_version`, `diag_notify`,
`diag_pending_uris`, `diag_last_slow`, `diag_last_fast`,
`diag_last_full`, `diag_result_ids`, `diag_suppressed`,
`workspace_diags`, `workspace_diag_pass_started`. Drop the `diag_`
prefix on the struct's fields (`self.diag_version` →
`self.diag.version`). Leave `supports_pull_diagnostics` with the other
client-capability flags.

**Group 2 — `SymbolIndex`** (field name e.g. `symbols`):
`uri_classes_index`, `fqn_uri_index`, `fqn_origin_index`,
`fqn_class_index`, `class_not_found_cache`, `gti_index`,
`method_store`, `global_functions`, `global_defines`,
`uri_globals_index`, `autoload_function_index`,
`autoload_function_origin_index`, `autoload_constant_index`,
`autoload_constant_origin_index`, `autoload_file_paths`. Do **not**
pull in the per-file parse artifacts (`symbol_maps`, `resolved_names`,
`file_imports`, `file_namespaces`, `parse_errors`, `parsed_uris`,
`parse_inflight`, `phar_archives`) or the `stub_*` indexes — they have
a different lifecycle and are out of scope here.

**Group 3 — `WorkspaceEnv`** (field name e.g. `workspace` — not
"WorkspaceConfig", to avoid colliding with the existing
`config: Mutex<config::Config>` field, which becomes a member of this
group): `workspace_root`, `psr4_mappings`, `vendor_uri_prefixes`,
`vendor_dir_paths`, `vendor_package_origin_roots`, `php_version`,
`config`.

**Acceptance.** Pure renames: `cargo nextest run` passes with only
mechanical field-path updates in tests; no lock types, wrapper types,
or clone semantics change.

**Why it matters.** Group 1 directly de-risks the scheduled D10 task;
the rest makes the Backend's state graph legible and shrinks the
constructors.

---

## 5. Extract the shared type engine out of `completion/`

**What to do.** Despite the name, `src/completion/` houses the
project's single type-resolution engine — the code that answers "what
is the type of this expression here?" — consumed by diagnostics, hover,
go-to-definition, and signature help, not just completion. The name
misleads every new contributor and buries the most load-bearing
subsystem inside a feature module. Move the engine into a top-level
module (proposed `src/type_engine/`; name negotiable but **not**
`resolve`/`resolution`, which collide with the existing `resolution.rs`
class/function *name* lookup), leaving `completion/` with only
completion-specific code. This is pure code motion — `impl Backend`
blocks can live in any file of the crate, so methods move without
becoming free functions.

**Engine subtrees to move** (whole directories): `completion/resolver/`,
`completion/call_resolution/`, `completion/types/`, and the
type-resolution half of `completion/variable/` — `resolution.rs`,
`forward_walk/`, `rhs_resolution/`, `foreach_resolution.rs`,
`closure_resolution.rs`, `class_string_resolution.rs`,
`raw_type_inference.rs`. The root-level `subject_expr.rs`,
`subject_extraction.rs`, and `subject_resolution.rs` are part of the
engine too — fold them in.

**Stays in `completion/`:** `handler/`, `context/`, `phpdoc/`,
`builder.rs`, `target.rs`, `array_shape.rs`, `array_callable.rs`,
`named_args.rs`, `use_edit.rs`, `eloquent_string.rs`,
`laravel_route_controller.rs`, `laravel_string_keys.rs`, and the one
non-clean split: `completion/variable/completion.rs` is variable *name*
completion (scope collection for the completion list), not type
resolution, so it stays while its sibling type-resolution files move.

**Steps.**
1. Create the new module and move the engine subtrees into it, updating
   `mod`/`use` declarations.
2. Split `completion/variable/`: keep `completion.rs` (and any
   scope-collection helpers only it uses) under `completion/`; move the
   rest under `<engine>/variable/`.
3. Update imports crate-wide (`crate::completion::resolver::…` →
   `crate::type_engine::resolver::…`, etc.). Add thin `pub use`
   re-exports in `completion/mod.rs` only if they meaningfully shrink
   the diff; otherwise fix call sites directly.
4. Update the module map and the "shared type engine lives under
   `completion/`" note in `docs/ARCHITECTURE.md`, and the Project
   Structure landmark in `AGENTS.md`, to point at the new module.

**Acceptance.** `cargo nextest run` passes with only mechanical
import-path updates; behavior is unchanged; `completion/` contains only
completion features; the type engine has one obvious top-level home.

**Why it matters.** Anti-pattern #6 in `AGENTS.md` ("do not build a
second type-resolution path") depends on the engine being
discoverable. While it hides under `completion/`, contributors reach
for a "simpler" local resolver instead of finding the shared one. This
is the largest churn of the set (crate-wide import moves), so it goes
last.

---
