# cyoa-lsp

Language Server Protocol (LSP) server for `.cyoa` files. Provides real-time
syntax error diagnostics, hover info (story metadata), and completion for
event IDs, stats, flags, effects, and DSL keywords.

Communicates over stdio (JSON-RPC), compatible with VS Code, Neovim, Helix,
and any LSP-compatible editor.

## Build

```bash
cargo build -p cyoa-lsp
```

The binary (`cyoa-lsp`) is placed in `target/debug/`.

## Features

| Feature | Trigger |
|---------|---------|
| Syntax diagnostics | File open / change |
| Hover info | Hover over a symbol |
| Completion | Trigger manually or on `.` |
| Error detail | Hover shows line/column info |

## Editor configuration

### VS Code (`settings.json`)

```json
{
  "cyoa-lsp": {
    "command": ["cyoa-lsp"],
    "languageId": "cyoa",
    "uri": "file:///path/to/cyoa-lsp"
  }
}
```

### Neovim (e.g., with `null-ls` or `nvim-lspconfig`)

```lua
require("lspconfig").cyoa_lsp.setup{}
```

LSP feature matrix: [docs/api-reference.md](../docs/api-reference.md)
