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

### B71. Completion in a Blade template edits the virtual PHP's coordinates
**Impact: Medium · Effort: Low**

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
**Impact: Medium · Effort: Medium**

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
**Impact: Medium · Effort: Medium**

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
**Impact: Medium · Effort: Low**

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
**Impact: Medium · Effort: Low**

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
**Impact: Medium · Effort: Medium**

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

### B78. `object{prop: Type}` shapes never match a concrete class, even when it satisfies the shape
**Impact: Medium-High · Effort: Medium**

`object{foo: int}` is correctly parsed and printed — it isn't mistaken for a
class name the way an unmodelled pseudo-type spelling used to be. But nothing
checks a concrete class or
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

### B79. Find-references falls back to variable-name-vs-class-name text matching when a receiver's type can't be resolved
**Impact: High · Effort: Medium-High**

`find_member_references` scopes candidates to the target method's class
hierarchy — but only when the receiver's type resolves. When
`resolve_subject_to_fqns` comes back empty (an untyped property, an
unannotated parameter, anything the forward walker can't pin down),
`unresolved_member_subject_matches_scope` takes over, and it isn't a type
check at all: it takes the receiver's *variable name* (`$context` →
`context`), normalizes it, and checks whether it textually matches the
*short name* of any class in the hierarchy (`Context` → `context`, plus a
couple of `Repository`/`Gateway`/`Repo` suffix variants). Any call
`$context->getAll()` anywhere in the project counts as a reference to
`Context::getAll()` once the receiver's type can't be resolved, regardless
of what `$context` actually holds — a `SymfonyContext`, an array, an
unrelated class that also happens to get assigned to a same-named
variable.

This is precisely the failure mode php-typing-conformance's LSP survey is
built to surface: `conformance/lsp/navigation.toml` plants same-named
decoy methods across the psysh checkout specifically so that "a large
extra count is how name-matching reference implementations become
visible" (`ProbeGrading::navigation()`'s own docblock). Our result for
`Psy\Context::getAll()` is `refs_expected = 10, refs_found = 10, refs_extra
= 3` — three results outside the curated reference set, which is
consistent with the heuristic firing on unrelated `getAll()` receivers
named similarly to `Context` rather than three references the fixture's
hand-curated list simply missed. (Not confirmed against the actual
checkout — `~/repo/php/steins-survey` is outside this repo — but the
mechanism is confirmed by reading `unresolved_member_subject_matches_scope`
directly, and it matches the documented decoy mechanism exactly.)

This runs on the same path `rename` uses
(`find_references_for_rename` → `find_member_references`), so it is not
just a references-panel accuracy issue: a rename on a method can silently
also rewrite an unrelated call site whose receiver variable happens to
share a name with the target class.

The heuristic exists for a real reason — Laravel's Eloquent/Builder
methods are frequently called through untyped or dynamically-typed
receivers, and the `Repository`/`Gateway`/`Repo` suffix handling in
`member_scope_name_keys` is clearly aimed at that pattern — so the fix is
to narrow it, not delete it. Options worth weighing: restrict the
fallback to contexts where the framework/project actually uses that
untyped-receiver convention (Laravel projects, the way other
Laravel-specific behaviour in this codebase is already gated on
`is_laravel_project`) rather than applying it universally; or require a
higher bar than a bare name match (e.g. only match when the hierarchy has
exactly one class, so the heuristic can't silently pick a wrong one out of
several same-named candidates).

### B80. A same-named user class loses to a pseudo-type keyword
**Impact: Low · Effort: Low**

A class the project declares takes precedence over a PHPDoc pseudo-type
of the same name — the class is a real symbol, the keyword is a
convention. We resolve it the other way round for the aliases that have a
native PHP counterpart, so a `@param Integer $value` annotating a
parameter of a user-declared `Integer` class is read as PHP's `integer`
alias for `int`, and passing an actual `Integer` instance is reported as
a mismatch:

```php
final class Integer {}

/** @param Integer $value */
function acceptsInteger($value): void {}

acceptsInteger(new Integer()); // reported: expects Integer, got App\Integer
```

`number` and `real` already get this right (`is_lowercase_only_pseudo_type`
keeps any casing other than all-lowercase out of the scalar), but
`integer`, `boolean`, `double`, and `resource` are folded case-insensitively
because PHP's own type keywords are. The distinction is that PHP has no
native `Integer` type: a class of that name is legal, so an annotation
naming one has to resolve to the class when the project declares it.

Reproduced in `php-typing-conformance/conformance/tests/`:
`phpdoc_advanced_pseudotype_class_precedence.php`.

