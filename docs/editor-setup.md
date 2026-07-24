# Editor Setup

PHPantom communicates over stdin/stdout using the standard [Language Server Protocol](https://microsoft.github.io/language-server-protocol/). Any editor with LSP support can use it. Point the client at the `phpantom_lsp` binary with `php` as the file type. No special initialization options are required.

<details>
<summary><b>Zed</b></summary>

PHPantom is supported directly by Zed's official PHP extension, no separate PHPantom extension needed. Install (or update) the PHP extension from Zed's Extensions panel, then add PHPantom to your Zed `settings.json`:

```json
{
  "languages": {
    "PHP": {
      "language_servers": ["phpantom", "!intelephense", "!phpactor", "!phptools", "..."]
    }
  }
}
```

If you'd prefer to use a locally built binary, put `phpantom_lsp` on your `PATH` and the extension will use it instead of downloading one.

</details>

<details>
<summary><b>Neovim</b></summary>

PHPantom is included in [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig). If you use nvim-lspconfig, enable it with:

```lua
require('lspconfig').phpantom.setup({})
```

Alternatively, with Neovim's built-in LSP client (no plugins required):

```lua
vim.lsp.config['phpantom'] = {
  cmd = { 'phpantom_lsp' },
  filetypes = { 'php' },
  root_markers = { 'composer.json', '.git' },
}
vim.lsp.enable('phpantom')
```

</details>

<details>
<summary><b>VS Code / Cursor</b></summary>

Install the [PHPantom extension](https://marketplace.visualstudio.com/items?itemName=phpantom.phpantom) from the VS Code Marketplace. It automatically downloads the language server binary and starts it when you open a PHP file.

</details>

<details>
<summary><b>PHPStorm</b></summary>

1. **Download PHPantom LSP binary**

   * Get it from [GitHub Releases](https://github.com/PHPantom-dev/phpantom_lsp/releases/latest)
   * Extract the binary to a preferred location

2. **Install and configure LSP plugin**

   * Go to **Editor → Plugins** and install [LSP4IJ](https://plugins.jetbrains.com/plugin/23257-lsp4ij)
   * Restart PHPStorm
   * Navigate to **Languages & Frameworks → Language Servers**
   * Click **+** to add a new server

     * Name: `PHPantom`
     * Command: path to your PHPantom binary
     * Mapping: set `PHP` on both the **Language** tab and the **File Type** tab (the dialogs are identical). Setting both ensures PHPStorm activates the server reliably.

<img width="779" height="645" alt="PHPStorm new language server dialog" src="https://github.com/user-attachments/assets/2da88e68-d012-476e-82e7-977dbfcd9653" />

<img width="779" height="645" alt="PHPStorm language server mapping dialog" src="https://github.com/user-attachments/assets/62358f9e-973c-487d-ac17-098d7dab007e" />

</details>

<details>
<summary><b>Sublime Text</b></summary>

1. **Install the LSP package.** Open the Command Palette (`Ctrl+Shift+P` on Linux/Windows, `Cmd+Shift+P` on macOS), type `Package Control: Install Package`, press Enter, then search for `LSP` and install it.

2. **Configure PHPantom.** Open the Command Palette again and type `Preferences: LSP Server Configurations`. This opens `LanguageServers.sublime-settings`. Add the following:

```json
{
  "phpantom": {
    "enabled": true,
    "command": ["phpantom_lsp"],
    "selector": "embedding.php",
    "priority_selector": "source.php"
  }
}
```

Make sure `phpantom_lsp` is on your `PATH`, or replace it with the full path to the binary.

</details>

<details>
<summary><b>Helix</b></summary>

Helix has built-in LSP support. Add PHPantom to your `languages.toml` (typically `~/.config/helix/languages.toml`):

```toml
[language-server.phpantom]
command = "phpantom_lsp"

[[language]]
name = "php"
language-servers = ["phpantom"]
```

</details>

<details>
<summary><b>Emacs (eglot)</b></summary>

> [!NOTE]
> This configuration is untested. If you get it working (or run into issues), please [open an issue](../../issues).

Eglot is built into Emacs 29+. Add to your `init.el`:

```elisp
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs
               '(php-mode . ("phpantom_lsp"))))
```

Then open a PHP file and run `M-x eglot`.

</details>

<details>
<summary><b>Emacs (lsp-mode)</b></summary>

> [!NOTE]
> This configuration is untested. If you get it working (or run into issues), please [open an issue](../../issues).

Add to your `init.el`:

```elisp
(with-eval-after-load 'lsp-mode
  (lsp-register-client
   (make-lsp-client
    :new-connection (lsp-stdio-connection '("phpantom_lsp"))
    :activation-fn (lsp-activate-on "php")
    :server-id 'phpantom)))
```

Then open a PHP file and run `M-x lsp`.

</details>

<details>
<summary><b>Kate</b></summary>

> [!NOTE]
> This configuration is untested. If you get it working (or run into issues), please [open an issue](../../issues).

Kate (KDE) has built-in LSP support. Open **Settings → Configure Kate → LSP Client → User Server Settings** and add:

```json
{
  "servers": {
    "php": {
      "command": ["/path/to/phpantom_lsp"],
      "url": "https://github.com/PHPantom-dev/phpantom_lsp"
    }
  }
}
```

</details>

For AI coding agent setup, see [Agent Setup](agent-setup.md).
