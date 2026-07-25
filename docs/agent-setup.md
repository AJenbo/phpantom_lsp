# Agent Setup

AI coding agents can use PHPantom for the same code intelligence editors get. The agent starts `phpantom_lsp` over LSP and consults it (diagnostics, go-to-definition, hover types) as it reads and edits PHP, so its changes are grounded in real type information instead of guesses.

## Claude Code

Install the PHPantom plugin from its marketplace:

```
/plugin marketplace add PHPantom-dev/phpantom_claude
/plugin install phpantom@phpantom
```

Restart Claude Code, then open a PHP file. The plugin starts the language server automatically, downloading the correct `phpantom_lsp` binary for your platform on first use (or using one already on your `PATH`). No Docker, PHP, or Rust toolchain is required.

Beyond per-file code intelligence, the plugin puts `phpantom_lsp` on the agent's `PATH` and bundles a skill that tells Claude when to use its project-wide CLI, so it can run `phpantom_lsp analyze` (type-coverage report) and `phpantom_lsp fix` (automated fixes across the codebase) on its own. It also runs `php -l` after each edit to catch syntax errors immediately.

To use a specific binary instead of the downloaded one, set the `PHPANTOM_SERVER_PATH` environment variable to its path, or put `phpantom_lsp` on your `PATH`.

## Opencode

Add this to your `opencode.json` file. It will be invoked when the AI uses the edit tool to edit a PHP file from within Opencode.

```json
{
  "$schema": "https://opencode.ai/config.json",
  "lsp": {
    "php": {
      "command": ["phpantom_lsp", "--stdio"],
      "extensions": [".php"]
    }
  }
}
```

You can add additional extensions to invoke it on if appropriate for your project.
