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

### B134. A constant operand is only read where a `@template` is bound

**Impact: Low · Effort: Medium**

```php
const ID_TABLE = ['immutable' => 1, 'mutable' => 'two'];

/** @param key-of<ID_TABLE> $key */
function acceptsKey(string $key): void {}

/** @return value-of<ID_TABLE> */
function anyValue() { return 1; }

acceptsKey('nope');            // not reported: should be 'immutable'|'mutable'
takesInt(anyValue());          // not reported: should be int|string
```

A constant holding an array literal is now read as its own array shape, so
`key-of<ID_TABLE>` and `ID_TABLE[K]` evaluate — but only along the path that
builds a template substitution map for a call, which runs solely when the
function or method declares `@template` params. A `key-of<CONSTANT>`
parameter or a `value-of<CONSTANT>` return on a plain function is still left
unevaluated and widens to its bound, so neither constrains anything.

**Fix:** run the constant-operand expansion (`constant_operand_shape` in
`type_engine/call_resolution/`) wherever a declared parameter or return type
containing an unevaluated operator is read, not only from
`finish_template_subs`. The awkward part is that the expansion needs a
`ResolutionCtx` while the sites that read those types (the argument
compatibility check, the untemplated return path) have varying access to
one, so the shared entry point has to come first.

### B135. A conditional return type is not resolved from an argument's default

**Impact: Low · Effort: Medium**

```php
function test(string $s): int {
    return str_word_count($s);          // reported: got array<string>|int
}
```

`str_word_count()`'s return type depends on its `$format` argument: `0`
(the default) returns `int`, `1` and `2` return `array<string>`. Neither
the passed value nor the declared default narrows it, so every call reads
back the full union and any use in a typed position is reported. Passing
`1` explicitly is equally unresolved, so `return str_word_count($s, 1);`
from an `array` return type is reported too.

This is what `examples/laravel/app/View/Components/PostSummary.php:37`
trips over, which is why the Laravel example reports four errors where
`docs/CONTRIBUTING.md` documents three.

**Fix:** resolve a conditional return type against the call's arguments,
falling back to a parameter's declared default when the argument is
omitted, rather than joining every branch.
