# Contributing to PHPantom

Thanks for your interest in contributing!

## Getting Started

1. Fork and clone the repository
2. Follow the [build instructions](BUILDING.md) to get a working development environment
3. Read [ARCHITECTURE.md](ARCHITECTURE.md) for an overview of how the codebase is structured

## Before Submitting a PR

All six CI checks must pass with zero warnings and zero failures:

```bash
cargo test
cargo clippy -- -D warnings
cargo clippy --tests -- -D warnings
cargo fmt --check
php -l example.php
php -d zend.assertions=1 example.php
```

Note that clippy runs twice, once for library code and once including test code. The `php -l` check ensures `example.php` remains valid PHP. The `php -d zend.assertions=1` run executes `runDemoAssertions()` to verify that scaffolding stubs actually return what their docblocks claim.

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

## Changelog

Update [CHANGELOG.md](CHANGELOG.md) when your PR adds, changes, or fixes something a user would notice. Add entries under `## [Unreleased]` in the appropriate subsection (`### Added`, `### Fixed`, `### Changed`, or `### Removed`). Write for end users, not developers: describe what changed in the editor, not which internal modules were touched. See the existing entries for the style and level of detail expected.

## Reporting Issues

Open an issue on GitHub with:

- What you expected to happen
- What actually happened
- Steps to reproduce (a minimal PHP snippet is ideal)