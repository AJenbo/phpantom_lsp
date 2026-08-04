# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B10. Extract Function's by-reference-write safety check can never trigger

**Impact: Low-Medium · Effort: Medium**

Found while cleaning up comments in `scope_collector/`. `RangeClassification::reference_writes`
(`src/scope_collector/scope_map.rs:172`, documented as "Variables that
are written by reference (`&$var`) inside the range") is declared,
sorted at the end of `classify_range`, and consumed as a safety gate in
Extract Function:

```rust
// code_actions/extract_function/mod.rs:198
if scope_map.uses_reference_params() && !classification.reference_writes.is_empty() {
    return None;
}
```

but nothing in `classify_range` ever pushes into it — confirmed via
`git log -S "reference_writes"` that no commit has ever added a
`.push()` call since the field was introduced. Since it is always
empty, this guard (meant to block extracting a range that writes to a
by-reference variable, which would silently break the reference
semantics once moved into a new function scope) can never fire.

The collector already tracks the pieces needed to populate it
correctly: `AccessKind` on each `VarAccess`, and the `ByRefResolver` /
`ByRefCallKind` machinery in the same module that resolves which call
arguments bind by reference. `classify_range` needs to identify
accesses inside `[start, end)` that write through a `&$var` binding
(a reference parameter, a `foreach (... as &$v)`, or an argument passed
to a by-ref parameter per the resolver) and populate `reference_writes`
with those variable names.

**Where to look:** `classify_range` in `src/scope_collector/scope_map.rs`
— the `parameters`/`return_values`/`locals` classification loops already
show the pattern for iterating `self.accesses` within the range; add an
equivalent pass keyed on by-reference write accesses.

#### B13. The hover scope cache is populated but never read

**Impact: Medium · Effort: Medium**

Found while cleaning up comments in
`type_engine/variable/forward_walk/diagnostic_cache.rs`. The hover
scope cache banner there claims that after the first hover walks a
method body once, "subsequent hovers on the same file content look up
the pre-computed snapshots in O(log N) time via a `BTreeMap::range`
search — no re-walk at all." The lookup path does not exist. The only
accesses to `HoverScopeCache.methods` in the entire codebase are
`contains_key` (`hover_scope_has_method`) and `insert`
(`populate_hover_scope_cache_for_method`); no code ever reads the
stored `ScopeSnapshotMap` values back.

The populate site in `resolve_in_method_body`
(`src/type_engine/variable/forward_walk/mod.rs`, the "Hover scope
cache" block) also carries a comment claiming the population benefits
"diagnostics member-access lookups via `lookup_diagnostic_scope`", but
that function reads the thread-local `DIAGNOSTIC_SCOPE`, which the
temporary guard in the same block clears immediately after the
snapshots are harvested into the hover cache.

Net effect: the cache makes hover strictly slower, not faster. The
first hover that reaches a given method body performs two walks (the
full-body population walk plus the standard walk that actually answers
the request) instead of one, and the harvested snapshot map sits in
thread-local storage as dead memory until the content hash changes.
The only thing the cache prevents is repeating its own useless
population walk. A lookup path was presumably lost in a refactor, or
was never wired up after the LHS-assignment-hover problem described in
the populate-site comment was discovered.

**Where to look:** either wire up the read path (a
`lookup_hover_scope(method_span_start, var_name, offset)` used by the
member-access/hover consumers that can tolerate statement-start
snapshots, keeping the standard walk only for the LHS-assignment case
the comment describes), or delete the cache entirely
(`activate_hover_scope_cache`, `is_hover_scope_cache_active`,
`hover_scope_has_method`, `populate_hover_scope_cache_for_method`, the
`HOVER_SCOPE_CACHE` thread-local, the activation call in
`type_engine/variable/resolution.rs`, and the population block in
`forward_walk/mod.rs`). Measure hover latency on a hover-heavy file
(the banner's own motivating case: a test file with 80+ `assertType()`
calls) before choosing — if the O(n²) problem the cache was built for
is real, the read path is the right fix.

#### B15. `IN_CLOSURE_THIS_OVERRIDE` is a coarse boolean re-entry guard

**Impact: Low · Effort: Medium**

Found while cleaning up comments in
`type_engine/variable/closure_resolution.rs`. The
`IN_CLOSURE_THIS_OVERRIDE` thread-local guarding
`find_closure_this_override` is a `Cell<bool>` — exactly the coarse
boolean re-entry guard shape the project's performance anti-patterns
warn about (item 5). It cannot distinguish re-entry on the *same*
closure (the `$this`-receiver cycle it exists to break) from
legitimate nested work on a *different* closure: with a closure inside
a closure where both call sites declare `@param-closure-this`, the
inner resolution short-circuits to `None` and silently falls back to
`current_class`, so `$this` inside the inner closure resolves to the
wrong type.

**Where to look:** `find_closure_this_override` in
`src/type_engine/variable/closure_resolution.rs`. Key the guard by the
entity being resolved (e.g. the closure's span offset or the call-site
span) in a small visited set, as `RESOLVING` does for class FQNs, so
only genuine same-entity cycles short-circuit.

#### B18. Override completion offers `final` parent methods

**Impact: Medium · Effort: Medium**

`collect_overridable_methods` in
`completion/context/override_completion.rs` filters private, magic, and
virtual methods, but nothing filters `final`. A `final public function
onLock()` on a parent is therefore offered as an override candidate, and
accepting the completion inserts a declaration PHP rejects outright
("Cannot override final method Base::onLock()").

Both override entry points are affected: the `function get|` path and
the class-body-root path, where it is more visible because the inserted
snippet is a complete declaration rather than just a name.

**Where to look:** `MethodInfo` has no `is_final` field — `is_final`
exists only on `ClassInfo`. The fix is to record the method modifier
during parsing (`method.modifiers` is already read for `is_abstract` and
`is_static` at `parser/classes.rs:1298`) and skip final methods in
`MethodCollector::push_from_class`. Adding the field touches every
`MethodInfo` construction site (~30 in `src/`, plus test fixtures), so
it is a mechanical but wide change; keep it a task of its own.

Note that a `final` parent method should still be offered by *member
access* completion (`$obj->onL|`) — only override candidates need the
filter.

#### B19. Override completion drops `readonly` from a redeclared property

**Impact: Medium · Effort: Medium**

At the class-body root, a parent's `public readonly string $onName` is
offered with the inserted declaration `public string $onName;`. PHP
rejects that: "Cannot redeclare readonly property A::$x as non-readonly
B::$x". Redeclaring it *as* readonly is legal (verified on PHP 8.5), so
the fix is to carry the modifier through, not to filter the property
out.

**Where to look:** `PropertyInfo` has no `is_readonly` field. The
modifier is currently recovered by a local re-parse in
`code_actions/generate_getter_setter.rs` (`has_readonly`), which shows
where to read it from during parsing. Once `PropertyInfo` carries it,
`build_property_override_completions` should emit `readonly ` after the
visibility keyword whenever `include_declaration` is set. Same blast
radius caveat as [B18](#b18-override-completion-offers-final-parent-methods):
adding the field touches every `PropertyInfo` construction site.

#### B20. A class constructed from a string literal picks up a `bool` generic argument

**Impact: Medium · Effort: Medium**

`new \Acme\Decimal\Decimal('0.00')` resolves to `Decimal<bool>`, and the
value is then rejected against a plain `Decimal` parameter:

```
Argument 1 ($amount) expects Acme\Decimal\Decimal, got Decimal<bool>
```

The generic argument is nonsense: the class is constructed from a string
literal and nothing in the call binds a `bool`. Because the reported type
is wrong, the diagnostic is a false positive, and it accounts for the
argument mismatches the analyzer reports on large Laravel projects.

Two contributing factors are worth separating when it is fixed:

1. Whatever binds `bool` as the template argument of a class constructed
   from a string literal. Start at `classify_template_binding` /
   `remap_inherited_ctor_subs` in
   `type_engine/variable/rhs_resolution/instantiation.rs`, and check what
   the class's own `@template` bound resolves to when the constructor
   argument is a literal.
2. `is_type_compatible`'s unloadable-short-name escape hatch only
   inspects `TypeKind::Named`, so a `Generic` whose base name cannot be
   loaded (here `Decimal` written without its namespace) skips the hatch
   and is compared anyway. Widening the hatch does not fix the wrong
   type, but it stops a name the project cannot even load from producing
   a mismatch.

**Reproduce:** point `analyze` at a project that constructs a generic
class from a string literal and passes it to a parameter typed with the
bare class.

#### B21. Completion's backward scan is blind to comments

**Impact: Low · Effort: Medium**

`scan_back_to_opener` in `src/completion/source/helpers.rs` walks
backwards from the cursor over brackets, parentheses, braces, and string
literals, but does not skip `//`, `#`, or `/* … */` comments. A stray
bracket or quote in a comment between the call and the cursor unbalances
the walk, and the completion drops out:

```php
route('users.show', [   // TODO: check (parameters)
    '|' => 1,
]);
```

It fails closed (no suggestions rather than wrong ones), which is why it
is low impact. The obstacle is that a backwards scan cannot tell a `//`
inside a string from the start of a comment without a forward pass, so
the fix likely means scanning the enclosing statement forward once and
masking comments before the backwards walk, the way the Blade
preprocessor masks non-PHP text.

#### B49. The type-engine resolvers are ambient thread-local state

**Impact: Low · Effort: Medium**

`Backend::activate_type_engine_resolvers` installs the body-return
inferrer, the auth user resolver, the validation rules resolver, and the
callable target cache into thread-locals. Every LSP request activates it
at one chokepoint (`Backend::with_file_content`), and the handlers that
fetch their own file content (completion, completion resolve, code
action resolve, the diagnostic pass, the `analyse` CLI) each call it
themselves. That is enough for every feature to resolve an expression
with the same facilities available, which is what made hover and
go-to-definition disagree, but the resolvers are still ambient state
rather than part of the resolution context the type engine already
threads through: a new entry point that fetches its own content can
forget them, and nothing in the type engine can tell a missing resolver
from a genuine absence of an answer.

**Where to change:** `src/type_engine/call_resolution/target_cache.rs`
holds the thread-locals and the bundled activation;
`call_resolution/return_types.rs` reads them. Carrying them on
`ResolutionCtx` (`src/type_engine/resolver/context.rs`) instead would
make them present by construction, at the cost of touching the ~35
places that build a context. It also settles whether they belong on the
`Backend`: each closure captures nothing but a `Backend` clone, so the
closure indirection buys nothing once the context is available where the
resolver is read.
