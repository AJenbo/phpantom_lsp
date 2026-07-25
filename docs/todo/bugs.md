# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

## B116. Renaming a constructor-promoted property parameter doesn't cascade to `$this->prop` usages

**Severity: Low-Medium (rename only updates the parameter declaration
itself; every `$this->field` reference elsewhere in the class is left
stale, silently producing a broken rename) · Discovered while
investigating a user-reported GitHub Q&A about rename support for
promoted properties**

```php
final class SomeService {
    public function __construct(
        private int $someField, // renaming this parameter...
    ) {}

    public function handle(): void {
        $this->someField; // ...doesn't rename this usage
    }
}
```

The symbol-map extraction for constructor parameters
(`src/symbol_map/extraction/class_like.rs:605-642`) unconditionally
tags every parameter's `VarDefSite`/`SymbolSpan` with
`kind: VarDefKind::Parameter` (line 633), with no check for
`param.is_promoted_property()`. This is the symbol map that
`find_references`/rename consult, and it's a separate structure from
the properties list the type engine builds in
`src/parser/classes.rs:1040-1049`, which *does* special-case
`is_promoted_property()` to synthesize a property for hover/completion/
type-inference of `$this->someField` — but that data isn't wired back
into the symbol map's `VarDefKind`.

Because of this, `lookup_var_def_kind_at` (`src/definition/resolve.rs:130`)
reports `VarDefKind::Parameter` for a promoted property, so
`src/references/dispatch.rs:136-172` routes the rename to
`find_variable_references` (`src/references/variables.rs:22`) instead
of the cross-file, member-access-aware `find_member_references`.
`find_variable_references` is explicitly file-local/scope-local and
never looks at `$this->foo` sites, so it only renames the parameter's
own token(s). The same `VarDefKind::Property` check gates
`is_property_rename` in `src/rename/prepare.rs:302-306`, so the rename
is also misclassified as a plain-variable rename rather than a
property rename, which skips the `$this->foo` prefix-fixup logic
`src/rename/mod.rs` documents for property renames.

Fix by making the symbol-map extraction in `class_like.rs` emit
`VarDefKind::Property` (or an equivalent alias treated like `Property`
everywhere) for parameters where `param.is_promoted_property()` is
true, and register the declaration wherever
`find_member_references`/the declaration-hierarchy resolvers expect
property declarations to live. Audit the other call sites that branch
on `VarDefKind::Parameter` vs `Property` for promoted params for their
own reasons (semantic tokens at `semantic_tokens.rs:757-770`, hover at
`hover/mod.rs:152-158`) so they keep working once the kind changes.
There is no existing test coverage for promoted-property rename or
references (`src/rename/tests.rs` and `tests/integration/references.rs`
have no `__construct`/promoted-property cases).
