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

---

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

---

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

---

### VS Code

1. **Install a generic LSP client extension**

   * Recommended: [Generic LSP Client (v2)](https://marketplace.visualstudio.com/items?itemName=zsol.vscode-glspc)
   * Install via VS Code Marketplace:

     ```vscode-extensions
     zsol.vscode-glspc
     ```

2. **Download PHPantom LSP binary**

   * Get it from [GitHub Releases](https://github.com/AJenbo/phpantom_lsp/releases/latest)
   * Extract the binary and place it in a preferred location

3. **Configure the extension**

   * Open VS Code settings for Generic LSP Client (v2)
   * Set the path to your PHPantom binary
   * Add the Language ID: `php`
   * Restart VS Code

---

### PHPStorm

1. **Download PHPantom LSP binary**

   * Get it from [GitHub Releases](https://github.com/AJenbo/phpantom_lsp/releases/latest)
   * Extract the binary to a preferred location

2. **Install and configure LSP plugin**

   * Go to **Editor → Plugins** and install [LSP4IJ](https://plugins.jetbrains.com/plugin/23257-lsp4ij)
   * Restart PHPStorm
   * Navigate to **Languages & Frameworks → Language Servers**
   * Click **+** to add a new server

     * Name: `PHPantom`
     * Command: path to your PHPantom binary
     * Mapping: add a new Language and set it to `PHP`

<img width="779" height="645" alt="Screenshot From 2026-02-21 00-28-37" src="https://github.com/user-attachments/assets/2da88e68-d012-476e-82e7-977dbfcd9653" />

<img width="779" height="645" alt="Screenshot From 2026-02-21 00-32-24" src="https://github.com/user-attachments/assets/62358f9e-973c-487d-ac17-098d7dab007e" />

---

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
