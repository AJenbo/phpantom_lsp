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

## Crashes

### B150. A `\xNN` escape in a string literal aborts the process

**Impact: High · Effort: Low**

Found while diffing analyser output over `laravel/framework` on
2026-08-14, not in the sample-project sweep. Seven lines reproduce it:

```php
<?php
function probe(string $output): void
{
    if (($pos = strpos($output, "\x8b")) !== false) {
        echo $pos;
    }
}
```

```
$ phpantom_lsp analyze r.php
thread 'diag-worker' panicked at library/core/src/str/validations.rs:48:36:
unsafe precondition(s) violated: hint::unreachable_unchecked must never be reached
thread caused non-unwinding panic. aborting.
```

`bytes_to_str` (`src/atom.rs`) is `str::from_utf8_unchecked`, on the
stated ground that "PHP source is always valid UTF-8 (mago guarantees
this after lexing)". That holds for identifiers, not for a string
literal's *decoded* value: mago resolves `\x8b` to the single byte
`0x8b`, and `LiteralString::value` then holds bytes no UTF-8 string
can. `argument_value_key`
(`type_engine/types/narrowing/resolve.rs`) builds a `&str` from that
value and iterates it with `.chars()`, which is where the
undefined-behaviour check fires. The panic is non-unwinding, so
`catch_unwind` cannot contain it and the whole process goes down: on
the CLI the run dies mid-analysis, and in the editor the server exits.

Two things make it worse than the repro suggests. The check that fires
here is a debug-build assertion, so a release build performs the
undefined behaviour silently instead of aborting. And the same
`bytes_to_str`-on-a-literal-value pattern is spread across roughly
three dozen call sites (`regex_shape::literal_string`,
`extract_array_key_for_shape`, the docblock and Blade readers, …), any
of which a `\xNN` escape can reach.

A raw non-UTF-8 byte written directly into the source file did not
reproduce it in the same position, so the decode path is the one that
matters.

**Fix:** make the conversion of a *literal value* checked, since that
is the one place the input is arbitrary bytes rather than lexed PHP
identifier text: `str::from_utf8(...)` with a lossy or skip-this-value
fallback. Keep `bytes_to_str` unchecked for the identifier and raw-token
paths where mago's guarantee does hold, and correct its safety comment
to say which inputs it covers.

## Type comparison

### B151. `?T` and `T|null` are judged by different rules

**Impact: High · Effort: Low-Medium**

Found on 2026-08-14 while checking why an optional array-shape key read
was not reported, not in the sample-project sweep. The same type in its
two spellings gets two different answers from every consumer:

```php
function f(?string $x): string {
    trim($x);            // silent
    return $x;           // silent
}

/** @param array{a: string|null} $row */
function g(array $row): void {
    trim($row['a']);     // reported: null does not satisfy string
}
```

Assigning a `?string` into a `string` property and passing one to a
user-defined `string` parameter are silent too, so this is not specific
to the argument check: `type_mismatch_argument`, `type_mismatch_return`
and the property-assignment check all report the union spelling and none
of them report the `?T` one.

The cause is the "Nullable arg → non-nullable param: MAYBE" escape hatch
in `is_type_compatible` (`diagnostics/type_errors/compatibility.rs`),
which returns compatible whenever the argument is a `TypeKind::Nullable`
whose inner type fits, on the grounds that the null may have been guarded
somewhere the walker could not follow. The hatch matches on the
*spelling* rather than on the type, so `TypeKind::Union([T, null])` walks
straight past it. Which spelling a value ends up with is an accident of
how it was produced: a declared `?string` and a nullable docblock stay
`Nullable`, while a union built by a branch merge, a `??` chain, or a
resolver join comes out as `T|null`.

This is what makes a read of an optional shape key
(`array{a?: string}`, and the `array{0?: string, 1?: string}` a
`preg_match` guard leaves behind where its branches rejoin) resolve
correctly as `?string` and still go unreported when it is passed to a
non-nullable parameter.

**Fix:** decide the policy once and apply it to both spellings. Reading
the null out of the argument type rather than off its shape is the
mechanical half (`accepts_null` / `non_null_type` already handle both);
the judgement is whether the hatch stays at all, and the architecture
note above it says what retiring one costs. Closing it is the larger
change of the two, since a `?T` argument is common, so it may be worth
narrowing it first (keep the hatch only where the null could plausibly
have been guarded out) rather than removing it outright.

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
