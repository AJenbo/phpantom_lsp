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

Most entries below come from the 2026-08-13 sample-project sweep (345
diagnostics across ten projects, ~330 of them false positives). Site
counts refer to that sweep; the git-ignored triage log has the full
per-project inventory. Entries filed later say where they came from.

## preg_match

### B144. `preg_match` `$matches` is shapeless

**Impact: Medium-High · Effort: Medium**

`$matches` is an array of unknown keys, so every group read off it is
`string` at best and the pattern's own capture groups say nothing:

```php
if (preg_match('/(?<amount>\d+)(?<unit>\w*)/', $size, $match)) {
    strtolower($match['unit']);   // no key check, no arity check
}
```

Two remaining defects:

1. No match-shape inference for literal patterns: PHPStan's
   `RegexArrayShapeMatcher` types group 0 and every always-matching
   group as `string` (`string|null` only under
   `PREG_UNMATCHED_AS_NULL`), with named groups as keys. PDepend,
   PHPMD, AGCMS and Bladestan all index `$match[1]`/`$match['name']`
   directly inside the guard.
2. `preg_match_all` group reads (`$matches[1]`) should be
   `list<string>`, not `array<string>`.

**Fix:** add a literal-pattern group-shape analysis (port the
capture-group walk from PHPStan's `RegexArrayShapeMatcher`). Depends
on B147: the matcher's result has nowhere to go until constant-array
shapes exist as a value representation.

## Array types

### B147. Array literals are not tuples: slot reads return the union of all elements

**Impact: Medium-High · Effort: Medium**

```php
$rows[] = [$violation, $location, $name];        // RuleViolation, string, string
foreach ($rows as $row) {
    [$violation, $location, $name] = $row;       // each: RuleViolation|string
    $writer->write($location);                   // reported: RuleViolation|string
}
```

A list literal collapses to `array<union-of-values>`, so list
destructuring and constant-offset reads cannot select a slot (6 sites
in PHPMD/PDepend). Two adjacent literal defects: a literal with a
*non-constant* key renders as the bogus shape `array{mixed: int}`
(stringifying the key's type as a field name) instead of falling back
to `array<K, V>`, and `(object) []` is not recognised as `stdClass`.

**Fix:** keep constant-array shapes for literals (ordered slots +
known keys), select slots on destructure/offset reads, fall back to a
generic array only for non-constant keys.

### B148. Element writes do not refine tracked array state

**Impact: Medium · Effort: Medium-High**

Several forms of the same weakness (~7 sites):

- `$a[$k][] = $v` never updates the inner element type: a value
  initialised as `[]` stays `array{}` in the outgoing type even
  though every loop iteration appends strings.
- A key written on every path through a loop body leaves
  `array<int, string>` where PHPStan reports
  `non-empty-array<int, string>`.
- `$a += ['slot' => $obj]` degrades to unconstrained `array`.
- A constant shape `array{item: string, qty: int}` fails the subtype
  check against `array<string, mixed>`, so shaped rows are rejected
  by a declared `array<int, array<string, mixed>>`.

**Fix:** refine the per-key state on nested writes (including
auto-vivification), merge `+=` like an array-shape union, and make
constant shapes satisfy their generic supertypes.
