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

## T1. Inherited docblock type propagation
**Impact: High · Effort: Medium**

When a child class overrides a method from a parent class or interface,
the ancestor's richer docblock types should flow down unconditionally.
Inheritance is the default — if the ancestor says `@return list<Pen>`
and the child just says `: array`, the resolved return type must be
`list<Pen>`. There is no opt-in; `@inheritDoc` is functionally
meaningless because a child that can run code already has the parent's
contract. The only thing that *blocks* inheritance is the child
providing its own docblock type that is equally or more specific.

**Example:**

```php
interface PenHolder {
    /** @return list<Pen> */
    public function getPens(): array;
}

class Drawer implements PenHolder {
    // No docblock — native return type is just `array`.
    public function getPens(): array { return []; }
}

$d = new Drawer();
$d->getPens()[0]-> // ← should complete Pen members
```

Today `Drawer::getPens()` resolves to `return_type: "array"` because
the method's own docblock has no `@return` tag and the native hint is
`array`. The interface's `@return list<Pen>` is never consulted.

**Root cause:** `resolve_class_with_inheritance` (inheritance.rs L155)
and `resolve_class_fully_inner` (virtual_members/mod.rs L360) both
skip a parent/interface method when the child already declares one
with the same name — the child wins unconditionally. No fallback
check compares the richness of the return type.

**What needs to change:**

1. **During inheritance merging** (`resolve_class_with_inheritance`):
   when the child already has a method with the same name, don't
   just skip — enrich it. If the child's `return_type` equals its
   `native_return_type` (i.e. no docblock refined it) and the
   ancestor's `return_type` differs from its `native_return_type`
   (i.e. the ancestor *does* have a richer docblock type), copy the
   ancestor's `return_type` onto the child's method. Do the same
   for each parameter's `type_hint` when the child's matches its
   `native_type_hint`. Also inherit `description` and
   `return_description` when the child lacks them.

   **Parameter name remapping.** When propagating `@param` types,
   match by position, not by name. A child may rename `$userId` to
   `$id`; the parent's `@param int $userId` at position 0 should
   still flow to the child's position 0. PHPStan's
   `PhpDocInheritanceResolver` builds an explicit positional mapping
   for this.

2. **During interface merging** (`resolve_class_fully_inner`): same
   logic — when an interface method is skipped because the class
   already defines it, enrich the existing method with the
   interface's richer types and descriptions.

3. **Child docblock wins when present.** If the child provides its
   own `@return` or `@param` type (even if less specific), that is
   an intentional override and the ancestor type is not propagated.
   The test is simple: does the child's effective type differ from
   its native type? If yes, the child wrote a docblock — respect it.

**Scope of the fix:** This affects completion (return type drives
chain resolution), hover (return type displayed), and signature help
(parameter types shown). All three automatically benefit once the
merged `MethodInfo` carries the richer type.

**Properties too:** The same pattern applies to properties. An
interface declaring `@property-read list<Pen> $pens` should
propagate to an implementing class's `$pens` property if the class
only has a native `array` type hint.

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

## T8. Null coalesce (`??`) type refinement
**Impact: Low-Medium · Effort: Low**

The `??` operator currently unions LHS and RHS unconditionally in both
`raw_type_inference.rs` and `rhs_resolution.rs`. Two improvements:

1. **Non-nullable LHS:** when the LHS type is provably non-nullable
   (e.g. `new Foo()`, a non-nullable return type, a literal), the RHS
   is dead code and the result should resolve to the LHS type only.
2. **Nullable LHS via function return:** when a function returns
   `?Foo`, `$x = getOptional() ?? $fallback` should strip `null` from
   the LHS and union with the RHS. Today the function return type
   doesn't flow through `??` in top-level variable contexts.

**Fixtures to activate:**

- `null_coalesce/non_nullable_lhs.fixture` — LHS is `new Foo()`, RHS
  should be ignored
- `null_coalesce/nullable_lhs.fixture` — LHS is `?Foo` from function
  return, result is `Foo|Fallback`

**phpactor ref:** `null-coalesce/null-coalesce_null.test`,
`null-coalesce/null-coalesce_nullable.test`

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

