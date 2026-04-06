# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Refactoring code actions.** Extract function, extract method, extract variable, extract constant, inline variable, promote constructor parameter, generate constructor (traditional and promoted), generate getter/setter, and generate property hooks (PHP 8.4+). Deferred computation ensures the lightbulb menu appears instantly; edit generation only runs when the user picks an action.
- **PHPStan quickfixes.** Automated fixes for a wide range of PHPStan diagnostics: update or remove mismatched `@return`/`@param`/`@var` tags, remove unused return type union members, fix unsafe `new static()` (add `@phpstan-consistent-constructor`, `final` class, or `final` constructor), add or remove `#[Override]`, add `#[\ReturnTypeWillChange]`, fix void return mismatches, add inferred iterable return types, remove unreachable statements, remove always-true `assert()` calls, fix overriding member visibility, fix vendor-prefixed class names, and simplify ternary expressions to `??` or `?->`. All quickfixes eagerly clear their diagnostic on apply.
- **`fix` CLI subcommand.** `phpantom_lsp fix` applies automated code fixes across a project. Specify rules with `--rule` (multiple allowed) or omit to run all preferred fixers. `--dry-run` reports what would change without writing files. The first shipped rule, `unused_import`, removes unused `use` statements project-wide, collapsing blank lines left behind by removals. Supports path filtering and single-file mode.
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
- **Standalone `@var` docblock for untyped closure parameters.** When a closure parameter lacks a type hint and no assignment follows, a `@var` block above the usage is now picked up as the variable's type.
- **`--stdio` CLI flag.** Accepted (and ignored) for compatibility with LSP client wrappers that pass `--stdio` by default.
- **Method-level template parameters resolve inside method bodies.** `@template T of Builder` with `@param T $query` now resolves `$query` to the template bound inside the method body, providing completions from the bound class.
- **Structural subtype checking.** `PhpType::is_subtype_of` answers whether one type is a structural subtype of another without consulting a class hierarchy. Covers the full PHP scalar lattice (including refinements like `positive-int`, `non-empty-string`, `class-string`), `never` as bottom, `mixed` as top, nullable/union/intersection decomposition, generic covariance for array-like containers, callable variance, literal types, and int ranges.
- **Union simplification.** `PhpType::simplified` deduplicates union members, merges `true|false` into `bool`, absorbs scalar refinements into their parent types (`positive-int|int` becomes `int`), collapses unions containing `mixed`, flattens nested unions and intersections, and unwraps single-member composites.
- **Intersection distribution over unions.** `PhpType::distribute_intersection` transforms `(A|B) & C` into `(A&C) | (B&C)`, producing disjunctive normal form for type narrowing.

### Changed

- **Faster file parsing.** Type override resolution (`resolve_effective_type`, `should_override_type`) now accepts pre-parsed types directly, eliminating redundant stringify-parse round-trips on every method and property during indexing.
- **Fewer false-positive diagnostics.** Variable resolution now produces the same result across completions, hover, and diagnostics, eliminating cases where diagnostics disagreed about a variable's type.
- **`@phpstan-ignore` is never the preferred quickfix.** The "Ignore PHPStan error" code action is explicitly non-preferred, so editor keyboard shortcuts no longer accidentally apply it when another fix is available.
- **Generate PHPDoc infers `@return` from the function body.** Typing `/**` above a function that returns `array` now produces a specific element type (e.g. `@return list<string>`) instead of `@return array<mixed>`.
- **Faster startup.** Stub loading during initialization is significantly faster.
- **More accurate generics resolution.** Type substitution and resolution for complex nested generic types is more correct, particularly for unions, intersections, array shapes, and deeply nested generic arguments.

### Fixed

- **Missing diagnostics and import actions in files without a namespace.** When a namespaced class (e.g. `Carbon\Carbon`) had already been parsed, using its short name (`Carbon`) in a file without a `namespace` declaration incorrectly resolved against the namespaced class. This suppressed both the "class not found" diagnostic and the "Import" code action. Bare-name lookups now only match classes that are themselves in the global namespace.
- **Find-references false positives for global classes.** Searching for references to a global-scope class (e.g. `Helper` with no namespace) could include references to unrelated namespaced classes with the same short name (e.g. `App\Helper`). Short-name fallback matching now only applies when the resolved name is unqualified.
- **Fluent chains only flag the first broken link.** In a chain where the first method does not exist, only that method is flagged instead of every subsequent call receiving its own warning.
- **Null narrowing from `!== null` checks.** Null-initialized variables guarded by `$var !== null`, `!is_null()`, or bare truthy checks now have `null` narrowed away inside the then-body and in subsequent `&&` operands. Works in chained conditions, ternary expressions, and return statements.
- **Variables assigned inside `if`/`while` conditions now resolve in the body.** `if ($admin = AdminUser::first())` and `while ($row = nextRow())` register the assignment so the variable has a type inside the branch or loop body.
- **Loop-body assignments not visible inside the same loop iteration.** When a variable is initialized as `null` and reassigned later in a loop body, the assigned type is now visible at every point inside the loop. Combined with null narrowing, variables correctly resolve to the assigned class type.
- **`@var` docblock annotations no longer leak across class and method boundaries.** A `@var` annotation for a same-named variable in a different class no longer bleeds into the current scope.
- **Inline `@var` cast no longer overrides the variable type on the RHS of the same assignment.** `/** @var array<string, mixed> */ $data = $data->toArray()` no longer resolves the RHS `$data` using the cast type.
- **`@param` docblock overrides ignored when the native type hint resolves.** When a parameter has both a native type hint and a more specific `@param` override, the docblock type now takes effect. Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/55.
- **Variable reassignment inside `try`/`catch`/`finally` blocks now tracked.** Subsequent accesses within the same block resolve against the reassigned type instead of the original.
- **Self-referential variable reassignments in nested loops no longer produce false "type could not be resolved" diagnostics.** Recursive resolution that hits the depth limit no longer poisons the cache for later lookups.
- **`instanceof` narrowing with unresolvable target class.** When the target class cannot be loaded, the variable's type is treated as unknown instead of keeping the un-narrowed type, eliminating false positives for members on the narrowed subclass.
- **`stdClass` and `object` types no longer produce false-positive diagnostics.** Variables typed as `object` or `stdClass` now permit arbitrary property access. `is_object()` correctly narrows `mixed` to `object` and compound `&&` conditions propagate the narrowing.
- **`class-string<T>` static method dispatch.** Calling static methods on a `class-string<Foo>` variable now resolves return types correctly, including `static` substitution to the bound class.
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
- **Aliased namespace imports used in attributes no longer flagged as unused.** `use Symfony\Component\Validator\Constraints as Assert;` with `#[Assert\Uuid(...)]` no longer produces a false "Unused import" diagnostic.
- **`DB::select()` return type.** `DB::select()` and related methods now return `array<int, stdClass>` instead of bare `array`, and `DB::selectOne()` returns `?stdClass`.
- **Redis `Connection` method resolution.** Redis commands on `Illuminate\Redis\Connections\Connection` now resolve through the phpredis stubs.
- **Array shape tracking from keyed assignments inside conditional branches.** Shape types built incrementally with variable keys inside loops with if/else branching are now preserved through foreach iteration.
- **Deprecated class in `implements` renders with strikethrough.** Deprecated classes referenced in `implements` clauses are correctly tagged.

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

[Unreleased]: https://github.com/AJenbo/phpantom_lsp/compare/0.6.0...HEAD
[0.6.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.5.0...0.6.0
[0.5.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.2.0...0.3.0
[0.2.0]: https://github.com/AJenbo/phpantom_lsp/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/AJenbo/phpantom_lsp/commits/0.1.0
