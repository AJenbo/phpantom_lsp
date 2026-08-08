# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.

#### B49. Analysis reports a different set of diagnostics on each run

**Impact: High · Effort: Medium**

Running `analyze` twice over the same unchanged directory reports
different diagnostics. On Laravel's `vendor/` the count moves by around
30 between consecutive runs, with a stable core and a flapping tail
(the `Redis::eval()` argument checks in `Illuminate\Queue\RedisQueue`
and `Illuminate\Redis\Limiters` are the most reliable flappers):

```
$ phpantom_lsp analyze vendor --format json | jq '[.files[].messages[]] | length'
6577
$ phpantom_lsp analyze vendor --format json | jq '[.files[].messages[]] | length'
6606
```

A file's diagnostics must not depend on what the workers happened to
resolve first. The likely cause is that a shared resolution cache is
populated in whatever order the parallel workers reach it, so a class
resolved before its dependants sees different member types than one
resolved after. Beyond the correctness problem, this makes it
impossible to tell a real regression from noise when comparing two
builds over a corpus.

