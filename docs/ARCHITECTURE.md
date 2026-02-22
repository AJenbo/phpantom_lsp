# PHPantom Architecture

This document explains how PHPantom resolves PHP symbols — classes, interfaces, traits, enums, and functions — across files and from the PHP standard library.

## Overview

PHPantom is a language server that provides completion and go-to-definition for PHP projects. It works by:

1. **Parsing** PHP files into lightweight `ClassInfo` / `FunctionInfo` structures (not a full AST — just the information needed for IDE features).
2. **Caching** parsed results in an in-memory `ast_map` keyed by file URI.
3. **Resolving** symbols on demand through a multi-phase lookup chain.
4. **Merging** inherited members (from parent classes, traits, interfaces, and mixins) at resolution time.

## Module Layout

```
src/
├── lib.rs                  # Backend struct, state, constructors
├── main.rs                 # Entry point (stdin/stdout LSP transport)
├── server.rs               # LSP protocol handlers (initialize, didOpen, completion, …)
├── types.rs                # Data structures (ClassInfo, MethodInfo, PropertyInfo, …)
├── composer.rs             # composer.json / PSR-4 autoload parsing
├── stubs.rs                # Embedded phpstorm-stubs (build-time generated index)
├── resolution.rs           # Multi-phase class/function lookup and name resolution
├── inheritance.rs          # Class inheritance merging (traits, mixins, parent chain)
├── subject_extraction.rs   # Shared helpers for extracting subjects before ->, ?->, ::
├── util.rs                 # Position conversion, class lookup, logging
├── parser/
│   ├── mod.rs              # Top-level parse entry points (parse_php, parse_functions, …)
│   ├── classes.rs          # Class, interface, trait, enum, and anonymous class extraction
│   ├── functions.rs        # Standalone function and define() constant extraction
│   ├── use_statements.rs   # use statement and namespace extraction
│   └── ast_update.rs       # update_ast orchestrator and name resolution helpers
├── docblock/
│   ├── mod.rs              # Re-exports from submodules
│   ├── tags.rs             # PHPDoc tag extraction (@return, @var, @mixin, @deprecated, …)
│   ├── templates.rs        # Template/generics/type-alias tags (@template, @extends, …)
│   ├── virtual_members.rs  # Virtual member tags (@property, @method)
│   ├── conditional.rs      # PHPStan conditional return type parsing
│   └── types.rs            # Type cleaning utilities (clean_type, strip_nullable, …)
├── completion/
│   ├── mod.rs              # Submodule declarations
│   ├── handler.rs          # Top-level completion request orchestration
│   ├── target.rs           # Extract what the user is completing (subject + access kind)
│   ├── resolver.rs         # Resolve subject → ClassInfo (type resolution engine)
│   ├── text_resolution.rs  # Text-based type resolution (assignment scanning, call chains)
│   ├── builder.rs          # Build LSP CompletionItems from resolved ClassInfo
│   ├── class_completion.rs # Class name, constant, and function completions
│   ├── variable_completion.rs  # Variable name completions and scope collection
│   ├── variable_resolution.rs  # Variable type resolution via assignment scanning
│   ├── foreach_resolution.rs   # Foreach value/key and array destructuring type resolution
│   ├── closure_resolution.rs   # Closure and arrow-function parameter resolution
│   ├── type_narrowing.rs       # instanceof / assert / custom type guard narrowing
│   ├── conditional_resolution.rs  # PHPStan conditional return type resolution at call sites
│   ├── array_shape.rs      # Array shape key completion and raw variable type resolution
│   ├── named_args.rs       # Named argument completion inside function/method call parens
│   ├── phpdoc.rs           # PHPDoc tag completion inside /** … */ blocks
│   ├── phpdoc_context.rs   # PHPDoc context detection and symbol info extraction
│   ├── comment_position.rs # Comment and docblock position detection
│   ├── throws_analysis.rs  # Shared throw-statement scanning and @throws tag lookup
│   ├── catch_completion.rs # Smart exception type completion inside catch() clauses
│   ├── type_hint_completion.rs # Type completion in parameter lists, return types, properties
│   └── use_edit.rs         # Use-statement insertion helpers
├── definition/
│   ├── mod.rs              # Submodule declarations
│   ├── resolve.rs          # Core go-to-definition resolution (classes, functions)
│   ├── member.rs           # Member-access resolution (->method, ::$prop, ::CONST)
│   ├── variable.rs         # Variable definition resolution ($var jump-to-definition)
│   └── implementation.rs   # Go-to-implementation (interface/abstract → concrete classes)
build.rs                    # Parses PhpStormStubsMap.php, generates stub index
stubs/                      # Composer vendor dir for jetbrains/phpstorm-stubs
tests/
├── common/mod.rs           # Shared test helpers and minimal PHP stubs
├── completion_*.rs         # Completion integration tests (by feature area)
├── definition_*.rs         # Go-to-definition integration tests
├── implementation.rs       # Go-to-implementation integration tests
├── docblock_*.rs           # Docblock parsing and type tests
├── parser.rs               # PHP parser / AST extraction tests
├── composer.rs             # Composer integration tests
└── …
```

## Symbol Resolution: `find_or_load_class`

When the LSP needs to resolve a class name (e.g. during completion on `Iterator::` or when following a type hint), it calls `find_or_load_class`. This method tries four phases in order, returning as soon as one succeeds:

```
find_or_load_class("Iterator")
│
├── Phase 0: class_index (FQN → URI)
│   Fast lookup for classes indexed by fully-qualified name.
│   Handles classes that don't follow PSR-4 (e.g. Composer autoload_files).
│   ↓ miss
│
├── Phase 1: ast_map scan
│   Searches all already-parsed files by short class name.
│   This is where cached results from previous phases are found on
│   subsequent lookups — a stub parsed in Phase 3 is cached here and
│   found in Phase 1 next time.
│   ↓ miss
│
├── Phase 2: PSR-4 resolution (user code)
│   Uses Composer PSR-4 mappings to locate the file on disk.
│   Example: "App\Models\User" → workspace/src/Models/User.php
│   Reads, parses, resolves names, caches in ast_map.
│   ↓ miss
│
├── Phase 3: Embedded PHP stubs
│   Looks up the class name in the compiled-in stub index
│   (from phpstorm-stubs). Parses the stub PHP source, caches
│   in ast_map under a phpantom-stub:// URI.
│   ↓ miss
│
└── None
```

### Caching

Every phase that successfully parses a file caches the result in `ast_map`. This means:

- Phase 2 (PSR-4) files are parsed once, then found via Phase 1.
- Phase 3 (stubs) are parsed once, then found via Phase 1.
- Files opened in the editor are parsed on `didOpen`/`didChange` and always in Phase 1.

## Embedded PHP Stubs

PHPantom bundles the [JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) directly into the binary. This provides type information for ~1,450 built-in classes/interfaces, ~5,000 built-in functions, and ~2,000 built-in constants without requiring any external files at runtime.

### Build-Time Processing

The `build.rs` script:

1. Reads `stubs/jetbrains/phpstorm-stubs/PhpStormStubsMap.php` — a generated index mapping symbol names to file paths.
2. Emits `stub_map_generated.rs` containing:
   - `STUB_FILES`: an array of `include_str!(...)` calls embedding every referenced PHP file (~502 files, ~8.5MB of source).
   - `STUB_CLASS_MAP`: maps class/interface/trait names → index into `STUB_FILES`.
   - `STUB_FUNCTION_MAP`: maps function names → index into `STUB_FILES`.
   - `STUB_CONSTANT_MAP`: maps constant names → index into `STUB_FILES`.

The build script watches `composer.lock` for changes, so running `composer update` followed by `cargo build` automatically picks up new stub versions.

### Runtime Lookup — Classes

At `Backend` construction, `stubs.rs` converts the static arrays into `HashMap`s for O(1) lookup. When `find_or_load_class` reaches Phase 3:

1. Look up the class short name in `stub_index` → get the PHP source string.
2. Parse it with the same parser used for user code.
3. Run name resolution (for parent classes, trait uses, etc.).
4. Cache in `ast_map` under `phpantom-stub://ClassName`.
5. Return the `ClassInfo`.

Because stubs are cached after first access, repeated lookups (e.g. every enum needing `UnitEnum`) hit Phase 1 and skip parsing entirely.

### Runtime Lookup — Functions (`find_or_load_function`)

Built-in PHP functions (e.g. `array_map`, `date_create`, `str_contains`) are resolved through `find_or_load_function`, which mirrors the multi-phase pattern used for classes:

```
find_or_load_function(["str_contains", "App\\str_contains"])
│
├── Phase 1: global_functions (user code + cached stubs)
│   Checks all candidate names against the global_functions map.
│   This is where previously-cached stub functions are found on
│   subsequent lookups.
│   ↓ miss
│
├── Phase 2: Embedded PHP stubs
│   Looks up each candidate name in stub_function_index.
│   When found:
│     1. Parses the entire stub file (extracting all FunctionInfo).
│     2. Caches ALL functions from that file into global_functions
│        under phpantom-stub-fn:// URIs.
│     3. Also caches any classes defined in the same stub file into
│        ast_map (so return type references can be resolved).
│     4. Returns the matching FunctionInfo.
│   ↓ miss
│
└── None
```

The `function_loader` closures in both `server.rs` (completion) and `definition/resolve.rs` (go-to-definition) build a list of candidate names — the bare name, the use-map resolved name, and the namespace-qualified name — then delegate to `find_or_load_function`. This means built-in function return types are available for:

- **Variable type resolution**: `$dt = date_create();` → `$dt` is `DateTime`
- **Call chain completion**: `date_create()->` offers `format()`, `modify()`, etc.
- **Nested call resolution**: `simplexml_load_string(...)->xpath(...)` works

User-defined functions in `global_functions` always take precedence over stubs because Phase 1 is checked first — stubs use `entry().or_insert()` to avoid overwriting existing entries.

### Runtime Lookup — Constants

The `stub_constant_index` (`HashMap<&'static str, &'static str>`) is built at construction time from `STUB_CONSTANT_MAP`, mapping constant names like `PHP_EOL`, `PHP_INT_MAX`, `SORT_ASC` to their stub file source. This infrastructure is in place for future use (e.g. constant value/type resolution, completion of standalone constants) but is not yet consulted by any resolution path.

### Graceful Degradation

If the stubs aren't installed (e.g. `composer install` hasn't been run), `build.rs` generates empty arrays and the build succeeds. The LSP just won't know about built-in PHP symbols.

## Inheritance Resolution

When building completion items or resolving definitions, PHPantom merges members from the full inheritance chain via `resolve_class_with_inheritance`:

```
ClassInfo (own members)
│
├── 1. Merge used traits (via `use TraitName;`)
│   └── Recursively follows trait composition and parent_class chains
│
├── 2. Walk the extends chain (parent_class)
│   └── For each parent: merge its traits, then its public/protected members
│
└── 3. Merge @mixin classes (lowest precedence)
    └── Resolved with full inheritance, only public members
```

### Precedence Rules

- **Class own members** always win.
- **Trait members** override inherited members but not own members.
- **Parent members** fill in anything not already present.
- **Mixin members** have the lowest precedence.
- **Private members** are never inherited from parents (but trait private members are copied, matching PHP semantics).

### Interface Inheritance in Traits/Used Interfaces

The `merge_traits_into` function also walks the `parent_class` chain of each trait/interface it loads. This is critical for enums: a backed enum's `used_traits` contains `BackedEnum`, and `BackedEnum extends UnitEnum`. The parent chain walk ensures `UnitEnum`'s members (`cases()`, `$name`) are merged alongside `BackedEnum`'s own members (`from()`, `tryFrom()`, `$value`).

## Implicit Enum Interfaces

PHP enums implicitly implement `UnitEnum` (for unit enums) or `BackedEnum` (for backed enums). The parser detects this and adds the appropriate interface to the enum's `used_traits`:

```
enum Color { ... }           → used_traits: ["\UnitEnum"]
enum Status: int { ... }     → used_traits: ["\BackedEnum"]
```

The leading backslash marks the name as fully-qualified so that namespace resolution doesn't incorrectly prefix it (e.g. an enum in `namespace App\Enums` won't resolve to `App\Enums\UnitEnum`).

At resolution time, `merge_traits_into` loads the `UnitEnum` or `BackedEnum` stub from the embedded phpstorm-stubs, and the interface inheritance chain provides all the standard enum methods and properties.

## Composer Integration

### PSR-4 Autoloading

`composer.rs` parses:

- `composer.json` → `autoload.psr-4` and `autoload-dev.psr-4` mappings
- `vendor/composer/autoload_psr4.php` → vendor package mappings

These mappings are used by Phase 2 of `find_or_load_class` to locate PHP files on disk from fully-qualified class names.

### Autoload Files

`vendor/composer/autoload_files.php` lists files containing global function definitions. These are parsed eagerly during `initialized()` and their functions are stored in `global_functions` for return-type resolution.

### Function Resolution Priority

When resolving a standalone function call (e.g. `app()`, `date_create()`), the lookup order is:

1. **User code** (`global_functions` from Composer autoload files and opened/changed files)
2. **Embedded stubs** (`stub_function_index` from phpstorm-stubs, parsed lazily)

This ensures that user-defined overrides or polyfills always win over built-in stubs.

## Go-to-Implementation: `find_implementors`

When the user invokes go-to-implementation on an interface or abstract class, PHPantom scans for concrete classes that implement or extend it. The scan runs five phases, each progressively wider:

```
find_implementors("Cacheable", "App\\Contracts\\Cacheable")
│
├── Phase 1: ast_map (already-parsed classes)
│   Iterates every ClassInfo in every file already in memory.
│   Checks interfaces list and parent_class chain against the target.
│   ↓ continue
│
├── Phase 2: class_index (FQN → URI entries not yet covered)
│   Loads classes via class_loader for entries not seen in Phase 1.
│   ↓ continue
│
├── Phase 3: classmap files (string pre-filter → parse)
│   Collects unique file paths from the Composer classmap.
│   Skips files already in ast_map.
│   Reads each file's raw source and checks contains(target_short).
│   Only matching files are parsed via parse_and_cache_file.
│   Every class in a parsed file is checked (not just the classmap FQN).
│   ↓ continue
│
├── Phase 4: embedded stubs (string pre-filter → lazy parse)
│   Checks each stub's static source string for contains(target_short).
│   Matching stubs are loaded via class_loader (parsed and cached).
│   ↓ continue
│
├── Phase 5: PSR-4 directory walk (user code only)
│   Recursively collects all .php files under every PSR-4 root.
│   Skips files already covered by the classmap (Phase 3) or ast_map.
│   Reads raw source, applies the same string pre-filter.
│   Matching files are parsed via parse_and_cache_file.
│   Discovers classes in projects without `composer dump-autoload -o`.
│   ↓ done
│
└── Vec<ClassInfo> (concrete implementors only)
```

### Phase 5 Scope: User Code Only (by design)

Phase 5 walks PSR-4 roots from `composer.json` (`autoload` and `autoload-dev`), **not** from `vendor/composer/autoload_psr4.php`. This means it only discovers classes in the user's own source directories (e.g. `src/`, `app/`, `tests/`), not in vendor dependencies.

This is intentional. Vendor dependencies are managed by Composer and don't change during development — they are fully covered by the classmap (`composer dump-autoload -o`). The user's own files, on the other hand, change constantly and may not be in the classmap yet. Phase 5 exists specifically to catch those newly-created or not-yet-indexed user classes.

Do not "fix" this by adding vendor PSR-4 roots to the Phase 5 walk — that would scan tens of thousands of vendor files on every go-to-implementation request for no benefit, since Phase 3 already covers them via the classmap.

### String Pre-Filter

Phases 3–5 avoid expensive parsing by first reading the raw file content and checking whether it contains the target class's short name. A file that doesn't mention `"Cacheable"` anywhere in its source can't possibly implement the `Cacheable` interface, so it's skipped without parsing. This keeps the scan fast even for large projects with thousands of files.

### Caching

`parse_and_cache_file` follows the same pattern as `find_or_load_class`: it parses the PHP file, resolves parent/interface names via `resolve_parent_class_names`, and stores the results in `ast_map`, `use_map`, and `namespace_map`. This means files discovered during a go-to-implementation scan are immediately available for subsequent completion, definition, and implementation lookups without re-parsing.

### Member-Level Implementation

When the cursor is on a method call (e.g. `$repo->find()`), `resolve_member_implementations` first resolves the subject to candidate classes. If any candidate is an interface or abstract class, `find_implementors` is called and each implementor is checked for the specific method. Only classes that directly define (override) the method are returned — inherited-but-not-overridden methods are excluded.

## Union Type Completion (by design)

When a variable can hold one of several types (from match arms, ternary
branches, null-coalescing, or conditional return types), the completion
list shows the **union** of all members across all possible types, not
just the intersection of shared members.

This is a deliberate choice that matches PHPStorm and Intelephense
behaviour. The rationale:

1. **The developer may not have isolated branches yet.** When working
   through a match or ternary, the code often starts with a shared
   variable before the developer splits behaviour per type. Hiding
   branch-specific members would block progress during that phase.
2. **Missing completions are worse than extra completions.** Restricting
   to the intersection would hide useful members whenever a variable has
   more than one possible type.
3. **Type safety belongs in diagnostics.** Calling a method that only
   exists on one branch is a potential bug, but the right place to flag
   it is a diagnostic/static-analysis pass, not the completion list.

This is distinct from narrowing via early return, `unset()`, or
`instanceof` guards. Those reflect deliberate developer intent to
eliminate types, so the narrowed-out members are correctly hidden. Union
completion is about types the developer has *not yet* separated.

Members that are only available on a subset of the union already show the
originating class in the `detail` field (e.g. "Class: AdminUser"), which
gives the developer a visual hint. A future enhancement could sort
intersection members above branch-only members or add an explicit marker
(see todo item 35).

## Context-Aware Class Name Completion

When the user types a class name outside of a member-access chain (`->`, `::`), the LSP needs to decide which symbols to offer. PHP reuses many of the same keywords in positions that accept very different kinds of symbols. `new` only makes sense with concrete classes. `implements` only accepts interfaces. `use` inside a class body only accepts traits. Offering the wrong kind of symbol is noisy at best and confusing at worst. This section explains how the LSP detects the context, selects which symbols to show, and handles the many edge cases that arise from incomplete information.

### Context Detection

`detect_class_name_context()` in `class_completion.rs` walks backward from the cursor through whitespace and comma-separated identifier lists to find the keyword that governs the completion position. It recognises eleven contexts:

| Context | Trigger | What is shown |
|---|---|---|
| `Any` | Bare identifier (e.g. `$x = DateT\|`) | All class-likes, constants, and functions |
| `New` | `new \|` | Concrete (non-abstract) classes only |
| `ExtendsClass` | `class A extends \|` | Non-final classes (abstract is OK) |
| `ExtendsInterface` | `interface B extends \|` | Interfaces only |
| `Implements` | `class C implements \|` | Interfaces only |
| `TraitUse` | `class D { use \|` | Traits only |
| `Instanceof` | `$x instanceof \|` | Classes, interfaces, enums (not traits) |
| `UseImport` | `use \|` (top level) | All class-likes + `function`/`const` keyword hints |
| `UseFunction` | `use function \|` | Functions only |
| `UseConst` | `use const \|` | Constants only |
| `NamespaceDeclaration` | `namespace \|` | Namespace names only |

Comma-separated lists are handled by walking past `Identifier,` sequences so that `implements Foo, Bar, \|` still resolves to `Implements`. Multi-line declarations work the same way because the backward walk skips all whitespace including newlines.

`TraitUse` vs `UseImport` is distinguished by brace depth: `use` at brace depth >= 1 is inside a class body (trait use), while brace depth 0 is a top-level import.

### Handler Routing

`try_class_constant_function_completion()` in `handler.rs` is the entry point. It extracts the partial identifier, detects the context, and branches:

- **`UseFunction`** and **`UseConst`** short-circuit to dedicated builders (`build_function_completions`, `build_constant_completions`). These bypass class-name logic entirely. Items from the current file are filtered out (importing a function from the file you are editing is pointless). A semicolon is appended to the insert text.

- **`NamespaceDeclaration`** short-circuits to `build_namespace_completions`, which produces namespace-path items from PSR-4 prefixes and known class FQNs.

- **`UseImport`** calls `build_class_name_completions` with an **empty use-map** (the file's own use-map contains the half-typed `use` line, which the parser interprets as a real import and would appear as a bogus completion item). The results are post-processed: classes defined in the current file are removed, a semicolon is appended, and `function`/`const` keyword hints are injected when the partial matches.

- **All other class-only contexts** (`New`, `ExtendsClass`, `ExtendsInterface`, `Implements`, `TraitUse`, `Instanceof`) call `build_class_name_completions` normally. Constants and functions are suppressed.

- **`Any`** calls all three builders (classes, constants, functions) and merges the results.

### Class Name Sources and Priority

`build_class_name_completions` collects candidates from five sources in priority order. Earlier sources get better `sort_text` prefixes so the editor ranks them higher. Deduplication is by FQN (`seen_fqns` set).

1. **Use-imported classes** (sort prefix `0_`). The file's `use` map entries. Highest priority because the developer has explicitly imported these names.
2. **Same-namespace classes** (sort prefix `1_`). Classes from the `ast_map` whose file declares the same namespace as the cursor file. These are available without a `use` statement.
3. **Class index** (sort prefix `2_`). Classes discovered during parsing of opened files (`class_index`).
4. **Composer classmap** (sort prefix `3_`). Classes from `vendor/composer/autoload_classmap.php`.
5. **PHP stubs** (sort prefix `4_`). Built-in classes from the embedded phpstorm-stubs.

The result set is capped at 100 items. When truncated, `is_incomplete` is set to `true` so the editor re-requests completions as the user types more characters. Items are sorted by `sort_text` before truncation so higher-priority sources survive.

### Kind Filtering

Each context defines which class-like kinds are valid through `matches_kind_flags(kind, is_abstract, is_final)`. For example, `TraitUse` only matches `ClassLikeKind::Trait`, and `New` only matches non-abstract classes. This filter is applied at every source, but the amount of information available varies by source, which creates a layered filtering strategy.

**Loaded classes** (in `ast_map`). The full `ClassInfo` is available, so the filter is exact. An interface will never appear in `TraitUse` completions if it has been parsed.

**Unloaded stubs** (in `stub_index` but not yet parsed into `ast_map`). The raw PHP source is available. `detect_stub_class_kind` scans it for the declaration keyword (`class`, `interface`, `trait`, `enum`) and modifiers (`abstract`, `final`, `readonly`). This is fast (string search, no tree-sitter parse) and gives accurate kind information for filtering.

**Unloaded project classes** (in `class_index` or `classmap` but not parsed). No kind information is available. These are allowed through the filter with benefit of the doubt. A naming-convention heuristic (`likely_mismatch`) demotes but does not remove suspicious names (e.g. `FooInterface` is demoted in `ExtendsClass` context).

### Use-Map Validation

The file's `use` map is the most problematic source because it can contain entries that are not class-likes at all:

- **Namespace aliases** (`use App\Models as M;` puts `M -> App\Models` in the map, but `App\Models` is a namespace, not a class).
- **Non-existent imports** (the user may have typed `use Vendor\Something;` for a class the LSP has never seen).
- **Half-typed lines** (the parser interprets `use c` as importing `"c"`).

Three filters run on use-map entries, in order:

1. **`is_likely_namespace_not_class`**. Checks all four class sources (ast_map, class_index, classmap, stubs). If the FQN is found in any of them, it is a real class and passes. If not found, the function looks for positive namespace evidence: is the FQN declared as a namespace in `namespace_map`, or do known classes exist under it as a prefix (e.g. `Cassandra\Exception\AlreadyExistsException` proves `Cassandra\Exception` is a namespace)? If positive evidence is found, the entry is rejected. If there is no evidence either way, the entry passes (benefit of the doubt).

2. **`matches_context_or_unloaded`**. If the class is loaded in `ast_map`, applies exact kind filtering. If not loaded but present in `stub_index`, scans the stub source for the declaration keyword. If truly unknown, allows through. This catches stub interfaces appearing in `TraitUse` and similar mismatches.

3. **`is_known_class_like`** (narrow contexts only). For contexts that expect a very specific kind (`TraitUse`, `Implements`, `ExtendsInterface`), an additional check rejects use-map entries whose FQN cannot be found in any class source at all. The reasoning: if we are looking specifically for traits and we have never seen this FQN anywhere, it is almost certainly not a trait. This is deliberately not applied to broader contexts like `New` or `Instanceof`, where an imported-but-not-yet-indexed class should still appear.

The half-typed-line problem is solved at a higher level: the `UseImport` handler passes an empty use-map to `build_class_name_completions`, so the parser's misinterpretation never enters the candidate set.

### UseImport Special Handling

The `UseImport` context (`use |` at top level) has several differences from other contexts:

- **FQN insertion**. The insert text is always the full qualified name, not a short name. Writing `use User;` is invalid PHP even if `User` is in the current namespace. The `is_fqn_prefix` flag is forced to `true` and `effective_namespace` is set to `None` to prevent namespace-relative simplification.

- **No auto-import text edits**. In other contexts, selecting a class from a different namespace generates an `additional_text_edits` entry that inserts a `use` statement. In `UseImport` context this would be circular (you are already writing the `use` statement), so no text edit is generated.

- **Semicolons**. All class items get a `;` appended to the insert text so accepting a completion produces `use App\Models\User;`.

- **Keyword hints**. When the partial matches, `function` and `const` keyword items are injected with a trailing space (not a semicolon) so the user can continue typing the function or constant name.

- **Current-file exclusion**. Classes defined in the cursor file are filtered out. The same applies to `UseFunction` (functions from the current file) and `UseConst` (constants from the current file).

### Unloaded Stub Scanning

Many stubs are never parsed into `ast_map` during a session. Rather than parse thousands of stub files eagerly, the LSP scans the raw PHP source on demand. `detect_stub_class_kind` finds the short name in the source, checks that it is preceded by a declaration keyword (`class`, `interface`, `trait`, `enum`), and extracts `abstract`/`final` modifiers. This gives accurate filtering without the cost of a full parse.

The scan handles PHP 8.2 `readonly` classes (`final readonly class Foo`) by stripping `readonly` before checking for `abstract`/`final`. Multi-class stub files (e.g. V8Js which declares both `V8Js` and `V8JsException`) are handled by searching for each name independently.

### Naming-Convention Heuristics

When a class is not loaded and not a stub (typically a classmap or class_index entry whose file has not been opened), the LSP has no kind information. Rather than guess, it uses naming conventions to adjust sort order:

- Names ending in `Interface` are demoted in `ExtendsClass` and `New`.
- Names ending in `Trait` are demoted in `Implements`, `ExtendsInterface`, and `New`.
- Names starting with `Abstract` are demoted in `New`.

Demotion means a worse sort prefix (`9_` instead of the source's normal prefix), pushing the item to the bottom of the list. The item is never removed, because naming conventions are not reliable enough to exclude candidates entirely.

## Name Resolution

PHP class names go through resolution at parse time (`resolve_parent_class_names`):

1. **Fully-qualified** (`\Foo\Bar`) → strip leading `\`, use as-is.
2. **In use map** (`Bar` with `use Foo\Bar;`) → expand to `Foo\Bar`.
3. **Qualified** (`Sub\Bar` with `use Foo\Sub;`) → expand first segment.
4. **Unqualified, not in use map** → prepend current namespace.
5. **No namespace** → keep as-is.

This runs on `parent_class`, `used_traits`, and `mixins` for every `ClassInfo` extracted from a file.