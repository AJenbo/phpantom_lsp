# PHPantom — Bug Fixes

Every bug below must be fixed at its root cause. "Detect the
symptom and suppress the diagnostic" is not an acceptable fix.
If the type resolution pipeline produces wrong data, fix the
pipeline so it produces correct data. Downstream consumers
(diagnostics, hover, completion, definition) should never need
to second-guess upstream output.





## B13. Template substitution through multi-level `@extends` chains

**Discovered:** SKIP audit of
`tests/psalm_assertions/template_class_template_extends.php`.

Template parameters are not resolved through certain inheritance
patterns:

- Function-level `@template` not substituted from concrete argument
  types at call sites
- Two-level `@template-extends` chains (grandchild → child → parent)
  lose substitution
- `@extends Pair<TValue, string>` where the child swaps parameter
  order maps them incorrectly
- Constructor generic inference not propagated to child classes that
  lack their own constructor
- `@template-implements IteratorAggregate<int, Foo>` not used to
  infer `getIterator()` return type
- `ArrayObject::getIterator` not substituting template params from
  `@template-extends`
- `__get` magic method not applying template substitution
- Literal `false` widened to `bool` when used as template argument

**Tests:** SKIPs in `tests/psalm_assertions/template_class_template_extends.php`
(lines 177, 227, 266, 311, 358, 427, 500, 681-682, 737-738,
770, 802, 843, 980-981, 1083-1084, 1087).


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
- `never` type not inferred for empty collections
  (`new ArrayCollection([])` → `ArrayCollection<never, never>`)
- Method-level template on static method returning generic
- Union of generic types from function return
  (`C<A>|C<B>` → element type `A|B`)
- `WeakReference::create` generic resolution

**Tests:** SKIPs in `tests/psalm_assertions/template_class_template.php`
(lines 16-17, 29, 41, 56, 68, 122, 191-192, 286-287, 451, 487,
565, 602, 640, 667, 701, 710, 752, 788, 800).


## B15. Loop exit type narrowing

**Discovered:** SKIP audit of `tests/psalm_assertions/loop_do.php`,
`loop_while.php`, `loop_foreach.php`.

After a loop exits (via condition failure or break), the variable
type is not properly narrowed. Examples:

- `do { $a = getA(); } while ($a !== null);` — after the loop,
  `$a` should be `null` but resolves to `A|null`
- `while ($a = getA()) { ... }` — after the loop, `$a` should
  be narrowed by the falsy exit condition
- `foreach` loop variable not narrowed away from initial `null`
  after non-empty iteration
- Break-in-else branch type not merged with loop variable
- Assignment from untyped array value inside null check not
  widened to `mixed`

Related to T20 (type narrowing reconciliation engine) and T29
(definite vs possible variable existence tracking).

**Tests:** SKIPs in `tests/psalm_assertions/loop_do.php` (lines
63, 80), `loop_while.php` (lines 39, 91, 115, 152),
`loop_foreach.php` (lines 81, 128, 156, 188, 208).


## B16. PDOStatement fetch mode-dependent return types

**Discovered:** SKIP audit of
`tests/psalm_assertions/method_call.php`.

`PDOStatement::fetch()` and `PDOStatement::fetchAll()` return
different types depending on the fetch mode constant passed as
the first argument. Currently all modes resolve to the same type.
This requires conditional return type support keyed on integer
constant arguments.

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


## B18. Property type narrowing through OR'd `instanceof`

**Discovered:** SKIP audit of
`tests/psalm_assertions/property_type.php`.

When a property is accessed after an OR'd `instanceof` check
(`$a instanceof B || $a instanceof C`), the property type should
be the union of both branches. Currently only one branch's type
is used.

**Tests:** SKIPs in `tests/psalm_assertions/property_type.php`
(lines 24, 51).


## B19. Return type resolution edge cases

**Discovered:** SKIP audit of
`tests/psalm_assertions/return_type.php`.

Several return type patterns are not resolved:

- `static` return type inside an array generic
  (`@return array<int, static>`)
- Overridden return type not resolved through child class when
  parent declares `@return static`
- Interface method return type not resolved on implementing class
- Arrow function return type inference (`fn(int $x): bool => ...`
  should produce `Closure(int):bool`)
- `(object)` cast of scalar or array not inferred as object shape
  (`object{scalar:int}`, `object{a:int}`)

**Tests:** SKIPs in `tests/psalm_assertions/return_type.php`
(lines 43, 64, 83, 121, 132, 146).


## B20. Mixin method resolution gaps

**Discovered:** SKIP audit of
`tests/psalm_assertions/mixin_annotation.php`.

`@mixin` method resolution fails in these cases:

- Static method called on a class that mixes in another class
- Method on `IteratorIterator` via mixin
- Mixin method return type not resolved through `static`

**Tests:** SKIPs in `tests/psalm_assertions/mixin_annotation.php`
(lines 34, 73, 168).


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
