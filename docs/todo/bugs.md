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

### B99. Formatting a `.blade.php` file has no extension guard

**Impact: Medium · Effort: Low**

`textDocument/formatting` (`src/server.rs`/`src/formatting.rs`) hands a
document straight to the Pint/php-cs-fixer/phpcbf/mago pipeline with no
check on the file extension. [`docs/todo/blade.md`](blade.md#bl16-blade-aware-formatting)
plans real Blade-aware formatting (BL16) as future work and describes an
interim safety measure — explicitly disabling formatting on `.blade.php`
files until then — but that interim guard was never added. Running
"Format Document" on a `.blade.php` file today sends raw Blade markup
(directives, `{{ }}` echoes, component tags) through a PHP formatter,
which will most likely error out or, worse, silently produce nonsense
edits, rather than being a no-op.

**Fix:** add an extension check in the formatting handler that returns no
edits for `.blade.php` documents, ahead of BL16's real implementation.

### B124. An argument's type is read from its source text, and several ordinary spellings read as nothing

**Impact: Medium · Effort: Medium**

```php
/** @param array{message: string} $data */
function report(array $data, string $body): void {
    $out = str_replace('a', 'b', $data['message']);   // array<array-key, string>|string
    $subject = $data['message'];
    $sameThing = str_replace('a', 'b', $subject);     // string — correct

    $version = preg_replace('/-.*/', '', PHP_VERSION);   // array<array-key, string>|string
    $trimmed = preg_replace('/\s+/', ' ', $body ?: ''); // array<array-key, string>|string
}
```

`Backend::resolve_arg_text_to_type` is the shared "what type is this
argument" helper that conditional return types and `@template` binding both
consult, and it works from the argument's *source text*. It answers for
literals, casts, variables, property chains, calls, `::class` and static
access, but several ordinary spellings resolve to nothing:

- an array element (`$data['message']`, `$rows[0]`): the general expression
  path reports only class-backed results, so an element holding a scalar
  comes back empty, and the raw-type fallback skips any text containing `[`
  outright;
- a global constant (`PHP_VERSION`, `PHP_EOL`): there is no constant branch,
  and `ResolutionCtx` does not carry the constant loader that
  `VarResolutionCtx` has, so one would have nothing to consult;
- an operator expression (`$body ?: ''`, `$a . $b`, `$n + 1`): nothing reads
  the operator, even where it alone decides the type.

Assigning the same expression to a variable first resolves fine, so the
answer depends on how the call was spelled. The visible effects are a
conditional return type that stays undecided (and therefore returns the
union of both branches, as `str_replace` does above) and a template
parameter that stays unbound.

**Fix:** each spelling needs its own small branch in the text resolver,
answering with what the same expression resolves to when it is assigned to a
variable first: array access via `SubjectExpr::ArrayAccess` plus the
array-shape key lookup the forward walker already has, a global constant via
the constant loader (which `ResolutionCtx` has to start carrying), and the
operators whose result type is fixed regardless of their operands.
