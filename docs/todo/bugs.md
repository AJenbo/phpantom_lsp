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

**a. A guard on `instanceof`.** The else path of `if ($id instanceof B)`
on an `A` keeps `A`, which spans `B`, so re-testing `$id instanceof B`
does not recover what the first test's branch filled:

```php
$a = null;
if ($id instanceof B) { $a = makeA(); }
return $id instanceof B ? takesA($a) : '';     // $a is still A|null
```

This wants the negative branch to carry the exclusion the condition
really proves ("an `A` that is not a `B`"), which is the representation
gap [T20](type-inference.md#t20-type-narrowing-reconciliation-engine)'s
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
