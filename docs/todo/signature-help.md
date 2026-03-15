# PHPantom — Signature Help

Signature help is architecturally solid. Dual-path detection (AST-based
`CallSite` lookup + text-based fallback), precomputed comma offsets for
active parameter tracking, content patching for unclosed parens, and
chain/constructor/first-class-callable resolution all work well. The
popup shows a compact parameter list with native PHP types, a shortened
return type, per-parameter `@param` descriptions, and default values in
parameter labels.

The remaining work requires new extraction or deeper protocol support.

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## S1. Attribute constructor signature help
**Impact: Medium · Effort: Medium**

Signature help should fire inside attribute argument lists:

```php
#[Route('/users', methods: ['GET'])]
//      ^ signature help here showing Route::__construct params
```

#### Current state

Attributes are parsed by Mago as `Attribute` AST nodes with an
`ArgumentList`, but no `CallSite` is emitted for them because they
are not function calls. The signature help detection path only looks
for function/method call expressions.

#### Implementation

1. **Emit synthetic `CallSite` for attributes** — in
   `symbol_map/extraction.rs`, when walking an `Attribute` node that
   has an argument list, emit a `CallSite` whose target is the
   attribute class's `__construct` method. The attribute name resolves
   through the use-map like any class reference.

2. **Resolve the constructor** — in `resolve_callable`, when the
   target is an attribute class, look up its `__construct` method.
   Most attributes have a simple constructor with named parameters
   (PHP 8.0+), so named argument awareness (S4) would pair well.

3. **Label prefix** — use the attribute short name (e.g. `Route`)
   as the signature label, not `__construct`.

#### Tests

- Integration test: `#[Route('/path', ` → assert signature help
  shows `Route::__construct` parameters.
- Integration test: `#[Deprecated(reason: ` → assert
  `active_parameter` points to the `$reason` parameter.
- Stub attributes like `#[Override]` (no constructor args) should
  return an empty signature or no signature.

---

## S2. Closure / arrow function parameter signature help
**Impact: Medium · Effort: Medium**

Signature help should work when invoking a variable that holds a closure
or arrow function:

```php
$format = fn(string $name, int $age): string => "$name ($age)";
$format('Alice', 30);  // ← signature help here
```

#### Current state

`extract_callable_target_from_variable` handles first-class callables
(`$fn = makePen(...)`) by scanning for the `(...)` suffix.  Closures
and arrow functions assigned to variables are not detected because they
don't end with `(...)`.

#### Implementation

1. **Detect closure/arrow assignments** — in
   `extract_callable_target_from_variable`, if the RHS does not end with
   `(...)`, check whether it starts with `function(` or `fn(`.  If so,
   return a synthetic identifier (e.g. `"__closure_at_L{line}"`) that
   the resolver can look up.

2. **Parse closure parameters** — alternatively, skip the
   `resolve_callable_target` pathway entirely.  When the variable is
   assigned a closure/arrow function, parse the parameters and return
   type directly from the AST of the assignment RHS.  Build the
   `ResolvedCallableTarget` inline without going through class
   resolution.

   This is the cleaner approach: closures don't have classes, so the
   existing class-based resolution is the wrong abstraction.  The
   `SymbolMap` already records `VarDefSite` for the assignment, and the
   AST is available.

3. **Label prefix** — use `$format` (the variable name) or the closure's
   inferred signature as the label prefix.

#### Tests

- Integration test: `$fn = fn(string $x): int => 0; $fn(` → assert
  signature help shows `string $x` with return type `int`.
- Integration test: `$fn = function(int $a, int $b): int { ... }; $fn('x', ` →
  assert `active_parameter` is 1.
- Integration test: `$fn = $obj->method(...)` (existing first-class
  callable path) → continues to work unchanged.

---

## S3. Multiple overloaded signatures
**Impact: Low · Effort: Medium-High**

Some PHP functions have multiple signatures depending on argument count
or types.  For example, `array_map` can be called as:

```php
array_map(callable $callback, array $array): array
array_map(null, array ...$arrays): array
```

The LSP protocol supports returning multiple `SignatureInformation`
entries with an `activeSignature` index.  Today we return a single
signature.

#### Current state

phpstorm-stubs define multiple function entries (or parameter variants
annotated with `#[PhpStormStubsElementAvailable]`) for overloaded
functions.  Our PHP-version filtering selects one variant.  We don't
model true overloads.

#### Implementation

This is a deeper change:

1. When a function has multiple stub entries (or when a class has
   multiple `__construct` signatures for different PHP versions),
   collect all applicable signatures.
2. Return them all in the `signatures` array.
3. Set `activeSignature` based on argument-count matching: pick the
   first signature whose parameter count accommodates the current
   argument count.

**Deferred** — the single-signature approach covers 99% of real usage.

---

## S4. Named argument awareness in active parameter
**Impact: Low · Effort: Medium**

When the user types a named argument (`callback: ` in `array_map(callback: `),
the active parameter should highlight the `$callback` parameter regardless
of its positional index.

#### Current state

Active parameter is computed purely by counting commas before the cursor.
Named arguments are handled by the named-argument completion system
(`completion/named_args.rs`) but the signature help active-parameter
tracking doesn't consult argument names.

#### Implementation

1. In `detect_call_site_from_map`, after computing the comma-based
   `active` index, extract the text of the current argument segment.
2. If the segment matches `identifier:` (named argument syntax), look up
   which parameter index corresponds to that name.
3. Override `active_parameter` with the named parameter's index.

This requires access to the resolved parameters (to map name → index),
which isn't available in the detection layer.  The override could be
applied later in `resolve_signature`, after `resolve_callable` returns
the parameter list.

---

## S5. Language construct signature help and hover
**Impact: Low · Effort: Low**

PHP language constructs that use parentheses (`unset()`, `isset()`, `empty()`,
`eval()`, `exit()`, `die()`, `print()`, `list()`) are not function calls in the
AST. Mago parses them as dedicated statement/expression nodes (e.g.
`Statement::Unset`) with no `ArgumentList`, so no `CallSite` is emitted and
neither signature help nor hover fires inside their parentheses. The phpstorm-stubs
don't define them either since they are keywords, not functions.

Supporting them requires emitting synthetic `CallSite` entries from the
statement-level extraction in `symbol_map.rs` and adding hardcoded parameter
metadata (e.g. `unset(mixed ...$vars): void`) in `resolve_callable`. Hover would
need a similar hardcoded lookup.