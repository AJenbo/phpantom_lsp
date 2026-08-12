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

### B88. `!== false` does not narrow inside a plain `if`/`while` truthy branch

**Impact: Medium · Effort: Low**

```php
/** @param non-empty-string|false $value */
function inspect($value): void {
    if ($value !== false) {
        takesNonEmptyString($value); // reported: got non-empty-string|false
    }
}
```

`apply_null_narrowing_truthy` (`type_engine/variable/forward_walk/cond_narrowing.rs:1540`)
handles `!== null`/`isset`/`!empty` for the truthy branch of an `if`/`while`
condition, but has no equivalent case for `!== false`. `strip_false_from_scope`
and `extract_false_equality_check_var` already exist (lines 1936 and 1783) but
are wired only into the guard-clause *inverse* path
(`apply_guard_clause_null_narrowing`, line 2057) for `=== false { throw; }`,
never into the truthy in-block branch. So the common `T|false` idiom guarded
with `!== false` (rather than `!$x`/`empty($x)`) never narrows inside the
`if` body.

**Fix:** add an `extract_non_false_check_var` mirroring
`extract_non_null_check_var`, and call it with `strip_false_from_scope` from
`apply_null_narrowing_truthy` the same way the null case is handled.

### B89. `assert($x !== null)` / `assert($x !== false)` does not narrow

**Impact: Medium-High · Effort: Medium**

```php
$handle = fopen('php://memory', 'r');
assert($handle !== false);
fclose($handle); // reported: fclose() expects resource, got resource|false
```

`process_assert_narrowing` (`type_engine/variable/forward_walk/assignment.rs:2649`)
only recognizes `assert($x instanceof Foo)`, `@phpstan-assert`/`@psalm-assert`
docblock-declared assertions, and built-in type-guard calls
(`is_string()`/`assertIsString()` and friends) — a bare equality-comparison
argument (`$x !== null`, `$x !== false`, `$x === null`, …) is never routed
through the shared condition-narrowing pipeline the way an `if`/`while`
condition is. This affects both `null` and `false`, independently of B88 —
even after B88 is fixed, `assert($x !== false)` stays broken because
`assert()`'s own narrowing never calls `apply_condition_narrowing` at all for
this form.

This is a common defensive idiom (`fopen`/`pg_connect`/`finfo_open`-style
`T|false` returns guarded with `assert($handle !== false)` before use) — every
one of `phpy`, Qodana, and Intelephense passes this case in
`php-typing-conformance`'s corpus.

**Fix:** in `process_assert_narrowing`, run the assert's argument expression
through `apply_condition_narrowing` (the same pipeline `if`/`while` conditions
use) rather than re-implementing a narrower parallel set of cases. That also
picks up B88's fix for `assert($x !== false)` for free once B88 lands.

### B90. An object union is not narrowed by a discriminating property check

**Impact: Medium · Effort: Medium-High**

```php
final class StrBox { public string $v = ''; }
final class IntBox { public int $v = 0; }

function pick(StrBox|IntBox $b): StrBox {
    if (is_string($b->v)) {
        return $b; // reported: StrBox|IntBox is incompatible with StrBox
    }
    throw new \LogicException('not a StrBox');
}
```

PHPantom already narrows an **array-shape** union by a literal-equality check
on one shape key (confirmed: `if ($a['tag'] === true) { return $a['name']; }`
on an `array{tag: true, name: string}|array{tag: false, code: int}` union
resolves cleanly) — but has no equivalent for an **object property**
discriminant. `is_string($b->v)` should narrow `StrBox|IntBox` to `StrBox`
the same way the array-shape case narrows its union, since only `StrBox::$v`
is typed `string`.

Both `phpy` and Qodana pass this case in `php-typing-conformance`'s corpus
(Intelephense does not); Mago and Phan also narrow it correctly per that
corpus's own notes, so this is also a correctness gap against our named
rival, not just a feature gap.

**Fix:** locate the array-shape-union-by-key-equality narrowing in
`cond_narrowing.rs` and add the property-access equivalent: when a type guard
or equality check on `$subject->prop` is true only for one member of an
object union, narrow the union to that member.

### B91. A native type hint silently accepts a PHPDoc-only pseudo-type

**Impact: Medium · Effort: Low-Medium**

```php
function takesResource(resource $value): void {} // not flagged; PHP has no native `resource` type
function takesReal(NotAClass $value): void {}     // correctly flagged: unknown_class
```

`is_scalar_name` (`php_type/keywords.rs:229`) folds `resource` (and
`class-string`, `interface-string`, `trait-string`, `number`, …) into "not a
class reference" unconditionally. That classifier's own doc comment says
outright that "PHP has no `resource` type-hint at all" — the pseudo-type is
valid only in a docblock, never in a native declaration — but nothing at the
native-hint call site checks *where* the identifier was written before
consulting it. So `resource` used as a real parameter/return type hint is
silently accepted instead of being flagged the same way an unrecognized class
name is.

Both Qodana and Intelephense flag this case in `php-typing-conformance`'s
corpus.

**Fix:** at the point where a native (not docblock) type hint is validated,
reject identifiers that `is_scalar_name`/`is_keyword_type` only recognize as
PHPDoc-only pseudo-types, the same way an unresolvable class name is rejected,
rather than treating "known to the type vocabulary" as "valid here."

### B92. An array literal's element types widen to base scalar types on a dynamic-key read

**Impact: Low-Medium · Effort: High**

```php
function takesInt(int $x): void {}
$values = [1, 1.5, '123'];
$key = array_rand($values);
takesInt($values[$key]); // reported: got int|float|string
```

Indexing a literal array with a non-literal key resolves the value to the
union of the literal array's *base* member types (`int|float|string`)
instead of the union of the *specific literal values* it was written with
(`1|1.5|'123'`). The distinction matters for a return type like `@return
numeric`: each literal individually satisfies `numeric` (`1`, `1.5`, and
`'123'` are all trivially numeric), but the widened `string` member cannot be
proven numeric on its own, so a value that is provably `numeric` at every
possible branch is reported as a mismatch.

Only Qodana passes this case in `php-typing-conformance`'s corpus (`phpy` and
Intelephense do not), so the signal is weaker than the other entries here.

**Fix:** preserve literal member types through a dynamic-key array access the
same way a literal-key access already does, rather than widening to base
scalar types as soon as the key stops being a compile-time constant.

### B93. `CONSTANT[T]` does not resolve per templated key

**Impact: Low-Medium · Effort: High**

```php
const ID_TABLE = ['immutable' => 1, 'mutable' => 'two'];

/**
 * @template T of key-of<ID_TABLE>
 * @param T $type
 * @return ID_TABLE[T]
 */
function lookUp(string $type = 'immutable'): int|string {
    return ID_TABLE[$type];
}

takesInt(lookUp('immutable')); // reported: got int|string, wanted int
```

`ID_TABLE[T]` in a `@return` tag should resolve, per call site, to the type
of the *specific* key the template argument is bound to — `lookUp('immutable')`
should read as `int`, `lookUp('mutable')` as `string` — not to the union of
every value the constant array holds. PHPantom currently returns the whole
table's value union regardless of which key `T` is bound to at the call
site, so calls that should pass a narrower parameter type are rejected.

Both `phpy` and Qodana pass this case in `php-typing-conformance`'s corpus
(Intelephense does not).

**Fix:** when substituting a template parameter bound to `key-of<CONSTANT>`
at a call site, resolve `CONSTANT[T]` by indexing the constant's array type
at that specific literal key rather than falling back to the value union.

### B94. `@psalm-this-out` / `@phpstan-self-out` method-level template mutation is not modelled

**Impact: Low · Effort: High**

```php
/** @template T */
final class MutableBox {
    public function __construct(public mixed $value) {}

    /**
     * @template U
     * @param U $value
     * @psalm-this-out self<U>
     * @phpstan-self-out self<U>
     */
    public function replace(mixed $value): void {}
}

/** @param MutableBox<int> $box */
function f(MutableBox $box): void {
    $box->replace('x');
    takesIntBox($box);    // still reported clean — should now fail
    takesStringBox($box); // reported: MutableBox<int> is not MutableBox<string> — should now pass
}
```

`@psalm-this-out self<U>` / `@phpstan-self-out self<U>` on a method says the
call mutates `$this`'s template parameter to `U`, so after
`$box->replace('x')`, `$box` should read as `MutableBox<string>` rather than
`MutableBox<int>` for the rest of the block. PHPantom does not track this
annotation at all, so `$box`'s type never changes after the call.

None of `phpy`, Qodana, or Intelephense pass this case in
`php-typing-conformance`'s corpus either — Mago intentionally dropped
`@this-out` support as unsound, and NoVerify/Phan don't model it — so this is
filed for completeness rather than because any tool we compare against
demonstrates users expect it.

**Fix:** not investigated beyond confirming the annotation has no effect;
would need a mechanism to re-bind a receiver's template arguments after a
method call the way an assignment re-binds a variable's type.

### B120. The short ternary (`?:`) keeps the condition's falsy branch in its own result

**Impact: Medium-High · Effort: Low**

```php
$body = $response->getContent() ?: '';
assertStringContainsString('ok', $body); // reported: got string|false
```

`resolve_conditional_chain` (`type_engine/variable/rhs_resolution/mod.rs:580`)
handles a short ternary by reusing the condition expression as the
"then" branch (`current.then.unwrap_or(current.condition)`), then adds
that branch's *full* resolved type to the union whenever
`static_condition_truthiness` cannot prove the condition false — which
is always the case for a non-literal condition like a method call. So
`$x ?: $default` keeps every falsy member of `$x`'s type (`false`,
`null`, …) in the combined result, even though reaching the "then"
value at runtime requires the condition to have been truthy.

**Fix:** when `then_expr` is the reused condition (the short-ternary
case), narrow its resolved type to the truthy subset — the same
narrowing an `if ($x)` truthy branch already applies — before adding it
to `combined`, instead of using the condition's raw resolved type.

### B121. `assert($array[$key] instanceof Foo)` does not narrow the array-access subject

**Impact: Medium · Effort: Low**

```php
assert($items[0] instanceof Foo);
takesFoo($items[0]); // reported: got Foo|Bar (or whatever $items[0] was before)
```

The `assert($x instanceof Foo)` narrowing added for B86-style call
subjects (`process_assert_narrowing`,
`type_engine/variable/forward_walk/assignment.rs:2670`) only matches
`Expression::Variable(Variable::Direct(_))` on the left-hand side of the
`instanceof` check — an `Expression::ArrayAccess` subject
(`$array[$key]`, `$array[$literalIndex]`) falls through and is never
recorded, so a repeated read of the same array slot after the assert
still carries its pre-assert type. `snapshot_narrowing.rs` already
handles `Expression::ArrayAccess` as a narrowing key for `if`/`while`
conditions, so the capability exists elsewhere in the narrowing code —
`assert()`'s own handling just never reuses it for this subject shape.

**Fix:** extend the LHS match in `process_assert_narrowing` to accept an
`Expression::ArrayAccess` subject (keyed the same way
`snapshot_narrowing.rs` keys one), not only a direct variable.

### B122. Repeated conditional writes to the same array key accumulate redundant union snapshots

**Impact: Medium · Effort: Medium**

```php
$rows = [];
foreach ($items as $item) {
    if ($item->kind === 'a') {
        $rows[$item->id] = ['a' => $item->a];
    }
    if ($item->kind === 'b') {
        $rows[$item->id] = ['b' => $item->b];
    }
    if ($item->kind === 'c') {
        $rows[$item->id] = ['c' => $item->c];
    }
}
return $rows;
// declared: array<int, array{a: mixed}|array{b: mixed}|array{c: mixed}>
// inferred: array|array<int|string, array{a: mixed}>
//           |array<int|string, array{a: mixed}|array{b: mixed}>
//           |array<int|string, array{a: mixed}|array{b: mixed}|array{c: mixed}>
```

Three mutually-exclusive `if` branches writing three different shapes to
the same dynamic key, inside one loop, should merge into a single
`array<int, ShapeA|ShapeB|ShapeC>`. Instead each subsequent keyed write
appears to snapshot the *cumulative* union built so far and add it as a
new, separate branch alongside the earlier snapshots, so the final type
is a union of increasingly-nested partial unions rather than one flat
merge — and the whole thing is unioned with a bare `array` on top. This
inflates real return-type signatures into unreadable, self-overlapping
unions and produces `type_mismatch_return` false positives against the
function's own honestly-narrower declared return type.

**Fix:** in `process_array_key_assignment`
(`type_engine/variable/forward_walk/assignment.rs`), when a key already
has a recorded value type from an earlier branch in the same merge
scope, union the new branch's shape into the existing per-key value type
in place rather than recording a new whole-array snapshot each time.

### B123. A docblock cannot refine one member of an all-scalar native return union

**Impact: Medium · Effort: Low**

```php
/**
 * @return false|string
 */
function parseImage(array $fragments): bool|string { /* … */ }

$image = parseImage($fragments);
if ($image !== false) {
    useString($image); // reported: got bool|string
}
```

`should_override_type_typed` (`docblock/tags.rs:1230`) decides whether a
docblock type may override a native one. For a native **union**, it
only allows the override when at least one member is non-scalar or a
broad container (`array`/`iterable`/`callable`/`object`) — see the
`TypeKind::Union` branch at `docblock/tags.rs:1287`. It never applies
the per-member "compatible refinement" check
(`is_compatible_refinement_typed`) that a *single* narrow scalar native
type already gets a few lines down. So a native `bool|string` return
type can never be refined by a more precise docblock `false|string`,
even though the same refinement (`bool` → `false`) is accepted when
`bool` is the *only* native member. Losing the refinement means
`!== false` narrowing has nothing to narrow away — `bool` has no
literal-`false` member to remove — so downstream code guarded with the
idiomatic `!== false` check still sees the full `bool|string`.

**Fix:** in the `TypeKind::Union`/`TypeKind::Intersection` branch of
`should_override_type_typed`, check each native member with
`is_compatible_refinement_typed` against the corresponding docblock
member (when one exists) instead of the blanket `!m.is_scalar() || …`
test, so a docblock can refine a scalar union member the same way it
already refines a lone scalar.
