# cyoa-cli

Command-line interface for the CYOA engine: compile stories from `.cyoa`
source, play them interactively, and validate syntax.

## Build

```bash
cargo build -p cyoa-cli --release
```

The binary (`cyoa`) is placed in `target/release/`.

## Commands

```bash
# Compile a story from .cyoa source to bytecode
cyoa compile examples/forest_adventure.cyoa
# → produces examples/forest_adventure.cyoa.bc

# Play interactively in the terminal
cyoa play examples/forest_adventure.cyoa.bc

# Validate syntax without producing output
cyoa validate examples/forest_adventure.cyoa
```

### Play-mode keys

| Key | Action |
|-----|--------|
| `0`–`9` | Select a choice by number |
| `s` | Save current state to JSON |
| `l` | Load state from JSON |
| `r` | Restart the story |
| `h` | Show choice history |
| `q` | Quit |

### From source

```bash
cargo run -p cyoa-cli -- compile examples/forest_adventure.cyoa
cargo run -p cyoa-cli -- play examples/forest_adventure.cyoa.bc
```

### Run tests

```bash
cargo test
```

CLI reference: [docs/rust-api.md](../docs/rust-api.md)
