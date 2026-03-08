# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Self-generated classmap.** PHPantom now works without running `composer dump-autoload -o`. When the Composer classmap is missing or incomplete, PHPantom scans the project's autoload directories itself to build a class index. Non-Composer projects are also supported by scanning all PHP files in the workspace. The strategy is configurable via `[indexing] strategy` in `.phpantom.toml` (`"composer"`, `"self"`, `"full"`, or `"none"`).
- **Pipe operator (PHP 8.5).** `$input |> trim(...) |> createDate(...)` resolves through the chain, returning the last callable's return type. Completion works after assigning the pipe result to a variable.
- **Pass-by-reference parameter type inference.** After calling a function that accepts a typed `&$var` parameter (e.g. `function foo(Baz &$bar)`), the variable acquires the parameter's type for subsequent completion.
- **Generic `@phpstan-assert` with `class-string<T>`.** `@phpstan-assert T $value` combined with a `@template T` bound via `class-string<T>` now resolves the narrowed type from the call-site argument. For example, `Assert::assertInstanceOf(Foo::class, $obj)` narrows `$obj` to `Foo`.
- **Interface template inheritance.** When a class implements an interface whose methods use `@template`, `@param class-string<T>`, or `@return T`, the implementing class's overridden methods now inherit the template parameters, bindings, conditional return types, and type assertions. Previously these were lost during interface merging.
- **Invoked closure and arrow function return types.** `(fn(): Foo => new Foo())()` and `(function(): Bar { return new Bar(); })()` now resolve to the return type of the closure or arrow function. Previously these patterns produced no completions.
- **`new $classStringVar` and `$classStringVar::method()`.** When a variable holds a class-string value (e.g. `$f = Foo::class`), `new $f` resolves to `Foo` and `$f::staticMethod()` resolves through `Foo`'s static members.
- **`iterator_to_array()` element type.** `iterator_to_array($iter)` now resolves the element type from the iterator's generic annotation, so `$items[0]->` offers the correct completions.
- **Compound negated guard clause narrowing.** After `if (!$x instanceof A && !$x instanceof B) { return; }`, the surviving code narrows `$x` to `A|B`. Previously only single negated instanceof guard clauses were recognized.
- **Enum case properties.** Completing on an enum case variable (`$case->`) now shows `name` (on all enums) and `value` (on backed enums) inherited from the `UnitEnum` and `BackedEnum` interfaces.
- **`@implements` generic resolution.** When a class declares `@implements SomeInterface<ConcreteType>`, template parameters on the interface's methods and properties are now substituted with the concrete types. Works with `@template-implements` and `@phpstan-implements` aliases, multiple `@implements` annotations on the same class, parameter type substitution (not just return types), and chained resolution through parent classes (e.g. `Test2 extends Test1<int>` where `Test1` has `@implements Iterator<TKey, string>`). Foreach iteration over classes implementing generic iterable interfaces (including through intermediate interface chains like `TypedCollection extends IteratorAggregate`) now resolves value and key types correctly.
- **`@phpstan-assert` on static methods.** Type guard annotations (`@phpstan-assert`, `@phpstan-assert-if-true`, `@phpstan-assert-if-false`) now work on static method calls like `Assert::instanceOf($value, Foo::class)`. Previously only standalone function calls triggered assertion-based narrowing.
- **Property-level narrowing.** `if ($this->prop instanceof Foo)` now narrows the type of `$this->prop` inside the then-body, else-body, and after guard clauses. `assert($this->prop instanceof Foo)` also works. Previously only plain variables participated in instanceof narrowing.
- **Attribute constructor signature help.** Signature help and named parameter completion now fire inside PHP 8 attribute parentheses (`#[Route('/path', <>)]`), showing the attribute class's constructor parameters.
- **Closure variable scope isolation.** Variables declared outside a closure are no longer offered as completions inside the closure body unless captured via `use()`. Previously outer variables leaked into closure scope.

### Fixed

- **Elseif chain narrowing.** In `if/elseif/else` chains, the else branch now strips types from all preceding conditions. For example, `if (is_string($x)) {} elseif (is_int($x)) {} else { $x-> }` correctly narrows `$x` by excluding both `string` and `int`.
- **Variadic `@param` template bindings.** `@param class-string<T> ...$items` now correctly binds the template parameter. Previously the `...` prefix prevented the parameter name from being recognized, so generic return types were not substituted.
- **Laravel relationship classification.** Relationship return types that are fully-qualified to a non-Eloquent namespace (e.g. a custom `App\Relations\HasMany`) are no longer misclassified as Eloquent relationships. Only types under `Illuminate\Database\Eloquent\Relations\` or unqualified short names are recognized.
- **Memory usage after go-to-implementation and find references.** Files transiently loaded during implementation scanning and workspace indexing are now evicted from the AST cache when they are not open in the editor. Previously these entries accumulated and increased memory usage over time.
- **`@phpstan-type` aliases in foreach.** Type aliases defined via `@phpstan-type` or `@psalm-type` now resolve correctly when the aliased type is iterated in a `foreach` loop, destructured with `list()`/`[]`, or used as a foreach key type.
- **Mixed `->` then `::` accessor chains.** Expressions like `$obj->prop::$staticProp` and `$obj->method()::staticMethod()` now resolve through the full chain instead of losing the instance prefix.
- **Inline `(new Foo)->method()` chaining.** Parenthesized `new` expressions used as the root of a method chain now resolve for completion. Previously only assigned `new` expressions (`$x = new Foo()`) worked.
- **Literal string conditional return types.** Conditional return types that check against a literal string value (`$param is "foo" ? A : B`) now resolve the correct branch when the call-site argument is a matching string literal.
- **Inherited `@method` and `@property` tags.** Virtual members declared via `@method` or `@property` on a parent class now appear on child classes. Previously only the declaring class itself surfaced these members.
- **Class constant and enum case assignment resolution.** Assigning from a class constant (`$x = Foo::SOME_CONST`) or enum case (`$x = Status::Active`) now resolves the variable's type correctly for subsequent completion.
- **Sequential assert narrowing.** Multiple `assert($x instanceof A); assert($x instanceof B);` statements now accumulate, showing members from both types. Previously only the last assertion's narrowing applied.
- **First-open performance.** Diagnostics on `did_open` now run asynchronously (matching `did_change` behaviour) instead of blocking the LSP response. Previously, opening a file with many class references triggered a cascade of lazy stub and vendor file parses that froze the editor for up to 20 seconds.
- **`instanceof` narrowing no longer widens specific types.** `assert($zoo instanceof ZooBase)` after `$zoo = new Zoo()` (where `Zoo extends ZooBase`) no longer replaces the type with the less-specific parent. The narrower `Zoo` type is preserved because it already satisfies the check. Previously this produced false-positive "unknown member" warnings for `@property` and `@method` members declared on the child class.
- **Arrow function parameter completion with incomplete expressions.** Typing `$foo->` inside an arrow function body (e.g. `fn(Foo $foo) => $foo->`) now resolves the parameter type even when the expression is incomplete. The parser recovery inserts a dummy token so the surrounding arrow function structure is recognized.
- **False-positive unknown-class warnings on PHPStan type syntax.** String literals in conditional return types (`$param is "foo" ? A : B`), numeric literals, and variance annotations on generic arguments (`Collection<int, covariant array{...}>`) no longer trigger "Class not found" warnings.


- **Project configuration.** PHPantom now reads a `.phpantom.toml` file from the project root (next to `composer.json`) for per-project settings. Currently supports `[php] version` to override the detected PHP version, and `[diagnostics] unresolved-member-access` to enable the new unresolved member access diagnostic. Run `phpantom --init` to generate a default config file with all options commented out. When the file is missing, all settings use their defaults. Parse errors are reported as a warning in the editor.
- **Unresolved member access diagnostic (opt-in).** A new hint-level diagnostic flags `->`, `?->`, and `::` accesses where PHPantom cannot resolve the subject type at all. Off by default because most codebases lack full type coverage. Enable it by adding `unresolved-member-access = true` under `[diagnostics]` in `.phpantom.toml`. Useful for discovering gaps in type annotations or places where PHPantom's inference falls short.
- **Rename.** `textDocument/rename` with `prepareRename` support. Rename variables, classes, methods, properties, functions, and constants across the entire workspace. Variable renames are scoped to the enclosing function or closure. Property renames handle the `$` prefix correctly at declaration vs. access sites. Symbols defined in vendor files are rejected. Non-renameable tokens (`$this`, `self`, `static`, `parent`) are rejected at the prepare step so the editor never opens the rename prompt.
- **Implement missing methods.** A new code action generates method stubs when a concrete class extends an abstract class or implements an interface with unimplemented methods. Place the cursor inside the class body and trigger "Quick Fix" to insert stubs with correct visibility, static modifier, parameter types with defaults, and return type. Works across files with PSR-4 resolution, handles deep inheritance chains, and respects the file's existing indentation style.
- **Reverse go-to-implementation.** Invoking go-to-implementation on a method definition in a concrete class jumps to the interface or abstract class that declares the prototype. Also works from interface and abstract class method declarations in the forward direction, finding all concrete implementations.
- **Find References.** "Find All References" locates every usage of a symbol across the project. Supports classes, interfaces, traits, enums, methods, properties, constants, functions, and variables. Variable references are scoped to the enclosing function or closure. Cross-file scanning lazily indexes user files on demand (vendor and stub files are excluded, matching PhpStorm's behaviour). The workspace walk respects `.gitignore` rules, so generated/cached directories (blade cache, Symfony `var/cache/`, `node_modules/`, etc.) are automatically skipped.
- **`#[Deprecated]` attribute support.** PHPantom now reads the `#[Deprecated]` attribute used by phpstorm-stubs (~362 elements) in addition to docblock `@deprecated` tags. The `reason` and `since` fields appear in hover, completion strikethrough, and deprecation diagnostics. When both a docblock tag and an attribute are present, the docblock message takes priority. Works on classes, interfaces, traits, enums, methods, properties, constants, and standalone functions.
- **Deprecation diagnostics for variable member access.** Calling a deprecated method or accessing a deprecated property through a variable (e.g. `$svc->oldMethod()`) now produces a strikethrough diagnostic. Previously only `self::`, `static::`, `$this->`, and explicit class name accesses were checked.
- **Unknown class diagnostics.** Class references that PHPantom cannot resolve through any phase (use-map, local classes, same-namespace, class_index, classmap, PSR-4, stubs) are underlined with a warning. The diagnostic pairs with the "Import Class" code action so pressing the quick-fix shortcut on the warning offers to add the missing `use` statement in one step. Template parameters, type aliases (`@phpstan-type`, `@phpstan-import-type`), and attribute classes are excluded to avoid false positives.
- **Unknown member diagnostics.** Method calls, property accesses, and constant references that do not exist on the resolved class are underlined with a warning after full resolution (inheritance, traits, virtual members from `@method`/`@property`/`@mixin`). Suppressed when the class declares `__call`, `__callStatic`, or `__get` magic methods, when the subject type cannot be resolved, and when any branch of the resolved type is `stdClass` (a universal object container that accepts arbitrary properties). PHP's case-insensitive method matching is respected.
- **Document highlighting.** Placing the cursor on a symbol highlights all other occurrences in the current file. Variables are scoped to their enclosing function or closure. Assignment targets, parameters, foreach bindings, and catch variables are marked as writes; all other occurrences are reads. Class names, members, functions, and constants highlight file-wide.

### Changed

- **Faster class resolution.** Fully-resolved classes (inheritance + virtual members) are now cached and reused across completion, hover, and go-to-definition within each request cycle. The cache is automatically cleared whenever a file changes, so results are never stale.
- **Resolution engine rewritten on AST.** Variable type inference, subject dispatch, call return type resolution, member-access detection, and go-to-definition lookups all run through the AST walker now. The text-based scanner and its line-by-line fallbacks have been removed entirely. This fixes a class of edge-case bugs with null-safe chains, parenthesized `new` expressions, chained method calls, and complex array access patterns.
- **Go-to-definition and go-to-implementation use only the symbol map.** The text-based fallbacks (`extract_word_at_position`, `extract_member_access_context`, `resolve_type_hint_at_variable_text`) have been removed from both features. Cursor context detection now relies exclusively on the precomputed symbol map.
- **Go-to-definition uses byte offsets exclusively.** All definition lookups use AST-derived byte offsets instead of text search, including for built-in stubs and `define()` constants.
- **Hover redesigned.** Hover popups use short names with a `namespace` line, show actual default values instead of `= ...`, display `@link` URLs, and highlight the precise hovered token. `new ClassName` shows the constructor signature. `@template` parameters show their declaration, variance, and bound. Hovering a generic class or interface lists its template parameters (e.g. `**template-covariant** \`TValue\` of \`object\``). Fully-qualified names in docblocks resolve correctly in namespaced files.

### Added

- **Hover origin indicators.** Hovering a method, property, or constant now shows whether it overrides a parent class member, implements an interface contract, or is a virtual (synthesized) member. Multiple origins combine when applicable (e.g. "↑ overrides **BaseView** · ◆ implements **Renderable**").
- **Enum case listing in hover.** Hovering an enum name shows all cases inside the code block, with values for backed enums. Regular class constants on enums are excluded.
- **Trait method signatures in hover.** Hovering a trait name shows its public methods, properties, and constants as a scannable summary inside the code block.
- **Deprecation messages in hover.** Hovering a deprecated method, property, constant, function, or class now shows the explanation text from the `@deprecated` tag (e.g. `🪦 **deprecated** Use collect() instead`) instead of a bare label.
- **Constant values in hover.** Hovering a class constant now shows its initializer value inline (e.g. `const STATUS_ACTIVE = 'active';`). Works for all constant types including strings, integers, arrays, and expressions. Typed constants (PHP 8.3+) show both the type and value. Global constants defined via `define()` or top-level `const` statements also show their value.
- **PHP version-aware stubs.** PHPantom detects the target PHP version from `composer.json` (`config.platform.php` or `require.php`) and filters built-in stub signatures accordingly. Functions, methods, and parameters annotated with `#[PhpStormStubsElementAvailable]` (including aliased forms used in some stub files) that do not apply to the detected version are excluded. For example, `array_map` on PHP 8.4 shows `array $array` instead of the untyped `$arrays` parameter from PHP 7.4. When no version is detected, PHP 8.5 is assumed.
- **Docblock navigation.** Go-to-definition and hover now work on class names inside callable type annotations (`\Closure(Request): Response`), array and object shape value types (`array{logger: Pen, debug: bool}`), and Ctrl+Click on object shape properties (`$profile->name` from `@return object{name: string}`) jumps to the key inside the docblock.
- **AST-based array type inference.** Array shape key completion and array element member access resolve through an AST walker that handles literal arrays, `new` expressions, call expressions, spread elements, incremental key assignments, and push-style assignments. Scalar values in array literals are inferred with precise types instead of `mixed`.
- **Symbol map coverage expanded.** Anonymous classes, top-level `const` declarations, language constructs (`isset`, `empty`, `print`, etc.), string interpolation expressions, first-class callable syntax (`strlen(...)`, `Foo::bar(...)`), standalone constant references, `declare` bodies, short echo tags, array append expressions, and pipe operator expressions now produce navigable symbol spans.
- **GTD from parameter and property variables.** Clicking a parameter or property variable at its definition site now jumps to the type hint class, matching the behaviour of assignment variables. Catch variables (`catch (Exception $e)`) with single or union type hints are also supported.
- **Signature help enriched.** The popup is now two lines: a compact parameter list with native PHP types and return type, plus a per-parameter `@param` description that includes the effective docblock type when it differs from the native hint. Optional parameters display their default value in the label (e.g. `int $limit = 25`).

### Changed

- **Async diagnostics.** Diagnostics now run in a background task with 500 ms debounce instead of blocking every `did_change` response. Completion, hover, and signature help remain responsive while diagnostics compute in the background.
- **Smarter cache invalidation.** Editing inside a method body no longer evicts any resolved-class cache entries. PHPantom compares the old and new class signatures (method names, types, visibility, inheritance, docblock tags) and only evicts when something resolution-relevant actually changed. Combined with per-FQN targeted invalidation and generic-aware cache keys, this means typing inside a method body keeps the entire cache warm, while changing a type hint or adding a method evicts only the affected class.

### Fixed

- **`__invoke()` return type resolution.** Calling `$f()` where `$f` holds an object with an `__invoke()` method now resolves the return type correctly. Completion, chaining (`$f()->method()->`), foreach iteration over the return value, and docblock return types on `__invoke()` all work. Parenthesized expression invocations like `($this->factory)()` are also supported.
- **Enum `from()` and `tryFrom()` chaining.** `MyEnum::from('value')->method()` now resolves through the enum type, so completion offers the enum's own methods and interface methods after the call.
- **Nested closures with reused parameter names no longer crash.** Hovering or requesting diagnostics on code with nested closures that reuse the same variable name (e.g. `$q` in both an outer and inner closure callback) no longer causes a stack overflow. The callable parameter inference now caps its recursion depth to break the cycle.
- **Scope methods on Builder variables.** Hover, signature help, and deprecation diagnostics now find model-specific members (e.g. Eloquent scope methods injected onto `Builder<Model>`) even when the resolved-class cache holds a differently-scoped entry for the same base class.
- **Go-to-implementation no longer produces false positives across namespaces.** Two interfaces with the same short name in different namespaces (e.g. `App\Logger` and `Vendor\Logger`) no longer cross-match. Implementor scanning and deduplication now use fully-qualified names when available.
- **Vendor class resolution simplified.** Vendor PSR-4 mappings (`vendor/composer/autoload_psr4.php`) are no longer loaded. The Composer classmap is the sole source of truth for vendor code. Go-to-definition now checks the classmap for vendor classes instead of relying on vendor PSR-4. If the classmap is missing or stale, vendor classes fail to resolve visibly instead of being silently papered over (fix: run `composer dump-autoload`). The `config.vendor-dir` setting is read once at startup and cached across all features.
- **Named-argument resolution for non-variable subjects.** Named arguments now resolve correctly when the call target is a bare class name, a chain result, or a static method whose class name requires variable/chain resolution.
- **GTD for `@method`/`@property` on interfaces.** Go-to-definition now walks implemented interfaces (own and from parents) before checking `@mixin` classes, so virtual members declared on interfaces resolve correctly.
- **`?->` null-safe chain resolution.** The `->` inside `?->` no longer confuses subject splitting across completion, go-to-definition, and signature help.
- **`(new Canvas())->easel` property access.** Parenthesized `new` expressions on the left side of `->` now resolve correctly for variable type inference.
- **Array function resolution.** `array_pop`, `array_filter`, `array_values`, `end`, and `array_map` now resolve element types correctly when the array comes from a method call chain or property access.
- **Hover on variable definition sites.** Hovering a parameter, foreach binding, or catch variable no longer shows a redundant popup when the type is already visible in the signature. Assignment variables still show their resolved type.
- **Inline `@var` annotations no longer leak across scopes.** A `/** @var Type $var */` annotation in one method no longer affects hover or completion for a same-named variable in a different method or class.
- **Docblock tag parsing in description text.** Tags like `@throws` appearing mid-sentence in docblock descriptions (e.g. `"filtered out of @throws suggestions."`) are no longer mistakenly parsed as tags. Only `@` at a valid tag position (after the `* ` line prefix) is recognized.
- **Double-negated `instanceof` narrowing.** `if (!!$x instanceof Foo)` now correctly narrows `$x` to `Foo` inside the block. Previously the double negation was not simplified and the type stayed unresolved.
- **Accessor on new line with whitespace.** Completion now works when `->` is on a new line with extra whitespace before the cursor (e.g. `$obj\n    ->   `). Previously the subject extraction failed to find the operator across the whitespace gap.
- **Partial static property completion.** Typing `$obj::$f` now returns static property completions. Previously the `$` sigil on the partially typed property name prevented the `::` operator from being detected.

## [0.4.0] - 2026-03-01

### Added

- **Signature help.** Parameter hints appear when typing inside function/method call parentheses. The active parameter highlights and updates as you type commas. Works for all call forms including constructors, static/instance methods, and cross-file resolution.
- **Hover.** Hovering over a symbol shows its type, signature, and docblock in a Markdown popup. Supports variables, methods, properties, constants, classes, functions, and keywords like `$this`/`self`/`static`/`parent`.
- **Closure and callable inference.** Untyped closure parameters are inferred from the callable signature of the receiving function (e.g. `$users->map(fn($u) => $u->name)` infers `$u` as `User`). First-class callable syntax (`strlen(...)`, `$obj->method(...)`) resolves as `Closure` with the underlying return type.
- **Laravel Eloquent support.** Relationship properties with correct collection/model types for all 10 relationship types. Scope methods on Model and Builder. Builder-as-static forwarding (`User::where(...)->get()`). Factory support. Custom Eloquent collections via `#[CollectedBy]` or `newCollection()`. Cast properties, accessor/mutator properties, `$attributes` defaults, and `$visible` array extraction.
- **Type narrowing improvements.** `in_array($var, $haystack, true)` narrows to the haystack element type. Early return guards stack and narrow subsequent code. `instanceof` works in ternaries and with interfaces/abstract types.
- **Anonymous class support.** `$this->` inside anonymous classes resolves to the anonymous class's own members, with full support for `extends`, `implements`, traits, and promoted properties.
- **Context-aware completions.** `extends` offers non-final classes, `implements` offers interfaces, `use` inside a class offers traits. Union members sort shared-across-all-types items first. Namespace segment completion for `App\...` chains. Completion suppressed inside string literals but allowed in interpolation.
- **Additional resolution.** Multi-line method chains. Nested key completion for literal arrays at arbitrary depth. Generator yield type inference. Conditional return types with template substitution through Builder forwarding. Template parameter bound fallback via `of`. Class-string variable forwarding to conditional return types. Switch statement and `unset()` variable tracking. Alphabetical `use` statement insertion.
- **Transitive interface go-to-implementation.** Go-to-implementation on an interface finds all concrete implementations through arbitrary inheritance depth.

### Fixed

- **Visibility filtering.** Protected members only appear in completions from the same class or subclasses.
- **Scope isolation.** `@param`/`@var` annotations no longer leak across sibling methods. Same-named parameters in different methods resolve to the correct type.
- **Static call chains.** `User::where('active', 1)->first()->profile->` resolves through the entire chain.
- **`static` return type.** `@return static` on a parent method returns the caller's subclass type.
- **Trait resolution.** `$this`/`static`/`self` return types on trait methods resolve to the using class. Variable resolution inside trait method bodies works correctly.
- **Mixin fluent chains.** `$this`/`self`/`static` return types from `@mixin` classes stay as-is instead of being rewritten to the mixin class name.
- **Go-to-definition accuracy.** Inherited members with same short name in different namespaces, foreach variables, RHS variables in `$value = $value->value`, static properties, typed constants, and multi-namespace files all resolve correctly.
- **Import handling.** Namespaced functions insert correct `use function` imports. Conflicting short names fall back to fully-qualified `\App\Exception` instead of duplicate `use` statements.
- **Edge cases.** UTF-8 boundary panic fixed. Parenthesized RHS expressions (`$var = (new Foo())`) resolve correctly. Multi-extends interfaces store all parent names. Interface `@method`/`@property` tags appear on implementing classes.

## [0.3.0] - 2026-02-21

### Added

- **Go-to-implementation.** Jump from an interface or abstract class to all concrete implementations. Scans open files, class index, classmap, stubs, and PSR-4 directories.
- **Method-level `@template`.** `@template T` with `@param T $model` and `@return Collection<T>` infers `T` from the actual argument at the call site. Works with inline chains, static methods, `new` expressions, and cross-file resolution.
- **`@phpstan-type` / `@psalm-type` aliases.** Local type aliases and `@phpstan-import-type` for importing aliases from other classes.
- **Array function type preservation.** `array_filter`, `array_map`, `array_pop`, `current`, and similar functions preserve the element type.
- **Early return narrowing.** Guard clauses (`if (!$x instanceof Foo) return;`) narrow the type for subsequent code. Multiple guards stack. Works in ternaries and `match(true)`.
- **Callable variable invocation.** `$fn()->` resolves the return type when `$fn` holds a closure, arrow function, or `Closure(...): T`.
- **Additional resolution.** Spread operator type tracking. Trait `insteadof`/`as` conflict resolution. Chained method calls in variable assignment. Named key destructuring from array shapes. Foreach on function return values. Type hint completion. Contextual try-catch exception suggestions.

### Fixed

- More robust PHPDoc type parsing and internal stability fixes.

## [0.2.0] - 2026-02-18

### Added

- **Generics.** Class-level `@template` with `@extends` substitution through inheritance chains. Method-level `class-string<T>` pattern. Generic trait substitution.
- **Array shapes and object shapes.** Key completion from `['key' => Type]` literals with no annotation needed. Incremental `$arr[] = new Foo()` and `$arr['key'] = $value` assignments build up the shape. Array destructuring and element access resolve types.
- **Foreach type resolution.** Key and value types from generic iterables, array shapes, `Collection<User>`, `Generator<int, Item>`, and `@implements IteratorAggregate`.
- **Expression type inference.** Ternary, null-coalescing, and match expressions resolve to the correct union types.
- **Additional completions.** Named arguments, variable name suggestions, standalone functions, `define()` constants, smart PHPDoc tag completion filtered to context, deprecated member detection, promoted property type via `@param`, property chaining, `require_once` function discovery, go-to type definition from property.

### Fixed

- Fixed `@mixin` context for return types, global class imports, namespace resolution, and aliased class go-to-definition.

## [0.1.0] - 2026-02-16

Initial release.

### Added

- **Completion.** Methods, properties, and constants via `->`, `?->`, and `::`. Context-aware visibility filtering.
- **Type resolution.** Class inheritance with parent/interface/trait merging. `self::`, `static::`, `parent::` resolution. Union type inference. Nullsafe `?->` chains.
- **PHPDoc support.** `@return`, `@property`, `@method`, `@mixin`, conditional return types, and inline `@var` annotations.
- **Type narrowing.** `instanceof`, `is_a()`, and `@phpstan-assert` annotations.
- **Enum support.** Case completion and implicit `UnitEnum`/`BackedEnum` interface members.
- **Go-to-definition.** Classes, interfaces, traits, enums, methods, properties, constants, functions, `new` expressions, and variable assignments.
- **Class name completion with auto-import.**
- **PSR-4 lazy loading and Composer classmap support.**
- **Embedded phpstorm-stubs** for standard library type information.
- **Zed editor extension.**

[Unreleased]: https://github.com/AJenbo/phpantom_lsp/compare/0.4.0...HEAD
[0.4.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/AJenbo/phpantom_lsp/commits/0.1.0
