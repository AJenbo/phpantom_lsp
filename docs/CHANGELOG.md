# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Array element type extraction from property generics.** Bracket access on properties annotated with generic array types (e.g. `$this->cache[$key]->` where `cache` is `array<string, IntCollection>`, or `$this->translations[0]->` where `translations` is `Collection<int, Translation>`) now resolves the element type correctly. Previously the generic parameters were lost during property chain resolution, causing "subject type could not be resolved" on any member access after the bracket. Works for `$this->prop`, `$obj->prop`, nested chains like `$this->nested->prop[0]->`, string-literal keys, and method chains after the bracket access.
- **Template binding from closure return types.** Methods like `Collection::reduce()` that declare a method-level `@template` parameter bound through a callable's return type now resolve correctly. When the closure or arrow function argument has a return type annotation (e.g. `fn(Decimal $carry, $item): Decimal => ...`), the return type binds the template parameter, so the method's own return type resolves to the concrete class. This works for `fn()` arrow functions, `function()` closures, and closures with `use()` clauses.
- **Inherited docblock type propagation.** When a child class overrides a method from a parent class or interface without providing its own `@return` or `@param` docblock, the ancestor's richer types now flow through automatically. An interface declaring `@return list<Pen>` with a native `: array` hint propagates `list<Pen>` to any implementor that only declares `: array`. The same applies to parent class methods, parameter types (matched by position so renamed parameters still inherit), and property type hints. Descriptions and return descriptions are also inherited when the child lacks them. If the child provides its own docblock type, it is respected as an intentional override.
- **Drupal project support.** Drupal projects are detected via `composer.json` (`drupal/core`, `drupal/core-recommended`, or `drupal/core-dev`). The web root is resolved from `extra.drupal-scaffold.locations.web-root` with a filesystem fallback. Drupal-specific directories (`core`, `modules/contrib`, `modules/custom`, `themes/contrib`, `themes/custom`, `profiles`, `sites`) are scanned with `.gitignore` bypassed so that Composer-managed code is always indexed. Drupal PHP extensions (`.module`, `.install`, `.theme`, `.profile`, `.inc`, `.engine`) are recognized as PHP source. Contributed by @syntlyx in https://github.com/AJenbo/phpantom_lsp/pull/52.
- **`@phpstan-assert-if-true $this` narrowing.** Instance methods annotated with `@phpstan-assert-if-true` or `@phpstan-assert-if-false` targeting `$this` now narrow the receiver variable in the corresponding branch. For example, `if ($app->isTestApp())` narrows `$app` to the asserted subtype inside the then-body. Contributed by @syntlyx in https://github.com/AJenbo/phpantom_lsp/pull/52.
- **Fix unsafe `new static()` code action.** When PHPStan reports `new.static`, three quickfixes are offered: add `@phpstan-consistent-constructor` to the class docblock (preferred), add `final` to the class, or add `final` to the constructor. The diagnostic is eagerly cleared after applying any of the fixes.
- **Keyword completions.** Context-aware PHP keyword suggestions in statement positions. Keywords are filtered by scope: `return` and `yield` only inside functions, `break` only inside loops and `switch`, `continue` only inside loops, `case` and `default` only inside `switch`, `namespace` only at the top level, and `extends`/`implements` only in declaration headers. Class-like bodies restrict completions to member keywords (`public`, `function`, `const`, etc.) appropriate for the specific kind (class, interface, trait, or enum). Enum backing types (`int`, `string`) are suggested after `enum Name:`. Modifier chains (`public static `) trigger member-keyword completions. Contributed by @ryangjchandler in https://github.com/AJenbo/phpantom_lsp/pull/43.
- **Attribute completion.** Typing inside `#[…]` now only offers classes decorated with `#[\Attribute]`, filtered by the target of the declaration the attribute applies to. An attribute targeting only methods will not appear when writing `#[…]` above a class, and vice versa. Multi-attribute lists (`#[A, B]`) and namespace-qualified prefixes are supported.
- **Deferred code action computation.** Code actions now use the two-phase `codeAction/resolve` model. The lightbulb menu appears instantly because expensive edit computation (extract function, inline variable, etc.) is deferred until the user actually picks an action. PHPStan quickfixes clear their diagnostic immediately when applied instead of relying on heuristic content scanning on every keystroke.
- **Extract function/method.** Select one or more complete statements inside a function or method body and extract them into a new function or private method (`refactor.extract`). Multiple return values use array destructuring. Type hints are inferred automatically. Early returns within the selection are supported, including guard clauses.
- **Extract variable.** Select an expression and extract it into a new local variable assigned just before the enclosing statement (`refactor.extract`). Smart name generation from method calls (`getName()` → `$name`), property access (`->email` → `$email`), and function calls.
- **Extract constant.** Select a literal value (string, integer, float, or boolean) inside a class and extract it into a class constant. All identical occurrences within the class are replaced. The constant is inserted at the top of the class body with an appropriate visibility modifier.
- **Inline variable.** Place the cursor on a variable assignment and inline its value into every read site, removing the original assignment (`refactor.inline`).
- **Promote constructor parameter.** Code action on a constructor parameter that has a matching property declaration and `$this->name = $name;` assignment offers to convert it into a constructor-promoted property. The property declaration and assignment are removed, and the parameter gains the original property's visibility modifier.
- **Generate constructor.** When a class has properties but no constructor, two code actions are offered. "Generate constructor" inserts a traditional `__construct` with parameters and assignments. "Generate promoted constructor" removes the property declarations and produces a constructor with promoted parameters that need no body. Only appears when the cursor is on a non-static property.
- **Generate getter/setter.** Place the cursor on a property declaration and trigger a code action to generate `getX()`/`setX()` accessor methods. Bool properties use an `is` prefix (`isActive()`). Readonly properties only offer a getter. Static properties generate static methods. Docblock-only types produce `@return`/`@param` tags instead of native type hints. Setters return `$this` for fluent chaining.
- **Generate property hooks (PHP 8.4+).** Place the cursor on a property declaration and generate `get` and/or `set` hooks inline on the property. Three code actions are offered: "Generate get hook", "Generate set hook", and "Generate get and set hooks". Interface properties generate abstract hook signatures without bodies.
- **Add `#[Override]` code action.** When PHPStan reports `method.missingOverride`, a quickfix inserts `#[Override]` above the method declaration with correct indentation and adds a `use Override;` import when the file declares a namespace. The diagnostic disappears on the next keystroke without waiting for the next PHPStan run.
- **Remove `#[Override]` code action.** When PHPStan reports `method.override`, `property.override`, or `property.overrideAttribute` (the attribute is present but the member does not actually override anything, or `#[Override]` on properties is not supported in the current PHP version), a quickfix removes the `#[Override]` attribute. If the attribute shares a line with other attributes, only the `Override` token is removed. The diagnostic is eagerly cleared once the attribute is gone.
- **Add `#[\ReturnTypeWillChange]` code action.** When PHPStan reports `method.tentativeReturnType`, a quickfix inserts `#[\ReturnTypeWillChange]` above the method declaration with correct indentation. The diagnostic is eagerly cleared once the attribute is present.
- **Simplify with null coalescing / null-safe operator.** Ternary expressions that guard against `null` are detected and a code action offers to rewrite them. `isset($x) ? $x : $default` and `$x !== null ? $x : $default` become `$x ?? $default`. `$x !== null ? $x->foo() : null` becomes `$x?->foo()` (PHP 8.0+ only).
- **Completion and signature help for `new self`, `new static`, and `new parent`.** Inside a class, typing `new sel` offers `self` and `static` as keyword completions with constructor parameter snippets. `parent` is offered when the class has a parent. Signature help triggers when typing inside the parentheses of `new self(`, `new static(`, or `new parent(`. Contributed by @RemcoSmitsDev in https://github.com/AJenbo/phpantom_lsp/pull/51.
- **Fix PHPDoc type mismatch code actions.** When PHPStan reports that a `@return`, `@param`, or `@var` tag has a type incompatible with the native type hint (`return.phpDocType`, `parameter.phpDocType`, `property.phpDocType`), two quickfixes are offered: update the tag type to match the native type, or remove the tag entirely. The diagnostic is eagerly cleared after applying either fix.
- **Fix overriding visibility code action.** When PHPStan reports `method.visibility` or `property.visibility` (a child member is more restrictive than the parent), the change-visibility action is promoted to a quickfix with only the valid target(s) offered. "Should also be public" offers a single preferred fix; "should be protected or public" offers both with the most restrictive marked preferred. The diagnostic is eagerly cleared and non-ignorable errors are excluded from the "Add `@phpstan-ignore`" action. The change-visibility refactoring also now filters alternatives by parent and interface constraints even without a PHPStan diagnostic present.
- **Fix prefixed class name code action.** When PHPStan reports `class.prefixed` (a class name with a vendor prefix like `_PHPStan_`, `RectorPrefix`, or `_PhpScoper`), a quickfix replaces the prefixed name with the corrected one. The diagnostic is eagerly cleared after applying the fix.
- **Remove always-true `assert()` code action.** When PHPStan reports `function.alreadyNarrowedType` for a call to `assert()` that will always evaluate to true, a quickfix offers to delete the no-op statement. Only `assert()` calls are matched — other functions sharing the same identifier (e.g. `is_string()` inside conditions) are excluded because removal would change control flow. The diagnostic is eagerly cleared once `assert(` no longer appears on the line.
- **Fix void return mismatch code actions.** When PHPStan reports `return.void` (a void function returns an expression), a quickfix strips the expression to produce a bare `return;`. When PHPStan reports `return.empty` (a non-void function has a bare `return;`), a quickfix changes the native return type to `void` and removes any `@return` docblock tag. The two actions chain naturally: fixing `return.void` may trigger `return.empty`, which then fixes the signature.
- **Remove unreachable statement code action.** When PHPStan reports `deadCode.unreachable`, a quickfix deletes the dead statement. The statement-removal helper is shared infrastructure that a future native dead-code diagnostic (D6) can reuse.

### Changed

- **`@phpstan-ignore` is never the preferred quickfix.** The "Ignore PHPStan error" code action now explicitly sets `is_preferred: false`. Previously it used `None`, which some editors treated as absent, causing the suppress-with-comment action to be applied on the keyboard shortcut (e.g. Ctrl+. then Enter) when no other quickfix set `is_preferred`.

- **Faster startup.** Stub loading during initialization is significantly faster.
- **More accurate type operations.** Type substitution during generic resolution (e.g. `Collection<int, User>` inheriting from `Collection<TKey, TValue>`) now operates on the structured type tree instead of string manipulation, improving correctness for complex nested types.
- **Faster type resolution.** The central type resolution pipeline now operates on structured types directly instead of converting to strings and re-parsing at each step. Union and intersection members, array shape lookups, and generic argument extraction all avoid redundant parsing.

### Fixed

- **Null narrowing from `!== null` checks in conditions.** When a null-initialized variable was guarded by `$var !== null` in an `if` or `while` condition, the variable still showed `null` in its type inside the condition's `&&` operands and inside the then-body. The `!== null` check (and `!is_null()`, bare truthy guards) now narrows away `null` both for subsequent `&&` operands and inside the corresponding body block. Chained conditions like `$a !== null && $b !== null && $a->method()` narrow all checked variables. Conditions wrapped inside ternary expressions and return statements are also handled.
- **Fluent chains only flag the first broken link.** In a chain like `$m->callHome()->callMom()->callDad()` where `callHome` does not exist, only `callHome` is flagged. Previously every subsequent link received its own "cannot verify" warning, burying the root cause in noise. Separate statements on the same variable (`$m->callHome(); $m->callMom();`) still flag independently. Scalar member access chains (`$user->getAge()->value->deep`) flag only the first scalar break. Null-safe, static, and mixed-operator chains are all handled.
- **Scope methods missing from completion on relationship results.** When a relationship method like `$product->translations()` returned `HasMany<ProductTranslation>`, scope methods from the related model (e.g. `language()`, `published()`) appeared in hover but not in the completion dropdown. The resolved-class cache stored a version of `HasMany` without generic arguments, and the completion builder's re-resolution hit this cache entry, discarding the scope-injected methods. Completions now merge back any methods from the generically-resolved candidate that the cache entry lacks.
- **Nullable `static` return types on inherited methods.** Methods returning `?static` or `static|null` now correctly resolve to the calling subclass instead of falling through to a name-based lookup that could fail for cross-file classes not in the current file's use-map.
- **Template binding with nested generics.** `@param` types like `Wrapper<Collection<T>, V>` previously broke during template binding because generic arguments were split on commas without respecting nesting depth. Template binding and parameter extraction now use the structured type parser.
- **`@property` and `@method` tags losing nullable types.** Tags like `@property int|null $foo` had their `|null` stripped by `clean_type()`, causing the property to appear as non-nullable. The full type is now preserved.
- **Callable types inside unions displayed ambiguously.** `(Closure(int): string)|Foo` was formatted as `Closure(int): string|Foo`, which reads as a callable returning `string|Foo`. Callable types inside unions are now wrapped in parentheses.
- **Hover and go-to-definition on attributes.** Attributes on properties, class constants, function/method parameters, and enum cases are now recognized by the symbol map. Previously only attributes on classes, methods, and top-level functions produced navigable symbols; hovering or Ctrl+Clicking an attribute on a property (e.g. `#[Assert\NotBlank]`) did nothing.
- **Function-level `@template` with `array<TKey, TValue>` parameters.** Template substitution at call sites now correctly resolves generic wrapper names like `array`, `iterable`, and `list`. Previously, `collect($users)->first()->` failed to infer the element type because the wrapper name was incorrectly discarded during binding classification.
- **Stack overflow when a foreach value variable shadows the iterator receiver.** Patterns like `foreach ($category->getBranch() as $category)` caused infinite recursion during type resolution because resolving the value variable re-entered the same foreach. The foreach resolver now detects this cycle at the AST level and skips the recursive path. A depth guard on `resolve_variable_types` provides a safety net for any remaining recursive patterns.
- **PHPStan diagnostics hidden when a native diagnostic exists on the same line.** Full-line PHPStan diagnostics were suppressed whenever any precise native diagnostic appeared on the same line, even for completely unrelated issues. For example, `class.prefixed` was hidden because a native `unknown_class` diagnostic covered the same line. Deduplication now only suppresses a full-line diagnostic when the precise diagnostic on that line reports a related issue.
- **Deprecated class in `implements` now renders with strikethrough.** Verified and tested that `DiagnosticTag::DEPRECATED` applies correctly for deprecated classes referenced in `implements` clauses, matching the existing coverage for `new`, type hints, and `extends`. Also verified that `$this`/`self`/`static` resolve to the correct class in files with multiple class declarations.
- **`@param` docblock overrides ignored when the native type hint resolves.** When a method parameter had both a native type hint (e.g. `Node $node`) and a `@param` override with a more specific type (e.g. `@param FuncCall $node`), completions and diagnostics used the native type because it resolved to a class first. The docblock override is now checked before resolution so the more specific type takes effect. Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/55.
- **"Remove unused import" left behind blank lines.** Removing a single unused import or bulk-removing all unused imports no longer leaves stray blank lines. When the entire import block is removed, the separator line between imports and class body is consumed. When an import between two groups is removed, the resulting doubled blank line is collapsed. Contributed by @calebdw in https://github.com/AJenbo/phpantom_lsp/pull/54.
- **Inline `@var` cast no longer overrides the variable type on the RHS of the same assignment.** `/** @var array<string, mixed> */ $data = $data->toArray()` previously resolved the RHS `$data` as `array<string, mixed>` instead of its previous type, producing false "method not found" errors. The cast now applies only after the assignment completes.
- **Single generic argument on collections bound to the wrong template parameter.** Writing `Collection<SectionTranslation>` on a class with `@template TKey of array-key` and `@template TValue` assigned the argument to `TKey` (by position), leaving `TValue` unsubstituted. When fewer generic arguments are provided than template parameters and the leading parameters have key-like bounds (`array-key`, `int`, `string`), the arguments are now right-aligned so they bind to the value parameters. Method-level template parameters whose bound parameter was not passed at the call site and defaults to `null` now resolve to `null` instead of remaining as raw template names. Together these fixes mean `Collection<SectionTranslation>::first()` correctly resolves to `SectionTranslation|null`.

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
