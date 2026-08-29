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

All entries below come from triage of the PHPStan Source sample project,
re-swept on 2026-08-29 (84 sites, down from 98 at the 2026-08-28 sweep,
126 before it and 180 at the 2026-08-27 triage). Site counts refer to
that sweep; every mechanism was either reproduced in a minimal project or
confirmed by reading the guard construct PHPStan honours. The sweep is a
snapshot, so a site named here may already read differently: re-run the
analyser before working an entry, and trim the shapes that no longer
reproduce. Line numbers drift by a line or two between sweeps — match on
the surrounding construct, not on the number.

Entries are grouped by the mechanism that has to change, not by the
symptom that surfaced: one entry is one root cause, however many shapes
it shows up in. Splitting a shape out into its own entry because it
reads differently in the source is how this list grew past forty in the
first place. If two entries would be fixed by the same change, they are
one entry. Defects too small to earn a row of their own are collected in
[B301](#b301-narrowing-defects-with-a-single-site-each) rather than given
one each.

Of the 79 distinct lines the latest sweep reports, 35 are attributed to
an entry below. The unattributed remainder is described in
[Not yet attributed](#not-yet-attributed).

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Reachability

No outstanding items.

## Narrowing

### B270. A proof the condition never states outright isn't reconstructed

**Impact: High · Complexity: Very High**

12 sites, and one root cause: what the narrowing store keys a proof
against, and what it takes to read that proof back. PHPStan keys
specified types by expression string and keeps them until something
writes to that expression, so a proof recorded about one spelling is
available to every other occurrence of it. Where we now match that for a
subject the condition names directly, we do not where the proof has to be
*reconstructed* — from a disjunction, or from the identical condition
tested a second time. That is the reconciliation engine planned as
[T20](type-inference.md#t20-type-narrowing-reconciliation-engine), and
both shapes below want it.

**a. A proof reached through a disjunction is lost** (8 sites). Entering
the branch means the whole condition held, so the guarded spelling is
narrowed on every path that gets there — but working that out means
reasoning about the disjunction rather than reading a recorded key. The
`||` leg has to be combined with the ternary that reads it back:

```php
if (
    ($stmt->valueVar instanceof Variable && is_string($stmt->valueVar->name))
    && ($stmt->keyVar === null || ($stmt->keyVar instanceof Variable && is_string($stmt->keyVar->name)))
) {
    $keyVarName = $stmt->keyVar instanceof Variable ? $stmt->keyVar->name : null;  // string|Expr|null
}
```

The `NodeScopeResolver` sites are all one `foreach` handler and all trace
to `$stmt->valueVar->name` / `$stmt->keyVar->name`. Two sites are a
*negated* `instanceof` on a property path
(`$expr instanceof FuncCall && !$expr->name instanceof Name`) that does
not reproduce from the construct alone, so start those by bisecting the
enclosing method. One is not narrowing at all: `getAttribute()` returns
`mixed`, and a `!== null` guard leaves it `mixed`, which we report as
unverifiable and PHPStan does not report on.
Sites: `src/Analyser/NodeScopeResolver.php:1702, 5000, 5103`,
`src/Analyser/NodeScopeResolver.php:2874`, `src/Analyser/TypeSpecifier.php:600`
(both negated `instanceof`),
`src/Analyser/NodeScopeResolver.php:3866 (×2)` (the `mixed` case),
`src/Rules/FunctionDefinitionCheck.php:195` (narrowed `$param`
`use`-captured by a closure).

**b. Re-testing a condition doesn't re-apply what it proved the first
time** (4 sites). Two shapes PHPStan's specified-types machinery
handles:

```php
// 1. The identical condition re-tested later:
if (count($args) > 0) { $acceptor = Selector::selectFromArgs(...); }
if (count($args) > 0) { use($acceptor); }                    // non-null

// 2. Two variables assigned together; checking one implies the other:
if ($assertions === null) { return null; } // $acceptor was set iff $assertions was
```

Shape 1 is what the three handler sites want. A branch join already
records what the branch proved under the variable it filled, so a `!==
null` test on that variable recovers it; what is missing is the same
thing keyed by the *condition* rather than by a variable, which is the
step that needs a proof keyed by a required type rather than by
non-nullness. The one `TypeCombinator` site is the else arm of
`$constArray = $constArrayIsI ? $types[$i] : $types[$j];`, which needs
shape 2: `!$isI` combined with the enclosing `$isI || $isJ` is what
proves the other dim.
Sites: `src/Analyser/ExprHandler/FuncCallHandler.php:977`,
`src/Analyser/ExprHandler/MethodCallHandler.php:350`,
`src/Analyser/ExprHandler/StaticCallHandler.php:455`,
`src/Type/TypeCombinator.php:1994`.

### B274. A null-seeded accumulator filled in a loop keeps `null` (or loses its type entirely)

**Impact: High · Complexity: High**

19 sites, two symptoms of one shape — PHPStan's own scope-merging
idiom, repeated across five files:

```php
$finalScope = null;
foreach ($executionEnds as $e) {
    $endScope = $e->getStatementResult()->getScope();
    if ($finalScope === null) { $finalScope = $endScope; continue; }
    $finalScope = $finalScope->mergeWith($endScope);   // unresolved from here on
}
if ($finalScope !== null) { $finalScope->processNodes(...); }  // still unresolved
```

The `=== null` early-continue must leave the accumulator non-null on
the merge line, and the loop fixed point must not poison the variable
so badly that even an explicit `!== null` guard after the loop can't
recover it. The same root also leaves the accumulator nullable after a
loop that provably runs (`if (count($xs) > 0) { ... foreach ($xs) ... }`
or a literal `$files = [$file]`), and after a foreach over a local
array that every branch pushed into (reproduced minimally; also
`src/Analyser/TypeSpecifier.php:542`).

An inline `/** @var … */` on the assignment inside the loop does not
rescue it either: the accumulator still leaves the loop nullable, so
`return $parameterSchema;` reports the `null` against a declared
`Schema` (`src/DependencyInjection/ContainerFactory.php:403`).

The damage spreads one hop: `$scope = $finalScope->rememberConstructorScope()`
leaves `$scope` unresolved too, so the next use of it reports an
unresolvable receiver of its own
(`src/Analyser/NodeScopeResolver.php:1121`, whose subject reads
`$scope->getClassReflection()`). Nothing is wrong with that line —
recovering the accumulator recovers it.

Sites: `src/Analyser/NodeScopeResolver.php:1103, 1112, 1116, 1121, 1903, 2001, 2153, 5406, 5414`,
`src/Rules/Properties/SetNonVirtualPropertyHookAssignRule.php:64, 72, 80, 81, 90`,
`src/Rules/TooWideTypehints/TooWideParameterOutTypeCheck.php:47, 56`,
`src/Reflection/BetterReflection/SourceLocator/OptimizedDirectorySourceLocator.php:149, 150`,
`src/Analyser/TypeSpecifier.php:542`,
`src/DependencyInjection/ContainerFactory.php:403`.

### B301. Narrowing defects with a single site each

**Impact: Medium · Complexity: Medium-High**

2 sites, two independent mechanisms. Neither is large enough to earn a
backlog row of its own, so they are collected here rather than filed
separately. Fixing one does not fix the other — take them one bullet at
a time.

**b. An `int|float` subject survives a guard that should split it.**
The plain `is_float()` shape resolves correctly on its own — an
`int|float` subject comes back as `float` in the `is_float()` branch,
`int` in the else, and `int` after a branch that reassigns it. Neither
site reproduces from that shape alone, and neither reproduces from the
constructs named below either, so what carries the defect is the branch
structure around them and not any one of these lines:

```php
// 1. The subject reaches the guard through a swap destructuring,
//    starting out int|float|null; we report `null|int|int|float`.
if ($min !== null && $max !== null && $min > $max) { [$min, $max] = [$max, $min]; }
if (is_float($min)) { $min = (int) ceil($min); }
IntegerRangeType::fromInterval($min, $max);

// 2. An inline `@var int|float` subject checked with an elseif;
//    the elseif branch keeps `int|float` where it must be `int`.
/** @var int|float $newAutoIndex */
$newAutoIndex = $offsetValue + 1;
if (is_float($newAutoIndex)) { … } elseif (!$optional) { $this->nextAutoIndexes = [$newAutoIndex]; }
```

The duplicated `int` and the surviving `null` in the first result say
the assignment, not the guard, is where the type is lost. Both sites sit
several branches deep inside long methods, so the next attempt should
start by bisecting the enclosing method down to a reproducing shape
rather than from the excerpts above. Sites:
`src/Reflection/InitializerExprTypeResolver.php:2533 (both args)`,
`src/Type/Constant/ConstantArrayTypeBuilder.php:242`.

**d. By-reference out-parameters.** Complements
[T41](type-inference.md#t41-param-out-is-parsed-but-never-read):

- A by-ref parameter the callee unconditionally assigns (no
  `@param-out` tag) should get the assigned type after the call
  (`ScopeOps::getTypeFromCache(..., ?string &$key)` always sets a
  `string`; `src/Analyser/MutatingScope.php:1031`).
- The *input* type of a by-ref argument that merely creates the
  variable must not be checked at all — PHPStan skips it
  (`preg_match_all(..., $matches, PREG_OFFSET_CAPTURE)` where the
  variable still holds the previous iteration's shape;
  `src/Parser/RichParser.php:183`).

## Arithmetic

No outstanding items.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

No outstanding items.

## Not yet attributed

44 of the 2026-08-28 sweep's 93 distinct lines are not attributed to an
entry above. Most are neighbours of a shape that already has an entry and
need only a read to place; the recurring ones are recorded here so a
future triage starts from them rather than rediscovering them.

- **Four are not our bug.** `PHPStan\ExtensionInstaller\GeneratedConfig`
  is written at install time and is absent from a plain checkout, so
  `GeneratedConfig::EXTENSIONS` genuinely names a class that is not
  there (`src/Analyser/ResultCache/ResultCacheManager.php:1396`,
  `src/Command/CommandHelper.php:309`,
  `src/Diagnose/PHPStanDiagnoseExtension.php:132, 142`). They are
  correct findings against the sample as checked out, not false
  positives; a sweep that counts them is counting four too many.
- **`src/Testing/TestCaseSourceLocatorFactory.php:75` is also not our
  bug.** `dirname($vendorDirProperty->getValue($classLoader))` reads
  Composer's `ClassLoader::$vendorDir`, which its own docblock types
  `string|null`, and nothing between the `hasProperty()` guard and the
  call rules the `null` out. `dirname()` rejects it under
  `strict_types`. PHPStan stays quiet only because reflection reads are
  `mixed` to it, so this is a place where we are the more accurate of the
  two rather than a false positive.
- **A `foreach` key reaching a `string` parameter** —
  `src/DependencyInjection/ConditionalTagsExtension.php:45` and
  `src/Rules/PhpDoc/WrongVariableNameInVarTagRule.php:376` both report
  "expects string, got int", so the key type of an array whose keys are
  strings is coming back as the `int` half of `array-key`.
- **Unresolved receivers with no entry yet**:
  `src/Analyser/ResultCache/ResultCacheManager.php:832, 837` (`$error`),
  `src/DependencyInjection/NeonAdapter.php:102, 103` (`$st`),
  `src/Type/Regex/RegexGroupParser.php:160` (`$child`),
  `src/PhpDoc/StubValidator.php:57`
  (`$pathRoutingParser`), `src/Analyser/ScopeOps.php:511` (an empty
  subject name, which is a reporting bug of its own).
- **`src/Analyser/ResultCache/ResultCacheManager.php:727`** returns
  `non-empty-list<int>|list<string>` where `array<string>` is declared —
  an array built in two branches that keeps the key type of one and the
  value type of the other.
