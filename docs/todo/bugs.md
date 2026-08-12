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

### B133. A `for` loop's increment clause never updates a variable's type

**Impact: Medium · Effort: Medium**

```php
class Node {
    public function __construct(public ?Node $next) {}
}
function useNode(Node $n): void {}

for ($node = $head; $node !== null; $node = $node->next) {
    useNode($node);   // $node's type never reflects the reassignment
}
```

`process_for` (`type_engine/variable/forward_walk/control_flow.rs`) never
runs `for_stmt.increments` through `process_assignment_expr` (or any other
assignment handling). The increment clause's own span is only ever used
for `record_scope_snapshot` (diagnostic hover/go-to-definition lookups on
the `for` line itself); its effect on the variable's type is never fed
back into the loop's fixed-point re-walk the way `for_stmt.initializations`
is. So a hand-walked-iterator pattern where the increment reassigns a
variable to a differently-typed value (e.g. `$node = $node->next()`)
never has that reassignment reflected in the body or in the post-loop
scope.

**Fix:** process `for_stmt.increments` with `process_assignment_expr`
after each body walk (mirroring how `process_while`'s condition
reassignment runs on each re-entry), and include the same processing in
the fixed-point re-entry closure passed to `walk_loop_body_to_fixed_point`.

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
