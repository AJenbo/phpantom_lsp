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

### B313. A namespace served by two PSR-4 roots moves both of them

**Impact: Medium · Complexity: Medium-High**

Composer allows an array of directories per PSR-4 prefix, and
`collect_psr4_entries` records one mapping per directory. A namespace
move then reads them inconsistently: `psr4_directory_for_namespace`
takes the *first* mapping that covers the destination, while
`build_namespace_psr4_rename_ops` sweeps *every* mapping that covers
the source. Moving one of the two roots therefore plans a move of both
onto the same destination:

```jsonc
"autoload-dev": { "psr-4": { "Tests\\": ["tests/", "shared/tests/"] } }
```

```
$ phpantom_lsp move --dry-run 'shared/tests' 'tests/Shared'
Error: failed to read …/tests/Unit/TokenTransferTest.php: No such file or directory
```

The directory the user named is resolved to the namespace `Tests`, and
from there the second root is indistinguishable from the first. It
fails loudly (`validate_moves` also rejects the duplicate destination),
so the damage is blocked rather than silent, but the message names a
file the user never mentioned and the move cannot be completed at all.

Honouring the directory the user actually passed means moving only the
classes declared beneath it, which the prefix-over-raw-text rewriter
has no granularity for — the same limitation F22 documents. Until that
exists, the move should refuse up front with a message naming both
roots, rather than failing on a path it derived.

**Where to look:** `src/rename/namespace.rs`
(`build_namespace_psr4_rename_ops`), `src/composer.rs`
(`psr4_directory_for_namespace`, `collect_psr4_entries`),
`src/move_cli.rs` (`namespace_from_dir`, `validate_moves`).

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
