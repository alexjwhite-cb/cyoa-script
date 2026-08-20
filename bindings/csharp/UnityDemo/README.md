# CYOA Unity Integration

This guide explains how to integrate the CYOA story engine into a Unity project using the C# wrapper (`CyoaEngine.cs`) that calls the native C-ABI (`cyoa-native` crate).

## Overview

| Component | Purpose |
|-----------|---------|
| `cyoa-native` (Rust) | `extern "C"` FFI layer — compiles to a `.dll`/`.so`/`.dylib` native plugin |
| `CyoaEngine.cs` (C#) | Thin wrapper around the C-ABI — `DllImport` calls + string marshaling |
| `CyoaUnityDemo.cs` (C#) | Demo MonoBehaviour showing a full play session |
| `StreamingAssets/` | Folder where compiled `.cyoa.bc` files live |

## Build the native plugin

```bash
# From the project root
cargo build -p cyoa-native --release
```

This produces:
- **Linux**: `target/release/libcyoa_native.so`
- **Windows**: `target\release\cyoa_native.dll`
- **macOS**: `target/release/libcyoa_native.dylib`

Copy the resulting `.dll`/`.so`/`.dylib` into `Assets/Plugins/` inside your Unity project.

## Prerequisites

- Unity 2022.2 or later (for `System.Text.Json` support)
- TextMeshPro package (for the UI demo)

## Quick start

1. Copy `CyoaEngine.cs` and `CyoaUnityDemo.cs` into your Unity project's `Assets/` folder.
2. Copy compiled `.cyoa.bc` files into `Assets/StreamingAssets/`.
3. Create a scene with the UI hierarchy described in `CyoaUnityDemo.cs` comments.
4. Assign UI references in the Inspector.

### Minimal usage

```csharp
using Cyoa;

// Load a story from a compiled bytecode file
byte[] bytecode = System.IO.File.ReadAllBytes(
    System.IO.Path.Combine(Application.streamingAssetsPath, "forest_adventure.cyoa.bc"));

using var engine = new CyoaEngine(bytecode);

// Read the current event
string eventId = engine.CurrentEventId;
string text = engine.CurrentEventText;
string[] choices = engine.CurrentChoices;

// Present choices to the player ...

// Apply a choice (0-based index)
engine.MakeChoice(0);

// The engine has now advanced to the next event
Console.WriteLine(engine.CurrentEventText);

// Save state
string saveJson = engine.GetStateJson();

// Load state later
engine.SetStateJson(saveJson);
```

### Multi-story with tag filtering

```csharp
using Cyoa;

// Create a catalog and register stories
using var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(File.ReadAllBytes("Assets/StreamingAssets/forest_adventure.cyoa.bc"));
catalog.RegisterStory(File.ReadAllBytes("Assets/StreamingAssets/tavern_tales.cyoa.bc"));

// Filter by tags
StoryInfo[] fantasyStories = catalog.StoriesWithTag("fantasy");
StoryInfo[] multiTagStories = catalog.StoriesWithAllTags(new[] { "fantasy", "exploration" });

// Create an engine for a specific story
using var engine = catalog.CreateEngineByName("ForestAdventure");
```

## API reference

### `CyoaEngine`

| Method / Property | Returns | Description |
|---|---|---|
| `CyoaEngine(byte[] bytecode)` | — | Create an engine from compiled bytecode |
| `CurrentEventId` | `string` | Internal event ID (name) |
| `CurrentEventText` | `string` | Event text paragraphs joined by `\n` |
| `ChoiceCount` | `int` | Number of available choices |
| `GetChoiceText(int index)` | `string?` | Text of a specific choice |
| `CurrentChoices` | `string[]` | All current choice texts |
| `MakeChoice(int index)` | — | Apply a choice (0-based) |
| `LastEffectText` | `string` | Effect text from the last choice |
| `HistoryLength` | `int` | Number of history entries |
| `GetHistoryEntry(int index)` | `ChoiceHistoryEntry?` | One history entry |
| `GetAllHistory()` | `ChoiceHistoryEntry[]` | All history entries |
| `GetStateJson()` | `string` | Full state as JSON (for save files) |
| `SetStateJson(string json)` | — | Restore state from JSON |
| `GetAvailableEvents()` | `string[]` | All event IDs in the story |
| `CanAccessEvent(string id)` | `bool` | Whether an event is reachable |
| `GetStats()` | `Dictionary<string,int>` | All stats as name→value |
| `GetStat(string name)` | `int` | Value of a specific stat |
| `GetStoryTags()` | `string[]` | Story-level tags (static) |
| `GetTags()` | `string[]` | Runtime tags |
| `GetFlags()` | `string[]` | Runtime flags |
| `Dispose()` | — | Free the native handle |

### `CyoaStoryCatalog`

| Method / Property | Returns | Description |
|---|---|---|
| `CyoaStoryCatalog()` | — | Create empty catalog |
| `StoryCount` | `int` | Number of registered stories |
| `RegisterStory(byte[] bytecode)` | `bool` | Register a compiled story |
| `ListStories()` | `StoryInfo[]` | All registered stories |
| `StoriesWithTag(string tag)` | `StoryInfo[]` | Stories with a specific tag |
| `StoriesWithAllTags(string[] tags)` | `StoryInfo[]` | Stories with all tags |
| `StoriesWithAnyTags(string[] tags)` | `StoryInfo[]` | Stories with any tag |
| `CreateEngine(int index)` | `CyoaEngine` | Create engine from catalog entry |
| `CreateEngineByName(string name)` | `CyoaEngine` | Create engine by story name |
| `Dispose()` | — | Free the native handle |

### Data types

#### `StoryInfo`
```csharp
public struct StoryInfo {
    public string Name;      // Story name (e.g. "ForestAdventure")
    public string[] Tags;    // Story-level tags (e.g. ["fantasy", "exploration"])
}
```

#### `ChoiceHistoryEntry`
```csharp
public struct ChoiceHistoryEntry {
    public string EventId;     // ID of the event where the choice was made
    public int ChoiceIndex;    // 0-based index of the chosen option
    public string ChoiceText;  // Text of the chosen option
}
```

## Memory management

The C# wrapper handles all native memory automatically:
- All `IntPtr` values are copied to managed `string` before the native memory is freed.
- `CyoaEngine` and `CyoaStoryCatalog` implement `IDisposable` — always wrap in `using` statements.
- The native plugin handles its own internal string buffers.

## Platform notes

| Platform | Plugin file | Notes |
|---|---|---|
| Windows (x64) | `cyoa_native.dll` | Place in `Assets/Plugins/x86_64/` |
| Linux | `libcyoa_native.so` | Place in `Assets/Plugins/` |
| macOS (Intel) | `libcyoa_native.dylib` | Place in `Assets/Plugins/` |
| macOS (Apple Silicon) | `libcyoa_native.dylib` | Universal binary recommended |
| Android | `libcyoa_native.so` | Arm64-v8a + armeabi-v7a |
| iOS | `libcyoa_native.a` | Statically linked (use `staticlib` build) |

For mobile platforms, cross-compile the native library:
```bash
# Android
cargo ndk -t arm64-v8a -t armeabi-v7a build -p cyoa-native --release

# iOS
cargo lipo --targets aarch64-apple-ios,armv7-apple-ios -p cyoa-native --release
```

## Troubleshooting

**"DllNotFoundException: cyoa_native"**
- Ensure the `.dll`/`.so`/`.dylib` file is copied to `Assets/Plugins/`.
- On Windows, check that the architecture (x86 vs x64) matches your Unity build target.

**"Failed to create CYOA engine"**
- Verify the `.cyoa.bc` file is valid. Run `cargo run -p cyoa-cli -- compile story.cyoa` first.
- Check that the bytecode file exists in `StreamingAssets/`.

**UTF-8 text shows garbled characters**
- Ensure the C# wrapper uses `Encoding.UTF8` (it does by default via `StringMarshal.PtrToUtf8String`).
