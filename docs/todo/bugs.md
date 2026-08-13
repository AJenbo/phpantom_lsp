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

### B125. A class narrowed by `instanceof` keeps an array alternative from before the check

**Impact: High · Effort: Medium**

```php
$file = $request->file('image'); // Illuminate\Http\UploadedFile|array<UploadedFile>|null

if (!$file instanceof UploadedFile) {
    throw new RuntimeException('missing');
}

$imageService->store($article, $file, $adminUser->id); // still reports UploadedFile|array<UploadedFile>|null
```

```php
$cover = $request->file(self::FORM_KEY_COVER);
if ($cover instanceof UploadedFile) {
    $coverValidator->validate($cover); // still reports UploadedFile|array<UploadedFile>
}
```

Both the guard-clause form (`if (!$x instanceof Y) { throw/return; }`) and the
plain then-branch form (`if ($x instanceof Y) { ...use $x... }`) leave a
non-class union member — here `array<UploadedFile>` from
`Illuminate\Http\Request::file()`'s own conditional return type — sitting
alongside the narrowed class at the use site, even though the `instanceof`
check has already proven the value cannot be an array. `null` is stripped
correctly in most of these paths (several call sites in
`type_engine/variable/forward_walk/cond_narrowing.rs` explicitly
`retain(|rt| !rt.type_string.is_null())` after narrowing), but nothing
strips other non-class alternatives such as a generic `array<T>`. The same
shape reproduces for a locally-declared class as the narrowed target, not
just `UploadedFile` (e.g. `App\Entity\Charity|array<string, string>` in
`vytrvalec-server`), so it is not Laravel- or `UploadedFile`-specific.

`ResolvedType::apply_narrowing` (`src/types/resolved_type.rs`) is one
confirmed contributor: its cleanup after a definite class narrowing only
drops entries that are `mixed` (`results.retain(|rt| !(rt.class_info.is_none()
&& rt.type_string.is_mixed()))`), leaving any other non-class entry (array,
scalar, shape) untouched regardless of whether the narrowing was definite.
Tracing which exact call site in `cond_narrowing.rs` this diagnostic's value
actually passed through was not completed in this triage session — the
single-instanceof branch read during investigation (around
`apply_condition_narrowing`, line ~403-627) filters by `class_info` in a way
that looks like it should already exclude non-class entries, so either a
different, not-yet-located call site is responsible, or something after
narrowing (a branch merge, or the diagnostic's own re-resolution at the call
site) re-widens the type. Needs a debugger/fixture-test trace to pin the
exact site before fixing.

**Impact:** at least 38 of the 121 `type_mismatch_argument` diagnostics in
`projects/luxplus-backoffice` (measured 2026-08-13 on commit `a0de679a`) are
this exact `UploadedFile|array<UploadedFile>(|null)` shape, all following a
correct `instanceof` guard in the source; one more instance in
`projects/vytrvalec-server` uses a different class.

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
