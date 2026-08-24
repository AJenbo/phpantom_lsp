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

### B258. Two variables assigned in the same branch lose their correlated nullability at the merge

**Impact: Medium · Complexity: High**

```php
$acceptor = null;
$reflection = null;
if ($name !== '') {
    $reflection = $this->find($name);
    if ($reflection !== null) {
        $acceptor = $this->select($reflection);   // non-null exactly when $reflection is
    }
}

if ($reflection !== null) {
    $this->useBoth($reflection, $acceptor);       // false type_mismatch_argument on $acceptor
}
```

The merge keeps each variable's own union (`?Reflection`, `?Acceptor`)
and forgets that the two were written on the same path, so the later
`$reflection !== null` check cannot recover what it implies about
`$acceptor`. Real PHPStan is silent on the shape as its own source
writes it, and it is not in its baseline, so it tracks the correlation
somehow — the mechanism is not root-caused here.

Reproduced standalone. Real-world hits are the `$parametersAcceptor`
cluster in `src/Analyser/ExprHandler/` (`FuncCallHandler.php:977,1042`,
`MethodCallHandler.php:169,350`, `StaticCallHandler.php:240,455`), where
the acceptor is built in the same branch that resolves the method
reflection the later check tests.

## Arithmetic

No outstanding items.

## Symbol resolution

### B243. `PHPStan\Analyser\Scope::mergeWith()` is a confirmed false positive with no explanation on the PHPStan side

**Impact: Low-Medium · Complexity: Unknown**

`Scope` genuinely has no `mergeWith` (only `MutatingScope` does), and the
expression `NodeScopeResolver.php:5404,5412` calls it on is
`StatementResult::getScope()`, declared `: Scope`. PHPantom's own
`dumpType`-equivalent agrees, and the same file spells the conversion out
explicitly 4,000 lines earlier
(`$statement->getScope()->toMutatingScope()` at line 1112), so reading the
source alone says the call is an error and the diagnostic is right. Yet
real PHPStan raises nothing, and no entry for it exists in
`phpstan-baseline.neon`. No stub, mixin, or reflection extension
explaining the silence was found, and running PHPStan on itself to settle
it needs a `composer install` in the checkout. Filed as a confirmed false
positive whose cause is still unaccounted for; the eight call sites are
the only ones seen anywhere.

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
