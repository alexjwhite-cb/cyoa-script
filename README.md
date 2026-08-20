# cyoa — A Declarative Language Engine for Choose-Your-Own-Adventure Games

> **Version 0.5.0** — Phase 1 + 2 + 3 + 4 complete (incl. LSP server + mobile docs).

## What Is This?

`cyoa` is a **declarative language and engine** for writing Choose-Your-Own-
Adventure-style stories in games. Writers and game designers use a simple,
prose-like DSL (no programming required) to create branching narratives with
stats, tags, reusable consequences, and prerequisites. Stories compile ahead of
time into compact bytecode that a lightweight VM interprets at runtime —
fast enough for large stories across web, desktop, mobile, and game engines.

## Why?

- **Writers, not programmers**: The DSL reads like writing a story outline
- **Fast text output**: Zero-copy string table delivers text without allocation
- **All platforms from one codebase**: Rust → WASM (web/Unity/Godot) or native
  C-ABI (desktop/mobile)
- **Large stories**: 10,000+ events supported; VM is register-based and lean
- **Writer-friendly features**: `{{templating}}`, reusable `effect` blocks,
  AND/OR + stat threshold `requires:`, choice history, multi-story

## Quick Start

```bash
# Install (requires Rust 1.80+)
git clone https://github.com/yourname/cyoa.git
cd cyoa

# Compile a story
cargo run -p cyoa-cli -- compile examples/forest_adventure.cyoa

# Play in the CLI
cargo run -p cyoa-cli -- play examples/forest_adventure.cyoa.bc

# Run tests
cargo test

# Build WASM (for web/Unity/Godot)
cargo build -p cyoa-wasm --target wasm32-unknown-unknown

# Run the web demo
python3 -m http.server 8080 --directory .
# Then open http://localhost:8080/web-demo/
```

## Example Story

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
```

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

```
cyoa-ast/       AST types (foundation)
cyoa-bytecode/  Binary format + postcard serialization
cyoa-compiler/  Pest grammar + parser + bytecode codegen
cyoa-runtime/   VM + PlayerState + StoryCursor
cyoa-wasm/      WASM bindings (Phase 2)
cyoa-native/    C-ABI bindings (Phase 3)
cyoa-cli/       CLI: compile, play, validate
cyoa-lsp/       LSP server (Phase 4)
```

## Web Demo (WASM)

A working demo is in [`web-demo/`](web-demo/). It loads two stories
(`forest_adventure.cyoa` and `tavern_tales.cyoa`) into a `StoryCatalog`,
filters them by tags, and plays them with zero-copy text rendering.

```bash
# From the project root:
python3 -m http.server 8080
# Open http://localhost:8080/web-demo/
```

### WASM API (JavaScript)

```typescript
import init, { WasmStoryCatalog, WasmEngine } from './cyoa-wasm/pkg/cyoa_wasm.js';
await init();

// Register and filter stories by tag
const catalog = new WasmStoryCatalog();
catalog.register(bytecodeBytes);   // Uint8Array from fetch

catalog.storiesWithTag("fantasy");          // array of { name, tags }
catalog.storiesWithAllTags('["fantasy","exploration"]');

// Create and play a story engine
const engine = catalog.createEngine(0);

const event = engine.getCurrentEvent();
// { id: "old_ruins", text: [...], choices: [...] }

engine.makeChoice(0);      // { effectText: [...], gameStateChanged: true }
engine.getStateJson();     // for save files
engine.setStateJson(json); // to load

// Zero-copy text (returns Uint8Array view into WASM memory)
const bytes = engine.currentEventTextBytes();
const text = new TextDecoder().decode(bytes);
```

Full API reference: [`docs/api-reference.md`](docs/api-reference.md)

## Unity Integration (C-ABI)

`cyoa-native` compiles to a native C-ABI shared library for use with
Unity, Godot, desktop C/C++ apps, and mobile platforms.

```bash
# Build the native plugin
cargo build -p cyoa-native --release

# Copy to Unity project
cp target/release/libcyoa_native.* YourUnityProject/Assets/Plugins/
```

A C# wrapper (`CyoaEngine.cs`) provides a rich, memory-safe API mirroring
the WASM/JS API. The Unity demo (`CyoaUnityDemo.cs`) shows two stories
loaded simultaneously with tag filtering, choice handling, stats display,
and save/load.

```csharp
using Cyoa;

// Multi-story with tag filtering
using var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(File.ReadAllBytes("forest_adventure.cyoa.bc"));
catalog.RegisterStory(File.ReadAllBytes("tavern_tales.cyoa.bc"));

var fantasyStories = catalog.StoriesWithTag("fantasy");
using var engine = catalog.CreateEngineByName("ForestAdventure");

string text = engine.CurrentEventText;
string[] choices = engine.CurrentChoices;
engine.MakeChoice(0);

string saveJson = engine.GetStateJson();  // for saving
engine.SetStateJson(saveJson);            // for loading
```

Unity integration guide: [`bindings/csharp/UnityDemo/README.md`](bindings/csharp/UnityDemo/README.md)

### Mobile (Android + iOS)

Cross-compile the native library for mobile platforms:

```bash
# Android (requires cargo-ndk)
cargo ndk -t arm64-v8a -t armeabi-v7a build -p cyoa-native --release

# iOS (requires cargo-lipo, macOS + Xcode)
cargo lipo -p cyoa-native --release
```

Full setup guide: [`docs/mobile.md`](docs/mobile.md)

## Godot Integration (GDScript + C#)

`cyoa-native` provides C-ABI bindings for Godot 4.x via a C# wrapper
(`bindings/godot/csharp/`) and a GDScript convenience script
(`bindings/godot/gdscript/`). Since GDScript cannot call native C functions
directly, the C# class serves as the FFI bridge.

```bash
# Build the native plugin (same as Unity)
cargo build -p cyoa-native --release
# Copy target/release/libcyoa_native.* to YourProject/addons/cyoa/native/
```

```gdscript
# GDScript usage
var engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
engine.load_from_path("res://addons/cyoa/native/forest_adventure.cyoa.bc")
print(engine.get_current_event_text())
print(engine.get_current_choices())
engine.make_choice(0)
```

```csharp
// C# direct usage
var engine = CyoaEngine.LoadFromFile("res://addons/cyoa/native/forest_adventure.cyoa.bc");
GD.Print(engine.CurrentEventText);
```

Godot integration guide: [`bindings/godot/README.md`](bindings/godot/README.md)

## Language Server Protocol (LSP)

The `cyoa-lsp` crate provides a language server for `.cyoa` files with syntax
diagnostics, hover info, and completion. It communicates over stdio (JSON-RPC),
making it compatible with any LSP-compatible editor (VS Code, Neovim, etc.).

```bash
# Build the LSP server
cargo build -p cyoa-lsp

# Example: configure VS Code (settings.json)
{
  "cyoa-lsp": {
    "command": ["cyoa-lsp"],
    "languageId": "cyoa",
    "uri": "file:///path/to/cyoa-lsp"
  }
}
```

**Features:**
- Real-time syntax error diagnostics on file open/change
- Hover shows story metadata: name, tags, stats, flags, effects, events
- Completion suggests event IDs, stat/flag/effect names, and DSL keywords
- Error hover shows line/column info

## Documentation

| Document | Purpose |
|----------|---------|
| [SPEC.md](SPEC.md) | Language specification (canonical) |
| [CLAUDE.md](CLAUDE.md) | Claude Code context (build/test) |
| [docs/syntax.md](docs/syntax.md) | Extended syntax reference |
| [docs/api-reference.md](docs/api-reference.md) | API reference index + architecture overview |
| [docs/wasm-api.md](docs/wasm-api.md) | WASM / JavaScript API |
| [docs/c-abi-api.md](docs/c-abi-api.md) | C-ABI + C# (Unity) API |
| [docs/rust-api.md](docs/rust-api.md) | Rust runtime API + bytecode format + CLI |
| [docs/gdscript-api.md](docs/gdscript-api.md) | GDScript API (Godot) |
| [docs/mobile.md](docs/mobile.md) | Mobile cross-compilation (Android/iOS) |

## License

Dual-licensed under MIT or Apache-2.0, at your option.

---

*This project was written and maintained with the help of AI (Anthropic Claude).*
The language design, Rust implementation, WASM/C-ABI bindings, C#/GDScript
bindings, LSP server, and documentation were developed in collaboration with
Claude Code across multiple implementation phases.
