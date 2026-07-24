# PHPantom -- a PHP language server

<p align="center">
  <img src="assets/spookaphant.svg" alt="PHPantom" width="200" />
</p>

## Welcome to PHPantom's documentation!

A fast, lightweight PHP language server written in Rust. Ready in
seconds, uses a fraction of the RAM other language servers need, and
stays responsive throughout. No indexing phase, no waiting.

!!! note
    PHPantom is in active development. The core editing features are solid and used daily on production codebases.

You may want to jump to:

- Documentation for the [latest released version](https://phpantom-dev.github.io/phpantom_lsp/latest/).
- Documentation for the [unreleased version](https://phpantom-dev.github.io/phpantom_lsp/prerelease/) (corresponds to the `main` branch).

## Key Features

- **Deep type intelligence.** Generics, conditional return types, closure parameter inference, array shapes, PHPStan types.
- **Laravel support.** Eloquent relationships, scopes, accessors, casts, Builder chains, macros, Blade templates -- no ide-helper or database access required.
- **Fast.** 5 seconds to ready on a 21K-file codebase. 360 MB RAM. No disk cache.
- **PHPStan, PHPCS, and Mago integration.** Run external tools on save and surface their diagnostics in the editor.
- **CLI tools.** Batch diagnostics (`analyze`) and automated fixes (`fix`) for CI and bulk cleanup.
- **Refactoring.** Rename, extract method/function/variable/constant/interface, implement interface methods, promote constructor parameters, modernize syntax.

## Useful Links

- [Installation](installation.md) and [Editor Setup](editor-setup.md)
- [Configuration Reference](configuration.md)
- [CLI Reference](cli.md)
- [Development Roadmap](todo.md)
- [Changelog](CHANGELOG.md)
- [Benchmarks](benchmarks.md)
