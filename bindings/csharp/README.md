# CYOA Engine — C# (Unity) Bindings

A C# wrapper around the CYOA native C-ABI (`cyoa-native`). Provides a
memory-safe, idiomatic API mirroring the WASM/JavaScript interface.

## Quick Start

### 1. Build the native plugin

```bash
# From the project root:
cargo build -p cyoa-native --release
```

This produces a shared library:

| Platform | File | Location |
|----------|------|----------|
| Linux | `libcyoa_native.so` | `target/release/` |
| Windows | `cyoa_native.dll` | `target/release/` |
| macOS | `libcyoa_native.dylib` | `target/release/` |

### 2. Copy the plugin into Unity

```bash
# Copy the native plugin to your Unity project
cp target/release/libcyoa_native.* YourUnityProject/Assets/Plugins/

# Copy the C# wrapper
cp CyoaEngine.cs YourUnityProject/Assets/Scripts/
```

### 3. Use the wrapper

```csharp
using Cyoa;

// Single story
byte[] bytecode = System.IO.File.ReadAllBytes("Assets/StreamingAssets/forest_adventure.cyoa.bc");
using var engine = new CyoaEngine(bytecode);

// Read the current event
string text = engine.CurrentEventText;
string[] choices = engine.CurrentChoices;

// Make a choice (0-based index)
engine.MakeChoice(0);
Console.WriteLine(engine.CurrentEventText);

// Save / load
string saveJson = engine.GetStateJson();
engine.SetStateJson(saveJson);
```

### 4. Multi-story with tag filtering

```csharp
using Cyoa;

// Register multiple stories into a catalog
using var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(File.ReadAllBytes("Assets/StreamingAssets/forest_adventure.cyoa.bc"));
catalog.RegisterStory(File.ReadAllBytes("Assets/StreamingAssets/tavern_tales.cyoa.bc"));

// Discover stories by tag
StoryInfo[] fantasy = catalog.StoriesWithTag("fantasy");
StoryInfo[] multiTag = catalog.StoriesWithAllTags(new[] { "fantasy", "exploration" });

// Create an engine for a specific story
using var engine = catalog.CreateEngineByName("ForestAdventure");
```

## Files

| File | Description |
|------|-------------|
| `CyoaEngine.cs` | C# wrapper — `DllImport` calls, string marshaling, JSON parsing |
| `UnityDemo/CyoaUnityDemo.cs` | Full Unity demo MonoBehaviour (catalog UI, tag filtering, save/load) |
| `UnityDemo/README.md` | Unity scene setup guide and API reference |

## Platform notes

| Platform | Plugin file | Destination |
|---|---|---|
| Windows (x64) | `cyoa_native.dll` | `Assets/Plugins/x86_64/` |
| Linux | `libcyoa_native.so` | `Assets/Plugins/` |
| macOS (Intel) | `libcyoa_native.dylib` | `Assets/Plugins/` |
| macOS (Apple Silicon) | `libcyoa_native.dylib` | `Assets/Plugins/` (universal recommended) |
| Android | `libcyoa_native.so` | `Assets/Plugins/arm64-v8a/` |
| iOS | `libcyoa_native.a` | `Assets/Plugins/iOS/` |

> **Mobile (Android/iOS)**: See [docs/mobile.md](../../docs/mobile.md) for
> step-by-step cross-compilation instructions and Unity/iOS setup.

## No external dependencies

The C# wrapper requires **no external NuGet packages**. JSON parsing is
implemented manually (sufficient for the arrays-of-objects schemas the engine
returns). Only `System.Text`, `System.Runtime.InteropServices`, and
`System.Collections.Generic` from the standard library are used.

See [`UnityDemo/README.md`](UnityDemo/README.md) for the full API reference
and the Unity demo scene setup guide.
