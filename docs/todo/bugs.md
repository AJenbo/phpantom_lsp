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

### B182. `non-falsy-string` still accepts a literal string that can be `0`

**Impact: Low · Complexity: Low**

The `non-falsy-string` supertype arm in `php_type/subtype.rs` accepts
`non-empty-literal-string`, `non-empty-lowercase-string`, and
`non-empty-uppercase-string` as subtypes, carried over unpruned from the
`non-empty-string` arm. All three are inhabited by the literal `"0"`,
which is falsy, so a `non-empty-literal-string`-typed value passed where
`non-falsy-string` is required produces no diagnostic even though `"0"`
would violate it. False negative only; pre-existing in the shared arm
before it was split, and still present in the split-off strict arm.

## Standard-library return types

No outstanding items.

## Narrowing

### B184. A negated `get_class()` check over-narrows and drops valid subclasses

**Impact: Medium · Complexity: Low-Medium**

`narrow_type_by_instanceof`'s negated branch
(`type_engine/types/narrowing/instanceof.rs`) ignores
`extraction.exact`, so a negated exact-class check is treated the same
as `!instanceof`, which wrongly excludes subclasses that would in fact
pass the exact check:

```php
/** @var list<Dog|Puppy|Cat> $a  (Puppy extends Dog) */
$b = array_filter($a, fn ($v) => get_class($v) !== Dog::class);
// reports array<int, Cat>; Puppy's get_class() is "Puppy", so it should survive too
```

The positive branch already handles `exact` correctly; only the negated
branch needs the same check.

### B185. A union member's generic arguments are lost when narrowed by `instanceof`

**Impact: Medium · Complexity: Medium**

`instanceof_member` (`type_engine/types/narrowing/instanceof.rs`) calls
`member.class_name()`, which only matches `TypeKind::Named`. A `Generic`
union member such as `Collection<User>` falls into the `else` arm and is
replaced by the bare checked class (`Collection`) instead of reaching the
`is_subtype_of_named` path that would preserve its type arguments:

```php
/** @var list<Collection<User>|string> $a */
$b = array_filter($a, fn ($v) => $v instanceof Collection);
// $b is array<int, Collection>; should be array<int, Collection<User>>
```

`$b[0]->first()` then loses the `User` type. A non-generic member
escapes this path via an early subtype check elsewhere; only the
union-narrowing path is affected.

### B186. A negated loose null comparison is narrowed as if it were strict

**Impact: Low · Complexity: Low**

`try_extract_null_comparison` (`type_engine/types/narrowing/guards.rs`)
collapses `!==` and `!=` to the same `Some(false)` result in its negation
arm, so `!($v != null)` (equivalent to the loose `$v == null`) is treated
as proving `null`, even though the function's own direct-comparison arm
correctly refuses to narrow `$v == null` (loose equality also matches
`''`, `0`, `[]`). Only the negated spelling slips through:

```php
/** @var list<?string> $xs */
$kept = array_filter($xs, fn ($v) => !($v != null));
// reports array<int, null>; kept entries also include ''
```

Contrived spelling, so low real-world reach, but the negation arm should
distinguish `NotIdentical` from `NotEqual` the same way the direct arm
distinguishes `Identical` from `Equal`.

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

### B191. The property hook call rewrite matches any class, not just `parent`

**Impact: Medium · Complexity: Low**

`extract_property_hook_call` (`symbol_map/extraction/expressions/calls.rs`)
rewrites any `X::$y::get()` / `X::$y::set(...)` static call into a
property-hook invocation, but PHP 8.4 only defines this syntax for
`parent::`. The extraction never checks `property_access.class`, so
legitimate code using the same token shape for an unrelated purpose is
hijacked:

```php
Registry::$instance::get('service'); // a real static property + static call
```

Here the `get` call's `MemberAccess` span is dropped entirely (hover and
go-to-definition on `get` answer nothing, and its unknown-method/
argument-count checks are skipped), and `$instance` is emitted as
`is_static: false`, so hover shows a static property as an instance one.
Restrict the match to a `parent` class reference.

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

### B217. A `mixed`-returning accessor loses the type its arguments decide

**Impact: Low-Medium · Complexity: High**

Find References resolves a member access's receiver to a type and keeps
the access only when that type is in the target's hierarchy. A receiver
whose type cannot be resolved is skipped, which is the right call over
matching by spelling, but it means a genuine reference through an
untyped value is silently dropped. Rename runs the same search, so such
a site is also left un-rewritten.

What remains is the case where the receiver's type is decided by the
*arguments* of a call to a user function that declares no return type of
its own:

```php
class Sudo {
    /** @return mixed Value of $object->property */
    public static function fetchProperty($object, string $property)
    {
        $prop = self::getProperty(new \ReflectionObject($object), $property);

        return $prop->getValue($object);
    }

    private static function getProperty(\ReflectionClass $refl, string $property): \ReflectionProperty
    {
        return $refl->getProperty($property);
    }
}

function probe(Configuration $config): void {
    $shell = Sudo::fetchProperty($config, 'shell');
    echo $shell::VERSION;  // not counted as a reference to Shell::VERSION
}
```

Measured on the conformance suite's navigation corpus, this is the one
missing reference in `Psy\Shell::VERSION` (20 of 21,
`src/functions.php:383`), and the code above is `Psy\Sudo` verbatim.

The reflected read itself types: `getProperty('shell')` on a
`ReflectionClass`/`ReflectionObject` with a known class yields the
declared type of that property, so the same access written in one frame
is found. Specialising the shape above one step at a time says exactly
where the type is lost:

| The callee, specialised | Inside its body | At the call site |
| ----------------------- | --------------- | ---------------- |
| `static`, no declared return, `Configuration $object`, literal name | `$prop` is `ReflectionProperty<Configuration, 'shell'>` | nothing |
| the same with `@return mixed` | `$prop` resolves | `mixed` |
| `$object` untyped, `string $property` (as psysh writes it) | `$prop` is a bare `ReflectionProperty` | nothing |

So the type already exists inside the body — it just never leaves the
function. Four things are in the way, and the first two are
[T42](type-inference.md#t42-body-return-inference-is-instance-call-only-and-stops-at-return-mixed),
which is worth doing on its own:

- **Body-return inference never runs for a static call.** The fallback
  lives at the tail of `resolve_owner_method_call`;
  `resolve_rhs_static_call` has none. Every link in the chain above is
  `static`.
- **A declared `@return mixed` short-circuits it.** The hint is returned
  before the inference fallback is reached. `mixed` says nothing and
  every type is a subtype of it, so refining it can only narrow.
- **Parameters are seeded from the declaration, not the call site.**
  This is the third row of the table, and it is the real work:
  `seed_params` reads the signature, and the resolution context that
  invokes inference carries neither the resolved argument types nor the
  `content`/`cursor_offset` needed to resolve them. The body-inference
  memo is keyed `(class FQN, method)` and would have to grow the
  argument types too, or one call site's answer gets served to another.
- **The helper's declared `: \ReflectionProperty` erases the binding.**
  Accepting an inferred refinement needs a rule — same base class, adds
  generic arguments, inferred wins — which is sound because the
  refinement is a strict subtype, but it means inference has to run
  where a return type is already declared. That is the performance
  cliff; the tightest gate found so far is to allow it only while
  already inside an inference frame.

Note that 0.9.0 scored 21 of 21 here. The hit came from the name
matching removed in f10aedba, which compared the receiver's variable
name against the short class names in the target hierarchy and paired
`$shell` with `Shell` on spelling alone. That heuristic was the source
of false references and wrong rename edits, so the drop is a loss of a
coincidence, not of a working feature. **Do not restore it.** The fix
is to type the receiver.

Intelephense and Phpactor miss the same line; the DEVSENSE server finds
it.

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
