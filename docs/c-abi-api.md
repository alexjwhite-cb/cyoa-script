# C-ABI API Reference

The `cyoa-native` crate compiles to a native shared library with `extern "C"`
FFI exports. Each story is a separate engine instance identified by an opaque
handle, and a catalog handle manages multiple registered stories with tag
filtering — mirroring the WASM `WasmStoryCatalog` / `WasmEngine` design.

## Building

```bash
cargo build -p cyoa-native --release
```

| Platform | Output file |
|----------|-------------|
| Linux | `libcyoa_native.so` |
| macOS | `libcyoa_native.dylib` |
| Windows | `cyoa_native.dll` |

See [mobile.md](mobile.md) for Android/iOS cross-compilation.

---

## Memory Model

| Return type | Lifetime | Free with |
|---|---|---|
| `const char *` | Engine-owned; valid until next call on same handle | — (copy immediately) |
| `char *` | Heap-allocated | `cyoa_free_string()` |

**Rule**: If a function returns `char *`, you must call `cyoa_free_string()`
on it when done. If it returns `const char *`, the pointer is into
engine-owned memory and is valid only until the next call on the same
handle — copy it immediately if you need it to persist.

---

## Engine Handle API

```c
typedef struct CyoaEngine CyoaEngine;
```

### Lifecycle

```c
CyoaEngine* cyoa_create(const uint8_t* bytecode, size_t len);
void cyoa_destroy(CyoaEngine* engine);
```

- `cyoa_create`: Create an engine from compiled `.cyoa.bc` bytes. Returns a
  handle, or `NULL` on failure.
- `cyoa_destroy`: Destroy an engine. Passing `NULL` is a no-op.

### Event queries

```c
const char* cyoa_current_event_id(CyoaEngine* engine);
const char* cyoa_current_event_text(CyoaEngine* engine);
int         cyoa_current_choice_count(CyoaEngine* engine);
const char* cyoa_choice_text(CyoaEngine* engine, int index);
```

- `cyoa_current_event_id`: Internal event ID (name). Engine-owned string.
- `cyoa_current_event_text`: Event text paragraphs joined by `\n`. Engine-owned.
- `cyoa_current_choice_count`: Number of visible choices.
- `cyoa_choice_text`: Text of choice at `index`, or `NULL` if out of bounds.

### Make a choice

```c
void cyoa_make_choice(CyoaEngine* engine, int index);
const char* cyoa_last_effect_text(CyoaEngine* engine);
```

- `cyoa_make_choice`: Apply the player's choice at `index`. After this call,
  the engine has advanced to the next event.
- `cyoa_last_effect_text`: Effect text from the most recent `make_choice` call.
  Multiple fragments joined by `\n`. Engine-owned.

### History

```c
int cyoa_history_length(CyoaEngine* engine);
const char* cyoa_history_entry(CyoaEngine* engine, int index);
```

- `cyoa_history_length`: Number of entries in the choice history.
- `cyoa_history_entry`: JSON string for one entry:
  `{"eventId":"...","choiceIndex":N,"choiceText":"..."}`. Returns `NULL` if
  `index` is out of bounds. Engine-owned.

### State management

```c
char* cyoa_get_state_json(CyoaEngine* engine);
void  cyoa_set_state_json(CyoaEngine* engine, const char* json);
```

- `cyoa_get_state_json`: Full player state as JSON (stats, flags, tags,
  current_event, choice_history, complete). **Caller must free** with `cyoa_free_string`.
- `cyoa_set_state_json`: Restore state from a JSON string produced by
  `cyoa_get_state_json`. Pass `NULL` for `json` (no-op).

### Queries

```c
char* cyoa_available_events_json(CyoaEngine* engine);
int   cyoa_can_access_event(CyoaEngine* engine, const char* id);
```

- `cyoa_available_events_json`: JSON array of all event IDs. **Caller must free**.
- `cyoa_can_access_event`: Returns `1` (true) or `0` (false).

### Stats / tags / flags

```c
char* cyoa_list_stats_json(CyoaEngine* engine);
char* cyoa_list_story_tags_json(CyoaEngine* engine);
char* cyoa_list_tags_json(CyoaEngine* engine);
char* cyoa_list_flags_json(CyoaEngine* engine);
int   cyoa_get_stat(CyoaEngine* engine, const char* name);
```

- `cyoa_list_stats_json`: JSON object `{"statName": value, ...}`. **Caller must free**.
- `cyoa_list_story_tags_json`: JSON array of story-level tags. **Caller must free**.
- `cyoa_list_tags_json`: JSON array of runtime tags. **Caller must free**.
- `cyoa_list_flags_json`: JSON array of runtime flags. **Caller must free**.
- `cyoa_get_stat`: Returns stat value as `i64`, or `0` if not found.

### Freeing strings

```c
void cyoa_free_string(char* s);
```

Free any string returned by `*_json` or `get_state_json` functions.
Passing `NULL` is a no-op.

---

## Catalog Handle API (multi-story + tag filtering)

```c
typedef struct CyoaCatalog CyoaCatalog;
```

### Lifecycle

```c
CyoaCatalog* cyoa_catalog_create(void);
void cyoa_catalog_destroy(CyoaCatalog* catalog);
```

### Registration

```c
int cyoa_catalog_register(CyoaCatalog* catalog, const uint8_t* bytecode, size_t len);
int cyoa_catalog_story_count(const CyoaCatalog* catalog);
```

- `cyoa_catalog_register`: Register a story from bytecode. Returns 1 on success, 0 on failure.
- `cyoa_catalog_story_count`: Number of registered stories.

### Discovery

All return JSON arrays of `{"name":"string","tags":["..."]}` objects.
**Caller must free** the returned `char *` with `cyoa_free_string`.

```c
char* cyoa_catalog_list_stories_json(const CyoaCatalog* catalog);
char* cyoa_catalog_stories_with_tag_json(const CyoaCatalog* catalog, const char* tag);
char* cyoa_catalog_stories_with_all_tags_json(const CyoaCatalog* catalog, const char* tags_json);
char* cyoa_catalog_stories_with_any_tags_json(const CyoaCatalog* catalog, const char* tags_json);
```

`tags_json` arguments accept a JSON array of tag strings (e.g., `["fantasy","exploration"]`).

### Engine creation

```c
CyoaEngine* cyoa_catalog_create_engine(CyoaCatalog* catalog, int index);
CyoaEngine* cyoa_catalog_create_engine_by_name(CyoaCatalog* catalog, const char* name);
```

- `cyoa_catalog_create_engine`: Create an engine for the story at `index`. Returns `NULL` if out of bounds.
- `cyoa_catalog_create_engine_by_name`: Create an engine for the named story. Returns `NULL` if not found.

---

## C Header

The canonical C header is at [`bindings/c/cyoa.h`](../cyoa-native/include/cyoa.h)
(mirrored in `bindings/c/`).

```c
#include "cyoa.h"
```

```c
CyoaCatalog* catalog = cyoa_catalog_create();

/* Load bytecode bytes (from file or embedded) */
uint8_t* bytes = /* ... */;
size_t len = /* ... */;
cyoa_catalog_register(catalog, bytes, len);

/* Filter and create engine */
CyoaEngine* engine = cyoa_catalog_create_engine_by_name(catalog, "ForestAdventure");

/* Play */
printf("Event: %s\n", cyoa_current_event_id(engine));
printf("%s\n", cyoa_current_event_text(engine));

int choiceCount = cyoa_current_choice_count(engine);
for (int i = 0; i < choiceCount; i++) {
    printf("  [%d] %s\n", i, cyoa_choice_text(engine, i));
}

cyoa_make_choice(engine, 0);

/* Save / load */
char* stateJson = cyoa_get_state_json(engine);
/* ... save stateJson to file, free later ... */
cyoa_set_state_json(engine, savedJson);
cyoa_free_string(stateJson);

/* Cleanup */
cyoa_destroy(engine);
cyoa_catalog_destroy(catalog);
```

---

## C# (Unity) Wrapper

The C# wrapper class (`bindings/csharp/CyoaEngine.cs`) uses `DllImport` to
call the native functions and provides a rich, memory-safe API mirroring the
WASM/JS API.

```csharp
using Cyoa;

// Catalog-based: load multiple stories and filter by tags
using var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(File.ReadAllBytes("forest_adventure.cyoa.bc"));
catalog.RegisterStory(File.ReadAllBytes("tavern_tales.cyoa.bc"));

// Tag-based discovery
StoryInfo[] fantasy = catalog.StoriesWithTag("fantasy");
StoryInfo[] multiTag = catalog.StoriesWithAllTags(new[] { "fantasy", "exploration" });

// Create an engine for a story
using var engine = catalog.CreateEngineByName("ForestAdventure");

// Play
string text = engine.CurrentEventText;
string[] choices = engine.CurrentChoices;
engine.MakeChoice(0);

// Save / load
string saveJson = engine.GetStateJson();
engine.SetStateJson(saveJson);
```

### C# API surface

| Method | Returns | Description |
|--------|---------|-------------|
| `new CyoaStoryCatalog()` | — | Create catalog |
| `RegisterStory(byte[] bytes)` | `void` | Register a story |
| `StoryCount` | `int` | Number of registered stories |
| `ListStories()` | `StoryInfo[]` | All stories |
| `StoriesWithTag(string tag)` | `StoryInfo[]` | Stories with tag |
| `StoriesWithAllTags(string[] tags)` | `StoryInfo[]` | Stories with ALL tags |
| `StoriesWithAnyTags(string[] tags)` | `StoryInfo[]` | Stories with ANY tags |
| `CreateEngine(int index)` | `CyoaEngine` | Create engine from catalog |
| `CreateEngineByName(string name)` | `CyoaEngine` | Create engine by name |
| `Dispose()` | — | Free native resources |

**Engine** (same as Unity `CyoaEngine`):

| Method | Returns | Description |
|--------|---------|-------------|
| `new CyoaEngine(byte[] bytecode)` | — | Create from bytecode |
| `CurrentEventId` | `string` | Current event ID |
| `CurrentEventText` | `string` | Event text (\n-joined) |
| `CurrentChoices` | `string[]` | Visible choice texts |
| `ChoiceCount` | `int` | Number of visible choices |
| `GetChoiceText(int index)` | `string` | One choice text |
| `MakeChoice(int index)` | — | Apply a choice |
| `LastEffectText` | `string` | Effect text from last choice |
| `GetHistory()` | `HistoryEntry[]` | Choice history |
| `HistoryLength` | `int` | History count |
| `GetHistoryEntry(int index)` | `HistoryEntry` | One history entry |
| `GetStateJson()` | `string` | Full state as JSON |
| `SetStateJson(string json)` | — | Restore state |
| `GetStats()` | `Dictionary<string, int>` | Stat name → value |
| `GetStat(string name)` | `long` | Single stat |
| `GetStoryTags()` | `string[]` | Story-level tags |
| `GetTags()` | `string[]` | Runtime tags |
| `GetFlags()` | `string[]` | Runtime flags |
| `CanAccessEvent(string id)` | `bool` | Event reachable? |
| `IsStoryComplete` | `bool` | True if a terminal choice was made |
| `GetAvailableEvents()` | `string[]` | All event IDs |
| `Dispose()` | — | Free native resources |

Unity integration guide: [`bindings/csharp/README.md`](../bindings/csharp/README.md)

---

## C# (Godot) Wrapper

The C# wrapper (`bindings/godot/csharp/CyoaEngine.cs`) uses `DllImport` to
call the same native C-ABI functions. Since GDScript cannot call native C
directly, this C# class serves as the FFI bridge, with a thin GDScript
wrapper (`bindings/godot/gdscript/cyoa_engine.gd`) for scene-based usage.

```csharp
using Cyoa.Godot;

// Load from a Godot resource path
var engine = CyoaEngine.LoadFromFile("res://addons/cyoa/native/forest_adventure.cyoa.bc");

GD.Print(engine.CurrentEventText);
string[] choices = engine.CurrentChoices;
engine.MakeChoice(0);

// Multi-story with tag filtering
using var catalog = new CyoaStoryCatalog();
catalog.RegisterStory(FileAccess.GetFileAsBytes("res://addons/cyoa/native/forest_adventure.cyoa.bc"));
StoryInfo[] fantasy = catalog.StoriesWithTag("fantasy");
```

See [`bindings/godot/README.md`](../bindings/godot/README.md) for the full
Godot integration guide and GDScript API.