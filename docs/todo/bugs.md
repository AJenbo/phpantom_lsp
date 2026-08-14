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

Most entries below come from the 2026-08-13 sample-project sweep (345
diagnostics across ten projects, ~330 of them false positives). Site
counts refer to that sweep; the git-ignored triage log has the full
per-project inventory. Entries filed later say where they came from.

## Conditional and argument-dependent return types

### B176. A `@template` bound through a union `@param` never binds

**Impact: Low-Medium · Effort: Medium**

A template parameter is bound from the argument whose `@param` names
it, but only when the annotation names it plainly (`@param T $x`),
inside an array (`@param T[] $x`), or as one generic wrapper
(`@param array<TKey, TValue> $x`). A *union* of shapes binds nothing:

```php
/**
 * @param Collection<TKey, TValue>|EloquentCollection<TKey, TValue>|array<TKey, TValue> $items
 * @return array<TKey, TValue>
 */
function pick($items) {}

pick($rows);   // array<int, string> in, reported array<array-key, mixed>
```

Each alternative is a binding site of its own, and the one whose shape
the argument matches is the one that should bind. spatie's
`Data::collect()` is the case that surfaced it: the branch its
arguments select is right, but its key type falls back to the declared
`array-key` bound. A method binds the *whole* argument type instead,
which is worse: `array<TKey, …>` with a `list<string>` argument reports
`array<array<int, string>, …>`.

**Fix:** in `classify_template_binding`, treat a union `@param` as the
set of binding sites it names and match the argument against each,
binding from the alternative it satisfies.

### B141. A `never` conditional branch does not assert the condition

**Impact: Medium-High · Effort: Medium**

```php
/** @return ($condition is false ? never : ($condition is non-empty-mixed ? TValue : never)) */
function throw_unless($condition, ...$args) {}

$dispatcher = Model::getEventDispatcher();      // Dispatcher|null
throw_unless($dispatcher, 'Exception', '…');
Model::setEventDispatcher($dispatcher);         // reported Dispatcher|null
```

When a call's conditional return resolves to `never` for some subtype
of an argument, that subtype cannot survive the call; PHPStan derives
an implicit assertion from it. PHPantom keeps the argument unchanged,
so every `throw_unless`/`throw_if`/`abort_unless` guard is invisible
(7 sites in one test file alone).

**Fix:** when the branch selected by a *falsy/truthy
subtype* of an argument is `never`, subtract that subtype from the
argument expression in the following scope — the same subtraction the
`if (!$x) { throw … }` form already gets.

### B142. Builtins with argument-dependent return types, round two

**Impact: Low-Medium · Effort: Medium**

The T38 work covered the replace family and `json_encode`; the
conditional-signature round covered `pathinfo`, `print_r`, `hrtime`,
`microtime`, `getenv`, `mb_convert_encoding`, `abs` and
`SimpleXMLElement::asXML()`/`saveXML()`. What is left needs the
*element* type of an array argument rather than the argument's own
category, which the conditional evaluator cannot express (`array<int>`
and `array<string>` both read as "array"):

- `array_sum(array<int>)` is `int` (`array_product` likewise). 2 sites.

**Fix:** a per-function rule in
`type_engine/variable/array_func_rules.rs`, which already has the
argument's raw type at hand. Blocked behind the same
`resolve_arg_raw_type` gap as [B146](#b146-array-builtins-lose-key-and-element-generics).

`ReflectionClass<T>::newInstance()`/`newInstanceArgs()`/
`newInstanceWithoutConstructor()` were listed here too but do not
reproduce: all three substitute `T` when the receiver carries a type
argument, and `object` is the honest answer for a bare
`ReflectionClass`.

### B143. Conditional-return arguments written as expressions still read as nothing

**Impact: Medium · Effort: Medium**

The successor to B124. The replace-family conditional keys on
`$subject`, but the subject's type is only read for simple argument
shapes, so `str_replace(NS, '', $class)` on a `string` parameter, or
`preg_replace($pats, $reps, file_get_contents($f) ?: '')`, still
returns `array|string` (5 sites). Same story for flag arguments built
from expressions: `json_encode($v, $options | self::FLAGS)` and
`json_encode($v, $encodeOptions)` where the local was assigned a
constant `|` chain never strip `false` even though the
`JSON_THROW_ON_ERROR` bit is provably set (3 sites).

**Fix:** resolve conditional-return argument types through the shared
`resolve_expression_type` pipeline rather than a call-site text reader,
and evaluate flag bits against an integer range ("bit definitely set")
instead of requiring a literal constant.

## preg_match

### B144. `preg_match` `$matches` is nullable and shapeless

**Impact: High · Effort: Medium**

The single largest false-positive source of the sweep (~40 sites in
seven projects):

```php
if (preg_match('/(?<amount>\d+)(?<unit>\w*)/', $size, $match)) {
    strtolower($match['unit']);   // reported: got null|string
}
```

Three compounding defects:

1. The stub's `?array &$matches = null` default leaks into the
   written-back type, so `$matches` is `array<string>|null` after the
   call and every offset read yields `string|null`. After a truthy
   `preg_match` (and unconditionally after `preg_match_all`),
   `$matches` is definitely an array.
2. No match-shape inference for literal patterns: PHPStan's
   `RegexArrayShapeMatcher` types group 0 and every always-matching
   group as `string` (`string|null` only under
   `PREG_UNMATCHED_AS_NULL`), with named groups as keys. PDepend,
   PHPMD, AGCMS and Bladestan all index `$match[1]`/`$match['name']`
   directly inside the guard.
3. `preg_match_all` group reads (`$matches[1]`) should be
   `list<string>`, not `array<string>|null`.

**Fix:** treat the by-ref out-parameter as definitely-assigned
non-null after the call; add a literal-pattern group-shape analysis
(port the capture-group walk from PHPStan's `RegexArrayShapeMatcher`).

## Array types

### B146. Array builtins lose key and element generics

**Impact: High · Effort: Medium**

~20 sites across six projects, all the same underlying gap — the
signature's `int[]|string[]`/`array-key` placeholders are returned
verbatim instead of substituting the input's generics:

- `array_keys(array<K, V>)` → `list<K>` (10 sites; the stub-literal
  `array<int>|array<string>` union then fails against
  `array<string>`/`list<int>` on the wrong branch).
- `array_search($needle, array<K, V>)` → `K|false`; `key()`,
  `array_key_first/last(array<K, V>)` → `K|null`.
- `array_values(array<K, V>)` → `list<V>`.
- `array_filter`/`array_map` (single-array form) preserve the key
  type; `array_filter` with no callback also strips falsy members
  from the value type (`array<string, string|null>` →
  `array<string, non-falsy-string>`).
- `array_flip(array<K, V>)` → `array<V, K>`.

**Fix:** the rules themselves are not the gap — `array_func_rules.rs`
already computes `array_values(array<string, int>) → array<string, int>`
correctly, and `array_flip` resolves through the stub's own `@template`.
The gap is that on the forward-walker path `resolve_arg_raw_type`
returns `None` for a docblock-typed parameter, so the rule never fires
and the stub's bare `array` wins; only the text-driven call resolver
reaches it. Fix that first, then add the per-function rules above
(`array_values` → `list<V>` rather than the input type it preserves
today, plus `array_keys`, `array_search`, `key`,
`array_key_first`/`last`, and `array_filter`'s falsy strip).

### B147. Array literals are not tuples: slot reads return the union of all elements

**Impact: Medium-High · Effort: Medium**

```php
$rows[] = [$violation, $location, $name];        // RuleViolation, string, string
foreach ($rows as $row) {
    [$violation, $location, $name] = $row;       // each: RuleViolation|string
    $writer->write($location);                   // reported: RuleViolation|string
}
```

A list literal collapses to `array<union-of-values>`, so list
destructuring and constant-offset reads cannot select a slot (6 sites
in PHPMD/PDepend). Two adjacent literal defects: a literal with a
*non-constant* key renders as the bogus shape `array{mixed: int}`
(stringifying the key's type as a field name) instead of falling back
to `array<K, V>`, and `(object) []` is not recognised as `stdClass`.

**Fix:** keep constant-array shapes for literals (ordered slots +
known keys), select slots on destructure/offset reads, fall back to a
generic array only for non-constant keys.

### B148. Element writes do not refine tracked array state

**Impact: Medium · Effort: Medium-High**

Several forms of the same weakness (~7 sites):

- `$a[$k][] = $v` never updates the inner element type: a value
  initialised as `[]` stays `array{}` in the outgoing type even
  though every loop iteration appends strings.
- A key written on every path through a loop body leaves
  `array<int, string>` where PHPStan reports
  `non-empty-array<int, string>`.
- `$a += ['slot' => $obj]` degrades to unconstrained `array`.
- A constant shape `array{item: string, qty: int}` fails the subtype
  check against `array<string, mixed>`, so shaped rows are rejected
  by a declared `array<int, array<string, mixed>>`.

**Fix:** refine the per-key state on nested writes (including
auto-vivification), merge `+=` like an array-shape union, and make
constant shapes satisfy their generic supertypes.

## Narrowing

### B150. Branch-local reassignment and narrowing are wrong at the join point

**Impact: Medium-High · Effort: Medium**

Two inverse defects at `if`/`else` merges (~5 sites):

- A reassignment inside a branch is *not* applied after the join:
  `if ($v instanceof AbstractNode) { $v = $v->getNode(); }` still
  carries `AbstractNode` afterwards; `if ($r instanceof User)
  { $r = $r->getToken(); }` still carries `User`.
- A narrowing *does* leak past the join: after
  `if ($r instanceof Verbose) { … }` the post-if type keeps the
  branch-narrowed member instead of re-merging to the declared type.

**Fix:** at the merge, each branch contributes its end-state (declared
type transformed by that branch's assignments/narrowings), and the
join is the union of branch end-states — nothing more, nothing less.

### B177. A branch-local proof about an untyped subject escapes the join

**Impact: Low-Medium · Effort: Medium**

```php
$version = $row->version;      // stdClass property: no type
if ($version instanceof Foo) { }
$version;                      // reported Foo, should stay untyped
```

Narrowing a subject the scope has no type for *establishes* that type
(this is what lets `assert($x instanceof Foo)` and
`if (!is_string($x)) { return; }` work at all), but the branch merge
cannot tell the resulting entry apart from a branch-local assignment:
a name absent on one incoming path and present on the other is adopted
wholesale, so the proof survives a join it should not. The negated form
leaks the same way, from the implicit-else path back into the body.

Harmless while the alternative is no information at all, and long-standing
— it predates the `instanceof`/type-guard narrowing work that made it
visible. Found while fixing the condition-proof handling, not by a sample
project.

**Fix:** distinguish "no entry" from "unknown" in `ScopeState` so the merge
can widen a narrowed-from-nothing entry back to unknown, rather than
adopting it. Overlaps [T29](type-inference.md#t29-definite-vs-possible-variable-existence-tracking),
which needs the same distinction for variable existence.

### B155. A checked call expression is forgotten by the next identical call

**Impact: Medium-High · Effort: Medium-High**

```php
if (mb_strpos($slug, $marker) !== false) {
    $slug = mb_substr($slug, 0, mb_strpos($slug, $marker));  // reported int|false
}
$from = $this->option('from') ? Carbon::parse($this->option('from')) : null;
```

Narrowing is keyed on variables only; a structurally identical
side-effect-free call repeated inside the guarded scope re-resolves
from scratch (~6 sites). PHPStan keys `SpecifiedTypes` on the printed
expression and invalidates on side effects; Phpactor caches by node
identity.

**Fix:** key the narrowing store by a canonical expression form
(receiver chain + arguments) for deterministic/pure calls, and
invalidate entries when a statement could change their inputs.

### B157. `@phpstan-assert` tags on called methods are ignored

**Impact: Medium-High · Effort: Medium**

PHPUnit's assertions declare their effect in phpDoc
(`@phpstan-assert resource $actual` on `assertIsResource()`,
`@psalm-assert =ExpectedType $actual` on `assertInstanceOf()`, and so
on). PHPantom honours several assertions (`assertNotFalse`,
`assertIsArray`, and `assertNotNull` all narrow correctly) but the
coverage comes from hard-coded knowledge, not the tags: in the same
sweep `assertIsResource` and `assertInstanceOf` (mock intersections)
left their argument untouched (~6 sites in test suites).

**Fix:** read `@phpstan-assert` / `@phpstan-assert-if-true` /
`@phpstan-assert-if-false` (and the `@psalm-assert` aliases,
including the `=` exact-type prefix) from called
functions/methods and apply them as type specifications; drop the
hard-coded PHPUnit list in favour of the tags.

### B158. Strict `in_array` against a constant list does not narrow the needle

**Impact: Low-Medium · Effort: Medium**

`if (!in_array($user->getEmail(), self::APPROVED, true)) { abort(403); }`
proves on the fall-through that the needle is one of the constant
list's literals (⊆ `string`), removing `null`. Requires B155's
expression keying for method-call needles. 2 sites.

### B159. An inline `@var` re-pins the variable on every read

**Impact: Medium · Effort: Medium**

```php
/** @var null|list<array{…}> $cached */
$cached = Cache::get(self::KEY);
if ($cached !== null) {
    return array_slice($cached, 0, $limit);   // reported null|list<…>
}
```

A plain `!== null` guard that works on any ordinary variable does
nothing here — and a later reassignment (`$x = []; … $x = narrow();`)
is also overridden by the annotation. The `@var` should seed the
assignment it documents and then submit to normal flow narrowing
(3 sites).

### B174. A `break` that leaves a loop early is missing from the post-loop join

**Impact: Medium · Effort: Medium**

```php
$a = 'x';
do {
    if (rand(0, 1)) { break; }   // leaves with $a === 'x'
    $a = 1;
} while (rand(0, 1));
$a;                              // reported 1, should be 'x'|1
```

The state a `break` carries out of a loop does not reach the post-loop
merge, so only the paths that ran to the end of the body contribute. The
inverse shows up in a nested loop, where a `break` out of the *inner*
loop loses the assignment it made: `foreach { foreach { … $b = 1;
break; } } $b;` reports the pre-loop value alone. Both forms are visible
in Psalm's `falseToBool` loop tests (the three `// SKIP` assertions in
`tests/psalm_assertions/loop_do.php` and `loop_foreach.php`), which were
only passing while every boolean widened to `bool` and made the merge
result the same either way.

**Fix:** record each `break`'s scope state as an exit edge of the loop it
leaves, and union those edges into the post-loop scope alongside the
normal fall-through.

## Laravel

### B165. `__()` / `trans()` report their raw `string|array|null` signature

**Impact: High · Effort: Medium**

27 sites in one project: every `{{ __('key') }}` echo, `__()` fed to
an `HtmlString`, and `assertSee(__('key'))` mismatches because the
helper's framework signature is `string|array|null`. PHPantom already
indexes translation keys and their shapes (`trans_keys.rs`); a
literal-key call can resolve to the actual translation's type
(`string` for a leaf, `array<…>` for a group), which is strictly
better than larastan's benevolent-union hand-wave. A keyless `__()`
returns `null`; an unresolvable dynamic key should stay permissive
(treat as accepting-any-branch rather than reporting every branch).

### B166. Console `argument()` / `option()` ignore the command's `$signature`

**Impact: High · Effort: Medium**

29 sites in one project. `$this->argument('name')` /
`$this->option('markets')` return the framework's raw
`array|string|float|int|bool|null` union. The command's own
`$signature` string decides the real type per entry: a value-less
`{--flag}` is `bool`, `{--opt=}` is `string|null`, `{arg}` is
`string`, `{arg*}` is `array<string>`. PHPantom already parses
signature strings for command-name indexing; extend that to type
these two accessors (Symfony `InputDefinition` semantics).

### B167. Factory `create()`/`make()` keep the collection half on single-model chains

**Impact: Medium-High · Effort: Low-Medium**

The shipped factory-count narrowing misses chains that pass through
`for()`/`has()`/`state()` (`$factory->for($brand)->create()` reports
`ProductCollection|Product`) and the count-argument static form
(`Product::factory($n)->create()` should be pure
`Collection<int, Product>`). Track the single-vs-collection state
across the whole fluent chain, in both directions. 8 sites.

### B168. `Request` input accessors deserve precise per-argument types

**Impact: Medium-High · Effort: Medium**

`$request->header('User-Agent', '')` reports `string|array|null`
against a `string` parameter — with a string default the real type is
`string`. `query()` with no key returns the full `array`;
`file('key')` is `UploadedFile|null` for a scalar key (larastan wraps
it in a benevolent union; resolving on the key/default is more
precise). Model the conditional shapes natively for
`header`/`query`/`input`/`cookie`/`file`. 5 sites. (Unguarded
`query('x')` uses that really can receive arrays were patched in the
sample sources as genuine.)

### B169. Blade `@if` narrowing does not reach `@include`d variables

**Impact: Medium-High · Effort: Medium**

```blade
@if ($product)
    @include('products.edit.page-links-product')   {{-- expects non-null $product --}}
@endif
```

An inherited variable is checked against the template's *declared*
signature type at the `@include` site, ignoring the enclosing Blade
control flow that provably narrows it (4 sites in two projects). Use
the flow-narrowed scope at the include position — the compiled
virtual PHP already contains the real `if`, so the walker has the
narrowing; it is the include-contract check that reads the wrong
scope.

## Miscellaneous

### B173. Classes shipped inside a dependency's phar are invisible

**Impact: Low-Medium · Effort: Medium**

A project extending PHPStan (`vendor/phpstan/phpstan` ships
`phpstan.phar` + stub headers) gets `mixed` for every
`PHPStan\Type\*` value, cascading into downstream false positives
(1 site — a custom extension calling `getConstantStrings()`). Check
what the package's `bootstrap.php` exposes and whether indexing the
phar (or its extracted stubs) is feasible.

### B177. A `@property` tag name is highlighted as a method

**Impact: Low · Effort: Low**

The member name in a `@property` tag emits the `method` semantic
token, while the same name emits `property` everywhere it is used:

```php
/**
 * @property string $displayName    <- `displayName` highlights as a method
 * @method string shout(string $x)  <- correct
 */
class Demo
{
    public function demo(): string
    {
        return $this->displayName;  // highlights as a property, as it should
    }
}
```

Both tags produce a `MemberDeclaration` span, and
`classify_member_declaration` looks the name up in the enclosing
class's own members. A tag-declared member is not there, so both fall
through to the `TT_METHOD` default. Classify the declaration from the
tag that produced it (or from the resolved virtual members) instead of
the fallback. `SemanticMagicMemberDemo` in
`examples/php/semantic_tokens.php` shows the case.
