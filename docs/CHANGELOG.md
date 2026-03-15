# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Inlay hints.** Parameter name and by-reference indicators appear at call sites (`textDocument/inlayHint`). Hints are suppressed when the argument already makes the parameter obvious: variable names matching the parameter, property accesses with a matching trailing identifier, string literals whose content matches, and well-known single-parameter functions like `count` and `strlen`. Named arguments never receive a redundant hint.
- **PHPStan diagnostics.** PHPStan errors appear inline as you edit, using PHPStan's editor mode (`--tmp-file` / `--instead-of`). Auto-detects `vendor/bin/phpstan` or `$PATH`. Runs in a dedicated background worker with a 2-second debounce and at most one process at a time, so native diagnostics are never blocked. Configurable via `[phpstan]` in `.phpantom.toml` (`command`, `memory-limit`, `timeout`).
- **PHPStan ignore code actions.** "Ignore PHPStan error" adds `// @phpstan-ignore <identifier>` to the line (appending to existing ignores when present). "Remove unnecessary @phpstan-ignore" cleans up unmatched ignores that PHPStan flags, removing the specific identifier from multi-identifier comments or deleting the entire comment when it is the only one.
- **Formatting.** Built-in PHP formatting via mago-formatter (PER-CS 2.0 style). Formatting works out of the box without any external tools. Projects that depend on php-cs-fixer or PHP_CodeSniffer in their `composer.json` `require-dev` automatically use those tools instead (both can run in sequence). Per-tool command overrides and disable switches in `[formatting]` in `.phpantom.toml`.
- **Progress indicators.** Go to Implementation and Find References now show a progress indicator in the editor while scanning.
- **Selection Ranges.** Smart select / expand selection returns AST-aware nested ranges from innermost to outermost (identifier, expression, statement, block, function, class, file).
- **Argument count diagnostic.** Flags function and method calls that pass too few or too many arguments. Variadic parameters and argument unpacking are handled correctly.
- **Document Symbols.** The outline sidebar and breadcrumbs now show classes, interfaces, traits, enums, methods, properties, constants, and standalone functions with correct nesting, icons, visibility detail, and deprecation tags.
- **Workspace Symbols.** "Go to Symbol in Workspace" (Ctrl+T / Cmd+T) searches classes, interfaces, traits, enums, functions, constants, methods, properties, and class constants across all indexed files. Vendor classes from the Composer classmap and discovered classes from the class index are included when a query is provided. Results include namespace context, deprecation markers, and are sorted by relevance (exact match, then prefix, then substring) with alphabetical tiebreaking.
- **Folding Ranges.** AST-aware code folding for class bodies, method/function bodies, closures, arrays, argument/parameter lists, if/else/switch/match/try/catch/finally blocks, doc comments, and consecutive single-line comment groups.
- **Code Lens.** Clickable annotations above methods that override a parent class method or implement an interface method. Clicking navigates to the prototype declaration. Parent/trait overrides show "↑ ClassName::method", interface implementations show "◆ InterfaceName::method".
- **Semantic Tokens.** Type-aware syntax highlighting that goes beyond what a TextMate grammar can achieve. Classes, interfaces, enums, traits, methods, properties, parameters, variables, functions, constants, and template parameters all get distinct token types. Modifiers convey declaration sites, static access, readonly, deprecated, and abstract status.
- **Type Hierarchy.** "Show Type Hierarchy" on any class, interface, trait, or enum reveals its supertypes (parent class, implemented interfaces) and subtypes (extending/implementing classes). Supports full up-and-down navigation through the inheritance tree, including cross-file PSR-4 resolution and transitive relationships.
- **Document Links.** `require`/`include` paths are now Ctrl+Clickable. Path resolution supports string literals, `__DIR__` concatenation, `dirname(__DIR__)`, `dirname(__FILE__)`, and nested `dirname` with levels. Only emits links for files that exist on disk.
- **`@see` navigation.** `@see` references in docblocks now work with go-to-definition. Supported forms: `@see ClassName`, `@see ClassName::method()`, `@see ClassName::$property`, `@see ClassName::CONSTANT`, `@see functionName()`, and inline `{@see ...}`.
- **`@link`/`@see` URLs in hover.** Hover popups now show all `@link` and `@see` URLs from the docblock as clickable links (previously only the first `@link` was shown).

### Changed

- **Memory efficiency.** Class metadata is now shared via reference counting (`Arc<ClassInfo>`) from storage through the class-loader boundary to all consumers, reducing deep copies during completion, hover, and diagnostics on large codebases.
- **Generic type substitution.** `apply_substitution` now returns early when no template parameters appear in the type string, avoiding allocation and recursion in the common case.
- **Cross-file resolution.** Fully-qualified class names are now stored in a single canonical form throughout the system, eliminating a class of bugs where name comparisons failed because one side had a leading backslash and the other did not. This improves reliability of completion, hover, go-to-definition, and cache invalidation across files.
- **tower-lsp 0.20.** Upgraded the LSP framework from 0.18 to 0.20, bringing native trait support for inlay hints, type hierarchy, and pull diagnostics. Inlay hints are now served through the standard protocol method instead of a custom handler.
- **Logging.** Replaced `log`/`env_logger` with `tracing`/`tracing-subscriber`, aligning with the logging framework tower-lsp already uses internally.
- **Dependencies.** Bumped minimum versions for `clap` (4.5) and `tokio` (1.39). Ran `cargo update` to pull latest compatible patch releases across the lockfile.

### Fixed

- **Diagnostics.** By-reference `@param` annotations (e.g. `@param array<int> &$data`) no longer produce a false "unknown class '$data'" diagnostic.

## [0.5.0] - 2026-03-12

### Added

- **Find References.** Locate every usage of a symbol across the project. Supports classes, methods, properties, constants, functions, and variables. Variable references are scoped to the enclosing function or closure. Member references are scoped to the class hierarchy, so unrelated classes sharing a method name are excluded.
- **Rename.** Rename variables, classes, methods, properties, functions, and constants across the workspace. Variable renames are scoped to their enclosing function or closure. Symbols in vendor files are rejected. Non-renameable tokens (`$this`, `self`, `static`, `parent`) are rejected at the prepare step.
- **Document highlighting.** Placing the cursor on a symbol highlights all occurrences in the current file. Variables are scoped to their enclosing function or closure with write vs. read distinction.
- **Implement missing methods.** Code action that generates method stubs when a class is missing required interface or abstract method implementations. Handles deep inheritance chains, cross-file resolution, correct visibility and types, and respects the file's indentation style.
- **Reverse go-to-implementation.** Go-to-implementation on a concrete method jumps to the interface or abstract class that declares the prototype, and vice versa.
- **Go to Type Definition.** Jump from a variable, property, method call, or function call to the class declaration of its resolved type. Union types produce multiple locations.
- **Diagnostics.** Unknown classes, unknown members, and unknown functions are flagged with appropriate severity. Duplicate diagnostics on the same span are suppressed. An opt-in unresolved member access diagnostic is available via `.phpantom.toml`.
- **Deprecation support.** `@deprecated` tags and `#[Deprecated]` attributes surface in hover, completion strikethrough, and diagnostics. A quick-fix code action rewrites deprecated calls when a `replacement` template is available.
- **Project configuration.** `.phpantom.toml` for per-project settings: PHP version override, diagnostic toggles, and indexing strategy. Run `phpantom --init` to generate a default config.
- **Self-generated classmap.** PHPantom works without `composer dump-autoload -o`. Missing or incomplete classmaps are supplemented by scanning autoload directories. Non-Composer projects are supported by scanning all PHP files.
- **Non-Composer function and constant discovery.** Cross-file function completion, go-to-definition, and constant resolution for projects without `composer.json`.
- **Monorepo support.** Discovers subdirectories that are independent Composer projects and processes each through the full pipeline.
- **Indexing progress indicator.** The editor shows a progress bar during workspace initialization, including per-subproject progress in monorepos.
- **PHP version-aware stubs.** Detects the target PHP version from `composer.json` and filters built-in stub signatures accordingly.
- **`@param-closure-this`.** `$this` inside a closure resolves to the type declared by `@param-closure-this` on the receiving parameter.
- **Function-level `@template` with generic return types.** Functions like `collect()` that use `@template` parameters inside generic return types now resolve concrete types from call-site arguments.
- **`@implements` generic resolution.** `@implements Interface<ConcreteType>` substitutes template parameters on the interface's methods and properties. Foreach iteration on generic iterable interfaces resolves value and key types.
- **Interface template inheritance.** Implementing classes inherit `@template` parameters, bindings, conditional return types, and type assertions from their interfaces.
- **Generic `@phpstan-assert` with `class-string<T>`.** Assertion methods like `Assert::instanceOf($value, Foo::class)` resolve the narrowed type from the call-site argument.
- **Property-level narrowing.** `if ($this->prop instanceof Foo)` narrows `$this->prop` in then/else bodies and after guard clauses.
- **Inline `&&` short-circuit narrowing.** The right-hand side of `&&` now sees the narrowed type from the left-hand side.
- **Compound negated guard clause narrowing.** `if (!$x instanceof A && !$x instanceof B) { return; }` narrows `$x` to `A|B` in the surviving code.
- **Invoked closure and arrow function return types.** `(fn(): Foo => ...)()` and `(function(): Bar { ... })()` resolve to their return type.
- **`new $classStringVar` and `$classStringVar::method()`.** Class-string variables resolve for `new` and static member access.
- **`iterator_to_array()` element type.** Resolves the element type from the iterator's generic annotation.
- **Enum case properties.** `$case->name` and `$case->value` resolve on enum case variables.
- **Pass-by-reference parameter type inference.** After calling a function with a typed `&$var` parameter, the variable acquires that type.
- **Pipe operator (PHP 8.5).** `$input |> trim(...) |> createDate(...)` resolves through the chain.
- **Closure variable scope isolation.** Variables outside a closure are no longer offered as completions unless captured via `use()`.
- **AST-based array type inference.** Array shape keys, element access, spread elements, and push-style assignments all resolve through an AST walker.
- **Docblock navigation.** Go-to-definition and hover work on class names inside callable types, array/object shape value types, and object shape properties.
- **GTD from parameter and property variables.** Clicking a parameter or property at its definition site jumps to the type hint class.
- **Inline `@var` on promoted constructor properties.** Overrides the native type hint, matching existing `@param` support.
- **`--version` and `--help` CLI flags.** Contributed by [@calebdw](https://github.com/calebdw) in [#7](https://github.com/AJenbo/phpantom_lsp/pull/7).

### Changed

- **Resolution engine rewritten on AST.** Variable type inference, subject dispatch, call return types, and go-to-definition all run through the AST walker. The text-based scanner has been removed entirely.
- **Hover redesigned.** Short names with `namespace` line, actual default values, `@link` URLs, precise token highlighting, constructor signatures on `new`, `@template` details, enum case listing, trait member listing, origin indicators, and deprecated explanations.
- **Signature help enriched.** Compact parameter list with native types, per-parameter `@param` descriptions, default values, and attribute parenthesis support.
- **Faster resolution and lower memory usage.** O(1) class resolution, per-request caching, hash-set deduplication, reference-counted file content, async diagnostics with 500 ms debounce, and signature-aware cache invalidation.
- **Two-phase diagnostic publishing.** Cheap diagnostics (unused imports, deprecation) publish immediately; expensive diagnostics (unknown classes/members/functions) arrive in a second pass.
- **Concurrent read access.** All read-heavy maps use `parking_lot::RwLock` for parallel request handling.
- **Parallel workspace indexing.** File parsing, PSR-4 scanning, and vendor scanning run across all CPU cores. `.gitignore` rules are respected. `memchr` SIMD acceleration for the byte-level scanner.
- **Merged classmap + self-scan pipeline.** Composer classmaps and self-scanning work together instead of being mutually exclusive. Stale classmaps are supplemented automatically.
- **Automatic stub fetching.** The build script downloads phpstorm-stubs automatically when missing. Composer is no longer needed to build PHPantom. Contributed by [@calebdw](https://github.com/calebdw) in [#16](https://github.com/AJenbo/phpantom_lsp/pull/16).
- **Feature comparison table corrected.** Phactor capabilities updated in the README. Contributed by [@dantleech](https://github.com/dantleech) in [#10](https://github.com/AJenbo/phpantom_lsp/pull/10).

### Fixed

- **Go-to-definition on trait `as` alias and `insteadof` declarations.** Method names, alias names, and trait names inside trait use adaptation blocks now resolve correctly.
- **Parallel file scanner panics no longer crash the server.**
- **Type alias array shape diagnostics no longer fire on object values.**
- **Inline array-element function calls resolve correctly in diagnostics.** `end($obj->items)->method()` no longer produces a false "unknown member" diagnostic.
- **Eloquent Builder scope chain diagnostics no longer flicker.**
- **Diagnostics refresh across open files when a class signature changes.**
- **Unknown member diagnostics on property and method return chains.**
- **Variable types resolve through ternary, elvis, null-coalesce, and match assignments.**
- **Parameter types resolve inside `function_exists` guards.**
- **Virtual property merging picks the most specific type.**
- **Custom cast classes declared as string literals resolve correctly.**
- **`@implements CastsAttributes<T>` takes priority over `get()` return type.**
- **Editing a cast class now updates model property types.**
- **Go-to-definition for variables captured via `use`.**
- **Closure parameter inference inside namespaces and across files.**
- **Signature help no longer fires inside closure/arrow function bodies or function definitions.**
- **Signature help parameter type display with parenthesized callable unions.**
- **`__invoke()` return type resolution.** Works with chaining, foreach, and parenthesized invocations.
- **Enum `from()` and `tryFrom()` chaining.**
- **Nested closures with reused parameter names no longer crash.**
- **Scope methods on Builder variables.**
- **`static`/`self`/`$this` in method return types used as iterable expressions.**
- **`instanceof` narrowing no longer widens specific types.**
- **Closure parameter with bare type hint inherits inferred generics.**
- **Closure parameter with parent type hint narrows to inferred subclass.**
- **Cross-file inheritance from global-scope classes imported via `use`.**
- **Model `@method` tags available on Builder instances.**
- **Arrow function outer-scope variable resolution and parameter completion.**
- **Inherited `@method` and `@property` tags.**
- **Elseif chain narrowing and sequential assert narrowing.**
- **First-open performance.** Diagnostics on `did_open` run asynchronously.
- **Variadic `@param` template bindings.**
- **Laravel relationship classification with non-Eloquent namespaces.**
- **Trait `use` no longer triggers false-positive unused import.**
- **PHPDoc types on constructor-promoted properties now recognised.**
- **PHPDoc type tags no longer skipped by unused-import safety net.**
- **`@phpstan-type` aliases in foreach, `list()`, and key types.**
- **Mixed `->` then `::` accessor chains.**
- **Inline `(new Foo)->method()` chaining.**
- **Literal string conditional return types.**
- **Class constant and enum case assignment resolution.**
- **False-positive unknown-class warnings on PHPStan type syntax.**
- **Go-to-implementation no longer produces false positives across namespaces.**
- **Named-argument resolution for non-variable subjects.**
- **"Remove all unused imports" only offered on `use` import lines.**
- **GTD for `@method`/`@property` on interfaces.**
- **`?->` null-safe chain resolution.**
- **`(new Canvas())->easel` property access resolution.**
- **Array function resolution for `array_pop`, `array_filter`, `array_values`, `end`, `array_map`.**
- **Hover on variable definition sites no longer shows redundant popups.**
- **Inline `@var` annotations no longer leak across scopes.**
- **Docblock tag parsing in description text.**
- **Double-negated `instanceof` narrowing.**
- **Accessor on new line with whitespace.**
- **Partial static property completion.**
- **Hover respects `instanceof`, `assert`, and inline `@var` narrowing.**
- **`instanceof` narrowing with same-named classes in different namespaces.**
- **Self-referential array key assignments no longer crash the LSP.**
- **Cross-file `@property` and `@method` type resolution.**
- **Editing a `@property` docblock now invalidates hover in other files.**
- **Vendor class resolution simplified.** Composer classmap is the sole source of truth for vendor code.

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

[Unreleased]: https://github.com/AJenbo/phpantom_lsp/compare/0.5.0...HEAD
[0.5.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/AJenbo/phpantom_lsp/commits/0.1.0
