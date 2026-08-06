# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B26. `detect_string_call_context` still walks backwards past comments

**Impact: Low · Effort: Low-Medium**

`detect_string_call_context`
(`src/completion/eloquent_string.rs`) now takes the literal the cursor is
in and the code before it from the forward lexical scan, but it still
recovers the *call* around that literal with backwards text scans that
read comments as code. `find_matching_open_paren` balances brackets
right-to-left, so a bracket in a comment unbalances it and the completion
drops out; `count_top_level_commas` counts commas between the `(` and the
literal without skipping comments, so it reports the wrong argument index
and the completion offered is the one for another parameter:

```php
foo('a' /* ) */, 'b|');   // no completion at all
$q->where('a' /* , */, 'b|');  // read as argument 2, not argument 1
```

Both fail on the same input the forward scan already handles.

**Where to look:** `code_context_at` returns `open_brackets`, whose
innermost `(` *is* the call's opening paren, from the same scan that
already produces `code_before` here. Taking `paren_pos` from it removes
`find_matching_open_paren` (and its `MAX_OPEN_PAREN_SCAN` bound)
entirely. The argument index wants the same treatment: count the commas
that the forward scan sees at the call's own bracket depth rather than
re-lexing the span afterwards. `extract_identifier_backwards` and
`extract_subject_backwards` read the callee and receiver before that
paren and are comment-blind in the same way (`foo /* x */ ('|')`), though
that shape is rarer.
