# Installation & Editor Setup

## Installation

### Pre-built Binaries

Download the latest binary for your platform from [GitHub Releases](../../releases). Available for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

### Build from Source

See [BUILDING.md](BUILDING.md) for full instructions. Quick version:

```bash
composer install
cargo build --release
# Binary is at target/release/phpantom_lsp
```

## Project Requirements

> [!IMPORTANT]
> Run `composer install -o` (or `composer dump-autoload -o`) **in your PHP project** to generate the optimized autoload files PHPantom needs for cross-file class resolution.

If your project doesn't use Composer, you can create a minimal `composer.json`:

```json
{ "autoload": { "classmap": ["src/"] } }
```

Then run `composer dump-autoload -o`.

## Editor Setup

PHPantom communicates over stdin/stdout using the standard [Language Server Protocol](https://microsoft.github.io/language-server-protocol/). Any editor with LSP support can use it. Point the client at the `phpantom_lsp` binary with `php` as the file type. No special initialization options are required.

### Zed

A Zed extension is included in the `zed-extension/` directory:

1. Open Zed
2. Open the Extensions panel
3. Click **Install Dev Extension**
4. Select the `zed-extension/` directory

The extension automatically downloads the correct pre-built binary from GitHub releases for your platform. If you'd prefer to use a locally built binary, ensure `phpantom_lsp` is on your `PATH` and the extension will use it instead.

To make PHPantom the default PHP language server, add to your Zed `settings.json`:

```json
{
  "languages": {
    "PHP": {
      "language_servers": ["phpantom_lsp", "!intelephense", "!phpactor", "!phptools", "..."]
    }
  }
}
```

### Neovim

> [!NOTE]
> This configuration is untested. If you get it working (or run into issues), please [open an issue](../../issues).

```lua
vim.lsp.config['phpantom'] = {
  cmd = { '/path/to/phpantom_lsp' },
  filetypes = { 'php' },
  root_markers = { 'composer.json', '.git' },
}
vim.lsp.enable('phpantom')
```

### VS Code

> [!NOTE]
> This configuration is untested. If you get it working (or run into issues), please [open an issue](../../issues).

Use a generic LSP client extension such as [LSP-client](https://github.com/nicolo-ribaudo/vscode-lsp-client) and configure it to run `phpantom_lsp` over stdio for PHP files.

### Sublime Text

> [!NOTE]
> This configuration is untested. If you get it working (or run into issues), please [open an issue](../../issues).

With [LSP for Sublime Text](https://github.com/sublimelsp/LSP):

```json
{
  "clients": {
    "phpantom": {
      "enabled": true,
      "command": ["/path/to/phpantom_lsp"],
      "selector": "source.php"
    }
  }
}
```
