# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B28. A comment before the arrow hides a request field's receiver via the `safe()` hop

**Impact: Low · Effort: Low**

`detect_request_field_context` (`src/virtual_members/laravel/request_fields.rs`)
recovers the receiver of `$request->input('|')` (and the `->safe()->only(['|'])`
hop-through) with its own backwards text walk — `strip_trailing_ident`,
`strip_arrow`, `strip_safe_call`, `trailing_variable` — over
`content[..call.code_before_call]`, independently of
`detect_string_call_context`'s own receiver extraction (fixed for the same
class of bug by using `OpenBracket::callee_operator` from the forward scan,
see the `A comment no longer hides a string-argument call's receiver`
changelog entry). A comment between the receiver and `->` still ends this
walk early:

```php
$request /* the request */ ->input('|');  // no completion
```

**Where to look:** `strip_arrow` needs the same fix `detect_string_call_context`
got — a code-before boundary from the scan rather than a raw trailing-comment
trim. `call.callee_operator` already gives the boundary for the *last* `->`
before the callee; the `safe()` hop needs a second one further back (the
`->` before `safe`), which is not currently exposed since `OpenBracket` only
records the operator immediately before its own callee. Extending the scan
to expose that (e.g. a small chain of recent operators, not just the last
one) would let both `strip_arrow` and `strip_safe_call` read comment-free
boundaries instead of walking raw text.
