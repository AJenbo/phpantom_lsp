# PHPantomLSP

A PHP Language Server Protocol (LSP) implementation in Rust.

> **Note:** This project is in early development. Features are minimal.

## Features

### Document Synchronization

- Full text document sync (open, change, close)

### Completion

- Instance member completion via `->` (methods and properties)
- Static member completion via `::` (static methods, static properties, and constants)
- `parent::` completion (static and non-static members, excluding private)
- `$this`, `self`, and `static` keyword resolution to the current class
- Property chain resolution (e.g. `$this->service->`)
- Variable type resolution from assignments (`$var = new Foo()`) and parameter type hints
- Inheritance-aware completion — walks the class hierarchy to include inherited members
- Magic method filtering (`__construct`, `__destruct`, etc. are excluded from results)
- Full method signature display in completion labels (parameters, types, return type)

<img width="683" height="339" alt="image" src="https://github.com/user-attachments/assets/65e8220d-5d94-466f-aea7-2f239a8d4b19" />

### Go to Definition

- Jump to class, interface, trait, and enum definitions
- Same-file and cross-file definition lookup
- Fully-qualified, partially-qualified, and unqualified name resolution via `use` statements and the current namespace

### PHP Parsing

- Extracts classes, interfaces, methods, properties, and constants
- Parses visibility modifiers, static modifiers, type hints, and parameter info
- Supports constructor-promoted properties
- Parses `use` statements and namespace declarations

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

MIT — see [LICENSE](LICENSE).
