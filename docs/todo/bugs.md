# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

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

#### B22. `find_open_quote` is blind to comments on the cursor's own line

**Impact: Low · Effort: Low**

`find_open_quote` in `src/completion/source/helpers.rs` finds the string
literal the cursor sits in by scanning forward from the start of the
cursor's line, tracking quotes but not comments. An apostrophe in a
comment earlier on that line is read as an opening quote, which pairs up
with the real opener and leaves the cursor looking like it is outside a
string, so every completion gated on it drops out:

```php
Artisan::call('app:sync', [ /* don't ( */ '|']);
```

It fails closed. The line-scoped anchor is also why a literal opened on
an earlier line is never seen (`$request->input(\n    'na|`).

**Where to look:** `code_context_at`
(`src/completion/source/code_context.rs`) already runs the forward
lexical scan that resolves this exactly — comments, heredocs, inline HTML
and all. Having it report the literal it ends inside, instead of only
`None`, would let `find_open_quote` be built on it and fix both gaps at
once. Note that this widens the scan from the cursor's line to the whole
file, so an unterminated literal *above* the cursor would then make the
cursor read as in-string; check the callers in
`virtual_members/laravel/request_fields.rs`,
`completion/eloquent_string.rs`, `completion/command_params.rs`, and
`completion/laravel_route_params.rs` for how they cope with that.

#### B23. `analyze` reports no Laravel string-key errors at all

**Impact: Medium · Effort: Low-Medium**

The `analyze` CLI no longer emits any of the Laravel string-key
diagnostics (`invalid_laravel_route`, `invalid_laravel_config`,
`invalid_laravel_view`, `invalid_laravel_trans`,
`invalid_laravel_command`, `invalid_laravel_morph_alias`). The CI
checklist in `docs/CONTRIBUTING.md` relies on one of them:
`examples/laravel/app/Demo.php:428` calls
`Artisan::call('does:not-exist')` so that
`phpantom_lsp analyze --project-root examples/laravel --no-colour` must
report exactly `[ERROR] Found 1 error`. It reports `[OK] No errors`.

**Reproduce:** run `composer install` in `examples/laravel`, then the
analyze command above. Adding further bogus keys (a
`config('totally.bogus.key')`, a second unknown command) to a copy of
that project does not produce errors either, so the whole family is
silent rather than one kind of key resolving too permissively. The LSP's
own tests for these diagnostics pass, so the gap is in what the analyse
pass feeds them.

**Where to look:** `collect_invalid_laravel_string_key_diagnostics`
(`src/diagnostics/mod.rs`) returns early when `symbol_maps` holds no
entry for the file, and every kind additionally skips when its
enumeration comes back empty (the deliberate escape hatch for
non-Laravel projects that also define `__()` or `trans()`). Either the
analyse pass leaves those maps unpopulated for the files it walks, or the
`is_laravel` gate at `src/diagnostics/mod.rs:361` reads false there.
`src/analyse/run.rs` does call `init_single_project` with the parsed
composer package and builds the command, macro, morph-map, and provider
resource indexes under an `is_laravel()` check, so start by confirming
which of the two gates closes.
