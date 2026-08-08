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

#### B50. An array literal argument binds no element type through a union hint

**Impact: Medium · Effort: Low-Medium**

A union parameter hint that offers "the element or a container of
elements" binds the template one level deeper for a container argument,
but an array *literal* resolves to a bare `array` with no element type,
so nothing useful binds:

```php
/**
 * @template TWrapValue
 * @param  iterable<array-key, TWrapValue>|TWrapValue  $value
 * @return static<array-key, TWrapValue>
 */
public static function wrap($value) {}

$w = Wrapper::wrap(['a', 'b']);
$w->push([1]);   // no diagnostic; TWrapValue should be string
```

The same call with a variable or a call argument of type
`array<string>` binds `TWrapValue` to `string` correctly. The generic
wrapper binding path already unwraps array literals element by element;
the union path needs the same treatment.

#### B51. A chained static factory call loses its method-level template

**Impact: Medium · Effort: Medium**

A method-level `@template` bound on a *static* factory survives into a
variable but not into a directly chained call:

```php
$w = Wrapper::make(names());
$w->push([1]);              // Argument 1 ($value) expects string — correct

Wrapper::make(names())->push([1]);   // no diagnostic at all
```

The instance-method equivalent (`$w->rewrap(names())->push([1])`)
reports correctly, so this is specific to the static-call receiver.
