# Manual Installation

Most editors install PHPantom automatically (see [Editor Setup](editor-setup.md)). Use these methods when your editor doesn't manage the binary for you, or when you want a specific version.

## Latest Release

=== "Cargo"

    ```bash
    cargo install phpantom_lsp --locked
    ```

    See [phpantom_lsp on crates.io](https://crates.io/crates/phpantom_lsp).

=== "Cargo binstall"

    ```bash
    cargo binstall phpantom_lsp
    ```

    [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) downloads
    pre-built binaries instead of compiling from source, so installation
    takes seconds instead of minutes.

=== "Homebrew"

    ```bash
    brew install phpantom-lsp
    ```

    Available on macOS and Linux.

=== "Pre-built Binary"

    Download the latest binary for your platform from
    [GitHub Releases](https://github.com/PHPantom-dev/phpantom_lsp/releases/latest).
    Available for:

    - `x86_64-unknown-linux-gnu`
    - `aarch64-unknown-linux-gnu`
    - `x86_64-apple-darwin`
    - `aarch64-apple-darwin`
    - `x86_64-pc-windows-msvc`

    The Linux binaries are statically linked (musl), so they have no
    minimum glibc requirement and run on any Linux distribution, including
    old-glibc systems (RHEL/CentOS 8, Debian 11) and musl-based ones
    (Alpine). The `-unknown-linux-gnu` name is retained for compatibility
    with existing installers.

## Latest Main (Unreleased)

Install the latest `main` branch to get fixes and features that haven't
been released yet. Useful for verifying bug fixes or testing new features.

```bash
cargo install --git https://github.com/PHPantom-dev/phpantom_lsp --branch main phpantom_lsp
```

## Build from Source

See [Building from Source](BUILDING.md) for full instructions. Quick version:

```bash
git clone https://github.com/PHPantom-dev/phpantom_lsp
cd phpantom_lsp
cargo build --release
# Binary is at target/release/phpantom_lsp
```

Once installed, see [Editor Setup](editor-setup.md) to configure your editor.
