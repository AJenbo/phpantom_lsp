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

## Conditional and argument-dependent return types

### B143. A constant built from another constant does not fold to its value

**Impact: Medium · Effort: Medium**

A flags argument is bit-tested by resolving each `|` operand to a
literal integer, so it reads `json_encode($v, 4194304)` and
`json_encode($v, JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR)` but stops at
one level of indirection. A constant whose *value* is itself a constant
expression resolves to plain `int`, and every form built on one is
missed:

```php
class Foo {
    const FLAGS = JSON_THROW_ON_ERROR;              // resolves to `int`
    const COMBO = JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR;
}
json_encode($v, self::FLAGS);                       // reported string|false
json_encode($v, $options | self::FLAGS);            // reported string|false
$local = JSON_PRETTY_PRINT | JSON_THROW_ON_ERROR;
json_encode($v, $local);                            // reported string|false
```

The same gap is why a `match` on a constant alias, or a comparison
against one, cannot be decided either — the flags case is just where it
was measured (3 sites).

**Fix:** fold a constant initialiser that names other constants (and
`|`/`&`/`<<` chains of them) to its literal value during constant
resolution, with a visited set so a cyclic initialiser terminates, and
carry the same folding onto a variable assigned such a chain. The bit
test in `type_engine::types::flag_returns` then needs no change: it
already asks the shared resolver for a literal and would start getting
one.

The other half of this entry — the replace family reading its `$subject`
through the shared pipeline — is fixed; `str_replace(self::NS, '',
$class)` and `preg_replace($p, $r, file_get_contents($f) ?: '')` both
resolve to `string` now.

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

## Narrowing

### B174. `assert($this instanceof X)` does not narrow `$this`

**Impact: Medium-High · Effort: Low-Medium**

A Pest test closure is bound to whatever `pest()->extends(…)` names,
which no expression in the test file says, so the suites write
`assert($this instanceof TestCase);` as the first line of the closure
to tell an analyser what it is bound to. The assertion is ignored for
`$this` (it narrows an ordinary variable), so every helper the base
class provides is reported `unknown_member` against Pest's default
`PHPUnit\Framework\TestCase` — ~21 sites in one project's browser
suite alone. Narrowing `$this` the way any other subject is narrowed
clears them; reading `pest()->extends(…)->in(…)` to bind the closure
in the first place would clear them without the assertion.

### B175. A call's recorded check survives a statement that could change what it returns

**Impact: Medium · Effort: Medium**

```php
if ($stmt->fetch('id') !== false) {
    $stmt->execute();
    $row = $stmt->fetch('id');   // still reported non-false
}
```

A call key is dropped when a variable it *reads* is written, but not
when an intervening statement could change the state behind the
receiver. Any call on the same receiver, a by-reference write the
walker does not model as an assignment, or a global mutation leaves the
recorded check standing. PHPStan invalidates every remembered
expression rooted at a receiver when an impure call is made on it.

**Fix:** invalidate the keys rooted at a receiver when a call is made on
that receiver, unless the callee is declared `@phpstan-pure` /
`@psalm-pure`. This predates expression keying (the argument-less
`$obj->get()` form has always had it), but keying calls with arguments
widens the surface.

### B176. `iterable` is not a type guard

**Impact: Low-Medium · Effort: Low-Medium**

`is_iterable($x)` narrows nothing, and neither does a
`@phpstan-assert iterable $actual` tag (PHPUnit's `assertIsIterable`).
`iterable` names no class, so the instanceof channel cannot carry it,
and the type-guard channel has no kind for it the way it does for
`array`, `string`, `callable` and the rest. The assertion is read and
then silently dropped.

**Fix:** add an `iterable` guard kind alongside the others, mapping to
`array|Traversable` for the inclusion test so a union narrows to the
members that can be iterated.

### B177. A doubly negated truthiness guard does not narrow

**Impact: Low · Effort: Low-Medium**

`if (!(!$user))` and Blade's `@unless (!$user)`, which compiles to it,
leave `$user` as `User|null` inside the guard, while the single-negation
and comparison spellings (`if ($user)`, `if (!($user === null))`,
`@unless ($user === null)`) all narrow. The negation is applied to the
operand rather than folded, so the inner `!` re-negates a proof the
outer one had already inverted. Found while confirming the Blade
`@include` narrowing, not from a sample-project site.

## Laravel

No outstanding items.

## Miscellaneous

### B173. Classes shipped inside a dependency's phar are invisible

**Impact: Low-Medium · Effort: Medium**

A project extending PHPStan (`vendor/phpstan/phpstan` ships
`phpstan.phar` + stub headers) gets `mixed` for every
`PHPStan\Type\*` value, cascading into downstream false positives
(1 site — a custom extension calling `getConstantStrings()`). Check
what the package's `bootstrap.php` exposes and whether indexing the
phar (or its extracted stubs) is feasible.
