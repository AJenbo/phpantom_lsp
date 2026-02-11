# Contributing to PHPantomLSP

Thanks for your interest in contributing!

## Getting Started

1. Fork and clone the repository
2. Install Rust via [rustup](https://rustup.rs/)
3. Run `cargo build` to verify everything compiles

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
