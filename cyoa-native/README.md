# cyoa-native

Native C-ABI bindings for the CYOA engine. Compiles to a shared library
(`.so` / `.dll` / `.dylib`) or static library (`.a`) for use with Unity,
Godot, desktop C/C++ apps, and mobile platforms.

## Build

```bash
# Linux / macOS / Windows
cargo build -p cyoa-native --release
```

Output files in `target/release/`:

| Platform | File |
|----------|------|
| Linux | `libcyoa_native.so` |
| macOS | `libcyoa_native.dylib` |
| Windows | `cyoa_native.dll` |

```bash
# Android (requires cargo-ndk)
cargo ndk -t arm64-v8a -t armeabi-v7a build -p cyoa-native --release

# iOS (requires cargo-lipo, macOS + Xcode)
cargo lipo -p cyoa-native --release
```

## Integrate

Copy the library + C header into your project:

```
YourProject/
├── libcyoa_native.so    # or .dll / .dylib / .a
├── cyoa.h               # C header
└── story.cyoa.bc        # compiled story
```

## Usage (C)

```c
#include "cyoa.h"

CyoaEngine* engine = cyoa_create(bytecode, len);
printf("%s\n", cyoa_current_event_text(engine));

int count = cyoa_current_choice_count(engine);
for (int i = 0; i < count; i++) {
    printf("  [%d] %s\n", i, cyoa_choice_text(engine, i));
}

cyoa_make_choice(engine, 0);

/* Save / load */
char* state = cyoa_get_state_json(engine);
cyoa_set_state_json(engine, saved_state);
cyoa_free_string(state);

cyoa_destroy(engine);
```

## Language bindings

| Target | Wrapper |
|--------|---------|
| Unity (C#) | [`bindings/csharp/CyoaEngine.cs`](../bindings/csharp/CyoaEngine.cs) |
| Godot (GDScript + C#) | [`bindings/godot/`](../bindings/godot/) |
| Web (WASM) | [`cyoa-wasm`](../cyoa-wasm/) |

C header: [`bindings/c/cyoa.h`](../bindings/c/cyoa.h) ·
API reference: [`docs/c-abi-api.md`](../docs/c-abi-api.md) ·
Mobile guide: [`docs/mobile.md`](../docs/mobile.md)
