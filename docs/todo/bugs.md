# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

### B1. A rename reaching a Blade prologue writes into the template

The Blade preprocessor puts declarations no template wrote into the
virtual PHP's prologue: `$errors` and `$__env`, the `@var` docblocks that
carry the injected variable types, and the `extends` clause of the
wrapper class a bound `$this` synthesizes. Those name real classes, and
the reference scanner reads a template's virtual PHP, so the names count
as references to them.

`BladeSourceMap::php_to_blade` maps every position above the prologue to
Blade `0:0` (there is no template text behind it). Find-references
therefore reports the template at `0:0`, and rename produces a text edit
there: an empty range at the very start of the file, so renaming a class
a template only mentions through its prologue *inserts the new name into
the template*. Renaming `App\Models\Order` while `resources/views/page.blade.php`
receives an `$order` of that type prepends `Invoice` to the template.

The fix is to drop matches that fall inside the prologue rather than
clamping them: a prologue position is not a position in the template, and
no consumer wants one. `php_to_blade` cannot express that today, so it
needs a fallible form (`try_php_to_blade`) that returns `None` above
`prologue_lines`, with the reference, rename, highlight, and code-action
paths filtering on it.
