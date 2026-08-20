# CLAUDE.md — Project Context for Claude Code

> **Source of truth**: `SPEC.md` (language spec) and `README.md` (user docs).
> This file is for Claude Code context: build, test, structure, and key concepts.
> Update `SPEC.md` → `CLAUDE.md` → `README.md` → `docs/*.md` on every change.

## Quick Start

```bash
# Build everything
cargo build

# Compile a story (.cyoa → .cyoa.bc)
cargo run -p cyoa-cli -- compile examples/forest_adventure.cyoa

# Play a story in the CLI
cargo run -p cyoa-cli -- play examples/forest_adventure.cyoa.bc

# Run all tests
cargo test

# WASM build (for web/Unity/Godot)
cargo build -p cyoa-wasm --target wasm32-unknown-unknown   # raw .wasm
wasm-pack build cyoa-wasm --target web --out-dir cyoa-wasm/pkg  # JS bindings

# Run the web demo
python3 -m http.server 8080 --directory .
# Then open http://localhost:8080/web-demo/

# Native C-ABI build (for desktop/mobile)
cargo build -p cyoa-native

# Native C-ABI build (release for Unity distribution)
cargo build -p cyoa-native --release
# Copy target/release/libcyoa_native.* to Unity Assets/Plugins/

# Android cross-compilation (requires cargo-ndk)
cargo ndk -t arm64-v8a -t armeabi-v7a build -p cyoa-native --release

# iOS cross-compilation (requires cargo-lipo)
cargo lipo -p cyoa-native --release

# LSP server binary
cargo build -p cyoa-lsp
```

## Project Structure

```
cyoa/
├── Cargo.toml              # Workspace root — 8 member crates
├── CLAUDE.md               # ← This file (Claude Code context)
├── SPEC.md                 # Language spec (grammar, bytecode, VM, API)
├── README.md               # User-facing docs (quick start, examples)
├── cyoa-ast/               # AST types — foundation crate (no deps)
├── cyoa-bytecode/          # Binary bytecode format + serialization
├── cyoa-compiler/          # Pest grammar + parser + bytecode codegen
├── cyoa-runtime/           # VM + PlayerState + StoryCursor + conditions
├── cyoa-wasm/              # WASM bindings (wasm-bindgen)
├── cyoa-native/            # C-ABI bindings (extern "C")
├── cyoa-cli/               # CLI: compile, play, validate
├── cyoa-lsp/               # LSP server (editor integration)
├── web-demo/               # Interactive web demo
│   └── index.html          # WASM web demo entry point
├── bindings/               # Language bindings for game engines
│   ├── c/                  # C header for native C-ABI
│   │   └── cyoa.h          # C header (mirror of cyoa-native/include/cyoa.h)
│   └── csharp/             # C# wrapper for Unity
│       ├── CyoaEngine.cs   # C# P/Invoke wrapper
│       ├── README.md       # C# integration guide
│       └── UnityDemo/      # Unity demo
│           ├── CyoaUnityDemo.cs # Demo MonoBehaviour
│           └── README.md   # Unity scene setup + API reference
├── std/                    # Standard library packages
│   ├── healing.cyoa
│   └── combat.cyoa
├── examples/               # Example .cyoa stories
│   ├── forest_adventure.cyoa
│   ├── tavern_tales.cyoa
│   └── castle_mystery.cyoa
└── docs/                   # Extended reference
    ├── syntax.md
    └── api-reference.md
```

## Crate Dependency Graph

```
cyoa-ast        (no deps — foundational types)
  ↑
cyoa-bytecode   (deps: postcard, serde)
  ↑
cyoa-compiler   (deps: pest, cyoa-ast, cyoa-bytecode)
cyoa-runtime    (deps: cyoa-ast, cyoa-bytecode, serde_json)
  ↑
cyoa-wasm       (deps: wasm-bindgen, serde-wasm-bindgen, js-sys, cyoa-runtime, cyoa-bytecode)
cyoa-native     (deps: cyoa-runtime, cyoa-bytecode, serde_json)
cyoa-cli        (deps: clap, cyoa-compiler, cyoa-runtime, cyoa-bytecode)
cyoa-lsp        (deps: lsp-types, cyoa-compiler)
```

**Key insight**: `cyoa-compiler` and `cyoa-runtime` are independent — they share `cyoa-ast` + `cyoa-bytecode` but don't depend on each other. Compilation (build time) and execution (runtime) are separate phases.

## Core Concepts

### DSL: Writer-First, Indentation-Based, Prose-Like

```cyoa
story ForestAdventure:

  tags: fantasy, exploration

  stat hp = 50
  stat courage = 0
  stat gold = 0

  effect found_mushroom:
    + courage by 1
    text "You find a glowing mushroom."

  event old_ruins:
    tags: exploration, early_game
    "You stand before ancient stone ruins."

    choice "Enter the ruins":
      set visited_old_ruins to true
      next river_crossing

    choice "Circle around":
      uses found_mushroom
      next dense_forest

  event forest_encounter:
    requires: visited_old_ruins AND courage >= 5
    "A wolf blocks your path."

    choice "Attack the wolf":
      + courage by 2
      uses wolf_scare
      next wolf_fight

    choice "Offer food" uses found_mushroom:
      - gold by 3
      next peace_with_wolf

  event tavern:
    "The barkeep eyes your coin purse."
    "You have {{gold}} gold pieces."   # template interpolation

    choice "Buy ale (5 gold)":
      requires: gold >= 5
      - gold by 5
      "You buy an ale."
      next tavern
```

### Bytecode: Zero-Copy String Table Layout

`.cyoa.bc` is a single flat binary, structured for zero-copy mmap loading:

```
┌──────────────┐
│ Header       │  magic, version, section offsets
├──────────────┤
│ String Table │  ← all text deduplicated, interned
├──────────────┤
│ Event Index  │  event_id → offset mapping
├──────────────┤
│ Events       │  array of fixed-size event headers
├──────────────┤
│ Choices      │  choice arrays (indexed by event)
├──────────────┤
│ Effect Blocks│  reusable effects
├──────────────┤
│ Conditions   │  prerequisite tree bytecode
└──────────────┘
```

The VM returns `&str` pointing directly into the loaded bytecode — no allocation, no copy.

### VM: Register-Based Stack Machine

Instructions: `GetText`, `RenderTemplate`, `GetChoice`, `ApplyEffect`, `ChangeStat`, `SetFlag`, `ClearFlag`, `AddTag`, `CheckCondition`, `RecordHistory`, `BranchIfTrue`, `Goto`, `Return`.

The VM owns `PlayerState` (stats/flags/tags) and `StoryCursor` (current event + history). Game engines never touch state directly — they use API calls. Story-level tags are static metadata stored in the `Bytecode` struct (not runtime state) and exposed via `Engine::list_story_tags()`.

### Multi-Story: One Engine Instance Per Story

Each story = independent `Engine` instance. Game engines manage coordination (e.g., shared gold pool across quests). No shared state.

**Story discovery**: Use `StoryCatalog` (Rust) / `WasmStoryCatalog` (JS) / `CyoaCatalog` (C-ABI/C#) to register multiple compiled stories and query them by tag(s) before creating an engine for each story.

### Import System: Compile-Time Merge

All `.cyoa` imports are resolved at compile time into a single self-contained `.cyoa.bc`:
1. **Parse pass**: parse main + all transitively-imported files
2. **Merge pass**: build dependency graph, detect cycles, resolve name collisions
3. **Codegen pass**: emit unified bytecode

Supported in Phase 1: `std/...` and `./...` paths. Package registry imports deferred to Phase 2+.

## Testing

```bash
cargo test                    # all unit + integration tests
cargo test -p cyoa-compiler   # parser + codegen only
cargo test -p cyoa-runtime    # VM + state tests
```

### Test Categories

| Level | Scope | Tools |
|-------|-------|-------|
| Unit | Per-crate (parser rules, bytecode round-trip, VM steps) | `cargo test` |
| Spec compliance | Every SPEC.md example parses/produces expected output | Integration tests |
| Integration | Full `.cyoa` → bytecode → VM playthrough | `examples/` |
| Performance | 10K events, measure text-output latency | `cargo bench` (future) |

## Build & CI

```bash
cargo fmt --check           # formatting
cargo clippy -- -D warnings # lint
cargo test                  # all tests (native)
cargo clippy -p cyoa-wasm --target wasm32-unknown-unknown -- -D warnings  # WASM lint
cargo clippy -p cyoa-native -- -D warnings                                 # Native C-ABI lint

# Godot bindings: C# wrapper in bindings/godot/csharp/ delegates to the C-ABI;
# GDScript wrapper in bindings/godot/gdscript/ is a thin client (GDScript can't
# call native C directly).

# GitHub Actions CI:
# - .github/workflows/ci.yml — runs on every push/PR: fmt, clippy, tests, WASM clippy
# - .github/workflows/release.yml — triggers on git tag v*: builds all artifacts +
#   deploys web-demo to GitHub Pages (cyoa-script.github.io)

## Key Risks (from plan)

1. **Declarative paradigm shift** — writers describe "what exists" not "do this then that"
2. **First-time compiler design** — leverage pest for grammar, Crafting Interpreters for concepts
3. **Rust ownership/borrowing** — lean on `Vec`, `Box`, and `clone()` when stuck
4. **WASM zero-copy** — return pointer+length into WASM memory, NOT `String` type

## Documentation Sync Protocol

Every code change requires updating docs in this order:
1. `SPEC.md` — grammar, bytecode format, VM instructions, API, state model
2. `CLAUDE.md` — this file (build/test commands, structure)
3. `README.md` — user-facing quick start and examples
4. `docs/syntax.md` and `docs/api-reference.md` — extended references

**Doc sync gates task completion.** A change that breaks docs is incomplete.
