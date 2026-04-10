# PHPantom — Bug Fixes

## B1 — Pathological `unknown_member` performance on large service files

`collect_unknown_member_diagnostics` takes 194+ seconds on
`src/core/Purchase/Services/PurchaseFileService.php`, causing the
analyze command to time out (debug build) or hang indefinitely
(release build). The next-slowest collectors on the same file never
even get to run.

Other files show the same pattern at smaller scale: `unknown_member`
dominates the breakdown on every slow file (often 50–70% of the total
time). The worst offenders are large service/repository classes that
chain many method calls on Eloquent models, payment gateways, and
similar deeply-inherited types.

Likely causes to investigate:

- Repeated full class merges (inheritance + virtual members) for the
  same type within a single file — the per-file resolved-class cache
  may not be covering all code paths in the unknown-member walker.
- Expensive subject resolution (long `$this->foo()->bar()->baz()`
  chains) re-resolved for every member access instead of being
  cached across diagnostic collectors that share the same file.
- Virtual member synthesis (Laravel model provider) running
  repeatedly for the same model class.

Reproducer: run `phpantom_lsp analyze --project-root <project>`
on any Laravel project with large service classes and observe the
`unknown_member` timing in the slow-file breakdown.

## B2 — Variable resolution pipeline produces short names instead of FQN

The variable resolution pipeline (`resolve_rhs_expression`,
`try_inline_var_override`, `try_standalone_var_docblock`, etc.)
returns `ResolvedType` values whose `type_string` field contains
short class names from raw docblock text or AST identifiers.
Parameter types on `ClassInfo` members are already FQN (resolved
during `resolve_parent_class_names`), so comparisons between the
two fail on name form alone.

Sources of short names:

- `try_inline_var_override` in `completion/variable/resolution.rs`
  gets a `PhpType` from `find_inline_var_docblock` and passes it
  to `from_type_string` or `from_classes_with_hint` without
  resolving names through the use-map.
- `resolve_rhs_instantiation` in `completion/variable/rhs_resolution.rs`
  constructs `PhpType::Named(name.to_string())` from the raw AST
  identifier (short name) and passes it through
  `from_classes_with_hint`. The `ClassInfo` has the FQN, but the
  `type_string` field retains the short name.
- `try_standalone_var_docblock` in `closure_resolution.rs` has the
  same pattern as `try_inline_var_override`.
- `find_iterable_raw_type_in_source` and `find_var_raw_type_in_source`
  in `docblock/tags.rs` return raw docblock types; every caller
  that stores them in a `ResolvedType` preserves short names.

Current mitigation: `collect_type_error_diagnostics` applies
`resolve_names` with the class loader on every resolved argument
type before comparison, so `type_error.argument` diagnostics are
not affected. But other consumers (hover type display, definition
matching, etc.) still see short names.

Fixing at the source is complicated because the same `ResolvedType`
values feed the PHPDoc generation code actions, which need short
names for user-facing output. The proper fix is to always store
FQN in `type_string` and shorten at display time (the way
`implement_methods.rs` already does with `shorten_type`).

## B3 — Array access on bare `array` returns empty instead of `mixed`

When a parameter is typed as bare `array` (no generic annotation),
accessing an element with `$params['key']` resolves to an empty
type instead of `mixed`. This causes downstream issues:

- `$x = $params['key'] ?? null` resolves `$x` to `null` (only
  the RHS of `??`) instead of `mixed|null`, because the LHS
  array access produced nothing.
- `type_error.argument` then flags `null` passed to `string`
  even though the value could be any type at runtime.

The fix should make array access on bare `array` (and `mixed`)
return `mixed` so that downstream resolution and diagnostics
see the correct "we don't know" type.

Reproducer:

```php
function foo(array $params = []): void {
    $authToken = $params['authToken'] ?? null;
    if (!$authToken || !is_string($authToken)) {
        throw new \Exception('missing');
    }
    // $authToken is string here, but diagnostic sees null
    bar($authToken);
}
function bar(string $s): void {}
```



