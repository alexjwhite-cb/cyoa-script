# Installation Instructions

This guide covers how to obtain and use the CYOA engine across different
platforms and game engines. The engine is distributed as pre-built artifacts on
[GitHub Releases](https://github.com/alexjwhite/cyoa-script/releases), so you
don't need to compile from source unless you're contributing.

## Table of Contents

- [Latest Release](#latest-release)
- [CLI Tool](#cli-tool)
- [Web / Browser](#web--browser)
- [Unity (C-ABI)](#unity-c-abi)
- [Godot](#godot)
- [Native C/C++](#native-c-c)
- [Rust](#rust)
- [Compiling Stories](#compiling-stories)
- [Building from Source](#building-from-source)

---

## Latest Release

Pre-built artifacts for each release are published to
[GitHub Releases](https://github.com/alexjwhite/cyoa-script/releases).

Each release includes:

| Artifact | Contents |
|----------|----------|
| `cyoa-cli-*` | Standalone CLI binary (compile, play, validate) |
| `cyoa-wasm/*` | WASM module (`cyoa_wasm.js` + `cyoa_wasm_bg.wasm`) + TypeScript declarations |
| `cyoa-native-*` | Native C-ABI shared/static library + C header (`cyoa.h`) |
| `cyoa-native-android/` | Android ABIs (`arm64-v8a`, `armeabi-v7a`) |
| `cyoa-native-ios/` | Universal iOS static library (`libcyoa_native.a`) |
| `*.cyoa.bc` | Compiled example story bytecode files |

Download the appropriate files for your platform.

---

## CLI Tool

The CLI binary provides three commands for story authors and testers.

### Install

Download the `cyoa-cli-<platform>` binary from the latest
[release page](https://github.com/alexjwhite/cyoa-script/releases/latest).

### Usage

```bash
# Compile a story from .cyoa source to bytecode
cyoa compile examples/forest_adventure.cyoa

# Play interactively in the terminal
cyoa play examples/forest_adventure.cyoa.bc

# Validate syntax without producing output
cyoa validate examples/forest_adventure.cyoa
```

---

## Web / Browser

The WASM build lets you embed the CYOA engine in any web page or game engine
that supports Web Workers / ES modules.

### Install

1. Download `cyoa-wasm/*` from the latest release.
2. Place the files where your web server can serve them:

```
your-web-project/
├── cyoa_wasm.js      # WASM JS bindings
├── cyoa_wasm_bg.wasm # The actual WASM binary
├── cyoa_wasm.d.ts    # TypeScript declarations
└── stories/
    ├── forest_adventure.cyoa.bc
    └── tavern_tales.cyoa.bc
```

### Usage

```html
<script type="module">
  import init, { WasmStoryCatalog, WasmEngine } from './cyoa_wasm.js';
  await init();

  // Register and discover stories by tag
  const catalog = new WasmStoryCatalog();
  const resp = await fetch('./stories/forest_adventure.cyoa.bc');
  catalog.register(new Uint8Array(await resp.arrayBuffer()));

  // Create an engine and play
  const engine = catalog.createEngineByName("ForestAdventure");
  const event = engine.getCurrentEvent();
  console.log(event.text, event.choices);
  engine.makeChoice(0);
</script>
```

**Live demo**: Visit the
[web demo](https://cyoa-script.github.io/web-demo/) on GitHub Pages.

---

## Unity (C-ABI)

The native C-ABI library integrates with Unity via a C# wrapper that uses
`DllImport` to call the native functions.

### Install

1. Download `cyoa-native-<platform>` from the latest release.
2. Copy the shared library to your Unity project:

```
YourUnityProject/
└── Assets/
    └── Plugins/
        ├── libcyoa_native.so   # Linux (or .dll / .dylib)
        └── cyoa.h              # C header (optional)
```

### Setup

1. Copy `bindings/csharp/CyoaEngine.cs` into your Unity project
   (e.g., `Assets/Scripts/Cyoa/`).
2. Copy compiled `.cyoa.bc` files into `Assets/StreamingAssets/`.

### Usage

```csharp
using Cyoa;

// Load and register stories
var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(File.ReadAllBytes(
    "Assets/StreamingAssets/forest_adventure.cyoa.bc"));

// Discover by tags
StoryInfo[] fantasy = catalog.StoriesWithTag("fantasy");

// Create engine and play
using var engine = catalog.CreateEngineByName("ForestAdventure");
Debug.Log(engine.CurrentEventText);
Debug.Log(string.Join("\n", engine.CurrentChoices));
engine.MakeChoice(0);

// Save / load
string saveJson = engine.GetStateJson();
```

Full integration guide: [`bindings/csharp/README.md`](../bindings/csharp/README.md)

---

## Godot

The C-ABI library works with Godot 4.x via a C# wrapper (FFI bridge) and a
thin GDScript convenience script.

### Install

1. Download `cyoa-native-<platform>` from the latest release.
2. Copy the library to your Godot addon folder:

```
YourGodotProject/
├── addons/
│   └── cyoa/
│       ├── cyoa_engine.gd       # GDScript wrapper
│       ├── scripts/
│       │   └── CyoaEngine.cs    # C# FFI bridge
│       └── native/
│           ├── libcyoa_native.so   # (or .dll / .dylib)
│           └── forest_adventure.cyoa.bc
```

### Usage (GDScript)

```gdscript
var engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
engine.load_from_path("res://addons/cyoa/native/forest_adventure.cyoa.bc")

print(engine.get_current_event_text())
print(engine.get_current_choices())
engine.make_choice(0)

var save_json = engine.get_state_json()  # for saving
```

Full integration guide: [`bindings/godot/README.md`](../bindings/godot/README.md)

---

## Native C/C++

Use the C-ABI shared library directly from C or C++.

### Install

Download `cyoa-native-<platform>` which includes:
- `libcyoa_native.so` / `cyoa_native.dll` / `libcyoa_native.dylib`
- `cyoa.h` (C header)

### Usage

```c
#include "cyoa.h"

/* Load bytecode */
uint8_t* bytes = /* ... read .cyoa.bc file ... */;
CyoaEngine* engine = cyoa_create(bytes, len);

/* Query current event */
printf("Event: %s\n", cyoa_current_event_id(engine));
printf("%s\n", cyoa_current_event_text(engine));

/* Present choices and get input */
int count = cyoa_current_choice_count(engine);
for (int i = 0; i < count; i++) {
    printf("  [%d] %s\n", i, cyoa_choice_text(engine, i));
}

/* Make a choice */
int choice = /* ... user input ... */;
cyoa_make_choice(engine, choice);

/* Save / load */
char* state = cyoa_get_state_json(engine);
/* ... store state ... */
cyoa_set_state_json(engine, saved_state);
cyoa_free_string(state);

/* Cleanup */
cyoa_destroy(engine);
```

C# / Unity wrapper: [`bindings/csharp/CyoaEngine.cs`](../bindings/csharp/CyoaEngine.cs)
GDScript wrapper: [`bindings/godot/gdscript/cyoa_engine.gd`](../bindings/godot/gdscript/cyoa_engine.gd)

---

## Rust

If you're building a native Rust application, use the `cyoa-runtime` crate
directly — no FFI overhead.

### Install

Add to your `Cargo.toml`:

```toml
[dependencies]
cyoa-runtime = { git = "https://github.com/alexjwhite/cyoa-script", tag = "v0.6.0" }
cyoa-bytecode = { git = "https://github.com/alexjwhite/cyoa-script", tag = "v0.6.0" }
```

Or use the pre-built runtime crate from crates.io if published.

### Usage

```rust
use cyoa_runtime::Engine;
use cyoa_bytecode::Bytecode;

let bytes = std::fs::read("forest_adventure.cyoa.bc")?;
let bc = Bytecode::from_bytes(&bytes)?;
let mut engine = Engine::new(bc);

// Play
println!("{}", engine.current_event_id());
for para in engine.current_event_text() {
    println!("{}", para);
}
let choices = engine.current_choices();
for (i, choice) in choices.iter().enumerate() {
    println!("  [{}] {}", i, choice);
}

engine.make_choice(0);

// Save / load
let save = engine.get_state_json();
let mut engine2 = Engine::new(bc);
engine2.set_state_json(&save)?;
```

For compiling stories from `.cyoa` source, use `cyoa-compiler`:

```toml
cyoa-compiler = { git = "https://github.com/alexjwhite/cyoa-script" }
```

```rust
use cyoa_compiler::Compiler;

let mut compiler = Compiler::new();
let bytecode = compiler.compile_file("forest_adventure.cyoa")?;
std::fs::write("forest_adventure.cyoa.bc", bytecode.to_bytes()?)?;
```

Rust API reference: [`docs/rust-api.html`](./rust-api.html)

---

## Compiling Stories

Stories are compiled from `.cyoa` source files to `.cyoa.bc` bytecode using the
CLI:

```bash
# Using the release binary
cyoa compile my_story.cyoa
# → produces my_story.cyoa.bc

# Or from source via cargo
cargo run -p cyoa-cli -- compile my_story.cyoa
```

The `.cyoa.bc` file is a single self-contained binary — all imports are merged
at compile time, so no runtime file access is needed.

---

## Building from Source

If you're contributing or need to build from source, see the
[Quick Start](#building-from-source) section of the
[README](https://github.com/alexjwhite/cyoa-script#quick-start).

```bash
# Clone and build
git clone https://github.com/alexjwhite/cyoa-script.git
cd cyoa-script

# Build all crates
cargo build --release

# Build WASM
cargo build -p cyoa-wasm --target wasm32-unknown-unknown

# Build native C-ABI
cargo build -p cyoa-native --release

# Run tests
cargo test
```

For mobile cross-compilation (Android/iOS), see the
[Mobile Guide](./mobile.html).
