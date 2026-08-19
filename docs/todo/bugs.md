# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Complexity** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

## Crashes

No outstanding items.

## Type comparison

No outstanding items.

## Standard-library return types

No outstanding items.

## Narrowing

### B187. Unsetting an array element is not modeled, so a loop can be assumed to run

**Impact: Low · Complexity: Medium**

`unset()` handling in
`type_engine/variable/forward_walk/assignment.rs` only clears a direct
variable, not an element unset (`unset($arr['k'])`). A `non-empty-array`
or shape type therefore survives being emptied out element by element,
so `process_foreach` still treats the loop body as guaranteed to run and
drops the pre-loop sentinel value, producing a wrong post-loop type (a
variable assigned only inside the loop is typed as if the loop always
executed, when the array it iterates was actually emptied first).

## Arithmetic

No outstanding items.

## Symbol resolution

### B196. The chain resolution cache key omits file identity

**Impact: Low-Medium · Complexity: Medium**

The chain cache in `type_engine/resolver/mod.rs` keys a variable-free
subject chain by its bare text (`expr.to_subject_text()`), with no file
identity in the key, while some cache activations span multiple files
(e.g. the reference-counts pending-item loop, and the request-level
guard used by find-references/rename while walking other files). Two
files that each `use` a different class under the same alias and spell
the same method-chain text can have the second file's resolution
poisoned by the first file's cached entry:

```php
// file A: use A\Pen;           file B: use B\Pen;
Pen::make()->write();           Pen::make()->write();  // may resolve against A\Pen
```

### B222. Class reference matching is case-sensitive

**Impact: Medium · Complexity: Medium**

PHP resolves class names case-insensitively, but `class_names_match`
(`references/mod.rs`), the `ReferenceIndexKey::Class` keys, and
`build_class_rename_edit` all compare them case-sensitively:

```php
class Widget {}
$a = new WIDGET();  // renaming Widget to Gadget leaves this calling WIDGET
$b = new Widget();
```

Find References omits the `WIDGET` site and rename leaves it behind,
which breaks the file it was supposed to fix. This is the same defect
fixed for functions in the reference index and `find_function_references`
(where the key is now folded to ASCII lowercase and the comparisons use
`eq_ignore_ascii_case`); classes need the same treatment, but they reach
their edits through the separate class-rename handler, which understands
`use` imports, aliases, and collisions, so the alias and import spellings
have to be folded consistently too. Constants are correctly
case-sensitive in PHP and must stay as they are.

### B218. `new ReflectionProperty(Foo::class, 'bar')` forgets what it reflects

**Impact: Low · Complexity: Medium**

A reflection value built by `ReflectionClass::getProperty('bar')` carries
the class and the property name, so reading it types as the property
declares. Constructing the same value directly does not:

```php
$viaClass = (new \ReflectionClass(Configuration::class))->getProperty('shell');
$viaClass->getValue($config);   // ?Shell

$direct = new \ReflectionProperty(Configuration::class, 'shell');
$direct->getValue($config);     // mixed
```

The two spellings are interchangeable in real code, so the second should
resolve like the first. The binding cannot come from the constructor's
docblock: `class-string<T>|T $class` would bind the class through the
existing machinery, but the `$property` name is a string literal, and a
literal only binds to a `@template` whose bound is a type operator
(`key-of<…>` and friends). Either the two `new`-expression resolution
paths need the same rule the two call paths got, or literal binding has
to be widened to a `@template TName of string`, which is what PHPStan
does for literal string types and would want measuring against the whole
corpus first.

### B224. A stored `preg_match` result loses the groups it matched

**Impact: Medium · Complexity: Medium**

`preg_match` writes its groups into an out-parameter, and the shape the
pattern describes is seeded at the call so a group read types as a
string. Storing the call's *result* in a variable first loses it:

```php
function guarded(string $html): void {
    $m = [];
    if (preg_match('#(a)(b)#', $html, $m)) {
        strrpos($m[1], 'x');       // string — correct
    }
}

function stored(string $html): void {
    $m = [];
    $ok = preg_match('#(a)(b)#', $html, $m);
    strrpos($m[1], 'x');           // null — should be string|null
    if ($ok) {
        strrpos($m[1], 'x');       // null — should be string
    }
}
```

Two separate steps are missing, and the first is the one that produces
the false positive above:

- **The seeding never runs.** `process_pass_by_ref` hands the whole
  statement expression to `seed_pass_by_ref_primitives`, which reads a
  `preg_call` off it; an assignment is not a call, so the out-parameter
  keeps whatever the `$m = []` before it left, and a group read off an
  empty shape is `null` rather than the `string|null` the call leaves.
  `seed_pass_by_ref_in_condition` already looks through an assignment to
  its right-hand side, which is what the statement path needs too.
- **The guard proves nothing.** `apply_preg_match_narrowing` reads the
  call out of the condition, so a condition that only names the variable
  holding the result narrows nothing. The walker already records what a
  boolean stands for when it is an `instanceof` (`VarAssertion`); a
  `preg_match` outcome needs the same treatment, so `if ($ok)` narrows
  `$m` to the matched shape and the `else` branch to the empty one.

Found while fixing the accessor-argument typing: a body read for its
return type now resolves further than it used to, so it reaches group
reads that were previously left unresolved. `HTMLPurifier_Lexer` in the
corpus shows all three false positives.

### B183. A Laravel Folio route is reported as unknown

**Impact: High · Complexity: Medium**

Folio registers routes from the filesystem: a page file under a mounted
directory becomes a route, and `Laravel\Folio\name()` inside the page
names it. The route index only reads registrations it can see in
`routes/`, service providers, and resource declarations, so a Folio
route does not exist as far as any consumer is concerned. The worst
symptom is a false positive:

```php
// resources/views/folio/explore/index.blade.php
use function Laravel\Folio\name;
name('explore');

// anywhere else
route('explore');  // Unknown route: 'explore'
```

Three more follow from the same cause: go-to-definition on the name
answers nothing, completion inside `route('')` omits every Folio route,
and hover falls back to the bare `Route name` label instead of naming
the file that declares it.

The mount points come from `Folio::path(...)` / `Folio::route(...)` calls
in a service provider (`FolioServiceProvider` by convention), and the
name comes from a `name()` call in the page file, imported as
`use function Laravel\Folio\name;`. Both need reading before the route
index can answer for these names. The URI a page maps to is derived from
its path relative to the mount, with `[param]` and `[...param]` segments
becoming route parameters, which is also what route-parameter completion
would need.

Reproduced against a Folio-based Laravel application; `route('home')`
from `routes/web.php` in the same file resolves correctly, so the gap is
specific to filesystem-derived routes.

## Array types

### B188. A concrete `ArrayAccess::offsetGet()` override is ignored on subscript read

**Impact: Low-Medium · Complexity: Medium**

```php
class Pen {
    public function write(): void {}
}

class PlainArrayAccess implements \ArrayAccess {
    /** @var Pen[] */
    private array $items = [];
    public function offsetExists(mixed $offset): bool { return isset($this->items[$offset]); }
    public function offsetGet(mixed $offset): Pen { return $this->items[$offset] ?? new Pen(); }
    public function offsetSet(mixed $offset, mixed $value): void { $this->items[$offset] = $value; }
    public function offsetUnset(mixed $offset): void { unset($this->items[$offset]); }
}

function test(): void {
    $pens = new PlainArrayAccess();
    $pens[0]->write();  // reported: subject type 'TValue' could not be resolved
}
```

`PlainArrayAccess` declares no generics at all and its own `offsetGet()`
returns a concrete `Pen`, but `$pens[0]` resolves through the stub
interface's own `ArrayAccess<TKey, TValue>::offsetGet(): TValue`
signature instead of the subclass's override, leaking the interface's
unbound template parameter name as a literal, unresolvable type. This
reproduces standalone with a single class and a single subscript
read — no second `ArrayAccess` implementer or generic binding is needed
to trigger it, though a file with more than one `ArrayAccess`
implementer changes which specific subscript expression the diagnostic
lands on, suggesting the wrong-signature lookup is cached or keyed by
something coarser than the receiver class.

Whatever resolves an `offsetGet()` call for `$obj[$key]` needs to prefer
the receiver's own (or nearest-ancestor's) concrete method the same way
ordinary method-call resolution already does, falling back to the
interface's own signature only when the receiver truly does not
override it.

## Docblock handling

No outstanding items.

## Miscellaneous

### B203. A custom Eloquent builder rebind can cache a degraded result on cache re-entry

**Impact: Medium · Complexity: Medium-High**

When `mark_in_flight(fqn)` reports same-thread re-entry on a custom
builder's FQN, `apply_post_merge_stages` (`virtual_members/resolve.rs`)
is skipped, but the base-inheritance-only rebound class is still
inserted into the persistent resolved-class cache under
`(fqn, generic_args)`. Nothing overwrites it afterwards unless the exact
same specialization is re-requested outside an in-flight guard, so a
later hover/completion/diagnostic on that builder specialization gets a
result missing its virtual members, `@mixin` methods, and Laravel
patches, until the entry is evicted by a file edit. Fix by skipping the
cache insert whenever `apply_post_merge_stages` was skipped.

### B204. A custom Eloquent builder two levels below `Builder` fails to rebind its model

**Impact: Low-Medium · Complexity: Medium**

`rebindable_parent` (`inheritance/mod.rs`) only handles a direct parent
that itself carries the template parameters
(`parent.template_params.len() == new_arg_count`). A two-level chain —
`class AdminUserBuilder extends UserBuilder`, where `UserBuilder extends
Builder` and neither declares its own `@template` — fails the arity
check at the first level (`UserBuilder` has zero params) and silently
degrades to the `TModel of Model` bound, reproducing the exact symptom a
recent fix addressed, one inheritance level deeper.

### B207. A `phpcs.xml` silently switches a Mago-formatted project's formatter to phpcbf

**Impact: Low-Medium · Complexity: Medium**

A recent commit made a `phpcs.xml` (or `phpcs.xml.dist`) certify phpcbf
as the project's formatter, and `External` formatter resolution
(`formatting.rs`) takes precedence over `BuiltIn(mago.toml)`. A project
that lints with PHPCS but formats with Mago (an explicit `[formatter]`
table in `mago.toml`) has its configured formatter silently overridden
by phpcbf, contradicting the "a config file records what a project uses
a tool for" principle applied to Mago in the same release. Needs a
decision on precedence when both an explicit Mago formatter config and a
phpcs ruleset are present.

### B208. A dynamic route-name group is still flagged unknown outside the one shape the fix covers

**Impact: Low-Medium · Complexity: Medium**

The dynamic-group-prefix fix only records an "open prefix" when the
dynamic `Route::name(...)` call is nested inside an *enclosing literal*
group (`virtual_members/laravel/route_names.rs`, guarded by
`!group.name.is_empty()`). Two related shapes still produce the false
positive the fix set out to remove: a top-level dynamic name group with
no enclosing literal group (`Route::name('filament.' . $panelId . '.')
->group(...)` at the top of a routes file), and the legacy array form
(`Route::group(['as' => $dynamic], ...)`, whose value extraction silently
returns `""` for a non-literal name and records nothing open). Both leave
`route('filament.admin.pages.dashboard')`-style calls flagged unknown.

### B209. A nullsafe first hop breaks the deprecated-diagnostic subject cache key

**Impact: Low · Complexity: Low-Medium**

`SubjectCacheKey::build` (`diagnostics/subject_cache.rs`) extracts a
chain's variable name with `find("->")`, so a nullsafe first hop such as
`$a?->b->deprecated()` yields the variable name `"$a?"` instead of `"$a"`.
The def-offset lookup for that malformed name fails, so accesses before
and after a reassignment of `$a` can share one cache entry, risking a
stale type for the deprecation check.

### B210. Undefined-variable diagnostics never check property hook bodies

**Impact: Low-Medium · Complexity: Low-Medium**

The undefined-variable walker (`diagnostics/undefined_variables/mod.rs`)
and `scope_collector/build.rs` both only enumerate `ClassLikeMember::Method`
bodies. Property hook bodies were taught to the symbol map, the forward
walker, and unknown-member diagnostics, but this separate walker still
skips them, so a typo such as `return $vlaue;` inside a `set` hook is
never flagged. No false positives result — the bodies are simply
unchecked — but the coverage gap is now user-visible since hooks are a
supported, documented feature.

### B223. Switching workspace diagnostics off mid-session leaves its results in place

**Impact: Low · Complexity: Medium**

`[diagnostics] workspace` is now read again on a live config reload, but
only in the enabling direction. Turning it off while the native pass is
running does not stop it: `drive_native_pass` only checks the shutdown
flag, so the pass runs to completion and publishes results for files the
user has just said they do not want diagnosed. Turning it off after the
pass finished leaves the stored results reported for the rest of the
session too, and `recompute_workspace_diags_for_closed_file` keeps
updating them on every close (it is gated on
`workspace_diag_pass_started`, not on the setting). Doing this properly
means stopping a running pass, clearing `WorkspaceDiagnostics`, and
telling the editor to drop what it was shown, and re-enabling afterwards
has to be able to start a fresh pass (`workspace_diag_pass_started` is
one-way today).

### B214. A closure inside an arrow-bodied property hook gets no variable snapshot

**Impact: Low · Complexity: Medium**

`walk_property_hook_bodies` (`type_engine/variable/forward_walk/diagnostic_walk.rs`)
assumes an arrow-form hook body is "a single expression with nothing to
assign," but a closure embedded in that expression has its own
parameters that never get a snapshot:

```php
public array $items {
    get => array_map(fn ($p) => $p->format(), $this->items);
}
```

Lookups on `$p` inside the closure return empty, so member diagnostics
inside closures embedded in arrow-bodied hooks are silently skipped
(fail-open) rather than checked.

### B215. A file closed mid-computation can have stale diagnostics reinserted after the close purge

**Impact: Low · Complexity: Medium**

Several diagnostic write-back paths check `open_files` once before
starting a multi-step computation and never re-check before the final
write. `assemble_and_push`'s read-merge-write across the six per-source
diagnostic caches is not atomic, so two concurrent completions (e.g. a
fast-phase worker and an external tool) can interleave such that the
stale merge's write lands last. Separately, the main diagnostic worker
(`diagnostics/mod.rs`) checks `open_files` only before starting
`publish_diagnostics_for_file`; a close landing mid-compute (the fast/slow
phases can take seconds) lets the tail of that computation re-insert
`last_fast`/`last_full`/`result_ids` after `clear_diagnostics_for_file`
has purged them. Both self-heal on the file's next open/edit, but until
then a closed file's cache holds diagnostics that should have been
cleared.
