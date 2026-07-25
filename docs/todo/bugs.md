# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

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
