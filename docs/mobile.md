# Mobile Integration Guide

Cross-compiling the CYOA native engine for Android and iOS using Rust's
toolchain. The `cyoa-native` crate uses `crate-type = ["cdylib", "staticlib", "rlib"]`,
producing both dynamic libraries (`.so`/`.dylib`) for runtime loading and static
libraries (`.a`) for static linking.

## Table of Contents

- [Android](#android)
- [iOS](#ios)
- [Unity mobile integration](#unity-mobile-integration)
- [Godot mobile integration](#godot-mobile-integration)
- [Troubleshooting](#troubleshooting)

---

## Android

### Prerequisites

1. **Rust with Android targets**:

   ```bash
   rustup target add aarch64-linux-android armv7-linux-androideabi
   ```

2. **cargo-ndk** (installs NDK and sets up targets automatically):

   ```bash
   cargo install cargo-ndk
   ```

   Alternatively, install the Android NDK manually via Android Studio →
   **Tools → SDK Manager → SDK Tools → NDK (Side by side)**.

3. **Unity**: Unity 2021+ with Android Build Support (IL2CPP). The C# wrapper
   uses `DllImport` with `CyoaNative` — no special Unity packages required.

### Building the native library

From the project root:

```bash
# Build for all Android ABIs
cargo ndk -t arm64-v8a -t armeabi-v7a build -p cyoa-native --release

# Or build for a single ABI
cargo ndk -t arm64-v8a build -p cyoa-native --release
```

Outputs:

| ABI | File | Location |
|-----|------|----------|
| arm64-v8a | `libcyoa_native.so` | `target/<triple>/release/` |
| armeabi-v7a | `libcyoa_native.so` | `target/<triple>/release/` |

### Unity Android setup

1. **Copy the `.so` files** to your Unity project:

   ```
   YourUnityProject/
   └── Assets/
       └── Plugins/
           ├── arm64-v8a/
           │   └── libcyoa_native.so
           └── armeabi-v7a/
               └── libcyoa_native.so
   ```

2. **Configure Plugin Import Settings** in Unity:
   - Select each `.so` file in the Inspector.
   - Under **Select platforms**, tick **Android**.
   - Set **ARM64** / **ARMv7** respectively.

3. **Copy the C# wrapper** to `Assets/Scripts/Cyoa/`.

4. **Build Settings** → **Android** → **IL2CPP** → **Target Architectures**:
   - Enable **ARM64** and/or **ARMv7** as needed.

5. The `CyoaEngine` C# wrapper handles everything else — same API as desktop:

   ```csharp
   using Cyoa;

   byte[] bytecode = File.ReadAllBytes("Assets/StreamingAssets/forest_adventure.cyoa.bc");
   using var engine = new CyoaEngine(bytecode);
   string text = engine.CurrentEventText;
   string[] choices = engine.CurrentChoices;
   engine.MakeChoice(0);
   ```

### Story asset placement

Place `.cyoa.bc` files in `Assets/StreamingAssets/`. They are copied verbatim
to the APK and can be read with `File.ReadAllBytes` at runtime.

---

## iOS

### Prerequisites

1. **Rust with iOS targets**:

   ```bash
   rustup target add aarch64-apple-ios x86_64-apple-ios
   ```

2. **cargo-lipo** (produces a universal/iOS static library):

   ```bash
   cargo install cargo-lipo
   ```

3. **Unity**: Unity 2021+ with iOS Build Support (IL2CPP). You need a
   macOS machine with Xcode installed to build for iOS.

### Building the static library

```bash
cargo lipo -p cyoa-native --release
```

This produces `libcyoa_native.a` (universal — covers both arm64 iOS devices
and x86_64 simulator).

Output location: `target/universal/release/libcyoa_native.a`

### Unity iOS setup

1. **Copy the static library** to your Unity project:

   ```
   YourUnityProject/
   └── Assets/
       └── Plugins/
           └── iOS/
               └── libcyoa_native.a
   ```

2. **Unity Inspector** — select `libcyoa_native.a`:
   - **Select platforms**: tick **iOS** only.
   - **Any Platform** should be unchecked for other platforms.

3. **Build Settings** → **iOS** → **Architectures**:
   - **SDK**: Device (arm64) and/or Simulator (x86_64) as needed.
   - **Build System**: Xcode.

4. **Link the library** in Xcode:
   When Unity generates the Xcode project, the `.a` file is automatically
   included. If you need to verify:
   - Open the generated Xcode project.
   - Go to **Build Phases → Link Binary with Libraries**.
   - `libcyoa_native.a` should be listed.

5. The C# wrapper works identically to desktop — no code changes needed:

   ```csharp
   using Cyoa;

   using var engine = new CyoaEngine(bytecode);
   ```

### Story asset placement

Place `.cyoa.bc` files in `Assets/StreamingAssets/`. Unity copies them into
the iOS app bundle, where they can be read with `Application.streamingAssetsPath`.

---

## Unity Mobile Integration

### Multi-story with tag filtering (mobile)

The `CyoaStoryCatalog` works on mobile platforms exactly as on desktop:

```csharp
using Cyoa;

// Register stories compiled ahead of time
byte[] story1 = File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath, "forest_adventure.cyoa.bc"));
byte[] story2 = File.ReadAllBytes(Path.Combine(Application.streamingAssetsPath, "tavern_tales.cyoa.bc"));

using var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(story1);
catalog.RegisterStory(story2);

// Discover by tags
StoryInfo[] fantasy = catalog.StoriesWithAllTags(new[] { "fantasy", "exploration" });
StoryInfo[] anyMatch = catalog.StoriesWithTag("combat");

// Create engines
using var engine = catalog.CreateEngineByName("ForestAdventure");
```

### Save system (mobile)

```csharp
// Save game state
string saveJson = engine.GetStateJson();
string savePath = Path.Combine(Application.persistentDataPath, "savegame.json");
File.WriteAllText(savePath, saveJson);

// Load game state
string saveJson = File.ReadAllText(savePath);
engine.SetStateJson(saveJson);
```

---

## Godot Mobile Integration

### Android

1. Build the native library:

   ```bash
   cargo ndk -t arm64-v8a -t armeabi-v7a build -p cyoa-native --release
   ```

2. Copy `.so` files to `addons/cyoa/native/` in your Godot project.

3. Godot's **.NET (C#)** build will load the native library at runtime via
   the C# wrapper's `DllImport`.

### iOS

1. Build the static library:

   ```bash
   cargo lipo -p cyoa-native --release
   ```

2. Copy `libcyoa_native.a` to `addons/cyoa/native/` in your Godot project.

3. Configure Godot's iOS build to include the static library.

---

## Troubleshooting

### "DllNotFoundException: cyoa_native"

- **Android**: Verify the `.so` files are in `Assets/Plugins/arm64-v8a/`
  (and/or `armeabi-v7a/`) with the exact name `libcyoa_native.so`.
- **iOS**: Verify `libcyoa_native.a` is in `Assets/Plugins/iOS/` and
  that **Select platforms → iOS** is enabled in the Inspector.
- Check that the **architecture** matches (arm64 vs x86_64).

### "Failed to create CYOA engine"

- Verify the `.cyoa.bc` file exists in `StreamingAssets/`.
- Run `cargo run -p cyoa-cli -- compile story.cyoa` to (re)compile.
- Check the file isn't empty or corrupted.

### Unity IL2CPP stripping

If you get `MissingMethodException` or `NullReferenceException` from
DllImport on mobile, add a **link.xml** file to prevent IL2CPP from
stripping the P/Invoke declarations:

```xml
<linker>
  <assembly fullname="CyoaEngine" preserve="all"/>
</linker>
```

Place this in `Assets/link.xml`.
