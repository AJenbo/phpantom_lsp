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

No outstanding items.

## Arithmetic

No outstanding items.

## Symbol resolution

No outstanding items.

## Array types

No outstanding items.

## Docblock handling

No outstanding items.

## Miscellaneous

#### B2. A request accessor written with named arguments loses its key

**Impact: Low · Complexity: Low**

`header()`, `query()`, `cookie()`, `input()`, `post()`, and `file()`
each declare one union spanning every way of calling them, and the
call's own arguments are what pick a member. Both resolution paths read
`$key` and `$default` off the call by position, so a named argument is
read as whichever slot it happens to sit in:

```php
$photos = $request->file(key: 'photos');   // list<UploadedFile>|null
                                           // resolves as UploadedFile|null
$all = $request->header(default: 'x');     // the whole HeaderBag
                                           // resolves as a plain string
```

The key text is `key: 'photos'`, which no string literal unquotes to, so
the validation rules that decide between one upload and a list are never
consulted, and a call that supplies only a default is read as if that
default were the key.

The argument texts reaching both sites keep their `name:` labels on
purpose, so that `bind_text_args_to_params` can route them; these two
sites just never call it. The AST counterpart (`bind_args_to_params`)
covers the default's expression the same way.

**Where to look:** `try_resolve_request_accessor_type` in
`type_engine/variable/rhs_resolution/calls.rs` and
`resolve_request_accessor_at_call` in
`type_engine/call_resolution/return_types.rs`. Binding needs the
accessor's declared parameters, and the `ClassInfo` both sites hold is
the receiver's own class rather than an inheritance-merged one, so
`get_method_ci("file")` on an app's `FormRequest` subclass finds
nothing: the lookup has to walk the parent chain.

#### B1. Editing a service provider does not re-scan what it registers

**Impact: Medium · Complexity: Medium**

`build_provider_resources` runs once, at `initialized`, and in the
`analyze` CLI. Nothing re-runs it when a provider file changes, so
everything the scan recovers goes stale for the rest of the session:
a container binding written now needs a restart before `app('key')`
resolves, hovers, or navigates, and the same applies to the view
directories, translation directories, route files, config files, and
component namespaces a provider registers.

Every other provider-derived table already has its per-file
counterpart (`refresh_laravel_gates`, `refresh_laravel_morph_map`,
`refresh_laravel_command_index`, `refresh_laravel_macros`,
`refresh_laravel_storage_drivers`); provider resources are the one
table without one. A `refresh_laravel_provider_resources(uri,
content)` on the same didChange/didSave path closes it.

Re-scanning a single file is not enough on its own: the binding table
is merged across every provider and its precedence depends on which
provider outranks which, so the refresh has to rebuild the merged set
rather than patch one file's entries into it. It must also reset
`laravel_aliases` and clear the class-not-found cache, the way
`build_provider_resources` already does, or the keys it just learned
stay unresolvable.
