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

### B72. A generic class named without its arguments returns its own template parameter

A subject written with no template arguments — a `@var ItemCollection $items`
docblock, a parameter or return type spelled without generics, anything that
names the class rather than instantiating it — resolves a member's
template-typed return to the *parameter itself*. `ItemCollection::first()`,
declared `@return TModel|null` on a class whose docblock says
`@template TModel of Item`, comes back as the class `TModel`, which resolves
nowhere.

Reproduced with a controller passing `$items->first()` to an unannotated
view: the injected prologue declares `@var TModel $item`, and the template
then reports `Cannot verify method 'title' — subject type 'TModel' could not
be resolved` on every member it reads. It is not specific to Blade or to
call-site inference, which only made it visible: the same subject reads the
same way in a plain PHP file, and Laravel's own collections are declared
this way (`PostCollection` in `examples/laravel` forwards
`@template TModel of BlogPost` to `Collection<TKey, TModel>`), so any
project that types a variable by a bare collection class hits it.

An unsupplied template parameter is not a class. Substitute the bound the
`@template` declares (`of Item` → `Item`), and `mixed` for a parameter
declared without one, so the type is the widest thing the declaration
guarantees rather than a name nothing can resolve.
