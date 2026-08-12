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

### B136. A `false` check narrows nothing in its else branch

**Impact: Medium · Effort: Low-Medium**

```php
/** @return string|false */
function readIt() { return false; }

$value = readIt();
if ($value === false) {
    // ...
} else {
    useString($value);              // reported: got string|false
}

if (!empty($value)) {
    useString($value);              // reported: got string|false
}

if ($value === false || rand(0, 1)) {
    return;
}
useString($value);                  // reported: got string|false
```

The guard-clause form (`if ($value === false) { return; }`) narrows
correctly, so the extraction and the stripping both work; what is missing
is the inverse direction. `apply_null_narrowing_inverse` handles `null` in
every shape (`=== null`, `!== null`, `isset`, `!isset`) but has no `false`
counterpart, so an explicit `else`, and the implicit else that an
`||` guard's De Morgan expansion produces, both leave `false` in place.
`!empty($x)` misses for a different reason: `extract_not_empty_var` feeds
`strip_null_from_scope`, which removes `null` only, even though `empty()`
is false exactly when the value is truthy.

This affects locals and property paths alike, so it is not the synthetic
member key that is at fault.

**Fix:** give `apply_null_narrowing_inverse` the `false` cases that
`apply_guard_clause_null_narrowing` already has (`extract_false_equality_check_var`
→ strip `false`, `extract_non_false_check_var` → narrow to `false`), and
route `!empty($x)` through `strip_falsy_from_scope` rather than
`strip_null_from_scope`.

### B137. A scalar check on an argument-less method call narrows nothing

**Impact: Low-Medium · Effort: Low-Medium**

```php
class Holder {
    public function value(): string|false { return false; }

    public function run(): void {
        if ($this->value() !== false) {
            useString($this->value()); // reported: got string|false
        }
    }
}
```

An argument-less call is a narrowing subject (`expr_to_subject_key` keys it
under `$this->value()`, and `narrowed_call` reads that key back), and an
`instanceof` check on one narrows correctly. A scalar check does not,
because the key is never seeded: `resolve_member_key_type`
(`type_engine/variable/forward_walk/cond_narrowing.rs`) skips a call whose
return type resolves to no class, on the grounds that a template parameter
or generic alias is answered better by the call resolver at the use site.
A concrete scalar union like `string|false` is caught by that rule too, so
there is nothing in scope for the check to strip `false` from.

**Fix:** seed a call key whose declared return type is built entirely from
keyword types. Those cannot be template parameters, so the call resolver
has nothing better to say about them, and seeding lets the same scalar
narrowing that already works for a property key apply.
