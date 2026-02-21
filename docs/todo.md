# PHPantom — Remaining Work

> Last updated: 2026-02-21

---

### Trait property completion/goto-definition gap: InteractsWithIO::$output and createProgressBar()

We are unable to go to `createProgressBar()` and can't provide completion for `$this->output` when a trait defines a property with a docblock type, and a class uses that trait:

```php
trait InteractsWithIO
{
    /**
     * @var \Illuminate\Console\OutputStyle
     */
    protected $output;
}

class Command extends SymfonyCommand
{
    use Concerns\InteractsWithIO;
}

final class ReindexSelectedCommand extends Command
{
    public function handle(): int
    {
        $bar = $this->output->createProgressBar();
    }
}
```

- Completion for `$this->output` does not resolve to `OutputStyle`
- Go-to-definition for `createProgressBar()` fails


## Completion Gaps

### 16. Multi-line method chain completion
**Priority: High**

Subject extraction in `completion/target.rs` and `subject_extraction.rs`
operates on the current line only.  Chains that span multiple lines produce
no completions:

```php
$this->getRepository()
    ->findAll()
    ->filter(fn($u) => $u->active)
    ->  // ← cursor here: no completion
```

`extract_completion_target` sees only whitespace and `->` on the cursor
line and `extract_arrow_subject` finds nothing meaningful to the left.
This is extremely common in Laravel/Symfony code with fluent builders,
query chains, and collection pipelines.

The same limitation affects go-to-definition: `extract_member_access_context`
in `definition/member.rs` also works on the current line, so Ctrl-clicking
a method in a multi-line chain cannot resolve the owning class.

**Fix:** before extracting the subject, collapse continuation lines around
the cursor.  Lines that start with `->` or `?->` (after optional
whitespace) should be joined with the preceding line, then the extraction
runs on the flattened text.

### ~~17. Switch statement variable type tracking~~ ✅

Resolved. Added a `Statement::Switch` arm to `walk_statements_for_assignments`
that iterates each case's statement list with `conditional = true`.
Both brace-delimited and colon-delimited (`switch(): … endswitch;`) forms
are handled via `switch.body.cases()`.

### 18. `?->` chaining loses intermediate segments
**Priority: Medium**

In `extract_arrow_subject` (`subject_extraction.rs`), when a `?->` is
encountered mid-chain, the code calls `extract_simple_variable` instead
of recursing with `extract_arrow_subject`.  The `->` path recurses
correctly, but `?->` does not:

```php
$user->getAddress()?->getCity()->  // extracts "$user?->getCity", loses "->getAddress()"
```

**Fix:** change the `?->` branch to call `extract_arrow_subject(chars, inner_arrow)`
instead of `extract_simple_variable(chars, inner_arrow)`, mirroring what
the `->` branch does.

### 19. `static` return type not resolved to concrete class at call sites
**Priority: Medium**

When a method declares `@return static` (common in builder/factory
patterns), `type_hint_to_classes` resolves `static` to
`owning_class_name` — the class that *declares* the method, not the
class it is called on:

```php
class Builder {
    /** @return static */
    public function configure(): static { return $this; }
}
class AppBuilder extends Builder {}

$builder = new AppBuilder();
$builder->configure()->  // resolves to Builder, not AppBuilder
```

The resolution works correctly when the subject is `$this` or `self`,
but when the method return type is `static` and the call is on a variable
typed as a subclass, the declaring class is used instead of the
variable's concrete type.

**Fix:** when `resolve_method_return_types_with_args` encounters a
`static` (or `$this`) return type, substitute the caller's class name
(the class the subject resolved to) rather than the class that declares
the method.

### 15. `unset()` tracking
**Priority: Medium**

`unset($var)` removes a variable from scope, and `unset($arr['key'])` removes
a key from an array shape. Neither is tracked today.

- **Variable scope.** After `unset($x)`, the variable `$x` should no longer
  appear in variable name suggestions, and `$x->` should not resolve to the
  type it had before the `unset`.
- **Array shape keys.** After `unset($config['host'])`, the key `host` should
  no longer appear in `$config['` key completions, and the inferred shape
  should reflect its removal.

Both cases require the assignment/variable scanner in
`completion/variable_resolution.rs` to recognise `unset(...)` statements
and update its tracking accordingly.

### 20. Non-`$this` property access in text-based assignment path
**Priority: Low**

In `extract_raw_type_from_assignment_text` (`completion/text_resolution.rs`),
property access on the RHS is only handled for `$this->propName`.  When
the RHS is `$otherVar->propName`, it falls through to `None`:

```php
$user = getUser();
$address = $user->address;  // text-based path returns None
$address->  // ← no completion (unless the AST-based path catches it)
```

The AST-based path in `resolve_rhs_expression` handles this correctly,
so the gap only surfaces in the text-based fallback used for intermediate
chained assignments and some edge cases.

**Fix:** after the `$this->propName` check, add a branch that resolves
`$var->propName` by first resolving `$var`'s type via
`extract_raw_type_from_assignment_text` (recursively), then looking up
the property on the resulting class.

### 22. Template parameter bounds not used for property type resolution
**Priority: Medium**

When a property's docblock type is a template parameter (e.g. `TNode`),
the resolver treats it as a raw class name and fails to find it. It
should recognise that `TNode` is a `@template` parameter of the
enclosing class and fall back to its upper bound for completion and
definition:

```php
/**
 * @template-covariant TNode of PDependNode
 */
abstract class AbstractNode
{
    /**
     * @param TNode $node
     */
    public function __construct(
        private readonly PDependNode $node,
    ) {
    }

    public function getParent()
    {
        $this->node->getParent();  // ← no completion / definition
    }
}
```

The native type hint is `PDependNode`, but `@param TNode $node` overrides
it via `resolve_effective_type`. The resolved type becomes `TNode`, which
does not match any class. The resolver should detect that `TNode` is a
template parameter declared on the enclosing class, read its bound
(`of PDependNode`), and use that bound as the effective type.

**Fix:** in the property/parameter type resolution path (likely
`resolve_subject_type` or `type_hint_to_classes` in
`completion/resolver.rs`), when a resolved type string does not match any
known class, check whether it appears in the enclosing class's
`@template` / `@template-covariant` / `@template-contravariant`
declarations. If it does and has an `of` bound, substitute the bound
type. If the template parameter has no bound (bare `@template T`), fall
back to the native type hint instead of the docblock type, since the
native hint is the only concrete type information available. This also
applies to method return types that use bare template parameters without
a concrete substitution available.

### 23. Match-expression class-string not forwarded to conditional return types
**Priority: Medium**

When a variable is assigned a `::class` value through a `match` expression
and then passed to a function whose return type depends on that argument
(e.g. `@template T @param class-string<T> @return T`), the resolver cannot
trace the class-string back through the match arms:

```php
$requestType = match ($typeName) {
    'creditnotes' => GetCreditnotesRequest::class,
    'orders'      => GetOrdersRequest::class,
};
$requestBody = app()->make($requestType);
$requestBody->enabled();  // ← no completion
```

The resolver already handles direct `::class` arguments at the call site
(e.g. `app()->make(GetCreditnotesRequest::class)`), but when the
class-string is stored in an intermediate variable whose value comes from
a match expression, the link is lost.

Two pieces are needed:

1. **Track class-string values from match arms.** When the RHS of an
   assignment is a `match` expression and every arm returns a `Foo::class`
   literal, record the set of possible class-string values on the
   variable (e.g. `$requestType` holds
   `GetCreditnotesRequest|GetOrdersRequest`).
2. **Resolve class-string variables at call sites.** When a variable is
   passed as an argument to a function with a `@template` + conditional
   return type, look up the variable's class-string value(s) and use them
   for template substitution, producing a union of the possible return
   types.

### 24. Namespaced class breaks `static` return type chain resolution
**Priority: Medium**

When a class lives in a namespace and calls a static method whose return
type is `Builder<static>`, the chain resolution breaks. The same code
works without a namespace.

```php
// lead_providers.php
namespace Luxplus\Core\Database\Model\LeadProviders;

final class LeadProviders extends Model {}

LeadProviders::query()->  // ← no completion (works without the namespace)
```

```php
// model.php (same or separate file)
abstract class Model
{
    /** @return \Illuminate\Database\Eloquent\Builder */
    public static function query() {}
}
```

```php
// builder.php
namespace Illuminate\Database\Eloquent;

class Builder
{
    public function where() {}
}
```

The generic argument in `Builder` should resolve to
`LeadProviders`, giving a `Builder`, and `where()` should then be
offered.  With a namespace present the chain breaks, likely because the
FQN resolution of `Model` when a namespace is involved.

---

## Go-to-Definition Gaps

### 21. No reverse jump: implementation → interface method declaration
**Priority: Medium**

Go-to-implementation lets you jump from an interface method to its concrete
implementations, but there is no way to jump from a concrete implementation
*back* to the interface or abstract method it satisfies.  For example,
clicking `handle()` in a class that `implements Handler` cannot jump to
`Handler::handle()`.

This would be a natural extension of `find_declaring_class` in
`definition/member.rs`: when the cursor is on a method *definition* (not
a call), check whether any implemented interface or parent abstract class
declares a method with the same name, and offer that as a definition
target.

---

## Go-to-Implementation Gaps

### 5a. Transitive interface inheritance
**Priority: Medium**

If `InterfaceB extends InterfaceA` and `ClassC implements InterfaceB`,
go-to-implementation on `InterfaceA` will not find `ClassC`.  The
transitive check in `class_implements_or_extends` walks `parent_class`
chains but does not walk interface-extends chains.

**Fix:** in `class_implements_or_extends`, when checking a class's
`interfaces` list, load each interface via `class_loader` and recursively
check whether it extends the target interface (with a depth bound).

### 5b. Short-name collisions in `find_implementors`
**Priority: Low**

`class_implements_or_extends` matches interfaces by both short name and
FQN (`iface_short == target_short || iface == target_fqn`).  Two
interfaces in different namespaces with the same short name (e.g.
`App\Logger` and `Vendor\Logger`) could produce false positives.
Similarly, `seen_names` in `find_implementors` deduplicates by short
name, so two classes with the same short name in different namespaces
could shadow each other.

**Fix:** always compare fully-qualified names by resolving both sides
before comparison.

---

## Missing LSP Features

### 6. Hover (`textDocument/hover`)
**Priority: High**

No hover support at all. Users can't see inferred types, docblock descriptions,
or method signatures by hovering. Most of the infrastructure already exists
(type resolution, class loading, docblocks) — wiring it into a hover handler
would be relatively straightforward and high-impact.

---

### 7. Signature Help (`textDocument/signatureHelp`)
**Priority: Medium**

No parameter hints shown while typing function/method arguments. Named arg
completion partially fills this role, but proper signature help is more
ergonomic.

---

### 8. Document Symbols (`textDocument/documentSymbol`)
**Priority: Medium**

No outline view. Editors can't show a file's class/method/property structure.

---

### 9. Find References (`textDocument/references`)
**Priority: Medium**

Can't find all usages of a symbol.

---

### 10. Rename (`textDocument/rename`)
**Priority: Low**

No rename refactoring support.

---

### 11. Workspace Symbols (`workspace/symbol`)
**Priority: Low**

Can't search for classes/functions across the project.

---

### 12. Diagnostics
**Priority: Low** (large scope)

No error reporting (undefined methods, type mismatches, etc.).

---

### 13. Code Actions
**Priority: Low**

No quick fixes or refactoring suggestions. No `codeActionProvider` in
`ServerCapabilities`, no `textDocument/codeAction` handler, and no
`WorkspaceEdit` generation infrastructure beyond trivial `TextEdit`s for
use-statement insertion.

#### 13a. Extract Function refactoring

Select a range of statements inside a method/function and extract them into a
new function. The LSP would need to:

1. **Scope analysis** — determine which variables are read in the selection but
   defined before it (→ parameters) and which are written in the selection but
   read after it (→ return values).
2. **Statement boundary validation** — reject selections that split an
   expression or cross control-flow boundaries in invalid ways.
3. **Type annotation** — use variable type resolution to generate parameter and
   return type hints on the new function.
4. **Code generation** — produce a `WorkspaceEdit` that replaces the selection
   with a call and inserts the new function definition nearby.

**Prerequisites (build these first):**

| Feature | What it contributes |
|---|---|
| Hover (§6) | "Resolve type at arbitrary position" — needed to type params |
| Document Symbols (§8) | AST range → symbol mapping — needed to find enclosing function and valid insertion points |
| Find References (§9) | Variable usage tracking across a scope — the same "which variables are used where" analysis |
| Simple code actions (add use stmt, implement interface) | Builds the code action + `WorkspaceEdit` plumbing |

---

## Performance / UX Ideas

### 14. Partial result streaming via `$/progress`
**Priority: Medium** (cross-cutting optimisation)

The LSP spec (3.17) allows requests that return arrays — such as
`textDocument/implementation`, `textDocument/references`,
`workspace/symbol`, and even `textDocument/completion` — to stream
incremental batches of results via `$/progress` notifications when both
sides negotiate a `partialResultToken`.  The final RPC response then
carries `null` (all items were already sent through progress).

This would let PHPantom deliver the *first* useful results almost
instantly instead of blocking until every source has been scanned.

#### Streaming between existing phases

`find_implementors` already runs five sequential phases (see
`docs/ARCHITECTURE.md` § Go-to-Implementation):

1. **Phase 1 — ast_map** (already-parsed classes in memory) — essentially
   free.  Flush results immediately.
2. **Phase 2 — class_index** (FQN → URI entries not yet in ast_map) —
   loads individual files.  Flush after each batch.
3. **Phase 3 — classmap files** (Composer classmap, user + vendor mixed)
   — iterates unique file paths, applies string pre-filter, parses
   matches.  This is the widest phase and the best candidate for
   within-phase streaming (see below).
4. **Phase 4 — embedded stubs** (string pre-filter → lazy parse) — flush
   after stubs are checked.
5. **Phase 5 — PSR-4 directory walk** (user code only, catches files not
   in the classmap) — disk I/O + parse per file, good candidate for
   per-file streaming.

Each phase boundary is a natural point to flush a `$/progress` batch,
so the editor starts populating the results list while heavier phases
are still running.

#### Prioritising user code within Phase 3

Phase 3 iterates the Composer classmap, which contains both user and
vendor entries.  Currently they are processed in arbitrary order.  A
simple optimisation: partition classmap file paths into user paths
(under PSR-4 roots from `composer.json` `autoload` / `autoload-dev`)
and vendor paths (everything else, typically under `vendor/`), then
process user paths first.  This way the results most relevant to the
developer arrive before vendor matches, even within a single phase.

#### Granularity options

- **Per-phase batches** (simplest) — one `$/progress` notification at
  each of the five phase boundaries listed above.
- **Per-file streaming** — within Phases 3 and 5, emit results as each
  file is parsed from disk instead of waiting for the entire phase to
  finish.  Phase 3 can iterate hundreds of classmap files and Phase 5
  recursively walks PSR-4 directories, so per-file flushing would
  significantly improve perceived latency for large projects.
- **Adaptive batching** — collect results for a short window (e.g. 50 ms)
  then flush, balancing notification overhead against latency.

#### Applicable requests

| Request | Benefit |
|---|---|
| `textDocument/implementation` | Already scans five phases; each phase's matches can be streamed |
| `textDocument/references` (§9) | Will need full-project scanning; streaming is essential |
| `workspace/symbol` (§11) | Searches every known class/function; early batches feel instant |
| `textDocument/completion` | Less critical (usually fast), but long chains through vendor code could benefit |

#### Implementation sketch

1. Check whether the client sent a `partialResultToken` in the request
   params.
2. If yes, create a `$/progress` sender.  After each scan phase (or
   per-file, depending on granularity), send a
   `ProgressParams { token, value: [items...] }` notification.
3. Return `null` as the final response.
4. If no token was provided, fall back to the current behaviour: collect
   everything, return once.
