# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Laravel example project.** All Laravel-specific demos (Eloquent, config/env, views, routes, translations) moved to a standalone `examples/laravel/` project with real config, route, view, and translation files. The main `examples/demo.php` is now framework-agnostic.
- **Blade template support.** Completion, hover, go-to-definition, diagnostics, semantic tokens, and inlay hints work inside `.blade.php` files. The preprocessor transforms Blade syntax into virtual PHP on the fly, with coordinate translation so all editor features report correct positions. ([#100](https://github.com/AJenbo/phpantom_lsp/pull/100) by [@MingJen](https://github.com/MingJen))
- **Blade keyword highlighting.** Blade directives, echo delimiters, PHP keywords, cast types, comments, and PHPDoc tags inside `.blade.php` files now receive semantic tokens for proper syntax coloring. PHP keywords and cast types are emitted from the AST and shared with `.php` files, so highlighting is fully in sync between both file types.

### Changed

- **Diagnostic code identifiers.** Standardised all diagnostic codes to a consistent `snake_case` noun-phrase scheme: `undefined_variable` → `unknown_variable`, `type_error.argument` → `type_mismatch_argument`, `argument_count` → `argument_count_mismatch`, `deprecated` → `deprecated_usage`, `implementation_error` → `missing_implementation`. Users with editor filters or scripts that match on these codes will need to update them.
- **LSP responsiveness.** Hover, go-to-definition, go-to-type-definition, signature help, code actions, document highlight, prepare-rename, and rename handlers now run on background threads instead of the async event loop. When one request is slow (e.g. resolving a deep trait hierarchy), other requests and cancellations are no longer blocked.

### Fixed

- **Type hierarchy registration.** Dynamic registration for type hierarchy is now gated on client capability, preventing errors in editors that don't support it.
- **`compact()` variables no longer flagged as unused.** Variables passed by name to `compact('var1', 'var2')` are now recognized as used, eliminating false-positive unused-variable diagnostics in controllers and views.
- **Type alias parameters no longer trigger false type errors.** When a parameter uses a `@phpstan-type` or `@psalm-type` alias that cannot be expanded (e.g. because it is defined on the declaring class), the type error diagnostic is suppressed instead of incorrectly flagging the argument.
- **Functions inside `if (!function_exists(...))` guards now resolve parameter types.** The diagnostic walker previously skipped function bodies nested inside conditional blocks, causing false-positive unresolved-member-access errors on typed parameters.
- **Eloquent `$dates` and `where{Property}` go-to-definition.** Ctrl+clicking a property backed by the `$dates` array now jumps to the string entry inside `$dates`, matching the behaviour already supported for `$casts`, `$attributes`, `$fillable`, `$guarded`, `$hidden`, `$visible`, and `$appends`. Ctrl+clicking a `where{Property}()` dynamic method (e.g. `whereFlour`, `whereKitchenId`) now jumps to the corresponding column entry in whichever Eloquent array defines that column.
- **Property `self`/`static` type resolution.** Properties declared with `@var self|null` or `static` type annotations now resolve to the owning class name instead of displaying the raw `self`/`static` keyword in hover and type inference.
- **Magic `__get` property access.** Accessing undefined properties on objects with a `__get` method now resolves to the method's declared return type, even when `__get` has no template parameters (e.g. `SimpleXMLElement::$child` resolves to `SimpleXMLElement`).
- **Magic `__call` method return type.** Calling undefined methods on objects with a `__call` method now resolves to `__call`'s declared return type for hover and type inference.
- **`self::class` and `static::class` in template arguments.** Passing `self::class` or `static::class` to a method with `@template T` + `@param class-string<T>` now correctly resolves T to the enclosing class instead of failing to resolve or resolving to the called class.
- **Multi-namespace function return type resolution.** In files with multiple `namespace { }` blocks, function return types were resolved against the first namespace instead of the function's own namespace. This caused incorrect type inference for variables assigned from function calls in later namespace blocks.
- **Template inference through stub interfaces.** `@template-implements` on stub-loaded interfaces (e.g. `IteratorAggregate<int, Foo>`) now correctly propagates substituted return types to child methods that omit a return type annotation. Previously these resolved as untyped.
- **Generic method return types from `@var` annotations.** When a variable is annotated with a generic type (e.g. `/** @var Collection<int, User> */ $items`), method calls on that variable now correctly substitute class-level template parameters into the return type. Previously, return types like `TValue` remained unsubstituted.
- **Short class name resolution in type hints.** When resolving an unqualified class name from a property or return type annotation, the resolver now prefers the class in the same namespace as the owning type before falling back to first-match.
- **Foreach narrowing with break in else.** When a foreach body contained an `if` with an `else { break; }` branch, the variable state from the break path was not included in the post-loop type, causing the merged type to be too narrow.
- **`@var` docblocks with additional tags.** When a `@var` annotation shared a docblock with other tags (e.g. `@psalm-suppress`, `@deprecated`), the extra lines were included in the type string, corrupting type resolution. Multi-line `@var` docblocks now correctly extract only the type from the first line.
- **Multi-namespace variable isolation.** In files with multiple `namespace { }` blocks, variable resolution now only considers the namespace block containing the cursor. Previously, a variable from an earlier block could shadow the correct definition in the current block.
- **Multi-namespace short class name resolution in top-level code.** Unqualified class names in `@var` annotations and type hints at the namespace top level now resolve through the namespace-aware class loader. Previously, the first class with a matching short name was returned regardless of namespace, causing wrong template substitution in multi-namespace files.
- **Foreach element type from untyped arrays.** Variables bound in a `foreach` over an untyped `array` (e.g. from a function returning bare `array`) now resolve to `mixed` instead of empty, so assignments from the loop variable propagate correctly.
- **Block comment highlighting.** Multi-line `/* ... */` and `/** ... */` comments no longer cause everything after the opening delimiter to be coloured as a comment. Each line of a block comment is now emitted as a separate token, matching what the LSP protocol requires.
- **Foreach target type after non-empty literal array.** When iterating a non-empty literal array such as `["a", "b", "c"]`, the pre-loop sentinel value of the target variable (e.g. `null` from `$tag = null`) no longer survives as a possible post-loop type.
- **Top-level variable resolution with `global` keyword.** When a file uses the `global` keyword, the pre-computed top-level scope is now reused for variables at namespace top level instead of running a redundant second forward walk.
- **Static call class resolution in multi-namespace files.** When resolving `ClassName::method()` in the forward walker's RHS resolution, the namespace-aware class loader is now consulted before the flat class list. Previously, a same-named class in an earlier namespace block could shadow the correct one, causing method return types to go unresolved.
- **Hover on reassigned variable shows post-assignment type.** When hovering on the left-hand side of a variable reassignment (e.g. `$date = $date->isoFormat(...)`), the hover now shows the type produced by the assignment rather than the type from before it.
- **Implement methods: no invalid generic return type hints.** When using the "Implement missing methods" code action on an interface method whose only return type annotation is a generic docblock type (e.g. `@return array<TKey, TValue>`), the generated stub now omits the return type hint instead of emitting the generic syntax as a native PHP type, which would have been a syntax error.
- **`@psalm-if-this-is` template inference.** Methods annotated with `@psalm-if-this-is` (or `@phpstan-if-this-is`) now have their method-level template parameters inferred by matching the receiver's concrete type against the annotation's type pattern. This enables correct return type resolution for methods like `compact()` whose return type depends on the caller's generic parameters.
- **Object cast type inference.** `(object)$scalar` now resolves to `object{scalar:T}` and `(object)$array` resolves to an object shape matching the array's keys, instead of bare `stdClass`.
- **Foreach `@var` annotations for key and value variables.** Multi-line docblocks with multiple `@var` tags before a `foreach` now correctly override the inferred key and value types. Previously only single-`@var` docblocks were recognized, and only for the value variable.
- **Template union inference from multiple arguments.** When multiple arguments bind to the same `@template T` parameter (e.g. `pick($int, $float)` or `test(B::class, C::class)` with `@param class-string<T>`), the resolved type is now the union of all inferred argument types instead of only the first one.
- **Spurious function auto-imports.** The byte-level scanner misidentified `use function is_array;` import statements as function declarations, polluting the function index with phantom entries like `CuyZ\Valinor\Cache\is_array`. These ghost functions appeared in completion with no detail and inserted incorrect `use function` statements when accepted.
- **Duplicate `use function` insertion.** Accepting a function completion no longer inserts a `use function` statement when the exact import already exists in the file.
- **Function import conflict handling.** When a different function with the same short name is already imported (e.g. `use function is_array;`), completing a namespaced function now inserts the leading-backslash FQN (e.g. `\NySTL\is_array()`) instead of the ambiguous short name.
- **Array shape keys with special characters.** Array literal keys containing backslashes, newlines, or other non-identifier characters are now properly quoted and escaped in type display (e.g. `array{'foo\\bar\nbaz': string}` instead of a truncated type string).
- **Static method calls on class-string unions.** `$variable::method()` where `$variable` holds a union of class-strings now resolves through all possible classes and unions their return types.
- **Trait `self` return type resolution through inheritance.** When a trait method has return type `self`, calling it on a subclass now correctly resolves to the class that uses the trait (the declaring class), not the calling subclass. `static` and `$this` continue to resolve to the runtime class.
- **SPL iterator generic type propagation.** `CachingIterator`, `InfiniteIterator`, `LimitIterator`, `NoRewindIterator`, `CallbackFilterIterator`, and `FilterIterator` now propagate the wrapped iterator's generic type parameters. Calling `->current()` or `->key()` on any of these decorators returns the correct element type instead of `mixed`.
- **`ArrayIterator` constructor generic inference.** `new ArrayIterator($typedArray)` now infers `TKey` and `TValue` from the array argument's generic type, so the resulting iterator preserves key and value types through the chain.
- **`range()` return type inference.** `range()` now returns `list<string>` when called with string arguments and `list<int|float>` otherwise, instead of bare `array`. This allows generic type propagation through `new ArrayIterator(range(...))` and similar patterns.
- **Conditional return type resolution for scalar arguments.** `$param is string`, `$param is int`, and `$param is float` conditions in `@return` annotations now resolve correctly when the argument is a literal value.
- **Template param inference from type bounds.** When a class template parameter has a bound like `TIterator as Iterator<TKey, TValue>` and `TIterator` is resolved to a concrete generic type, the nested template params (`TKey`, `TValue`) are now inferred from the concrete type's generic arguments.
- **`@psalm-param`/`@phpstan-param` priority over `@param`.** When a docblock contained both `@param array $x` and `@psalm-param array<TKey, T> $x`, the parameter type was resolved from whichever tag appeared first in the docblock. Now `@phpstan-param` always takes precedence over `@psalm-param`, which always takes precedence over `@param`, matching PHPStan and Psalm behaviour.
- **`static` return type through first-class callables.** `self::method(...)()`, `static::method(...)()`, `parent::method(...)()`, and `$this->method(...)()` now preserve `static` in the return type when the underlying method declares `@return static`.
- **ArrayAccess array-access assignment.** `$obj[$key] = $val` on objects implementing `ArrayAccess` no longer overwrites the variable's generic type annotation with an array type.
- **Method-level `@template` with `key-of` bound.** When a method declares `@template K as key-of<TData>` and returns `TData[K]`, passing a string literal argument (e.g. `$config->get('name')`) now infers K from the literal and resolves the return type to the specific array shape value type.
- **`@template T as Bound` parsing.** Template declarations using the `as` keyword (e.g. `@template T as SomeClass`) now correctly parse the upper bound. Previously only `of` was recognized, causing `as` bounds to silently fall back to `mixed`.
- **`__get` template return type substitution.** Dynamic property access on generic classes with a `__get` method now correctly substitutes class-level template parameters. Previously `__get` was resolved against the raw class, ignoring constructor-inferred type arguments.
- **Intersection type method return.** Method calls on intersection types (e.g. `IChild&IParent<C>`) no longer include `mixed` from unparameterized members when a parameterized member provides a concrete return type.
- **Property narrowing after guard clauses.** After `if (!$this->prop instanceof Foo) { return; }`, the narrowed type of `$this->prop` was being stripped before subsequent code could use it. Member accesses on the narrowed property (e.g. in a foreach) now resolve correctly.

### Changed

- **Updated embedded phpstorm-stubs.** Generic annotations for `ArrayIterator`, `ArrayObject`, `SplDoublyLinkedList`, `SplQueue`, `SplStack`, `SplPriorityQueue`, `SplFixedArray`, `SplObjectStorage`, and `array_reduce` are now provided by upstream stubs directly, removing the need for runtime patches.

### Added

- **Argument type mismatch diagnostics.** Flags function and method calls where an argument's resolved type is incompatible with the declared parameter type.
- **Invalid class-like kind diagnostics.** Flags class-like names used in positions where their kind is guaranteed to fail at runtime: `new` on abstract classes, interfaces, traits, or enums; `extends` on a final class, interface, or trait; `implements` with a non-interface; trait `use` with a non-trait; `instanceof` with a trait (always false); `catch` with a non-Throwable type or trait; and traits in native type-hint positions.
- **Unused variable diagnostics.** Variables assigned but never read are flagged with hint severity and rendered as dimmed text. Covers local variables, foreach key/value bindings, catch variables, and closure `use` bindings. Variables named `$_` or prefixed with `$_` are exempt.
- **Mago diagnostic proxy.** Mago lint and analyze diagnostics are surfaced as LSP diagnostics with quick-fix code actions. Configurable under `[mago]` in `.phpantom.toml`. Requires Mago 1.15+.
- **PHPCS diagnostic proxy.** PHP_CodeSniffer violations are surfaced as LSP diagnostics with sniff-name diagnostic codes and severity mapping. Configurable under `[phpcs]` in `.phpantom.toml`.
- **Laravel view, route, and translation key navigation.** Go to Definition now works for Blade view names (`view('admin.users.index')`, `View::make(...)`), route names (`route('admin.users.create')`), and translation keys (`__('auth.failed')`, `trans(...)`, `trans_choice(...)`, `Lang::get(...)`). Route group name prefixes (both fluent `->name('prefix.')` chains and `Route::group(['as' => ...])` arrays) are resolved so the target always points to the final name declaration. Nested route files in subdirectories under `routes/` are included in the search. Contributed by @MingJen in https://github.com/AJenbo/phpantom_lsp/pull/101.
- **Laravel config and env key navigation.** "Go to Definition" and "Find All References" now work for string-literal config keys and env variables (`config('app.name')`, `Config::get(...)`, `config()->string(...)`, `env('APP_KEY')`). Covers the `Config` facade, the `config()` helper (including fully-qualified `\config()`), and all typed accessors (`string`, `integer`, `float`, `boolean`, `array`, `collection`, `set`, `prepend`, `push`). Contributed by @MingJen in https://github.com/AJenbo/phpantom_lsp/pull/93 and https://github.com/AJenbo/phpantom_lsp/pull/96.
- **Namespace renaming.** Renaming a namespace segment updates all namespace declarations, use statements, group use declarations, and fully-qualified name references across the workspace. When a PSR-4 autoload mapping exists, the corresponding directory is moved to keep the filesystem consistent.
- **Linked editing ranges.** Place the cursor on a variable and all occurrences within its definition region enter linked editing mode. Typing a new name updates every occurrence simultaneously.
- **Import all missing classes.** A bulk code action that imports every unresolved class name in the file at once. Only names with a single unambiguous candidate are imported; ambiguous names are left for manual resolution.
- **Context-aware import candidate filtering.** Import class actions now filter candidates by syntactic context (only interfaces after `implements`, only traits after `use`, etc.).
- **Convert to instance variable.** A code action that promotes a local variable inside a method to a class property, rewriting all references to `$this->prop` (or `self::$prop` in static methods).
- **Closure and arrow function inlay hints.** When a closure or arrow function is passed to a callable-typed parameter, inlay hints show inferred parameter types and the return type. Types are derived from the enclosing callable signature with generic substitution from sibling arguments.
- **Return type inference from method bodies.** Methods without a declared return type or `@return` docblock now have their return type inferred from `return` statements, improving completion, hover, and diagnostics for untyped code.
- **Untyped property type inference from constructor.** Properties without type declarations are resolved by inspecting the constructor body for assignments and promoted parameter defaults. PHPDoc generation uses the inferred type. Contributed by @lucasacoutinho in https://github.com/AJenbo/phpantom_lsp/pull/81.
- **Binary expression type inference.** Hover and variable resolution now show result types for all binary operators with operand-type-aware arithmetic (`int + int` → `int`, `int + float` → `float`, `int / int` → `int|float`). Concatenation returns `string`, comparison and logical return `bool`, bitwise returns `int` (or `string` when both operands are strings), spaceship returns `int`, and `+` distinguishes array union from numeric addition. Compound assignments (`/=`, `*=`, `%=`, `**=`, `.=`, `<<=`, `&=`, `|=`, `^=`, `??=`) update the variable's type with the same operand-aware rules. Incrementing a numeric string literal (`$a = "123"; $a++`) produces `int|float`. Parenthesized binary expressions resolve correctly through the structural inference path.
- **`array_reduce`, `array_sum`, and `array_product` return type inference.** `array_reduce()` resolves to the type of its initial value argument. `array_sum()` and `array_product()` resolve to `int|float`.
- **`global` keyword variable resolution.** Variables imported with `global $var` now resolve to their top-level type, enabling completion, hover, and go-to-definition.
- **Nested array shape inference from multi-level key assignments.** Assignments like `$b['a']['b'] = 'x'` now produce a nested array shape type (`array{a: array{b: string}}`), enabling array key completion for arrays built incrementally with nested keys.
- **Machine-readable CLI output.** Both `analyze` and `fix` accept a `--format` flag with `table`, `github`, and `json` options. When `GITHUB_ACTIONS` is set, table output automatically includes GitHub annotations.
- **Generics.** `@mixin` tags referencing a template parameter now resolve through the template bound. `new $var()` where `$var` is `class-string<T>` resolves to `T`. SPL collection classes (`ArrayIterator`, `SplQueue`, `WeakMap`, etc.) now carry `@template` parameters so `->current()` and `->key()` resolve to concrete type arguments.
- **PHPStan assertType test runner.** A data-driven harness imports PHPStan's `assertType()` test corpus to validate type inference against the gold standard. Twenty-six test files imported covering narrowing, generics, shapes, enums, and more.

### Fixed

- **`__get` magic method template resolution.** Property access on a class whose `__get` uses method-level `@template` with `key-of<T>` bounds and `T[K]` return types now infers the concrete type from the property name. For example, `$bag->a` on a `DataBag<array{a: int, b: string}>` resolves to `int`.
- **`(object)` cast type inference.** Casting a scalar to object now resolves to `object{scalar: <type>}` and casting a typed array shape resolves to an object shape with matching properties.
- **Interface method return type inheritance.** When a class implements a same-file interface with `@implements Interface<ConcreteType>` and overrides a method without a return type, the interface method's template-substituted return type is now propagated.
- **Class loader global fallback.** Unqualified class names used in namespaced code without a `use` statement (e.g. `implements IteratorAggregate` in `namespace App`) now fall back to global scope lookup when the namespace-qualified name doesn't exist.
- **Literal `true`/`false` preserved in template inference.** Passing `true` or `false` to a generic constructor now keeps the precise type as the template argument (e.g. `C<false>`) instead of widening to `bool`.
- **`@psalm-method` overrides `@method`.** When a class has both `@method int foo()` and `@psalm-method string foo()`, the vendor-prefixed tag now takes priority instead of using whichever appeared first in the docblock.
- **Multi-namespace class resolution.** In files with multiple `namespace` blocks, short class names now resolve against the correct namespace for the current scope instead of picking the first same-named class in the file.

- **False-positive diagnostics on startup.** Files opened while the project was still indexing could produce hundreds of spurious "class not found" and "function not found" errors. Diagnostics are now deferred until initialization completes.
- **Chained instantiation preserves constructor-inferred generics.** Expressions like `(new Box(new Product()))->get()` and `new Box(new Product())->get()` now propagate template arguments inferred from constructor parameters to subsequent method calls, so `get()` returns the concrete type instead of `mixed`.
- **`@method` tag resolution.** Colon return type syntax (`@method foo(): bool`), parenthesised return types (`@method (callable():string) foo()`, `@method (string|int)[] bar()`), and the ambiguous single-`static` pattern (`@method static getStatic()`) are now parsed correctly. Template parameters in `@method` return types are substituted through `@extends` and `@implements` annotations (e.g. `@method T get()` on a parent class resolves to the concrete type when a child declares `@extends Parent<string>`). `$this` return types on `@method` tags preserve generic arguments (e.g. `A<B>` when the receiver is `A<B>`).
- **First-class callable invocation return types.** Immediately invoking a first-class callable (`Foo::method(...)()`, `$this->method(...)()`, `self::method(...)()`, `static::method(...)()`, `parent::method(...)()`, `func(...)()`) now resolves to the underlying method or function's return type. Previously these expressions returned no type.
- **`isset()` and `empty()` narrowing.** `isset($x)` in a condition now strips `null` from the variable's type in the truthy branch (matching `$x !== null` semantics). `!isset($x)` narrows to `null` only. `!empty($x)` strips `null` from nullable types (e.g. `string|null` narrows to `string`). Both simple variables and property access (`$obj->prop`) are supported, and multiple arguments (`isset($a, $b)`) narrow all listed variables. Guard clauses (`if (!isset($x)) { return; }`) strip null from the variable in the code that follows.
- **Hover scales linearly on large files.** Processing many hover requests on the same file (e.g. a test suite with 80+ assertion calls) no longer takes O(n²) time. The first hover on a method body walks it once and caches scope snapshots; every subsequent hover on the same file content is an O(log n) lookup with no re-walk.
- **`@return numeric` pseudo-type.** Functions annotated with `@return numeric` now resolve to the `numeric` pseudo-type instead of `string`. Arithmetic on `numeric` operands correctly infers `int|float`, and `$a++` on a `numeric` variable yields `int|float`. The underlying cause was that same-file function calls in multi-namespace files fell through to an incorrect fallback when the file-level namespace differed from the function's namespace.
- **`parent::__construct()` with `@extends` generics.** Calling `parent::__construct($arg)` in a child class that specifies `@extends Parent<Concrete>` no longer produces false-positive type errors for the substituted parameter types.
- **Array access on bare `array` and `mixed` types.** Accessing a key on a parameter typed as plain `array` (e.g. `$params['key']`) now resolves to `mixed` instead of an empty type, eliminating false-positive type errors downstream (e.g. `$x = $params['key'] ?? null` followed by an `is_string()` guard).
- **Analyzer and LSP no longer hang on files with deeply nested loops.** Files with multiple levels of foreach/while/for inside if-branches no longer cause exponential blowup.
- **Non-deterministic diagnostic counts eliminated.** Projects with heavy use of method-level `@template` no longer see spurious false positives that vary between runs due to cache contamination by concrete type arguments.
- **Pull-diagnostic reliability.** Editors like Zed that support pull diagnostics no longer show duplicate diagnostics, no longer get stuck with stale results after edits, and now display all diagnostics on first file open without requiring an edit to trigger them.
- **Infinite loop on array key reassignment patterns.** Files containing `$arr['key'] = f($arr['key'])` or similar read-then-write patterns on the same array key no longer hang the analyzer.
- **Stack overflow on large codebases and large files.** The `analyze` command and files with hundreds of class definitions no longer crash with stack overflows.
- **`analyze` and `fix` commands run at consistent speed regardless of invocation style.** Running from within a project directory is no longer ~8x slower than using `--project-root`.
- **Type narrowing.** Comprehensive fixes to type narrowing: non-exiting `if` branches no longer leak narrowed types into post-merge scope; `is_float()`, `is_null()`, and other `is_*()` guards correctly narrow multi-member unions; negated `instanceof` on nullable unions preserves `null`; `is_object()` narrows to all class types in a union (not just the first); `instanceof` on `mixed` or `object` narrows to the checked type; `=== null` and `== null` narrow to `null` in the truthy branch; `assert($x instanceof Foo)` followed by any `if` block no longer loses the narrowed type; falsy guard clauses strip both `false` and `null`; template-constrained types are expanded before guard filtering. `is_numeric()`, `is_bool()`, and `is_scalar()` guard clauses now narrow union types correctly. `exit` and `die` are recognized as unconditional exits for guard clause narrowing. Property access expressions (`$a->foo`) are now narrowed through chained if/elseif/else conditions using `is_string()`, `instanceof`, and other type guards. Array shape keys are now narrowed through null-check guard clauses and conditional reassignment (e.g. `$a["test"]` resolves to `int` instead of `?int` after `if ($a["test"] === null) { return; }`). After `instanceof` check plus reassignment inside the if-body, the post-merge type collapses to the common parent class instead of producing a redundant `Child|Parent` union. Property access after OR'd `instanceof` checks (`$a instanceof B || $a instanceof C`) now resolves to the union of all branches' property types. After `while` and `do-while` loops exit, the loop condition's inverse is applied to narrow variable types (e.g. `do { $a = $a->next; } while ($a)` narrows `$a` to `null` after the loop; `while ($a instanceof Foo)` narrows `$a` to the excluded type after exit). Branch merging no longer loses nullable information when two branches produce the same class with different nullability (e.g. merging `A` with `A|null` correctly yields `A|null` instead of silently dropping the null).
- **Mixin resolution.** Static method calls on instances of a class with `@mixin` now resolve through the mixin. `@method` and `@property` tags declared on a mixin class are propagated to the consuming class. `$this` return types on mixin methods resolve to the consumer class. `IteratorIterator` is now patched with `@template` parameters and `@mixin TIterator` (matching PHPStan's stubs).
- **Generics.** Constructor generic inference now works through inherited constructors: child classes without their own constructor infer template parameters from the parent's constructor arguments, with correct remapping through multi-level `@extends` chains (including swapped or renamed template parameters). Function-level `@template` parameters bound to generic wrapper types (e.g. `@param Container<TItem> $c`) are inferred from arguments that extend the wrapper class. Method calls are now case-insensitive, matching PHP semantics (e.g. `$obj->getId()` finds `getID()`). Closure literals passed to `@template` parameters are recognised as `Closure`. Class-level template parameters are preserved through chained method calls. Template parameters fall back to their declared bound (or `mixed` when unbounded) when subclasses omit `@extends` or `@use` annotations. Unbound template parameters at call sites resolve to their declared bounds or `mixed` instead of leaking raw names. Method-level templates resolve correctly through generic wrappers and nested call chains. Return type generic arguments are preserved for template substitution, fixing false "expects TRelatedModel, got Translation" diagnostics. Iterating over a subclass that extends a generic collection with scalar type arguments (e.g. `IntCollection extends Collection<int, int>`) now yields the concrete scalar type instead of the raw template parameter name. Calling a method on a union of generic types (e.g. `$var->get()` where `$var` is `C<A>|C<B>`) now resolves to the union of each branch's return type (`A|B`) instead of only the first branch. Empty array literals passed to generic constructors or functions infer `never` for element type parameters (e.g. `new ArrayCollection([])` resolves to `ArrayCollection<never, never>`).
- **Type narrowing.** Comprehensive fixes to type narrowing: non-exiting `if` branches no longer leak narrowed types into post-merge scope; `is_float()`, `is_null()`, and other `is_*()` guards correctly narrow multi-member unions; negated `instanceof` on nullable unions preserves `null`; `is_object()` narrows to all class types in a union (not just the first); `instanceof` on `mixed` or `object` narrows to the checked type; `=== null` and `== null` narrow to `null` in the truthy branch; `assert($x instanceof Foo)` followed by any `if` block no longer loses the narrowed type; falsy guard clauses strip both `false` and `null`; template-constrained types are expanded before guard filtering. `is_numeric()`, `is_bool()`, and `is_scalar()` guard clauses now narrow union types correctly. `exit` and `die` are recognized as unconditional exits for guard clause narrowing. Property access expressions (`$a->foo`) are now narrowed through chained if/elseif/else conditions using `is_string()`, `instanceof`, and other type guards. Array shape keys are now narrowed through null-check guard clauses and conditional reassignment (e.g. `$a["test"]` resolves to `int` instead of `?int` after `if ($a["test"] === null) { return; }`). After `instanceof` check plus reassignment inside the if-body, the post-merge type collapses to the common parent class instead of producing a redundant `Child|Parent` union.
- **Generics.** Constructor generic inference now works through inherited constructors: child classes without their own constructor infer template parameters from the parent's constructor arguments, with correct remapping through multi-level `@extends` chains (including swapped or renamed template parameters). Function-level `@template` parameters bound to generic wrapper types (e.g. `@param Container<TItem> $c`) are inferred from arguments that extend the wrapper class. Method calls are now case-insensitive, matching PHP semantics (e.g. `$obj->getId()` finds `getID()`). Closure literals passed to `@template` parameters are recognised as `Closure`. Class-level template parameters are preserved through chained method calls. Template parameters fall back to their declared bound (or `mixed` when unbounded) when subclasses omit `@extends` or `@use` annotations. Unbound template parameters at call sites resolve to their declared bounds or `mixed` instead of leaking raw names. Method-level templates resolve correctly through generic wrappers and nested call chains. Return type generic arguments are preserved for template substitution, fixing false "expects TRelatedModel, got Translation" diagnostics. Iterating over a subclass that extends a generic collection with scalar type arguments (e.g. `IntCollection extends Collection<int, int>`) now yields the concrete scalar type instead of the raw template parameter name. Calling a method on a union of generic types (e.g. `$var->get()` where `$var` is `C<A>|C<B>`) now resolves to the union of each branch's return type (`A|B`) instead of only the first branch. Empty array literals passed to generic constructors or functions infer `never` for element type parameters (e.g. `new ArrayCollection([])` resolves to `ArrayCollection<never, never>`). Array literal arguments to `@param array<TKey, TValue>` constructor parameters now infer key and value types separately (e.g. `["a" => new Foo()]` infers `TKey=string, TValue=Foo`). `key-of<T>`, `value-of<T>`, and indexed access types (`T[K]`) now evaluate to concrete types after template substitution (e.g. `value-of<array{name: string, user: User}>` resolves to `string|User`).
- **Vendor functions and constants.** Functions and constants defined in vendor packages are now indexed at startup, eliminating false-positive "Function not found" diagnostics.
- **Use-imported classes no longer shadowed by global-namespace stubs.** The use-map is now checked first for unqualified names, fixing Laravel Facade static method resolution.
- **Same-name class in a different namespace no longer shadows inherited members.** Parent resolution no longer picks up a same-named class from the current namespace instead of the intended global-namespace parent.
- **Short-name collisions eliminated project-wide.** Two unrelated classes sharing a short name are no longer treated as identical across diagnostics, go-to-implementation, and trait conflict resolution.
- **Transitive interface inheritance.** A class implementing an interface that extends another interface is now correctly recognized as a subtype of the parent interface.
- **Conditional return types with concrete `class-string` bounds.** Methods with conditional return types now check whether the argument class actually implements the bound interface before choosing the return branch.
- **Conditional return type names resolved across files.** Class names in conditional type annotations are now resolved through the defining file's use statements.
- **Promoted properties.** Inline `/** @var */` annotations on promoted constructor properties now resolve inside the constructor body and take precedence over `@param` tags.
- **Backed enums.** Accessing `->value` on a backed enum resolves to the specific backing type (`string` or `int`). `@implements` generics on enums are resolved correctly.
- **Class constants.** Inherited constants accessed via `self::CONST` or `ChildClass::CONST` resolve correctly through multi-level inheritance chains.
- **Hover / type display.** `T[]` displays as `array<T>`, `mixed[]` as `array`. PHPDoc type aliases and case-insensitive keywords are normalized to canonical PHP forms. Methods returning `parent` resolve to the actual parent class name.
- **Chain assignments.** `$a = $b = new Foo()` resolves all variables in the chain to the correct type.
- **Destructuring.** Array destructuring (`[$a, $b] = $expr`, `list()`, keyed shapes, nested patterns) and foreach destructuring now resolve types for all destructured variables.
- **Hover on generics.** Variables assigned from method calls on classes with `@implements Interface<ConcreteType>` show the substituted return type. Hovering over methods inherited from generic traits shows concrete return types instead of raw template names.
- **Variable type resolution.** Short class names from `@var`, `@param`, and `new ClassName()` are now resolved to FQN before entering the type pipeline. Template parameter bounds are also resolved through use statements.
- **Closure inlay hints.** Template parameters in callable signatures are substituted with concrete types inferred from sibling arguments.
- **Laravel scopes.** Public methods with the `#[Scope]` attribute are no longer treated as scopes (Laravel requires them to be `protected`).
- **Property narrowing after guard clauses.** `if (!$this->prop instanceof Foo) { return; }` now narrows `$this->prop` to `Foo` in subsequent code.
- **Static methods.** `$this` no longer resolves inside static methods.
- **Hover cache invalidation.** Editing a cross-file class's docblock now correctly invalidates the cache so hover immediately reflects updated content.
- **CLI with relative paths.** Relative paths are canonicalized before building URIs, fixing cross-file resolution when the workspace root is relative.
- **Performance.** Namespace-qualified scalar type keywords are short-circuited before class resolution, eliminating thousands of wasted lookups per analysis run.
- **Foreach over nested generic array access.** Iterating over expressions like `$this->rules[$key]` where the property is typed `array<string, list<Rule>>` now correctly infers element types.
- **Foreach key variable type from static properties and scalar keys.** Static property access resolves through property type hints, and scalar key types are extracted from generic iterables.
- **Foreach loop prescan no longer leaks reassigned types into same-statement RHS.** The loop-body prescan results are deferred until after the normal body walk.
- **Foreach by-reference binding.** `foreach ($array as &$value)` no longer triggers false-positive undefined/unused variable diagnostics.
- **Foreach iterable type alias expansion.** Foreach loops over `@phpstan-type` aliases resolve the element type correctly.
- **Array shape key completion through conditional branches.** Keys added inside `if` blocks are now available for completion after the branch.
- **Loop-carried assignments visible during completion.** Variables assigned later in a loop body are now discovered during completion inside the same loop.
- **Variable completion inside assignment RHS sees the pre-assignment type.** The variable retains its previous type when the cursor is inside the right-hand side of a reassignment.
- **Scope leakage after closures in chained method calls.** Variables from the enclosing method are no longer invisible after a closure argument in a chained call.
- **Docblock `@param` annotations no longer leak across sibling methods or closures.** The docblock scanner stops at function boundaries and verifies annotations are attached to their closure.
- **`class-string<T>` parameter completion.** Parameters typed as `class-string<T>` with a bounded template resolve to the bound class. Static member access like `$class::KEY` finds members on the bound.
- **Inherited parameter types propagate to child methods.** When a child method overrides a parent without its own `@param` docblock, the parent's type annotation is used for completion.
- **`@phpstan-assert-if-false self` no longer narrows to the enclosing class.** Guard clauses now narrow against the declaring class.
- **Unresolvable instanceof target no longer causes false diagnostics.** The variable's type is cleared rather than keeping the pre-narrowing type.
- **False positive type error for closures passed to callable parameters.** `\Closure` is now recognised as a subtype of `callable` in argument type checking.
- **Union-typed method calls no longer lose resolution on second occurrence.** Repeated calls on union-typed variables resolve correctly instead of returning stale cache entries.
- **Fluent method chains in namespaced classes.** Methods returning `static` or `self` resolve correctly across namespaces.
- **False-positive undefined variable for by-reference parameters.** Passing an undefined variable to a by-reference parameter no longer produces false diagnostics in namespaced code, inherited static methods, or constructors.
- **False-positive undefined variable on nested array access assignment.** `$b['a']['a'] = 'a'` no longer flags `$b` as undefined.
- **Missing blank line between namespace and first import.** Auto-imported use statements now get a preceding blank line when needed (PSR-12 convention).
- **"Remove all unused imports" missing in braced namespaces.** The bulk action now appears inside `namespace Foo { ... }` blocks.
- **Exception types in `catch` clauses matched incorrectly across namespaces.** Thrown exception types are now matched regardless of whether short or fully-qualified names are used.
- **Nested `match(true)` expressions.** Multiple `match(true)` expressions sharing narrowed variable names no longer produce incorrect diagnostics.
- **Lowercase built-in class names recognized as subtypes of `object`.** PHP classes with lowercase names (e.g. `finfo`, `stdClass`) are now correctly treated as class-like in subtype checks.
- **Variables prefixed with `$this` receiving stale type information.** Variables like `$thisItem` no longer receive type information from `$this`-related narrowing.
- **False "class not found" for global-namespace classes loaded via Composer's `files` autoloading.** Classes autoloaded through Composer's `files` directive are now resolved correctly.
- **False-positive type errors on generic class methods and templated static methods.** Class-level and method-level `@template` parameters are now substituted into method parameter types before checking argument compatibility.

### Changed

- **Lower memory usage for variable type tracking.** Scope snapshots used by the forward variable resolver now use interned strings for variable name keys, eliminating redundant heap allocations when cloning scope state at each statement boundary.
- **Diagnostic delivery model reworked for pull/push coexistence.** Editors that support pull diagnostics now get diagnostics on first file open without waiting for a debounce timer. Each diagnostic source caches results independently, and updates from external tools no longer re-run the entire native diagnostic pipeline.
- **Rewritten variable resolver.** Variable type resolution now uses a single top-to-bottom pass through each function body with zero recursion and no depth limit.
- **Broader type narrowing.** `instanceof`, type-guard functions, `in_array()` strict mode, `assert()`, `@phpstan-assert-if-true`/`-if-false`, and compound `&&`/`||` conditions now narrow types in if/else branches, guard clauses, while-loop bodies, ternary expressions, and `match(true)` arms.
- **Chain resolution cache.** The per-request chain resolution cache is now active for all LSP handlers, not just diagnostics.
- **Closure and arrow function parameter inference.** Untyped closure parameters are inferred from the enclosing call's callable signature, including through method chains that return `static`. Generic type substitution flows through to inferred parameters.
- **Editing responsiveness.** Classes evicted from the cache after a file edit are now eagerly re-populated in dependency order.
- **Virtual member resolution.** Virtual member providers (PHPDoc mixins, Laravel model, Eloquent Builder) now resolve completely on every class, eliminating cases where mixins or accessors were missing after edits.
- **Faster analysis.** Analysis time cut significantly on large projects through interning, O(1) method lookup, shared metadata, and reduced redundant work.
- **Loop type propagation.** Replaced the fixed two-pass loop strategy with assignment-depth-bounded iteration with fixed-point early exit. Variables assigned late in loop bodies are now visible from the start on subsequent iterations.

## [0.7.0] - 2026-04-08

### Added

- **`@psalm-return`, `@psalm-param`, and `@psalm-var` tag support.** Psalm-prefixed docblock tags are now recognized alongside their PHPStan equivalents for return types, parameter types, variable types, conditional return types, template parameter bindings, and semantic token highlighting.
- **Refactoring code actions.** Extract function, extract method, extract variable, extract constant, inline variable, promote constructor parameter, generate constructor (traditional and promoted), generate getter/setter, and generate property hooks (PHP 8.4+). Deferred computation ensures the lightbulb menu appears instantly; edit generation only runs when the user picks an action.
- **PHPStan quickfixes.** Automated fixes for a wide range of PHPStan diagnostics: update or remove mismatched `@return`/`@param`/`@var` tags, remove unused return type union members, fix unsafe `new static()` (add `@phpstan-consistent-constructor`, `final` class, or `final` constructor), add or remove `#[Override]`, add `#[\ReturnTypeWillChange]`, fix void return mismatches, add inferred iterable return types, remove unreachable statements, remove always-true `assert()` calls, fix overriding member visibility, fix vendor-prefixed class names, and simplify ternary expressions to `??` or `?->`. All quickfixes eagerly clear their diagnostic on apply.
- **`fix` CLI subcommand.** `phpantom_lsp fix` applies automated code fixes across a project. Specify rules with `--rule` (multiple allowed) or omit to run all preferred fixers. `--dry-run` reports what would change without writing files. The first shipped rule, `unused_import`, removes unused `use` statements project-wide, collapsing blank lines left behind by removals (contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/54). Supports path filtering and single-file mode.
- **Keyword completions.** Context-aware PHP keyword suggestions filtered by scope (e.g. `return` only inside functions, `break` only inside loops, member keywords inside class bodies, enum backing types after `enum Name:`). Contributed by @ryangjchandler in https://github.com/AJenbo/phpantom_lsp/pull/43.
- **Attribute completion.** Typing inside `#[…]` offers only classes decorated with `#[\Attribute]`, filtered by the target declaration kind.
- **Eloquent model enhancements.** Timestamp properties (`created_at`, `updated_at`) are automatically typed as `Carbon` with support for `$timestamps = false` and custom column constants. Legacy `$dates` arrays produce typed virtual properties. `$appends` entries produce virtual properties. `where{PropertyName}()` dynamic methods are synthesized from all known columns (including `@property` annotations) on both the model and the Builder. `whereHas`/`whereDoesntHave` closure parameters resolve to `Builder<RelatedModel>` by traversing relationship methods, with dot-notation chain support. `Conditionable::when()`/`unless()` chains preserve type information.
- **Type-guard narrowing.** `is_array()`, `is_string()`, `is_int()`, `is_float()`, `is_bool()`, `is_object()`, `is_numeric()`, and `is_callable()` narrow union types inside `if`/`else`/`elseif` bodies and after guard clauses, preserving generic element types through narrowing.
- **Array value type tracking.** Arrays built incrementally with variable keys inside loops now carry element types through `foreach` iteration, bracket access, and null-coalescing. Foreach over generic arrays with non-class element types (array shapes, scalars) now preserves the full element type.
- **Inherited docblock type propagation.** When a child class overrides a method without providing its own `@return` or `@param` docblock, the ancestor's richer types flow through automatically. Applies to return types, parameter types (matched by position), property type hints, and descriptions.
- **Bidirectional template inference from closures.** Templates appearing in callable parameter signatures are now inferred from both the closure's return type and its parameter types. Positional matching is supported, and return-type bindings take priority when the same template appears in both positions.
- **Drupal project support.** Drupal projects are detected via `composer.json`. Drupal-specific directories and PHP extensions (`.module`, `.install`, `.theme`, `.profile`, `.inc`, `.engine`) are recognized and indexed. Contributed by @syntlyx in https://github.com/AJenbo/phpantom_lsp/pull/52.
- **Completion and signature help for `new self`, `new static`, and `new parent`.** Constructor parameter snippets and signature help inside the parentheses. Contributed by @RemcoSmitsDev in https://github.com/AJenbo/phpantom_lsp/pull/51.
- **Hover on parameter variables at their definition site.** Hovering on a function or method parameter now shows its resolved type, using the `@param` docblock type when it is richer than the native hint. Contributed by @RemcoSmitsDev in https://github.com/AJenbo/phpantom_lsp/pull/68.
- **Array element type extraction from property generics.** Bracket access on properties annotated with generic array or collection types (e.g. `$this->cache[$key]->`) now resolves the element type correctly through nested chains, string-literal keys, and method chains after the bracket.
- **`@phpstan-assert-if-true $this` narrowing.** Instance methods annotated with `@phpstan-assert-if-true` or `@phpstan-assert-if-false` targeting `$this` now narrow the receiver variable in the corresponding branch. Contributed by @syntlyx in https://github.com/AJenbo/phpantom_lsp/pull/52.
- **Namespace completion from file path.** When creating a new PHP file, typing `namespace ` suggests the correct namespace inferred from the file's location and the project's PSR-4 autoload mappings. The most specific mapping is preselected so you can accept it with a single keypress. When multiple PSR-4 roots match the same directory, all candidates appear ranked by specificity (longest match first).
- **Standalone `@var` docblock for untyped closure parameters.** When a closure parameter lacks a type hint and no assignment follows, a `@var` block above the usage is now picked up as the variable's type.
- **`--stdio` CLI flag.** Accepted (and ignored) for compatibility with LSP client wrappers that pass `--stdio` by default. Contributed by @markkimsal in https://github.com/AJenbo/phpantom_lsp/pull/67.
- **`--tcp` CLI flag.** `phpantom_lsp --tcp 9257` starts the server listening on a TCP port instead of stdin/stdout. Useful for debugging or connecting from IDE plugins that prefer a network transport over spawning a child process. Accepts a full address (`127.0.0.1:9257`) or just a port number. The server accepts one connection and exits when the client disconnects.
- **Zed extension setup instructions.** Contributed by @daronspence in https://github.com/AJenbo/phpantom_lsp/pull/47.
- **SETUP.md improvements.** Contributed by @mattsches in https://github.com/AJenbo/phpantom_lsp/pull/61.
- **Method-level template parameters resolve inside method bodies.** `@template T of Builder` with `@param T $query` now resolves `$query` to the template bound inside the method body, providing completions from the bound class.
- **Undefined variable diagnostic.** Variable reads that have no prior definition (assignment, parameter, foreach binding, catch variable, `global`, `static`, `use()` clause, or destructuring) in the same scope are flagged as errors. Writes must appear before the read in source order, catching use-before-assign bugs, while assignments inside branches (if/else, switch, try/catch) still count to avoid false positives. Suppressed for superglobals, `isset()`/`empty()` guards, `compact()` references, `extract()` calls, variable variables (`$$`), `@` error suppression, and `@var` annotations. Static property accesses (`self::$prop`, `static::$prop`, `parent::$prop`) are excluded. Variables passed to by-reference parameters are recognized as definitions: 40+ built-in PHP functions are covered (regex, cURL, OpenSSL, sockets, DNS, etc.), and user-defined functions, static methods, and constructors with `&$param` parameters are detected automatically from their signatures. Scoping is tracked through arbitrary nesting of closures, arrow functions, and catch blocks. Top-level code outside functions is skipped.
- **By-reference parameter type inference for method, static, and constructor calls.** When a variable is passed to a by-reference parameter with a type hint (e.g. `function foo(Baz &$bar)`), the variable acquires that type after the call. Previously this only worked for standalone function calls. Now it also works for `$this->method()`, static method calls, and constructor calls.

### Changed

- **Fewer false-positive diagnostics.** Variable resolution now produces the same result across completions, hover, and diagnostics, eliminating cases where diagnostics disagreed about a variable's type.
- **`@phpstan-ignore` is never the preferred quickfix.** The "Ignore PHPStan error" code action is explicitly non-preferred, so editor keyboard shortcuts no longer accidentally apply it when another fix is available.
- **Generate PHPDoc infers `@return` from the function body.** Typing `/**` above a function that returns `array` now produces a specific element type (e.g. `@return list<string>`) instead of `@return array<mixed>`.
- **Faster startup.** Stub loading during initialization is significantly faster.
- **More accurate generics resolution.** Type substitution and resolution for complex nested generic types is more correct, particularly for unions, intersections, array shapes, and deeply nested generic arguments.
- **More accurate type predicates.** `NULL`, `Null`, and case variants of `null` are now handled consistently throughout type checking, matching PHP's case-insensitive treatment of type keywords.
- **Go-to-definition at declaration sites returns the symbol's own location.** Class, member, and variable declaration names now return their own location instead of nothing, so editors that detect "definition == cursor" can automatically fall back to Find References. Contributed by @lucasacoutinho in https://github.com/AJenbo/phpantom_lsp/pull/76.

### Fixed

- **Completion no longer triggers on the `<?php` open tag.** Typing `<?php` and pressing enter no longer applies a spurious function suggestion like `php_ini_loaded_file()`.
- **Case-insensitive `parent` handling in chained static calls.** `resolve_lhs_to_class` now handles `parent::method(...)` in chained callable expressions and uses case-insensitive matching for `self`/`static` in the same context.
- **Intersection types preserved through resolution.** Variables and parameters with intersection types (e.g. `Countable&Serializable`) now display correctly in hover, extract-function parameter hints, and generated docblocks. Previously intersection types were flattened to unions (`Countable|Serializable`).
- **Return types now carry class info through the resolution pipeline.** Method and function return types that name a class (e.g. `Collection<User>`) now populate the resolved class info eagerly, so downstream consumers (hover, narrowing, completion) no longer need a second resolution pass.
- **Generic parameters preserved on resolved types.** Catch clause variables, pass-by-reference parameters, closure parameters, and constructor calls now thread the original type hint (including generic parameters) through the resolution pipeline instead of discarding it.
- **Type-guard narrowing no longer drops class info on unions.** Narrowing a union like `Foobar|string|int` with `is_string()`/`is_int()` in elseif chains now correctly preserves class info for the remaining class member.
- **False-positive undefined-variable diagnostic on static property access.** `self::$prop`, `static::$prop`, and `ClassName::$prop` no longer trigger an undefined variable warning. Dynamic forms (`self::$$prop`, `self::${expr}`) still correctly flag undefined variables used in the expression. Contributed by @lucasacoutinho in https://github.com/AJenbo/phpantom_lsp/pull/75.
- **Case-insensitive `self`, `static`, and `parent` resolution.** `SELF::method()`, `Static::create()`, `PARENT::foo()`, and other non-lowercase spellings now resolve correctly. Previously only the exact lowercase forms were recognized.
- **Property type resolution in call arguments.** When a method argument is `$this->prop` and the property has a generic, nullable, or union type, the full type structure is now preserved. Previously only the base class name was extracted, discarding generics and union components.
- **Update docblock enrichment comparison.** The "Update docblock" code action now uses structural type comparison instead of string equality when deciding whether a `@param` type needs enrichment. Types that are semantically equivalent but formatted differently (e.g. `\App\User` vs `App\User`) no longer trigger spurious updates. Body-based `@return` enrichment now correctly detects when an existing `@return` tag already has type structure, instead of always proposing a replacement.
- **`@phpstan-assert` and `@psalm-assert` tags with generic types.** Assertions like `@phpstan-assert Collection<int, User> $param` now parse the full generic type instead of truncating at the first space inside angle brackets.
- **`parent::method()` resolution in inline arguments.** Passing `parent::method()` as an argument to a function now resolves the return type correctly, matching the existing handling for `self::` and `static::`.
- **Laravel Eloquent Builder and Collection type resolution.** Generic and nullable types on Eloquent models (e.g. `Collection<int, User>`, `?User`) now resolve correctly when used for Builder scope injection, custom collection swapping, and relationship chain inference. Previously these types were stringified with their generic parameters or nullable prefix, causing lookups to fail silently.
- **Docblock generation no longer panics on lines with multibyte characters.** Files containing non-ASCII characters (e.g. accented letters) could cause the `/**` docblock trigger to crash or produce misaligned edits due to a mismatch between UTF-16 column offsets and byte offsets.
- **Conditional return types showing `mixed` in hover.** When a method with a conditional return type (e.g. `@phpstan-return ($type is class-string<T> ? T : mixed)`) resolved to a concrete class, hover still displayed the method's declared return type (`mixed`) instead of the resolved class. Affects methods like Symfony's `SerializerInterface::deserialize()`.
- **Method-level `@throws` types now resolve short names to FQN.** Exception types in `@throws` tags on class methods are now fully qualified using the file's `use` imports, matching the behaviour already in place for standalone functions. Cross-file throws propagation and the "Update docblock" code action produce correct results when the exception class is imported via a `use` statement.
- **Missing diagnostics and import actions in files without a namespace.** When a namespaced class (e.g. `Carbon\Carbon`) had already been parsed, using its short name (`Carbon`) in a file without a `namespace` declaration incorrectly resolved against the namespaced class. This suppressed both the "class not found" diagnostic and the "Import" code action. Bare-name lookups now only match classes that are themselves in the global namespace.
- **Find-references false positives for global classes.** Searching for references to a global-scope class (e.g. `Helper` with no namespace) could include references to unrelated namespaced classes with the same short name (e.g. `App\Helper`). Short-name fallback matching now only applies when the resolved name is unqualified.
- **Fluent chains only flag the first broken link.** In a chain where the first method does not exist, only that method is flagged instead of every subsequent call receiving its own warning.
- **Null narrowing from `!== null` checks.** Null-initialized variables guarded by `$var !== null`, `!is_null()`, or bare truthy checks now have `null` narrowed away inside the then-body and in subsequent `&&` operands. Works in chained conditions, ternary expressions, and return statements.
- **Variables assigned inside `if`/`while` conditions now resolve in the body.** `if ($admin = AdminUser::first())` and `while ($row = nextRow())` register the assignment so the variable has a type inside the branch or loop body.
- **Loop-body assignments not visible inside the same loop iteration.** When a variable is initialized as `null` and reassigned later in a loop body, the assigned type is now visible at every point inside the loop. Combined with null narrowing, variables correctly resolve to the assigned class type.
- **`@var` docblock annotations no longer leak across class and method boundaries.** A `@var` annotation for a same-named variable in a different class no longer bleeds into the current scope.
- **Inline `@var` cast no longer overrides the variable type on the RHS of the same assignment.** `/** @var array<string, mixed> */ $data = $data->toArray()` no longer resolves the RHS `$data` using the cast type.
- **Foreach over union types containing arrays now resolves the element type.** A parameter typed `User|array<User>` iterated with `foreach` now correctly yields `User` as the loop variable type. Previously the element type extraction did not look inside union members, producing no completions.
- **`@param` docblock overrides ignored when the native type hint resolves.** When a parameter has both a native type hint and a more specific `@param` override, the docblock type now takes effect. Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/55.
- **Variable reassignment inside `try`/`catch`/`finally` blocks now tracked.** Subsequent accesses within the same block resolve against the reassigned type instead of the original.
- **Self-referential variable reassignments in nested loops no longer produce false "type could not be resolved" diagnostics.** Recursive resolution that hits the depth limit no longer poisons the cache for later lookups.
- **`instanceof` narrowing with unresolvable target class.** When the target class cannot be loaded, the variable's type is treated as unknown instead of keeping the un-narrowed type, eliminating false positives for members on the narrowed subclass.
- **`stdClass` and `object` types no longer produce false-positive diagnostics.** Variables typed as `object` or `stdClass` now permit arbitrary property access. `is_object()` correctly narrows `mixed` to `object` and compound `&&` conditions propagate the narrowing.
- **Docblock type refinement no longer matches class names containing type keywords.** A class named `PointOfInterest` would incorrectly be treated as an `int` refinement because the refinement check used substring matching. Refinement compatibility now uses structural type predicates.
- **`class-string<T>` static method dispatch.** Calling static methods on a `class-string<Foo>` variable now resolves return types correctly, including `static` substitution to the bound class.
- **`self`/`static`/`$this` in cross-file method return types now resolve correctly.** When a method on a cross-file class returns a type referencing `self` (e.g. `@return HasMany<self, $this>`), the owning class was looked up by short name through the consuming file's import table, which failed when the consuming file did not import that class. The owning class is now looked up by its fully-qualified name.
- **`in_array` guard clause no longer wipes out variable type.** When the haystack's element type matches the variable's type, the narrowing system no longer excludes the type entirely.
- **Method chains through `__call` no longer lose the return type.** When `__call` returns `$this`, `static`, or `self`, the chain type is preserved through dynamic method calls.
- **Scope methods on Eloquent Builder no longer produce false-positive diagnostics.** Bare `Builder` return types on scope methods are automatically wrapped as `Builder<ConcreteModel>` to preserve the chain.
- **Scope methods missing from completion on relationship results.** Scope methods from related models now appear in completions, not just hover.
- **Closure and variable hover now preserves generic arguments.** Closure parameters inferred from callable signatures, variables assigned from chained methods returning `static`/`$this`/`self`, and hovering on the `$` sign of a variable at its assignment site all now show the correct generic type.
- **Callable parameter inference preserves generic arguments from the receiver.** A closure typed as `fn(Builder $q)` inside a `Builder<Product>` chain now infers `$q` as `Builder<Product>`, so model-specific scope methods resolve correctly.
- **`@see` tags in floating docblocks now support go-to-definition.** Docblock comments not directly attached to a class, function, or statement (e.g. inline `/** @see SupervisorOptions::$balanceCooldown */` inside array literals or after expressions) are now parsed for symbol references. Previously these were silently ignored, particularly in files without a namespace.
- **Nullable `static` return types on inherited methods.** Methods returning `?static` or `static|null` now correctly resolve to the calling subclass across files.
- **Template binding with nested generics.** Parameter types like `Wrapper<Collection<T>, V>` no longer break during template binding.
- **Single generic argument on collections bound to the wrong template parameter.** `Collection<User>` now binds to the value parameter instead of the key parameter when key-like template parameters precede value parameters.
- **Nullable return types losing `|null` after template substitution.** `@return TValue|null` now preserves `|null` through substitution, so calls like `::first()` correctly show the nullable type.
- **`@mixin` referencing a template parameter now resolves.** A class with `@template T` and `@mixin T` now pulls in methods from the concrete type passed via generic arguments.
- **`@property` and `@method` tags losing nullable types.** Tags like `@property int|null $foo` no longer have `|null` stripped.
- **Callable types inside unions displayed ambiguously.** `(Closure(int): string)|Foo` is now parenthesized correctly in hover and completions.
- **Hover and go-to-definition on attributes.** Attributes on properties, class constants, parameters, and enum cases are now navigable.
- **Function-level `@template` with generic wrapper parameters.** Template substitution at call sites now correctly handles `array`, `iterable`, and `list` as wrapper names.
- **Closure parameter inference from function-level `@template` bindings.** Functions like `array_any` and `array_all` now infer concrete types for untyped closure arguments from the array parameter's element type.
- **Property chain arguments in template substitution.** Expressions like `$this->items` passed to templated functions now resolve their type for template binding.
- **Variadic parameter element type lost in `foreach`.** Iterating over a variadic parameter now resolves the loop variable to the element type.
- **Anonymous class variables now resolve their type.** `$model = new class extends Foo { ... }` followed by `$model->method()` now resolves through the anonymous class's inherited members.
- **Namespaced functions imported via `use function` no longer flagged as unknown.** Functions defined in one file and imported via `use function` in another now resolve correctly.
- **`parent::method()` return type resolution in variable analysis.** Calling `parent::method()` and assigning the result now correctly resolves the parent method's return type.
- **Closure parameter inference inside `switch` cases and `if` conditions.** Closure parameters that should be inferred from the enclosing callable context now resolve correctly when the closure appears inside a switch case or if-condition.
- **Generic arguments propagated through transitive `@extends` chains.** When a class extends a parent that itself extends a generic grandparent, generic arguments now flow through the full chain.
- **Stack overflow when a foreach value variable shadows the iterator receiver.** Patterns like `foreach ($category->getBranch() as $category)` no longer cause infinite recursion.
- **PHPStan `*` wildcard in generic type arguments.** Type strings like `Relation<TRelatedModel, *, *>` now parse correctly.
- **Types with `covariant` or `contravariant` variance annotations in generic args now parse correctly.** Annotations like `BelongsTo<Category, covariant $this>` no longer cause the entire type to become unresolvable.
- **Diagnostics now work for vendor files open in the editor.** Projects using `--prefer-source` or monorepo setups no longer have diagnostics suppressed in vendor files.
- **PHPStan diagnostics no longer hidden by unrelated native diagnostics on the same line.** Deduplication now only suppresses a full-line diagnostic when the precise diagnostic on the same line reports a related issue.
- **Nullable boolean properties now use `is` prefix for getters.** Properties typed `?bool` or `?boolean` now generate `isFoo()` instead of `getFoo()` when using the "Generate getter" code action.
- **Aliased namespace imports used in attributes no longer flagged as unused.** `use Symfony\Component\Validator\Constraints as Assert;` with `#[Assert\Uuid(...)]` no longer produces a false "Unused import" diagnostic.
- **`DB::select()` return type.** `DB::select()` and related methods now return `array<int, stdClass>` instead of bare `array`, and `DB::selectOne()` returns `?stdClass`.
- **Redis `Connection` method resolution.** Redis commands on `Illuminate\Redis\Connections\Connection` now resolve through the phpredis stubs.
- **Array shape tracking from keyed assignments inside conditional branches.** Shape types built incrementally with variable keys inside loops with if/else branching are now preserved through foreach iteration.
- **Deprecated class in `implements` renders with strikethrough.** Deprecated classes referenced in `implements` clauses are correctly tagged.
- **Interleaved array access and property chains no longer produce false positives.** Expressions like `$results[$i]->activities[$id]->extras` where array subscript and property access alternate were incorrectly parsed, causing the intermediate property chain to be dropped. This led to "Property not found on class" false positives when the element type was resolved but the subsequent property lookup was skipped.
- **FQN `\assert()` now narrows types.** Writing `\assert($var instanceof Foo)` with a leading backslash was not recognized as an instanceof narrowing, causing false-positive "property not found" diagnostics after the assertion.
- **Generic template substitution producing invalid types.** When a template parameter was the base of a generic type (e.g. `T<int>` where `T` maps to `Collection<string>`), the substitution produced malformed types like `Collection<string><int>`. The replacement's base name is now used correctly, yielding `Collection<int>`.

## [0.6.0] - 2026-03-26

### Added

- **Semantic Tokens.** Type-aware syntax highlighting that goes beyond what a TextMate grammar can achieve. Classes, interfaces, enums, traits, methods, properties, parameters, variables, functions, constants, and template parameters all get distinct token types. Modifiers convey declaration sites, static access, readonly, deprecated, and abstract status.
- **PHPStan diagnostics.** PHPStan errors appear inline as you edit. Auto-detects `vendor/bin/phpstan` or `$PATH`. Runs in the background without blocking native diagnostics. Configurable via `[phpstan]` in `.phpantom.toml` (`command`, `memory-limit`, `timeout`). "Ignore PHPStan error" and "Remove unnecessary @phpstan-ignore" code actions manage inline ignore comments.
- **Formatting.** Built-in PHP formatting (PER-CS 2.0 style). Formatting works out of the box without any external tools. Projects that depend on php-cs-fixer or PHP_CodeSniffer in their `composer.json` `require-dev` automatically use those tools instead (both can run in sequence). Per-tool command overrides and disable switches in `[formatting]` in `.phpantom.toml`.
- **Inlay hints.** Parameter name and by-reference indicators appear at call sites. Hints are suppressed when the argument already makes the parameter obvious: variable names matching the parameter, property accesses with a matching trailing identifier, string literals whose content matches, well-known single-parameter functions like `count` and `strlen`, and spread arguments. Named arguments never receive a redundant hint.
- **PHPDoc block generation.** Typing `/**` above any declaration generates a docblock skeleton. Tags are only emitted when the native type hint needs enrichment. Properties and constants always get `@var`. Class-likes with templated parents or interfaces get `@extends`/`@implements` tags. Uncaught exceptions get `@throws` with auto-import. Works both via completion and on-type formatting.
- **Syntax error diagnostic.** Parse errors from the Mago parser now appear as Error-severity diagnostics instantly as you type.
- **Implementation error diagnostic.** Concrete classes that fail to implement all required methods from their interfaces or abstract parents are now flagged with an Error-severity diagnostic on the class name. The existing "Implement missing methods" quick-fix appears inline alongside the error.
- **Argument count diagnostic.** Flags function and method calls that pass too few arguments. The "too many arguments" check is off by default (PHP silently ignores extra arguments) and can be enabled with `extra-arguments = true` in the `[diagnostics]` section of `.phpantom.toml`.
- **Completion item documentation.** Selecting a completion item in the popup now shows rich documentation including the full typed signature, description, deprecation notice, and parameter details. Previously only the class name was shown.
- **Method commit characters.** Typing `(` while a method completion is highlighted auto-accepts it and begins the argument list.
- **Document Symbols.** The outline sidebar and breadcrumbs now show classes, interfaces, traits, enums, methods, properties, constants, and standalone functions with correct nesting, icons, visibility detail, and deprecation tags.
- **Workspace Symbols.** "Go to Symbol in Workspace" (Ctrl+T / Cmd+T) searches across all indexed files including vendor classes. Results include namespace context and deprecation markers, sorted by relevance.
- **Type Hierarchy.** "Show Type Hierarchy" on any class, interface, trait, or enum reveals its supertypes and subtypes with full up-and-down navigation through the inheritance tree, including cross-file resolution and transitive relationships.
- **Code Lens.** Clickable annotations above methods that override a parent class method or implement an interface method. Clicking navigates to the prototype declaration.
- **Update docblock.** Code action on a function or method whose existing docblock is out of sync with its signature. Adds missing `@param` tags, removes stale ones, reorders to match the signature, fixes contradicted types, and removes redundant `@return void`. Refinement types and unrelated tags are preserved. Only triggers on the signature or the preceding docblock, not inside the function body.
- **Change visibility.** Code action on any method, property, constant, or promoted constructor parameter offers to change its visibility (`public`, `protected`, `private`). Only triggers on the declaration signature, not inside the body.
- **`@throws` code actions.** Quick-fixes for adding missing and removing unnecessary `@throws` tags, triggered by PHPStan diagnostics. Adding inserts the tag and a `use` import when needed. Removing cleans up orphaned blank lines and deletes the entire docblock when it would be empty. The diagnostic disappears on the next keystroke without waiting for the next PHPStan run.
- **File rename on class rename.** Renaming a class whose file follows PSR-4 naming now also renames the file to match. The file is only renamed when it contains a single class-like declaration and the editor supports file rename operations.
- **Folding Ranges.** AST-aware code folding for class bodies, method/function bodies, closures, arrays, argument/parameter lists, control flow blocks, doc comments, and consecutive single-line comment groups.
- **Selection Ranges.** Smart select / expand selection returns AST-aware nested ranges from innermost to outermost.
- **Document Links.** `require`/`include` paths are now Ctrl+Clickable. Path resolution supports string literals, `__DIR__` concatenation, `dirname(__DIR__)`, `dirname(__FILE__)`, and nested `dirname` with levels.
- **Analyze command.** `phpantom_lsp analyze` scans a Composer project and reports PHPantom's own diagnostics in a PHPStan-like table format. Useful for measuring type coverage across an entire codebase without opening files one by one. Accepts an optional path argument to limit the scan to a single file or directory. Output includes diagnostic identifiers and supports `--severity` filtering and `--no-colour` for CI.
- **Null-coalesce (`??`) type refinement.** When the left-hand side of `??` is provably non-nullable (e.g. `new Foo()`, `clone $x`, a literal), the right-hand side is recognized as dead code and the result resolves to the LHS type only. When the LHS is nullable (e.g. a `?Foo` return type), `null` is stripped from the LHS and the result is the union of the non-null LHS with the RHS.
- **`@mixin` generic substitution.** When a class declares `@mixin Foo<T>`, the generic arguments are now preserved and substituted into the mixin's members, including through multi-level inheritance chains.
- **PHPDoc `@var` completion.** Inline `@var` above variable assignments sorts first and pre-fills the inferred type when available. Template parameters from `@template` enrich `@param`, `@return`, and `@var` type hints.
- **`@see` and `@link` improvements.** `@see` references in docblocks now work with go-to-definition (class, member, and function forms). Hover popups show all `@link` and `@see` URLs as clickable links. Deprecation diagnostics include `@see` targets when the `@deprecated` docblock references them.
- **Progress indicators.** Go to Implementation and Find References now show a progress indicator in the editor while scanning.
- **Phar archive class resolution.** Classes inside `.phar` archives (e.g. PHPStan's `phpstan.phar`) are now discovered and indexed automatically. No PHP runtime needed. Only uncompressed phars are supported (the format used by PHPStan and most other phar-distributed tools).
- **PSR-0 autoload support.** Packages that use the legacy PSR-0 autoloading standard are now discovered automatically.
- **Global config.** Settings from a global `.phpantom.toml` in the user's config directory (typically `~/.config/phpantom_lsp/.phpantom.toml`) are now loaded as defaults. Project-level configs take precedence. Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/39.
- **Config schema.** A JSON schema for `.phpantom.toml` is now bundled, enabling autocompletion and validation in editors that support TOML schemas. Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/38.

### Changed

- **Pull diagnostics.** Diagnostics are now delivered via the LSP 3.17 pull model when the editor supports it. The editor requests diagnostics only for visible files, and cross-file invalidation no longer recomputes every open tab. Clients without pull support fall back to the previous push model automatically.
- **Hover type accuracy.** Hover now resolves variable types through the same pipeline as completion, so all narrowing features (instanceof, assert, custom type guards, in_array) apply. When the cursor is inside a specific if/else branch, hover shows only the type visible in that branch. Complex expressions like null-coalesce chains, array shapes, empty arrays, and unresolved symbols all display correctly.
- **Version-aware stub types.** Built-in function signatures that changed across PHP versions (e.g. `int|false` in 7.x becoming `int` in 8.0) now show the correct type for your project's PHP version. This eliminates false-positive diagnostics and incorrect completions from stale type annotations.
- **Completion labels.** Method and function completion items now show only parameter names in the label (e.g. `setName($name)`) with the return type displayed inline (e.g. `: User`). Properties and constants show just the type hint. The previous `Class: ClassName` detail line has been removed; class context is available in the documentation panel when the item is highlighted.
- **Completion sort order.** Member completion items are now sorted by kind (constants, then properties, then methods) before alphabetical order within each group. Union-type completions apply the same kind-based ordering within both the intersection and branch-only tiers.
- **Class name completion ranking.** Completions now rank by match quality first (exact match, then starts-with, then substring), so typing `Order` puts `Order` above `OrderLine` above `CheckOrderFlowJob` regardless of where the class comes from. Within each match quality group, use-imported and same-namespace classes appear first, followed by everything else sorted by namespace affinity (classes from heavily-imported namespaces rank higher).
- **Use-import completion.** Same-namespace classes no longer appear in `use` statement completions (PHP auto-resolves them without an import). Classes that are already imported are filtered out. Namespace affinity still ranks the remaining candidates.
- **Deprecation tags.** Completion items use the modern `tags: [DEPRECATED]` field instead of the legacy `deprecated` boolean. Both convey the same strikethrough rendering in editors.
- **Import class code action ordering.** The "Import Class" code action now sorts candidates by namespace affinity (derived from existing imports) instead of alphabetically, so the most likely namespace appears first.
- **Cross-file resolution.** Completion, hover, and go-to-definition no longer fail when one reference uses a leading backslash and another does not.
- **Embedded stubs track upstream master.** The bundled phpstorm-stubs are now pulled from the `master` branch instead of the latest GitHub release, matching what PHPStan does. This brings in upstream fixes and new PHP version annotations weeks or months before a formal release.

### Fixed

- **CLI analyze performance.** Single-file analysis is up to 5.8× faster. Full-project analysis of ~2 500 files is up to 10× faster.
- **Diagnostic performance on large files.** Unknown-member diagnostics on files with many member accesses are up to 7× faster.
- **Position encoding.** All LSP position conversions now correctly count UTF-16 code units, matching the LSP specification. Files containing emoji or supplementary Unicode characters no longer produce incorrect positions.
- **Rename and find references for parameters.** Renaming a parameter in a function, method, or closure now correctly updates all usages in the body and the `@param` tag in the docblock. Previously, parameters were scoped incorrectly because they sit physically before the opening `{` of the body, causing rename and find references to miss body usages when triggered from the parameter (and vice versa). Document highlight is also fixed.
- **Rename updates imports.** Renaming a class now updates `use` statement FQNs, preserves explicit aliases, and introduces an alias when the new name collides with an existing import.
- **False-positive diagnostics for `$this` inside traits.** Accessing host-class members via `$this->`, `self::`, `static::`, or `parent::` inside a trait method no longer produces "not found" warnings, including chain expressions and accesses inside closures or arrow functions nested within trait methods.
- **False-positive diagnostics for same-named variables in different methods.** Diagnostic resolution is now scoped to the enclosing function/method/closure body, so two methods using a variable like `$order` resolve it independently.
- **False positive on namespaced constants.** Standalone namespaced constant references (e.g. `\PHPStan\PHP_VERSION_ID`) no longer produce a spurious "Class not found" diagnostic. Previously the symbol map classified them as class references instead of constant references.
- **Diagnostic deduplication.** Multiple diagnostics on the same span or line are no longer collapsed into one. If PHPStan reports five issues on a line, all five are shown. When PHPantom and PHPStan both flag the same issue, the more precise native diagnostic wins.
- **Diagnostics.** Enums that implement interfaces are now checked for missing methods. Scalar member access errors detect method-return chains where an intermediate call returns a scalar type. By-reference `@param` annotations no longer produce a false "unknown class" diagnostic.
- **Removed PHP symbols in stubs.** Functions, methods, and classes annotated with `@removed X.Y` in phpstorm-stubs are now filtered out when the target PHP version is at or above the removal version. Previously symbols like `mysql_tablename` (removed in PHP 7.0) and `each` (removed in PHP 8.0) appeared in completions and resolved without warnings.
- **Hover on union member access.** Hovering over a method, property, or constant on a union type (e.g. `$ambiguous->turnOff()` where `$ambiguous` is `Lamp|Faucet`) now shows hover information from all branches that declare the member, separated by a horizontal rule. Previously only the first matching branch was shown. When both branches inherit the member from the same declaring class, the hover is deduplicated to a single entry.
- **Hover on inherited members.** Hovering over an inherited method, property, or constant now shows the declaring class in the code block (e.g. `class Model { public static function find(...) }`) instead of the class it was accessed on. Previously `User::find()` would incorrectly show `class User` even though `find()` is declared on `Model`.
- **Constant type inference.** Variables assigned from global constants (`$a = MY_CONST`) or class constants without type hints (`$b = Config::TIMEOUT`) now resolve to the type implied by the constant's initializer value. Integer, float, string, bool, null, and array literals are all recognised. Typed class constants (`public const string NAME = '...'`) continue to use their declared type hint.
- **Variable type after reassignment.** When a method parameter is reassigned mid-body (e.g. `$file = $result->getFile()`), subsequent member accesses now resolve against the new type instead of the original parameter type.
- **Variable assignments inside foreach loops.** Variables conditionally reassigned inside a `foreach` body are now visible after the loop.
- **Variable-to-variable type propagation.** Assignments like `$found = $pen` now resolve `$found` to the type of `$pen`. This also eliminates false-positive diagnostics when the initial assignment was `$found = null` and a later reassignment provided the real type.
- **Variable type inside self-referencing assignment RHS.** In `$request = new Foo(arg: $request->uuid)`, the `$request` reference inside the constructor arguments now correctly resolves to the original type instead of the type being assigned.
- **Variable resolution inside anonymous classes.** Variables inside anonymous class methods (e.g. closure parameters in `return new class extends Migration { ... }`) now resolve correctly. Previously, anonymous class bodies were invisible to the variable resolution pipeline because they appear as expressions inside statements rather than top-level class declarations.
- **Closure and arrow function variable scope.** Variable name completion now correctly respects PHP scoping rules for anonymous functions and arrow functions. Parameters and `use`-captured variables are visible inside closures. Arrow function parameters are visible inside the arrow body while the enclosing scope's variables remain accessible.
- **Function return type resolution across files.** Standalone functions that declare return types using short names from their own `use` imports now resolve correctly in consuming files. Function parameter types and `@throws` types are also resolved.
- **Native type override compatibility.** A docblock type only overrides a native type hint when it is a compatible refinement (e.g. `class-string<Foo>` can refine `string`, but `array<int>` no longer incorrectly overrides `string`).
- **PHPStan pseudo-type recognition.** Types like `non-positive-int`, `non-negative-int`, `non-zero-int`, `lowercase-string`, `truthy-string`, `callable-object`, and many other PHPStan pseudo-types are now recognized across the entire pipeline.
- **Nullable and generic types in class lookup.** Variables typed as `?ClassName` or `Collection<Item>` now resolve correctly across all code paths.
- **Generic substitution through transitive interface chains.** When a class implements an interface that itself extends another generic interface, template parameters are now substituted at each level instead of propagating raw template parameter names.
- **Generic shape substitution.** Template parameters inside array shapes (`array{data: T}`) and object shapes (`object{name: T}`) are now correctly substituted when inherited through `@extends`.
- **Type narrowing with same-named classes from different namespaces.** instanceof narrowing now correctly distinguishes classes that share a short name but live in different namespaces (e.g. `Contracts\Provider` vs `Concrete\Provider`).
- **Guard clause narrowing across instanceof branches.** After `if ($x instanceof Y) { return; }`, subsequent `instanceof` checks on the same variable no longer incorrectly resolve to `Y`.
- **`instanceof self/static/parent` narrowing.** Type narrowing with `instanceof self`, `instanceof static`, and `instanceof parent` now works correctly in all contexts (assert, if-blocks, guard clauses, compound conditions).
- **Type narrowing inside `return` statements.** `instanceof` checks in `&&` chains and ternary conditions now narrow the variable type when the expression is the operand of a `return` statement.
- **Inline array access on method returns.** Expressions like `$c->items()[0]->getLabel()` now resolve the element type correctly for both completion and diagnostics.
- **Array shape bracket access.** Variables assigned from string-key bracket access on array shapes (`$name = $data['name']`) now resolve to the correct value type. Chained access (`$first = $result['items'][0]`) walks through shape keys and generic element types in sequence.
- **Ternary and null-coalesce member access.** Accessing a member on a ternary or null-coalesce expression (e.g. `($a ?: $b)->property`, `($x ?? $y)->method()`) now resolves correctly for hover, go-to-definition, and diagnostics.
- **Null-safe method chain resolution.** Null-safe method calls (`$obj?->method()`) now resolve the return type correctly for variable type inference, including cross-file chains.
- **Clone expressions.** `(clone $var)->` now resolves to the same type as `$var`, providing correct completion, hover, and diagnostics.
- **`self::/static::/parent::` in member access chains.** Expressions like `self::Active->value` inside an enum method now resolve correctly. Previously, `self`, `static`, and `parent` were only recognized as bare subjects, not when followed by `::MemberName` in a chain.
- **Inherited methods missing through deep stub chains.** Methods are now found on classes that inherit through multi-level chains where intermediate classes live in stubs.
- **Interface constants through multi-extends chains.** Constants defined on parent interfaces are now found when an interface extends multiple other interfaces.
- **Double parentheses when completing calls.** Completing a function, constructor, or static method name when parentheses already follow the cursor (e.g. `array_m|()`, `new Gadge|()`, `throw new Excepti|()`) no longer inserts a second pair of parentheses. Previously only `->` and `::` method calls were handled.
- **Namespace alias completion.** Typing a class name through a namespace alias (e.g. `OA\Re` with `use OpenApi\Attributes as OA`) now correctly suggests classes under the aliased namespace.
- **Catch clause completion.** Throwable interfaces and abstract exception classes now appear in catch clause completions.
- **Type-hint and PHPDoc completion.** Traits are now excluded from completions in parameter types, return types, property types, and PHPDoc type tags. `@throws` continues to use Throwable-filtered completion.
- **Trait alias go-to-definition.** Clicking a trait alias (e.g. `$this->__foo()` from `use Foo { foo as __foo; }`) now jumps to the trait method instead of the class's own same-named method.
- **Self-referential array key assignments no longer crash.** Patterns like `$numbers['price'] = $numbers['price']->add(...)` no longer cause a stack overflow during hover or completion.
- **Eloquent `morphedByMany` relationships.** The inverse side of polymorphic many-to-many relationships is now recognised. Virtual properties and `_count` properties are synthesized for models using this relationship type.
- **Virtual property merging.** Native type hints are now considered when determining virtual property specificity, preventing properties with native PHP type declarations from being incorrectly overridden by less specific virtual properties.

## [0.5.0] - 2026-03-12

### Added

- **Diagnostics.** Unknown classes, unknown members, and unknown functions are flagged with appropriate severity. An opt-in unresolved member access diagnostic is available via `.phpantom.toml`.
- **Find References.** Locate every usage of a symbol across the project. Supports classes, methods, properties, constants, functions, and variables. Variable references are scoped to the enclosing function or closure. Member references are scoped to the class hierarchy, so unrelated classes sharing a method name are excluded.
- **Rename.** Rename variables, classes, methods, properties, functions, and constants across the workspace. Variable renames are scoped to their enclosing function or closure.
- **Deprecation support.** `@deprecated` tags and `#[Deprecated]` attributes surface in hover, completion strikethrough, and diagnostics. A quick-fix code action rewrites deprecated calls when a `replacement` template is available.
- **Document highlighting.** Placing the cursor on a symbol highlights all occurrences in the current file. Variables are scoped to their enclosing function or closure with write vs. read distinction.
- **Implement missing methods.** Code action that generates method stubs when a class is missing required interface or abstract method implementations.
- **Project configuration.** `.phpantom.toml` for per-project settings: PHP version override, diagnostic toggles, and indexing strategy. Run `phpantom --init` to generate a default config.
- **Reverse go-to-implementation.** Go-to-implementation on a concrete method jumps to the interface or abstract class that declares the prototype, and vice versa.
- **Go to Type Definition.** Jump from a variable, property, method call, or function call to the class declaration of its resolved type. Union types produce multiple locations.
- **Self-generated classmap.** PHPantom works without `composer dump-autoload -o`. Missing or incomplete classmaps are supplemented by scanning autoload directories. Non-Composer projects are supported by scanning all PHP files.
- **Monorepo support.** Discovers subdirectories that are independent Composer projects and processes each through the full pipeline.
- **`@implements` generic resolution.** `@implements Interface<ConcreteType>` substitutes template parameters on the interface's methods and properties. Foreach iteration on generic iterable interfaces resolves value and key types.
- **Interface template inheritance.** Implementing classes inherit `@template` parameters, bindings, conditional return types, and type assertions from their interfaces.
- **Function-level `@template` with generic return types.** Functions that use `@template` parameters inside generic return types now resolve concrete types from call-site arguments.
- **Generic `@phpstan-assert` with `class-string<T>`.** Assertion methods that accept a `class-string<T>` parameter resolve the narrowed type from the call-site argument.
- **Property-level narrowing.** `if ($this->prop instanceof Foo)` narrows `$this->prop` in then/else bodies and after guard clauses.
- **Inline `&&` short-circuit narrowing.** The right-hand side of `&&` now sees the narrowed type from the left-hand side.
- **Compound negated guard clause narrowing.** `if (!$x instanceof A && !$x instanceof B) { return; }` narrows `$x` to `A|B` in the surviving code.
- **Closure variable scope isolation.** Variables outside a closure are no longer offered as completions unless captured via `use()`.
- **Pipe operator (PHP 8.5).** `$input |> trim(...) |> createDate(...)` resolves through the chain.
- **AST-based array type inference.** Array shape keys, element access, spread elements, and push-style assignments all resolve through an AST walker.
- **`new $classStringVar` and `$classStringVar::method()`.** Class-string variables resolve for `new` and static member access.
- **Invoked closure and arrow function return types.** `(fn(): Foo => ...)()` and `(function(): Bar { ... })()` resolve to their return type.
- **Docblock navigation.** Go-to-definition and hover work on class names inside callable types, array/object shape value types, and object shape properties.
- **GTD from parameter and property variables.** Clicking a parameter or property at its definition site jumps to the type hint class.
- **PHP version-aware stubs.** Detects the target PHP version from `composer.json` and filters built-in stub signatures accordingly.
- **`@param-closure-this`.** `$this` inside a closure resolves to the type declared by `@param-closure-this` on the receiving parameter.
- **Non-Composer function and constant discovery.** Cross-file function completion, go-to-definition, and constant resolution for projects without `composer.json`.
- **Indexing progress indicator.** The editor shows a progress bar during workspace initialization, including per-subproject progress in monorepos.
- **Pass-by-reference parameter type inference.** After calling a function with a typed `&$var` parameter, the variable acquires that type.
- **`iterator_to_array()` element type.** Resolves the element type from the iterator's generic annotation.
- **Enum case properties.** `$case->name` and `$case->value` resolve on enum case variables.
- **Inline `@var` on promoted constructor properties.** Overrides the native type hint, matching existing `@param` support.
- **`--version` and `--help` CLI flags.** Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/7.

### Changed

- **Resolution engine rewritten on AST.** Variable type inference, call return types, and go-to-definition all run through the AST walker for better accuracy.
- **Hover redesigned.** Short names with `namespace` line, actual default values, `@link` URLs, precise token highlighting, constructor signatures on `new`, `@template` details, enum case listing, trait member listing, origin indicators, and deprecated explanations.
- **Signature help enriched.** Compact parameter list with native types, per-parameter `@param` descriptions, default values, and attribute parenthesis support.
- **Faster resolution and lower memory usage.**
- **Parallel workspace indexing.** File parsing, PSR-4 scanning, and vendor scanning run across all CPU cores. `.gitignore` rules are respected.
- **Two-phase diagnostic publishing.** Cheap diagnostics (unused imports, deprecation) publish immediately; expensive diagnostics (unknown classes/members/functions) arrive in a second pass.
- **Merged classmap + self-scan pipeline.** Composer classmaps and self-scanning work together instead of being mutually exclusive. Stale classmaps are supplemented automatically.
- **Automatic stub fetching.** The build script downloads phpstorm-stubs automatically when missing. Composer is no longer needed to build PHPantom. Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/16.
- **Feature comparison table corrected.** Phactor capabilities updated in the README. Contributed by @dantleech in https://github.com/AJenbo/phpantom_lsp/pull/10.

### Fixed

- **Cross-file inheritance from global-scope classes imported via `use`.**
- **Inherited `@method` and `@property` tags across files.**
- **Diagnostics refresh across open files when a class signature changes.**
- **Variable types resolve through ternary, elvis, null-coalesce, and match assignments.**
- **`instanceof` narrowing no longer widens specific types.**
- **Elseif chain narrowing and sequential assert narrowing.**
- **`@phpstan-type` aliases in foreach, `list()`, and key types.**
- **False-positive unknown-class warnings on PHPStan type syntax.**
- **Go-to-implementation no longer produces false positives across namespaces.**
- **`__invoke()` return type resolution.** Works with chaining, foreach, and parenthesized invocations.
- **Enum `from()` and `tryFrom()` chaining.**
- **`static`/`self`/`$this` in method return types used as iterable expressions.**
- **Mixed `->` then `::` accessor chains.**
- **Inline `(new Foo)->method()` chaining.**
- **`?->` null-safe chain resolution.**
- **Array function resolution for `array_pop`, `array_filter`, `array_values`, `end`, `array_map`.**
- **Inline `@var` annotations no longer leak across scopes.**
- **Literal string conditional return types.**
- **Class constant and enum case assignment resolution.**
- **Go-to-definition on trait `as` alias and `insteadof` declarations.**
- **Inline array-element function calls resolve correctly in diagnostics.** `end($obj->items)->method()` no longer produces a false diagnostic.
- **Double-negated `instanceof` narrowing.**
- **Self-referential array key assignments no longer crash.**

## [0.4.0] - 2026-03-01

### Added

- **Signature help.** Parameter hints in function/method calls with active parameter highlighting.
- **Hover.** Type, signature, and docblock in a Markdown popup for all symbol kinds.
- **Closure and callable inference.** Untyped closure parameters inferred from the callable signature. First-class callable syntax resolves return types.
- **Laravel Eloquent.** Relationships, scopes, Builder forwarding, factories, custom collections, casts, accessors, mutators, `$attributes`, and `$visible`.
- **Type narrowing.** `in_array()` with strict mode, early return guards, `instanceof` in ternaries and with interfaces.
- **Anonymous class support.** `$this->` resolves inside anonymous classes with full inheritance support.
- **Context-aware completions.** `extends`, `implements`, `use` inside class body, union member sorting, namespace segments, string literal suppression.
- **Additional resolution.** Multi-line chains, nested array keys, generator yield types, conditional return types with template substitution, switch/unset variable tracking.
- **Transitive interface go-to-implementation.**

### Fixed

- Visibility filtering, scope isolation, static call chains, `static` return type, trait resolution, mixin fluent chains, go-to-definition accuracy, import handling, UTF-8 boundaries, and parenthesized RHS expressions.

## [0.3.0] - 2026-02-21

### Added

- **Go-to-implementation.** Interface/abstract class to all concrete implementations.
- **Method-level `@template`.** Infers `T` from the call-site argument.
- **`@phpstan-type` / `@psalm-type` aliases** and `@phpstan-import-type`.
- **Array function type preservation.** `array_filter`, `array_map`, `array_pop`, `current`, etc.
- **Early return narrowing.** Guard clauses narrow types for subsequent code.
- **Callable variable invocation.** `$fn()->` resolves return types.
- **Additional resolution.** Spread operators, trait `insteadof`/`as`, chained assignments, destructuring, foreach on function returns, type hint completion, try-catch suggestions.

### Fixed

- PHPDoc type parsing and internal stability fixes.

## [0.2.0] - 2026-02-18

### Added

- **Generics.** Class-level `@template` with `@extends` substitution. Method-level `class-string<T>`. Generic trait substitution.
- **Array shapes and object shapes.** Key completion from literals, incremental assignments, destructuring, element access.
- **Foreach type resolution.** Generic iterables, array shapes, `Collection<User>`, `Generator<int, Item>`, `IteratorAggregate`.
- **Expression type inference.** Ternary, null-coalescing, and match expressions.
- **Additional completions.** Named arguments, variable name suggestions, standalone functions, `define()` constants, PHPDoc tags, deprecated members, promoted property types, property chaining, `require_once` discovery, go-to type definition.

### Fixed

- `@mixin` context for return types, global class imports, namespace resolution, and aliased class go-to-definition.

## [0.1.0] - 2026-02-16

Initial release.

### Added

- **Completion.** Methods, properties, and constants via `->`, `?->`, and `::` with visibility filtering.
- **Type resolution.** Inheritance merging, `self`/`static`/`parent`, union types, nullsafe chains.
- **PHPDoc support.** `@return`, `@property`, `@method`, `@mixin`, conditional return types, inline `@var`.
- **Type narrowing.** `instanceof`, `is_a()`, `@phpstan-assert`.
- **Enum support.** Case completion and `UnitEnum`/`BackedEnum` interface members.
- **Go-to-definition.** Classes, methods, properties, constants, functions, `new` expressions, variables.
- **Class name completion with auto-import.**
- **PSR-4 lazy loading and Composer classmap support.**
- **Embedded phpstorm-stubs.**
- **Zed editor extension.**

[Unreleased]: https://github.com/AJenbo/phpantom_lsp/compare/0.7.0...HEAD
[0.7.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.6.0...0.7.0
[0.6.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/AJenbo/phpantom_lsp/commits/0.1.0
