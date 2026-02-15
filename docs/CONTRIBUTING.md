# Contributing to PHPantomLSP

Thanks for your interest in contributing!

## Getting Started

1. Fork and clone the repository
2. Install Rust via [rustup](https://rustup.rs/)
3. Install [Composer](https://getcomposer.org/) (PHP dependency manager)
4. Run `composer install` to fetch the PHP standard library stubs
5. Run `cargo build` to verify everything compiles

> **Note:** The `composer install` step downloads [JetBrains phpstorm-stubs](https://github.com/JetBrains/phpstorm-stubs) into `stubs/`, which are then embedded into the binary at compile time. Without this step the build will succeed, but the LSP won't know about built-in PHP symbols like `Iterator`, `Countable`, `UnitEnum`, etc.

## Before Submitting a PR

Please make sure all checks pass:

```bash
cargo test
cargo clippy
cargo fmt --check
```

## Code Style

- Run `cargo fmt` before committing
- Address any `cargo clippy` warnings
- Add tests for new functionality in `tests/integration_tests.rs`

## Testing

- Unit/integration tests: `cargo test`
- Manual LSP testing: `./test_lsp.sh` (sends JSON-RPC messages over stdin/stdout)

## Reporting Issues

Open an issue on GitHub with:
- What you expected to happen
- What actually happened
- Steps to reproduce
