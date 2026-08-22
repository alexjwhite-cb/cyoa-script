# cyoa — A Declarative Language Engine for Choose-Your-Own-Adventure Games

## What Is This?

`cyoa` is a **declarative language and engine** for writing Choose-Your-Own-
Adventure-style stories in games. Writers and game designers use a simple,
prose-like DSL (no programming required) to create branching narratives with
stats, tags, reusable consequences, and prerequisites. Stories compile ahead
of time into compact bytecode that a lightweight VM interprets at runtime —
fast enough for large stories across web, desktop, mobile, and game engines.

## Why?

- **Writers, not programmers**: The DSL reads like writing a story outline
- **Fast text output**: Zero-copy string table delivers text without allocation
- **All platforms from one codebase**: Rust → WASM (web/Unity/Godot) or native
  C-ABI (desktop/mobile)
- **Large stories**: 10,000+ events supported; VM is register-based and lean
- **Writer-friendly features**: `{{templating}}`, reusable `effect` blocks,
  AND/OR + stat threshold `requires:`, choice history, multi-story

## Web Demo

An interactive web demo including two stories is available at
[alexjwhite-cb.github.io/cyoa-script/web-demo](https://alexjwhite-cb.github.io/cyoa-script/web-demo).

## Quick Start

### Install

Pre-built artifacts are published to
[GitHub Releases](https://github.com/alexjwhite-cb/cyoa-script/releases).
No Rust toolchain required — download the package for your platform:

| Target | Artifact |
|--------|----------|
| CLI tool | `cyoa-cli-<platform>` — standalone binary |
| Web (WASM) | `cyoa-wasm/*` — `cyoa_wasm.js` + `cyoa_wasm_bg.wasm` |
| Unity / Godot / C++ | `cyoa-native-<platform>` — shared library + `cyoa.h` |
| Android | `cyoa-native-android/` — ABIs for `arm64-v8a`, `armeabi-v7a` |
| iOS | `cyoa-native-ios/` — universal static library |
| Example stories | `*.cyoa.bc` — compiled bytecode files |

To compile stories from `.cyoa` source or build from source, install
[Rust 1.80+](https://rustup.rs):

```bash
git clone https://github.com/alexjwhite-cb/cyoa-script.git
cd cyoa-script
cargo build --release
```

Detailed installation for every platform and game engine:
[docs/installation.md](docs/installation.md)

### Example Story

The CYOA DSL reads like a story outline. Writers declare stats, tags, effects,
events, and choices — prerequisites are checked automatically.

```cyoa
story ForestAdventure:

  tags: fantasy, exploration

  stat hp = 50
  stat courage = 0
  stat gold = 0

  effect found_mushroom:
    + courage by 1
    text "You find a glowing mushroom. It hums softly."

  event old_ruins:
    tags: exploration, early_game
    "You stand before ancient stone ruins, half-swallowed by ivy."

    choice "Enter the ruins":
      set visited_old_ruins to true
      next river_crossing

    choice "Circle around" uses found_mushroom:
      next dense_forest

  event forest_encounter:
    requires: visited_old_ruins AND courage >= 5
    "A wolf blocks your path."

    choice "Attack the wolf":
      + courage by 2
      uses found_mushroom
      next wolf_fight

    choice "Offer food" uses found_mushroom:
      - gold by 3
      next peace_with_wolf

  event tavern:
    "The barkeep eyes your coin purse."
    "You have {{gold}} gold pieces."  # template interpolation

    choice "Buy ale (5 gold)":
      requires: gold >= 5
      - gold by 5
      "You buy an ale."
      next tavern
```

### JavaScript (WASM)

Compile the engine to WASM and load stories in any browser or web-based game
engine:

```html
<script type="module">
  import init, { WasmStoryCatalog } from './cyoa_wasm.js';
  await init();

  // Register stories from compiled .cyoa.bc bytecode
  const catalog = new WasmStoryCatalog();
  const resp = await fetch('./forest_adventure.cyoa.bc');
  catalog.register(new Uint8Array(await resp.arrayBuffer()));

  // Discover and play
  const engine = catalog.createEngineByName("ForestAdventure");
  const event = engine.getCurrentEvent();
  console.log(event.text, event.choices);
  engine.makeChoice(0);

  // Zero-copy text (returns a view into WASM linear memory)
  const bytes = engine.currentEventTextBytes();
  const text = new TextDecoder().decode(bytes);
</script>
```

Full API: [docs/wasm-api.md](docs/wasm-api.md) · Web demo: [web-demo/](web-demo/)

### C# (Unity)

Use the C-ABI native library through a memory-safe C# wrapper via `DllImport`:

```csharp
using Cyoa;

// Register multiple stories with tag-based discovery
using var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(File.ReadAllBytes(
    "Assets/StreamingAssets/forest_adventure.cyoa.bc"));

// Discover and filter
StoryInfo[] fantasy = catalog.StoriesWithTag("fantasy");

// Create an engine and play
using var engine = catalog.CreateEngineByName("ForestAdventure");
Console.WriteLine(engine.CurrentEventText);
Console.WriteLine(string.Join("\n", engine.CurrentChoices));
engine.MakeChoice(0);

// Save / load
string saveJson = engine.GetStateJson();
engine.SetStateJson(saveJson);
```

Full guide: [bindings/csharp/README.md](bindings/csharp/README.md) ·
Unity demo: [bindings/csharp/UnityDemo/README.md](bindings/csharp/UnityDemo/README.md)

### GDScript (Godot)

The C# wrapper serves as the FFI bridge from GDScript to the native C-ABI:

```gdscript
# Catalog mode — register and filter stories
var catalog = preload("res://addons/cyoa/cyoa_engine.gd").new()
catalog.register_story("res://addons/cyoa/native/forest_adventure.cyoa.bc")

var fantasy_stories = catalog.stories_with_tag("fantasy")

# Create an engine and play
var engine = catalog.create_engine_by_name("ForestAdventure")
print(engine.get_current_event_text())
print(engine.get_current_choices())
engine.make_choice(0)
```

Full guide: [bindings/godot/README.md](bindings/godot/README.md)

## Features

| Feature | Status | Description |
|---------|--------|-------------|
| Text output | ✅ | Zero-copy delivery via string table |
| Stats | ✅ | Numeric state (`i64`) tracked over time |
| Flags | ✅ | Boolean story progress markers |
| Tags | ✅ | Story + event-level labels for filtering |
| Reusable effects | ✅ | Define once, use from multiple choices |
| AND/OR prerequisites | ✅ | `requires: courage >= 5 AND hp > 0` |
| Text templating | ✅ | `{{stat}}` interpolation in prose |
| Choice history | ✅ | Serializable history of all choices |
| Multi-story | ✅ | Independent engine per story |
| File imports | ✅ | `import "std/..."` and `import "./..."` |
| WASM/Web | ✅ | Phase 2 — zero-copy text, StoryCatalog, web demo |
| Unity (C#) | ✅ | Phase 3 |
| Godot (GDScript + C#/.NET) | ✅ | Phase 4 |
| Mobile (Android + iOS) | ✅ | Phase 4.3 |
| LSP server | ✅ | Phase 4.2 |
| Articy:Draft import | ⏳ | Phase 5 |

## Architecture

```
┌────────────────┐
│  Game Engines  │  Web | Unity | Godot | Native
├────────────────┤
│  C-ABI / WASM  │
├────────────────┤
│  Runtime VM    │←── State mutations
├────────────────┤
│  .cyoa.bc      │←── zero-copy mmap
├────────────────┤
│  Compiler      │←── .cyoa source
└────────────────┘
```

### Crate Layout

Each crate has its own [`README`](cyoa-ast/README.md) with build and usage
details:

| Crate | Purpose |
|-------|---------|
| [`cyoa-ast/`](cyoa-ast/README.md) | AST types — foundation crate (no deps) |
| [`cyoa-bytecode/`](cyoa-bytecode/README.md) | Binary format + postcard serialization |
| [`cyoa-compiler/`](cyoa-compiler/README.md) | Pest grammar + parser + bytecode codegen |
| [`cyoa-runtime/`](cyoa-runtime/README.md) | VM + PlayerState + StoryCursor |
| [`cyoa-wasm/`](cyoa-wasm/README.md) | WASM bindings (ES module) |
| [`cyoa-native/`](cyoa-native/README.md) | C-ABI bindings (shared/static library) |
| [`cyoa-cli/`](cyoa-cli/README.md) | CLI: compile, play, validate |
| [`cyoa-lsp/`](cyoa-lsp/README.md) | LSP server (diagnostics, hover, completion) |

**Key insight**: `cyoa-compiler` and `cyoa-runtime` are independent — they
share `cyoa-ast` + `cyoa-bytecode` but don't depend on each other. Compilation
(build time) and execution (runtime) are separate phases.

## Language Server Protocol (LSP)

The `cyoa-lsp` crate provides a language server for `.cyoa` files with syntax
diagnostics, hover info, and completion. It communicates over stdio (JSON-RPC),
making it compatible with any LSP-compatible editor (VS Code, Neovim, etc.).

```bash
# Build the LSP server
cargo build -p cyoa-lsp
```

**Features:**
- Real-time syntax error diagnostics on file open/change
- Hover shows story metadata: name, tags, stats, flags, effects, events
- Completion suggests event IDs, stat/flag/effect names, and DSL keywords
- Error hover shows line/column info

## Development

Build everything from source:

```bash
cargo build            # all crates
cargo test             # all unit + integration tests
cargo fmt --check      # formatting
cargo clippy -- -D warnings   # lint (native)
```

## Documentation

| Document | Purpose |
|----------|---------|
| [SPEC.md](SPEC.md) | Language specification (canonical) |
| [CLAUDE.md](CLAUDE.md) | Build/test commands (Claude Code context) |
| [docs/installation.md](docs/installation.md) | Installation instructions (consuming releases) |
| [docs/syntax.md](docs/syntax.md) | Extended syntax reference |
| [docs/api-reference.md](docs/api-reference.md) | API reference index + architecture overview |
| [docs/wasm-api.md](docs/wasm-api.md) | WASM / JavaScript API |
| [docs/c-abi-api.md](docs/c-abi-api.md) | C-ABI + C# (Unity) API |
| [docs/rust-api.md](docs/rust-api.md) | Rust runtime API + bytecode format + CLI |
| [docs/gdscript-api.md](docs/gdscript-api.md) | GDScript API (Godot) |
| [docs/mobile.md](docs/mobile.md) | Mobile cross-compilation (Android/iOS) |
| [Per-crate READMEs](cyoa-ast/README.md) | Build & usage for each crate |

## License

Dual-licensed under MIT or Apache-2.0, at your option.

---

*This project was written and maintained with the help of AI (Anthropic Claude).*
The language design, Rust implementation, WASM/C-ABI bindings, C#/GDScript
bindings, LSP server, and documentation were developed in collaboration with
Claude Code across multiple implementation phases.
