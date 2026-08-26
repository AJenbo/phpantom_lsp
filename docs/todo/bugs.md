# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Complexity** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

All entries below come from the 2026-08-25 triage of the PHPStan Source
sample project (242 confirmed false positives after the genuine findings
were patched in the sample). Site counts refer to that sweep; every
mechanism was either reproduced in a minimal project or confirmed by
reading the guard construct PHPStan honours. The sweep is a snapshot, so
a site named here may already read differently: re-run the analyser
before working an entry, and trim the shapes that no longer reproduce.

Entries are grouped by the mechanism that has to change, not by the
symptom that surfaced: one entry is one root cause, however many shapes
it shows up in. Splitting a shape out into its own entry because it
reads differently in the source is how this list grew past forty in the
first place. If two entries would be fixed by the same change, they are
one entry.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

### B290. Standard-library results stay wider than their arguments justify

**Impact: Medium · Complexity: Medium-High**

6 sites, four shapes of one missing capability: reading the arguments a
builtin was actually handed and computing the answer from them, instead
of falling back on the widest type the signature allows.

- `strtolower()` of a literal union must constant-fold
  (`'Interface'|'Trait'|'Enum'|'Class'` →
  `'interface'|'trait'|'enum'|'class'`;
  `src/Rules/Classes/DuplicateDeclarationRule.php:37`). There is no
  literal folding of string builtins in the type engine at all: the
  only const-evaluator that folds `strtolower`/`str_replace`/`sprintf`
  is the Laravel route-name one in
  `virtual_members::laravel::const_eval`, which is not on the shared
  resolution path.
- `method_exists(TestCase::class, 'assertFileDoesNotExist')` with a
  statically known class and a literal method name folds to a constant
  `true`, so the `!method_exists(...)` branch is dead — yet we report
  the removed PHPUnit 9 method called inside it
  (`src/Testing/LevelsTestCase.php:201`). PHPStan even flags the guard
  itself as `function.alreadyNarrowedType` (it's in the sample's
  baseline) and reports nothing inside the dead branch.
- A `(?<position>\d+)` group in `Strings::matchAll()` results should be
  a numeric (decimal integer) string, so that
  `$placeholder['position'] - 1` is `int` rather than `int|float`
  (`src/Rules/Functions/PrintfHelper.php:113`).
- `pow()`'s `object` return belongs to the operator-overloading
  extensions (GMP, BCMath), so two operands that are provably numeric
  produce `int|float` and an operand that is provably an object
  produces `object`. An operand typed `mixed` decides neither, and a
  conditional return type that cannot be decided answers with the union
  of both branches — which is honest (a `mixed` really could hold a
  `GMP`) but reports what PHPStan does not: `private static function
  pow(mixed $base, mixed $exp): float|int|null` returning
  `pow($base, $exp)` is flagged as returning an `object` its signature
  does not allow (`src/Type/ExponentiateHelper.php:128`). Fixing this
  one needs a way to say "prefer the narrow branch when the argument
  cannot be pinned down" for a specific conditional, which
  `type_engine::types::conditional` has no vocabulary for today: the
  undecided case always unions.

## Narrowing

### B270. Narrowing a repeated or non-variable subject doesn't survive

**Impact: High · Complexity: Very High**

44 sites — by far the largest cluster here, and one root cause: what the
narrowing store keys a proof against, and what invalidates it. PHPStan
keys specified types by expression string and keeps them until something
writes to that expression. We handle plain variables reliably and
everything else fragilely, which surfaces as four shapes. Related to the
reconciliation engine planned as
[T20](type-inference.md#t20-type-narrowing-reconciliation-engine).

**a. A guard on a call's result doesn't narrow the same call afterwards**
(18 sites). This ubiquitous idiom is clean under PHPStan and re-resolved
from the declaration by us, which hands back the wide type:

```php
if ($analyserResult->getDependencies() !== null) {
    $this->switchTmpFile($analyserResult->getDependencies(), ...); // non-null here
}
// and the ternary form:
$f = $r->getFileName() !== false ? $r->getFileName() : null;      // string|null
```

Concentrated around reflection accessors (`getFileName()`,
`getDocComment()`, `getResolvedPhpDoc()`, `getDependencies()`).
Sites: `src/Analyser/NodeScopeResolver.php:5365`,
`src/Analyser/ResultCache/ResultCacheManager.php:858`,
`src/Command/AnalyseApplication.php:329, 333, 337`,
`src/PhpDoc/PhpDocInheritanceResolver.php:246, 282`,
`src/Reflection/BetterReflection/BetterReflectionProvider.php:302 (×2), 348, 356`,
`src/Reflection/Php/PhpClassReflectionExtension.php:299, 313, 678, 682, 823, 866`,
`src/Rules/Exceptions/TooWidePropertyHookThrowTypeRule.php:61`.

**b. Property fetches, array dims and call chains lose their narrowing**
(16 sites). `instanceof` / `!== null` / `is_string()` guards whose
subject is not a plain variable, in specific reproduced shapes:

```php
// 1. Member use inside the condition breaks the narrowing for the body:
if ($this->pair !== null && $this->pair[0] instanceof SubA && $this->pair[0]->n > 0) {
    $this->pair[0]->n;        // "Property 'n' not found" — without the third leg it works
}
// 2. Chained getters re-read in the same condition:
if ($h->getExpr()->getExpr() instanceof Vari && is_string($h->getExpr()->getExpr()->name) && $flag) { ... }
// 3. Plain property fetch behind an elseif-continue:
if ($tag->value === null) { continue; } elseif (!($tag->value instanceof Sub)) { continue; }
$tag->value->n;               // lost
```

Notably `src/PhpDoc/TypeNodeResolver.php:1303` shows the engine *does*
apply consecutive `instanceof` subtractions on a property fetch but not
a final `!== null` in an `elseif`. Sites:
`src/Analyser/MutatingScope.php:2946, 2949`,
`src/Analyser/NodeScopeResolver.php:2873, 4837, 4843, 4999, 5102`,
`src/Analyser/TypeSpecifier.php:600`,
`src/PhpDoc/TypeNodeResolver.php:1303`,
`src/Rules/FunctionDefinitionCheck.php:195` (narrowed `$param`
`use`-captured by a closure),
`src/Rules/PhpDoc/InvalidPhpDocTagValueRule.php:98, 99`,
`src/Rules/TooWideTypehints/TooWideTypeCheck.php:214`,
`src/Type/Constant/ConstantArrayType.php:2567, 3503`,
`src/Type/ValueOfType.php:59`.

**c. Re-testing a condition, or a boolean flag holding it, doesn't
re-apply its narrowing** (9 sites). Three shapes PHPStan's
specified-types machinery handles:

```php
// 1. The identical condition re-tested later:
if (count($args) > 0) { $acceptor = Selector::selectFromArgs(...); }
if (count($args) > 0) { use($acceptor); }                    // non-null

// 2. A boolean flag recording an instanceof disjunction:
$shouldCheck = $node instanceof Function_ || $node instanceof ClassMethod || ...;
if ($stmtCount === 0 && $shouldCheck) { $node->getReturnType(); }

// 3. Two variables assigned together; checking one implies the other:
if ($assertions === null) { return null; } // $acceptor was set iff $assertions was
```

Sites: `src/Analyser/ExprHandler/FuncCallHandler.php:977, 1042`,
`src/Analyser/ExprHandler/MethodCallHandler.php:350`,
`src/Analyser/ExprHandler/StaticCallHandler.php:455`,
`src/Analyser/NodeScopeResolver.php:693, 707, 727, 732`,
`src/Type/TypeCombinator.php:1994`.

**d. Reading a property of `$this` before `$this instanceof Subclass`
kills the narrowing** (1 site, isolated to a two-line repro). The store
drops the proof it should be keeping:

```php
$description = $this->className;        // remove this line and the branch resolves
if ($this instanceof GenericObjectType) {
    $this->getTypes();                  // "Method 'getTypes' not found on ObjectType"
}
```

Regular and promoted properties both trigger it; same-file and
cross-file subclasses both fail. Site: `src/Type/ObjectType.php:744`.

### B274. A null-seeded accumulator filled in a loop keeps `null` (or loses its type entirely)

**Impact: High · Complexity: High**

18 sites, two symptoms of one shape — PHPStan's own scope-merging
idiom, repeated across five files:

```php
$finalScope = null;
foreach ($executionEnds as $e) {
    $endScope = $e->getStatementResult()->getScope();
    if ($finalScope === null) { $finalScope = $endScope; continue; }
    $finalScope = $finalScope->mergeWith($endScope);   // unresolved from here on
}
if ($finalScope !== null) { $finalScope->processNodes(...); }  // still unresolved
```

The `=== null` early-continue must leave the accumulator non-null on
the merge line, and the loop fixed point must not poison the variable
so badly that even an explicit `!== null` guard after the loop can't
recover it. The same root also leaves the accumulator nullable after a
loop that provably runs (`if (count($xs) > 0) { ... foreach ($xs) ... }`
or a literal `$files = [$file]`), and after a foreach over a local
array that every branch pushed into (reproduced minimally; also
`src/Analyser/TypeSpecifier.php:542`).

Sites: `src/Analyser/NodeScopeResolver.php:1103, 1112, 1116, 1903, 2001, 2153, 5405, 5413`,
`src/Rules/Properties/SetNonVirtualPropertyHookAssignRule.php:64, 72, 80, 81, 90`,
`src/Rules/TooWideTypehints/TooWideParameterOutTypeCheck.php:47, 56`,
`src/Reflection/BetterReflection/SourceLocator/OptimizedDirectorySourceLocator.php:149, 150`,
`src/Analyser/TypeSpecifier.php:542`.

### B276. A `@phpstan-assert bool` doesn't rule out `null`

**Impact: Low-Medium · Complexity: Medium**

1 site, reproduced minimally, and the last of what this entry used to
cover — the assert tag itself, on a property or on a method result, and
inherited two interfaces up, all narrow correctly now. What is left is
the single combination of asserted type and subject type that does not:

```php
/** @phpstan-assert bool $v */
private function assertBool(mixed $v): void {}

$this->assertBool($v);   // $v is ?bool
return $v;               // reported as bool|null
```

The neighbouring cases all work, which is what makes this a narrow
defect rather than a missing feature: `@phpstan-assert bool` over
`bool|string` narrows, `@phpstan-assert string` over `?string` narrows,
and the equivalent `if (is_bool($v))` guard over `?bool` narrows. Only
the assert-`bool`-over-`null` pairing leaves the `null` behind.
Site: `src/Reflection/ClassReflection.php:1524`.

### B277. `is_float()` branches don't eliminate `float` from `int|float`

**Impact: Medium · Complexity: Medium**

3 sites. The plain shape resolves correctly — an `int|float` subject
comes back as `float` in the `is_float()` branch, `int` in the else, and
`int` after a branch that reassigns it — so what breaks is the shape at
the sites, where the subject reaches the check through a swap
destructuring (`[$min, $max] = [$max, $min];`) and starts out
`int|float|null`:

```php
if ($min !== null && $max !== null && $min > $max) { [$min, $max] = [$max, $min]; }
if (is_float($min)) { $min = (int) ceil($min); }
IntegerRangeType::fromInterval($min, $max);   // we report null|int|int|float
```

The duplicated `int` and the surviving `null` in that result say the
destructuring, not the guard, is where the type is lost. Sites:
`src/Reflection/InitializerExprTypeResolver.php:2533 (both args)`,
`src/Type/Constant/ConstantArrayTypeBuilder.php:242`.

### B281. `instanceof self` in a trait narrows to the trait instead of the using class

**Impact: Medium · Complexity: Medium-High**

1 site, reproduced minimally. Inside a trait method, `self` is the
class using the trait; `$type instanceof self && $this->value === $type->value`
must resolve `$value` against that class. We narrow to an intersection
with the *trait* and report "Property 'value' not found on any of the 2
possible types (PHPStan\Type\Type, ...ConstantScalarTypeTrait)".
Site: `src/Type/Traits/ConstantScalarTypeTrait.php:74`.

## Arithmetic

No outstanding items.

## Symbol resolution

### B291. A name that is both a class and a namespace fails to resolve through imports and templates

**Impact: Medium · Complexity: Medium-High**

4 sites. `PhpParser\Node\Scalar` is a class (`Node/Scalar.php`) *and* a
namespace (`Node/Scalar/`). With `use PhpParser\Node\Scalar;` in scope,
a docblock `@param list<Scalar|...>` stays unresolved as the short name
`Scalar` (`build/PHPStan/Build/OrChainIdenticalComparisonToInArrayRule.php:107`),
and `@implements ExprHandler<Scalar>` + `@param T $expr` substitution
produces a `Scalar` that isn't recognised as a subtype of
`PhpParser\Node\Expr` even though `abstract class Scalar extends Expr`
(`src/Analyser/ExprHandler/ScalarHandler.php:49, 59, 64`).

## Array types

### B286. Array element types are lost by writes, and key checks don't restore them

**Impact: Medium-High · Complexity: Medium-High**

15 sites. Every shape below is the same gap seen from one side or the
other: the element type of an array the engine watched being built, or
the element type a key check proves is there.

- A locally built array of tuples read back by a destructuring foreach:
  `$offsetTypes[$key] = [$trinary, $type];` …
  `foreach ($offsetTypes as $key => [$hasOffsetValue, $offsetType])` —
  both variables come back unresolved
  (`src/Type/Php/ArrayMergeFunctionDynamicReturnTypeExtension.php:188, 250, 255, 258, 300, 306`,
  `src/Type/Php/ArrayReplaceFunctionReturnTypeExtension.php:177, 227, 231, 264, 270`).
- Indexing a docblock shape through two dims unions the tuple slots
  instead of selecting one: `$alternatives[$exprString][1]` on
  `array<string, array{Expr, list<...>}>` returns `Expr|list<...>`
  (`src/Analyser/SpecifiedTypes.php:587`).
- `isset(self::$anonymousClasses[$className])` doesn't make the
  subsequent read of that offset resolve (static property with a
  `@var ClassReflection[]` docblock —
  `src/Reflection/BetterReflection/BetterReflectionProvider.php:170`),
  and `isset($options['default'])` over `array<string, ?Type>` must
  strip `null` from the value type
  (`src/Type/Php/FilterFunctionReturnTypeHelper.php:198`).
- The mirror image, where the *key* is what the check proves something
  about: `array_key_exists($tag, $flippedMapping)` over an
  `array<string, class-string>` proves `$tag` is a `string`, so a `$tag`
  read out of `array_keys()` on a bare `array` (Nette's
  `Definition::getTags()`) stops being `array-key` inside the branch.
  `isset($arr[$k])` says the same thing. Site:
  `src/DependencyInjection/ValidateServiceTagsExtension.php:93`.

## Docblock handling

### B295. Values PHPStan types as `mixed` come back unresolved, or narrower than the docblock says

**Impact: High · Complexity: High**

The biggest cluster left: ~46 sites. The sample analyses clean under
PHPStan level 8 because these values are `mixed` there, and level 8
doesn't check members of or arguments from `mixed`. Our own severity
table in [`todo/diagnostics.md`](diagnostics.md) already classifies
"`mixed` subject member access" as an opt-in **Hint** — but the engine
returns *unresolved* instead of `mixed` for these sources, so the
diagnostics fire as errors:

- Elements of an undocblocked `array` parameter (closures included) —
  e.g. `src/Type/UnionType.php:527`, where `instanceof`-guarded
  accesses on the same element are correctly silent and only the bare
  `else` branch reports.
- `getAttribute()`-style `@return mixed` values combined with `??`
  (`mixed ?? Arg` must be `mixed`) and flowing through `use()` captures
  (`src/Reflection/ParametersAcceptorSelector.php:129-145`,
  `src/Command/CommandHelper.php:608`, `src/Command/WorkerCommand.php:130`).
- `@param mixed ...$args` variadic elements
  (`src/Testing/TypeInferenceTestCase.php:161, 212, 225`).
- Dynamic static-method names (`TrinaryLogic::{$name}()`,
  `src/Rules/Debug/FileAssertRule.php:222, 227`) and static calls on
  `$node['type']` from untyped arrays
  (`src/Dependency/ExportedNode/*.php`, `src/Parallel/ParallelAnalyser.php:345`).
- List-destructuring a `@return mixed|null` cache load
  (`src/Type/FileTypeMapper.php:400`).
- An array accumulated from an undocblocked `array $json` argument, which
  makes its element type unreadable and blocks the rules that read one:
  `array_sum($peakMemoryUsages)` stays on the stub's `int|float` rather
  than folding to `int`, because the array was filled from
  `$json['memoryUsage']` (`src/Parallel/ParallelAnalyser.php:158`).

Conversely, where the docblock *says* `mixed`, we substitute a narrower
body-inferred type and then flag mismatches PHPStan never sees
(`src/DependencyInjection/ContainerFactory.php:335, 403`,
`src/Command/ErrorFormatter/BaselineNeonErrorFormatter.php:100`,
`src/Testing/TestCaseSourceLocatorFactory.php:75`) — and a docblock
`mixed` doesn't even agree with a native one: `strlen($s) + $line - 1`
where `$line` comes from an untyped method with `@return mixed`
(narrowed by `!== null`) produces `int|float`, while the same code with
a *native* `mixed` return type produces `int`, which is what PHPStan
reports (`src/Rules/PhpDoc/PhpDocLineHelper.php:25`).

The fix direction: these sources must uniformly produce `mixed`,
whichever spelling declares it, and member/argument checks on `mixed`
must follow the severity policy (hint, not error).

### B296. Closure and arrow-function signatures aren't completed from their context and body

**Impact: High · Complexity: High**

23 sites, three shapes. A closure's declared signature is only part of
what its parameters and return type are: the rest comes from the
parameter it is passed to, from its own body, and from a docblock that
isn't attached to the closure node.

**a. Callback parameters aren't inferred from templated callable
parameters** (16 sites). The `usort` stub declares `@template T` /
`@param TArray $array` / `@param callable(T, T): int $callback`; an
untyped closure passed as the callback must get its parameters bound
from the array argument's element type:

```php
/** @return array{list<Error>, list<IdentifierRuleError>} */
[$actualErrors, $delayed] = $this->gather(...);
usort($actualErrors, static function ($a, $b) { return $a->getLine() <=> $b->getLine(); });
// $a, $b must be Error
```

Sites: `src/Testing/RuleTestCase.php:213-231` (13),
`src/DependencyInjection/ContainerExtensionsExtension.php:45` (×2, with
a generic `TargetClass<T>` element), and the return-position variant
`array_map(static fn ($builder) => $builder->build(), $errorBuilders)`
where `build(): T` on a union of two `RuleErrorBuilder<...>`
instantiations comes back as the builder itself
(`build/PHPStan/Build/NamedArgumentsRule.php:238`,
a `type_mismatch_return`).

**b. Arrow-function return types ignore the body** (4 sites). PHPStan
computes an arrow function's return type as the intersection of the
declared type and the body's inferred type. We use only the declared
type, so `static fn (X $p): MethodReflection => $p->getTransformedMethod()`
(body returns `ExtendedMethodReflection`) fails against an
`ExtendedMethodReflection[]` parameter.
Sites: `src/Reflection/Type/IntersectionTypeUnresolvedMethodPrototypeReflection.php:48`,
`src/Reflection/Type/IntersectionTypeUnresolvedPropertyPrototypeReflection.php:47`,
`src/Reflection/Type/UnionTypeUnresolvedMethodPrototypeReflection.php:49`,
`src/Reflection/Type/UnionTypeUnresolvedPropertyPrototypeReflection.php:47`.

**c. `@param` docblocks aren't applied in two placement shapes**
(3 sites):

- A doc comment above `$closure = static function (...) { ... };`
  attaches to the expression statement, not the closure node; its
  `@param` tags must still type the closure's parameters
  (`src/Analyser/ExprHandler/FuncCallHandler.php:707, 708`).
- A trait method implementing an interface method must inherit the
  interface's `@param` docblock (`Type::getTemplateType()` declares
  `@param class-string`; the trait implementation loses it —
  `src/Type/Traits/LateResolvableTypeTrait.php:86`).

### B299. By-reference out-parameter types: no body inference, and by-ref inputs are type-checked

**Impact: Medium · Complexity: Medium-High**

2 sites, complementing [T41](type-inference.md#t41-param-out-is-parsed-but-never-read):

- A by-ref parameter the callee unconditionally assigns (no
  `@param-out` tag) should get the assigned type after the call
  (`ScopeOps::getTypeFromCache(..., ?string &$key)` always sets a
  `string`; `src/Analyser/MutatingScope.php:1031`).
- The *input* type of a by-ref argument that merely creates the
  variable must not be checked at all — PHPStan skips it
  (`preg_match_all(..., $matches, PREG_OFFSET_CAPTURE)` where the
  variable still holds the previous iteration's shape;
  `src/Parser/RichParser.php:183`).

### B300. Template arguments aren't recovered from `class-string`, `@implements`, or a constructor argument

**Impact: Medium · Complexity: High**

8 sites, two shapes. Both need a template argument that is nowhere in
the call's own type arguments, and both then have to carry the recovered
value through the expressions that read it.

- `CollectedDataNode::get()` declares
  `@template TCollector of Collector<Node, TValue>` /
  `@param class-string<TCollector>` /
  `@return array<string, list<TValue>>`; `TValue` must be recovered from
  the collector class's `@implements Collector<..., array{...}>`. The
  value then survives nested array writes and an `array_values(...)[0]`.
  Sites: `src/Rules/Comparison/ConstantConditionInTraitRule.php:68, 80`,
  `src/Rules/Comparison/FunctionCallConstantConditionRule.php:100, 119`.
- PHPStan's `RecursiveIteratorIterator` stub is
  `@template T of \RecursiveIterator|\IteratorAggregate` with `@mixin T`;
  `foreach (new RecursiveIteratorIterator(new RecursiveDirectoryIterator($dir)) as $file)`
  must bind `T` from the constructor argument and resolve iteration
  through the mixin to `SplFileInfo`. Line 169 is downstream of the same
  unresolved `$file`:
  `str_replace(DIRECTORY_SEPARATOR, '/', $file->getPathname())` has a
  subject nobody can type, so the conditional return keyed on it answers
  with both the array and the string branch. Sites:
  `build/PHPStan/Build/TurboAttributeCollector.php:162, 165, 169`,
  `src/Cache/FileCacheStorage.php:151`.

## Miscellaneous

No outstanding items.
