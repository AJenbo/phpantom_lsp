# Changelog

All notable changes to PHPantom will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Function-level `@template` with generic return types.** Functions like `collect()` that use `@template` parameters inside a generic return type (e.g. `@return Collection<TKey, TValue>`) now resolve the concrete types from call-site arguments. Previously only the narrow `@return T` pattern worked. This covers `collect()`, `value()`, `retry()`, `tap()`, `with()`, `transform()`, `data_get()`, and any non-Laravel function with the same annotation style.

- **Find References.** "Find All References" locates every usage of a symbol across the project. Supports classes, interfaces, traits, enums, methods, properties, constants, functions, and variables. Variable references are scoped to the enclosing function or closure. Member references are scoped to the class hierarchy of the target symbol, so unrelated classes that happen to share a method name are excluded. Cross-file scanning lazily indexes user files on demand (vendor and stub files are excluded, matching PhpStorm's behaviour).
- **Rename.** `textDocument/rename` with `prepareRename` support. Rename variables, classes, methods, properties, functions, and constants across the entire workspace. Variable renames are scoped to the enclosing function or closure. Property renames handle the `$` prefix correctly at declaration vs. access sites. Symbols defined in vendor files are rejected. Non-renameable tokens (`$this`, `self`, `static`, `parent`) are rejected at the prepare step so the editor never opens the rename prompt.
- **Document highlighting.** Placing the cursor on a symbol highlights all other occurrences in the current file. Variables are scoped to their enclosing function or closure. Assignment targets, parameters, foreach bindings, and catch variables are marked as writes; all other occurrences are reads. Class names, members, functions, and constants highlight file-wide.
- **Implement missing methods.** A new code action generates method stubs when a concrete class extends an abstract class or implements an interface with unimplemented methods. Place the cursor inside the class body and trigger "Quick Fix" to insert stubs with correct visibility, static modifier, parameter types with defaults, and return type. Works across files with PSR-4 resolution, handles deep inheritance chains, and respects the file's existing indentation style.
- **Reverse go-to-implementation.** Invoking go-to-implementation on a method definition in a concrete class jumps to the interface or abstract class that declares the prototype. Also works from interface and abstract class method declarations in the forward direction, finding all concrete implementations.
- **Diagnostics.** Unknown class references are underlined with a warning and pair with the "Import Class" code action for one-step `use` statement insertion. Unknown member accesses (methods, properties, constants) are flagged after full resolution including inheritance, traits, and virtual members. Calls to unknown functions produce an error diagnostic ("Function 'x' not found"). Member access on a variable whose type is an unresolvable class now produces a warning naming the unknown type instead of a generic hint. When multiple diagnostic providers flag the same span, the lower-value diagnostic is suppressed (e.g. `unresolved_member_access` is dropped when `unknown_class` or `scalar_member_access` already covers the range). An opt-in unresolved member access diagnostic (`[diagnostics] unresolved-member-access` in `.phpantom.toml`) flags accesses where the subject type cannot be resolved at all. Template parameters, type aliases, and magic method classes are excluded to avoid false positives.
- **Deprecation support.** PHPantom reads both `@deprecated` docblock tags and `#[Deprecated]` attributes (used by phpstorm-stubs for ~362 elements). The `reason` and `since` fields appear in hover, completion strikethrough, and diagnostics. When the attribute declares `since: "X.Y"` and your project targets an older PHP version, the diagnostic is suppressed. Deprecated member access through variables (e.g. `$svc->oldMethod()`) now produces diagnostics. A quick-fix code action rewrites deprecated calls when a `replacement` template is available.
- **Project configuration.** PHPantom reads a `.phpantom.toml` file from the project root for per-project settings. Supports `[php] version` to override the detected PHP version, `[diagnostics] unresolved-member-access` to enable the unresolved member access diagnostic, and `[indexing] strategy` to control class discovery. Run `phpantom --init` to generate a default config file.
- **Self-generated classmap.** PHPantom now works without running `composer dump-autoload -o`. When the Composer classmap is missing or incomplete, PHPantom scans the project's autoload directories itself to build a class index. Non-Composer projects are also supported by scanning all PHP files in the workspace. Composer autoload files are scanned with a lightweight byte-level pass and lazily parsed on first access.
- **Non-Composer function and constant discovery.** Projects without `composer.json` now get cross-file function completion, go-to-definition, and constant resolution. The workspace scanner extracts standalone `function` declarations, `define()` constants, and top-level `const` statements alongside classes in a single byte-level pass.
- **Monorepo support.** When the workspace root has no `composer.json`, PHPantom discovers subdirectories that are independent Composer projects and processes each through the full Composer pipeline. Loose PHP files outside subproject directories are discovered by a separate workspace scan.
- **Indexing progress indicator.** The editor now shows a progress bar during workspace initialization. In monorepo workspaces with multiple subprojects, the indicator shows which subproject is being indexed and overall progress.
- **PHP version-aware stubs.** PHPantom detects the target PHP version from `composer.json` (`config.platform.php` or `require.php`) and filters built-in stub signatures accordingly. Functions, methods, and parameters that do not apply to the detected version are excluded. When no version is detected, PHP 8.5 is assumed.
- **`@implements` generic resolution.** When a class declares `@implements SomeInterface<ConcreteType>`, template parameters on the interface's methods and properties are now substituted with the concrete types. Works with `@template-implements` and `@phpstan-implements` aliases, multiple annotations on the same class, parameter type substitution, and chained resolution through parent classes. Foreach iteration over classes implementing generic iterable interfaces now resolves value and key types correctly.
- **Interface template inheritance.** When a class implements an interface whose methods use `@template`, `@param class-string<T>`, or `@return T`, the implementing class's overridden methods now inherit the template parameters, bindings, conditional return types, and type assertions.
- **Generic `@phpstan-assert` with `class-string<T>`.** `@phpstan-assert T $value` combined with a `@template T` bound via `class-string<T>` now resolves the narrowed type from the call-site argument. Also works on static method calls like `Assert::instanceOf($value, Foo::class)`.
- **Property-level narrowing.** `if ($this->prop instanceof Foo)` now narrows the type of `$this->prop` inside the then-body, else-body, and after guard clauses. `assert($this->prop instanceof Foo)` also works. Previously only plain variables participated in instanceof narrowing.
- **Inline `&&` short-circuit narrowing.** In `if ($x instanceof Foo && $x->bar())`, the right-hand side of `&&` now sees the narrowed type from the left-hand side. Previously narrowing only applied inside the if body, so method calls within the condition itself were checked against the original (broader) type.
- **Compound negated guard clause narrowing.** After `if (!$x instanceof A && !$x instanceof B) { return; }`, the surviving code narrows `$x` to `A|B`. Previously only single negated instanceof guard clauses were recognized.
- **Invoked closure and arrow function return types.** `(fn(): Foo => new Foo())()` and `(function(): Bar { return new Bar(); })()` now resolve to the return type of the closure or arrow function.
- **`new $classStringVar` and `$classStringVar::method()`.** When a variable holds a class-string value (e.g. `$f = Foo::class`), `new $f` resolves to `Foo` and `$f::staticMethod()` resolves through `Foo`'s static members.
- **`iterator_to_array()` element type.** `iterator_to_array($iter)` now resolves the element type from the iterator's generic annotation.
- **Enum case properties.** Completing on an enum case variable (`$case->`) now shows `name` (on all enums) and `value` (on backed enums) inherited from the `UnitEnum` and `BackedEnum` interfaces.
- **Pass-by-reference parameter type inference.** After calling a function that accepts a typed `&$var` parameter, the variable acquires the parameter's type for subsequent completion.
- **Pipe operator (PHP 8.5).** `$input |> trim(...) |> createDate(...)` resolves through the chain, returning the last callable's return type.
- **Closure variable scope isolation.** Variables declared outside a closure are no longer offered as completions inside the closure body unless captured via `use()`.
- **AST-based array type inference.** Array shape key completion and array element member access resolve through an AST walker that handles literal arrays, `new` expressions, call expressions, spread elements, incremental key assignments, and push-style assignments.
- **Docblock navigation.** Go-to-definition and hover now work on class names inside callable type annotations, array and object shape value types, and object shape properties.
- **GTD from parameter and property variables.** Clicking a parameter or property variable at its definition site now jumps to the type hint class. Catch variables with single or union type hints are also supported.
- **Inline `@var` on promoted constructor properties.** A `/** @var array<EventModel> */` docblock placed directly above a promoted parameter now overrides the native type hint, matching the existing `@param` support on the constructor. Common with Spatie's laravel-data and similar packages that use `array|Optional` union types.

### Changed

- **Resolution engine rewritten on AST.** Variable type inference, subject dispatch, call return type resolution, member-access detection, and go-to-definition lookups all run through the AST walker now. The text-based scanner and its line-by-line fallbacks have been removed entirely. This fixes a class of edge-case bugs with null-safe chains, parenthesized `new` expressions, chained method calls, and complex array access patterns. Go-to-definition and go-to-implementation use only the precomputed symbol map with byte offsets exclusively.
- **Hover redesigned.** Hover popups use short names with a `namespace` line, show actual default values, display `@link` URLs, and highlight the precise hovered token. `new ClassName` shows the constructor signature. `@template` parameters show their declaration, variance, and bound. Hovering an enum shows all cases, hovering a trait shows its public members. Origin indicators show whether a member overrides a parent, implements an interface, or is virtual. Deprecated members show the explanation text. Constants show their initializer value inline.
- **Signature help enriched.** The popup shows a compact parameter list with native PHP types and return type, plus a per-parameter `@param` description with the effective docblock type. Optional parameters display their default value. Signature help also fires inside PHP 8 attribute parentheses.
- **Faster resolution and lower memory usage.** Class resolution uses O(1) hash-map lookups and caches fully-resolved classes per request cycle. Inheritance merging uses hash-set deduplication. File content and symbol maps are reference-counted. Cross-file scans share data by reference instead of deep-cloning. Diagnostics run asynchronously with 500 ms debounce. Cache invalidation is signature-aware, so edits inside method bodies keep the cache warm.
- **Two-phase diagnostic publishing.** Cheap diagnostics (unused import dimming, deprecated strikethrough) are published immediately. Expensive diagnostics (unknown classes, unknown members, unknown functions) arrive in a second pass. Previously all diagnostics waited for the slowest collector to finish, so unused imports could take several seconds to grey out in large files.
- **Concurrent read access to shared state.** All read-heavy maps now use `parking_lot::RwLock` instead of `std::sync::Mutex`, allowing multiple requests to read in parallel.
- **Parallel workspace indexing.** Find References and other workspace-wide operations now parse files across multiple CPU cores. The workspace walk respects `.gitignore` rules instead of hardcoding directory names to skip. Self-scan classmap building, PSR-4 directory scanning, and vendor package scanning now read and scan files in parallel across all CPU cores. The byte-level PHP scanner uses `memchr` SIMD acceleration to skip comments, strings, and heredocs instead of scanning byte-by-byte. Redundant `composer.json` reads during initialization have been eliminated.

### Fixed

- **Type alias array shape diagnostics no longer fire on object values.** When a method returns a `@phpstan-type` alias that expands to an array shape containing object values (e.g. `array{pen: Pen}`), accessing a method on the object value no longer triggers a false diagnostic. Type aliases are now expanded before walking array shape segments in the resolution pipeline.
- **Inline array-element function calls resolve correctly in diagnostics.** `end($obj->items)->method()` no longer produces a false "unknown member" diagnostic. Previously the argument text for property access expressions was lost during symbol map extraction, causing the resolver to fall back to the native `mixed|false` return type instead of extracting the element type from the array's generic annotation.
- **Eloquent Builder scope chain diagnostics no longer flicker.** Scope methods on Builder chains (e.g. `Bakery::where(...)->fresh()->get()`) no longer produce false diagnostics that flip on and off between edits. The diagnostic now checks the pre-resolved class returned by the completion pipeline before re-resolving through the cache, preventing stale cache entries from hiding context-specific virtual members.
- **Diagnostics refresh across open files when a class signature changes.** Editing a class (adding a method, changing a relation, modifying `$casts`, renaming a property) now triggers a diagnostic re-check in all other open files. Previously only the edited file was re-diagnosed, so stale "unknown member" or "unknown class" warnings in other tabs persisted until those files were manually re-opened or edited.
- **Unknown member diagnostics on property chains and method return chains.** `$obj->prop->member` and `$obj->getX()->member` now flag unknown members on the resolved intermediate type. Previously the diagnostic was silently skipped because the subject resolver could not handle chained expressions. Virtual properties from `@property` annotations are also resolved through the chain. When the intermediate type is scalar (e.g. `$model->boolProp->value`), a "Cannot access property on type 'bool'" diagnostic is emitted instead of silently skipping.
- **Variable type resolves through ternary, elvis, null-coalesce, and match assignments.** When a variable is reassigned via `$x = $x ?: Foo::bar()`, `$x = $a ?? $b`, `$x = $cond ? $a : $b`, or `$x = match(...) { ... }`, hover and foreach iteration now show the resolved type from the assignment branches. Previously these expressions returned unknown because the raw type inference path did not recurse into conditional expressions.
- **Parameter types resolve inside `function_exists` guards.** Functions wrapped in `if (!function_exists('name')) { … }` now resolve parameter type hints for completion and hover. Previously the nested function scope was invisible to the variable resolver.
- **Virtual property merging picks the most specific type.** When multiple sources contribute the same model property (e.g. `$fillable` produces `mixed`, `$casts` produces `array`, `@property` declares `array<string>`), the most specific type now wins. Previously the first writer was kept regardless of specificity.
- **Custom cast classes declared as string literals resolve correctly.** Cast entries like `'description' => 'App\Casts\HtmlCast'` no longer have their FQN mangled by prepending the file's namespace. Previously this produced broken names like `App\Models\App\Casts\HtmlCast`.
- **`@implements CastsAttributes<T>` takes priority over `get()` return type.** The class-level generic annotation on a custom cast class is now the primary type signal. Previously the `get()` method was checked first, so a `mixed` return type on `get()` would shadow the more specific `@implements` annotation.
- **Editing a cast class now updates model property types.** Changing the `@implements CastsAttributes<T>` annotation or `get()` return type on a custom cast class is now reflected immediately in models that reference it via `$casts`. Previously the model's cached resolution was not invalidated when the cast class changed.
- **Go-to-definition for variables captured via `use`.** Clicking a `$var` inside a closure body now jumps to the `use ($var)` clause. Clicking `$var` in the `use` clause itself jumps to the outer assignment. Previously captured variables had no GTD support inside the closure body.
- **Closure parameter inference inside namespaces.** When code is wrapped in a `namespace Foo\Bar { … }` block or follows a `namespace Foo\Bar;` declaration, closure parameter types inferred from callable signatures (e.g. `@param (\Closure(static): mixed)` on Laravel's `where()`) now resolve correctly. Previously the namespace boundary made variable assignments and closure calls invisible to the type resolver.
- **Closure parameter inference across files.** When the receiver class is loaded from a different file (e.g. Laravel's Eloquent Builder from vendor), `static` and `$this` in callable parameter types are now replaced with the fully-qualified class name. Previously the short class name was used, which failed to resolve cross-file.
- **Signature help no longer fires inside closure and arrow function bodies.** When a closure is passed as an argument (e.g. `$query->where(function ($q) { … })`), signature help for the outer call now stops at the opening brace. Inside the closure body you see signature help only for calls you write there, not for the enclosing call whose argument list you have already moved past.
- **Signature help parameter type display with parenthesized callable unions.** The opening parenthesis in types like `(\Closure(static): mixed)|string|array` was stripped in the signature help tooltip. The full type now displays correctly.

- **`__invoke()` return type resolution.** Calling `$f()` where `$f` holds an object with an `__invoke()` method now resolves the return type correctly. Completion, chaining, foreach iteration, and parenthesized expression invocations like `($this->factory)()` all work.
- **Enum `from()` and `tryFrom()` chaining.** `MyEnum::from('value')->method()` now resolves through the enum type.
- **Nested closures with reused parameter names no longer crash.** The callable parameter inference now caps its recursion depth to break cycles.
- **Scope methods on Builder variables.** Hover, signature help, and deprecation diagnostics now find model-specific members even when the resolved-class cache holds a differently-scoped entry for the same base class.
- **Vendor class resolution simplified.** The Composer classmap is the sole source of truth for vendor code. Vendor PSR-4 mappings are no longer loaded. If the classmap is missing or stale, vendor classes fail to resolve visibly (fix: run `composer dump-autoload`).
- **`static`/`self`/`$this` in method return types used as iterable expressions.** When a method returns `static[]` and the result is iterated or assigned, the `static` token is now replaced with the actual owner class name. Also works when the result is stored in an intermediate variable before iterating.
- **`instanceof` narrowing no longer widens specific types.** `assert($zoo instanceof ZooBase)` after `$zoo = new Zoo()` (where `Zoo extends ZooBase`) no longer replaces the type with the less-specific parent.
- **Closure parameter with bare type hint now inherits inferred generics.** When a closure parameter has an explicit bare type hint (e.g. `Collection $customers`) and the callable signature infers a more specific generic form (e.g. `Collection<int, Customer>`), the inferred type is used so that foreach resolves the element type.
- **Closure parameter with parent type hint now narrows to inferred subclass.** When a closure parameter has an explicit type hint that is a parent class (e.g. `Model $item`) but the callable signature infers a more specific subclass (e.g. `BrandTranslation` where `BrandTranslation extends Model`), the inferred subclass type is used for completion, hover, and diagnostics. Previously the broader parent type was kept, hiding subclass-specific members.
- **Cross-file inheritance from global-scope classes imported via `use`.** When a class extends a global class through a `use` import (e.g. `use Exception; class AppException extends Exception {}`), inherited members now resolve correctly across namespaces.
- **Model `@method` tags available on Builder instances.** Virtual methods declared via `@method` on a model or its traits (e.g. `withTrashed` from `SoftDeletes`) now resolve on `Builder<Model>` instances in method chains. Previously these methods were only available when called statically on the model, so `Customer::where(...)->withTrashed()` lost resolution after the first chain link.
- **Arrow function outer-scope variable resolution.** Variables assigned before an arrow function are now resolved correctly inside the arrow function body. Previously, accessing a captured variable like `$feature->id` inside `fn($v) => $v->x === $feature->id` failed to resolve `$feature`, producing false-positive diagnostics and missing completions.
- **Arrow function parameter completion with incomplete expressions.** Typing `$foo->` inside an arrow function body now resolves the parameter type even when the expression is incomplete.
- **Inherited `@method` and `@property` tags.** Virtual members declared on a parent class now appear on child classes.
- **Signature help on function definitions.** Signature help no longer fires when the cursor is inside a function or method definition's parameter list.
- **Elseif chain narrowing.** The else branch now strips types from all preceding conditions in `if/elseif/else` chains.
- **Sequential assert narrowing.** Multiple `assert($x instanceof A); assert($x instanceof B);` statements now accumulate correctly.
- **First-open performance.** Diagnostics on `did_open` now run asynchronously instead of blocking the LSP response.
- **Variadic `@param` template bindings.** `@param class-string<T> ...$items` now correctly binds the template parameter.
- **Laravel relationship classification.** Relationship return types fully-qualified to a non-Eloquent namespace are no longer misclassified as Eloquent relationships.
- **Trait `use` no longer triggers false-positive unused import.** When a class uses a trait via `use TraitName;` inside the class body, the corresponding namespace import is no longer flagged as unused.
- **PHPDoc types on constructor-promoted properties now recognised.** Classes referenced in `/** @var list<Foo> */` annotations on promoted constructor parameters are no longer flagged as unused imports, and hover/go-to-definition works on those type references.
- **PHPDoc type tags no longer skipped by unused-import safety net.** Docblock lines containing type-bearing tags (`@var`, `@param`, `@return`, `@throws`, `@template`, etc.) are now checked for class references instead of being blanket-skipped as comments.
- **`@phpstan-type` aliases in foreach.** Type aliases now resolve correctly when iterated in a `foreach` loop, destructured with `list()`/`[]`, or used as a foreach key type.
- **Mixed `->` then `::` accessor chains.** Expressions like `$obj->prop::$staticProp` now resolve through the full chain.
- **Inline `(new Foo)->method()` chaining.** Parenthesized `new` expressions used as the root of a method chain now resolve for completion.
- **Literal string conditional return types.** Conditional return types checking against a literal string value now resolve the correct branch.
- **Class constant and enum case assignment resolution.** Assigning from a class constant or enum case now resolves the variable's type correctly.
- **False-positive unknown-class warnings on PHPStan type syntax.** String literals in conditional return types, numeric literals, and variance annotations on generic arguments no longer trigger "Class not found" warnings.
- **Go-to-implementation no longer produces false positives across namespaces.** Implementor scanning and deduplication now use fully-qualified names.
- **Named-argument resolution for non-variable subjects.** Named arguments now resolve correctly when the call target is a bare class name, a chain result, or a static method.
- **"Remove all unused imports" no longer offered everywhere.** The bulk code action now only appears when the cursor is on a `use` import line instead of on every line in the file.
- **GTD for `@method`/`@property` on interfaces.** Go-to-definition now walks implemented interfaces before checking `@mixin` classes.
- **`?->` null-safe chain resolution.** The `->` inside `?->` no longer confuses subject splitting.
- **`(new Canvas())->easel` property access.** Parenthesized `new` expressions on the left side of `->` now resolve correctly for variable type inference.
- **Array function resolution.** `array_pop`, `array_filter`, `array_values`, `end`, and `array_map` now resolve element types correctly when the array comes from a method call chain or property access.
- **Hover on variable definition sites.** Hovering a parameter, foreach binding, or catch variable no longer shows a redundant popup when the type is already visible in the signature.
- **Inline `@var` annotations no longer leak across scopes.** A `/** @var Type $var */` annotation in one method no longer affects a same-named variable in a different method or class.
- **Docblock tag parsing in description text.** Tags appearing mid-sentence in docblock descriptions are no longer mistakenly parsed as tags.
- **Double-negated `instanceof` narrowing.** `if (!!$x instanceof Foo)` now correctly narrows `$x` to `Foo`.
- **Accessor on new line with whitespace.** Completion now works when `->` is on a new line with extra whitespace before the cursor.
- **Partial static property completion.** Typing `$obj::$f` now returns static property completions.
- **Hover respects `instanceof` and `assert` narrowing.** Hovering a variable after `assert($var instanceof Foo)` now shows the narrowed type instead of the original assignment type. Inline `/** @var Type */` annotations on assignments are also respected by the hover type-string path. Previously these narrowing patterns only affected completion, not hover.
- **`instanceof` narrowing with same-named classes in different namespaces.** When two classes share the same short name (e.g. `Contracts\Provider` and `Concrete\Provider`), `instanceof` narrowing no longer incorrectly treats them as subtypes of each other.
- **Self-referential array key assignments no longer crash the LSP.** Patterns like `$numbers['price'] = $numbers['price']->add(...)` caused infinite recursion in the raw-type inference path, producing a stack overflow that killed the server process. The resolver now reduces the cursor offset before evaluating the right-hand side, matching the protection already present for simple variable assignments.
- **Cross-file `@property` and `@method` type resolution.** When a class declares `@property Carbon $created` using a short class name imported via `use`, accessing that property from a different file now resolves the type correctly. Previously the short name was resolved against the consuming file's imports instead of the declaring file's imports, causing completion and hover to fail.
- **Editing a `@property` docblock now invalidates hover in other files.** Changing a class-level `@property` (or `@method`) type was not reflected when hovering on a child class that inherits the virtual member. The resolved-class cache now transitively evicts dependent classes (children, trait users, implementors, mixin consumers) when an ancestor's signature changes.

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
