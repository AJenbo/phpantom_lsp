# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

Each entry below carries an **Impact · Effort** rating using the same
scale defined in [`docs/todo.md`](../todo.md); that table is also where
each bug's row lives in the current sprint/backlog.

### B134. A constant operand is only read where a `@template` is bound

**Impact: Low · Effort: Medium**

```php
const ID_TABLE = ['immutable' => 1, 'mutable' => 'two'];

/** @param key-of<ID_TABLE> $key */
function acceptsKey(string $key): void {}

/** @return value-of<ID_TABLE> */
function anyValue() { return 1; }

acceptsKey('nope');            // not reported: should be 'immutable'|'mutable'
takesInt(anyValue());          // not reported: should be int|string
```

A constant holding an array literal is now read as its own array shape, so
`key-of<ID_TABLE>` and `ID_TABLE[K]` evaluate — but only along the path that
builds a template substitution map for a call, which runs solely when the
function or method declares `@template` params. A `key-of<CONSTANT>`
parameter or a `value-of<CONSTANT>` return on a plain function is still left
unevaluated and widens to its bound, so neither constrains anything.

**Fix:** run the constant-operand expansion (`constant_operand_shape` in
`type_engine/call_resolution/`) wherever a declared parameter or return type
containing an unevaluated operator is read, not only from
`finish_template_subs`. The awkward part is that the expansion needs a
`ResolutionCtx` while the sites that read those types (the argument
compatibility check, the untemplated return path) have varying access to
one, so the shared entry point has to come first.

### B135. A conditional return type is not resolved from an argument's default

**Impact: Low · Effort: Medium**

```php
function test(string $s): int {
    return str_word_count($s);          // reported: got array<string>|int
}
```

`str_word_count()`'s return type depends on its `$format` argument: `0`
(the default) returns `int`, `1` and `2` return `array<string>`. Neither
the passed value nor the declared default narrows it, so every call reads
back the full union and any use in a typed position is reported. Passing
`1` explicitly is equally unresolved, so `return str_word_count($s, 1);`
from an `array` return type is reported too.

This is what `examples/laravel/app/View/Components/PostSummary.php:37`
trips over, which is why the Laravel example reports four errors where
`docs/CONTRIBUTING.md` documents three.

**Fix:** resolve a conditional return type against the call's arguments,
falling back to a parameter's declared default when the argument is
omitted, rather than joining every branch.
