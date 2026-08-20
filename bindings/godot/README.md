# CYOA Engine — Godot Integration

Integrate the CYOA story engine into Godot 4.x using the C-ABI native library.

## Overview

| Component | Purpose |
|-----------|---------|
| `cyoa-native` (Rust) | `extern "C"` FFI layer — compiles to a `.dll`/`.so`/`.dylib` native plugin |
| `csharp/CyoaEngine.cs` | C# wrapper — `DllImport` calls + string marshaling + JSON parsing (Godot.NET) |
| `gdscript/cyoa_engine.gd` | GDScript wrapper — delegates to the C# class |
| `native/` | Where the compiled `.cyoa.bc` files and native library go |

Godot's GDScript cannot call native C functions directly, so the C# wrapper
serves as the FFI bridge. GDScript code calls a thin GDScript wrapper that
delegates to the C# class.

## Prerequisites

- **Godot 4.x** with .NET support (Godot.NET / Mono build)
- **A compiled `.cyoa.bc` story file** — compile with:
  ```bash
  cargo run -p cyoa-cli -- compile examples/forest_adventure.cyoa
  ```

## Quick Start

### 1. Build the native plugin

```bash
cargo build -p cyoa-native --release
```

| Platform | File |
|----------|------|
| Linux | `target/release/libcyoa_native.so` |
| Windows | `target/release/cyoa_native.dll` |
| macOS | `target/release/libcyoa_native.dylib` |

### 2. Set up your Godot project

```
YourGodotProject/
├── addons/
│   └── cyoa/
│       ├── cyoa_engine.gd        # GDScript wrapper
│       ├── scripts/
│       │   └── CyoaEngine.cs     # C# wrapper
│       └── native/
│           ├── cyoa_native.dll   # or .so / .dylib
│           ├── forest_adventure.cyoa.bc
│           └── tavern_tales.cyoa.bc
```

1. Copy `cyoa_engine.gd` to `addons/cyoa/`
2. Copy `CyoaEngine.cs` to `addons/cyoa/scripts/`
3. Copy the native library and `.cyoa.bc` files to `addons/cyoa/native/`
4. **Project → Project Settings → Mono → Runtime → Assembly Definitions** — add
   `CyoaEngine.cs` to your project's C# source.

### 3. GDScript usage

```gdscript
# Load a single story
var engine_gd = preload("res://addons/cyoa/cyoa_engine.gd").new()
engine_gd.load_from_path("res://addons/cyoa/native/forest_adventure.cyoa.bc")

# Read the current event
print(engine_gd.get_current_event_text())
var choices = engine_gd.get_current_choices()

# Make a choice
engine_gd.make_choice(0)
print(engine_gd.get_current_event_text())

# Save / load
var save_json = engine_gd.get_state_json()
engine_gd.set_state_json(save_json)
```

### 4. Multi-story with tag filtering

```gdscript
# Use the GDScript wrapper in catalog mode
var catalog = preload("res://addons/cyoa/cyoa_engine.gd").new()
catalog.register_story("res://addons/cyoa/native/forest_adventure.cyoa.bc")
catalog.register_story("res://addons/cyoa/native/tavern_tales.cyoa.bc")

# Discover and filter stories
var fantasy_stories = catalog.stories_with_tag("fantasy")
print(catalog.story_count())  # 2

# Create an engine by name
var engine = catalog.create_engine_by_name("ForestAdventure")
print(engine.get_current_event_text())
```

### 5. C# direct usage (for C# scripts)

```csharp
using Cyoa.Godot;

// Load directly from a .cyoa.bc file
var engine = CyoaEngine.LoadFromFile("res://addons/cyoa/native/forest_adventure.cyoa.bc");

GD.Print(engine.CurrentEventText);
string[] choices = engine.CurrentChoices;
engine.MakeChoice(0);

string saveJson = engine.GetStateJson();
engine.SetStateJson(saveJson);
```

## API Reference

### `CyoaEngineGD` (GDScript)

| Method | Returns | Description |
|--------|---------|-------------|
| `register_story(path)` | `void` | Register a story in **catalog mode** |
| `register_story_from_bytes(bytes)` | `void` | Register from bytecode bytes (catalog mode) |
| `story_count()` | `int` | Number of registered stories (catalog mode) |
| `list_stories()` | `Array` | All stories (catalog mode) |
| `stories_with_tag(tag)` | `Array` | Stories with a tag (catalog mode) |
| `stories_with_all_tags(tags)` | `Array` | Stories with all tags (catalog mode) |
| `stories_with_any_tags(tags)` | `Array` | Stories with any tag (catalog mode) |
| `create_engine(index)` | `Object` | Create engine from catalog (catalog mode) |
| `create_engine_by_name(name)` | `Object` | Create engine by name (catalog mode) |
| `load_from_path(path)` | `Error` | Load a story from a `.cyoa.bc` file |
| `load_from_bytes(bytes)` | `Error` | Load from a `PackedVectorByteArray` |
| `get_current_event_id()` | `String` | Internal event ID (name) |
| `get_current_event_text()` | `String` | Event text (paragraphs joined by `\n`) |
| `get_choice_count()` | `int` | Number of available choices |
| `get_choice_text(index)` | `String` | Text of a specific choice |
| `get_current_choices()` | `PackedStringArray` | All current choice texts |
| `make_choice(index)` | `void` | Apply a choice (0-based) |
| `get_last_effect_text()` | `String` | Effect text from the last choice |
| `get_history_length()` | `int` | History entry count |
| `get_history_entry(index)` | `Dictionary` | One entry: `{ event_id, choice_index, choice_text }` |
| `get_all_history()` | `Array` | All history entries |
| `get_state_json()` | `String` | Full state as JSON (save files) |
| `set_state_json(json)` | `void` | Restore state from JSON |
| `get_available_events()` | `PackedStringArray` | All event IDs |
| `can_access_event(id)` | `bool` | Whether an event is reachable |
| `get_stats()` | `Dictionary` | Stat name → value |
| `get_stat(name)` | `int` | Individual stat value |
| `get_story_tags()` | `PackedStringArray` | Story-level tags (static) |
| `get_tags()` | `PackedStringArray` | Runtime tags applied during play |
| `get_flags()` | `PackedStringArray` | Runtime flags currently set |

### `CyoaEngine` (C#)

Mirrors the [C# Unity wrapper API](../csharp/README.md) with these additions:

| Method | Returns | Description |
|--------|---------|-------------|
| `CyoaEngine.LoadFromFile(path)` | `CyoaEngine` | Load from a Godot resource path |
| `CyoaEngine(byte[] bytecode)` | — | Create from raw bytecode bytes |
| All properties from the Unity wrapper (CurrentEventId, CurrentEventText, etc.) | — | Same as Unity API |

### `CyoaStoryCatalog` (C#)

Same API as the Unity `CyoaStoryCatalog` — `RegisterStory`, `StoriesWithTag`,
`StoriesWithAllTags`, `StoriesWithAnyTags`, `CreateEngine`, `CreateEngineByName`.

## Memory management

The C# wrapper implements `IDisposable` and frees native handles on `Dispose()`.
Always wrap in a `using` statement or call `Dispose()` when done:

```csharp
using var engine = CyoaEngine.LoadFromFile("res://story.cyoa.bc");
// ... use engine ...
// Dispose called automatically at end of using block
```

## Platform notes

| Platform | Plugin file | Destination |
|---|---|---|
| Windows (x64) | `cyoa_native.dll` | `addons/cyoa/native/` |
| Linux | `libcyoa_native.so` | `addons/cyoa/native/` |
| macOS | `libcyoa_native.dylib` | `addons/cyoa/native/` |

For mobile (Android/iOS), cross-compile the native library. See
[docs/mobile.md](../../docs/mobile.md) for full instructions including
Unity/Godot integration.

**Android**:

```bash
cargo ndk -t arm64-v8a -t armeabi-v7a build -p cyoa-native --release
```

**iOS**:

```bash
cargo lipo -p cyoa-native --release
```

## Troubleshooting

**"DllNotFoundException: cyoa_native"**
- Ensure the `.dll`/`.so`/`.dylib` file is in `addons/cyoa/native/`
- Verify the architecture matches (x86_64 vs ARM64)

**"Failed to create CYOA engine"**
- Verify the `.cyoa.bc` file exists and is valid
- Run `cyoa compile story.cyoa` before using the file

**"Could not load native library"** (Godot.NET)
- Ensure Godot 4.x with .NET is installed (not the GDScript-only build)
- Check the Project Settings → Mono → Runtime configuration

**GDScript can't find CyoaEngine.cs**
- Ensure the `.cs` file is in a directory included by the C# project
- Reimport the project (Godot → Project → Tools → Reimport C# Project)
