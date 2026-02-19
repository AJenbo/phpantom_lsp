# Contributing to PHPantom

Thanks for your interest in contributing!

## Getting Started

1. Fork and clone the repository
2. Follow the [build instructions](BUILDING.md) to get a working development environment
3. Read [ARCHITECTURE.md](ARCHITECTURE.md) for an overview of how the codebase is structured

## Before Submitting a PR

All five CI checks must pass with zero warnings and zero failures:

```bash
cargo test
cargo clippy -- -D warnings
cargo clippy --tests -- -D warnings
cargo fmt --check
php -l example.php
```

Note that clippy runs twice, once for library code and once including test code.

## Code Style

- Run `cargo fmt` before committing
- Fix clippy warnings rather than suppressing them. Avoid `#[allow(clippy::...)]` unless truly necessary.
- Add `///` doc comments to all public functions and struct fields

## Testing

- Integration tests go in `tests/completion_*.rs` or `tests/definition_*.rs`, one file per feature area
- Use `create_test_backend()` from `tests/common/mod.rs` for same-file tests
- Use `create_psr4_workspace()` for cross-file / PSR-4 tests
- Test the happy path, edge cases, and interactions with existing features
- When adding a feature, update `example.php` with working examples (and verify with `php -l example.php`)

See [BUILDING.md](BUILDING.md) for more on running tests and manual LSP testing.

## Reporting Issues

Open an issue on GitHub with:

- What you expected to happen
- What actually happened
- Steps to reproduce (a minimal PHP snippet is ideal)