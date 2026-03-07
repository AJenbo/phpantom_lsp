# PHPantom — Ignored Fixture Tasks

There are **228 fixture tests** in `tests/fixtures/`. Of these, **176
pass** and **52 are ignored** because they exercise features or bug
fixes that are not yet implemented. Each ignored fixture has a
`// ignore:` comment explaining what is missing.

This document groups the 52 ignored fixtures by the underlying work
needed to un-ignore them. Tasks are ordered by the number of fixtures
they unblock (descending), then by estimated effort. Once a task is
complete, remove the `// ignore:` line from each fixture, verify the
fixture passes, and delete the task from this file.

After completing a task, run the full CI suite:

```
cargo test
cargo clippy -- -D warnings
cargo clippy --tests -- -D warnings
cargo fmt --check
php -l example.php
```

---

## 1. `@implements` generic resolution (7 fixtures)

**Ref:** [type-inference.md §17](type-inference.md#17-implements-generic-resolution)
**Impact: Medium-High · Effort: Medium**

When a class declares `@implements SomeInterface<ConcreteType>`, the
generic parameters are not substituted into inherited interface methods.
This already works for `@extends` on classes, so the substitution
infrastructure exists. The `@implements` path needs the same wiring.

**Fixtures:**

- [ ] `generics/class_implements_single.fixture` — `@implements Repo<User>` resolves `T` on method return
- [ ] `generics/class_implements_multiple.fixture` — multiple `@implements` with different concrete types
- [ ] `generics/class_template_implements.fixture` — `@template-implements` syntax (alias for `@implements`)
- [ ] `generics/implements_parameter_type.fixture` — `@implements` resolves `T` on method parameters
- [ ] `generics/iterator_aggregate_complex.fixture` — `@implements IteratorAggregate<Item>` for foreach
- [ ] `generics/nested_iterator_chain_gh1875.fixture` — `@implements Iterator<Item>` through nested chain
- [ ] `foreach/iterator_aggregate_key_value.fixture` — `@implements IteratorAggregate<K, V>` with key+value types

**Implementation notes:**

In `inheritance.rs`, `build_substitution_map` handles `@extends`
annotations. Add the same logic for `@implements` annotations on the
class. When merging interface methods in `resolve_class_fully_inner`
(virtual_members), apply the substitution map from `@implements` to
each inherited interface method's types. The `@template-implements`
syntax should be treated as an alias, same as `@template-extends` is
for `@extends`.

---

## 2. Function-level `@template` generic resolution (9 fixtures)

**Ref:** [type-inference.md §2](type-inference.md#2-function-level-template-generic-resolution)
**Impact: High · Effort: Medium**

`FunctionInfo` lacks `template_params` and `template_bindings` fields,
so standalone functions and constructors with `@template` cannot infer
generic types from call-site arguments. This blocks Laravel helpers
(`collect()`, `tap()`, `value()`) and constructor-based inference.

**Fixtures:**

- [ ] `generics/constructor_params.fixture` — `new Foo($bar)` infers `T` from constructor arg
- [ ] `generics/constructor_array_arg.fixture` — constructor with array argument infers `T`
- [ ] `generics/constructor_generic_arg.fixture` — constructor with generic-typed argument
- [ ] `generics/constructor_param_and_extend.fixture` — constructor inference combined with `@extends`
- [ ] `generics/method_returns_templated_generic.fixture` — method returning `Collection<T>` where `T` is inferred from constructor
- [ ] `generics/covariant_template.fixture` — `@template-covariant` modifier on function-level template
- [ ] `generics/class_string_generic_union.fixture` — `class-string<T>` with variadic params
- [ ] `generics/class_string_variadic_union.fixture` — `class-string<T>` with variadic union scenario
- [ ] `generics/class_string_nested_return.fixture` — `class-string<T>` with nested generic return type

**Implementation notes:**

1. Add `template_params: Vec<String>` and
   `template_bindings: Vec<(String, String)>` to `FunctionInfo` in
   `types.rs`, mirroring `MethodInfo`.
2. Populate them in `parser/functions.rs` using `extract_template_params`
   and `extract_template_param_bindings`.
3. At call sites in `variable/rhs_resolution.rs`, after loading
   `FunctionInfo`, check for `template_params`. Infer concrete types
   from arguments, build a substitution map, and apply it to the return
   type before resolving.

---

## 3. Property-level narrowing (5 fixtures)

**Ref:** [type-inference.md §21](type-inference.md#21-property-level-narrowing)
**Impact: Medium · Effort: Medium**

Only local variables participate in type narrowing today.
`$this->prop instanceof Foo` inside an `if` block does not narrow
`$this->prop` for subsequent member access. The narrowing engine needs
to track member access expressions in addition to bare variables.

**Fixtures:**

- [ ] `narrowing/property_narrowing.fixture` — `if ($this->prop instanceof Foo)` narrows
- [ ] `narrowing/property_narrowing_negated.fixture` — negated property narrowing with early return
- [ ] `combination/property_instanceof.fixture` — property instanceof in combination context
- [ ] `member_access/access_from_union.fixture` — narrowing on `$this->prop` to access members
- [ ] `function/assert_property_instanceof.fixture` — `assert($this->prop instanceof Foo)` narrows

**Implementation notes:**

Extend `NarrowedType` (or the narrowing state structure) to accept a
member access path (`$this->prop`) as a narrowing key in addition to
plain variable names. When emitting narrowing from `instanceof` checks,
detect whether the left side is a property access and store the full
path. During variable resolution, when encountering `$this->prop`,
check the narrowing state for a matching member access path.

---

## 4. `@phpstan-assert` on static method calls (3 fixtures)

**Ref:** [type-inference.md §18](type-inference.md#18-phpstan-assert-on-static-method-calls)
**Impact: Medium · Effort: Medium**

Type guards declared with `@phpstan-assert`, `@phpstan-assert-if-true`,
and `@phpstan-assert-if-false` only work on standalone function calls
today. Static method calls like `Assert::instanceOf($value, Foo::class)`
do not trigger narrowing.

**Fixtures:**

- [ ] `narrowing/phpstan_assert_static.fixture` — `Assert::isInstanceOf($x, Foo::class)` narrows `$x`
- [ ] `narrowing/phpstan_assert_if_true.fixture` — `@phpstan-assert-if-true` on static method
- [ ] `narrowing/phpstan_assert_if_false.fixture` — `@phpstan-assert-if-false` on static method

**Implementation notes:**

In the narrowing pass, when processing call expressions, check whether
the callee is a static method call (`Foo::bar()`). If so, resolve the
class and method, extract `@phpstan-assert*` tags from the method's
docblock, and apply the same narrowing logic used for standalone
functions.

---

## 5. Attribute context support (3 fixtures)

**Ref:** [signature-help.md §4](signature-help.md#4-attribute-constructor-signature-help)
**Impact: Medium · Effort: Medium**

PHP 8 attributes take constructor arguments (`#[Route('/path', methods: ['GET'])]`),
but no `CallSite` is emitted for attribute nodes. Signature help and
named parameter completion do not fire inside attribute parentheses.

**Fixtures:**

- [ ] `named_parameter/attribute_constructor.fixture` — named params in `#[Attr(name: <>)]`
- [ ] `signature_help/attribute_constructor.fixture` — sig help inside `#[Attr(<>)]`
- [ ] `signature_help/attribute_second_param.fixture` — sig help active param tracking in `#[Attr('a', <>)]`

**Implementation notes:**

In `symbol_map/extraction.rs`, add a visitor for `Attribute` AST nodes
that emits a `CallSite` pointing at the attribute class's `__construct`
method. The comma offsets and argument positions need to be extracted
the same way as for regular `ObjectCreationExpression` nodes. Once the
`CallSite` exists, signature help and named parameter completion should
work without further changes.

---

## 6. `@method` with `static`/`$this` return type on parent (2 fixtures)

**Ref:** [type-inference.md §26](type-inference.md#26-method-with-static-or-this-return-type-on-parent-class)
**Impact: Medium · Effort: Low-Medium**

When a parent class declares `@method static foo()` or
`@method $this bar()`, calling the method on a child class should
return the child class type. Virtual method return types are not
rewritten through the inheritance chain today.

**Fixtures:**

- [ ] `virtual_member/method_returns_static.fixture` — `@method static foo()` on parent, called on child
- [ ] `virtual_member/method_returns_this.fixture` — `@method $this bar()` on parent, called on child

**Implementation notes:**

During inheritance merging, when copying virtual methods from a parent
class, apply the same `static`/`$this`/`self` return type rewriting
that already works for regular methods. Check `rewrite_self_static` in
`inheritance.rs` and ensure it also processes `MethodInfo` entries that
originated from `@method` tags.

---

## 7. Invoked closure/arrow function return type (2 fixtures)

**Ref:** [type-inference.md §30](type-inference.md#30-invoked-closurearrow-function-return-type)
**Impact: Low · Effort: Low-Medium**

Immediately invoked closures and arrow functions do not resolve their
return type. `(fn(): Foo => new Foo())()` and similar patterns produce
`mixed`.

**Fixtures:**

- [ ] `call_expression/arrow_fn_invocation.fixture` — `(fn() => new Foo())()->` resolves
- [ ] `arrow_function/parameter_type.fixture` — arrow function parameter type for completion inside body

**Implementation notes:**

In the call expression resolution path, detect when the callee is a
parenthesized closure or arrow function expression. Extract the return
type from its signature or body. For `arrow_function/parameter_type`,
the arrow function parameter's type hint should be resolved the same
way closure parameters are (likely in `variable/closure_resolution.rs`).

---

## 8. `new $classStringVar` / `$classStringVar::staticMethod()` (2 fixtures)

**Ref:** [type-inference.md §27](type-inference.md#27-new-classstringvar-and-classstringvarstaticmethod)
**Impact: Low-Medium · Effort: Medium**

When a variable holds a `class-string<Foo>`, `new $var` should resolve
to `Foo` and `$var::staticMethod()` should resolve through `Foo`'s
static methods.

**Fixtures:**

- [ ] `type/class_string_new.fixture` — `new $classStringVar` resolves to the class type
- [ ] `type/class_string_static_call.fixture` — `$classStringVar::staticMethod()` resolves return type

**Implementation notes:**

In the object creation and static call resolution paths, when the class
name is a variable, resolve the variable's type. If it is
`class-string<T>`, extract `T` and use it as the class name.

---

## 9. Enum case instance properties (2 fixtures)

**Ref:** [bugs.md §9](bugs.md#9-enum-case-instance-properties-not-shown-in---completion)
**Impact: Medium · Effort: Low**

`->` completion on an enum case does not show the `name` property
(available on all enums via `UnitEnum`) or the `value` property
(available on backed enums via `BackedEnum`). The enum's own methods
and trait methods appear, but these built-in properties are missing.

**Fixtures:**

- [ ] `enum/enum_case_members.fixture` — `$case->name` available on unit enum
- [ ] `enum/backed_enum_case_members.fixture` — `$case->value` and `$case->name` on backed enum

**Implementation notes:**

During enum class resolution (or in the completion builder), inject
synthetic `PropertyInfo` entries for `name` (type `string`, on all
enums) and `value` (type matching the backing type, on backed enums).
These are defined by the `UnitEnum` and `BackedEnum` interfaces in the
stubs, so alternatively ensure that enum resolution inherits from those
interfaces and their properties are included.

---

## 10. Mixed `->` then `::` accessor chaining (2 fixtures)

**Ref:** [bugs.md §10](bugs.md#10-mixed-arrow-then-static-accessor-chaining-not-resolved)
**Impact: Low · Effort: Low**

`$obj->prop::$staticProp` and `$obj->method()::staticMethod()` are not
resolved. The subject extractor does not handle a transition from `->`
to `::` within the same chain.

**Fixtures:**

- [ ] `completion/static_prop_after_arrow.fixture` — `$obj->prop::$staticProp` chain
- [ ] `member_access/static_property_instance.fixture` — same pattern in member_access context

**Implementation notes:**

In subject extraction (or the AST-based chain walker), when processing
a chain segment that switches from instance (`->`) to static (`::`)
access, resolve the instance segment first, then use its result type as
the class for the static access.

---

## 11. `class-string<T>` on interface method not inherited (1 fixture)

**Ref:** [type-inference.md §25](type-inference.md#25-class-stringt-on-interface-method-not-inherited)
**Impact: Medium · Effort: Medium**

When an interface method uses `class-string<T>` and a class implements
that interface, the generic return type is lost during inheritance
merging.

**Fixture:**

- [ ] `generics/class_string_generic_interface.fixture` — `class-string<T>` on interface method not propagated

---

## 12. Sequential `assert()` calls do not accumulate (1 fixture)

**Ref:** [type-inference.md §22](type-inference.md#22-sequential-assert-calls-do-not-accumulate)
**Impact: Low-Medium · Effort: Low**

Multiple `assert($x instanceof Foo); assert($x instanceof Bar);`
statements should accumulate. Only the last assertion's narrowing
applies today.

**Fixture:**

- [ ] `combination/intersect_interface_assert.fixture` — sequential assert narrows to both types

---

## 13. Compound negated guard clause narrowing (1 fixture)

**Ref:** [type-inference.md §23](type-inference.md#23-double-negated-instanceof-narrowing) (related)
**Impact: Low · Effort: Low-Medium**

After `if (!$x instanceof A && !$x instanceof B) { return; }`, the
surviving code should know that `$x` is `A|B`. This requires the
narrowing engine to invert compound negated conditions across guard
clauses.

**Fixture:**

- [ ] `completion/parenthesized_narrowing.fixture` — compound negated instanceof with guard clause narrows to union

---

## 15. Negated `@phpstan-assert !Type` (1 fixture)

**Ref:** [type-inference.md §19](type-inference.md#19-negated-phpstan-assert-type)
**Impact: Medium · Effort: Low-Medium**

`@phpstan-assert !Foo $param` should remove `Foo` from the variable's
union type. The `!` prefix is not parsed today.

**Fixture:**

- [ ] `narrowing/phpstan_assert_negated.fixture` — negated assert removes type from union

---

## 16. Generic `@phpstan-assert` with `class-string<T>` (1 fixture)

**Ref:** [type-inference.md §20](type-inference.md#20-generic-phpstan-assert-with-class-stringt-parameter-inference)
**Impact: Medium · Effort: Medium-High**

`@phpstan-assert T $value` with `@template T` bound via a
`class-string<T>` parameter should infer the narrowed type from the
class-string argument at the call site.

**Fixture:**

- [ ] `narrowing/phpstan_assert_generic.fixture` — `assertInstanceOf(Foo::class, $x)` narrows `$x` to `Foo`

---

## 17. Accessor on new line with whitespace (1 fixture)

**Ref:** [bugs.md §8](bugs.md#8-accessor-on-new-line-with-extra-whitespace-not-resolved)
**Impact: Medium · Effort: Low**

When `->` is on a new line with leading whitespace before the cursor,
subject extraction fails.

**Fixture:**

- [ ] `completion/accessor_on_new_line.fixture` — `$obj\n    ->` resolves

---

## 18. Partial static property prefix filtering (1 fixture)

**Ref:** [bugs.md §11](bugs.md#11-partial-static-property-prefix-filtering-returns-empty-results)
**Impact: Low · Effort: Low**

`$foobar::$f` returns empty results instead of filtering static
properties starting with `$f`. The `$` prefix is not stripped correctly
during matching.

**Fixture:**

- [ ] `completion/partial_static_property.fixture` — `$foobar::$f<>` filters static properties

---

## 19. Inline `(new Foo)->method()` chaining (1 fixture)

**Ref:** [bugs.md §12](bugs.md#12-inline-new-foo-method-chaining-not-resolved)
**Impact: Medium · Effort: Low-Medium**

Parenthesized `new` expressions used inline as the root of a method
chain do not resolve for completion. `$x = (new Foo())` works, but
`(new Foo())->method()->` does not.

**Fixture:**

- [ ] `member_access/new_no_parenthesis.fixture` — `(new Foo)->bar()->` resolves

---

## 20. Elseif chain narrowing with `is_*()` (1 fixture)

**Ref:** [type-inference.md §3](type-inference.md#3-parse-and-resolve-param-is-t--a--b-return-types) (related)
**Impact: Medium · Effort: Medium**

Simple `is_string()` narrowing works (tested in the passing
is_string_narrowing fixture), but an `if/elseif/else` chain
with `is_string` in the `if` and `is_int` in the `elseif` does not
strip both types in the `else` branch. This is an elseif-chain
narrowing propagation issue rather than `is_*()` parsing.

**Fixture:**

- [ ] `function/is_type_elseif_chain.fixture` — elseif chain strips `string` and `int`, leaving `Foobar` in else

---

## 21. `iterator_to_array()` return type (1 fixture)

**Ref:** [completion.md §1](completion.md#1-array-functions-needing-new-code-paths)
**Impact: Medium · Effort: Medium**

`iterator_to_array()` should return the iterator's value type as an
array element type. This needs a special code path similar to the
existing `array_pop`/`array_shift` handling.

**Fixture:**

- [ ] `function/iterator_to_array.fixture` — `iterator_to_array($gen)` resolves element type

---

## 22. Literal string conditional return type (1 fixture)

**Ref:** [type-inference.md §24](type-inference.md#24-literal-string-conditional-return-type)
**Impact: Low · Effort: Low-Medium**

Conditional return types using literal string comparison
(`$param is "foo"`) are not resolved. Only class/interface type
conditions work today.

**Fixture:**

- [ ] `type/conditional_return_type_string.fixture` — literal string conditional resolves correct branch

---

## 23. `@phpstan-type` alias in foreach context (1 fixture)

**Ref:** [type-inference.md §29](type-inference.md#29-phpstan-type-alias-in-foreach-context)
**Impact: Low · Effort: Low**

When a method's return type uses a `@phpstan-type` alias and the result
is iterated in a `foreach`, the alias is not resolved before extracting
the foreach value type.

**Fixture:**

- [ ] `type/phpstan_type_alias.fixture` — type alias resolves for foreach iteration

---

## 24. Variable scope isolation in closures (1 fixture)

**Impact: Low · Effort: Low-Medium**

Variables declared outside a closure are visible inside the closure body
even without a `use()` clause. PHP closures have strict scope isolation:
only variables captured via `use($var)` or superglobals should be
available.

**Fixture:**

- [ ] `variable/closure_scope_isolation.fixture` — `$foobar` and `$barfoo` not visible inside closure without `use()`

**Implementation notes:**

During variable resolution, when the cursor is inside a closure body,
restrict the variable search scope to: (a) variables defined within the
closure body, (b) variables explicitly captured in the `use()` clause,
(c) `$this` if the closure is not `static`, and (d) superglobals. Do
not walk past the closure boundary into the enclosing scope.

---

## 25. Pass-by-reference parameter type inference (1 fixture)

**Ref:** [type-inference.md §7](type-inference.md#7-narrow-types-of-var-parameters-after-function-calls)
**Impact: Low · Effort: Medium**

Functions that accept `&$var` parameters can change the variable's type.
After calling such a function, the variable's type should reflect the
function's documented effect (e.g. `preg_match($pattern, $subject, $matches)`
should give `$matches` an array type).

**Fixture:**

- [ ] `variable/pass_by_reference.fixture` — `&$var` parameter type inferred after call

---

## 26. Pipe operator (PHP 8.5) (1 fixture)

**Ref:** [type-inference.md §1](type-inference.md#1-pipe-operator-php-85)
**Impact: Low · Effort: Medium**

The `|>` pipe operator (PHP 8.5) passes the left side as the first
argument to the right side and returns the result.

**Fixture:**

- [ ] `pipe_operator/basic_pipe.fixture` — `$x |> foo(...)` resolves return type

---

## Summary by effort

Quick wins (Low effort, 1 fixture each):

| Task | Fixture |
|---|---|
| §17 Accessor on new line | `completion/accessor_on_new_line` |
| §18 Partial static property prefix | `completion/partial_static_property` |
| §23 `@phpstan-type` in foreach | `type/phpstan_type_alias` |
| §12 Sequential assert accumulation | `combination/intersect_interface_assert` |

Moderate wins (Low-Medium effort, multiple fixtures):

| Task | Fixtures |
|---|---|
| §6 `@method` static/`$this` rewriting | 2 |
| §9 Enum case `name`/`value` properties | 2 |
| §10 Mixed `->` then `::` chaining | 2 |

Biggest unlocks (Medium effort, many fixtures):

| Task | Fixtures |
|---|---|
| §1 `@implements` generic resolution | 7 |
| §2 Function-level `@template` | 9 |
| §3 Property-level narrowing | 5 |
| §4 `@phpstan-assert` on static methods | 3 |
| §5 Attribute context support | 3 |