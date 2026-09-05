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

### B312. A namespace move judges each reference by its recorded name, not by what the file actually spells

**Impact: High · Complexity: Medium**

`collect_fqn_reference_edits` in `src/rename/namespace.rs` decides both
*whether* to rewrite a class reference and *how* to spell the
replacement from the `ClassReference` span's recorded `name` and its
`is_fqn` flag. Neither says what the source text holds: `class_ref_span`
strips the leading `\` before storing the name, and records qualified
spellings that are not rooted at the global namespace as `is_fqn:
false`. Two defects fall out of the same mistake, and both hit
`textDocument/rename` on a namespace segment as well as
`phpantom_lsp move`.

**A rooted reference loses its root.** The `name.starts_with('\\')`
test that guards re-adding the backslash can never be true, so the span
(which covers the `\`) is replaced with an unrooted name:

```php
// file declares `namespace App\Providers;`
'page' => \App\Old\Widget::class,        // before
'page' => App\New\Widget::class,         // after `move 'App\Old' 'App\New'`
//         ^ now resolves to App\Providers\App\New\Widget
```

`::class` does not require the class to exist, so a morph map or a
container binding built this way keeps running and stores a name that
resolves to nothing. In type positions PHPStan catches it; in `::class`
positions nothing does.

**A reference written without the root is not rewritten at all.** Only
`is_fqn: true` spans are considered, so a qualified name in a file with
no `namespace` declaration — where PHP resolves it against the global
namespace, making it exactly the FQN — is left pointing at the old
name:

```php
// config/app.php, no namespace declaration
'providers' => [
    App\Old\Widget::class,               // untouched by the move
],
```

Laravel's `config/` is full of these, and the class the entry names is
loaded at boot, so the breakage surfaces as a boot failure rather than
as a diagnostic.

The fix is to resolve each `ClassReference` span through the file's own
context (`file_context(uri).resolve_name_at(name, offset)`, as
`resolve_class_rename_fqn` in `src/rename/class.rs` already does),
compare *that* against the moved prefix, and derive the replacement's
qualification from the span's source text rather than from the recorded
name — re-resolving the new spelling in the same context to confirm it
still names the moved class, and falling back to a rooted `\New\Name`
when it does not. The class-move path in `src/rename/class.rs` already
reads `source_text.starts_with('\\')` and is unaffected.

**Where to look:** `src/rename/namespace.rs`
(`collect_fqn_reference_edits`), `src/symbol_map/docblock.rs`
(`class_ref_span`), `src/rename/class.rs` (`resolve_class_rename_fqn`,
the qualification rule to mirror).

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
