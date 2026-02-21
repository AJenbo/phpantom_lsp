# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Multi-line method chain completion.** Fluent chains spanning multiple lines now produce completions and support go-to-definition. Continuation lines starting with `->` or `?->` are joined with preceding lines before subject extraction, so builder patterns, query chains, and collection pipelines work seamlessly.
- **Template parameter bound resolution.** When a property or variable type is a `@template` parameter (e.g. `TNode`), the resolver falls back to the upper bound declared via `of` (e.g. `@template TNode of SomeClass`) for completion and go-to-definition.
- **Transitive interface inheritance in go-to-implementation.** If `InterfaceB extends InterfaceA` and `ClassC implements InterfaceB`, go-to-implementation on `InterfaceA` now finds `ClassC`. Works through arbitrary depth and with interfaces that extend multiple parents.
- **Switch statement variable type tracking.** Variables assigned inside `switch` case bodies now resolve their types. Both brace-delimited and colon-delimited (`switch(): … endswitch;`) forms are supported, and all cases contribute to a union type.
- **`unset()` variable tracking.** After `unset($var)`, the variable no longer appears in name suggestions and `$var->` does not resolve to its previous type. Re-assignment after `unset` restores the variable with the new type. Conditional `unset` (inside `if` blocks) is handled conservatively, keeping the variable because it might still exist.

### Fixed

- **Go-to-definition for static properties and typed constants via `::`.** `ClassName::$staticProp` now jumps to the property declaration. Previously the `$` prefix caused it to be misidentified as a local variable, bypassing member access resolution. Also fixed `find_member_position` failing on PHP 8.3 typed constants (`const string NAME = ...`) where a type between `const` and the name broke the pattern match. Works same-file, cross-file, and with `self::`/`static::`.
- **`static` return type resolved to concrete class at call sites.** When a method declares `@return static` and is called on a subclass variable, the resolver now returns the caller's concrete class rather than the declaring (parent) class. Chained fluent calls preserve the subclass through multiple `static` returns.
- **Namespaced FQN return types no longer break chain resolution.** `clean_type` now preserves the leading `\` on fully-qualified names so that `resolve_type_string` does not incorrectly prepend the current file's namespace. Cross-file FQN return types (e.g. `@return \Illuminate\Database\Eloquent\Builder`) resolve correctly regardless of the caller's namespace.
- **Parenthesized RHS expressions now resolved.** Assignments like `$var = (new Foo())` and `$var = ($cond ? $a : $b)` now resolve correctly through the AST path. Previously the `Expression::Parenthesized` wrapper was not unwrapped in `resolve_rhs_expression`.
- **`$var::` completion for class-string variables.** When a variable holds a class-string (e.g. `$cls = User::class`), using `$cls::` now offers the referenced class's static members, constants, and static properties. Handles `self::class`, `static::class`, `parent::class`, and unions from match/ternary/null-coalescing expressions.
- **`?->` chaining fallback now recurses correctly.** The `?->` fallback branch in subject extraction called `extract_simple_variable` instead of `extract_arrow_subject`. The primary `->` branch already handled `?->` chains correctly via a `?` skip, so this was not user-visible, but the fallback is now consistent.
- **Multi-extends interfaces now fully stored.** Interfaces extending multiple parents (e.g. `interface C extends A, B`) now store all parent names, not just the first one.

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