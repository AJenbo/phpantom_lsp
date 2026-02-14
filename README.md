# PHPantomLSP

A fast, lightweight PHP language server that stays out of your way. Using only a few MB of RAM regardless of project size and fully usable in milliseconds without requiring high-end hardware.

> **Note:** This project is in active development.

## Features

### Document Synchronization

- Full text document sync (open, change, close)

### PHP Analysis & Type Resolution

PHPantom uses a shared analysis engine built on [Mago](https://github.com/carthage-software/mago)'s PHP parser for parsing and type resolution, powering both completion and go-to-definition.

- Extracts classes, interfaces, traits, enums, and standalone functions
- Parses methods, properties, constants, and constructor-promoted properties with visibility, static modifiers, and type hints
- Parses `use` statements and namespace declarations
- PHPDoc annotations including:
  - `@return`, `@var`, `@property`, `@method`, `@mixin`
  - PHPStan-style conditional return types
- Resolves `$this`, `self`, `static`, and `parent` keywords
- Infers variable types from assignments and parameter type hints
- Supports property chains and method call chaining (e.g., `$this->getService()->doSomething()`)
- Resolves function and static method return types (e.g., `app()->`, `Class::make()->`)
- Inheritance-aware resolution including traits cases
- Handles union types (`A|B`) and ambiguous variables across conditional branches

### Completion

- Instance member completion via `->` (methods and properties)
- Static member completion via `::` (static methods, static properties, constants, and enum cases)
- `parent::` completion (static and non-static members, excluding private)
- Magic method filtering (`__construct`, `__destruct`, etc. are excluded from results)
- Full method signature display in completion labels (parameters, types, return type)

<img width="683" height="339" alt="image" src="https://github.com/user-attachments/assets/65e8220d-5d94-466f-aea7-2f239a8d4b19" />

### Go to Definition

- Jump to class, interface, trait, enum, and standalone function definitions
- Jump to method, property, and constant definitions on a class
- Same-file and cross-file definition lookup
- Fully-qualified, partially-qualified, and unqualified name resolution via `use` statements and the current namespace

### Composer Integration

- Parses `composer.json` for PSR-4 class mapping
- Parses vendor autoload file and PSR-4 loading
- Caches parsed files in memory to avoid redundant loading

## Building

```bash
cargo build

# or for a release build
cargo build --release
```

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

## Contributing

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).

## License

MIT - see [LICENSE](LICENSE).