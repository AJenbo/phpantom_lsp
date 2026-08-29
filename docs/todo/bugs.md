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
re-swept on 2026-08-29 (79 sites, down from 84 earlier the same day, 98
at the 2026-08-28 sweep, 126 before it and 180 at the 2026-08-27
triage). Site counts refer to that sweep; every mechanism was either
reproduced in a minimal project or confirmed by reading the guard
construct PHPStan honours. The sweep is a snapshot, so a site named here
may already read differently: re-run the analyser before working an
entry, and trim the shapes that no longer reproduce. Line numbers drift
by a line or two between sweeps — match on the surrounding construct, not
on the number.

Entries are grouped by the mechanism that has to change, not by the
symptom that surfaced: one entry is one root cause, however many shapes
it shows up in. Splitting a shape out into its own entry because it
reads differently in the source is how this list grew past forty in the
first place. If two entries would be fixed by the same change, they are
one entry. Defects too small to earn a row of their own are collected
under [Miscellaneous](#miscellaneous) rather than given one each.

Of the 74 distinct lines the latest sweep reports, 30 are attributed to
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

### B270. A proof keyed by a value the guard leaves unnarrowed isn't reconstructed

**Impact: Medium · Complexity: Very High**

What the narrowing store keys a proof against, and what it takes to read
that proof back. PHPStan keys specified types by expression string and
keeps them until something writes to that expression, so a proof recorded
about one spelling is available to every other occurrence of it. A branch
join now records what each path proved under the keys the two paths
disagree about, and a later test that re-establishes one of those keys
re-applies the rest — but only when the two paths left that key holding
values that cannot both be the one in hand. Where they overlap, the test
proves nothing and the proof is dropped. Two shapes fall in that gap.

**a. A guard on a plain boolean.** Entering `if ($isI)` narrows `$isI` to
`true`, but the path that skipped it is left with the declared `bool`
rather than `false`, and `true` is one of the values `bool` spans. So the
join records nothing and re-testing the flag recovers nothing:

```php
$a = null;
if ($isI) { $a = makeA(); }
return $isI ? takesA($a) : '';                 // $a is still A|null
```

The same gap makes `instanceof` one hop worse: the else path of
`if ($id instanceof B)` on an `A` keeps `A`, which spans `B`, so
re-testing `$id instanceof B` does not recover what the first test's
branch filled. Both want the negative branch to carry the exclusion the
condition really proves, which is the representation gap
[T20](type-inference.md#t20-type-narrowing-reconciliation-engine)'s
sure/sure-not split closes.

**b. Two variables assigned together, where checking one implies the
other through an enclosing disjunction.** `!$isI` combined with an
enclosing `$isI || $isJ` is what proves the other dim:

```php
if (!$isI && !$isJ) { return null; }
$constArray = $isI ? $types[$i] : $types[$j];  // the else arm needs $isJ
```

Reading the else arm's proof back means carrying the enclosing
disjunction as a live clause and resolving it against `!$isI`, which is
the clause algebra T20 plans rather than anything the join can record.

Neither shape reproduces in the PHPStan Source sample project any more —
both constructs were rewritten upstream between sweeps — so the repros
above are the record of them.

### B274. An unresolvable call inside a loop erases the accumulator instead of naming what it could not find

**Impact: Medium · Complexity: High**

13 sites, all PHPStan's own scope-merging idiom:

```php
$finalScope = null;
foreach ($executionEnds as $e) {
    $endScope = $e->getStatementResult()->getScope();   // Scope, not MutatingScope
    if ($finalScope === null) { $finalScope = $endScope; continue; }
    $finalScope = $finalScope->mergeWith($endScope);
}
```

`mergeWith()` is declared on `MutatingScope`, not on the `Scope`
interface `StatementResult::getScope()` returns, so a diagnostic on the
merge line is correct. What is wrong is *which* diagnostic: an
assignment whose right-hand side does not resolve records the variable
as unknown, unknown is the top of the join lattice, and the loop's fixed
point therefore feeds unknown back into the next iteration. By the last
walk the seed type is gone, so instead of "method `mergeWith` not found
on class `Scope`" — which is what the same call reports outside a loop —
the merge line reports "type of `$finalScope` could not be resolved",
and the reads below it that would have resolved fine against `Scope`
report unresolvable receivers of their own
(`SetNonVirtualPropertyHookAssignRule.php:80, 81, 90` call
`hasExpressionType()`, which `Scope` does declare, and
`NodeScopeResolver.php:1121` reads `$scope->getClassReflection()`).

The fix is to keep the seed type in the fixed point beside the unknown
result rather than letting the join collapse to unknown. The collapse is
deliberate — it is what stops a branch-local proof about an untyped
subject from escaping a join (see `ScopeState::merge_branch`) — so this
needs the join to distinguish "no type was ever known" from "a type was
known and one path lost it", not a relaxation of the existing rule.

Sites: `src/Analyser/NodeScopeResolver.php:1103, 1112, 1116, 1121, 5406, 5414`,
`src/Rules/Properties/SetNonVirtualPropertyHookAssignRule.php:64, 72, 80, 81, 90`,
`src/Rules/TooWideTypehints/TooWideParameterOutTypeCheck.php:47, 56`.

### B302. An `||` leg's own narrowing is applied as though the leg had held

**Impact: Medium · Complexity: Medium**

Entering a branch guarded by `A || B` proves only that one of them held,
but every narrowing pass reads the operands of a disjunction as if both
had, so a leg's conclusion reaches the branch body unconditionally:

```php
function f(?Variable $v, bool $flag): void {
    if ($v instanceof Variable || $flag) {
        acceptVariable($v);        // reported clean; $v is still ?Variable
    }
}
```

This is a false *negative* — the branch body is checked against a type
narrower than the guard proves — so it hides mismatches rather than
inventing them. The shape reproduces for `instanceof`, for the type
guards (`is_string($x) || $flag`), and for the null checks alike.

The disjunction split in `apply_disjunct_operand_narrowing`
(`type_engine/variable/forward_walk/cond_narrowing.rs`) is the correct
treatment — narrow one scope per leg and join them — but it runs *after*
the other passes and takes their already-leaked scope as its base, so it
can only add to what leaked rather than replace it. It is also limited to
chains with a conjunctive leg, because the join re-widens a subject that
an earlier conjunct pinned down when a leg's `instanceof` replaces rather
than intersects with it
(`$b instanceof Generic && ($cls === Generic::class || $b instanceof Template)`).
Fixing this properly means splitting the condition's operands into the
conjuncts and the disjunctions *first*, running the other passes over the
conjuncts only, and letting the join own every disjunction.

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
