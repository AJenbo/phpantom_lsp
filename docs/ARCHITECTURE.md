# PHPantom Architecture

This document explains how PHPantom resolves PHP symbols — classes, interfaces, traits, enums, and functions — across files and from the PHP standard library.

## Overview

PHPantom is a language server for PHP projects. It provides completion, go-to-definition, go-to-implementation, find references, hover, signature help, and diagnostics. It works by:

1. **Parsing** PHP files into lightweight `ClassInfo` / `FunctionInfo` structures (not a full AST — just the information needed for IDE features).
2. **Caching** parsed results in an in-memory `ast_map` keyed by file URI.
3. **Building** a precomputed symbol map (`symbol_maps`) during parsing for O(log n) go-to-definition lookups and call-site detection for signature help.
4. **Resolving** symbols on demand through a multi-phase lookup chain.
5. **Merging** inherited members (from parent classes, traits, interfaces, and mixins) at resolution time.

### Multi-file scanning

Most features (completion, go-to-definition, hover, signature help, diagnostics) use maps or known file names and never walk directories. Only two features do multi-file scanning:

- **Go-to-implementation** scans for concrete classes that implement an interface or extend an abstract class (see `find_implementors`). Walks PSR-4 source directories only.
- **Find References** scans for all occurrences of a symbol across the project (see `ensure_workspace_indexed`). Walks the entire workspace root.

Both features follow the same principle for vendor code: the Composer classmap is the source of truth. Vendor directories are never walked. User PSR-4 roots from `composer.json` are walked because user files change between `dump-autoload` runs.

The two walkers differ in scope because GTI only needs class declarations (which live in PSR-4 roots), while Find References needs any usage of a symbol, which could be in a standalone script, config file, or `index.php` at the project root.

## Design Philosophy

PHPantom is built in layers. Each layer is independently useful and independently testable.

- **Layer 0: Stubs.** Embedded PHP standard library types. Available immediately, no project context needed.
- **Layer 1: Single file.** Parse the open file, extract classes/functions/symbols. Completion, hover, and go-to-definition work within the file with no cross-file resolution at all.
- **Layer 2: On-demand resolution.** When a symbol references a class in another file, resolve it through the classmap or PSR-4 and parse that file. Only the files actually needed are touched.
- **Layer 3: Classmap.** A name-to-path index covering the whole project. Enables class name completion and O(1) cross-file lookup. Built from Composer's output or self-generated via a fast byte-level scan.
- **Layer 4: Full index (opt-in).** Background-parse every file in the classmap. Enables workspace symbols, fast find-references, and rich completion item detail.

Each layer builds on the one below it. A bug in classmap generation doesn't break single-file completion. A slow full index doesn't block on-demand resolution. New features can be developed and tested against the lower layers without waiting for a full project scan. This is also why PHPantom starts fast: Layer 0-2 are ready in milliseconds, Layer 3 takes seconds, and Layer 4 (when enabled) fills in over the following minute.

## Module Layout

```
src/
├── lib.rs                  # Backend struct, state, constructors
├── main.rs                 # Entry point (stdin/stdout LSP transport)
├── server.rs               # LSP protocol handlers (initialize, didOpen, completion, …)
├── types.rs                # Data structures (ClassInfo, MethodInfo, SubjectExpr, ResolvedCallableTarget, …)
├── composer.rs             # composer.json / PSR-4 autoload parsing
├── stubs.rs                # Embedded phpstorm-stubs (build-time generated index)
├── resolution.rs           # Multi-phase class/function lookup and name resolution
├── inheritance.rs          # Base class inheritance merging (traits, parent chain)
├── symbol_map/
│   ├── mod.rs              # Data structures (SymbolSpan, SymbolKind, VarDefSite, CallSite, SymbolMap) and impl
│   ├── docblock.rs         # Docblock symbol extraction (type span emission, @template/@method tag scanning, navigability filter)
│   └── extraction.rs       # AST walk that builds a SymbolMap (extract_symbol_map and all extract_from_* helpers)
├── virtual_members/
│   ├── mod.rs              # VirtualMemberProvider trait, VirtualMembers struct, merge logic
│   ├── laravel.rs          # LaravelModelProvider (relationships, scopes, casts, accessors)
│   └── phpdoc.rs           # PHPDocProvider (@method, @property, @property-read, @property-write, @mixin)
├── subject_extraction.rs   # Shared helpers for extracting subjects before ->, ?->, ::
├── util.rs                 # Position conversion, class lookup, logging, directory walkers (collect_php_files, collect_php_files_gitignore)
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
│   ├── mod.rs              # Submodule declarations + backward-compatible re-exports
│   ├── handler.rs          # Top-level completion request orchestration
│   ├── target.rs           # Extract what the user is completing (subject + access kind)
│   ├── resolver.rs         # Resolve subject → ClassInfo (type resolution engine), shared resolve_callable_target
│   ├── call_resolution.rs  # Call expression and callable target resolution
│   ├── builder.rs          # Build LSP CompletionItems from resolved ClassInfo
│   ├── array_shape.rs      # Array shape key completion and raw variable type resolution
│   ├── named_args.rs       # Named argument completion inside function/method call parens
│   ├── use_edit.rs         # Use-statement insertion helpers
│   ├── variable/           # Variable resolution
│   │   ├── resolution.rs       # Variable type resolution via assignment scanning
│   │   ├── completion.rs       # Variable name completions and scope collection
│   │   ├── rhs_resolution.rs   # Right-hand-side expression resolution
│   │   ├── class_string_resolution.rs  # Class-string variable resolution ($cls = User::class)
│   │   ├── raw_type_inference.rs   # Raw type inference (array shapes, array functions, generators)
│   │   ├── foreach_resolution.rs   # Foreach value/key and array destructuring type resolution
│   │   └── closure_resolution.rs   # Closure and arrow-function parameter resolution
│   ├── types/              # Type resolution
│   │   ├── resolution.rs       # Type-hint string → ClassInfo mapping (unions, generics, aliases)
│   │   ├── narrowing.rs        # instanceof / assert / custom type guard narrowing
│   │   └── conditional.rs      # PHPStan conditional return type resolution at call sites
│   ├── context/            # Context-specific completion
│   │   ├── catch_completion.rs # Smart exception type completion inside catch() clauses
│   │   ├── class_completion.rs # Class name completions (class, interface, trait, enum)
│   │   ├── constant_completion.rs  # Global constant name completions
│   │   ├── function_completion.rs  # Standalone function name completions
│   │   ├── namespace_completion.rs # Namespace declaration completions
│   │   └── type_hint_completion.rs # Type completion in parameter lists, return types, properties
│   ├── phpdoc/             # PHPDoc completion
│   │   ├── mod.rs              # PHPDoc tag completion inside /** … */ blocks
│   │   └── context.rs          # PHPDoc context detection and symbol info extraction
│   └── source/             # Source analysis
│       ├── comment_position.rs # Comment and docblock position detection
│       ├── helpers.rs          # Source-text scanning helpers (closure/callable return types, new-expression parsing)
│       └── throws_analysis.rs  # Shared throw-statement scanning and @throws tag lookup
├── signature_help.rs       # Signature help: parameter hints inside function/method call parens
├── hover/
│   └── mod.rs              # Hover handler: symbol-map dispatch, type/signature/docblock formatting
├── definition/
│   ├── mod.rs              # Submodule declarations
│   ├── resolve.rs          # Core go-to-definition: symbol-map dispatch + text-based fallback
│   ├── member.rs           # Member-access resolution (->method, ::$prop, ::CONST) with stored offsets
│   ├── variable/
│   │   ├── mod.rs          # VarDefSearchResult enum, Backend methods, tests
│   │   ├── var_definition.rs # AST walk finding variable definition sites
│   │   └── type_hint.rs    # AST walk extracting type hints at definition sites
│   └── implementation.rs   # Go-to-implementation (interface/abstract → concrete classes)
├── references/
│   ├── mod.rs              # Find References handler: same-file and cross-file symbol scanning
│   └── tests.rs            # Unit tests for find-references
├── highlight/
│   └── mod.rs              # Document highlighting: same-file symbol occurrence highlighting
├── diagnostics/
│   ├── mod.rs              # Diagnostic collection and publishing (skips vendor files)
│   ├── deprecated.rs       # @deprecated usage diagnostics (strikethrough)
│   └── unused_imports.rs   # Unused use-statement dimming
build.rs                    # Parses PhpStormStubsMap.php, generates stub index
stubs/                      # Composer vendor dir for jetbrains/phpstorm-stubs
tests/
├── common/mod.rs           # Shared test helpers and minimal PHP stubs
├── completion_*.rs         # Completion integration tests (by feature area)
├── definition_*.rs         # Go-to-definition integration tests
├── hover.rs                # Hover integration tests
├── signature_help.rs       # Signature help integration tests
├── implementation.rs       # Go-to-implementation integration tests
├── document_highlight.rs   # Document highlighting integration tests
├── docblock_*.rs           # Docblock parsing and type tests
├── parser.rs               # PHP parser / AST extraction tests
├── composer.rs             # Composer integration tests
└── …
```

## Go-to-Definition Architecture

The definition subsystem uses a **three-tier** approach, from fastest to slowest:

### Tier 1: Precomputed Symbol Map (primary)

During `update_ast`, every navigable symbol occurrence in a file is recorded as a `SymbolSpan` in a sorted vec stored in `symbol_maps` (keyed by file URI). Each span records the byte range and a `SymbolKind` variant:

| `SymbolKind` | What it captures |
|---|---|
| `ClassReference` | Class/interface/trait/enum names in type contexts (`new Foo`, `extends`, `implements`, type hints, catch, docblock types) |
| `ClassDeclaration` | Class name at its declaration site (cursor already at definition) |
| `MemberAccess` | `->`, `?->`, `::` member names with the subject text and static/method-call flags |
| `Variable` | `$variable` tokens (both usage and definition sites) |
| `FunctionCall` | Standalone function call names |
| `SelfStaticParent` | `self`, `static`, `parent` keywords in navigable contexts |
| `ConstantReference` | Constant names (`define()` name, class constant access, standalone constant reference) |
| `MemberDeclaration` | Method, property, or constant name at its declaration site (not navigable for go-to-definition, but needed for find-references) |

When a go-to-definition request arrives, `resolve_definition` converts the cursor position to a byte offset and does a binary search on the symbol map. If a `SymbolSpan` is found, it dispatches directly to the appropriate resolution path — no text scanning needed. If the offset falls in a gap (whitespace, string interior, comment interior, etc.), the request is instantly rejected.

Docblock type references (`@param`, `@return`, `@var`, `@template`, `@method`, etc.) are extracted by a dedicated string scanner during the AST walk, since docblocks are trivia in the `mago_syntax` AST and produce no expression/statement nodes. These use the same `ClassReference` kind and resolution path as code type hints.

The symbol map also stores:

- **Variable definition sites** (`var_defs`): records every assignment, parameter, foreach binding, catch variable, static/global declaration, and destructuring site with its byte offset, scope boundary, and `effective_from` offset (for assignments, this is the end of the statement so the RHS sees the previous definition). Go-to-definition for `$var` finds the most recent definition before the cursor within the enclosing scope via binary search.
- **Scope boundaries** (`scopes`): function, method, closure, and arrow function body ranges. Used by `find_enclosing_scope` to determine which scope the cursor is in.
- **Template parameter definitions** (`template_defs`): `@template` tag locations so that template parameter names (e.g. `TKey`, `TModel`) that appear in docblock types can be resolved to their declaration site.

### Tier 2: Stored Byte Offsets (cross-file jumps)

`MethodInfo`, `PropertyInfo`, `ConstantInfo`, and `FunctionInfo` each carry a `name_offset: u32` field recording the byte offset of the member's name token in the source file. `ClassInfo` carries `keyword_offset: u32` for the `class`/`interface`/`trait`/`enum` keyword. These are populated during parsing.

When a cross-file jump lands on a class or member, `find_member_position` and `find_definition_position` convert the stored offset directly to an LSP `Position` via `offset_to_position` — no line-by-line text scanning needed.

A value of `0` means "not available" (stubs parsed before offsets were stored, synthetic/virtual members). In that case, the system falls back to Tier 3.

### Tier 3: Docblock Tag Scanning (virtual members only)

The only remaining line-by-line text scanner is in `find_member_position` (`definition/member/file_lookup.rs`). When a member has no stored AST byte offset (`name_offset` is `None` or `0`), the function scans docblock comments for `@property` / `@property-read` / `@property-write` and `@method` tags that declare the virtual member. This is inherent to virtual members: they exist only as docblock annotations and have no AST node to store an offset for.

All cursor context detection (word extraction, member access detection, type-hint resolution) now relies exclusively on the precomputed `SymbolMap` and AST walks. The former text-based helpers `extract_word_at_position`, `extract_member_access_context`, and `resolve_type_hint_at_variable_text` have been removed.

### Variable Definition Resolution

Variable go-to-definition (`$var` → jump to definition) uses two layers:

1. **Symbol map** (`var_defs`): the primary path. Finds the most recent `VarDefSite` before the cursor within the enclosing scope. When the cursor is physically on a definition token (parameter, foreach binding, catch variable), it returns `None` so the caller can fall through to type-hint resolution.
2. **AST-based search** (`resolve_variable_definition_ast`): parses the file and walks the enclosing scope to find the definition site. Handles destructuring (`[$a, $b] = ...`, `list($a, $b) = ...`) and nested scopes correctly. Used as a fallback when the symbol map doesn't have a match. Returns `None` when the AST parse fails.

## Signature Help Architecture

Signature help shows parameter hints when the cursor is inside the parentheses
of a function or method call. Detection uses a **two-tier** strategy:

### Tier 1: Precomputed Call Sites (primary)

During `update_ast`, every call expression in the file is recorded as a
`CallSite` in the `SymbolMap`. Each entry stores:

| Field | Purpose |
|---|---|
| `args_start` | Byte offset immediately after the opening `(` |
| `args_end` | Byte offset of the closing `)` |
| `call_expression` | The call target in `resolve_callable` format (see below) |
| `comma_offsets` | Byte offsets of each top-level comma separator |

Call sites are emitted for all five AST call kinds: `Call::Function`,
`Call::Method`, `Call::NullSafeMethod`, `Call::StaticMethod`, and
`Expression::Instantiation`. The `call_expression` string is built by
`expr_to_subject_text`, which recursively converts the AST expression into
the text format the resolver expects:

- `"functionName"` for standalone function calls
- `"$subject->method"` for instance and null-safe method calls
- `"ClassName::method"` for static method calls
- `"new ClassName"` for constructor calls

When a signature help request arrives, `detect_call_site_from_map` converts
the LSP position to a byte offset and searches the `call_sites` vec in
reverse for the innermost entry whose argument range contains the cursor.
The active parameter index is computed by counting how many precomputed
comma offsets fall before the cursor. This handles nesting, strings
containing commas/parens, and arbitrary chain depth correctly because the
offsets come from the parser's token stream.

### Tier 2: Text-Based Fallback

When the symbol map has no matching call site (typically because the parser
could not recover the call node from incomplete code, e.g. an unclosed `(`
while the user is typing), the text-based scanner
`detect_call_site_text_fallback` fires. It walks backward from the cursor
to find an unmatched `(`, extracts the call expression with simple
character-level scanning, and counts top-level commas. This path handles
simple calls reliably but cannot resolve property chains, method return
chains, or expressions containing balanced parentheses.

If the text fallback also fails to resolve the callable (e.g. because the
class context is missing from the broken AST), a content-patching strategy
inserts `);` at the cursor position and re-parses the file to recover
class context for resolution.

### Call Expression Resolution

`resolve_callable` dispatches on the call expression format:

1. **`new ClassName`** — loads the class via `class_loader`, finds
   `__construct`. Returns an empty parameter list when no constructor
   is defined.
2. **`$subject->method`** — splits at the last `->`, resolves the subject
   via `resolve_target_classes` (which handles `$this`, variables,
   property chains, call return chains, array access, etc.), then looks
   up the method on the resolved class.
3. **`ClassName::method`** — resolves `self`/`static`/`parent` keywords
   and bare class names via `class_loader`, with a fallback to
   `resolve_target_classes` for class-string variables
   (e.g. `$cls = Pen::class; $cls::make()`).
4. **`functionName`** — resolves via `resolve_function_name` (use map,
   namespace, stubs). Falls back to first-class callable resolution:
   if the expression is a `$variable`, scans backward for
   `$var = target(...)` and recursively resolves the underlying target.

## Symbol Resolution: `find_or_load_class`

When the LSP needs to resolve a class name (e.g. during completion on `Iterator::` or when following a type hint), it calls `find_or_load_class`. This method tries four phases in order, returning as soon as one succeeds:

```
find_or_load_class("Iterator")
│
├── Phase 1: ast_map scan
│   Searches all already-parsed files by short class name + namespace.
│   This is where cached results from previous phases are found on
│   subsequent lookups — a classmap file parsed in Phase 1.5 or a
│   stub parsed in Phase 3 is cached here and found in Phase 1
│   next time.
│   ↓ miss
│
├── Phase 1.5: Composer classmap (FQN → file path)
│   Direct hash lookup in the classmap parsed from
│   vendor/composer/autoload_classmap.php.  More targeted than PSR-4
│   and covers classes that don't follow PSR-4 conventions.  When the
│   user runs `composer dump-autoload -o`, *all* classes (including
│   vendor) end up in the classmap, giving complete coverage.
│   ↓ miss
│
├── Phase 2: PSR-4 resolution
│   Uses PSR-4 mappings from composer.json to locate the file on disk.
│   These mappings only cover user code (vendor PSR-4 is not loaded).
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

- Phase 1.5 (classmap) files are parsed once, then found via Phase 1.
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
│   Keys are always FQNs: "array_map" for global functions,
│   "Illuminate\Support\enum_value" for namespaced ones.
│   No short-name fallback entries are stored.
│   ↓ miss
│
├── Phase 2: Embedded PHP stubs
│   Looks up each candidate name in stub_function_index.
│   When found:
│     1. Parses the entire stub file (extracting all FunctionInfo).
│     2. Caches ALL functions from that file into global_functions
│        under phpantom-stub-fn:// URIs (FQN keys only).
│     3. Also caches any classes defined in the same stub file into
│        ast_map (so return type references can be resolved).
│     4. Returns the matching FunctionInfo.
│   ↓ miss
│
└── None
```

The `function_loader` closures in both `server.rs` (completion) and `definition/resolve.rs` (go-to-definition) build a list of candidate names — the bare name, the use-map resolved name, and the namespace-qualified name — then delegate to `find_or_load_function`. Because `global_functions` is keyed by FQN only, bare calls like `enum_value()` in namespace `App` succeed via the namespace-qualified candidate `App\enum_value`. This means built-in function return types are available for:

- **Variable type resolution**: `$dt = date_create();` → `$dt` is `DateTime`
- **Call chain completion**: `date_create()->` offers `format()`, `modify()`, etc.
- **Nested call resolution**: `simplexml_load_string(...)->xpath(...)` works

User-defined functions in `global_functions` always take precedence over stubs because Phase 1 is checked first — stubs use `entry().or_insert()` to avoid overwriting existing entries.

#### Function Completion: Namespace Awareness

`build_function_completions` handles two modes:

- **`use function` context** (`for_use_import = true`): The insert text is the FQN (e.g. `Illuminate\Support\enum_value`) so the resulting statement reads `use function Illuminate\Support\enum_value;`. The label is the FQN for namespaced functions and the signature for global ones.

- **Inline context** (`for_use_import = false`): The insert text is a snippet using the short name (e.g. `enum_value($0)`). For namespaced functions, an `additional_text_edits` entry inserts `use function FQN;` at the alphabetically correct position, mirroring how class auto-import works. The detail shows the namespace (e.g. `function (Illuminate\Support)`). Functions in the same namespace as the current file do not receive an auto-import.

Deduplication uses the map key (FQN), so two functions with the same short name in different namespaces both appear as separate completion items.

### Runtime Lookup — Constants

The `stub_constant_index` (`HashMap<&'static str, &'static str>`) is built at construction time from `STUB_CONSTANT_MAP`, mapping constant names like `PHP_EOL`, `PHP_INT_MAX`, `SORT_ASC` to their stub file source. This infrastructure is in place for future use (e.g. constant value/type resolution, completion of standalone constants) but is not yet consulted by any resolution path.

### Version-Aware Filtering

phpstorm-stubs annotate functions, methods, and parameters with `#[PhpStormStubsElementAvailable]` attributes to indicate which PHP versions they apply to. For example, `array_map` has two variants of its second parameter: `array $array` (from PHP 8.0) and an untyped `$arrays` (PHP 5.3 to 7.4). Without filtering, both appear in signatures.

PHPantom detects the target PHP version during `initialized`:

1. Read `config.platform.php` from `composer.json` (explicit platform override).
2. Fall back to `require.php` from `composer.json` (e.g. `"^8.4"` → 8.4).
3. Default to PHP 8.5 when neither is available.

The detected version is stored on the `Backend` as a `PhpVersion(major, minor)`.

When parsing stubs (both class stubs via `find_or_load_class` and function stubs via `find_or_load_function`), the PHP version is passed through `DocblockCtx.php_version` to the extraction functions. Three filtering points apply:

- **Function-level:** `extract_functions_from_statements` checks the function's `#[PhpStormStubsElementAvailable]` attribute. If the version range excludes the target, the entire function is skipped. This handles duplicate function definitions (e.g. `array_combine` has separate signatures for PHP ≤7.4 and ≥8.0).
- **Method-level:** `extract_class_like_members` applies the same check to methods. For example, `SplFixedArray::__serialize` (from PHP 8.2) is excluded when targeting PHP 8.1.
- **Parameter-level:** `extract_parameters` filters individual parameters. For example, `array_map`'s untyped `$arrays` parameter (PHP 5.3–7.4) is excluded when targeting PHP 8.0+, leaving only the typed `array $array`.

The attribute supports named arguments (`from: '8.0'`, `to: '7.4'`) and a positional argument (`'8.1'` treated as `from`). Both bounds are inclusive. A missing bound means unbounded in that direction.

User code is never filtered. The `php_version` field in `DocblockCtx` is `None` for files parsed via `update_ast`, so the filtering logic is a no-op for non-stub code.

### Graceful Degradation

If the stubs aren't installed (e.g. `composer install` hasn't been run), `build.rs` generates empty arrays and the build succeeds. The LSP just won't know about built-in PHP symbols.

## Inheritance Resolution

When building completion items or resolving definitions, PHPantom merges members from the full inheritance chain. Resolution proceeds in two phases:

1. **Base resolution** (`resolve_class_with_inheritance` in `inheritance.rs`): merges own members, trait members, and parent chain members with generic type substitution.

2. **Virtual member providers** (`resolve_class_fully` in `virtual_members/mod.rs`): queries registered providers in priority order and merges their contributions. Virtual members never overwrite real declared members or contributions from higher-priority providers.

All completion and go-to-definition call sites use `resolve_class_fully`, which is the primary entry point for full resolution.

### Base Resolution

```
ClassInfo (own members)
│
├── 1. Merge used traits (via `use TraitName;`)
│   └── Recursively follows trait composition and parent_class chains
│
└── 2. Walk the extends chain (parent_class)
    └── For each parent: merge its traits, then its public/protected members
```

### Virtual Member Providers

After base resolution, `resolve_class_fully` applies registered virtual member providers. These are implementations of the `VirtualMemberProvider` trait (defined in `src/virtual_members/mod.rs`) that synthesize methods and properties not present in the PHP source code.

Each provider implements two methods:

- `applies_to(class, class_loader) -> bool`: cheap pre-check to skip providers early.
- `provide(class, class_loader) -> VirtualMembers`: produce the virtual methods and properties.

Providers receive the base-resolved class (own + traits + parents) but without other providers' contributions. This prevents circular loading.

```
resolve_class_fully(class)
│
├── 1. resolve_class_with_inheritance(class)
│   └── Returns base-resolved ClassInfo
│
└── 2. For each provider (in priority order):
    └── if applies_to(class): merge provide(class) into result
        └── Skips members that already exist (no overwrites)
```

Provider priority order (highest first):

1. **Framework provider** (e.g. Laravel): richest type info
2. **PHPDoc provider**: `@method`, `@property`, `@property-read`, `@property-write`, `@mixin`

Two providers are currently registered in `default_providers()`:

- **`LaravelModelProvider`** (`virtual_members/laravel.rs`): synthesizes virtual members for classes extending `Illuminate\Database\Eloquent\Model`. Produces relationship properties (methods returning `HasMany`, `HasOne`, `BelongsTo`, etc. generate a virtual property typed from the relationship's generic parameters), scope methods (both the `scopeActive` naming convention and the `#[Scope]` attribute from Laravel 11+ are supported; either style becomes `active()` as both static and instance), Builder-as-static forwarding (`User::where()->get()` resolves end-to-end), accessors (legacy `getXAttribute()` and modern `Attribute` casts), and cast properties (`$casts` array or `casts()` method entries are mapped to PHP types like `datetime` to `\Carbon\Carbon`, `boolean` to `bool`, custom cast classes to their `get()` return type). Highest priority among virtual member providers. Scope methods are also injected onto `Builder<Model>` instances via a post-generic-substitution hook in `type_hint_to_classes_depth` (see "Scope Methods on Builder Instances" below).
- **`PHPDocProvider`** (`virtual_members/phpdoc.rs`): parses `@method`, `@property`, `@property-read`, `@property-write`, and `@mixin` tags from the class-level docblock stored in `ClassInfo.class_docblock`. Explicit `@method` / `@property` tags are not parsed eagerly during AST extraction; instead, the raw docblock string is preserved and parsed lazily when `provide` is called. For `@mixin` tags, the provider loads the referenced classes and merges their public members. Within the provider, explicit tags take precedence over mixin members. Recurses into mixin-of-mixin chains up to `MAX_MIXIN_DEPTH`.

### Precedence Rules

- **Class own members** always win.
- **Trait members** override inherited members but not own members.
- **Parent members** fill in anything not already present.
- **Virtual members** from providers sit below all real declared members (own, trait, parent). Higher-priority providers shadow lower-priority ones. Within the PHPDoc provider, `@method` / `@property` tags beat `@mixin` members.
- **Private members** are never inherited from parents (but trait private members are copied, matching PHP semantics).

### Scope Methods on Builder Instances

Scope methods (both `scopeX` convention and `#[Scope]` attribute) are synthesized on the Model class by `LaravelModelProvider`, but they also need to be available on `Builder<Model>` instances (e.g. `Brand::where('id', $id)->isActive()`). This cannot be handled by a virtual member provider alone because providers run before generic argument substitution, so the provider would not know the concrete model type.

Instead, scope injection happens in three places:

1. **Post-generic-substitution hook** (`completion/resolver.rs`, inside `type_hint_to_classes_depth`): after `resolve_class_fully` + `apply_generic_args` produces a `Builder<User>` class, the resolver detects that the result is an Eloquent Builder with a concrete model generic argument. It calls `build_scope_methods_for_builder(model_name, class_loader)` which loads the model, fully resolves it, extracts scope methods, and returns them as instance methods with `static` in return types substituted to the concrete model name. These are merged onto the Builder's method list, giving `$q->active()` after `$q = User::where(...)`.

2. **Scope body Builder enrichment** (`completion/variable_resolution.rs`, `enrich_builder_type_in_scope`): inside a scope method body, the `$query` parameter is typically typed as `Builder` without generic arguments. The enrichment function detects when the enclosing method is a scope (either the `scopeX` naming convention or the `#[Scope]` attribute) on a class that extends Eloquent Model, and the parameter type is `Builder` without generics. It rewrites the type to `Builder<EnclosingModel>`, which then flows through the generic-args path and triggers the post-substitution hook above. This makes `$query->otherScope()` resolve inside scope bodies.

3. **Go-to-definition fallback** (`definition/member.rs`, `find_scope_on_builder_model`): when go-to-definition resolves a member on a Builder class and the normal lookup chain (own members, traits, parents, mixins, builder forwarding) fails, this fallback checks whether the member is a scope method injected from the model. It confirms the resolved candidate (with injected scopes) has the method, extracts the model name from the scope method's return type generic argument, loads the model, and finds the declaration through the model's inheritance chain. For `scopeX`-style scopes it looks for `scopeXxx`; for `#[Scope]`-attributed methods it falls back to the original method name. This bridges the gap between completion (which works on the merged ClassInfo) and GTD (which traces back to the declaring source file).

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

PSR-4 mappings come exclusively from the project's own `composer.json`. Vendor PSR-4 (`vendor/composer/autoload_psr4.php`) is not loaded. The Composer classmap is the sole source of truth for vendor code.

**Design principle:** if the classmap is missing or stale, vendor classes fail to resolve visibly rather than being silently papered over by PSR-4. This makes the problem obvious to the user (fix: run `composer dump-autoload`). User PSR-4 roots are walked by Go-to-implementation (Phase 5) and Find References because user files change between `dump-autoload` runs.

### Self-Generated Classmap

When Composer's `autoload_classmap.php` is missing or incomplete, PHPantom builds a classmap itself using a fast byte-level PHP scanner (`classmap_scanner.rs`). The scanner is a single-pass state machine that extracts `namespace\ClassName` pairs without a full AST parse, handling comments, strings, heredocs, and property accesses (`$node->class`). `memchr` provides SIMD-accelerated keyword pre-screening so files without class-like keywords are rejected immediately.

The indexing strategy is configurable via `[indexing] strategy` in `.phpantom.toml`:

- **`"composer"`** (default) — use Composer's classmap when available and complete; fall back to self-scan when it is missing or incomplete (i.e. the project's own PSR-4 namespaces have no entries in the classmap).
- **`"self"`** — always self-scan, ignoring Composer's classmap entirely.
- **`"full"`** — same as `"self"` for now; reserved for future background indexing.
- **`"none"`** — no proactive scanning; uses Composer's classmap if present but never falls back to self-scan.

When self-scanning with a `composer.json` present, the scanner reads `autoload.psr-4`, `autoload-dev.psr-4`, `autoload.classmap`, and `autoload-dev.classmap` to determine which directories to walk. PSR-4 directories are filtered: only classes whose FQN matches the namespace prefix plus the relative file path are included. Vendor packages are discovered from `vendor/composer/installed.json` (both Composer 1 and 2 formats). When no `composer.json` exists at all, the scanner falls back to walking all `.php` files under the workspace root (excluding hidden directories, `node_modules`, `vendor`, and `target`).

The result is a `HashMap<String, PathBuf>` in the same format as the existing `Backend.classmap`. Everything downstream (resolution, diagnostics, go-to-definition) works unchanged.

**Vendor dir detection:** the `config.vendor-dir` setting is read from `composer.json` once during `initialized` (via `parse_composer_json`, which returns both the PSR-4 mappings and the vendor dir name). The vendor dir name is cached on `Backend.vendor_dir_name` and a `file://` URI prefix is stored in `Backend.vendor_uri_prefix` for fast vendor-file detection at runtime.

### Autoload Files

`vendor/composer/autoload_files.php` lists files containing global function definitions. These are parsed eagerly during `initialized()` and their functions are stored in `global_functions` for return-type resolution.

### Function Resolution Priority

When resolving a standalone function call (e.g. `app()`, `date_create()`), the lookup order is:

1. **User code** (`global_functions` from Composer autoload files and opened/changed files)
2. **Embedded stubs** (`stub_function_index` from phpstorm-stubs, parsed lazily)

This ensures that user-defined overrides or polyfills always win over built-in stubs.

## Go-to-Implementation: `find_implementors`

When the user invokes go-to-implementation on an interface or abstract class, PHPantom scans for concrete classes that implement or extend it. When invoked on a method definition in a concrete class, it performs a reverse jump to the interface or abstract method that declares the prototype. The forward scan runs five phases, each progressively wider:

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

Phase 5 walks PSR-4 roots from `composer.json` (`autoload` and `autoload-dev`). Since PSR-4 mappings are sourced exclusively from the project's own `composer.json` (vendor PSR-4 is not loaded), Phase 5 inherently only discovers classes in the user's own source directories (e.g. `src/`, `app/`, `tests/`). Vendor dependencies are fully covered by the classmap (Phase 3).

Phase 5 exists to catch newly-created or not-yet-indexed user classes that are missing from the classmap (e.g. the user hasn't run `composer dump-autoload -o`).

Note: `collect_php_files` still receives the vendor dir name because a fallback mapping like `"" => "."` resolves to the workspace root, where the walk must skip the vendor directory (and hidden directories like `.git`).

### String Pre-Filter

Phases 3–5 avoid expensive parsing by first reading the raw file content and checking whether it contains the target class's short name. A file that doesn't mention `"Cacheable"` anywhere in its source can't possibly implement the `Cacheable` interface, so it's skipped without parsing. This keeps the scan fast even for large projects with thousands of files.

### Caching

`parse_and_cache_file` follows the same pattern as `find_or_load_class`: it parses the PHP file, resolves parent/interface names via `resolve_parent_class_names`, and stores the results in `ast_map`, `use_map`, and `namespace_map`. This means files discovered during a go-to-implementation scan are immediately available for subsequent completion, definition, and implementation lookups without re-parsing.

### Member-Level Implementation

When the cursor is on a method call (e.g. `$repo->find()`), `resolve_member_implementations` first resolves the subject to candidate classes. If any candidate is an interface or abstract class, `find_implementors` is called and each implementor is checked for the specific method. Only classes that directly define (override) the method are returned — inherited-but-not-overridden methods are excluded.

### Reverse Jump: Concrete Method → Prototype Declaration

When go-to-implementation is invoked on a `MemberDeclaration` symbol (a method name at its definition site), `resolve_reverse_implementation` checks whether the enclosing class implements any interface or extends an abstract class that declares a method with the same name. If found, it returns the location of the prototype method in the interface or abstract class. This covers:

- Direct interfaces (`class Foo implements Bar`)
- Interfaces inherited from parent classes (`class Foo extends Base` where `Base implements Bar`)
- Interface-extends chains (`interface A extends B`)
- Abstract parent methods (`abstract class Base { abstract function foo(); }`)

For interface and abstract class method declarations, the same handler works in the forward direction: it calls `resolve_interface_member_implementations` to find all concrete classes that define the method.

### FQN-Based Comparison

All comparisons in `class_implements_or_extends` and deduplication in `find_implementors` use fully-qualified names (built from `ClassInfo.name` + `ClassInfo.file_namespace`). The short-name fallback is only used when the target has no namespace information, preventing false positives when two interfaces in different namespaces share the same short name (e.g. `App\Logger` vs `Vendor\Logger`).

## Find References: `ensure_workspace_indexed`

When the user invokes "Find All References", PHPantom scans all user files for occurrences of the symbol. Vendor files are excluded (matching PhpStorm's behaviour).

### Indexing

Before scanning, `ensure_workspace_indexed` ensures all user files have symbol maps:

1. **Phase 1: class_index files (user only)** — files already known from `update_ast` calls. Vendor and stub URIs are skipped.
2. **Phase 2: `.gitignore`-aware workspace walk** — uses the `ignore` crate's `WalkBuilder` to recursively discover PHP files under the workspace root, respecting `.gitignore` rules (including nested and global gitignore files). This automatically skips generated/cached directories like `storage/framework/views/` (Laravel blade cache), `var/cache/` (Symfony), and `node_modules/`. The vendor directory is always skipped regardless of `.gitignore` content. Hidden directories are skipped by default.

### Cross-file scanning

The `user_file_symbol_maps()` helper snapshots all symbol maps whose URI does not fall under the vendor directory or the internal stub scheme. Four scanners use this snapshot:

- `find_class_references` — matches `ClassReference` spans by resolved FQN
- `find_member_references` — matches `MemberAccess` and `MemberDeclaration` spans by member name
- `find_function_references` — matches `FunctionCall` spans by resolved FQN
- `find_constant_references` — matches `ConstantReference` spans by name

Variable references (`$this`, local variables) are scoped to the enclosing function/class in the current file only, and do not use the cross-file scan.

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

- **`UseFunction`** and **`UseConst`** short-circuit to dedicated builders (`build_function_completions`, `build_constant_completions`). These bypass class-name logic entirely. Items from the current file are filtered out (importing a function from the file you are editing is pointless). A semicolon is appended to the insert text. For `UseFunction`, namespaced functions use their FQN as insert text so the resulting statement reads `use function Illuminate\Support\enum_value;`.

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

## Memory Overhead

The `symbol_maps` store is keyed by file URI, matching `ast_map`. A typical PHP file has 100–400 navigable symbol tokens. Each `SymbolSpan` is ~50–100 bytes (two `u32` fields plus the `SymbolKind` enum with a `String` or two), totalling ~20–40 KB per open file. Variable definition sites add ~1–3 KB. For comparison, `open_files` already stores the full file content (often 50–200 KB per file), so the symbol map is a small fraction of existing memory use.

Files that are not open (vendor files loaded via PSR-4 on demand) do not get a symbol map — they use the stored byte offsets from Tier 2 (which live on `ClassInfo` / `MethodInfo` / etc. in `ast_map`).

## Name Resolution

PHP class names go through resolution at parse time (`resolve_parent_class_names`):

1. **Fully-qualified** (`\Foo\Bar`) → strip leading `\`, use as-is.
2. **In use map** (`Bar` with `use Foo\Bar;`) → expand to `Foo\Bar`.
3. **Qualified** (`Sub\Bar` with `use Foo\Sub;`) → expand first segment.
4. **Unqualified, not in use map** → prepend current namespace.
5. **No namespace** → keep as-is.

This runs on `parent_class`, `used_traits`, and `mixins` for every `ClassInfo` extracted from a file.