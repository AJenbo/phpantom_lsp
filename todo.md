# PHPantom — Remaining Work

> Last updated: 2026-02-21

---

## Completed

### ~~1. Trait `insteadof` / `as` conflict resolution~~ ✅

Implemented in `parser/classes.rs` (parse `TraitUseConcreteSpecification`
adaptations into `TraitPrecedence` / `TraitAlias` structs), `types.rs`
(new `TraitPrecedence`, `TraitAlias`, `ExtractedMembers` types, new fields
on `ClassInfo`), `parser/ast_update.rs` (resolve trait names in adaptations),
and `inheritance.rs` (`merge_traits_into` applies `insteadof` exclusions,
visibility-only `as` changes, and `as` alias creation during merge).

### ~~2. Method-level `@template` parameters (general case)~~ ✅

Implemented general method-level `@template` substitution beyond the
existing `class-string<T>` special case.  When a method declares
`@template T` + `@param T $model` + `@return Collection<T>`, the resolver
now infers `T` from the actual argument type at the call site and
substitutes it into the return type.

Changes across the codebase:

- `types.rs` — added `template_params: Vec<String>` and
  `template_bindings: Vec<(String, String)>` fields to `MethodInfo`.
- `docblock/tags.rs` — new `extract_template_param_bindings()` function
  that maps `@param T $model`-style annotations to `(T, $model)` pairs.
- `parser/classes.rs` — `extract_class_like_members` now extracts
  method-level template params and bindings from the docblock and stores
  them on each `MethodInfo`.
- `completion/resolver.rs` — `resolve_method_return_types_with_args`
  accepts a `template_subs` map; new helpers `build_method_template_subs`
  and `resolve_arg_text_to_type` resolve argument expressions (`$var`,
  `new Foo()`, `$this->prop`, `ClassName::class`) to concrete type names.
- `completion/variable_resolution.rs` — the `$this->method()` and
  `ClassName::staticMethod()` AST paths now extract argument text and
  pass it through for template (and conditional) resolution.
- `inheritance.rs` — `apply_substitution` promoted to `pub(crate)`.
- `completion/conditional_resolution.rs` — `split_text_args` promoted to
  `pub(crate)`.

Handles: inline chains (`$repo->wrap($user)->`), assignments
(`$result = $this->wrap($user)`), static methods, `new` expressions,
`$this->property` arguments, `@phpstan-template` prefix, nullable and
union-with-null param types, multiple template params, and cross-file
PSR-4 resolution.

### ~~3. Callable / closure variable invocation~~ ✅

Implemented callable/closure variable invocation so that `$fn()->` resolves
the return type when `$fn` holds a closure, arrow function, or a variable
annotated as `Closure(…): T` or `callable(…): T`.

Changes across the codebase:

- `completion/resolver.rs` — `resolve_call_return_types` gained a
  "Variable invocation: `$fn()`" section that tries (1) docblock
  annotation via `find_iterable_raw_type_in_source` +
  `extract_callable_return_type`, then (2) native return type hint
  extraction from closure/arrow-function literal assignments via
  `extract_closure_return_type_from_assignment`.
- `completion/resolver.rs` — new helper
  `extract_closure_return_type_from_assignment` parses `$fn = function(…): T`
  and `$fn = fn(…): T => …` patterns, handling `use (…)` clauses.
- `docblock/types.rs` — new `extract_callable_return_type` function that
  extracts the return type from `Closure(…): T` and `callable(…): T` type
  strings, respecting `<…>` nesting.
- `completion/variable_resolution.rs` — `resolve_rhs_expression` handles
  `$fn()` in assignment RHS by resolving the variable's callable type
  and extracting the return type.

Handles: closure literals with native return type hints, arrow functions,
`use (…)` clauses, docblock `@var Closure(): T` and `@param callable(): T`
annotations, FQN prefixed types, nullable callables, chaining after
`$fn()`, cross-file PSR-4 resolution, inline `@var` annotations, and
property access on return values.

### ~~4. Spread operator type tracking in array literals~~ ✅

Implemented spread operator (`...$var`) type tracking in array literals
so that `$all = [...$users, ...$admins]; $all[0]->` resolves to the union
of element types from all spread variables.

Changes across the codebase:

- `docblock/types.rs` — new `extract_iterable_element_type` function that
  extracts the element type from iterable type annotations (including
  scalar element types, unlike `extract_generic_value_type` which only
  returns class types).  Handles `list<T>`, `array<K, V>`, `T[]`,
  `Collection<K, V>`, nullable and FQN-prefixed types.
- `docblock/mod.rs` — re-exports `extract_iterable_element_type`.
- `completion/array_shape.rs` — `extract_spread_expressions` function
  (already present but unused) extracts `...$var` expressions from array
  literals (`[…]` and `array(…)` syntax).  Made `pub` for test access.
- `completion/array_shape.rs` — `resolve_raw_type_from_assignment` now
  calls `extract_spread_expressions`, resolves each spread variable's raw
  type via `resolve_variable_raw_type`, extracts element types via
  `extract_iterable_element_type`, and merges them into the push-style
  type list so that `build_list_type_from_push_types` produces the
  correct `list<T|U>` union.
- `completion/resolver.rs` — `extract_raw_type_from_assignment_text` has
  the same spread handling, resolving via `find_iterable_raw_type_in_source`
  with fallback to recursive assignment resolution.
- `completion/mod.rs` — `array_shape` module promoted to `pub` for test
  access to `extract_spread_expressions`.

Handles: single and multiple spread variables, `list<T>` / `array<K,V>` /
`T[]` annotations, `@var` and `@param` annotations, `array(…)` syntax,
spread combined with push assignments, spread inside class methods,
deduplication of identical element types, cross-file PSR-4 resolution.

---

## Completion Gaps


## Missing LSP Features

### ~~5. Go-to-implementation (`textDocument/implementation`)~~ ✅

Implemented `textDocument/implementation` so users can jump from an interface
or abstract class name (or a method call typed as one) to all concrete
implementations.

Changes across the codebase:

- `types.rs` — added `interfaces: Vec<String>` field to `ClassInfo` to track
  which interfaces a class/enum implements.
- `parser/classes.rs` — extracts interface names from `implements` clauses on
  classes and enums.
- `parser/ast_update.rs` — resolves interface names to fully-qualified names
  via the use-map and namespace, matching the existing `parent_class` handling.
- `definition/implementation.rs` — new module implementing `resolve_implementation`
  (entry point), `resolve_member_implementations` (method/property on
  interface/abstract), `find_implementors` (scans ast\_map, class\_index,
  classmap), `class_implements_or_extends` (direct + transitive parent chain
  check), and `find_member_position_in_class` (scoped member search to avoid
  false positives when multiple classes share a file).
- `definition/mod.rs` — declares the `implementation` module and updates the
  module-level documentation.
- `definition/member.rs` — promoted `MemberKind`, `is_member_access_context`,
  `extract_member_access_context`, `find_class_file_content`, and
  `find_member_position` to `pub(crate)` for reuse by the implementation module.
- `server.rs` — registers `implementationProvider` capability and wires the
  `goto_implementation` handler with panic-catching.

Handles: interface names, abstract class names, method calls on
interface/abstract-typed variables, transitive implementation via parent
chains, multiple interfaces per class, enum `implements`, filtering out
abstract subclasses (only concrete classes returned), method-level results
limited to classes that actually override the method, cross-file PSR-4
resolution, same-file multi-class scoped member search, classmap string
pre-filter (reads raw files without parsing to skip non-candidates),
embedded stub string pre-filter (cheap `contains` check on static strings),
and PSR-4 directory scanning for classes not in the classmap.

`find_implementors` scans five phases:

1. **ast_map** — all already-parsed classes (open files + previously loaded)
2. **class_index** — FQN → URI entries not yet in ast_map
3. **classmap files** — iterates unique file paths from the classmap,
   skips files already in ast_map, reads raw source for a cheap string
   pre-filter (`contains(target_short)`), then parses matching files
   via `parse_and_cache_file` and checks every class in each file
4. **embedded stubs** — static strings baked into the binary, string
   pre-filtered then lazy-parsed and cached
5. **PSR-4 directory walk** — recursively collects all `.php` files under
   every PSR-4 root directory, skips files already covered by the classmap
   or ast_map, applies the same string pre-filter → parse → check pipeline.
   This discovers classes in projects that haven't run
   `composer dump-autoload -o`.

Helper additions:
- `collect_php_files(dir)` — standalone recursive `.php` file walker
- `parse_and_cache_file(path)` — parses a PHP file on disk, resolves
  parent/interface names, and caches the results in `ast_map`/`use_map`/
  `namespace_map` following the same pattern as `find_or_load_class`

**Known limitations:**

- **No transitive interface inheritance** — if `InterfaceB extends InterfaceA`
  and `ClassC implements InterfaceB`, go-to-implementation on `InterfaceA`
  will not find `ClassC`.  The transitive check walks `parent_class` chains
  but does not walk interface-extends chains.  Fix: in
  `class_implements_or_extends`, when checking a class's `interfaces` list,
  load each interface via `class_loader` and recursively check whether it
  extends the target interface (with a depth bound).

- **Short-name collisions** — `class_implements_or_extends` matches
  interfaces by both short name and FQN (`iface_short == target_short ||
  iface == target_fqn`).  Two interfaces in different namespaces with the
  same short name (e.g. `App\Logger` and `Vendor\Logger`) could produce
  false positives.  Similarly, `seen_names` in `find_implementors`
  deduplicates by short name, so two classes with the same short name in
  different namespaces could shadow each other.  Fix: always compare
  fully-qualified names by resolving both sides before comparison.

---

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