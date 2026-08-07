# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B27. A comment before the arrow hides a string-argument call's receiver

**Impact: Low · Effort: Low-Medium**

`detect_string_call_context` (`src/completion/eloquent_string.rs`) takes
the call's opening paren, its argument index, and the code before its
callee from the forward lexical scan, so a comment is no longer read as
code around any of those. The *receiver* is still recovered by
`extract_subject_backwards` walking raw source right to left, which stops
at the first byte that cannot continue an identifier. A comment between
the receiver and the operator therefore ends the walk and the completion
drops out:

```php
$q /* the query */ ->where('|');   // no completion
User /* the model */ ::with('|');  // no completion
```

**Where to look:** the callee is read from `OpenBracket::code_before`
(`src/completion/source/code_context.rs`), which the forward scan records
as the end of the code before the bracket. The receiver wants the same
thing two tokens earlier — a code-before offset for the boundary in front
of the callee and in front of the `->` / `::` — recorded by that scan
rather than recovered from the text afterwards.
