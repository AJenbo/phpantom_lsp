# PHPantomLSP Architecture

This document explains how PHPantomLSP resolves PHP symbols — classes, interfaces, traits, enums, and functions — across files and from the PHP standard library.

## Overview

PHPantomLSP is a language server that provides completion and go-to-definition for PHP projects. It works by:

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
│   ├── classes.rs          # Class, interface, trait, and enum extraction
│   ├── functions.rs        # Standalone function and define() constant extraction
│   ├── use_statements.rs   # use statement and namespace extraction
│   └── ast_update.rs       # update_ast orchestrator and name resolution helpers
├── docblock/
│   ├── mod.rs              # Re-exports from submodules
│   ├── tags.rs             # PHPDoc tag extraction (@return, @var, @property, @mixin, …)
│   ├── conditional.rs      # PHPStan conditional return type parsing
│   └── types.rs            # Type cleaning utilities (clean_type, strip_nullable, …)
├── completion/
│   ├── mod.rs              # Submodule declarations
│   ├── handler.rs          # Top-level completion request orchestration
│   ├── target.rs           # Extract what the user is completing (subject + access kind)
│   ├── resolver.rs         # Resolve subject → ClassInfo (type resolution engine)
│   ├── builder.rs          # Build LSP CompletionItems from resolved ClassInfo
│   ├── class_completion.rs # Class name, constant, and function completions
│   ├── variable_completion.rs  # Variable name completions and scope collection
│   ├── variable_resolution.rs  # Variable type resolution via assignment scanning
│   ├── closure_resolution.rs   # Closure and arrow-function parameter resolution
│   ├── type_narrowing.rs       # instanceof / assert / custom type guard narrowing
│   ├── conditional_resolution.rs  # PHPStan conditional return type resolution at call sites
│   ├── named_args.rs       # Named argument completion inside function/method call parens
│   ├── phpdoc.rs           # PHPDoc tag completion inside /** … */ blocks
│   └── use_edit.rs         # Use-statement insertion helpers
├── definition/
│   ├── mod.rs              # Submodule declarations
│   ├── resolve.rs          # Core go-to-definition resolution (classes, functions)
│   ├── member.rs           # Member-access resolution (->method, ::$prop, ::CONST)
│   └── variable.rs         # Variable definition resolution ($var jump-to-definition)
build.rs                    # Parses PhpStormStubsMap.php, generates stub index
stubs/                      # Composer vendor dir for jetbrains/phpstorm-stubs
tests/
├── common/mod.rs           # Shared test helpers and minimal PHP stubs
├── completion_*.rs         # Completion integration tests (by feature area)
├── definition_*.rs         # Go-to-definition integration tests
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

PHPantomLSP bundles the [JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) directly into the binary. This provides type information for ~1,450 built-in classes/interfaces, ~5,000 built-in functions, and ~2,000 built-in constants without requiring any external files at runtime.

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

When building completion items or resolving definitions, PHPantomLSP merges members from the full inheritance chain via `resolve_class_with_inheritance`:

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

## Name Resolution

PHP class names go through resolution at parse time (`resolve_parent_class_names`):

1. **Fully-qualified** (`\Foo\Bar`) → strip leading `\`, use as-is.
2. **In use map** (`Bar` with `use Foo\Bar;`) → expand to `Foo\Bar`.
3. **Qualified** (`Sub\Bar` with `use Foo\Sub;`) → expand first segment.
4. **Unqualified, not in use map** → prepend current namespace.
5. **No namespace** → keep as-is.

This runs on `parent_class`, `used_traits`, and `mixins` for every `ClassInfo` extracted from a file.