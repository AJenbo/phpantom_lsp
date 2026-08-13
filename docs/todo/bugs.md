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

### B99. Formatting a `.blade.php` file has no extension guard

**Impact: Medium · Effort: Low**

`textDocument/formatting` (`src/server.rs`/`src/formatting.rs`) hands a
document straight to the Pint/php-cs-fixer/phpcbf/mago pipeline with no
check on the file extension. [`docs/todo/blade.md`](blade.md#bl16-blade-aware-formatting)
plans real Blade-aware formatting (BL16) as future work and describes an
interim safety measure — explicitly disabling formatting on `.blade.php`
files until then — but that interim guard was never added. Running
"Format Document" on a `.blade.php` file today sends raw Blade markup
(directives, `{{ }}` echoes, component tags) through a PHP formatter,
which will most likely error out or, worse, silently produce nonsense
edits, rather than being a no-op.

**Fix:** add an extension check in the formatting handler that returns no
edits for `.blade.php` documents, ahead of BL16's real implementation.
