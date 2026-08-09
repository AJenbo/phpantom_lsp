# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

### B69. A `Blueprint` macro is missing from the schema index until a migration is edited

`load_schema_index` takes the project's `Blueprint` macro closures so a
migration calling a custom column helper (`$table->money('total')`,
registered with `Blueprint::macro()`) contributes the columns that helper
adds. At startup the map it is handed is always empty:
`build_laravel_macro_index` is what fills `laravel_macros`, and it runs
later in `initialized()` than the schema load does.

So the initial index is built as if no macro existed, and every column a
macro adds is absent from the model's virtual properties: hover, completion,
and the unknown-property diagnostic all disagree with the database. Editing
any migration or `config/database.php` afterwards calls
`reload_laravel_schema_index`, which reads the (now populated) macro map and
produces the correct index, so the columns appear only after an unrelated
edit and the result depends on session history.

Fix: build the macro index before the schema index at startup, or rebuild
the schema index once the macro scan has run. The macro scan needs the
class index, so it cannot simply move ahead of `init_single_project`.

### B70. Migration discovery and the migration watcher disagree on ignored directories

Default migration discovery (`collect_default_migration_files`) walks the
workspace with the `ignore` crate, so a `database/migrations` directory that
is gitignored, hidden, or under `vendor/` contributes nothing to the schema
index. The watched-file path uses `is_migration_php_file`, which checks only
`vendor/` and the directory name, so editing a file in one of those same
directories *does* apply it to the index.

A project therefore ends up with a different schema index depending on which
migrations happen to have been touched in the session, and a table defined
only in an ignored migration appears on a model's virtual members after an
edit and disappears again on restart.

Fix: apply the same ignore rules on both paths, either by having
`is_migration_php_file` consult a `Gitignore` matcher built once per
workspace, or by testing candidate paths against the discovery walk's own
result set.
