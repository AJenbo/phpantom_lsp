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

### B125. A `while` body that reassigns the checked variable loses the condition's narrowing

**Impact: Medium · Effort: Medium**

```php
/** @return string|false */
function readLine() { return false; }

$line = readLine();
while ($line !== false) {
    useString($line);   // reported: got string|false
    $line = readLine(); // …because of this line, below the read
}
```

`while ($line !== false)` narrows its body correctly as long as the body
does not write to `$line`. Advancing the cursor inside the loop — which is
what every `fgets()`/`fgetcsv()`/`readLine()` read loop does — puts the
un-narrowed assignment type back at the *top* of the body, so a read
written *above* the reassignment is judged against the widened type. The
same happens with `null` (`while ($line !== null) { …; $line = next(); }`),
so this is the loop re-walk merging the assignment into the entry scope
rather than anything specific to `false`: the narrowing the condition
established is dropped for the whole body instead of holding until the
statement that invalidates it.

An `if` body behaves correctly here, so only the loop path is affected.

**Fix:** re-apply the loop condition's narrowing at the top of each
re-walk of the body, so the entry scope of every iteration is the merged
type *narrowed by the condition* rather than the merged type alone.

### B126. A scalar check on a property narrows nothing

**Impact: Medium · Effort: Medium**

```php
class Holder {
    public string|false $value = false;

    public function run(): void {
        if ($this->value !== false) {
            useString($this->value); // reported: got string|false
        }
        if ($this->value) {
            useString($this->value); // reported: got string|false
        }
    }
}
```

A property subject only ever gets *class-level* narrowing. `instanceof`
on `$this->prop` works, because `Expression::Access` falls through to
`narrowed_by_rewalk` → `apply_property_narrowing`, which narrows a
`Vec<ClassInfo>`. A check that removes a scalar member instead
(`!== false`, `!== null`, a bare truthy `if`) has no class to swap, and
the forward walker's scope entry for the key never reaches the
diagnostic: `narrowed_subject_from_scope` is consulted for the key first,
so the narrowing either is not recorded in the snapshot cache under the
synthetic key or is discarded when the branch scope merges. The same
check on a local variable narrows correctly, so it is the property key,
not the check, that is unsupported.

This is easy to mistake for working, because the nullable form is hidden
by a deliberately permissive rule: a `?T` argument passed to a `T`
parameter is accepted without narrowing at all
(`diagnostics/type_errors/compatibility.rs`, the "Nullable arg →
non-nullable param: MAYBE" case), on the grounds that the caller may have
guarded. A `T|false` union has no such escape hatch, so that is where the
missing narrowing surfaces.

**Fix:** find out which of the two halves drops the narrowing — the
snapshot recording for a synthetic member key, or the branch-scope merge
— and record it so that `narrowed_subject_from_scope` answers for a
property key the way it already does for an array-access key.

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

### B124. A ternary's truthy condition does not narrow its own then branch

**Impact: Medium · Effort: Low-Medium**

```php
/** @return string|false */
function content() { return ''; }

$x = content();
useString($x ? $x : ''); // reported: got string|false
```

The then branch of a full ternary is resolved with the cursor placed
inside it (`resolve_conditional_chain`,
`type_engine/variable/rhs_resolution/mod.rs`) precisely so the
forward walker's condition narrowing applies there, and an `instanceof`
condition does narrow that way. A *truthiness* condition on a plain
variable does not: `if ($x) { … }` strips `null`/`false` from `$x`
inside the block, but the equivalent ternary leaves the then branch
reading the un-narrowed type, so the idiomatic `$x ? $x : $default`
keeps the falsy members the ternary exists to replace. The short form
(`$x ?: $default`) resolves correctly, since it narrows the reused
condition directly rather than relying on the walker.

**Fix:** record the truthy-branch narrowing for a ternary condition the
same way `apply_null_narrowing_truthy` records it for an `if`/`while`
body, so the then branch's offset resolves against the narrowed scope
instead of the declared one.

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
