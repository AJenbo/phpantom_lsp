# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B1. `analyze` flags framework Artisan commands as unknown

**Impact: Medium · Effort: Low**

`phpantom_lsp analyze` reports `Unknown command: 'queue:work'` for a
command the framework itself ships. Reproduce by adding
`Artisan::call('queue:work');` to `examples/laravel/app/Demo.php` and
running a **release** build of
`analyze --project-root examples/laravel` (a debug build runs a reduced
collector list that excludes the Laravel string-key checks, so the
false positive is invisible there).

The cause is which files populate the command index. The LSP's
`initialized` handler calls `build_laravel_command_index`, which scans
the whole FQN → URI index — vendor packages included. The headless
pipeline in `analyse/run.rs` never calls it, and instead gets entries
only from `refresh_laravel_command_index`, which `update_ast` fires per
parsed file. `analyze` parses user files, so the index ends up holding
the project's own commands and nothing else. Since it is then non-empty,
the "index is empty, discovery must have failed" guard in
`collect_invalid_laravel_string_key_diagnostics` does not fire, and every
vendor command name is reported as unknown.

`build_laravel_macro_index` is missing from the same block, with the same
shape of consequence: only macros registered in parsed user files are
recovered, so a macro registered by a vendor package's service provider
can produce a false-positive unknown member.

**Where to change:** call `build_laravel_command_index` and
`build_laravel_macro_index` from the Laravel block in `analyse/run.rs`,
next to `build_laravel_date_class`, `build_provider_resources`, and
`build_laravel_morph_map_index`.


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

#### B4. `analyze` reports Blade type diagnostics six lines too early

**Impact: Medium · Effort: Low**

Diagnostics that a Blade template reports through the type-mismatch
passes land six lines above the code that produced them, so the CLI
points at unrelated markup:

```
resources/views/pages/invoice.blade.php
  545   Argument 1 ($amount) expects Acme\Decimal\Decimal, got Decimal<bool>
```

The call is on line 551. The offset is exactly `blade::PROLOGUE_LINES`,
and it is a double translation. `Backend::offset_range_to_lsp_range`
(`diagnostics/mod.rs`) already maps a Blade file's virtual-PHP range back
to Blade coordinates via `map.php_to_blade`, and then `analyse/run.rs`
maps the same range a second time:

```rust
let line = if let Some(ref map) = source_map {
    map.php_to_blade(d.range.start).line + 1
} else {
    d.range.start.line + 1
};
```

The Laravel string-key diagnostics (`invalid_laravel_view`,
`invalid_laravel_route`, …) are unaffected because they build their range
with the *free* `offset_range_to_lsp_range(content, …)`, which does no
Blade translation, so `run.rs` supplies the only one they need. Every
pass that goes through the `&self` method — the three `type_mismatch_*`
collectors, `unknown_member`, `unknown_variable` — is shifted.

The LSP path does not double-translate, so this is `analyze`-only.

The fix is to pick one owner for the translation. Having every collector
emit Blade coordinates (what the `&self` method already does) and
dropping the `php_to_blade` call in `run.rs` is the smaller change, but
it needs an audit that no collector still emits virtual-PHP coordinates.

**Where to look:** `src/analyse/run.rs` (the `source_map` branch),
`Backend::offset_range_to_lsp_range` and the free
`offset_range_to_lsp_range` in `src/diagnostics/mod.rs`.

**Reproduce:** point `analyze` at any project whose Blade templates
trigger a `type_mismatch_argument`, and compare the reported line with
the source.

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

#### B7. Eloquent string-completion's backward paren scan has no bound

**Impact: Low · Effort: Low**

Found while cleaning up comments in `completion/eloquent_string.rs`.
`detect_string_call_context` calls `find_matching_open_paren` on
`content[..quote_pos]` (everything in the file before the cursor's
opening quote) with no length cap. The function scans backward byte by
byte tracking bracket depth; if the preceding source has an unbalanced
`)`/`]` before the real matching `(` — e.g. the cursor's string
literal is itself inside a malformed/mid-edit expression — depth never
returns to 0 and the scan runs all the way to the start of the file.

This mirrors a case already fixed elsewhere in the codebase: the
`function`-keyword backward scan in
`code_actions/phpstan/add_throws.rs`'s `find_enclosing_docblock` had
the same unbounded-backward-scan shape and was capped at 2000 bytes.
`find_matching_open_paren` has no equivalent cap, and it is on the
completion hot path (triggered on every keystroke inside a string
literal argument), so a large file with an unbalanced bracket before
the cursor pays for an O(file size) scan on every trigger.

**Where to look:** `find_matching_open_paren` in
`completion/eloquent_string.rs` — give the backward scan a byte-offset
floor (matching the pattern and rationale already used in
`add_throws.rs`) so pathological/mid-edit source can't force an
unbounded scan.

#### B8. `extends` keyword completion offered inside `enum` declaration headers

**Impact: Low · Effort: Low**

Found while cleaning up comments in `completion/context/keyword_completion.rs`.
`build_keyword_context` sets
`in_extends_declaration_header: decl_kind.is_some()`, which is `true`
whenever `decl_kind` is `Class`, `Interface`, *or* `Enum`. PHP enums
cannot use `extends` (only `implements`), so typing `enum Foo ext|`
offers an `extends` completion that will always produce invalid PHP.
`in_implements_declaration_header` a few lines below already gets this
right, restricting itself to `Class | Enum`.

**Where to look:** `build_keyword_context` in
`completion/context/keyword_completion.rs` — change
`in_extends_declaration_header` to
`matches!(decl_kind, Some(DeclarationHeaderKind::Class | DeclarationHeaderKind::Interface))`,
matching the pattern already used for `in_implements_declaration_header`.
No existing test exercises this path; add one for `enum Foo ext|` (no
completion) and `interface Foo ext|` (completion offered, extends
existing coverage of the Class case).

#### B9. `class_level_signature_eq` never compares `template_param_defaults`

**Impact: Low · Effort: Low**

Found while cleaning up comments in `types/mod.rs`. `ClassInfo::template_param_defaults`
(`@template T = default` clauses) feeds conditional-return-type default
evaluation, per its own doc comment. `class_level_signature_eq` compares
`template_params` and `template_param_bounds` but not
`template_param_defaults`:

```rust
|| self.template_params != other.template_params
|| self.template_param_bounds != other.template_param_bounds
// template_param_defaults is missing here
|| self.extends_generics != other.extends_generics
```

If only a `@template T = default` value changes (e.g.
`@template TAsync of bool = false` → `= true`) with no other tracked
field changing, `signature_eq` returns `true`, so a cache entry keyed on
it is not evicted and conditional return types depending on that default
can resolve stale. Unlike `links`/`see_refs` (legitimately excluded as
display-only), there is no comment or rationale suggesting this
exclusion is intentional.

**Where to look:** `class_level_signature_eq` in `src/types/mod.rs`
(~line 2165) — add `|| self.template_param_defaults != other.template_param_defaults`
alongside the `template_param_bounds` comparison.

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

#### B11. `type_engine/subject_resolution.rs` duplicates `util::resolve_to_fqn` with a subtle behavioral difference

**Impact: Low · Effort: Low**

Found while cleaning up comments in `type_engine/subject_expr.rs`.
`type_engine/subject_resolution.rs` has its own private
`resolve_to_fqn(name, use_map, namespace)` (~line 120) that duplicates
the public `crate::util::resolve_to_fqn` (`src/util.rs:41`) instead of
calling it. The two differ in one place: when a short name resolves via
`use_map`, the `type_engine` copy does
`fqn.trim_start_matches('\\').to_string()` on the matched value, while
`util::resolve_to_fqn` returns `fqn.clone()` unmodified. If a `use_map`
entry is ever stored with a leading `\` (worth checking how entries are
populated — some paths in this codebase do store FQNs with a leading
backslash), the two functions return different strings for the same
input, and only one of them strips it.

This is the kind of parallel resolution path the project's type-engine
conventions warn against: a second implementation of "resolve a name to
its FQN" that can silently drift from the canonical one in `util.rs`.
It is currently used for two call sites in `subject_resolution.rs`
(parent-class and general subject FQN resolution feeding `SubjectExpr`
resolution), not the main shared `resolve_rhs_expression` pipeline, so
the blast radius is contained, but a future fix to `util::resolve_to_fqn`
(e.g. a `use_map` normalization change) would not automatically apply
here.

**Where to look:** `resolve_to_fqn` in `src/type_engine/subject_resolution.rs`
— replace its two call sites with `crate::util::resolve_to_fqn` and
delete the local duplicate, after confirming `use_map` entries are
consistently normalized (with or without a leading `\`) so removing the
`trim_start_matches` step doesn't change behavior for either call site.

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

#### B14. `array_pop`/`array_shift` on nested containers resolve to the container type

**Impact: Low-Medium · Effort: Low**

Found while cleaning up comments in
`type_engine/variable/raw_type_inference.rs`. For element-extracting
functions (`ARRAY_ELEMENT_FUNCS`: `array_pop`, `array_shift`, etc.),
`resolve_rhs_function_call` in
`src/type_engine/variable/rhs_resolution/calls.rs` (~line 845) tries
`resolve_array_func_element_type` first, but only returns its result
when `type_hint_to_classes_typed` resolves the element type to at
least one class. When the element type is not class-like — e.g.
popping a `list<list<int>>` yields element type `list<int>` — the
correct result is discarded and control falls through to the
`resolve_array_func_raw_type` branch, whose `ARRAY_ELEMENT_FUNCS` arm
returns the *input container type unchanged* as a type-string-only
result.

So `$row = array_pop($matrix)` with `$matrix: list<list<int>>` resolves
`$row` to `list<list<int>>` instead of `list<int>`, and
`foreach (array_pop($matrix) as $x)` resolves `$x` to `list<int>`
instead of `int` — one level of unwrapping is missed whenever the
element type doesn't name a class. The simple class case
(`array_pop($users)` on `list<User>`) is unaffected because the
element-type branch resolves `User` and returns early.

**Where to look:** the element-type branch in
`resolve_rhs_function_call` should return the element type as a
type-string-only result (mirroring what the raw-type branch below it
already does for unresolvable-but-informative types) instead of
falling through. The `ARRAY_ELEMENT_FUNCS` arm of
`resolve_array_func_raw_type` then only fires when no element type
could be computed at all; whether it should keep returning the
container at that point is worth revisiting at the same time.

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

#### B16. Cursor-position narrowing inside a `for` condition isn't applied

**Impact: Low · Effort: Low**

Found while cleaning up comments in `type_engine/variable/forward_walk/mod.rs`.
`walk_body_forward`'s cursor-narrowing pass applies `instanceof`/guard
narrowing when the cursor sits inside an `if` or `while` condition:

```rust
match stmt {
    Statement::If(if_stmt) => { ... apply_cursor_ternary_narrowing(if_stmt.condition, ...) }
    Statement::While(while_stmt) => { ... apply_cursor_ternary_narrowing(while_stmt.condition, ...) }
    _ => {}
}
```

There is no `Statement::For` arm, so the same narrowing never applies
inside a `for` loop's condition list, e.g.:

```php
for ($e = $iter->current(); $e instanceof Foo && $e->x; $e = $iter->next()) {
    // cursor on `$e->x` above doesn't get instanceof narrowing
}
```

`Statement::For`'s `conditions` field is `&[Expression]` (PHP allows
comma-separated conditions, unlike `if`/`while`'s single expression),
so the fix isn't a direct copy-paste of the `If`/`While` arms — each
condition in the list needs the cursor-containment check, and (per PHP
semantics) only the last one's boolean value controls loop
continuation, though any of them could be the one the cursor is on.

**Where to look:** `walk_body_forward` in
`src/type_engine/variable/forward_walk/mod.rs`, the cursor-narrowing
`match stmt` block — add a `Statement::For` arm that iterates
`for_stmt.conditions` and applies `apply_cursor_ternary_narrowing` to
whichever condition contains the cursor.

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

#### B22. A `rules()` method supplied by a trait is not read

**Impact: Low · Effort: Low**

`collect_rules_method` in
`src/virtual_members/laravel/validation_rules.rs` only looks at
`Node::Class`, so a `FormRequest` that gets its `rules()` from a trait
offers no request-input keys and no `validated()` shape. The parent-chain
walk covers `extends`, but nothing follows a `use SomeTrait;`.

**Where to change:** resolve the class's used traits (the class index
already carries them) and read `rules()` out of the trait's file with the
same `rules_from_class_source` pass, keeping the trait's own file as the
entry `origin` so go-to-definition lands on the trait.
