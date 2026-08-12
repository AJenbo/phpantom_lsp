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

### B128. A variable assigned in a loop condition is not narrowed by that condition

**Impact: Medium · Effort: Low-Medium**

```php
/** @return string|false */
function readLine() { return false; }

while (($line = readLine()) !== false) {
    useString($line);   // reported: got string|false
}

while ($line = readLine()) {
    useString($line);   // reported: got string|false
}
```

The `while (($line = fgets($handle)) !== false)` idiom assigns and checks
in one expression, and the check narrows nothing: the body sees the full
`string|false` the assignment produced. Both the explicit comparison and
the bare truthy form are affected, and so is the `null` sentinel
(`while (($line = readLine()) !== null)`).

Two things stand in the way, both in `process_while`
(`type_engine/variable/forward_walk/control_flow.rs`):

- `apply_condition_narrowing` runs *before*
  `process_condition_assignment`, so when the narrowing looks the
  variable up it is not in scope yet and
  `strip_false_from_scope`/`strip_null_from_scope` return early on an
  empty type list.
- `extract_non_false_check_var` (and its `null` counterpart) reads the
  comparison's operands with `expr_to_var_name`, which only matches a
  bare `Expression::Variable`. The operand here is a parenthesized
  assignment, so no subject is extracted at all.

**Fix:** seed the condition's assignment before narrowing the condition,
and peel a parenthesized assignment down to its target when extracting
the narrowing subject, so the assigned variable is the subject the check
narrows. `if (($line = readLine()) !== false)` has the same shape and
should be covered by the same change.

### B130. Echoing a translation in a template is reported as printing an array

**Impact: High · Effort: Low-Medium**

```blade
{{ __('messages.welcome') }}   {{-- reported: e() got array|string --}}
{{ trans('messages.welcome') }}
```

`{{ __('messages.welcome') }}` is the single most common line in a
localised Blade template, and every one of them is reported: Blade
compiles an echo to `e($value)`, `e()` takes a
`Htmlable|BackedEnum|string|int|float|null`, and `__()` is declared
`array|string` because a translation key may name a whole group of
strings. So the array branch, which a scalar key can never return, is
reported against the argument.

This is the Laravel case of T38 (a return type that depends on an
argument's *value* rather than its type): the key is a literal, and
PHPantom already resolves translation keys to their entries, so which
branch applies is known at the call site. The volume makes it worth
handling separately from the core builtins T38 lists: 15 of the 20
diagnostics `analyze` reports on `examples/laravel` are this one shape,
which puts the project's documented CI gate (3 deliberate diagnostics,
see `docs/CONTRIBUTING.md`) 17 over.

**Fix:** resolve the translation key at the call site and return `string`
when it names a scalar entry, keeping `array|string` only for a key that
names a group or that cannot be resolved. `Lang::get()`, `trans()`, and
`trans_choice()` share the signature and should share the treatment.

The remaining two diagnostics of that 20 are a separate question:
`examples/laravel/resources/views/welcome.blade.php` passes
`$posts->first()` (a genuinely nullable `BlogPost|null`) to a component
whose constructor takes a non-nullable `BlogPost`. Either the check is
too strict for a component attribute or the example should pass
something non-nullable; the maintainer's call which.

### B129. Arithmetic on a refined int widens to `int|float`

**Impact: Medium · Effort: Low**

```php
function total(string $text): int {
    $length = 0;
    $length += strlen($text);   // strlen() is declared `@return int<0,max>`

    return $length;             // reported: int|float is incompatible with int
}
```

`int + int` is `int`, and PHPantom gets that right for a plain `int`. It
does not for any of the *refinements* of `int`: `int<0,max>`,
`positive-int`, `non-negative-int`, and the rest classify as "not a
number I recognise", which falls through to the conservative `int|float`
result. `strlen()`, `count()`, `strpos()`, and most of the standard
library's counting functions are declared with a range, so this fires on
ordinary accumulator code and is reported at the `return`, several lines
away from the addition that caused it.

`classify_php_type`
(`type_engine/variable/forward_walk/assignment.rs`) enumerates the
int-like spellings by name (`int`, `integer`, `bool`, …) and has no arm
for `TypeKind::IntRange` or for the refined `int` names, so it returns
`None` and `infer_arithmetic_result_type` takes its unknown-operand
branch.

**Fix:** classify every int subtype as int-like. `PhpType::is_int_subtype`
already knows the full set (including `IntRange`), so the name matching
in `classify_php_type` can defer to it, with the same treatment for
`is_float_subtype` on the float side.

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

### B127. An inline `@method` template parameter is read as a class name

**Impact: Low · Effort: Low-Medium**

```php
/**
 * @method TVal get<TVal of mixed>(TVal $default)
 */
class Box {}
// reported twice: Class 'Demo\TVal' not found
```

A `@method` tag may declare its own template parameters inline, between
the method name and its parameter list, and PHPantom does not read that
declaration: `TVal` is not registered as a template parameter for the
tag, so both occurrences of it (the return type and the parameter type)
resolve as class references and are reported as unknown classes. This is
visible in `examples/demo.php`, which carries the two diagnostics.

**Fix:** parse the `<TVal of mixed>` list in a `@method` tag as the
tag's own `@template` parameters, and register them so the symbol map's
template-definition lookup finds them for the tag's own span.

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
