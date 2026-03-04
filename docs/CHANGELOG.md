# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
- **Constant values in hover.** Hovering a class constant now shows its initializer value inline (e.g. `const STATUS_ACTIVE = 'active';`). Works for all constant types including strings, integers, arrays, and expressions. Typed constants (PHP 8.3+) show both the type and value.
- **PHP version-aware stubs.** PHPantom detects the target PHP version from `composer.json` (`config.platform.php` or `require.php`) and filters built-in stub signatures accordingly. Functions, methods, and parameters annotated with `#[PhpStormStubsElementAvailable]` (including aliased forms used in some stub files) that do not apply to the detected version are excluded. For example, `array_map` on PHP 8.4 shows `array $array` instead of the untyped `$arrays` parameter from PHP 7.4. When no version is detected, PHP 8.5 is assumed.
- **Docblock navigation.** Go-to-definition and hover now work on class names inside callable type annotations (`\Closure(Request): Response`), array and object shape value types (`array{logger: Pen, debug: bool}`), and Ctrl+Click on object shape properties (`$profile->name` from `@return object{name: string}`) jumps to the key inside the docblock.
- **AST-based array type inference.** Array shape key completion and array element member access resolve through an AST walker that handles literal arrays, `new` expressions, call expressions, spread elements, incremental key assignments, and push-style assignments. Scalar values in array literals are inferred with precise types instead of `mixed`.
- **Symbol map coverage expanded.** Anonymous classes, top-level `const` declarations, language constructs (`isset`, `empty`, `print`, etc.), string interpolation expressions, first-class callable syntax (`strlen(...)`, `Foo::bar(...)`), standalone constant references, `declare` bodies, short echo tags, array append expressions, and pipe operator expressions now produce navigable symbol spans.
- **GTD from parameter and property variables.** Clicking a parameter or property variable at its definition site now jumps to the type hint class, matching the behaviour of assignment variables. Catch variables (`catch (Exception $e)`) with single or union type hints are also supported.
- **Signature help enriched.** The popup is now two lines: a compact parameter list with native PHP types and return type, plus a per-parameter `@param` description that includes the effective docblock type when it differs from the native hint. Optional parameters display their default value in the label (e.g. `int $limit = 25`).

### Fixed

- **Named-argument resolution for non-variable subjects.** Named arguments now resolve correctly when the call target is a bare class name, a chain result, or a static method whose class name requires variable/chain resolution.
- **GTD for `@method`/`@property` on interfaces.** Go-to-definition now walks implemented interfaces (own and from parents) before checking `@mixin` classes, so virtual members declared on interfaces resolve correctly.
- **`?->` null-safe chain resolution.** The `->` inside `?->` no longer confuses subject splitting across completion, go-to-definition, and signature help.
- **`(new Canvas())->easel` property access.** Parenthesized `new` expressions on the left side of `->` now resolve correctly for variable type inference.
- **Array function resolution.** `array_pop`, `array_filter`, `array_values`, `end`, and `array_map` now resolve element types correctly when the array comes from a method call chain or property access.
- **Hover on variable definition sites.** Hovering a parameter, foreach binding, or catch variable no longer shows a redundant popup when the type is already visible in the signature. Assignment variables still show their resolved type.

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