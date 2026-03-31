# PHPantom — Type Inference

Type resolution gaps: generic resolution, conditional return types,
type narrowing, PHP version features, and stub attribute handling.
Items that are purely about *completion UX* or *stub metadata
extraction* live in [completion.md](completion.md).

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## T2. File system watching for vendor and project changes
**Impact: Medium-High · Effort: Medium**

PHPantom loads Composer artifacts (classmap, PSR-4 mappings, autoload
files) once during `initialized` and caches them for the session. If
the user runs `composer update`, `composer require`, or `composer remove`
while the editor is open, the cached data goes stale. The user gets
completions and go-to-definition based on the old package versions
until they restart the editor.

### What to watch

| Path | Trigger | Action |
|---|---|---|
| `vendor/composer/autoload_classmap.php` | Changed | Reload classmap |
| `vendor/composer/autoload_psr4.php` | Changed | Reload PSR-4 mappings |
| `vendor/composer/autoload_files.php` | Changed | Re-scan autoload files for global functions/constants |
| `composer.json` | Changed | Reload project PSR-4 prefixes, re-check vendor dir |
| `composer.lock` | Changed | Good secondary signal that packages changed |

All three `autoload_*.php` files are rewritten atomically by Composer
on every `install`, `update`, `require`, `remove`, and `dump-autoload`.
Watching these is sufficient to catch any package change.

### Implementation options

1. **LSP `workspace/didChangeWatchedFiles`** — register file watchers
   via `client/registerCapability` during `initialized`. The editor
   handles the OS-level watching and sends notifications. This is the
   cleanest approach and works cross-platform. Register glob patterns
   for the vendor Composer files and `composer.json`.

2. **Server-side `notify` crate** — use the `notify` Rust crate to
   watch the file system directly. More control but adds a dependency
   and duplicates what the editor already provides.

Option 1 is preferred. The LSP spec's `DidChangeWatchedFilesRegistrationOptions`
supports glob patterns like `**/vendor/composer/autoload_*.php`.

### Reload strategy

- On change notification, re-run the same parsing logic from
  `initialized` for the affected artifact.
- Invalidate `class_index` entries that came from vendor files (their
  parsed AST may have changed).
- Clear and re-populate `classmap` from the new `autoload_classmap.php`.
- Log the reload to the output panel so the user knows it happened.
- Debounce rapid changes (Composer writes multiple files in sequence)
  with a short delay (e.g. 500ms) to avoid redundant reloads.

### `textDocument/didSave` handler

PHPantom does not currently implement `textDocument/didSave`. This
means changes to files that are not open in the editor (e.g. files
saved by a script, a git checkout, or another tool) are invisible
until the file is opened. This is standard behaviour for most LSPs,
but it matters for the file-watching story: even after
`workspace/didChangeWatchedFiles` is wired up for Composer artifacts,
changes to user PHP files made outside the editor (e.g. code
generation, `artisan make:model`) will not be picked up until the
file is opened.

When file system watching is implemented, consider also registering
a `didSave` handler (or a broad `*.php` watcher) to trigger a
targeted single-file rescan for files in PSR-4 directories, matching
the plan described in [indexing.md Phase 2](indexing.md#phase-2-staleness-detection-and-auto-refresh).

---

## T3. Property hooks (PHP 8.4)
**Impact: Medium · Effort: Medium**

PHP 8.4 introduced property hooks (`get` / `set`):

```php
class User {
    public string $name {
        get => strtoupper($this->name);
        set => trim($value);
    }
}
```

The mago parser (v1.8) already produces `Property::Hooked` and
`PropertyHook` AST nodes, and the generic `.modifiers()`, `.hint()`,
`.variables()` methods mean hooked properties are extracted for basic
completion. However:

- **Hook bodies are never walked.** Variables and anonymous classes
  inside `get`/`set` bodies are invisible to resolution.
- **`$value` parameter** inside `set` hooks is not offered for
  variable completion.
- **Asymmetric visibility** (`public private(set) string $name`) is
  not recognised — the `set` visibility is ignored, so filtering
  may incorrectly allow setting a property that should be
  write-restricted.

**Fix:** In `extract_class_like_members`, match `Property::Hooked`
explicitly, walk hook bodies for anonymous classes and variable
scopes, and parse the set-visibility modifier into a new
`set_visibility` field on `PropertyInfo`.

### Asymmetric visibility (also PHP 8.4 / 8.5)

Separate from hooks, PHP 8.4 allows asymmetric visibility on plain
and promoted properties. PHP 8.5 extended this to static properties.

```php
class Settings {
    public private(set) string $name;

    public function __construct(
        public protected(set) int $retries = 3,
    ) {}
}
```

PHPantom currently extracts a single `Visibility` per property.
Completion filtering uses this to decide whether a property should
appear. A `public private(set)` property should appear for reading
from outside the class but not for assignment contexts.

Add an optional `set_visibility: Option<Visibility>` to
`PropertyInfo`. Populate it from the AST modifier list (the parser
exposes the set-visibility keyword). Completion filtering does not
currently distinguish read vs write contexts, so the immediate fix
is just to store the value; context-aware filtering can follow later.

This shares the same `set_visibility` field as the hooked-property
fix above, so both should be implemented together.

---

## T4. Non-empty-* type narrowing and propagation
**Impact: Low-Medium · Effort: Low**

PHPStan tracks `non-empty-string` and `non-empty-array` through
built-in functions. These narrowings don't directly enable
class-based completion, but they improve hover type display and
would catch bugs if we add diagnostics. All three sub-items share
the same implementation pattern (function-name-triggered type
narrowing in conditions or return types) and should be implemented
together.

**String containment narrowing.** When `str_contains($haystack,
$needle)` appears in a condition and `$needle` is known to be a
non-empty string, narrow `$haystack` to `non-empty-string`. Same
for `str_starts_with`, `str_ends_with`, `strpos`, `strrpos`,
`stripos`, `strripos`, `strstr`, and the `mb_*` equivalents.
See `StrContainingTypeSpecifyingExtension` in PHPStan.

**Count narrowing.** `if (count($arr) > 0)` or
`if (count($arr) >= 1)` narrows `$arr` to `non-empty-array`.
PHPStan handles a full matrix of comparison operators and integer
range types against `count()` / `sizeof()` calls. See
`CountFunctionTypeSpecifyingExtension`.

**String function propagation.** Passing a `non-empty-string` to
`addslashes()`, `urlencode()`, `htmlspecialchars()`,
`escapeshellarg()`, `escapeshellcmd()`, `preg_quote()`,
`rawurlencode()`, or `rawurldecode()` should return
`non-empty-string`. See `NonEmptyStringFunctionsReturnTypeExtension`.

---

## T5. Fiber type resolution
**Impact: Low · Effort: Low**

`Generator<TKey, TValue, TSend, TReturn>` has dedicated support for
extracting each type parameter (value type for foreach, send type
for `$var = yield`, return type for `getReturn()`). `Fiber` has no
equivalent handling — `Fiber::start()`, `Fiber::resume()`, and
`Fiber::getReturn()` don't resolve their generic types.

PHP userland rarely annotates Fiber with generics (unlike Generator),
so this is low priority. If demand appears, the fix would mirror the
Generator extraction in `docblock/types.rs`.

---

## T6. `Closure::bind()` / `Closure::fromCallable()` return type preservation
**Impact: Low · Effort: Low-Medium**

Variables holding closure literals, arrow functions, and first-class
callables now resolve to the `Closure` class, so `$fn->bindTo()`,
`$fn->call()`, etc. offer completions.  The remaining gap is
*preserving the closure's callable signature* through `Closure::bind()`
and resolving `Closure::fromCallable('functionName')` to the actual
function's signature as a typed `Closure`.  This is relevant for DI
containers and middleware patterns but is a niche use case.

See `ClosureBindDynamicReturnTypeExtension` and
`ClosureFromCallableDynamicReturnTypeExtension` in PHPStan.

---

## T7. `key-of<T>` and `value-of<T>` resolution
**Impact: Medium · Effort: Medium**

PHPantom already parses `key-of<T>` and `value-of<T>` as type keywords
but does not resolve them to concrete types. When `T` is bound to a
concrete array type, these utility types should resolve:

- `value-of<array{a: string, b: int}>` → `string|int`
- `key-of<array{a: string, b: int}>` → `'a'|'b'`
- `value-of<array<string, User>>` → `User`
- `key-of<array<string, User>>` → `string`

These types commonly appear in PHPStan-typed libraries and in
`@template` constraints. For example:

```php
/**
 * @template T of array
 * @param T $array
 * @return value-of<T>
 */
function first(array $array): mixed;
```

**Implementation:** plug into the generic substitution pipeline in
`inheritance.rs` / `completion/types/resolution.rs`. After template
parameters are substituted with concrete types, detect `key-of<...>`
and `value-of<...>` wrappers and resolve them by inspecting the inner
type:

- If the inner type is an `array{...}` shape, collect the key or value
  types from the shape fields.
- If the inner type is `array<K, V>`, extract `K` or `V` directly.
- If the inner type is still an unresolved template parameter, leave
  it as-is (it may resolve later in the chain).

---



## T9. Dead-code elimination after `never`-returning calls
**Impact: Low · Effort: Low-Medium**

When a function or method has return type `never`, any code path that
calls it is guaranteed to terminate. Variables assigned before the
`never` call in a conditional branch should not have their type
polluted by the branch's assignments.

```php
$x = 'hello';
if (rand(0,1)) {
    $x = 'other';
    abort(); // returns never
}
$x; // should be "hello", not "hello"|"other"
```

Today PHPantom's branch-merging logic unions all branch assignments
regardless of whether the branch terminates. Recognising `never` as a
terminating statement (alongside `return`, `throw`, `die`, `exit`)
would fix this.

**Fixture to activate:**

- `type/never_return_type.fixture`

**phpactor ref:** `type/never.test`

---

## T10. Ternary expression as RHS of list destructuring
**Impact: Low · Effort: Low-Medium**

List destructuring (`[$a, $b] = expr`) resolves element types when
the RHS is a function call returning an array shape, or a simple
array literal. When the RHS is a ternary expression whose branches
are array literals or array-shape-returning calls, the resolver
doesn't drill into the branches to union the element types.

```php
[$a, $b] = $cond ? [new Foo(), new Bar()] : [new Bar(), new Foo()];
$a->  // should see Foo|Bar members
```

**Fixture to activate:**

- `assignment/list_destructuring_conditional.fixture`

**phpactor ref:** `assignment/list_assignment.test`

---

## T11. Nested list destructuring
**Impact: Low · Effort: Low-Medium**

Nested destructuring like `[[$one, $two]] = $source` is not resolved.
When the RHS has a type like `array{array{Foo, Bar}}`, the outer
destructuring peels the first dimension but the inner destructuring
doesn't resolve individual elements.

```php
/** @return array{array{Foo, Bar}} */
function getPair(): array { return [[new Foo(), new Bar()]]; }

[[$one, $two]] = getPair();
$one->  // should see Foo members
```

**Fixture to activate:**

- `assignment/nested_list_destructuring.fixture`

**phpactor ref:** `assignment/list_desconstruct_nested.test`

---

## T12. Intersection types flattened to unions by `type_strings_joined`
**Impact: Low-Medium · Effort: Low (after M4)**

`ResolvedType::type_strings_joined` joins all resolved type entries
with `|`. When a variable has an intersection type (`A&B`), the
resolution pipeline produces separate `ResolvedType` entries for each
part, and the join produces `A|B` instead of `A&B`.

This affects any consumer that reads the joined type string, including
hover display, extract function parameter types, and docblock
generation on extracted methods.

**Example:**

```php
function measure(Countable&Serializable $thing): void {
    // Select and extract:
    echo $thing->count();
}
// Extracted method gets `Countable|Serializable $thing` instead of
// `Countable&Serializable $thing`.
```

**Blocked by M4.** The fix requires `PhpType::Intersection` from the
mago-type-syntax migration. The current `Vec<ResolvedType>` has no way
to distinguish "these types form an intersection" from "these types
form a union". With `PhpType`, the intersection is a single tree node.

**After fixing:** verify that extract function docblock generation
preserves intersection types in both the native hint and the `@param`
tag.

---

## T13. Closure variables lose callable signature detail
**Impact: Low-Medium · Effort: Medium**

When a variable holds a closure or arrow function, the resolution
pipeline resolves it to the `Closure` class name. The callable
signature (parameter types, return type) is lost. This means:

1. Passing `$fn` to an extracted method produces `Closure $fn` with
   `@param (Closure(): mixed)` instead of the concrete signature.
2. An explicit `/** @var (Closure(int): string) $fn */` annotation
   is recognised by variable resolution (`find_var_raw_type_in_source`
   returns the annotated type), but `clean_type_for_signature` now
   correctly extracts `Closure` as the native hint. The raw type is
   preserved for docblock generation.

The remaining gap is that *unannotated* closures like
`$fn = function(int $x): string { ... }` resolve to bare `Closure`
with no signature detail. `extract_closure_return_type_from_assignment`
extracts the return type for call-site resolution, but does not
produce a full callable type string for variable-type contexts.

**Example:**

```php
$fn = function(int $x): string { return (string)$x; };
// Extracting code that uses $fn as a parameter produces:
//   @param (Closure(): mixed) $fn
// Instead of:
//   @param (Closure(int): string) $fn
```

**What needs to change:**

1. When resolving a variable whose assignment RHS is a closure or
   arrow function, build a callable type string from the literal's
   parameter list and return type hint (e.g. `(Closure(int): string)`).
   Return this as the variable's type string instead of bare `Closure`.

2. `clean_type_for_signature` already handles parenthesized callable
   types by extracting the base name (`Closure` or `callable`), so
   the native hint will be correct.

3. `enrichment_plain` should recognise that a raw type like
   `(Closure(int): string)` already carries a full signature and
   should not be re-enriched to `(Closure(): mixed)`.

**After fixing:** verify that extract function docblock generation
emits the concrete callable signature in the `@param` tag.

---

## T16. Closure parameter type inference from generic host method
**Impact: Medium · Effort: Medium**

When a closure is passed to a method like
`Collection::each(callable(TValue): void)`, PHPantom should infer
the closure parameter type from the host method's template-bound
parameter type.  Currently, untyped closure parameters remain
unresolved.

**Reproducer:**

```php
/** @var Collection<int, CustomerDocument> $collection */
$collection->each(function ($class) {
    $class->someMethod();  // "Cannot resolve type of '$class'"
});
```

**What should work:** The `each` method's `@param` says the callable
receives `TValue`.  If `TValue` is bound to `CustomerDocument`, the
closure's `$class` should be typed `CustomerDocument`.

**Where to fix:**
- `src/completion/variable/resolution.rs` — closure parameter inference.
- `src/completion/variable/rhs_resolution.rs` — resolve the host
  method's parameter type and map template arguments to the closure's
  parameter positions.

**Discovered in:** analyze-triage iteration 5 (F4).  
**Impact in shared codebase:** ~5 diagnostics.

---

## T17. Method-level template binding from closure return type
**Impact: Medium · Effort: Medium**

Methods like `Collection::reduce()` declare a method-level template
parameter (`TReduceReturnType`) whose binding depends on the
closure argument's return type.  PHPantom doesn't infer method-level
template bindings from closure return types.

**Reproducer:**

```php
$total = $products->reduce(
    fn(Decimal $carry, Orderproduct $p): Decimal => $carry->add($p->price),
    new Decimal('0')
);
$total->add($order->postage);
//     ^^^ "subject type 'TReduceReturnType' could not be resolved"
```

**What should work:** The closure's return type annotation
(`Decimal`) should bind `TReduceReturnType = Decimal`, making
`reduce()` return `Decimal`.

**Where to fix:**
- `src/completion/call_resolution.rs` —
  `resolve_method_return_types_with_args`: when a method has a
  method-level template parameter, check whether a closure argument
  provides a return type annotation that can bind it.

**Discovered in:** analyze-triage iteration 7 (F13).  
**Impact in shared codebase:** 3 diagnostics + 7 cascading
(FlowService.php L106, L343, L517).

---



