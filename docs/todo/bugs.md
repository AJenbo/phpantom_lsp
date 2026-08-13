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

### B126. A guard clause that exits through a `never` call on a non-variable subject does not terminate the branch

**Impact: Medium · Effort: Medium**

```php
$file = $request->file('image');
if (!$file instanceof UploadedFile) {
    app()->abort(422);           // Application::abort() is @return never
}

$imageService->store($article, $file); // still reports UploadedFile|array<UploadedFile>|null
```

The guard body's only statement is a call to a `never`-returning method, so
the branch cannot fall through and the code after the `if` sees only the
narrowed type. `expression_is_never_call`
(`src/type_engine/types/narrowing/guards.rs`) recognises this, but only when
the receiver is a variable: `receiver_class_names` returns nothing for any
other subject expression, and its own doc comment records the reason ("a
property chain, a call result, is left unresolved rather than re-entering
expression resolution from a control-flow predicate"). So `$app->abort()`
terminates the branch while `app()->abort()` and `(new App())->abort()` do
not, and the pre-guard union survives the merge.

Rewriting the same guard as `$app = app(); $app->abort(422);` works today,
which isolates the gap to the receiver expression rather than to `never`
detection or to the conditional return type on Laravel's `app()` helper.

The fix has to resolve the receiver without re-entering the type engine from
a control-flow predicate in a way that can recurse (the predicate runs
inside the forward walker that would be asked to resolve the call). A cached
or snapshot-based resolution of the receiver expression is the likely shape.

**Impact:** 13 of the 141 `type_mismatch_*` diagnostics in
`projects/luxplus-backoffice` (measured 2026-08-13, after the B125 fix) are
this shape, all of them `app()->abort(4xx)` guard clauses over
`Request::file()`.

