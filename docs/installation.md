# Installation

=== "Cargo"

    ```bash
    cargo install phpantom_lsp --locked
    ```

    See [phpantom_lsp on crates.io](https://crates.io/crates/phpantom_lsp).

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

=== "Build from Source"

    See [Building from Source](BUILDING.md) for full instructions. Quick version:

    ```bash
    cargo build --release
    # Binary is at target/release/phpantom_lsp
    ```

Once installed, see [Editor Setup](editor-setup.md) to configure your editor.
