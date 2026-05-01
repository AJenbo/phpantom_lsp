# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.





## B13. Remaining template inference gaps

**Discovered:** SKIP audit of
`tests/psalm_assertions/template_class_template_extends.php`.

Constructor generic inference through inherited constructors,
case-insensitive method lookup, function-level `@template`
inference through generic wrapper params, function name
resolution in multi-namespace files, `@extends` with swapped
parameter order, `__get` magic method with `key-of<T>`/`T[K]`,
`@template-implements` return type inheritance from stub
interfaces, and class-level generic substitution in method
call return types via `@var` annotations are now fixed.
Remaining gaps:

- **Array-access assignment overwrites `@var` generic type on
  `ArrayAccess` objects**: `$obj[$key] = $val` on an object that
  implements `ArrayAccess` causes the forward walker to lose the
  `@var` generic annotation on `$obj`. Works correctly when there
  is no array-access assignment between the `@var` and the method
  call.
- **Method-level `@template` with `key-of<T>` bound and `T[K]` return**:
  `key-of<T>`, `value-of<T>`, and `T[K]` now evaluate correctly after
  class-level template substitution. However, inferring a method-level
  template parameter `K` from a string literal argument (to resolve
  `T[K]` at a specific call site) is not yet supported.

**Tests:** SKIPs in `tests/psalm_assertions/template_class_template_extends.php`
(line 500).




## B14. Template/generic resolution in namespace-level and complex scenarios

**Discovered:** SKIP audit of
`tests/psalm_assertions/template_class_template.php`.

Several template resolution patterns fail:

- Hover fails in namespace-level code with iterator generics
  (outside any class/function body)
- `class-string<T>` generic resolution for factory patterns
- `self` not resolved to declaring class in inherited static
  methods (returns parent's template param name instead)
- `@property` virtual members with unresolved templates
- `__get` with template return type not resolved
- Intersection types with template interfaces
- Method-level template on static method returning generic
- `WeakReference::create` generic resolution

Many of these fail in the multi-namespace Psalm test file because
`FileContext.namespace` stores only the first namespace, so the
class loader resolves bare names (e.g. `Foo`) against the wrong
namespace. Items that work correctly in single-namespace contexts
(template bound defaults, static method-level templates) are blocked
only by this infrastructure limitation.

**Tests:** SKIPs in `tests/psalm_assertions/template_class_template.php`
(lines 16-17, 29, 41, 56, 68, 122, 191-192, 286-287, 451, 487,
640, 667, 701, 710, 788, 800).



## B16. PDOStatement fetch mode-dependent return types

**Blocked on:** [phpstorm-stubs#1882](https://github.com/JetBrains/phpstorm-stubs/pull/1882)

`PDOStatement::fetch()` and `PDOStatement::fetchAll()` return
different types depending on the fetch mode constant passed as
the first argument. Once the upstream PR is merged and we update
our stubs, the existing conditional return type support should
handle this automatically.

**Tests:** SKIPs in `tests/psalm_assertions/method_call.php`
(lines 79-85, 87-89).


## B21. Remaining static-late-binding and generics gaps

**Discovered:** SKIP audit.

Remaining minor gaps:

- Escaped backslash in array shape key not normalized
  (`annotation.php` line 71)
- Template unification across multiple `class-string<T>` params in
  same-file function calls fails because the function loader cannot
  find functions defined in a different namespace within the same
  file (`template_function_class_string_template.php` line 117)

**Tests:** Referenced in-line above.



## Bulk un-SKIP after fixes

There are `// SKIP` markers across `tests/phpstan_nsrt/*.php` and
`tests/psalm_assertions/*.php` covering gaps in the type engine.
When working on any type engine improvement, grep for `// SKIP` in
the assertion files to find tests that may now pass. Run
`cargo nextest run --test assert_type_runner --no-fail-fast` with
the SKIP removed to verify.

Some SKIPs are **out of scope** for an LSP (value-range tracking,
int overflow detection, constant-expression folding, `*NEVER*`
after impossible conditions, `*ERROR*` diagnostics). These should
just be removed from the test files.
