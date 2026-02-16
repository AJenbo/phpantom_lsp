# PHPantomLSP

A fast, lightweight PHP language server that stays out of your way. Using only a few MB of RAM regardless of project size and fully usable in milliseconds without requiring high-end hardware.

> **Note:** This project is in active development.

## Features

### Completion

- **Instance members** via `->` — methods, properties, constructor-promoted properties
- **Static members** via `::` — static methods, static properties, constants, enum cases
- **`parent::`** — inherited and overridden members (excludes private)
- **Traits and interfaces** — trait members appear on the using class; interface contracts are resolved
- **`@mixin` classes** — members from `@mixin` annotations are included, even when the mixin is declared on a parent class (the Laravel `Model`/`Builder` pattern works out of the box)
- **Magic members** — `@property`, `@property-read`, `@method` from PHPDoc
- **Method chaining** — return types are followed through arbitrarily long chains
- **Null-safe chaining** — `?->` is handled identically to `->`
- **Full signatures** in completion labels (parameters, types, return type)
- Magic methods (`__construct`, `__destruct`, etc.) are filtered out of results

<img width="683" height="339" alt="image" src="https://github.com/user-attachments/assets/65e8220d-5d94-466f-aea7-2f239a8d4b19" />

### Go to Definition

- **Classes, interfaces, traits, enums** — same-file and cross-file
- **Methods, properties, constants** — resolves through inheritance, traits, and mixins
- **Standalone functions** — including PHP built-ins via embedded stubs
- **Variables** — jumps to the most recent assignment or declaration (assignment, parameter, `foreach`, `catch`, `static`/`global`)
- **Namespace resolution** — fully-qualified, partially-qualified, and aliased names via `use ... as ...`

### Type Resolution

PHPantom infers variable types from assignments, parameter hints, return types, and PHPDoc annotations, then uses them to power both completion and go-to-definition.

- **Union types** (`A|B`) and **intersection types** (`A&B`), including PHP 8.2 DNF types like `(A&B)|C`
- **Conditional return types** — PHPStan-style `@return ($param is class-string<T> ? T : mixed)`, used by patterns like `app(User::class)->getEmail()`
- **`@var` overrides** — inline `/** @var Type $var */` docblocks refine variable types
- **Ambiguous variables** — when a variable is assigned different types in conditional branches, all candidates are offered

### Type Narrowing

Completion results adapt to runtime type checks. PHPantomLSP narrows union types in both the positive and inverse branches of `if`/`else`, `while`, and `match(true)`:

- `instanceof` and negated `!instanceof`
- `is_a($var, ClassName::class)`
- `get_class($var) === ClassName::class` and `$var::class === ClassName::class` (including `!==` and reversed operand order)
- `assert($var instanceof ClassName)`
- **Custom assertion functions** via `@phpstan-assert` / `@psalm-assert` annotations:
  - `@phpstan-assert Type $param` — unconditional narrowing after the call
  - `@phpstan-assert-if-true Type $param` — narrows in the then-branch
  - `@phpstan-assert-if-false Type $param` — narrows in the else-branch

### Composer & Project Awareness

- Parses `composer.json` for PSR-4 and file autoload mappings
- Resolves cross-file class lookups on demand

## Building

The PHP stubs are managed as a Composer dependency in `stubs/`. Install them before building:

```bash
# Install the PHP stubs (requires Composer)
composer install

# Build
cargo build

# or for a release build
cargo build --release
```

> **Note:** The build will succeed without `composer install`, but the resulting binary won't know about built-in PHP symbols like `Iterator`, `Countable`, `UnitEnum`, etc. Always run `composer install` first for a fully functional build.

After updating stubs (`composer update`), just rebuild — the `build.rs` script watches `composer.lock` and re-embeds everything automatically.

For more details on how symbol resolution and stub loading work, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Testing

Run the test suite:

```bash
cargo test
```

### Manual LSP Testing

The included `test_lsp.sh` script sends JSON-RPC messages to the server over stdin/stdout, exercising the full LSP protocol flow (initialize, open file, hover, completion, shutdown):

```bash
./test_lsp.sh
```

This is useful for verifying end-to-end behavior outside of an editor.

### Debugging

Enable logging by setting the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run 2>phpantom.log
```

Logs are written to stderr, so redirect as needed.

## Editor Integration

PHPantomLSP communicates over stdin/stdout. Point your editor's LSP client at the binary:

- **Path:** `target/release/phpantom_lsp` (after `cargo build --release`)

### Zed

Install it as a dev extension from the `zed-extension/` directory in this repo:

1. Open Zed
2. Open the Extensions panel
3. Click **Install Dev Extension**
4. Select the `zed-extension/` directory

The extension automatically downloads the correct pre-built binary from GitHub releases for your platform. If you'd prefer to use a locally built binary, ensure `phpantom_lsp` is on your `PATH` and the extension will use it instead.

To configure PHPantom LSP as the default PHP language server in Zed, add the following to your Zed settings (`settings.json`):

```json
{
  "languages": {
    "PHP": {
      "language_servers": ["phpantom_lsp", "!intelephense", "!phpactor", "!phptools", "..."]
    }
  }
}
```

## Contributing

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

## License

MIT - see [LICENSE](LICENSE).
