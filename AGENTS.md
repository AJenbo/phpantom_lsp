# AI Agent Guidelines for PHPantom

This is a Rust-based PHP language server. Performance and memory
efficiency are critical -- PHPantom is one of the fastest language
servers available and it must stay that way.

## Before Committing

Always run these checks before considering any change complete:

```bash
cargo clippy -- -D warnings
cargo clippy --tests -- -D warnings
cargo fmt
```

Clippy runs twice: once for library code, once including tests. Run
`cargo fmt` after clippy, not before -- clippy fixes can affect
formatting.

## Contributing Guidelines

Read and follow [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for the
full set of CI checks, testing conventions, and code style rules.

## Key Rules

- **Performance is critical.** Every allocation, clone, and lock
  matters. Avoid unnecessary heap allocations, prefer `&str` over
  `String` where possible, and be mindful of hot paths. Do not
  introduce regressions in startup time or memory usage.
- **Run the full lint pipeline.** `cargo clippy` and `cargo fmt` must
  pass with zero warnings before every commit. Do not skip this.
- **Update the changelog.** When a change affects user-visible
  behaviour, add an entry under `## [Unreleased]` in
  `docs/CHANGELOG.md`. Write for end users, not developers. Include
  `Contributed by @username` with the GitHub username of the author.
- **Reference issues in commits.** When fixing a GitHub issue, include
  `Closes #123` in the commit message body.
- **Prefer single tests.** Run individual tests (`cargo test test_name`)
  rather than the full suite during development for faster feedback.
- **Debug root causes.** When investigating a bug, determine the root
  cause rather than patching symptoms.
- **Clean commit history.** Use atomic commits, each representing one
  logical change. No fixup or WIP commits. Use
  [conventional commits](https://www.conventionalcommits.org/) for the
  subject line (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`, etc.).
  The commit body should explain *why* the change was made, not just
  what changed. Wrap the body at 80 characters.
- **Comments only where they add value.** Don't add obvious or
  boilerplate comments. Do comment tricky logic, non-obvious design
  decisions, and workarounds. Follow existing conventions.
  All files must end with a newline.
