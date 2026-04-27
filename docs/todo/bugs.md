# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

## B2 — Variable resolution pipeline produces short names instead of FQN

**Root cause:** The variable resolution pipeline returns
`ResolvedType` values whose `type_string` field contains short
class names taken verbatim from docblock text or AST identifiers.
The pipeline never resolves these names through the use-map or
class loader before storing them.

**Where to fix:** Every code path that produces a `ResolvedType`
from raw source text must resolve names to FQN before returning.
The fix belongs in the resolution pipeline itself, not in each
downstream consumer. Specifically:

- `try_inline_var_override` in `completion/variable/resolution.rs`
  gets a `PhpType` from `find_inline_var_docblock` and passes it
  to `from_type_string` or `from_classes_with_hint` without
  resolving names through the use-map. It must resolve first.
- `resolve_rhs_instantiation` in `completion/variable/rhs_resolution.rs`
  constructs `PhpType::Named(name.to_string())` from the raw AST
  identifier (short name). It must resolve the name to FQN before
  wrapping it.
- `try_standalone_var_docblock` in `closure_resolution.rs` has the
  same pattern as `try_inline_var_override`.
- `find_iterable_raw_type_in_source` and `find_var_raw_type_in_source`
  in `docblock/tags.rs` return raw docblock types. Every caller
  that stores them in a `ResolvedType` must resolve names first.

Current mitigation: `collect_type_error_diagnostics` applies
`resolve_names` with the class loader on every resolved argument
type before comparison. This papers over the problem for one
consumer but leaves others broken (hover type display, definition
matching, etc.).

The proper fix is to always store FQN in `type_string` at the
point of creation and shorten at display time (the way
`implement_methods.rs` already does with `shorten_type`).
Consumers that need short names for user-facing output (e.g.
PHPDoc generation code actions) should shorten on the way out,
not expect short names from the pipeline.

## B3 — Array access on bare `array` returns empty instead of `mixed`

**Root cause:** The type resolution pipeline does not handle array
element access on the bare `array` type. When a parameter is typed
as `array` (no generic annotation), accessing an element with
`$params['key']` resolves to an empty/untyped result instead of
`mixed`.

**Where to fix:** The array access resolution code (wherever
`$var['key']` is resolved to a type) must recognise bare `array`
and `mixed` as "unknown element type" and return `mixed`. This is
a fix in the variable/expression type resolution pipeline, not in
any diagnostic.

**Downstream effect:** Once the pipeline returns `mixed` for array
access on bare `array`, the following resolve correctly without any
additional changes:

- `$x = $params['key'] ?? null` resolves `$x` to `mixed|null`
  instead of just `null`.
- `type_error.argument` no longer flags `null` passed to `string`
  because the resolved type is `mixed|null`, which is compatible
  with anything.

Reproducer:

```php
function foo(array $params = []): void {
    $authToken = $params['authToken'] ?? null;
    if (!$authToken || !is_string($authToken)) {
        throw new \Exception('missing');
    }
    // $authToken is string here, but diagnostic sees null
    bar($authToken);
}
function bar(string $s): void {}
```

## B9 — `parent::__construct()` does not substitute `@extends` generics into inherited parameter types

**Root cause:** When a child class has `@extends Parent<Concrete>`
and calls `parent::__construct($arg)`, the diagnostic pipeline
resolves the callable target to the parent's constructor without
applying the child's `@extends` generic substitution. The parent
constructor's `@param ?T $item` retains the raw template name `T`
instead of being substituted with the concrete type from the
child's `@extends` annotation.

**Where to fix:** The callable target resolution for
`parent::__construct(...)` (in `resolve_constructor_callable` or
the `NewExpr` arm of `resolve_callable_target_with_args`) must
detect that the call originates from a child class, look up the
child's `extends_generics`, and apply template substitution to the
parent class before returning its constructor's parameter types.

Reproducer:

```php
/**
 * @template T of object
 */
class ItemResult {
    /** @param ?T $item */
    public function __construct(private readonly ?object $item) {}
}

/**
 * @extends ItemResult<BonusCashItem>
 */
final class BonusCashItemResult extends ItemResult {
    public function __construct(?BonusCashItem $credited) {
        parent::__construct($credited);
        // false positive: "expects ?T, got BonusCashItem"
    }
}

class BonusCashItem {}
```

## B10 — Foreach iteration on `@extends` subclass yields raw template param instead of concrete type

**Root cause:** When iterating over a variable whose type is a
subclass that extends a generic collection (e.g.
`IntCollection extends Collection<int, int>`), the foreach
element-type extraction does not look through the child's
`@extends` generics to substitute the parent's template params.
The iteration variable gets typed as raw `TValue` instead of `int`.

**Where to fix:** The foreach element-type resolution (in
`foreach_resolution.rs` or wherever the iterable element type is
extracted) must resolve `@extends` generics from the child class
before extracting the element type. When the variable's class is
`IntCollection` and it extends `Collection<int, int>`, the
iteration element type must be `int`, not `TValue`.

**Replicate on shared project:**

```
phpantom_lsp analyze --project-root shared --no-colour 2>/dev/null -- src/database/Model/Products/Filters/ProductFilterTermCollection.php
```

Reproducer:

```php
/**
 * @template TKey of array-key
 * @template TValue
 */
class Collection implements \ArrayAccess {
    /** @return TValue */
    public function offsetGet(mixed $offset): mixed {}
    public function offsetExists(mixed $offset): bool {}
    public function offsetSet(mixed $offset, mixed $value): void {}
    public function offsetUnset(mixed $offset): void {}
}

/** @extends Collection<int, int> */
final class IntCollection extends Collection {}

function test(): void {
    $ids = new IntCollection();
    foreach ($ids as $id) {
        // $id should be int, but resolves to TValue
        array_key_exists($id, [1 => 'a']);
        // false positive: "expects int|string, got TValue"
    }
}
```

## B11 — Static method-level `@template` not substituted when argument is a closure literal

**Root cause:** When a static method declares a method-level
`@template T of SomeType` and `@param T $param`, and the call-site
argument is a closure literal (e.g. `fn(array $q): bool => ...`),
`build_method_template_subs` either fails to resolve the argument
text to a type or the binding mode does not fire. The raw template
name (e.g. `TClosure`) leaks into the parameter type.

**Where to fix:** `build_method_template_subs` in
`call_resolution.rs` and/or `resolve_arg_text_to_type`. When the
argument text starts with `fn(` or `function(`, it should be
recognised as a `Closure` type (or more specifically
`Closure(params): ReturnType`) and used to bind the template param.

Reproducer:

```php
class Mockery {
    /**
     * @template TClosure of \Closure
     * @param TClosure $closure
     * @return ClosureMatcher
     */
    public static function on($closure) {
        return new ClosureMatcher($closure);
    }
}

class ClosureMatcher {}

function test(): void {
    Mockery::on(fn(array $query): bool => true);
    // false positive: "expects TClosure, got Closure"
}
```

## B8 — Class-level template parameters lost through chained method calls

**Root cause:** When a method returns a generic class (e.g.
`Collection<Product>`) and the next method in the chain accesses a
member of that class, the generic type arguments are discarded
during the chain resolution. Specifically,
`resolve_call_return_types_expr` converts intermediate
`ResolvedType` values (which carry generic args in their
`type_string` field) to `Vec<Arc<ClassInfo>>` via
`into_arced_classes`. This conversion discards the `type_string`,
so by the time the next method's return type needs to be
template-substituted, the generic arguments are gone.

**Where to fix:** The `MethodCall` arm of
`resolve_call_return_types_expr` must thread `ResolvedType` (with
its `type_string`) through to the method return-type resolution
step instead of flattening to bare `ClassInfo` first. The generic
arguments from the intermediate return type must survive into
`build_generic_subs` so that template substitution works at every
level of the chain, not just the first.

The first call in a chain already works (B6 fix). The fix here is
to apply the same pattern to subsequent calls in the chain.

Reproducer:

```php
/**
 * @template TItem
 */
class Collection {
    /** @param TItem $item */
    public function add($item): void {}

    /** @return self<TItem> */
    public function filter(): self { return $this; }
}

class Product {}

class Store {
    /** @return Collection<Product> */
    public function products(): Collection { return new Collection(); }
}

function test(): void {
    $store = new Store();
    $product = new Product();
    // First level works: $store->products()->add($product)
    // Second level fails: $store->products()->filter()->add($product)
    // false positive: "expects TItem, got Product"
    $store->products()->filter()->add($product);
}
```

## B12 — Hover cross-file property docblock cache invalidation fails after edits

**Root cause:** When a class is loaded from a cross-file source
(PSR-4 or classmap) and its docblock is later edited, hover
continues to show the stale docblock content instead of the updated
version. The parsed `ClassInfo` cached in `ast_map` and/or
`fqn_index` is not invalidated when the dependency file changes.

**Tests:** Six integration tests covering this bug were removed
because they were committed in a failing state. The fix must
include new passing tests for at least these scenarios:

- PSR-4 lazy-loaded class, then docblock edited (`did_change`)
- Dependent child class inheriting a changed `@property`
- `@var`-annotated variable accessing a cross-file property
- Method-chain access (`$this->getJob()->class_name`)
- Cache warm → edit → hover (eviction path)
- Child class with Model parent (Laravel `@property` interaction)

**Where to fix:** The cache layer that stores cross-file
`ClassInfo` results must be invalidated (or re-parsed) when
`didChange` or `didSave` fires for the dependency file. The
`resolved_class_cache` and/or `fqn_index` entries for the changed
URI must be evicted so that the next hover request re-parses the
file and picks up the new docblock content.

## B19 — Namespace-qualified scalar types hit class resolution

**Root cause:** When `find_or_load_class` is called with names like
`Tests\Feature\BusinessCentral\int`, `Tests\Support\array`, or
`Tests\Unit\Customers\bool`, these are scalar type hints that were
namespace-qualified by the name resolver (or the variable resolution
pipeline) instead of being recognised as built-in types. The class
resolution pipeline then walks through `fqn_index`, `class_index`,
`classmap`, and PSR-4 for each one before giving up and caching
a negative result. In the analyse pipeline this adds thousands of
wasted lookups per run.

**Where to fix:** Two complementary fixes:

1. The callers that produce these names (variable resolution,
   type-hint resolution) should recognise bare scalar keywords
   (`int`, `float`, `string`, `bool`, `array`, `object`, `mixed`,
   `void`, `null`, `never`, `true`, `false`, `callable`, `iterable`,
   `self`, `static`, `parent`) and never pass them to class
   resolution — even when they carry a namespace prefix. A type
   whose last segment is a scalar keyword is never a class.

2. As a safety net, `find_or_load_class_inner` could short-circuit
   on names whose last segment is a known scalar keyword, avoiding
   the multi-phase search entirely.

## B20 — `path_to_uri` produces malformed `file://` URIs from relative paths

**Root cause:** `crate::util::path_to_uri` calls
`Url::from_file_path(path)`, which requires an absolute path.
When the workspace root is relative (e.g. `shared`), all derived
paths are also relative (e.g. `shared/tests/TestCase.php`).
`Url::from_file_path` fails and the fallback
`format!("file://{}", path.display())` produces URIs like
`file://shared/tests/TestCase.php`. This is malformed: the `shared`
segment becomes the URI authority (hostname), not a path component.
The correct project-relative URI would use only the path within the
workspace root (e.g. `file://tests/TestCase.php`), and even that is
non-standard since `file://` URIs are meant to carry absolute paths.

**Symptoms:**

- `class_index` and `ast_map` are keyed by these malformed URIs.
  `Url::parse(...).to_file_path()` fails on them, so any code path
  that converts a cached URI back to a file path silently returns
  `None`.
- Any future code that relies on round-tripping a stored URI back
  to a file path (e.g. for cache eviction, go-to-definition across
  files, or incremental re-indexing) will silently fail for every
  file indexed under a relative workspace root.

**Where to fix:** `path_to_uri` (in `util.rs`) should canonicalize
relative paths to absolute before calling `Url::from_file_path`.
Alternatively, all callers that construct paths from the workspace
root should produce absolute paths in the first place. The
workspace root itself could be canonicalized at startup in
`analyse::run` and `fix::run` (for the CLI) and in the LSP
`initialize` handler (for the server).  Whichever approach is
chosen, every URI stored in `ast_map`, `class_index`, `fqn_index`,
`use_map`, `namespace_map`, and `symbol_maps` must use consistent
absolute `file:///` URIs so that lookups and cache eviction work
correctly regardless of how the tool was invoked.

---

## B22 — `$this` resolves in static methods

**Symptom:** Inside a `static` method, `$this->method()` resolves
as if the method were non-static. Variables assigned from
`$this->method()` get a type, and member access on those variables
produces no diagnostic. PHP would throw a fatal error at runtime.

**Where to fix:** The forward walker (and/or backward scanner) seeds
`$this` with the enclosing class type without checking whether the
method is static. The seeding logic should skip `$this` when the
enclosing function has the `static` modifier.

**Test:** `tests/integration/diag_timing.rs` —
`this_not_seeded_in_static_method` (currently `#[ignore]`). Remove
the `#[ignore]` attribute once fixed.

## B26 — Re-entry root cause in `process_array_key_assignment`

**Severity: Low (mitigated by guard, not root-caused)**

**Status:** A thread-local re-entry guard prevents the hang. The
symptom (infinite loop on `$arr['key'] = f($arr['key'])`) is fixed.
The root cause of the re-entry is still unknown.

**Background:** `process_array_key_assignment` in `forward_walk.rs`
was called in an infinite loop on files containing read-then-write
patterns on the same array key. The function itself is not recursive,
but something in its call chain (likely `resolve_rhs_with_scope` →
RHS resolution → scope variable lookup → re-evaluation of the array
shape type) triggers re-entry. The `for stmt in statements` loop
should only visit each statement once, yet profiling showed the
function firing thousands of times per second on the same assignment.

**Remaining work:** Identify WHY the outer loop re-visits the same
statement. Possible causes:

1. The shape merge in `merge_nested_shape_keys` produces a type that
   triggers re-processing of the same AST node through
   `record_scope_snapshot` or some snapshot-driven re-walk.
2. `walk_closures_in_statement` or `record_and_chain_snapshots`
   somehow re-dispatches the same expression to
   `process_assignment_expr`.
3. An iterator adapter on the AST statement list is being consumed
   multiple times due to a logic error in the walk.

Once the root cause is understood, the re-entry guard can be removed
in favour of a proper fix.

## B27 — CallSite matching in `emit_closure_hints` fails to find the parent call for template substitution

**Symptom:** When a function has `@template T` with a callable
parameter typed `callable(T): T`, the closure parameter inlay
hints show no type (T falls back to `mixed` and is filtered)
instead of the concrete type inferred from sibling arguments.

**Root cause:** `emit_closure_hints` in `inlay_hints.rs` tries to
find the matching `CallSite` for each `UntypedClosureSite` so it
can extract the full argument text and pass it to
`resolve_callable_target_with_args` for template substitution.
The matching logic compares `call_expression` strings and checks
whether any closure offset falls within the call site's
`(args_start, args_end)` range.  In practice, the match fails —
`call_args_text` ends up `None`, so no template substitution
occurs.  The fallback-to-bounds logic then maps every unbound
template parameter to `mixed`, and `is_mixed()` filters the hint.

**Where to fix:** Debug the offset comparison in the CallSite
matching block of `emit_closure_hints`.  The infrastructure is
already in place (the method accepts `call_sites` and builds
`call_args_text`); only the matching condition needs fixing.
Likely causes:

1. The `UntypedClosureSite` offsets (param variable offsets and
   close-paren offset) may all point inside the *closure's* own
   parameter list, which is nested inside the call's argument
   range but the byte offsets may not satisfy the `> args_start`
   / `< args_end` comparison due to off-by-one or encoding
   differences.
2. The `call_expression` string stored in `UntypedClosureSite`
   may differ subtly from the one in `CallSite` (e.g. trailing
   whitespace, different casing for method calls).

Once the matching works, the existing `build_function_template_subs`
/ `build_method_template_subs` machinery handles the actual
inference and substitution automatically.

**Affected features:** Inlay hints for closure/arrow-function
parameters when the callable type contains template parameters.
Also affects completion, hover, and signature help inside closure
bodies through the same callable type resolution path.
