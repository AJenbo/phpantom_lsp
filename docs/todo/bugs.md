# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

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
