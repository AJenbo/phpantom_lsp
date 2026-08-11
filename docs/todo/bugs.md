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
