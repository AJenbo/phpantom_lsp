# PHPantom — Phpactor Test Parity

Track remaining gaps between phpactor's inference test suite
(`phpactor/lib/WorseReflection/Tests/Inference/`) and PHPantom's
fixture tests (`tests/fixtures/`). Each section groups related gaps
and references the specific phpactor `.test` files to port when the
underlying feature is implemented or verified.

When completing an item, port the phpactor test as a `.fixture` file,
verify it passes, and delete the item from this file. If a feature is
not planned, mark the item with *(won't fix)* and a reason.

---

## Already tracked elsewhere

These gaps have dedicated todo items with fixtures already created
(some ignored). No action needed here — they are listed for
completeness so we don't duplicate work.

| Gap | Todo ref | Fixture(s) |
|-----|----------|------------|
| Null coalesce type refinement | [T8](type-inference.md#t8-null-coalesce--type-refinement) | `null_coalesce/non_nullable_lhs.fixture`, `null_coalesce/nullable_lhs.fixture` |
| Dead-code after `never` return | [T9](type-inference.md#t9-dead-code-elimination-after-never-returning-calls) | `type/never_return_type.fixture` |
| Ternary RHS in list destructuring | [T10](type-inference.md#t10-ternary-expression-as-rhs-of-list-destructuring) | `assignment/list_destructuring_conditional.fixture` |
| Nested list destructuring | [T11](type-inference.md#t11-nested-list-destructuring) | `assignment/nested_list_destructuring.fixture` |

---

## 1. Type guard functions we don't test

We have good coverage for `is_string`, `is_int`, `is_null`,
`is_array`, and `instanceof`. These type guard functions have no
tests at all:

| Function | phpactor ref | What to test |
|----------|-------------|--------------|
| `is_callable()` | `function/is_callable.test` | After `if (is_callable($foo))`, variable narrows to `callable` |
| `is_float()` | `function/is_float.test` | After `if (is_float($foo))`, variable narrows to `float` |

**Effort: Low** — our narrowing pipeline already handles `is_*`
functions generically. These likely already work; we just need the
test fixtures to prove it.

**Action:** Create `narrowing/is_callable.fixture` and
`narrowing/is_float.fixture`. If they pass, done. If not, the fix
is a one-line addition to the type-guard function list.

---

## 2. `in_array()` with class constants as haystack

We test `in_array()` narrowing with literal arrays. phpactor also
tests narrowing when the haystack contains class constants:

```php
in_array($foo, [Foo::BAR, Foo::BAZ], true)
```

**phpactor ref:** `function/in_array.test` (the class-constant
variant)

**Effort: Low** — verify whether our existing `in_array` narrowing
handles this, add a fixture if so.

---

## 3. Elseif with terminating statement

We test `elseif` chains (`elseif_instanceof_chain.fixture`,
`is_type_elseif_chain`) but not the case where an `elseif` branch
terminates with `die()` or `throw`, eliminating a type from the
union after the block.

**phpactor ref:** `if-statement/elseifdie.test`

**Example:**
```php
function test(Foobar|string|int $foo) {
    if (is_string($foo)) { /* string */ }
    elseif (is_int($foo)) { die(); }
    // after: int is eliminated → Foobar|string
}
```

**Effort: Low** — create `narrowing/elseif_die.fixture`.

---

## 4. Else-branch assignment merging

When `if`/`else` branches assign different values to the same
variable, the type after the block should be the union.

**phpactor ref:** `if-statement/else_assign.test`

**Example:**
```php
if ($cond) { $foo = new A(); } else { $foo = new B(); }
// $foo is A|B
```

**Effort: Low** — create `narrowing/else_branch_assignment.fixture`.

---

## 5. Combined negated type guards

phpactor tests combined negated guards like
`false === is_string($x) && false === $x instanceof Foo` with a
throw, narrowing the variable to `string|Foo` afterward.

**phpactor ref:** `if-statement/is_not_string_and_not_instanceof.test`

**Effort: Low-Medium** — create
`narrowing/combined_negated_guards.fixture`. Likely already works
given our `&&` narrowing support.

---

## 6. Namespace-qualified instanceof in or-chains

phpactor tests `instanceof Baz || instanceof \Boo` inside a
namespace, verifying relative vs absolute name resolution.

**phpactor ref:** `if-statement/namespace.test`

**Effort: Low** — create `narrowing/namespaced_or_instanceof.fixture`.
We have `namespace_instanceof.fixture` for a single instanceof but
not the or-chain variant.

---

## 7. Open (non-terminating) branches don't leak narrowing

We have `open_branches_no_leak.fixture` but phpactor has a more
detailed test with multiple sequential open branches on `mixed`:

**phpactor ref:** `if-statement/multiple_statements_open_branches.test`

**Effort: Low** — verify our existing fixture covers this or add a
more explicit variant.

---

## 8. Array mutation tracking

phpactor tracks array type changes through push operations:

| Scenario | phpactor ref |
|----------|-------------|
| `$arr[] = 123` on empty array → `array{123}` | `assignment/array_add.test` |
| `$arr[] = $item` in foreach from `Generator<Bar>` → `Bar[]` | `assignment/array_add_in_foreach.test` |
| `$arr[] = $param` where param is `string` → `string[]` | `assignment/array_add_string.test` |
| Conditional array key addition → union of shapes | `assignment/array_2.test` |
| Unknown key assignment → `array<<missing>, T>` | `assignment/unknown_key.test` |

**Effort: Medium** — we don't track `$arr[] =` mutations today.
This is a new feature. Create ignored fixtures for each and consider
whether to add a dedicated todo item.

---

## 9. Ternary assignment producing union type

Assigning from a ternary where branches are different class types:
`$foo = $cond ? new A() : new B()` → `A|B`.

**phpactor ref:** `assignment/ternary_expression.test`

**Effort: Low** — likely already works. Create
`assignment/ternary_assignment_union.fixture`.

---

## 10. Variable-variable (`${$bar}`) resolution

phpactor tests `${$bar}` resolving to the type of the inner
variable's value.

**phpactor ref:** `variable/braced_expression.test`

**Effort: Low-Medium** — niche feature. Create an ignored fixture
if not supported.

---

## 11. Cast expression type resolution

`(string)`, `(int)`, `(float)`, `(bool)`, `(array)`, `(object)`
cast expressions should resolve to the target type.

**phpactor ref:** `cast/cast.test`

**Effort: Low** — create `type/cast_expression.fixture`.

---

## 12. Variadic parameter type inside function body

`string ...$foo` should resolve to `string[]` inside the function.

**phpactor ref:** `type/variadic.test`

**Effort: Low** — create `type/variadic_param.fixture`. We test
variadic in code actions and signatures but not the inferred type
inside the body.

---

## 13. `list<T>` type alias

`@param list<string>` should resolve to `array<int, string>`.

**phpactor ref:** `type/list.test`

**Effort: Low** — create `type/list_type.fixture`. Our docblock
parser likely already handles this.

---

## 14. `string|false` return type

Functions returning `string|false` (common in PHP stdlib) should
resolve correctly.

**phpactor ref:** `type/false.test`

**Effort: Low** — create `type/string_or_false.fixture`. Likely
already works.

---

## 15. Callable and Closure docblock types

`@param callable(Foo, int): string` and
`@param Closure(string, int): string` should be parsed and
preserved.

**phpactor ref:** `type/callable.test`, `type/closure.test`

**Effort: Low-Medium** — create `type/callable_param.fixture` and
`type/closure_param.fixture`.

---

## 16. `int<min, max>` range types

PHPStan `int<min, max>` range type annotations should be parsed.

**phpactor ref:** `type/int-range.test`

**Effort: Low** — parsing may work; resolution is less critical
since it doesn't affect completion. Create a hover fixture.

---

## 17. Parenthesized union types with narrowing

`(string|int)|int` should narrow correctly with `is_int`.
`(Closure(string,int): string)|string` should narrow to the
Closure type when `string` is excluded.

**phpactor ref:** `type/parenthesized.test`,
`type/parenthesized_closure.test`

**Effort: Low-Medium** — create
`type/parenthesized_union.fixture`.

---

## 18. Union from relative docblock names

Relative class names in docblock union types should resolve to
their FQN within the current namespace.

**phpactor ref:** `type/union_from_relative_docblock.test`

**Effort: Low** — likely already works. Create a fixture to verify.

---

## 19. Binary expression type inference

phpactor infers result types for binary expressions. This is low
priority for completion but could improve hover:

| Category | phpactor ref | Example |
|----------|-------------|---------|
| Arithmetic | `binary-expression/arithmetic.test` | `1 + 2` → `3` |
| Concatenation | `binary-expression/concat.test` | `'a' . 'b'` → `"ab"` |
| Comparison | `binary-expression/compare.scalar.test` | `1 === 1` → `true` |
| Logical | `binary-expression/logical.test` | `true && false` → `false` |
| Bitwise | `binary-expression/bitwise.test` | `1 & 2` → `0` |
| Array union | `binary-expression/array-union.test` | `$a + $b` → combined shape |
| instanceof expr | `binary-expression/type.test` | `$x instanceof Foo` → `bool` |

**Effort: High** — these are all new. Low impact on completion.
Not a priority unless hover accuracy matters.

---

## 20. Postfix increment/decrement

`$i++` on a literal `0` → `1`, `$i--` on literal `2` → `1`.

**phpactor ref:** `postfix-update/increment.test`,
`postfix-update/decrement.test`

**Effort: Low** — niche. Only relevant for literal type tracking.

---

## 21. Return statement type inference

phpactor tests return type inference from method bodies:

| Scenario | phpactor ref |
|----------|-------------|
| Single literal return | `return-statement/class_method.test` |
| Missing return type → `<missing>` | `return-statement/missing_return_type.test` |
| Multiple returns → union | `return-statement/multiple_return.test` |
| No return → `void` | `return-statement/no_return.test` |

**Effort: Medium** — body return type inference is a separate
feature from our current declared-type-based resolution.

---

## 22. `global` keyword

Variables imported with `global $var` inside functions should be
accessible.

**phpactor ref:** `global/global_keyword.test`

**Effort: Low-Medium** — niche feature. Create an ignored fixture.

---

## 23. `define()` constant resolution

`define('FOO', 'bar')` should make `FOO` resolve to `"bar"`.
Also tests namespaced constants and `use const` imports.

**phpactor ref:** `constant/constant.test`,
`constant/constant_namespaced.test`,
`constant/constant_namespaced_imported.test`

**Effort: Low** — we have `define()` support in go-to-definition.
Verify type resolution works and add hover fixtures.

---

## 24. `array_reduce()` return type inference

The return type of `array_reduce()` should be inferred from the
initial value argument.

**phpactor ref:** `function/array_reduce.test`

**Related todo:** [C1](completion.md#c1-array-functions-needing-new-code-paths)

**Effort: Medium** — requires function-specific return type logic.

---

## 25. `array_sum()` return type inference

`array_sum([10, 20, 30])` → `int`, `array_sum([1, 2.5])` → `float`.

**phpactor ref:** `function/array_sum.test`

**Related todo:** [C4](completion.md#c4-non-array-functions-with-dynamic-return-types)

**Effort: Medium** — requires function-specific return type logic.

---

## Summary by effort

### Low effort (likely already work, just need fixtures)

| # | Item | phpactor ref |
|---|------|-------------|
| 1 | `is_callable()` narrowing | `function/is_callable.test` |
| 1 | `is_float()` narrowing | `function/is_float.test` |
| 2 | `in_array()` with class constants | `function/in_array.test` |
| 3 | Elseif + die/throw | `if-statement/elseifdie.test` |
| 4 | Else-branch assignment union | `if-statement/else_assign.test` |
| 5 | Combined negated guards | `if-statement/is_not_string_and_not_instanceof.test` |
| 6 | Namespaced or-instanceof | `if-statement/namespace.test` |
| 9 | Ternary assignment union | `assignment/ternary_expression.test` |
| 11 | Cast expressions | `cast/cast.test` |
| 12 | Variadic param type | `type/variadic.test` |
| 13 | `list<T>` type alias | `type/list.test` |
| 14 | `string\|false` return | `type/false.test` |
| 18 | Relative docblock union | `type/union_from_relative_docblock.test` |

### Low-Medium effort (may need minor code changes)

| # | Item | phpactor ref |
|---|------|-------------|
| 7 | Open branches no leak (extended) | `if-statement/multiple_statements_open_branches.test` |
| 10 | Variable-variable `${$bar}` | `variable/braced_expression.test` |
| 15 | Callable/Closure docblock types | `type/callable.test`, `type/closure.test` |
| 16 | `int<min,max>` range types | `type/int-range.test` |
| 17 | Parenthesized union narrowing | `type/parenthesized.test` |
| 22 | `global` keyword | `global/global_keyword.test` |

### Medium effort (new features needed)

| # | Item | phpactor ref |
|---|------|-------------|
| 8 | Array mutation tracking | `assignment/array_add*.test` |
| 21 | Return statement type inference | `return-statement/*.test` |
| 24 | `array_reduce()` return type | `function/array_reduce.test` |
| 25 | `array_sum()` return type | `function/array_sum.test` |

### High effort / low priority

| # | Item | phpactor ref |
|---|------|-------------|
| 19 | Binary expression types | `binary-expression/*.test` |
| 20 | Postfix increment/decrement | `postfix-update/*.test` |