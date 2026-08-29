# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Complexity** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

Bugs land here from wherever they surface: found while working on another
task, or sweeps of the sample projects under `projects/`. Entries are
grouped by the mechanism that has to change, not by the symptom that
surfaced: one entry is one root cause, however many shapes it shows up in.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Reachability

No outstanding items.

## Narrowing

### B309. Re-testing a call result does not recover what the first test's branch assigned

**Impact: Medium · Complexity: High**

PHPStan remembers the types of arbitrary expressions, keyed by their
printed form — including method calls. After
`if ($id->isClass()) { $files = ['a']; }` merges, a later
`if ($id->isClass())` re-selects the branch facts: inside it, `$files`
is the non-empty literal again, so a `foreach` over it provably runs and
its loop variables are definitely assigned. Our walker keys only
variables, so the second test proves nothing and the loop variables stay
possibly-`null`:

```php
if ($identifier->isClass()) {
    $files = ['a.php'];
} elseif ($identifier->isFunction()) {
    $files = $this->findFilesByFunction();   // list<string>, may be empty
} else {
    return null;
}

if ($identifier->isClass()) {
    $node = null;
    foreach ($files as $file) {              // provably non-empty here
        $node = $this->fetch($file);
    }
    $this->takesNode($node);                 // reports null|Node
}
```

The fix direction is the expression-string keying PHPStan uses: record
call results in the scope alongside variables so a repeated test
correlates the branches. (Whether a call is safe to remember — purity,
intervening writes to the receiver — is part of the design.) Sweep
sites: `src/Reflection/BetterReflection/SourceLocator/OptimizedDirectorySourceLocator.php:149-150`
(PHPStan Source, 2026-08-29).

## Arithmetic

No outstanding items.

## Symbol resolution

### B310. An assignment expression as a call receiver never resolves

**Impact: Medium · Complexity: Medium**

Calling a method directly on a parenthesized assignment loses the
receiver's type entirely — plain assignment, `??=` on a variable, and
`??=` on an array offset all report
`Cannot verify method 'm' — type of '' could not be resolved`:

```php
if (($x = $map[$key])->truthy()) { ... }             // unresolved
if (($cached ??= $map[$key])->truthy()) { ... }      // unresolved
if (($cache[$key] ??= $map[$key])->truthy()) { ... } // unresolved

$r = $cache[$key] ??= $map[$key];
if ($r->truthy()) { ... }                            // resolves fine
```

Two defects share the root: the subject resolver does not compute a type
for `Assign`/`AssignOp` receiver expressions, and the diagnostic prints
an empty subject name for them. Curiously, the shapes resolve in a
small scratch project but fail when the identical file is dropped into
`projects/pdepend` or `projects/phpstan-src` — some small-project
fallback rescues the case, which is worth understanding during the fix
but is not the fix: the resolver should handle assignment receivers
directly. Sweep site: `src/Analyser/ScopeOps.php:513` (PHPStan Source,
2026-08-29; the sibling construct at `:496` uses the assigned-variable
form and resolves).

## Array types

### B307. Foreach over an array with no declared key type yields an `int` key

**Impact: Medium-High · Complexity: Medium**

Docblocks that say nothing about keys — `mixed[]`, `T[]`, `array<T>` —
have key type `array-key`, and a `foreach` over such a value should
bind the key as `int|string`. We bind it as `int`, dropping the string
half, so string keys reaching string parameters report
`expects string, got int`:

```php
/** @param mixed[] $config */
function f(array $config): void
{
    foreach ($config as $type => $tags) {
        takesString($type);   // reports "expects string, got int"
    }
}
```

The same wrong key also explains the odd return-type report where a
list built from the keys merges with a string-literal fallback branch as
`non-empty-list<int>|non-empty-list<string>` against a declared
`array<string>` — with the key fixed the loop arm becomes
`list<int|string>` and the report collapses to the genuine half. Once
the key is `int|string`, confirm that an `is_int($key) { continue; }`
guard narrows the survivor path to `string` (the isolated `is_int`
negation on a `@param int|string` already narrows correctly).

Sweep sites (PHPStan Source, 2026-08-29):
`src/DependencyInjection/ConditionalTagsExtension.php:45`,
`src/Rules/PhpDoc/WrongVariableNameInVarTagRule.php:376`,
`src/Analyser/ResultCache/ResultCacheManager.php:727`.

## Docblock handling

No outstanding items.

## Miscellaneous

No outstanding items.
