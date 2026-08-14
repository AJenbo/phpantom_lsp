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

### B149. `$matches` is typed as though the match succeeded

**Impact: Low · Effort: Medium**

`preg_match()` writes its out-parameter with the shape its pattern
describes, and the forward walker applies that shape wherever the call
appears, the code after a guard that may not have been taken included:

```php
if (preg_match('/(\d+)/', $s, $m)) { /* … */ }
echo $m[1];                 // string; at runtime the key may not exist
```

PHPStan types this as `array{}|array{0: string, 1: string}`, the empty
array being what a failed match leaves behind. Doing the same needs the
seeding to know which branch of the condition it is writing into, which
a single write into the shared scope before the branches are walked
cannot express.

The result is an under-report rather than a false positive: a key read
outside the guard resolves to `string` instead of `string|null`.

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
