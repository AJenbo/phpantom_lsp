# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.


## B17. `range()` lacks conditional return type

`range()` in phpstorm-stubs returns bare `array`. Psalm and
PHPStan infer `list<int>`, `list<float>`, or `list<string>`
depending on the argument types. Without a conditional return
type (or a stub patch), generic information is lost when the
result is passed to `new ArrayIterator(range(...))`.

A stub patch with conditional return types would fix this.
Alternatively, wait for phpstorm-stubs to add proper generics.

**Tests:** SKIPs on lines 16, 29, 41, 56, 68 of
`tests/psalm_assertions/template_class_template.php`.


## B18. Multi-namespace assertion runner does not resolve short names

The `tests/assert_type_runner.rs` test harness opens a single file
and hovers over synthetic variables. When the file contains
multiple `namespace { }` blocks, short class names in `@var`
annotations or method return types are not expanded to FQN before
resolution. This causes hover to return no type for expressions
that depend on cross-namespace class lookup.

The type engine itself is correct: the same assertions pass in
single-namespace files. The fix belongs in the test runner (or
in the LSP's namespace-aware name resolution for inline `@var`
annotations).

**Tests:** SKIPs on lines 602 and 788 of
`tests/psalm_assertions/template_class_template.php`;
3 SKIPs in `tests/psalm_assertions/magic_method_annotation.php`.


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


## Bulk un-SKIP after fixes

There are `// SKIP` markers across `tests/psalm_assertions/*.php`
covering gaps in the type engine. When working on any type engine
improvement, grep for `// SKIP` in the assertion files to find
tests that may now pass. Run
`cargo nextest run --test assert_type_runner --no-fail-fast` with
the SKIP removed to verify.

Remaining SKIPs (11) are:
- `template_class_template.php` (5) — B17: `range()` returns bare
  `array`; (2) — B18: multi-namespace test runner limitation
- `magic_method_annotation.php` (3) — B18: multi-namespace test
  runner limitation
- `mixin_annotation.php` (1) — `IteratorIterator` not in fixture
  runner stubs (feature works with full stubs)
