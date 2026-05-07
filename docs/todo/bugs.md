# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.


## B16. PDOStatement fetch mode-dependent return types

**Blocked on:** [phpstorm-stubs#1882](https://github.com/JetBrains/phpstorm-stubs/pull/1882)

`PDOStatement::fetch()` and `PDOStatement::fetchAll()` return
different types depending on the fetch mode constant passed as
the first argument. Once the upstream PR is merged and we update
our stubs, the existing conditional return type support should
handle this automatically.

**Tests:** Assertion lines were removed from
`tests/psalm_assertions/method_call.php` (out of scope until
upstream stubs land).


## B17. Blade `$loop` variable triggers unused-variable false positive

The Blade preprocessor injects `$loop = new \stdClass();` inside
`@foreach` / `@forelse` blocks to provide completion for the magic
`$loop` variable. When the template does not actually reference
`$loop`, the unused-variable diagnostic flags it. The injected
assignment should either be suppressed from unused-variable checks
(e.g. by marking it as a synthetic definition) or the preprocessor
should only inject it when `$loop` is actually used in the block.
