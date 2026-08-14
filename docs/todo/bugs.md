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

Most entries below come from the 2026-08-13 sample-project sweep (345
diagnostics across ten projects, ~330 of them false positives). Site
counts refer to that sweep; the git-ignored triage log has the full
per-project inventory. Entries filed later say where they came from.

## Crashes

### B159. Inferring a method's return type panics on a multi-byte file

**Impact: Medium · Effort: Low**

`infer_body_return_type` (`type_engine/call_resolution/target_cache.rs`)
slices `content[..offset]` with the method's recorded `name_offset`. When
that offset does not land on a character boundary of the content it read
back — the file changed, or the URI resolved to a different file than the
one the offset was recorded against — the slice panics rather than
returning `None`, and the panic aborts the diagnostic worker for the whole
file. The bounds check above it only compares against `content.len()`, so
any file with multi-byte characters can hit it:

```
panicked at type_engine/call_resolution/target_cache.rs:246:
end byte index 245419 is not a char boundary; it is inside '─'
```

Counting the newlines before the offset does not need a slice at all, so
the fix is to count over the bytes and drop the panicking index. Found
while working on constant resolution, by pointing `analyze` at a
directory of unrelated files.

## Type comparison

No outstanding items.

## Narrowing

No outstanding items.

## Symbol resolution

### B160. A second `namespace` block silences the argument-type check

**Impact: Medium-High · Effort: Low-Medium**

A file with two `namespace` blocks reports no argument type mismatches at
all. The same code in a single-namespace file reports them:

```php
<?php
namespace App\Other;

class Marker {}

namespace App;

function takesInt(int $x): void {}

function plain(string $key): void {
    takesInt($key);      // not reported; reported without the first block
}
```

`FileContext::namespace` is built from the *first* namespace span in the
file (`file_context` in `backend/file_access.rs`), so every name in the
second block is resolved against the wrong namespace and the called
function is never found. `resolve_function_name_at` already takes an
offset and consults `resolved_names` for exactly this case, so the
diagnostic collectors need to resolve per call site rather than per file.
Found while writing tests for namespaced constant resolution.

## Array types

No outstanding items.
