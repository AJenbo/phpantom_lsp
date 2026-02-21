# PHPantom — Remaining Work

> Last updated: 2026-02-21

Items are ordered by recommended implementation sequence: quick wins
first, then high-impact items, then competitive-parity features, then
long-tail polish.

---

## Completion & Go-to-Definition Gaps

### High impact

#### 23. Match-expression class-string not forwarded to conditional return types

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

---

#### Trait property completion/goto-definition gap: InteractsWithIO::$output and createProgressBar()

We are unable to go to `createProgressBar()` and can't provide completion
for `$this->output` when a trait defines a property with a docblock type,
and a class uses that trait:

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

---

### Competitive parity (close the gap with PHPStorm / Intelephense)

#### 30. No completion or go-to-definition inside anonymous classes

Anonymous classes (`new class { ... }`) are not parsed by
`extract_classes_from_statements`, which only handles named
`Statement::Class` / `Statement::Interface` / `Statement::Trait` /
`Statement::Enum` nodes. Because anonymous classes are expression-level
constructs (`Expression::AnonymousClass`), they are invisible to the
AST extraction pass.

This means:
- `$this->` inside an anonymous class body does not resolve to the
  anonymous class's own members.
- Members declared in the anonymous class are not available for
  completion.
- Go-to-definition for members of the anonymous class fails.

```php
$handler = new class($dependency) extends BaseHandler {
    public function handle(): Response {
        $this->  // ← no completion for anonymous class's own members
    }
};
```

**Fix:** in `extract_classes_from_statements`, walk expression
statements and detect `Expression::AnonymousClass` nodes. Synthesize a
`ClassInfo` with a unique internal name (e.g. `__anonymous@<offset>`)
so that `find_class_at_offset` resolves `$this` correctly.

---

#### 26. First-class callable syntax not tracked

PHP 8.1's first-class callable syntax (`$fn = strlen(...)`,
`$fn = $obj->method(...)`, `$fn = ClassName::staticMethod(...)`) creates
a `Closure` object, but the assignment scanner does not recognise the
`(...)` token. The variable `$fn` gets no type, so neither `$fn()` return
type resolution nor `$fn->` (for `Closure` methods like `bindTo()`) works.

```php
$fn = strlen(...);
$fn();  // ← return type not resolved (should be int)

$fn2 = $user->getName(...);
$fn2();  // ← return type not resolved
```

**Fix:** in `resolve_rhs_expression`, handle the first-class callable AST
node by resolving the referenced function/method and wrapping its return
type information so that `$fn()` resolves correctly.

---

#### 31. No context-aware filtering for class name completions

Class name completion always offers the full unfiltered list of known
classes, interfaces, traits, and enums regardless of syntactic context.
In practice this means:

- `extends ` suggests interfaces, traits, enums, and final classes
  (only non-final classes are valid)
- `implements ` suggests classes, traits, and enums (only interfaces
  are valid)
- `use ` (trait use inside a class body) suggests classes, interfaces,
  and enums (only traits are valid)
- `#[` (attribute) suggests non-attribute classes
- `instanceof ` suggests traits (which are not valid on the RHS)

Completion technically *works* in all these positions (the correct item
is in the list), but the results contain many invalid suggestions.

**Fix:** detect these syntactic contexts in the completion handler
(similar to `is_new_context` / `is_throw_new_context`) and pass a
filter to `build_class_name_completions` that restricts by
`ClassLikeKind` and/or `is_final` / `is_abstract`.

---

#### 32. No namespace-segment completion in `use` import statements

When typing `use App\Models\`, the class name completion path offers
full class FQNs that match the prefix. It does not offer intermediate
namespace segments as standalone suggestions (e.g. offering `Models\`
as a navigable segment when typing `use App\`).

Most PHP LSPs show namespace segments as folder-like completions so the
user can incrementally drill into the namespace tree. PHPantom jumps
straight to full class names, which works but can be overwhelming in
large projects with deep namespace hierarchies.

---

#### 21. No reverse jump: implementation → interface method declaration

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

### Remaining by user need

#### 27. No completion inside string interpolation

Inside double-quoted strings with variable interpolation, member access
completion does not work. PHP supports `"Hello $user->name"` and
`"Hello {$user->getName()}"`, but the completion handler has no
string-interpolation awareness. The subject extraction may accidentally
work in trivial cases but is not reliable because the surrounding quote
characters can confuse offset calculation and the patched content.

```php
$greeting = "Hello {$user->}";
//                         ^ no completion
```

---

#### 20. Non-`$this` property access in text-based assignment path

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

---

#### 34 / 36. No go-to-definition for built-in (stub) functions and constants

Clicking on a built-in function name like `array_map`, `strlen`, or
`json_decode` does not navigate anywhere. `resolve_function_definition`
finds the function in `stub_function_index` and caches it under a
synthetic `phpantom-stub-fn://` URI, but then explicitly skips navigation
because the URI is not a real file path. The same applies to built-in
constants like `PHP_EOL`, `SORT_ASC`, `PHP_INT_MAX` — they exist in
`stub_constant_index` for completion but `resolve_constant_definition`
only checks `global_defines`.

User-defined functions and `define()` constants work correctly. Only
built-in PHP symbols from stubs are affected.

**Fix:** either embed the stub source files as navigable resources (e.g.
write them to a temporary directory and use real file URIs), or accept
that stub go-to-definition is out of scope and document it as a known
limitation.

---

#### 33. Generator yield type not inferred inside generator bodies

When a method is annotated `@return Generator<int, User>`, the yield
value type (`User`) is correctly extracted when iterating the generator
with `foreach`. However, *inside* the generator body itself, there is
no inference from the declared return type back to the yielded values.

```php
class UserRepository {
    /** @return \Generator<int, User> */
    public function findAll(): \Generator {
        // Inside this body, `$user` should be typed as User
        // based on the Generator return annotation, but it isn't.
        yield $user;
        $user->  // ← no completion
    }
}
```

This is a niche scenario (the developer writing the generator usually
knows the types), but it would help when the generator body grows large
and variables are passed around before being yielded.

---

## Go-to-Implementation Gaps

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
