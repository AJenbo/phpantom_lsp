# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B1. `analyze` flags framework Artisan commands as unknown

**Impact: Medium · Effort: Low**

`phpantom_lsp analyze` reports `Unknown command: 'queue:work'` for a
command the framework itself ships. Reproduce by adding
`Artisan::call('queue:work');` to `examples/laravel/app/Demo.php` and
running a **release** build of
`analyze --project-root examples/laravel` (a debug build runs a reduced
collector list that excludes the Laravel string-key checks, so the
false positive is invisible there).

The cause is which files populate the command index. The LSP's
`initialized` handler calls `build_laravel_command_index`, which scans
the whole FQN → URI index — vendor packages included. The headless
pipeline in `analyse/run.rs` never calls it, and instead gets entries
only from `refresh_laravel_command_index`, which `update_ast` fires per
parsed file. `analyze` parses user files, so the index ends up holding
the project's own commands and nothing else. Since it is then non-empty,
the "index is empty, discovery must have failed" guard in
`collect_invalid_laravel_string_key_diagnostics` does not fire, and every
vendor command name is reported as unknown.

`build_laravel_macro_index` is missing from the same block, with the same
shape of consequence: only macros registered in parsed user files are
recovered, so a macro registered by a vendor package's service provider
can produce a false-positive unknown member.

**Where to change:** call `build_laravel_command_index` and
`build_laravel_macro_index` from the Laravel block in `analyse/run.rs`,
next to `build_laravel_date_class`, `build_provider_resources`, and
`build_laravel_morph_map_index`.

