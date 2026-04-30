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
case-insensitive method lookup, and function-level `@template`
inference through generic wrapper params are now fixed.
Remaining gaps:

- **Function name resolution in multi-namespace files**: bare
  function names in namespaced code are not resolved to their FQN,
  so function-level template inference fails in single-file tests
  with multiple namespaces. Works correctly in real projects.
- **Method-level `@template` with `key-of<T>` bound and `T[K]` return**:
  `key-of<T>`, `value-of<T>`, and `T[K]` now evaluate correctly after
  class-level template substitution. However, inferring a method-level
  template parameter `K` from a string literal argument (to resolve
  `T[K]` at a specific call site) is not yet supported.

**Tests:** SKIPs in `tests/psalm_assertions/template_class_template_extends.php`
(lines 177, 227, 427, 500, 682, 843 (namespace)).


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


## B15. Loop exit type narrowing

**Discovered:** SKIP audit of `tests/psalm_assertions/loop_while.php`,
`loop_foreach.php`.

Loop exit truthiness narrowing (e.g. `while ($a) { ... }` narrowing
`$a` to `null` after the loop) works for do-while loops but not for
while loops when the variable is reassigned inside the loop body.
Remaining issues:

- While loop exit narrowing not applied to variables reassigned
  inside the loop body (e.g. `$a = $a->parent` in `while ($a)`
  should yield `null` after exit)
- `instanceof` loop exit narrowing does not strip the matched type
  (e.g. `while ($a instanceof A)` should narrow to `B` for `A|B`)
- Property access on loop-narrowed variable returns no type
- `foreach` loop variable not narrowed away from initial `null`
  after non-empty iteration (would require tracking iterable
  emptiness)
- Break-in-else branch type not merged with loop variable
- Assignment from untyped array value inside null check not
  widened to `mixed`

Related to T20 (type narrowing reconciliation engine) and T29
(definite vs possible variable existence tracking).

**Tests:** SKIPs in `loop_while.php` (lines 39, 67, 91, 115, 152),
`loop_foreach.php` (lines 81, 128, 156, 188, 208).




## B16. PDOStatement fetch mode-dependent return types

**Blocked on:** [phpstorm-stubs#1882](https://github.com/JetBrains/phpstorm-stubs/pull/1882)

`PDOStatement::fetch()` and `PDOStatement::fetchAll()` return
different types depending on the fetch mode constant passed as
the first argument. Once the upstream PR is merged and we update
our stubs, the existing conditional return type support should
handle this automatically.

**Tests:** SKIPs in `tests/psalm_assertions/method_call.php`
(lines 79-85, 87-89).


## B17. Stub-level property and method resolution gaps

**Discovered:** SKIP audit of `tests/psalm_assertions/method_call.php`
and `tests/psalm_assertions/property_type.php`.

Several built-in PHP classes have incorrect or missing type
resolution:

- `DateTimeImmutable::sub()` / `modify()` — static return type
  not resolved (should return `MyDate` when called on subclass)
- `SimpleXMLElement` — resolves as `stdClass` instead of itself;
  `asXML()` overloaded return type not resolved; magic property
  access not resolved
- `DOMDocument` — grandparent stub property `ownerDocument` not
  resolved (hover returns no type)
- `DOMNode::$nextSibling` — `self` type alias not resolved to
  concrete class name `Node`
- `SplObjectStorage` — generic defaults (`<never, never>`) not
  inferred for empty construction
- `SplDoublyLinkedList::bottom()` — generic return type not
  resolved
- `@psalm-no-seal-methods` `__call` return type

**Tests:** SKIPs in `tests/psalm_assertions/method_call.php`
(lines 15-16, 31, 40-41, 97),
`tests/psalm_assertions/property_type.php`
(lines 59, 79, 95-96).


## B19. Object cast type inference

**Discovered:** SKIP audit of
`tests/psalm_assertions/return_type.php`.

`(object)` cast of scalar or array not inferred as object shape
(`object{scalar:int}`, `object{a:int}`).

**Tests:** SKIPs in `tests/psalm_assertions/return_type.php`
(lines 132, 146).


## B21. Remaining static-late-binding and generics gaps

**Discovered:** SKIP audit of
`tests/phpstan_nsrt/static-late-binding.php`,
`tests/phpstan_nsrt/generics.php`,
`tests/psalm_assertions/annotation.php`,
`tests/psalm_assertions/generator.php`,
`tests/psalm_assertions/trait.php`,
`tests/psalm_assertions/template_function_class_string_template.php`.

Miscellaneous type resolution gaps:

- `$variable::method()` on class-string union does not produce
  union return type (`static-late-binding.php` lines 88, 97)
- `static` keyword not preserved through first-class callable
  invocation (`static-late-binding.php` lines 92-95)
- `@template T of (A|B)` union bound not used as return type for
  `pick()` (`generics.php` line 344)
- Template not resolved through `unbox()` generic function
  (`generics.php` lines 482, 485)
- PHPStan's `T (function traced(), argument)` display format
  (`generics.php` line 499) — out of scope for an LSP
- Loop variable union includes array key type (`annotation.php`
  line 33)
- Loop variable resolves to `null` instead of `null|stdClass`
  (`annotation.php` line 34)
- Escaped backslash in array shape key not normalized
  (`annotation.php` line 71)
- Generator variable hover (`generator.php` lines 40, 87)
- `NoRewindIterator` wrapping generator (`generator.php` line 102)
- Trait method returning `new static()` resolves to trait user
  instead of trait definer (`trait.php` line 46)
- Function-level `@template` with intersection and union types
  (`template_function_class_string_template.php` lines 62, 90, 117)

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
