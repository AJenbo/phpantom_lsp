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

### B73. Narrowing survives a reassignment of the subject's base variable

The narrowing re-walk in `apply_property_narrowing` looks for a check on a
subject path anywhere in the enclosing body and applies it at the cursor.
It reads conditions only, so an assignment between the check and the use is
invisible to it:

```php
if ($a->value instanceof StringExpr) {
    $a = $other;              // $a->value is a different value now
    $a->value->value;         // still resolved as StringExpr
}
```

The forward walker gets this right (`invalidate_dependent_keys` drops every
key rooted at a reassigned variable), so during a diagnostic pass the scope
holds no entry for the path. But an absent scope entry is indistinguishable
from a path the walker never seeded, so the caller falls through to the
re-walk, which reinstates the narrowing the walker had just invalidated.
The result is a missing diagnostic on a member the value no longer has.

Property paths and argument-less call subjects (`$a->get()`) are affected
alike, since both resolve through the same re-walk.

Fix: teach the re-walk to drop a subject whose base variable is assigned
between the check and the cursor, rather than making the fall-through
conditional on which consumer is asking.

### B74. `analyze --format json` prints a prose note ahead of the payload

A project root without a `composer.json` makes `run_analysis` print
`Note: no composer.json found in … — analysing as a plain PHP project.`
on stdout before anything else, regardless of `--format`. Under
`--format json` that line sits ahead of the object, so a consumer that
parses stdout as JSON fails outright, and one that recovers has to cut
the payload out by brace-matching:

```console
$ phpantom_lsp analyze --format json --project-root /tmp/plain /tmp/plain
Note: no composer.json found in /tmp/plain — analysing as a plain PHP project.
{
  "totals": { "errors": 0, "file_errors": 0 },
```

The same applies to any future informational line on the machine-readable
formats. Fix: write the note to stderr, so stdout carries only the payload
the format promises.

### B75. A path that does not exist is reported as "No PHP files found" with exit 0

`analyze` resolves a relative `PATH` against `--project-root` rather than
the working directory, and a path that resolves nowhere produces
`No PHP files found.` and exit code 0 — the same answer as a clean run of
an empty directory. A caller cannot tell "analysed nothing, all good" from
"you pointed me at a path I could not find":

```console
$ phpantom_lsp analyze --project-root conformance conformance/tests/x.php
No PHP files found.
$ echo $?
0
```

Two things to settle here. A `PATH` that exists relative to the working
directory should be accepted as such (that is what every other analyzer
CLI does, and what a shell tab-completion produces), and a path that
resolves nowhere should be an error on stderr with a non-zero exit
distinct from 1 (which already means "diagnostics found").

### B76. `{@see method()}` unqualified inside its own class resolves as a global function

`@see` (and presumably `@link`/`@uses`) resolves a bare `name()` reference by
looking up a global function, never checking whether the enclosing class
declares a method of that name. A docblock on a class that implements an
interface, referring to one of its own unqualified methods, gets
"Function 'name' not found" instead of resolving to the method:

```php
/**
 * ... {@see covers()} draws that line ...
 */
final class QodanaChecker implements CoverageAware
{
    public function covers(TestCase $testCase): bool { ... }
}
```

phpDocumentor and PHPStorm both resolve a bare `name()` in `@see` against the
enclosing class's own members before falling back to a global function, which
is the convention this pattern relies on (see
`conformance/src/Checker/QodanaChecker.php` and `CoverageAware.php` in the
php-typing-conformance corpus for a real instance). Fix: when resolving an
`@see`-family reference, check the containing class's members (own, then
inherited) for a matching method/property/constant before falling back to a
global function/constant lookup.

### B77. An unrecognized PHPDoc pseudo-type spelling is treated as a class name and enforced literally

When the docblock parser meets a lowercase-hyphenated pseudo-type
spelling it doesn't have a keyword for — `value-of<T>`, `literal-int`,
`stringable-object`, `pure-callable`, and likely every other
`phpdoc_advanced_fallback_*`/`phpdoc_advanced_psalm_*` spelling in the
php-typing-conformance corpus that isn't already a recognized keyword —
it falls through to the same path as an unresolved class name: it gets
namespace-qualified against the file's own namespace and then used
verbatim as the declared type, so every call site is checked against a
type that can never match anything.

The result is not "not implemented" (an honest, expected outcome the
conformance suite explicitly does not treat as a defect) but active
false positives on entirely valid code, alongside missed detection of
the actually-invalid cases, which then get the same generic message as
the false positives:

```php
/** @return value-of<array{a: int, b: int}> */
function returnsValue(): int { return 1; }
// reported: "Return type 1 is incompatible with declared return type
//            value-of<array{a: int, b: int}>" — false positive, 1 IS a value-of

/** @param value-of<array{a: int, b: int}> $value */
function acceptsValue($value): void {}
acceptsValue(1);   // false positive: "expects value-of<...>, got 1"
acceptsValue('x'); // should report this — instead gets the identical generic message
```

`key-of<T>` shows the same shape but partially recognizes the syntax
(the type prints as `key-of<array{...}>` rather than being
namespace-qualified into a bogus class), so the parser has *started*
handling it, but nothing expands it to the union of the shape's
literal keys before enforcement runs — the effect on call sites is
identical.

Reproduced in `php-typing-conformance/conformance/tests/`:
`phpdoc_advanced_fallback_value_of.php`,
`phpdoc_advanced_fallback_key_of.php`,
`phpdoc_advanced_psalm_literal_int.php`,
`phpdoc_advanced_psalm_stringable_object.php`,
`phpdoc_advanced_fallback_pure_callable.php`,
`phpdoc_advanced_fallback_decimal_int_string.php`,
`phpdoc_advanced_fallback_non_decimal_int_string.php`,
`phpdoc_advanced_psalm_arraylike_object.php`. DEVSENSE (measured as `phpy`
in that suite) recognizes every one of these; it's the tool our own
diagnostics diverge from most sharply on this specific failure mode.

This violates the "no diagnostic suppression" principle in the
opposite direction: an unrecognized spelling must degrade to "not
recognized" (skip enforcement, and for docblock type positions ideally
fall back to `mixed`, the way B72 substitutes a `@template` bound),
never to "treat the spelling as if it were a class and enforce it
literally." Fix at the type-string parser
(`src/docblock/type_strings.rs` / `src/php_type/parse.rs`): a
pseudo-type spelling with no matching keyword and no matching class
must resolve to `mixed`, not to a synthesized class-like type name, and
`key-of`/`value-of` need their expansion (to the shape's key union /
value union) wired in before enforcement, not just accepted as syntax.

### B78. `object{prop: Type}` shapes never match a concrete class, even when it satisfies the shape

Unlike B77's cluster, `object{foo: int}` is correctly parsed and printed —
it isn't mistaken for a class name. But nothing checks a concrete class or
anonymous object against the shape structurally: every object argument is
rejected, including ones that satisfy it.

```php
final class Reading {
    public int $foo = 1;
}

/** @param object{foo: int} $shape */
function takesObjectShape(object $shape): void {}

takesObjectShape(new Reading());        // false positive: Reading has `foo: int`
takesObjectShape((object) ['foo' => 1]); // false positive: an anonymous stdClass with `foo: int`
takesObjectShape(new Mistyped());        // correctly invalid ($foo is string) — reported for the wrong reason
```

All three calls get the same generic "expects `object{foo: int}`, got X"
message, so the two valid calls and the one genuinely invalid call are
indistinguishable in the output — the diagnostic is never actually
checking property-by-property compatibility, just failing unconditionally
whenever the declared type isn't literally `object{foo: int}` itself.

Reproduced in
`php-typing-conformance/conformance/tests/phpdoc_advanced_fallback_object_shape.php`.
mir, Intelephense, and DEVSENSE (`phpy`) all pass this one; we're the
outlier.

Fix: when the expected side of a compatibility check is an object shape,
resolve the actual side to a class (or anonymous object literal) and check
each declared property against the shape's fields (type, and required vs.
optional) instead of falling through to nominal comparison. Likely lives
next to whatever `is_type_compatible` (see T32) already does for array
shapes against array literals — object shapes need the same treatment
against object literals and named classes.
