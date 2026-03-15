# PHPantom — External Stubs

This document covers how PHPantom can support external PHP stub files
beyond the built-in phpstorm-stubs embedded in the binary. External
stubs let users get type information for PHP extensions, framework
helpers, and IDE-specific annotations that the bundled stubs don't
cover or that the user wants to override.

Items are ordered by **impact** (descending), then **effort** (ascending)
within the same impact tier.

| Label      | Scale                                                                                                                  |
| ---------- | ---------------------------------------------------------------------------------------------------------------------- |
| **Impact** | **Critical**, **High**, **Medium-High**, **Medium**, **Low-Medium**, **Low**                                           |
| **Effort** | **Low** (≤ 1 day), **Medium** (2-5 days), **Medium-High** (1-2 weeks), **High** (2-4 weeks), **Very High** (> 1 month) |

---

## Current state

PHPantom embeds JetBrains phpstorm-stubs at compile time via
`build.rs`. The stubs are baked into the binary as static string
arrays and indexed by class, function, and constant name. At runtime,
`find_or_load_class` checks the `stub_index` as a final fallback
(Phase 3) after `ast_map`, classmap, and PSR-4. Stub files are parsed
lazily on first access and cached under `phpantom-stub://` URIs.

This works well for the PHP standard library but has limitations:

- **Version lag.** The embedded stubs are pinned to whatever version
  of phpstorm-stubs was installed when the binary was built. Users on
  newer PHP versions or extensions released after the build get no
  coverage until a PHPantom update ships.
- **No extension stubs.** PHP extensions not covered by phpstorm-stubs
  (or covered poorly) have no resolution path. Common examples:
  Swoole, OpenSwoole, RoadRunner, event, uv, and various PECL
  extensions.
- **No project-level overrides.** Packages like
  `phpstan/phpstan-extensions`, `php-stubs/wordpress-stubs`,
  `wimg/php-compatibility-stubs`, or hand-written project stubs
  cannot augment or override the built-in definitions.
- **No GTD for built-in symbols.** Embedded stubs use synthetic
  `phpantom-stub://` URIs with no on-disk file. Go-to-definition
  returns nothing for `array_map`, `Iterator`, `PDO`, etc. If the
  user has phpstorm-stubs (or another stub package) installed locally,
  GTD could navigate to those real files.
- **No generic annotations on SPL.** The embedded phpstorm-stubs lack
  `@template` annotations on SPL iterator classes. PHPStan maintains
  its own stub overlays for these. Detecting project-level stubs
  would let PHPantom pick up richer type information automatically.

---

## Stub sources (in priority order)

External stubs come from four places, listed from highest to lowest
priority. When the same symbol is defined by multiple sources, the
first source wins.

### 1. `.phpantom.toml` stub paths

The highest-priority source. For projects that need stubs not
available as Composer packages, or for non-Composer projects
entirely, `.phpantom.toml` can list additional directories:

```toml
[stubs]
paths = [
    "./stubs",
    "/opt/company/php-stubs",
]
```

Paths are resolved relative to the workspace root unless absolute.
Each path is a directory that is scanned recursively for `.php` files
at init time.

This takes top priority because it represents an explicit, deliberate
choice by the user for this project. If they placed a stub file here,
they want it to win over everything else.

Most users will never touch this setting. It exists for non-Composer
projects, company-internal stubs, hand-written polyfill annotations,
and overrides where the user knows better than any automated source.

### 2. Project-level stubs from Composer

PHP projects commonly install stub packages via Composer as
`require-dev` dependencies:

```json
{
    "require-dev": {
        "jetbrains/phpstorm-stubs": "^2025.3",
        "php-stubs/wordpress-stubs": "^6.0",
        "php-stubs/acf-pro-stubs": "^6.0"
    }
}
```

These packages land in the vendor directory and contain `.php` files
with annotated class/function/constant definitions. Some ship their
own map files; most are just directories of PHP files.

This is the primary zero-config mechanism. It requires no PHPantom
configuration, works with existing Composer workflows, and lets
projects pin a specific stubs version. When a new PHP version ships
and the embedded stubs lag behind, `composer update
jetbrains/phpstorm-stubs` in the project is all it takes.

**Detection:** During `initialized`, after loading `composer.json`
and the classmap, check `vendor/composer/installed.json` for known
stub package patterns (see "Known stub packages" below). Also check
whether `jetbrains/phpstorm-stubs` is listed as an installed package.

### 3. IDE-provided stub path

IDE extensions that bundle PHPantom (Zed, VS Code, Neovim plugin
packages, etc.) may ship their own stubs directory alongside the
binary. The extension knows where those stubs live; the user does
not need to.

The path is communicated via `initializationOptions` in the LSP
`initialize` request:

```json
{
    "initializationOptions": {
        "stubs": {
            "path": "/path/to/bundled/stubs"
        }
    }
}
```

This lets an IDE extension:

- Build PHPantom **without** embedded stubs (empty `STUB_FILES`
  array) to produce a smaller binary.
- Bundle phpstorm-stubs (or any stub set) as plain files alongside
  the binary.
- Get GTD for built-in symbols for free (the stubs are real files).
- Update stubs independently of PHPantom releases.

The user never configures this path. It is an integration point
between PHPantom and the extension that wraps it. IDE-provided stubs
sit below `.phpantom.toml` and Composer because project-specific
choices should override what the IDE ships by default.

### 4. Embedded stubs (current behaviour)

The phpstorm-stubs compiled into the binary. Always available as the
final fallback. Every other source overrides these when they define
the same symbol.

When PHPantom is built without embedded stubs, this source is empty
and effectively skipped.

---

## phpstorm-stubs fast path

`jetbrains/phpstorm-stubs` gets special treatment regardless of which
source provides it. The package ships `PhpStormStubsMap.php`, a
generated index that maps every class, function, and constant name to
its file path. PHPantom's `build.rs` already parses this file at
compile time. The same parsing logic can run at runtime.

When phpstorm-stubs is found (in any source: `.phpantom.toml`,
Composer vendor, IDE-provided path), PHPantom checks for the presence
of `PhpStormStubsMap.php`. If found, it parses the map file to build
name-to-path indices in a single fast text scan. This is much cheaper
than directory-walking and byte-level scanning every `.php` file.

Other stub packages (wordpress-stubs, extension stubs, hand-written
stubs) do not ship a map file. These are scanned with the byte-level
classmap scanner.

The processing order at init is:

1. **Map-file indexed stubs first.** Parse `PhpStormStubsMap.php`
   from whichever source provides phpstorm-stubs. This populates the
   external stub indices with the full PHP standard library in one
   pass.
2. **Directory-scanned stubs on top.** Scan all other stub
   directories (wordpress-stubs, custom stubs, etc.) with the
   byte-level scanner. These insert into the indices only when the
   key is not already present (respecting source priority) or when
   they define symbols that phpstorm-stubs does not cover.

This means phpstorm-stubs always provides the fast baseline, and
other packages layer additional or overriding definitions on top
according to their source priority.

---

## E1. Project-level phpstorm-stubs for GTD

**Goal:** When `jetbrains/phpstorm-stubs` is installed in the
project's vendor directory, use those on-disk files for
go-to-definition on built-in symbols. All other resolution (type
information, completion, hover) continues to use the embedded stubs.

This is the smallest useful increment: no new config, no new scanning,
no priority changes. It solves the most frequent user complaint ("I
can't Ctrl+Click on `array_map`").

### Detection

During `initialized`, after parsing `installed.json`, check whether
the `jetbrains/phpstorm-stubs` package is present. If so, record the
path to its install directory (e.g.
`vendor/jetbrains/phpstorm-stubs/`).

The stubs ship with `PhpStormStubsMap.php`, the same file PHPantom's
`build.rs` reads at compile time. Parse it at runtime using the same
`parse_section` logic to build class/function/constant name-to-path
maps pointing at the on-disk files.

### GTD changes

When go-to-definition resolves a symbol to a `phpantom-stub://` or
`phpantom-stub-fn://` URI (which currently returns `None` because
there is no real file), check whether the project-level phpstorm-stubs
path is available. If so, map the symbol name back to the on-disk
stub file and return a `Location` pointing at the declaration.

Finding the exact line within the stub file can reuse the existing
member-lookup logic in `definition/member/file_lookup.rs` (read the
file, parse it, find the symbol by name/offset).

### What this does NOT change

- Type resolution still uses the embedded stubs. The on-disk stubs
  are only consulted for navigation.
- No new config options.
- No scanning of stub files at init (just parsing the map file, which
  is a single fast text scan).

### Effort

Low. The map-parsing logic already exists in `build.rs` and can be
extracted into a shared helper. The GTD fallback is a small addition
to `resolve_class_definition` / `resolve_function_definition`.

---

## E2. Project-level stubs as resolution source

**Goal:** Let project-level stub packages override or augment the
embedded stubs for type resolution, completion, and hover. This is
where external stubs become a real type-intelligence feature rather
than just a navigation aid.

### Priority model

When multiple sources define the same symbol, the highest priority
source wins:

1. **User code** (opened files, PSR-4, classmap). Always wins. A
   user-defined class with the same name as a stub class shadows
   the stub entirely.
2. **`.phpantom.toml` stubs.** Explicit user overrides for this
   project.
3. **Composer project-level stubs.** Packages from the vendor
   directory. When a project installs `jetbrains/phpstorm-stubs` at
   a newer version than what is embedded, the project version is
   used.
4. **IDE-provided stubs** (`initializationOptions`). The IDE
   extension's bundled stubs.
5. **Embedded stubs** (current behaviour). Final fallback.

This means a project that installs `php-stubs/wordpress-stubs` gets
WordPress function/class resolution automatically. A project that
installs a newer phpstorm-stubs gets updated type information without
waiting for a PHPantom release. And a user who places a custom stub
in `.phpantom.toml` paths can override anything.

### Discovery: known stub packages

Stub packages follow a few conventions:

**Packages with a map file.** `jetbrains/phpstorm-stubs` ships
`PhpStormStubsMap.php`. Parse it to get symbol-to-file mappings.
This is the fastest path: no directory scanning needed.

**Packages without a map file.** Most stub packages (wordpress-stubs,
acf-pro-stubs, etc.) are just directories of `.php` files. These
need to be scanned using the byte-level classmap scanner (Phase 1
of indexing.md) extended with function/constant detection (Phase 2.5
of indexing.md). The scan produces name-to-path indices just like
the autoload file scanner.

**Detection heuristic:** A Composer package is treated as a stub
package when any of these conditions are true:

- Its package name is `jetbrains/phpstorm-stubs`.
- Its package name matches `php-stubs/*` or `*-stubs`.
- Its `composer.json` `type` field is `phpstorm-stubs` or
  `php-stubs` (a convention some packages follow).

Packages matched by the heuristic are scanned at init and their
symbols are added to new external stub indices.

### New indices

Three new maps on `Backend`, structured identically to the embedded
stub indices but holding owned data (file paths) instead of static
string references:

| Field | Type | Purpose |
|---|---|---|
| `external_stub_class_index` | `HashMap<String, PathBuf>` | Class/interface/trait/enum FQN to stub file path |
| `external_stub_function_index` | `HashMap<String, PathBuf>` | Function FQN to stub file path |
| `external_stub_constant_index` | `HashMap<String, PathBuf>` | Constant name to stub file path |

### Resolution changes

Insert a new phase in each resolution chain between user code and
embedded stubs:

**`find_or_load_class`:**
1. Phase 1: `ast_map` (user code, already-parsed files)
2. Phase 1.5: Composer classmap
3. Phase 2: PSR-4
4. **Phase 2.5 (new): External stub class index.** Checks the
   unified external stub index (populated from `.phpantom.toml`,
   Composer stubs, and IDE-provided stubs in priority order). Read
   the file, parse and cache in `ast_map` under a
   `phpantom-ext-stub://` URI.
5. Phase 3: Embedded stubs

**`find_or_load_function`:**
1. `global_functions` (user code + cached results)
2. `autoload_function_index` (from Phase 2.5 of indexing.md)
3. **External stub function index (new).** Same unified index.
   Read the file, parse, cache in `global_functions`.
4. `stub_function_index` (embedded stubs)

**Constants:** Same pattern. External stub constants slot in before
embedded stub constants.

### GTD improvement

Since external stubs point at real on-disk files, go-to-definition
works naturally. The `phpantom-ext-stub://` URI scheme carries the
real file path, so GTD resolves to a navigable `Location`. This
supersedes the Phase 1 GTD-only approach for any symbol that has
an external stub (from any source).

### Interaction with embedded phpstorm-stubs

When `jetbrains/phpstorm-stubs` is installed at the project level:

- The project-level version takes priority for all symbols it defines.
- Symbols that exist only in the embedded version (because the
  project-level version is older or has removed entries) still
  resolve via the embedded fallback.
- This means the user always gets the union of both sets, with the
  project-level version winning on conflicts.

When a non-phpstorm-stubs package defines a symbol that also exists
in the embedded stubs (e.g. `wordpress-stubs` redefining `wpdb`),
the external package wins. This is the correct behaviour: the
project-specific definition is more accurate than the generic one.

### Effort

Medium. The scanning infrastructure depends on Phase 2.5 of
indexing.md (byte-level function/constant scanner). The resolution
changes are straightforward (one new phase in each lookup chain).
The `PhpStormStubsMap.php` parser for project-level phpstorm-stubs
is already written in `build.rs` and just needs to be available at
runtime.

---

## E3. IDE-provided and `.phpantom.toml` stub paths

**Goal:** Support stub directories provided by IDE extensions (via
`initializationOptions`) and by users (via `.phpantom.toml`). Phase 2
handles Composer-discovered stubs. This phase adds the remaining two
external sources.

### IDE-provided path via `initializationOptions`

IDE extensions that bundle PHPantom can pass a stubs directory in the
LSP `initialize` request. PHPantom reads the path from
`initializationOptions.stubs.path` and scans it at init. The user
never sees or configures this.

This enables a distribution model where the IDE extension:
1. Builds PHPantom without embedded stubs (smaller binary).
2. Ships phpstorm-stubs as plain files alongside the binary.
3. Passes the path at startup.

Because the stubs are real on-disk files, GTD works out of the box
with no extra logic. The extension can update stubs independently
of PHPantom releases.

The phpstorm-stubs fast path applies here too: if the IDE-provided
directory contains `PhpStormStubsMap.php`, parse the map file for
fast indexed lookup instead of directory scanning.

### `.phpantom.toml` paths

For non-Composer projects and for explicit overrides:

```toml
[stubs]
paths = [
    "./stubs",
    "/opt/company/php-stubs",
]
```

Paths are resolved relative to the workspace root unless absolute.
Each path is scanned recursively for `.php` files at init.

### Scanning and priority

All sources use the same byte-level scanner (or the phpstorm-stubs
map-file fast path when available). The external stub indices are
populated in priority order, highest first. Each insert is
skip-if-present, so higher-priority sources win:

1. **`.phpantom.toml` paths.** Scanned first. Explicit user choices
   for this project override everything else.
2. **Composer project-level stubs** (Phase 2). The project's vendor
   directory.
3. **IDE-provided stubs** (`initializationOptions`). The IDE
   extension's bundled stubs.
4. **Embedded stubs.** Final fallback (not in the external index;
   checked separately as the last resolution phase).

### Use cases

- **IDE extension distribution.** A Zed/VS Code extension ships
  PHPantom + stubs as a single package. No Composer needed. GTD
  on built-in symbols works immediately.
- **Non-Composer projects.** A legacy codebase without `composer.json`
  can point at a stubs directory via `.phpantom.toml`.
- **Extension stubs.** Swoole, RoadRunner, or other PECL extension
  stubs not available as Composer packages.
- **Company-internal stubs.** Hand-written type annotations for
  proprietary code.
- **Overrides.** A user who disagrees with a phpstorm-stubs type
  annotation can place a corrected stub in their `.phpantom.toml`
  paths and it wins over everything.

### Effort

Low (once Phase 2 is done). The scanning is identical. The new work
is reading `initializationOptions` during `initialize`, reading
`.phpantom.toml` `[stubs]` paths, resolving them, and feeding them
into the existing scanner.

---

## E4. Embedded stub override with external stubs

**Goal:** When a project-level or global stub defines a symbol with
richer type annotations than the embedded stub (e.g. `@template` on
SPL iterators), use the richer version for type resolution.

### The SPL iterator problem

The embedded phpstorm-stubs lack `@template` annotations on SPL
iterator classes (`ArrayIterator`, `FilterIterator`,
`RecursiveIteratorIterator`, etc.). PHPStan maintains its own stub
overlays that add these annotations. Without them, `foreach` over
an SPL iterator resolves element types as `mixed`.

Phase 2 already solves this if the user installs a stub package that
includes the annotations. Phase 4 addresses the question: should
PHPantom ship its own SPL overlay stubs, or rely on users to bring
their own?

### Decision: ship minimal overlays, prefer external

1. **Ship a small set of built-in overlay stubs** for the most
   impactful SPL classes (10-15 classes). These are embedded in the
   binary alongside the phpstorm-stubs, but with `@template`
   annotations added. They take priority over the base phpstorm-stubs
   for the classes they cover.

2. **External stubs always win.** If any external source
   (`.phpantom.toml`, Composer, or IDE-provided) defines the same
   class, the external version takes priority over both the overlay
   and the base embedded stub. This means users who install PHPStan's
   stubs or write their own overlays are never fighting with the
   built-in ones.

### Implementation

The overlay stubs can be embedded via `build.rs` the same way the
base stubs are. They go into a separate `STUB_OVERLAY_CLASS_MAP`
array. At resolution time, when `find_or_load_class` reaches Phase 3
(embedded stubs), it checks the overlay map first, then the base map.

### Effort

Low. The overlay stubs are small hand-written PHP files. The build
and resolution changes are minor additions to the existing
infrastructure.

---

## Open questions

### Should external stubs be scanned eagerly or lazily?

**Option A: Eager scan, lazy parse (recommended).** At init, run the
byte-level scanner over all external stub directories to build the
name-to-path indices. Parse individual files on demand when a symbol
is first accessed. This is consistent with the approach in Phase 2.5
of indexing.md (lazy autoload file indexing) and keeps init fast.

**Option B: Fully lazy.** Don't scan at init. When a symbol is not
found in user code or embedded stubs, search through external stub
directories on the fly. This has the worst first-access latency and
makes completion of stub symbols impossible until something triggers
a scan.

Option A is the clear winner. The byte-level scan is fast (sub-second
for typical stub packages) and gives us the name index needed for
completion.

### How does this interact with the classmap?

External stub packages installed via Composer may appear in the
classmap (`autoload_classmap.php`). This is fine: Phase 1.5 of
`find_or_load_class` already handles classmap lookups, and any class
found there is parsed and cached normally. The external stub index
serves as a parallel discovery path for stub packages that are
`require-dev` dependencies (which may not be in the classmap if the
user ran `composer install --no-dev` in production).

In practice, most stub packages declare their classes in
`autoload.classmap` in their own `composer.json`, so they do appear
in the generated classmap. The external stub index provides a
safety net and is also needed for function and constant stubs (which
the classmap does not cover).

### What about `phpstan-extension-installer` and PHPStan config?

Some projects configure stub files through `phpstan.neon`:

```neon
parameters:
    stubFiles:
        - stubs/MyCustomStub.php
```

Reading PHPStan config is out of scope for now. PHPantom is not
PHPStan and should not parse its configuration. If users want
PHPantom to see these stubs, they can add the path to
`[stubs] paths` in `.phpantom.toml`. A future iteration could
optionally read `phpstan.neon` `stubFiles` entries as a convenience,
but it is not a priority.

### Building without embedded stubs

The `build.rs` script already handles a missing `stubs/` directory
gracefully by generating empty arrays. If the automatic GitHub
fetch fails (e.g. no network access during the build), the binary
compiles and runs normally; it just has no built-in fallback for
PHP standard library symbols.

For this to work, stubs must come from another source. The most
reliable combinations:
- IDE extension provides stubs via `initializationOptions` (Phase 3).
- The user's project has `jetbrains/phpstorm-stubs` in Composer
  (Phase 2).
- The user points at stubs via `.phpantom.toml`.

Any of these is sufficient. Without any external stubs and without
embedded stubs, built-in symbols would be invisible.

---

## Summary

| #   | Goal | Effort | Dependencies |
|---|---|---|---|
| E1 | GTD for built-in symbols via project-level phpstorm-stubs | Low | None |
| E2 | Project-level stubs as a type resolution source | Medium | indexing.md (byte-level function/constant scanner) |
| E3 | IDE-provided and `.phpantom.toml` stub paths | Low | E2 |
| E4 | Ship SPL overlay stubs, let external stubs override | Low | E2 |

E1 can be done immediately and independently. It provides
immediate value (GTD on `array_map`, `PDO`, `Iterator`, etc.) with
minimal code. E2-E4 build on the scanner infrastructure from
indexing.md and on each other.

The priority order (`.phpantom.toml` > Composer > IDE > embedded)
ensures the user's explicit choices always win. Most users never
touch `.phpantom.toml` and get stubs through Composer (automatic) or
their IDE extension (transparent). The toml paths exist for
overrides, non-Composer projects, and edge cases.

`jetbrains/phpstorm-stubs` receives special treatment regardless of
source: its `PhpStormStubsMap.php` is parsed for fast indexed lookup
instead of directory scanning, then other stub packages are scanned
on top.

---

## E5. Extension stub selection (`[stubs] extensions`)

**Impact: Low-Medium · Effort: Low**

Override which PHP extension stubs are loaded. By default PHPantom
loads core + all commonly bundled extensions, plus any declared in
the project's `composer.json` via `ext-*` keys.

```toml
[stubs]
extensions = [
  "Core", "standard", "json", "mbstring", "curl",
  "redis", "imagick", "mongodb",
]
```

### Auto-detection from `composer.json`

When `extensions` is unset, PHPantom reads the `require` and
`require-dev` sections of `composer.json` and collects every `ext-*`
key. These are added on top of the default set. Only `composer.json`
is read, not `composer.lock`. Transitive `ext-*` requirements from
dependencies are intentionally ignored.

### Manual override

When `extensions` is set, only the listed extensions are loaded and
auto-detection is skipped. Extension names match the directory names
in phpstorm-stubs (e.g. `"redis"`, `"imagick"`, `"swoole"`). An
unrecognised name is silently ignored with a log message.

### Implementation

The build script already embeds all stub files. Filtering happens at
runtime: when building the stub class/function indices, skip entries
whose source file path does not start with one of the enabled
extension directories. This is a simple string prefix check on the
relative path from `STUB_CLASS_MAP`.

---

## E6. Stub install prompt for non-Composer projects

**Impact: Low · Effort: Low**

For non-Composer projects, offer to install phpstorm-stubs into the
project so that go-to-definition works for built-in symbols. The
answer (`true` or `false`) is written to `[stubs] install` in
`.phpantom.toml` so the prompt does not reappear.

This is not implemented yet. The config writing infrastructure
(using `toml_edit` to preserve comments and formatting) is a
prerequisite.