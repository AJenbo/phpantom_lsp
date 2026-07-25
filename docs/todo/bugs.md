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

## B115. Closure-parameter inference uses call-position instead of the declared parameter index, so named arguments that reorder a call break it

**Severity: Low-Medium (closure/arrow-function parameters silently lose
their inferred type — falling back to no completions/hover — for any
call that uses named arguments to reorder or skip parameters ahead of
the callable argument) · Discovered while deduplicating the receiver
resolution helpers shared between `closure_resolution.rs` and
`forward_walk/callable_inference.rs`; confirmed with a regression test**

```php
/**
 * @template T
 * @param bool $flag
 * @param class-string<T> $class
 * @param callable(T): void $cb
 */
function process(bool $flag, string $class, callable $cb): void {}

process(class: Product::class, flag: true, function ($p) {
    $p-> // no completions: T never binds to Product
});
```

The forward walker's closure-argument inference
(`walk_closures_in_call_args` and `walk_closure_in_partial_call_args` in
`src/type_engine/variable/forward_walk/diagnostic_walk.rs`, and the
completion/hover-path counterpart `infer_callable_params_for_call` in
`src/type_engine/variable/forward_walk/callable_inference.rs`) computes
`arg_idx` from `arguments.iter().enumerate()` — the argument's position
in the *call*. That index is then used directly as the *declared*
parameter index, both to decide whether the argument at that slot is a
closure and to look up `fi.parameters[arg_idx]` /
`class.get_method(..).parameters[arg_idx]` for the callable's own
`callable(...)`/`Closure(...)` type hint (see
`extract_callable_params_at_fw` and its callers).

PHP argument binding does not guarantee call-position equals declared
position once named arguments are involved: a named argument can appear
anywhere in the call, and named arguments before the closure can be
reordered or a preceding optional parameter can be omitted entirely.
Whenever that happens, `arg_idx` no longer identifies the closure's own
parameter, so `extract_callable_params_at_fw` looks up the wrong
parameter (typically a non-callable one), returns no callable param
types, and the closure gets none of its parameters inferred — even
though the same call resolves correctly when arguments are positional
and in declaration order.

Fix by resolving each argument's *declared* parameter index (via
something like the existing `bind_text_args_to_params` /
`param_name_bare` machinery in `src/call_args.rs`, which already knows
how to route named arguments to their declared slot) before using that
index to look up the callable's own parameter type or to decide which
argument is "the closure at position N". This affects both the
diagnostic-path walker (`diagnostic_walk.rs`) and the completion/hover
path (`callable_inference.rs`), which must stay in sync per the
project's shared-pipeline convention.
