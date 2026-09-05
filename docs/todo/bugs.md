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

### B314. Blade templates are invisible to renames and moves

**Impact: Medium-High · Complexity: Medium-High**

A class reference inside a `.blade.php` template is never rewritten by
`textDocument/rename` or `phpantom_lsp move`, and nothing reports that
it was skipped, so `files_changed` reads as a complete count when it is
not. In a Laravel codebase the templates carry FQCNs in `@var`
doc-comments that Bladestan type-checks, and in `@php` blocks that are
ordinary PHP; a move leaves every one of them naming a class that no
longer exists.

Two separate things have to line up:

- The workspace index parses templates as raw PHP.
  `build_ast_index_update` (the parallel index path) runs the parser
  over the file's own bytes, unlike `update_ast`, which preprocesses a
  template into virtual PHP first. Everything Blade-specific is inline
  HTML to the raw parse, so the template's symbol map holds no class
  references to rewrite.
- The rename reads raw file content. `find_references` already handles
  templates by reading `blade_virtual_content` and translating
  positions back through `blade_source_maps`
  (`reference_file_content`); the namespace rewriter calls
  `get_file_content` instead. Feeding it virtual content without the
  translation would be worse than skipping — the offsets would land in
  the wrong place in the real file. Note that a template that *does*
  end up with a virtual-content symbol map trips the
  `matches_source` length check, and that check abandons the entire
  rename, so this needs the translation in place before the index
  starts preprocessing.

**Where to look:** `src/rename/namespace.rs`
(`collect_fqn_reference_edits`, `build_namespace_prefix_rename_edit`),
`src/parser/ast_update.rs` (`build_ast_index_update` vs `update_ast`),
`src/references/mod.rs` (`reference_file_content`), `src/blade/source_map.rs`.
