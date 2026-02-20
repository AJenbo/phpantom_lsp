# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

## [0.3.0] - 2026-02-21

### Added

- **Go-to-implementation.** Jump from an interface or abstract class (or a method call typed as one) to all concrete implementations. Scans open files, class index, classmap, embedded stubs, and PSR-4 directories in five phases.
- **Method-level `@template` (general case).** When a method declares `@template T` with `@param T $model` and `@return Collection<T>`, the resolver infers `T` from the actual argument at the call site. Works with inline chains, static methods, `new` expressions, and cross-file resolution.
- **`@phpstan-type` / `@psalm-type` aliases.** Local type aliases defined on a class are expanded during resolution, including `@phpstan-import-type` for importing aliases from other classes.
- **Array function type preservation.** `array_filter`, `array_map`, `array_pop`, `current`, and similar functions preserve the element type instead of losing it to `array`.
- **Spread operator type tracking.** `$all = [...$users, ...$admins]` resolves to the union of element types from all spread sources.
- **Callable/closure variable invocation.** `$fn()->` resolves the return type when `$fn` holds a closure, arrow function, or a variable annotated as `Closure(...): T`.
- **Early return narrowing.** Guard clauses (`if (!$x instanceof Foo) return;`) narrow the type for subsequent code. Multiple guards stack. Works in ternaries and `match(true)`.
- **`instanceof` narrowing to interface and abstract types.**
- **`instanceof` narrowing inside ternary expressions.**
- **Trait `insteadof` / `as` conflict resolution.** Visibility changes and method aliasing via `as`, exclusion via `insteadof`.
- **Generics tracked through loop iterations.**
- **Yield type resolution.**
- **Chained method calls in variable assignment.** `$x = $this->foo()->bar()` resolves through the full chain.
- **Named key destructuring from array shapes.** `['name' => $name] = $shape` resolves `$name` to the correct type.
- **Type hint completion in function/method signatures.**
- **Variable and clone assignment type tracking.**
- **Iterate directly on function return values.** `foreach (getUsers() as $user)` resolves `$user`.
- **User constant completion.**
- **Required argument completion.**
- **Contextual try-catch completion.** Exception suggestions are scoped to what the `try` block can actually throw.
- **Void detection for `@return` PHPDoc suggestions.**

### Fixed

- More robust PHPDoc type parsing.
- Fixed false positive type lookups for internal stubs.
- Fixed crash in variable resolver.
- Fixed incorrect method resolution.
- Fixed finding definitions inside comments.
- Fixed use of incorrect import map for name resolution.
- Fixed completion suggestions being too aggressive or appearing in comments.

## [0.2.0] - 2026-02-18

### Added

- **Generics.** Class-level `@template` with `@extends` substitution through inheritance chains. Method-level `class-string<T>` pattern. Generic trait substitution.
- **Array shapes.** `['key' => Type]` literals offer key completion with no annotation needed. Incremental assignments extend the shape.
- **Object shapes.**
- **Array growth tracking.** `$arr[] = new Foo()` and `$arr['key'] = $value` build up the shape incrementally.
- **Array destructuring.** `[$a, $b] = $pair` resolves element types.
- **Array element access.** `$arr[0]->` resolves the element type.
- **Foreach key type resolution.** Keys from generic iterables and array shapes.
- **Iterable value type resolution.** Foreach on `Collection<User>`, `Generator<int, Item>`, and `@implements IteratorAggregate<int, User>`.
- **Ternary and null-coalescing type resolution.**
- **Match expression type inference.**
- **Named argument completion.**
- **Variable name suggestions.**
- **Standalone function completion.**
- **`define()` constant completion.**
- **Smart PHPDoc tag completion.** Tags filtered to context (`@var` only in property docblocks, `@param` only when there are undocumented parameters). `@throws` detects uncaught exceptions. `@param` pre-fills name and type. Already-documented tags are not suggested again.
- **Deprecated member detection.**
- **Promoted property type via `@param`.**
- **Property chaining.**
- **`require_once` function discovery.**
- **Go-to type definition from property.**

### Fixed

- Fixed `@mixin` context for return types.
- Fixed import of global classes and namespace context.
- Fixed go-to-definition for aliased classes.

## [0.1.0] - 2026-02-16

Initial release.

### Added

- **Completion.** Methods, properties, and constants via `->`, `?->`, and `::`. Context-aware visibility filtering. Alphabetically ordered results.
- **Class inheritance.** Parent classes, interfaces, and traits with correct member merging.
- **`self::`, `static::`, `parent::` resolution.**
- **PHPDoc support.** `@return` type resolution, `@property` virtual properties, `@method` virtual methods, `@mixin` class merging.
- **Conditional return types.** `@return ($param is class-string<T> ? T : mixed)` and similar PHPStan-style conditional types.
- **Inline `@var` annotations.** `/** @var User $user */` resolves the variable type.
- **Enum support.** Case completion, implicit `UnitEnum`/`BackedEnum` interface members.
- **Type narrowing.** `instanceof`, `is_a()`, and `@phpstan-assert` annotations.
- **Nullsafe operator.** `?->` completion.
- **Class name completion with auto-import.** Suggests class names and inserts the `use` statement.
- **Union type inference.**
- **Go-to-definition.** Classes, interfaces, traits, enums, methods, properties, constants, standalone functions, `new` expressions, and variable assignments.
- **PSR-4 lazy loading** via Composer.
- **Composer classmap support.**
- **Embedded phpstorm-stubs.** Standard library type information bundled in the binary.
- **Namespace aliasing and prefix imports.**
- **Zed editor extension.**

[Unreleased]: https://github.com/AJenbo/phpantom_lsp/compare/0.3.0...HEAD
[0.3.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/AJenbo/phpantom_lsp/commits/0.1.0