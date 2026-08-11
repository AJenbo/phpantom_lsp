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
