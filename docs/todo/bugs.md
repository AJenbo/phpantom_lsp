# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

## B1 Rename can build edits from a symbol map that predates the buffer

`symbol_maps` is refreshed on a background task after each `didChange`,
and is left untouched entirely when a parse panics. Until that lands, a
request reads a map whose byte offsets index the *previous* version of
the file. Converting those offsets against the newer buffer yields
ranges over unrelated code.

Linked editing hit this in practice (inserting a line above a variable
and typing mirrored the keystrokes into the middle of a method name
further down the file). It was fixed by checking
`SymbolMap::matches_source` and verifying that every range it reports
still spells the variable, dropping the whole response otherwise.

`rename` and `prepareRename` read the same map and turn the same offsets
into `TextEdit`s, so the same corruption is reachable there:

- `rename/prepare.rs` builds the prepare-rename range straight from
  `span.start`/`span.end` via `offset_to_position` against `content`.
- `rename/namespace.rs` and `rename/class.rs` read `symbol_maps` per
  file and emit edits from the span offsets they find.

It is much harder to trigger than linked editing was, because rename is
an explicit user action taken after a pause rather than something the
editor fires on every cursor move, and because the rename box shows the
range so a wrong one is visible before the user confirms. It is the same
defect class, though, and the fix is the same: gate the edit-producing
paths on `matches_source` and verify each emitted range against the
buffer text before returning it.

Note that the cross-file paths need slightly more than linked editing
did: they read maps for files other than the request URI, so each one
must be checked against *that* file's content, not the buffer the
request arrived on.

## B2 `mem-audit` feature build fails: `Ustr::capacity()` does not exist

`cargo build --features mem-audit` fails on `main` (reproduced before
any local changes, via `git stash`):

```
error[E0599]: no method named `capacity` found for reference `&Ustr` in the current scope
   --> src/mem_audit.rs:1269:27
    |
1269 |                 sym.add(k.capacity());
    |                           ^^^^^^^^ method not found in `&Ustr`

error[E0599]: no method named `capacity` found for reference `&Ustr` in the current scope
   --> src/mem_audit.rs:1271:34
    |
1271 |                 member_idx.add(k.capacity());
    |                                  ^^^^^^^^ method not found in `&Ustr`
```

`k` in both call sites is a `Ustr` (interned string handle from the
`ustr` crate) used as a hash map key, and the audit code is calling
`.capacity()` on it to account for heap bytes the way it does for
`String`/`Box<str>` keys elsewhere in the same file. `Ustr` has no
`capacity()` method — it is a pointer into a global intern table, not
an owned allocation, so the right accounting is either to treat it as
zero marginal heap cost (the string data is shared and already counted
once wherever it's interned) or to look up whatever `ustr` exposes for
the underlying byte length, not capacity.

The current profile (dev, `cargo build --lib`) never builds this
feature, so normal development and `cargo test` are unaffected. It only
surfaces when actually running the memory-audit binary, which is
presumably why it went unnoticed. Fix by correcting the two call sites
in `src/mem_audit.rs` to match whatever the installed `ustr` version's
API actually provides.
