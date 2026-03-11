# Building & Development

## Quick Start

```bash
cargo build --release   # build the binary
```

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)

## Build

The `build.rs` script automatically fetches the latest [JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) from GitHub and embeds them directly into the binary. This gives the LSP full knowledge of built-in PHP classes, functions, and constants with no runtime dependencies.

The stubs are downloaded on first build and cached in `stubs/`. To update to the latest stubs, delete the `stubs/` directory and rebuild.

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
