# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

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
