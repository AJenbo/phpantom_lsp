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

### B189. Renaming a `define()`-declared constant leaves the `define()` call unrenamed

**Impact: High · Complexity: Medium-High**

`ConstantReference` spans are only emitted for top-level `const`
declarations, `use const` imports, and `ConstantAccess` sites
(`symbol_map/extraction/statements.rs` and related). A constant
registered via `define('FOO', ...)` produces no span for the string
argument, even though `constant('FOO')`, `FOO`, and `function_exists`
style lookups resolve it via the parser's symbol table. Renaming started
from any use site of a `define()`-declared constant therefore rewrites
every use but leaves the `define()` call itself untouched, producing
code that no longer defines the constant it now reads:

```php
define('FOO', 1);
echo FOO; // rename FOO -> BAR rewrites this to BAR...
// ...but define('FOO', 1) above is left as-is: BAR is now undefined at runtime
```

The docblock on `SymbolKind::ConstantReference` already claims `define()`
names are covered; they are not. Fix belongs in symbol-map extraction of
`define()` call arguments.

### B190. Reference-count hints read zero for a global function called from namespaced code

**Impact: Medium · Complexity: Medium**

`symbol_name_keys` (`reference_index.rs`) credits a call's reference
count to its *resolved* FQN only. Because `mago-names` unconditionally
qualifies an unqualified call with the current namespace (PHP's runtime
fallback semantics), a call to a global `function helper() {}` written
as `helper()` inside `namespace App;` is credited to `Function("App\helper")`,
a function that doesn't exist, while the declaration's own key is
`Function("helper")`. The reference-count inlay hint above the
declaration therefore reads "0 references" in any PSR-4-style project
that calls the global function from inside a namespace, even though Find
References (which separately falls back to short-name matching) lists
every call correctly.

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

### B192. Rename started from a `use function` or aliased call site misses fallback call sites

**Impact: Low-Medium · Complexity: Low**

`references/dispatch.rs`'s `FunctionCall` arm passes the raw import span
text (e.g. `Foo\bar`) as the short-name fallback target instead of
shortening it with `short_name()` first, the way the constant arm right
below it does. Starting Find References/rename from a `use function
Foo\bar;` import, or from an aliased call site, therefore only matches
the fully-qualified key and never reaches unqualified call sites in
other namespaces that resolve to the same function — sites that *are*
found when the search starts from the declaration instead.

### B193. Function reference matching is case-sensitive

**Impact: Low · Complexity: Low-Medium**

PHP resolves function names case-insensitively, but the string-keyed
reference index and `find_function_references`
(`references/functions.rs`) match case-sensitively. A call spelled
`HELPER()` resolves to `App\HELPER`, whose key never intersects the
`helper` candidate keys, so the file containing that call is filtered
out before scanning and a rename of `helper` leaves `HELPER()` calling
the old (now renamed-away) name. `import_aware_edit_text` already passes
`case_insensitive: true` for functions, but only for edit text of
locations already found by the case-sensitive search; constants are
correctly case-sensitive throughout and are not affected.

### B194. Rename cannot be started from a fully-qualified call site

**Impact: Low · Complexity: Low**

`span_spells_its_name` (`rename/validate.rs`) compares the recorded name
(without its leading `\`) against the span text (with it) for a
fully-qualified reference such as `\Foo\bar()`, so the comparison fails
and prepare-rename returns nothing when started from that site. Rename
started from any other, non-fully-qualified site still edits
fully-qualified sites correctly, so this only blocks initiating the
rename from an FQN spelling.

### B195. A constant's own declaration is not recognized as a declaration by the reference index

**Impact: Low · Complexity: Low**

The `is_declaration` match in `reference_index.rs` covers
`ClassDeclaration`, `FunctionCall { is_definition: true }`, and
`MemberDeclaration`, but not `ConstantReference { is_definition: true }`
— the flag was added for constants without updating this match. No
consumer currently displays constant reference counts, so this is
latent, but a future count/hint feature for constants would count the
declaration itself as one of its own references.

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

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

### B216. A project-wide external tool run can overwrite a fresher per-file result for a file that stays open

**Impact: Low-Medium · Complexity: Medium-High**

`store_workspace_external_results` (`diagnostics/workspace.rs`) writes a
project-wide external tool's scan-time results straight into the
per-file `last_diags` caches (`phpstan_tool`, `phpcs_tool`,
`mago_lint_tool`, `mago_analyze_tool`) for any file that is open when it
writes. If a concurrent per-file run of the same tool (triggered by an
edit to that same open file) finishes and writes a fresher result to the
same cache entry while the project-wide scan is still awaiting
`flush_workspace_diag_updates` (up to the 10s `REFRESH_TIMEOUT`), the
scan's unconditional `cache.lock().insert(...)` at the end can overwrite
that fresher, already-corrected result with its own stale one — a
last-write-wins race, since neither writer knows about the other. Fixing
this needs some notion of freshness (e.g. a per-uri generation counter
bumped by the per-file workers, with the project-wide writer skipping
its own write when the generation has moved since the scan captured that
file) rather than a simple open/closed re-check, which is why it is
tracked separately from the closed-file race this was split off from.

### B199. A workspace diagnostics scan never replaces a worker retired by the give-up timeout

**Impact: Medium-High · Complexity: Medium**

The native diagnostics scan spawns a fixed worker pool once per pass
(`(cores/2).max(2)`, `diagnostics/workspace.rs`) and, when a worker is
retired after exceeding the 10s per-file give-up, never spawns a
replacement. On a small machine (2 workers on a 4-core box), and because
the file list is sorted (clustering files from the same directory), two
pathological files can wedge both workers in quick succession; once both
are retired, every remaining file in the queue is permanently unchecked
for the rest of the session, since the pass is single-shot per session.
A warning is logged, but the degradation is total for the remainder of
the project rather than limited to the slow files themselves. Fix by
spawning a replacement worker on retire (or budgeting a small number of
retries) rather than shrinking the pool permanently.

### B200. The Laravel-without-required-analyzer gate is defeated by its own PATH fallback

**Impact: Medium · Complexity: Low-Medium**

`resolve_phpstan` (`phpstan.rs`) deliberately refuses to certify
`vendor/bin/phpstan` on a Laravel project that lacks Larastan, "rather
than run through an analyser that would misread its own framework" — but
then falls through to `which("phpstan")` on `$PATH` unconditionally. A
user with a global PHPStan install gets exactly the misreading the gate
was written to prevent, on every save, using a binary that additionally
lacks the project's own vendored extensions. `resolve_mago` (`mago.rs`)
has the same shape (additionally gated by `enabled_services`, but with
the same fallback once past that gate).

### B201. `phpstan.dist.neon` is accepted as project config but does not certify the vendored binary

**Impact: Low-Medium · Complexity: Low**

`has_project_config` (`phpstan.rs`) accepts `phpstan.neon`,
`phpstan.neon.dist`, and `phpstan.dist.neon` as valid project config
files, but `has_phpstan_neon_config`, used by `resolve_phpstan` to decide
whether `vendor/bin/phpstan` may be used, only checks the first two. A
project using the `phpstan.dist.neon` spelling with PHPStan only as a
transitive dependency passes the workspace-run gate but is then refused
the vendored binary, and (per B200) falls back to a PATH binary or
nothing despite having valid config. The two file lists should be a
single shared function.

### B202. Any `illuminate/*` dependency is treated as a full Laravel app

**Impact: Low · Complexity: Low**

`is_laravel_project` (`composer.rs`) matches any `illuminate/*` package,
so a plain library that requires e.g. `illuminate/support` plus
`phpstan/phpstan` directly (with no root neon file) is denied
`vendor/bin/phpstan` for lacking `*/larastan`, even though it isn't a
Laravel application and doesn't need Larastan. Narrow in practice (such
libraries usually ship their own `phpstan.neon.dist`, which triggers a
separate acceptance path), but the heuristic conflates "uses an
Illuminate component" with "is a Laravel app needing Larastan".

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

### B205. The documented macOS global config path does not match where it is read from

**Impact: Low-Medium · Complexity: Low**

`global_config_path()` (`config.rs`) uses `etcetera::choose_base_strategy()`,
which resolves to the XDG path (`~/.config/phpantom_lsp/.phpantom.toml`)
on macOS as well as Linux. `docs/configuration.md` documents
`~/Library/Application Support/phpantom_lsp/.phpantom.toml` for macOS. A
macOS user who hand-creates the documented path gets a config file that
is never read. Decide which path is intended (switching to
`choose_app_strategy` migrates existing users' config location, so this
is worth doing before or clearly noted at release) and align the other
side.

### B206. `config-schema.json` is missing the new Mago lint/analyze keys

**Impact: Low · Complexity: Low**

The `mago.lint` and `mago.analyze` config keys were added to `Config`
and documented in `docs/configuration.md`, but `config-schema.json`'s
`mago` section still only lists `command` and the timeouts. Editors that
use the schema for `.phpantom.toml` autocomplete/validation offer no
completion for the new keys and may flag them as unknown.

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

### B211. Enabling workspace diagnostics through a live config reload has no effect until restart

**Impact: Low · Complexity: Medium**

`run_workspace_diagnostics` has exactly one caller: the startup task,
which already returns early if `[diagnostics] workspace` was off at
startup. `reload_config` (`indexing/watch.rs`) never re-triggers it, so
flipping the setting on via a live-reloaded project or global config (a
feature this release adds) does nothing until the editor is restarted,
contradicting `docs/configuration.md`'s claim that only the PHP version
and indexing strategy require a restart.

### B212. The workspace scan watchdog can mislabel a file that just finished as timed out

**Impact: Low · Complexity: Low**

Between the workspace-scan watchdog's `slot.in_flight()` read and its
`slot.retire()` call (`diagnostics/workspace.rs`), the worker can finish
the timed-out file and move on to the next one. The retire then logs the
just-finished file as abandoned ("gave up on X after 10s... please
report it") even though it completed and its diagnostics were published,
and retires the worker while it is genuinely mid-flight on the file it
just claimed — if the pass then reaches `all_stopped` before that new
file finishes, its result is never harvested and it is reported as not
checked. Roughly a 100ms window (the watchdog poll interval); no state
corruption, just an incorrect log message and an occasional dropped file.

### B213. Two diagnostic refresh requests await the client with no timeout

**Impact: Low-Medium · Complexity: Low**

`request_diagnostic_refresh` was given a 10s cap in a recent commit
specifically because "a client that is busy (or never answers) would
otherwise park this task indefinitely." `did_change_watched_files`
(`server.rs`) and the background index-completion task each still await
`client.workspace_diagnostic_refresh()` directly, with no timeout, and
both fire in the same bursty scenarios (branch switches, initial index
completion) that motivated the timeout elsewhere. Convert both call
sites to `request_diagnostic_refresh()`.

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
