# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B2. A property assigned inside a guarded `if` keeps its declared type after the block

**Impact: Medium · Effort: Medium**

The lazy-initialisation idiom leaves a property at its declared type
once the `if` closes, so returning it from a method with a narrower
return type is reported as a mismatch:

```php
protected ?AbstractType $instance = null;

public function getType(): ConcreteType
{
    if (!$this->instance instanceof ConcreteType) {
        $this->instance = $this->context->makeConcrete();  // : ConcreteType
    }

    return $this->instance;   // false positive: ?AbstractType
}
```

Both paths out of the `if` give `ConcreteType`: the implicit else is the
negation of the condition, and the then-branch assigns one. The two need
to be merged the way the forward walker already merges branch outcomes
for local variables. Property keys are seeded into the walker's scope for
`instanceof` narrowing (`seed_property_keys_into_scope`), so the missing
piece is recording a property *assignment* into that scope and joining
the branches at the end of the block, not new machinery.

Surfaced by removing the supertype-where-subtype escape hatch from the
argument/return compatibility layer, which is what made the stale type
visible. Reproducible in an open-source project: PDepend's
`ASTClassReference::getType()` and `ASTTraitReference::getType()` are the
two remaining diagnostics `analyze` reports there, and PHPStan at level
max reports neither.

**Where to look:** `type_engine/variable/forward_walk/` (branch merging
and `seed_property_keys_into_scope`) and
`type_engine/resolver/property_narrowing.rs`.


#### B3. A property write overrides the declared type even when it goes through `__set`

**Impact: Low-Medium · Effort: Medium**

`$obj->prop = expr` records the assigned type under a property-path key
(`assignment.rs`, the `Expression::Access(Access::Property(_) | ...)`
branch) so that a later read of the same path resolves through the
assignment "rather than the declaring class's declared property hints".
That is right for a real declared property, and it is what makes
`stdClass` chains resolve. It is wrong when the write dispatches to
`__set`, which is free to transform, reroute, or drop the value instead
of storing it as given. `__get` then decides what a read returns, and the
recorded write type has no authority over it.

```php
/** @template TData of array */
class DataBag {
    /** @param TData $data */
    public function __construct(private array $data) {}
    /** @return TData[K] */
    public function __get(string $property) { return $this->data[$property]; }
    /** @param TData[K] $value */
    public function __set(string $property, $value) { /* may store anything */ }
}

/** @extends DataBag<array{a: int, b: string}> */
class FooBag extends DataBag {}

$foo = new FooBag(["a" => 5, "b" => "hello"]);
$foo->a = 9;
$a = $foo->a;   // PHPantom: 9      Psalm: int (via __get's TData[K])
```

The write should not be recorded when the property is undeclared on the
subject's class *and* that class declares `__set`; the read should keep
resolving through `__get`. Psalm and PHPStan both treat magic property
writes as opaque for this reason.

This predates literal preservation in expression resolution: the recorded
type used to coincide with the declared type (`int` either way), so the
override was invisible. It surfaced when the write started recording `9`.

**Where to look:** the property-assignment branch of
`type_engine/variable/forward_walk/assignment.rs`, and the magic-member
resolution in `virtual_members/`.

**Test:** the `inheritTemplateParamViaConstructor` case in
`tests/psalm_assertions/template_class_template_extends.php` carries two
`// SKIP` markers that clear when this is fixed.

#### B5. "Promote constructor param" leaves an orphaned docblock behind

**Impact: Low · Effort: Low-Medium**

The "Promote property to constructor parameter" code action deletes only
the property declaration's own line, not a `/** @var ... */` docblock
that precedes it:

```php
class Foo {
    /** @var int */
    private int $bar;   // deleted

    public function __construct(private int $bar) {}   // promoted here
}
```

Applying the action leaves the now-orphaned `/** @var int */` sitting
above the constructor. `property_delete_start`
(`code_actions/promote_constructor_param.rs`) is computed with
`find_line_start`, which only walks back to the previous newline on the
same line — it cannot reach a comment on an earlier line, by design (it
is also used for the non-docblock leading-whitespace case). There is no
current test covering a property with a preceding docblock.

**Where to look:** `build_promotion_candidate` in
`code_actions/promote_constructor_param.rs` — extend
`property_delete_start` to also swallow an immediately-preceding
docblock (mirroring how other code actions in this module, e.g.
`update_docblock.rs`, locate an existing docblock above a declaration).

#### B6. "Add `final`" may insert before a class's opening brace instead of its constructor (needs confirmation)

**Impact: Low-Medium · Effort: Medium**

Found by manual trace while cleaning up comments in
`code_actions/phpstan/new_static.rs`; not yet confirmed with a failing
test, so verify before fixing.

`build_constructor_info` locates where to insert `final ` before an
unmodified constructor. For a constructor with no explicit visibility
keyword, where the enclosing class's opening brace sits alone on its
own line (Allman style):

```php
class Foo
{
    function __construct() {}
}
```

Tracing the offsets by hand: `before_kw.trim_end().rfind('\n')` (line
~516) skips backward past the blank run of whitespace between the
brace line and the `function` keyword's line, landing right *before*
the `{` rather than at the start of `    function …`. The next check
(`line_trimmed.is_empty()`, line ~520-523), meant to detect "no
modifiers on this line, fall back further", instead sees the lone `{`
character as if it were modifier text on the same line as `function`
and skips the fallback branch. The final `decl_start` then points at
the `{` character itself, so inserting `final ` would produce
`class Foo\nfinal {\n    function __construct() {}\n}` — `final`
attached to the brace line, not the constructor — which is invalid
PHP.

**Where to look:** `build_constructor_info` in
`code_actions/phpstan/new_static.rs`. The line/modifier detection at
~514-531 needs to distinguish "text on the function's own line" from
"trailing content from an earlier line that `trim_end()` bridged past
a blank line" — e.g. by checking whether `decl_start_offset` and
`func_kw_offset` are on the same source line (count newlines between
them) rather than only checking whether the slice between them is
non-empty after trimming.

**To confirm:** add a test with a modifier-less constructor whose
class brace is on its own line (as above) and assert the insertion
point lands on the `function` line, not the brace line.

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

#### B12. `resolve_rhs_expression`'s `RHS_EXPR_DEPTH` cap silently returns empty past depth 100

**Impact: Low-Medium · Effort: Medium-High**

Found while cleaning up comments in `type_engine/variable/rhs_resolution/`.
`resolve_rhs_expression` (`src/type_engine/variable/rhs_resolution/mod.rs:346`)
guards against unbounded recursion with a thread-local counter:

```rust
thread_local! {
    static RHS_EXPR_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}
let depth = RHS_EXPR_DEPTH.with(|d| { let v = d.get() + 1; d.set(v); v });
if depth > 100 {
    RHS_EXPR_DEPTH.with(|d| d.set(depth - 1));
    return vec![];
}
```

This is the exact "depth cap papering over unbounded recursion"
anti-pattern this project's own conventions warn against (a hard cap is
a safety net, not a fix — see the performance anti-patterns in
`CLAUDE.local.md`, item 2), and it isn't one of the previously-known
instances (`MAX_RESOLVE_DEPTH`, `MAX_LOOP_DEPTH`,
`MAX_RESOLVE_TARGET_DEPTH`) — `RHS_EXPR_DEPTH` doesn't follow the
`MAX_*` naming convention, so it was easy to miss in a grep sweep for
those names.

When the cap fires, `resolve_rhs_expression` silently returns an empty
`Vec<ResolvedType>` — the caller sees "this expression has no type"
with no indication the resolver bailed rather than genuinely finding
nothing. Since this function is the single shared entry point for RHS
expression resolution (feeding assignment tracking, hover, and
diagnostics type strings per its own doc comment), a pathological or
deeply-nested chain of calls/match/ternary/`??` expressions produces
silently wrong (empty) results rather than a bounded-but-correct one.

**Where to look:** `resolve_rhs_expression` in
`src/type_engine/variable/rhs_resolution/mod.rs` — apply one of the
standard fixes from this project's own anti-pattern list instead of
raising the cap: cache resolved expressions by identity so recursive
resolution of shared sub-expressions is O(1) on re-entry (the approach
PHPStan/Phpactor use), or break cycles with a keyed visited set that
returns a defined partial result rather than an empty one.

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

#### B17. `virtual_members/resolve.rs` reimplements generic substitution without right-alignment

**Impact: Low-Medium · Effort: Low-Medium**

Found while cleaning up comments in `virtual_members/resolve.rs`. The
canonical generic-substitution builder,
`build_substitution_map`/`apply_generic_args` in
`src/inheritance/generics.rs`, right-aligns a short type-argument list
against trailing template parameters when every skipped leading
parameter has a key-like bound (`array-key`/`int`/`string`) — the
universal convention for collection key parameters, via
`right_align_offset` (`src/inheritance/generics.rs:478`). For example
`@extends Collection<User>` against `class Collection<TKey, TValue>`
binds `TValue => User`, not `TKey => User`.

`resolve_class_fully_inner` in `src/virtual_members/resolve.rs` (~line
588) has its own inline copy of this substitution-map building logic,
used while walking the `extends` chain to collect and substitute
`@implements` generics for virtual-member (`@method`/`@property`)
resolution. Its comment claims it mirrors
`resolve_class_with_inheritance`'s logic, but the implementation is a
plain `enumerate()` zip:

```rust
for (i, param_name) in parent.template_params.iter().enumerate() {
    if let Some(arg) = args.get(i) {
        ...
        map.insert(param_name.to_string(), resolved);
    }
}
```

with no `right_align_offset` call. When a class in the chain provides
fewer `@extends`/`@implements` type arguments than the parent
interface's template params, and those params have key-like leading
bounds, this path binds the short argument to the *first* (key)
parameter instead of the trailing (value) parameter that
`build_substitution_map` would choose — the opposite of the rest of
the codebase's convention. This only affects interface-tag merging
during virtual-member resolution (not the main inheritance-merge
path), so the practical impact is `@method`/`@property` synthesis
picking up a misbound generic argument for interfaces reached this
way.

**Where to look:** `resolve_class_fully_inner` in
`src/virtual_members/resolve.rs` (~line 611) — replace the manual
`enumerate()` substitution-map loop with a call to
`crate::inheritance::generics::right_align_offset` (or, better,
refactor to share `build_substitution_map` directly instead of
duplicating its logic), consistent with CLAUDE.local.md's guidance
against parallel type-resolution paths. Add a regression test with a
short `@extends`/`@implements` argument list against an interface with
a key-like leading template param, reached via `@mixin`/`@implements`
tag merging rather than the main inheritance chain.

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

#### B23. Three type-engine resolvers are activated for only four consumers

**Impact: Medium · Effort: Medium**

`activate_body_return_inferrer`, `activate_auth_user_resolver`, and
`activate_validation_rules_resolver` install thread-locals that the type
engine consults while resolving an expression, and all three are
activated at exactly four sites: `hover/mod.rs`, `diagnostics/mod.rs`,
`completion/handler/mod.rs`, and `analyse/run.rs`. Every other feature
resolves the same expression with those resolvers absent, so it gets a
different, poorer answer than hover does for the identical code:
`$request->validated()` is a plain `array` rather than an array shape,
`auth()->user()` is the framework contract rather than the model the
guard is configured with, and a function whose return type is only
knowable from its body has none.

Go-to-definition, find-references, signature help, code actions, rename,
and inlay hints are all on the wrong side of that line. Hovering
`auth()->user()->email` resolves the model and shows the property, while
go-to-definition on the same `email` has nothing to jump to.

This is exactly anti-pattern 4 (consumer-gated caches): a facility that
is live for one consumer and not others makes the two code paths
diverge. Fixing it for one feature at a time would just add a fifth and
sixth activation site.

**Where to change:** `src/type_engine/call_resolution/target_cache.rs`
holds the thread-locals and the three `activate_*` methods. The cheap
version is to bundle the three guards into one request-scope helper on
`Backend` and call it from every LSP entry point rather than four of
them. The version that removes the pattern is to carry the resolvers on
the resolution context the type engine already threads through, so they
are present by construction and no entry point can forget them; that
also settles whether they belong on the `Backend` rather than in
thread-local state.
