# Building & Development

## Quick Start

```bash
composer install        # fetch PHP stubs (requires Composer)
cargo build --release   # build the binary
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [Composer](https://getcomposer.org/) (for PHP standard library stubs)

## Build

The PHP stubs are managed as a Composer dependency in `stubs/`. The `build.rs` script embeds the [JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) directly into the binary, giving the LSP full knowledge of built-in PHP classes, functions, and constants with no runtime dependencies.

> [!NOTE]
> The build will succeed without `composer install`, but the resulting binary won't know about built-in PHP symbols like `Iterator`, `Countable`, `UnitEnum`, etc. Always run `composer install` first for a fully functional build.

After updating stubs (`composer update`), just rebuild. `build.rs` watches `composer.lock` and re-embeds everything automatically.

For details on how symbol resolution and stub loading work, see [ARCHITECTURE.md](ARCHITECTURE.md).

## Testing

Run the full test suite:

```bash
cargo test
```

### CI Checks

Before submitting changes, run exactly what CI runs:

```bash
cargo test
cargo clippy -- -D warnings
cargo clippy --tests -- -D warnings
cargo fmt --check
php -l example.php
```

All five must pass with zero warnings and zero failures.

### Manual LSP Testing

The included `test_lsp.sh` script sends JSON-RPC messages to the server over stdin/stdout, exercising the full LSP protocol flow (initialize, open file, hover, completion, shutdown):

```bash
./test_lsp.sh
```

This is useful for verifying end-to-end behavior outside of an editor.

## Debugging

Enable logging by setting the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug cargo run 2>phpantom.log
```

Logs are written to stderr, so redirect as needed.

For editor setup instructions, see [SETUP.md](SETUP.md).