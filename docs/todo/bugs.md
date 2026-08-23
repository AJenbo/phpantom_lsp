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

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

### B227. The replace function family's conditional return type never collapses

**Impact: Medium-High · Complexity: Low-Medium**

`str_replace`, `str_ireplace`, `substr_replace`, `preg_replace`,
`preg_replace_callback`, `preg_replace_callback_array`, and `preg_filter`
are patched in `stub_patches.rs` (`patch_replace_family`) with a
conditional return type keyed on `$subject`: array in, array out;
string in, string out. In practice the conditional never resolves to a
single branch — it always falls back to the full `string|array<string>`
union, even when `$subject` is a plain, unambiguous `string` local or a
string literal:

```php
function relative(string $filename): string
{
    return str_replace('\\', '/', $filename); // string|array<string>, not string
}
```

By contrast, other functions patched the same way but keyed on an
"is string" discriminant (`range()`, keyed on `$start`) resolve
correctly. The "is array" discriminant is the common thread across
every failing case (`patch_range` uses `PhpType::named(atom("string"))`
as its condition; `patch_replace_family` uses `PhpType::array()`), which
points at `condition_category`/`type_category` in
`type_engine/types/conditional.rs` or the conditional-evaluation
dispatch in `type_engine/types/narrowing/assertions.rs`
(`evaluate_conditional_for`) mishandling an array discriminant
specifically. Confirmed via minimal repro with a string variable, a
string literal, and a plain array variable as `$subject` — all three
return the unresolved union instead of picking a branch.

Real-world hits: any `str_replace()`/`str_ireplace()` call whose result
feeds a `string`-typed parameter or return raises a false
`type_mismatch_argument`/`type_mismatch_return`.

## Narrowing

### B228. A condition-established narrowing of a method-call expression doesn't survive assignment, `elseif` siblings, or loop merges

**Impact: High · Complexity: Very High**

`@phpstan-assert-if-true`/`-if-false` tags whose subject is a method-call
expression off the receiver (`$this->getFoo()`, written `$this->getFoo()`
in the tag) narrow correctly for a *direct* re-evaluation of that same
call right after the guard:

```php
if (!$scope->isInClass()) {
    return;
}
$scope->getClassReflection()->isFinal(); // fine: direct re-evaluation
```

But the narrowing is lost as soon as the guarded fact has to survive
anything beyond that:

- **Assigned to a variable**, then the variable used elsewhere:
  ```php
  if (!$scope->isInClass()) {
      return;
  }
  $classReflection = $scope->getClassReflection(); // stored type is still ?ClassReflection
  $this->consistentConstructorHelper->findConsistentConstructor($classReflection); // false type_mismatch_argument
  ```
  (`src/Rules/Classes/InstantiationRule.php:153`, and the `$declaringClass`/`$parametersAcceptor`/`$scope` clusters throughout `type_mismatch_argument`.)
- **A sibling `elseif`'s condition** also checks the same guard method,
  which corrupts the fact for a later, unrelated branch
  (`src/Analyser/ExprHandler/InstanceofHandler.php:140,142`).
- **A `foreach` + `continue` guard** (`if (!$scope->isInClass()) { continue; }`)
  loses the fact for the rest of the loop body
  (`src/Rules/Api/NodeConnectingVisitorAttributesRule.php:62`,
  `src/Rules/Properties/ReadOnlyByPhpDocPropertyAssignRule.php:88`,
  `src/Rules/Properties/ReadOnlyPropertyAssignRule.php:78,81`).
- **A plain (non-assert-if-true) `=== null` + `continue` guard on a
  repeated method call**, once nested inside an outer loop with extra
  loop-scoped state, loses the fact the same way
  (`src/Type/Php/BackedEnumFromMethodDynamicReturnTypeExtension.php:67`).
- The same shape recurs, unconfirmed in isolation but structurally
  identical to the above, in `src/Analyser/NodeScopeResolver.php:1103,1112,1116`
  (`$finalScope === null` → assign → `continue`, later `!== null` → use).

The mechanism that establishes these facts is the synthetic-key scope
tracking in `type_engine/variable/forward_walk/cond_narrowing.rs`
(`apply_assertion_to_key`, `seed_synthetic_key_if_needed`, keyed by a
string like `$scope->getClassReflection()`). Direct re-evaluation of the
same call expression clearly consults this key (the first example
works), but the general-purpose RHS-assignment path
(`type_engine/variable/rhs_resolution/`) computes a method call's type
from the raw signature only and never checks whether the call's printed
form matches a currently-narrowed synthetic key — and whatever merges
scope state at branch/loop boundaries in `forward_walk/control_flow.rs`
doesn't carry synthetic keys across the merge either. Fixing the
assignment path and the merge path are likely two separate, but
related, changes.

### B229. `instanceof` narrowing is not recognized for several common condition shapes

**Impact: High · Complexity: High**

Plain `$var instanceof Foo` narrowing works for the simple case (a bare
local variable, tested directly in an `if`'s top-level condition), but
several very common variations on the same idiom are not recognized —
this is the dominant false-positive cluster in `unknown_member` (60+ of
105 diagnostics):

- **A right-hand `&&` clause inside an `elseif` condition** doesn't see
  the `instanceof` on its left:
  ```php
  } elseif ($expr->var instanceof Variable && is_string($expr->var->name)) {
  ```
  (`src/Analyser/ExprHandler/AssignHandler.php:167`,
  `src/Analyser/ExprHandler/MatchHandler.php:279`,
  `src/PhpDoc/TypeNodeResolver.php:786`, `src/Type/FileTypeMapper.php:467`.)
- **An array-index expression as the `instanceof` subject**
  (`$types[$i] instanceof MixedType && !$types[$i]->isExplicitMixed()`),
  even inside a plain `if` and across statements in the body
  (`src/Type/TypeCombinator.php:251-297` — 6 lines,
  `src/Type/Constant/ConstantArrayType.php:2567,3503`,
  `src/Type/Generic/GenericObjectType.php:323`).
- **A ternary condition**
  (`$paramType instanceof UnionType ? $paramType->getTypes() : [...]`)
  (`src/Reflection/GenericParametersAcceptorResolver.php:176`).
- **`$this`**, once the enclosing class implements *any* interface
  (confirmed: identical code is fine with no `implements`, breaks the
  moment one is added) (`src/Type/ValueOfType.php:59`,
  `src/Type/ObjectType.php:744`).

Likely home: `type_engine/variable/forward_walk/` — the condition
narrowing appears to special-case a plain `if` over a simple local
variable subject and not generalize to `ElseIf_`/`Ternary`/array-index/
property-fetch subjects the way real PHPStan's `TypeSpecifier` does.

### B230. `$a === $b` doesn't narrow the nullable side against a definite-type operand

**Impact: Medium · Complexity: Medium**

```php
$inAssignRightSideVariableName = $context->getInAssignRightSideVariableName(); // ?string
// ...
if ($inAssignRightSideVariableName === $use->var->name) { // $use->var->name is string
    $scope->hasVariableType($inAssignRightSideVariableName); // false: expects string, got ?string
```

(`src/Analyser/NodeScopeResolver.php:3080-3098`.) For `===` to hold, both
operands must share a value, so a nullable variable compared identical
to a definite non-null value narrows to that non-null type within the
truthy branch — real PHPStan does this and is silent on this file.
`type_engine/variable/forward_walk/cond_narrowing.rs` has specific
handling for `$x === null`/`$x == null` but nothing for `$x === $y`
where `$y`'s type is independently known to exclude null.

### B231. `array_key_exists()` doesn't narrow an optional array-shape key the way `isset()` does

**Impact: Medium · Complexity: Medium**

```php
/** @param array{a?: array<int,string>} $shape */
function get(array $shape): array
{
    if (isset($shape['a'])) {
        return $shape['a']; // fine: array<int,string>
    }
    if (array_key_exists('a', $shape)) {
        return $shape['a']; // false type_mismatch_return: ?array<int,string> vs array
    }
    return [];
}
```

Confirmed against `src/File/FileExcluderFactory.php:37,47`
(`array_key_exists('analyse', $this->excludePaths)` guarding
`$this->excludePaths['analyse']`, an optional key in a promoted
property's docblock shape) and reproduced standalone for both local
variables and properties, so it isn't a property-vs-local-var
distinction — `array_key_exists` on an optional shape key simply isn't
wired into the same narrowing `isset()` already gets.

### B232. `use (&$var)` doesn't establish `$var` as defined when it's new to the enclosing scope

**Impact: Medium · Complexity: Medium**

```php
$linesOfCode = 0;              // <- if this line didn't exist:
$counter = function () use (&$linesOfCode): void {
    $linesOfCode++;            // by-ref use auto-vivifies $linesOfCode as null
};
```

PHP auto-vivifies a by-reference `use` capture that doesn't already
exist in the outer scope (as `null`), which is legal and is exactly
`src/Analyser/NodeScopeResolver.php:3666` (`$hookImpurePoints`) and
`src/Command/AnalyseApplication.php:272` (`$linesOfCode`) — both are
only ever assigned *inside* the closure body. PHPantom's forward walker
reports "Undefined variable" at the `use` clause instead of treating it
as a declaration. Home: `type_engine/variable/forward_walk/`, wherever
closure `use` clauses are processed.

### B233. `isset($var) && …` doesn't mark a possibly-undefined variable as defined for the rest of the branch

**Impact: Medium · Complexity: Medium**

```php
if (isset($tokenType) && $tokenType !== IgnoreLexer::TOKEN_WHITESPACE) {
    $lastTokenTypeLabel = $this->tokenLabel($tokenType); // false "Undefined variable"
}
```

(`src/Parser/RichParser.php:308-309` — `$tokenType` is only assigned
starting on the second iteration of the enclosing loop, so on the first
pass it's genuinely possibly-undefined outside the guard, but
`isset($tokenType)` being true in the truthy branch guarantees it's
defined for the rest of that branch, which real PHPStan accounts for
and PHPantom does not.)

### B234. `is_float()`/`is_int()`'s negative branch doesn't strip a member from a declared union

**Impact: Medium · Complexity: Medium**

```php
/** @var int|float $newAutoIndex */
$newAutoIndex = $offsetValue + 1;
if (is_float($newAutoIndex)) {
    // ...
} elseif (!$optional) {
    $this->nextAutoIndexes = [$newAutoIndex]; // false: expects list<int>, got array{int|float}
```

(`src/Type/Constant/ConstantArrayTypeBuilder.php:235-242`.) Real PHPStan
narrows `$newAutoIndex` to plain `int` inside the `is_float()` check's
negative branch (`elseif`); PHPantom leaves the full `int|float` union
in place, which then leaks into the array-literal element type.

### B235. A lost chained-call re-narrowing misattributes the member lookup to the enclosing file's own class

**Impact: Medium-High · Complexity: Medium-High**

When a `!== null` check on a method-call chain
(`$scope->getParentScope() !== null`) is reused later without the
chain's result being cached, PHPantom doesn't just fail to narrow — it
misattributes the member lookup to the *current file's own class*,
producing a diagnostic that names a completely unrelated class:

```
Method 'hasExpressionType' not found on class 'PHPStan\Debug\ReproChainReuse'
```

for code that never mentions `ReproChainReuse` at all. Confirmed at
`src/Node/ClassPropertiesNode.php:246` and
`src/Reflection/BetterReflection/BetterReflectionProvider.php:170`. Same
narrowing subsystem as B228, but called out separately because the
symptom actively misleads about *which* class is missing the member,
not just *that* one is.

### B236. `instanceof $variable` (a class-string held in a variable) narrowing is unsupported

**Impact: Medium · Complexity: Medium-High**

```php
/** @param class-string<Continue_>|class-string<Break_> $stmtClass */
function f(Node $statement, string $stmtClass): void
{
    if (!$statement instanceof $stmtClass) {
        continue;
    }
    $statement->num; // unknown_member: PHPantom never narrowed $statement at all
}
```

(`src/Analyser/StatementResult.php:95`,
`src/Analyser/InternalStatementResult.php:106`.) Only `instanceof` with
a literal class name appears to be handled; a variable holding a
`class-string<T>` as the right-hand side isn't matched at all, so the
guard has no effect. Likely home: `type_engine/resolver/`'s
`instanceof` handling.

### B237. A vendor interface's own `@phpstan-assert-if-true Type $this` tag isn't honored

**Impact: Medium · Complexity: Medium**

```php
// PHPUnit\Event\Code\Test, declared in the vendor package itself:
// @phpstan-assert-if-true TestMethod $this
public function isTestMethod(): bool;
```

```php
if (!$test->isTestMethod()) {
    return;
}
$test->className(); // unknown_member: $test not narrowed to TestMethod
```

(`src/Testing/PHPUnit/InitContainerBeforeTestSubscriber.php:23`.) Unlike
`PHPStan\Analyser\Scope::isInClass()` (B228's example), this tag isn't
missing from the vendor source — PHPUnit declares it directly on its
own interface — so this isn't the same gap `stub_patches.rs`'s
`THIRD_PARTY_ASSERT_IF_TRUE` list works around. The general
`@phpstan-assert-if-true` call-site mechanism (documented in
`docs/todo/type-inference.md`) doesn't appear to apply to a `$this`-target
tag declared on a class outside the project/vendor scan root, or
there's a narrower gap in how this specific interface gets its
docblock tags parsed; not fully root-caused.

## Arithmetic

No outstanding items.

## Symbol resolution

### B238. `new self(...)`/`new static(...)` resolves by short name, colliding with a same-named global class

**Impact: High · Complexity: Medium**

```php
namespace PHPStan\Analyser;

final class Error implements JsonSerializable
{
    public function __construct(private string $message, private ?int $line = null) {}

    public function changeFilePath(string $newFilePath): self
    {
        return new self($this->message, $this->line); // resolves to the built-in \Error!
    }
}
```

`type_engine/variable/rhs_resolution/instantiation.rs`
(`resolve_rhs_instantiation`, around line 26-33) resolves `self`/`static`
via `ctx.current_class.name.to_string()` — the class's *short* name —
instead of `ctx.current_class.fqn()`. Every other branch of the same
match resolves a written class name through
`crate::util::resolve_source_class_name` (namespace-aware). Since
`PHPStan\Analyser\Error` shares its short name with the built-in
`\Error`, `new self(...)` inside it misresolves to the global class,
producing false `type_mismatch_argument`/`type_mismatch_return` on
every constructor call and every method that returns `new self(...)`
(confirmed at `src/Analyser/Error.php` — 8 call sites × 2 argument
positions, plus 10 `type_mismatch_return` hits across its fluent
`changeXxx()` methods). Any user class whose short name collides with a
PHP built-in (or any other loaded class) is affected, not just this one.

### B239. `$this` inside a trait method resolves to the trait itself, not the using class

**Impact: High · Complexity: Medium-High**

```php
trait TemplateTypeTrait
{
    public function getTypeWithoutSubtractedType(): Type
    {
        // ...
        return $this; // Return type <TheTrait> is incompatible with declared return type Type
    }
}
```

(`src/Type/Generic/TemplateTypeTrait.php:140` and 20+ similar sites
across `src/Type/Traits/*.php`.) `$this` inside a trait method that
returns `$this` typed as an interface/abstract type the trait doesn't
itself implement should resolve to the class the trait is mixed into
(late static binding through traits), not to the trait as a
pseudo-class. This is the classic "trait `$this` not resolved" gap;
minimal repro confirms it reproduces with a bare `trait`/`use`/`return
$this` with no other machinery involved.

### B240. `static` return type on an intersection-typed receiver drops the non-declaring interface members

**Impact: Medium · Complexity: Medium**

```php
interface IfaceA { /** @return static */ public function filter(): self; }
interface IfaceB { public function ifaceBMethod(): void; }

function needsBoth(IfaceA&IfaceB $x): void {}

function test(IfaceA&IfaceB $scope): void
{
    $filtered = $scope->filter();
    needsBoth($filtered); // false: expects IfaceA&IfaceB, got IfaceA
}
```

Confirmed minimal repro, and live in phpstan-src at
`src/Rules/Methods/CallMethodsRule.php:55` /
`CallStaticMethodsRule.php:56` (`Scope&NodeCallbackInvoker&CollectedDataEmitter`,
narrowed via `$scope->filterByTruthyValue(...)` which is `@return
static`). `static` should resolve to the receiver's full statically-known
type — here the whole intersection — but instead resolves only to
whichever single interface declares the called method.

### B241. An abstract class isn't accepted where a union of exactly its known subclasses is declared

**Impact: Low · Complexity: High**

```php
// PhpParser\Node\Stmt\ClassLike is abstract; Class_/Interface_/Trait_/Enum_
// are its only subclasses in the known universe.
private function createAstClassReflection(Node\Stmt\ClassLike $stmt, ...): ClassReflection
{
    // ...
    $nodeToReflection->__invoke($this->reflector, $stmt, ...);
    // $node param declared: Class_|Interface_|Trait_|Enum_|Function_|Closure|ArrowFunction|Const_|FuncCall
    // false: ClassLike does not satisfy that union
}
```

(`src/Analyser/NodeScopeResolver.php:2794` and three
`BetterReflection/SourceLocator/*.php` sites.) Real PHPStan is silent —
it appears to reason that `ClassLike`'s only subclasses are exactly the
four named in the union, so a `ClassLike`-typed value is exhaustively
covered. Reproducing this "sealed hierarchy" exhaustiveness check would
need enumerating every known subclass of an abstract type across the
whole project + vendor at check time; rare enough in practice
(`PhpParser\Node\Stmt\ClassLike` may be the only real-world instance
seen so far) that it's filed for completeness rather than urgency.

### B242. `Composer\Autoload\ClassLoader` is invisible to class resolution

**Impact: Medium · Complexity: Medium**

Every Composer-managed project has this class available via
`vendor/composer/ClassLoader.php`, but it's loaded through a
hand-written `spl_autoload_register` bootstrap in
`vendor/composer/autoload_real.php` (`loadClassLoader()` →
`require __DIR__ . '/ClassLoader.php'`), not through
`autoload_classmap.php` or any package's `psr-4`/`psr-0` map. The vendor
classmap scanner (`classmap_scanner/`, `composer.rs`) has no special
case for this universal bootstrap file, so `Composer\Autoload\ClassLoader`
resolves as `unknown_class` on any project that references it directly
— a fairly common pattern for code that introspects its own autoloader,
as phpstan-src does at `src/Testing/TestCaseSourceLocatorFactory.php:55,56`
and `src/autoloadFunctions.php:62,73`. Since `vendor/composer/ClassLoader.php`
is present verbatim in every Composer install regardless of declared
dependencies, this would recur on any Composer project, not just this
one.

### B243. Two more confirmed `unknown_member` false positives with an unidentified mechanism

**Impact: Low-Medium · Complexity: Unknown**

Two additional confirmed-real false positives surfaced during the
`unknown_member` sweep whose exact cause wasn't pinned down:

- `Nette\Neon\Exception::getMessage()` not found
  (`src/DependencyInjection/NeonAdapter.php:57`,
  `NeonCachedFileReader.php:45`), despite the class extending
  `\Exception` — an identical hand-written class in isolation resolves
  fine, so it's specific to something about this vendor path (possibly
  a classmap/PSR-4 interaction in a monorepo with many nested
  `composer.json` files), not `extends \Exception` in general.
- `PHPStan\Analyser\Scope::mergeWith()` not found
  (`src/Analyser/NodeScopeResolver.php:5404,5412` and 6 similar lines).
  `Scope` genuinely has no `mergeWith` (only `MutatingScope` does), and
  PHPantom's own `dumpType`-equivalent confirms it believes the
  expression's static type is bare `Scope` — yet real PHPStan raises no
  error on the call. No stub, mixin, or reflection extension explaining
  why real PHPStan accepts this was found; filed as a confirmed false
  positive without a full explanation of the real-PHPStan side.

### B218. `new ReflectionProperty(Foo::class, 'bar')` forgets what it reflects

**Impact: Low · Complexity: Medium**

A reflection value built by `ReflectionClass::getProperty('bar')` carries
the class and the property name, so reading it types as the property
declares. Constructing the same value directly does not:

```php
$viaClass = (new \ReflectionClass(Configuration::class))->getProperty('shell');
$viaClass->getValue($config);   // ?Shell

$direct = new \ReflectionProperty(Configuration::class, 'shell');
$direct->getValue($config);     // mixed
```

The two spellings are interchangeable in real code, so the second should
resolve like the first. The binding cannot come from the constructor's
docblock: `class-string<T>|T $class` would bind the class through the
existing machinery, but the `$property` name is a string literal, and a
literal only binds to a `@template` whose bound is a type operator
(`key-of<…>` and friends). Either the two `new`-expression resolution
paths need the same rule the two call paths got, or literal binding has
to be widened to a `@template TName of string`, which is what PHPStan
does for literal string types and would want measuring against the whole
corpus first.

### B226. A function-`static` variable's type is not tracked across its own reads

**Impact: Low-Medium · Complexity: Medium-High**

`type_engine/variable/forward_walk/` has no handling at all for a
`static $var;` declaration (there is no `StaticVariable` case anywhere
under it); the walker treats the name as an ordinary, unassigned local
until it sees an assignment to it in the same top-to-bottom pass. That
loses the one thing a `static` local actually means: its value can carry
over from an *earlier call* that assigned it in a branch the current
call never reaches.

```php
function info(?Configuration $config = null) {
    static $lastConfig;
    if ($config !== null) {
        $lastConfig = $config;
        return null;
    }
    $config = $lastConfig ?: new Configuration();
    // $shell::VERSION below needs $config resolved to Configuration for
    // Sudo::fetchProperty($config, 'shell') to type as ?Shell (the
    // pass-through accessor inference already handles that part).
    $shell = Sudo::fetchProperty($config, 'shell');
    if ($shell) {
        $shellInfo = ['PsySH version' => $shell::VERSION];
    }
}
```

On the call that falls through to the second half, `$lastConfig` is read
without ever having been assigned within *this* walk, so `$config`
resolves too conservatively for the accessor pass-through (see the
`ReflectionProperty`/`Sudo::fetchProperty` inference above) to carry
`Configuration::$shell`'s declared type through to `$shell`, and
`$shell::VERSION` cannot be resolved. Found via
`php-typing-conformance`'s LSP navigation probe against psysh
(`Psy\Shell::VERSION`, `src/functions.php:383`): find-references reports
20 of 21 known references, missing exactly this one; Intelephense and
Phpactor miss the same reference, but DEVSENSE resolves it, which is
worth chasing. The same gap also degrades hover and inferred types
wherever code narrows on a `static` local this way, not just
find-references. A correct fix needs to seed a
`static $var`'s type from the union of every assignment reachable
anywhere in the enclosing function body (not only the ones preceding the
read in this pass), since the assignment that matters can sit in a
branch this call never takes.

## Array types

### B246. `array_keys()` on an array whose keys the `+` operator merged

**Impact: Low · Complexity: Medium**

```php
/** @param array<string, Holder> $a
 *  @param array<string, Holder> $b */
function f(array $a, array $b): void
{
    foreach (array_keys($a + $b) as $key) {
        // $key: array-key, should be string
    }
}
```

(`src/Analyser/VolatileExpressionHelper.php:94-98`.) Assigning the same
expression first (`$c = $a + $b;`) and calling `array_keys($c)` narrows
correctly, so the array-union operator itself is understood. The gap is in
the *text*-based argument resolver
(`resolve_arg_iterable_raw_type` / `Backend::resolve_arg_text_to_type` in
`type_engine/variable/rhs_resolution/calls.rs` and
`type_engine/call_resolution/`): it reads an argument written as source
text, and `resolve_operator_type` there knows `.` and `?:` but not `+`,
while the class-walk resolver behind it only reports class-backed results
and so answers nothing for an array. Splitting `+` in that text resolver
would duplicate what the AST walker already computes correctly, so the
real fix is to give the argument path access to the walker's answer
rather than to teach the text path a second set of operator rules.

### B255. An array shape key spelled with a backslash widens the shape's key type to `int|string`

**Impact: Low · Complexity: Medium**

```php
$replacements = ['~\n~' => '|n', '~\r~' => '|r'];
foreach (array_keys($replacements) as $key) {
    // $key: int|string, should be string
}
```

(`src/Command/ErrorFormatter/TeamcityErrorFormatter.php:121-126`.)
`iterable_key_type` in `php_type/mod.rs` widens any shape key containing a
backslash to `int|string`, because the parser stores a shape key in its
*escape spelling* rather than its decoded runtime value, and a
double-quoted `"\x38"` really does decode to the integer key `8`. A
single-quoted key decodes nothing of the sort, so the widening is wrong
for it, but the stored key no longer records which quote style it came
from. The fix is to decode the key at parse time (or record the quote
style alongside it) so `is_decimal_int_array_key` can be asked about the
runtime key, after which the backslash special case can go entirely.

### B256. `instanceof` does not narrow an array element (`$types[$i]`)

**Impact: Medium-High · Complexity: Medium**

```php
/** @param list<Ty> $types */
function f(array $types): void
{
    for ($i = 0; $i < count($types); $i++) {
        if ($types[$i] instanceof ShapeTy) {
            echo $types[$i]->propName(); // false unknown_member on Ty
        }
    }
}
```

The narrowing engine keys a narrowed type by its subject, and an
array-dim-fetch subject is not one of them: the guard is recorded but
never read back, so the element keeps the array's declared element type
inside the branch. Narrowing the same value through a local
(`$t = $types[$i]; if ($t instanceof ShapeTy)`) or a `foreach` binding
works, which is the shape most code takes and why this stayed hidden.
It surfaced as ~17 false `unknown_member` reports across
`src/Type/TypeCombinator.php:1791-2063` the moment the elements of
`$types` resolved to a real union instead of to nothing. PHPStan and
Psalm both narrow a constant offset this way.

### B257. A closure parameter binds the whole container when the argument is a union of array shapes

**Impact: Medium · Complexity: Medium**

```php
$expected = $cond ? [self::TOKEN_A] : [self::TOKEN_A, self::TOKEN_B];
array_map(fn ($token) => $this->lexer->getLabel($token), $expected);
// $token: array{3}|array{4, 2}, should be int
```

(`src/Parser/RichParser.php:311-316`.) The `ArrayElement` template binding
in `build_function_template_subs`
(`type_engine/variable/rhs_resolution/calls.rs`) reads the element type
with `extract_value_type`, which answers nothing for a *union* of
`array{…}` shapes; the `!resolved_type.is_array_like()` fallback beside it
then binds the template to the container itself, so the callback's
parameter is typed as the array rather than as one of its elements. Two
things need fixing: `extract_value_type` should join the element types of
a shape union the way `iterable_element_type` already does, and the
fallback should decline rather than bind a container it could not read an
element out of.

## Docblock handling

No outstanding items.

## Miscellaneous

### B251. A closure's by-reference `use` mutation isn't tracked when the closure is invoked indirectly

**Impact: Medium · Complexity: Medium-High**

```php
$constructorResult = null;
$nodeScopeResolver->processStmtNode($expr->class, $scope, $storage, new GatheringNodeCallback(
    static function (Node $node, ...) use (&$constructorResult): void {
        $constructorResult = $node;
    },
    $nodeCallback,
), ...);
if ($constructorResult !== null) {
    $constructorResult->getStatementResult(); // still typed null past the guard
```

(`src/Analyser/ExprHandler/NewHandler.php:153,154`.) A directly-invoked
`use (&$x)` closure (`$callback($arg)`) is tracked fine; wrapping the
same closure in an object and invoking it through a separate method
call (mirroring `GatheringNodeCallback`'s `invoke()`) loses the
reference mutation entirely, so `$constructorResult`'s type never
updates and stays "null" even past the `!== null` guard.

### B252. `??=` doesn't route its RHS through the callable-property-invocation resolution path

**Impact: Medium · Complexity: Medium**

```php
/** @var callable(): MutatingScope $leftTruthyScope */
private $leftTruthyScope;

$leftTruthyScope = null;
$leftTruthyScope ??= ($this->leftTruthyScope)();
$leftTruthyScope->getType($targetExpr); // false: method call on null
```

(`src/Analyser/DisjunctionHolderProjectionAugment.php:78,84`.) Isolated
down to the exact failing ingredient: a direct call `($this->prop)()`
alone resolves fine, `$x ??= new Foo()` alone resolves fine, only
`$x ??= ($this->callableProp)()` fails — the `??=` handler doesn't
reuse the same resolution path a bare invocation of a callable-typed
property already gets right.

### B253. A closure parameter's type isn't inferred from a `@template`-parameterized `callable(T): X` signature

**Impact: Medium-High · Complexity: High**

```php
/** @template T @param T $node @param callable(T): void $fn */
function fixNode($node, callable $fn): void { $fn($node); }

fixNode($property, function ($property) {
    $property->isReadOnly(); // $property's inferred type is unresolved/mismatched, not the bound T
});
```

When a closure is passed to a method whose docblock is
`@param callable(T): X $fn` with `T` bound from the call-site argument,
real PHPStan infers the closure's own parameter type from `T` (even
overriding a wider explicit hint); PHPantom leaves it as the literal
template name or the declared hint. Explains every `unknown_member`
"subject type 'TNode'/'TValue' could not be resolved" hit:
`src/Parser/LastConditionVisitor.php:91`,
`src/Rules/Properties/OverridingPropertyRule.php:68,104`,
`build/PHPStan/Build/NamedArgumentsRule.php:183,232`,
`build/PHPStan/Build/FinalClassRule.php:77`,
`src/Reflection/Type/IntersectionTypeMethodReflection.php:240`. Likely
home: `type_engine/`'s generic/template substitution for
callable-type parameters.

### B254. Unused-variable check doesn't account for a closure's `use()` capture feeding a `require`d file's scope

**Impact: Low · Complexity: Medium**

```php
(static function (string $file) use ($container): void {
    require_once $file; // the required file executes in this closure's own variable scope
})($file);
```

(`src/Command/CommandHelper.php:642`,
`src/Testing/PHPStanTestCaseTrait.php:49` — both flag `$container` as
unused.) `$container` is never referenced *by name* inside the closure
body, but `require_once` runs the target file's code inside the
closure's own scope, so the required file can legitimately read
`$container` as a local variable — a known idiom for exposing a
variable to dynamically-included code without leaking the rest of the
enclosing scope. The `unused_variable` check has no notion of "used via
a same-scope `require`/`include`" at all. (Neither bundled bootstrap
file in phpstan-src currently reads `$container` back, so these two
specific instances may or may not be live in practice — but the
checker's blind spot is real regardless of this instance's outcome.)
