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

### B138. A `@param` on a docblock's opening line is ignored

**Impact: Medium · Effort: Low**

```php
/** @param 'a'|'b' $key
 *  @return string */
function pick(string $key): string {
    takesInt($key);   // reported as `string`, not as 'a'|'b'
    return $key;
}
```

A tag written on the same line as the opening `/**` of a *multi-line*
docblock is not read for parameters. The fully single-line spelling
(`/** @param 'a'|'b' $key */`) works, and so does the same tag moved to a
continuation line, so only the "first tag shares the opening line" shape is
affected. The parameter falls back to its native hint, which is wider than
what the docblock declared: narrowing, argument checks, and hover all read
the wide type, and a `@return` on the same docblock (which *is* read) can
then be reported as incompatible with the body's widened value.

**Fix:** find where the parameter scan decides a docblock's tag lines
(`find_iterable_raw_type_in_source` in `docblock/tags.rs`) and let the text
following `/**` count as a tag line, the way the single-line spelling
already does. The `@return` path reads it, so the two disagree on where a
docblock's first line starts.
