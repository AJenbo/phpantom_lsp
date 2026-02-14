# PHPantomLSP

A fast, lightweight PHP language server that stays out of your way. Using only a few MB of RAM regardless of project size, fully usable in milliseconds, without requiring high-end hardware.

> **Note:** This project is in active development.

## Features

### Document Synchronization

- Full text document sync (open, change, close)

### Type Resolution

Both completion and go-to-definition draw from a shared type resolution engine:

- `$this`, `self`, `static`, and `parent` keyword resolution
- Variable type inference from assignments (`$var = new Foo()`) and parameter type hints
- Property chain and method call chaining (e.g. `$this->getService()->doSomething()`)
- Function and static method call return type resolution (e.g. `app()->`, `Class::make()->`)
- Inheritance-aware: walks the class hierarchy including traits
- Enum case resolution
- Union types: `A|B` in return types, property types, and parameter hints are split into individual candidates
- Ambiguous variables: when a variable is assigned different types in conditional branches, all possible types are tried
- PHPDoc support: `@return`, `@property`, `@method`, `@mixin`
- PHPStan conditional return types: annotations like `@return ($abstract is class-string<TClass> ? TClass : mixed)` are resolved based on call-site arguments

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

### PHP Parsing

- Extracts classes, interfaces, traits, and enums
- Parses methods, properties, and constants with visibility, static modifiers, and type hints
- Extracts standalone function definitions (global and namespaced)
- Supports constructor-promoted properties
- Parses `use` statements and namespace declarations

#### PHPDoc Parsing

Built on [Mago](https://github.com/carthage-software/mago)'s PHP parser.

- `@return` type extraction with compatibility checks against native type hints
- `@var` type annotations
- `@property` virtual property declarations
- `@method` virtual method declarations
- `@mixin` class delegation tags
- PHPStan style conditional return type expressions (recursive/nested conditionals)

### Composer / PSR-4 Integration

- Parses `composer.json` for PSR-4 autoload mappings (`autoload` and `autoload-dev`)
- Parses vendor autoload mappings from `vendor/composer/autoload_psr4.php`
- On-demand class loading from disk via PSR-4 resolution
- Caches parsed files to avoid redundant parsing

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