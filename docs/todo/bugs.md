# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

### B71. Completion in a Blade template edits the virtual PHP's coordinates

A completion item that carries a `TextEdit` puts the range it computed on
`content`, and for a `.blade.php` file `handle_completion` swaps `content`
for the preprocessed virtual PHP. Nothing translates the range back on the
way out (`goto_definition`, hover, inlay hints, and diagnostics all do),
so every such item names a position in a file the editor does not have:
completing a view name inside `@include('|')` on line 1 of a template comes
back as an edit at line 7, six prologue lines below where the user is
typing, and shifted along the line by the width of the directive's
replacement.

Reproduced against `@include('|')`, where the offered view names carry an
edit at `7:24` for a cursor at `1:10`. Every strategy that returns a
`TextEdit` or `additional_text_edits` is affected the same way: Laravel
string keys, array-shape keys, Eloquent column strings, request keys, and
the `use` statement an imported class name inserts.

Fix: translate the ranges of a Blade file's completion items back through
`try_translate_blade_range` before returning them, dropping an item whose
range falls in the injected prologue rather than clamping it to the start
of the template.
