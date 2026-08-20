# CYOA Engine API Reference

> Version 0.3.0 — C-ABI native bindings + C# Unity wrapper.

This document is the reference for all engine APIs: WASM/JavaScript,
C-ABI (native), and the Rust core (`cyoa-runtime`).

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [WASM / JavaScript API](#wasm--javascript-api)
  - [`WasmStoryCatalog`](#wasmstorycatalog)
  - [`WasmEngine`](#wasmengine)
  - [Zero-Copy Text Access](#zero-copy-text-access)
  - [State JSON Format](#state-json-format)
- [C-ABI API (Native)](#c-abi-api-native)
  - [Engine handle](#engine-handle)
  - [Catalog handle](#catalog-handle-multi-story--tag-filtering)
  - [C# (Unity) wrapper](#c-unity-wrapper)
  - [C# (Godot) wrapper](#c-godot-wrapper)
- [Rust Runtime API](#rust-runtime-api)
- [Bytecode Format](#bytecode-format)

---

## Architecture Overview

The engine follows a handle-based architecture:

```
┌─────────────┐    register    ┌──────────────────┐
│ Game Engine │ ─────────────→│ WasmStoryCatalog │
│  (JS/C#/     │               │  (multi-story)   │
│   GDScript)  │               └────────┬─────────┘
└─────────────┘                        │ createEngine(i)
                                       ▼
                              ┌──────────────────┐
                              │  WasmEngine      │
                              │  (per story)     │
                              └────────┬─────────┘
                                       │
                              ┌────────▼──────────┐
                              │  State & Cursor   │
                              │  (owned by VM)    │
                              └───────────────────┘
```

- **One `Engine` per story** — state is never shared between stories.
- **Game engine** handles coordination (e.g., passing gold from one story to another).
- **Zero-copy** — the VM reads text directly from bytecode memory; no allocation on the hot path.

---

## WASM / JavaScript API

The WASM module is built with `wasm-bindgen` and exported as an ES module.

### Loading

```typescript
import init, { WasmStoryCatalog, WasmEngine } from './cyoa_wasm.js';

// Must be called once before using any exported class.
await init();
```

### `WasmStoryCatalog`

A registry of compiled stories with tag-based filtering. Use this to
discover stories (especially for random event systems) before creating
an `Engine` to play them.

#### Constructor

##### `new WasmStoryCatalog()`

Creates an empty story catalog.

```typescript
const catalog = new WasmStoryCatalog();
```

#### Methods

##### `register(bytes: Uint8Array): void`

Register a compiled story from bytecode bytes. The bytecode must have been
produced by `cyoa compile` (or the `Compile` struct's `to_bytes()`).

```typescript
const response = await fetch('./forest_adventure.cyoa.bc');
const bytes = new Uint8Array(await response.arrayBuffer());
catalog.register(bytes);
```

**Throws** if the bytecode fails to decode.

##### `len(): number`

Number of registered stories.

##### `isEmpty(): boolean`

Whether the catalog has zero registered stories.

##### `listStories(): { name: string; tags: string[] }[]`

List all registered stories with their names and tags.

```typescript
const stories = catalog.listStories();
// → [{ name: "ForestAdventure", tags: ["fantasy", "exploration"] }, ...]
```

##### `storiesWithTag(tag: string): { name: string; tags: string[] }[]`

Find all stories that have a specific tag.

```typescript
const fantasyStories = catalog.storiesWithTag("fantasy");
```

##### `storiesWithAllTags(tagsJson: string): { name: string; tags: string[] }[]`

Find all stories that have **ALL** of the specified tags.

`tagsJson` is a JSON array of tag strings:

```typescript
const stories = catalog.storiesWithAllTags('["fantasy", "exploration"]');
```

##### `storiesWithAnyTags(tagsJson: string): { name: string; tags: string[] }[]`

Find all stories that have **ANY** of the specified tags.

```typescript
const stories = catalog.storiesWithAnyTags('["combat", "social"]');
```

##### `createEngine(index: number): WasmEngine | null`

Create an `WasmEngine` for the story at the given index (0-based).

Returns `null` if the index is out of bounds.

```typescript
const engine = catalog.createEngine(0);
```

##### `createEngineByName(name: string): WasmEngine | null`

Find a story by its declared name and create an `WasmEngine` for it.

Returns `null` if no story with that name is registered.

```typescript
const engine = catalog.createEngineByName("ForestAdventure");
```

---

### `WasmEngine`

A CYOA story engine instance. Each engine is independent — state is
never shared between instances.

#### Constructor

##### `new WasmEngine(bytes: Uint8Array)`

Create an engine directly from compiled bytecode bytes. Alternatively,
use `WasmStoryCatalog.createEngine()` to create engines from a catalog.

```typescript
const engine = new WasmEngine(bytecodeBytes);
```

**Throws** if the bytecode fails to decode.

#### High-Level API

##### `currentEventId(): string`

Get the current event's internal ID (name).

```typescript
console.log(engine.currentEventId());
// → "old_ruins"
```

##### `getCurrentEvent(): { id: string; text: string[]; choices: string[] }`

Get the current event as a JavaScript object. Templates are already
rendered (e.g., `{{gold}}` → `42`). Choices with unmet prerequisites
are excluded from the list.

```typescript
const event = engine.getCurrentEvent();
// → {
//     id: "old_ruins",
//     text: ["You stand before ancient stone ruins...", "A cold wind whispers..."],
//     choices: ["Enter the ruins", "Circle around"]
//   }
```

##### `makeChoice(choiceIndex: number): { effectText: string[]; gameStateChanged: boolean }`

Make a choice at the given 0-based index. The index references only choices
visible to the player (prerequisites are already filtered).

The returned `effectText` array contains any text produced by the
choice's effects (e.g., "You find a glowing mushroom.").

```typescript
const result = engine.makeChoice(0);
// → { effectText: ["You find a glowing mushroom. It hums softly."], gameStateChanged: true }
```

##### `getHistory(): { eventId: string; choiceIndex: number; choiceText: string }[]`

Get the choice history as an array of objects. This is useful for
UI display, analytics, and save/load systems.

```typescript
const history = engine.getHistory();
// → [
//     { eventId: "start", choiceIndex: 0, choiceText: "Enter the forest" },
//     { eventId: "forest_path", choiceIndex: 1, choiceText: "Return to the entrance" },
//     ...
//   ]
```

##### `getStateJson(): string`

Get the full player state as a JSON string. Use this for save files.
The JSON includes `stats`, `flags`, `tags`, `current_event`, and
`choice_history` — enabling full save/load between sessions. The
engine can be restored to the exact same position using `setStateJson()`.

```typescript
const saveJson = engine.getStateJson();
localStorage.setItem("my-save", saveJson);
```

**Memory contract**: The returned string is valid until the next method call
on this engine.

##### `setStateJson(json: string): void`

Restore state from a JSON string produced by `getStateJson()`.

```typescript
const saveJson = localStorage.getItem("my-save");
engine.setStateJson(saveJson);
```

**Throws** if the JSON is malformed or doesn't match the expected schema.

##### `listStats(): { [key: string]: number }`

Get stats as a JavaScript object mapping stat names to current values.

```typescript
const stats = engine.listStats();
// → { hp: 50, courage: 3, gold: 10 }
```

##### `getStat(name: string): bigint`

Get a single stat value by name. Returns `0` if the stat doesn't exist.

**Note**: The value is returned as a `bigint` in JavaScript (Rust's `i64`
maps to JS `bigint`). For typical game values (which fit in `i32`), you
can convert with `Number()`.

```typescript
const hp = Number(engine.getStat("hp"));
// → 50
```

##### `listStoryTags(): string[]`

List story-level tags — static metadata declared in the `.cyoa` file at
the story block level. These do **not** change during play.

```typescript
console.log(engine.listStoryTags());
// → ["fantasy", "exploration"]
```

##### `listTags(): string[]`

List runtime tags currently applied during play. These are added by
the `add tag` instruction or the `AddTag` opcode.

```typescript
console.log(engine.listTags());
// → ["combat"]
```

##### `listFlags(): string[]`

List all runtime flags currently set (boolean story progress markers).

```typescript
console.log(engine.listFlags());
// → ["visited_old_ruins", "wolf_friend"]
```

##### `canAccessEvent(id: string): boolean`

Check if an event by ID exists in the story's event index.

```typescript
if (engine.canAccessEvent("wolf_fight")) {
  console.log("The wolf fight event is reachable from this story.");
}
```

##### `availableEvents(): string[]`

List all event IDs in the story. Useful for quest logs and event
discovery.

```typescript
const events = engine.availableEvents();
// → ["start", "forest_path", "old_ruins", "forest_encounter", ...]
```

---

#### Zero-Copy Text Access

For maximum performance when rendering large blocks of text, the engine
provides `Uint8Array` views directly into WASM linear memory. These
avoid the `TextDecoder` serialization overhead of the serde-based
high-level API.

The views are valid **until the next method call on the same engine**.
The typical usage pattern is: **call → decode → use → next call is safe**.

##### `currentEventTextBytes(): Uint8Array`

Returns a `Uint8Array` view of all current event text paragraphs,
newline-separated (`\n`).

```typescript
const bytes = engine.currentEventTextBytes();
const text = new TextDecoder().decode(bytes);
// text now contains all paragraphs joined by newlines
```

##### `currentChoicesBytes(): Uint8Array`

Returns a `Uint8Array` view of all current choice texts, newline-separated.

```typescript
const bytes = engine.currentChoicesBytes();
const choices = new TextDecoder().decode(bytes).split('\n');
```

**Performance note**: The zero-copy API avoids wasm-bindgen's `String`
allocation + `TextDecoder.decode()` overhead. Benchmarks in the web demo
show 2–5× speedup depending on text length.

---

#### State JSON Format

The state JSON (returned by `getStateJson()` and accepted by
`setStateJson()`) has this structure:

```json
{
  "stats": {
    "hp": 50,
    "courage": 3,
    "gold": 10
  },
  "flags": ["visited_old_ruins", "wolf_friend"],
  "tags": ["combat"],
  "current_event": 3,
  "choice_history": [
    { "event_id": "old_ruins", "choice_index": 0, "choice_text": "Enter the ruins" },
    { "event_id": "dense_forest", "choice_index": 1, "choice_text": "Climb a tree" }
  ]
}
```

- **`stats`**: Map of stat name → current `i64` value.
- **`flags`**: Set of flag names currently set to `true`.
- **`tags`**: Set of runtime tags currently applied this session.
- **`current_event`**: Index of the current event in the story's event table (used by `set_state_json` to restore the cursor position).
- **`choice_history`**: Array of `{ event_id, choice_index, choice_text }` entries recording all choices made so far.

> **Important**: `tags` in state are **runtime-applied** tags (added during play
> via `add tag`). **Story-level tags** are static metadata in the bytecode
> and are **not** part of this JSON — use `listStoryTags()` to query them.

---

## C-ABI API (Native)

The `cyoa-native` crate compiles to a native shared library with `extern "C"`
FFi exports. Each story is a separate engine instance identified by an opaque
handle, and a catalog handle manages multiple registered stories with tag
filtering — mirroring the `WasmStoryCatalog` / `WasmEngine` design.

### Building

```bash
cargo build -p cyoa-native --release          # Linux: libcyoa_native.so
                                                  # macOS: libcyoa_native.dylib
                                                  # Windows: cyoa_native.dll
```

### Memory model

| Return type | Lifetime | Free with |
|---|---|---|
| `const char *` | Engine-owned; valid until next call on same handle | — (copy immediately) |
| `char *` | Heap-allocated | `cyoa_free_string()` |

### Engine handle

```c
typedef struct CyoaEngine CyoaEngine;

/* Lifecycle */
CyoaEngine* cyoa_create(const uint8_t* bytecode, size_t len);  /* NULL on failure */
void cyoa_destroy(CyoaEngine* engine);

/* Event queries — const char* = engine-owned, valid until next call */
const char* cyoa_current_event_id(CyoaEngine* engine);
const char* cyoa_current_event_text(CyoaEngine* engine);  /* paragraphs joined by \n */
int         cyoa_current_choice_count(CyoaEngine* engine);
const char* cyoa_choice_text(CyoaEngine* engine, int index);  /* NULL if OOB */

/* Make a choice */
void cyoa_make_choice(CyoaEngine* engine, int index);
const char* cyoa_last_effect_text(CyoaEngine* engine);  /* effect text from last choice */

/* Choice history */
int cyoa_history_length(CyoaEngine* engine);
const char* cyoa_history_entry(CyoaEngine* engine, int index);  /* JSON or NULL */

/* State management — char* = heap-allocated, caller must free */
char* cyoa_get_state_json(CyoaEngine* engine);
void  cyoa_set_state_json(CyoaEngine* engine, const char* json);

/* Queries */
char* cyoa_available_events_json(CyoaEngine* engine);  /* JSON array, caller frees */
int   cyoa_can_access_event(CyoaEngine* engine, const char* id);

/* Stats / tags / flags — char* = heap-allocated, caller frees */
char* cyoa_list_stats_json(CyoaEngine* engine);
char* cyoa_list_story_tags_json(CyoaEngine* engine);
char* cyoa_list_tags_json(CyoaEngine* engine);
char* cyoa_list_flags_json(CyoaEngine* engine);
int   cyoa_get_stat(CyoaEngine* engine, const char* name);

/* Free a heap-allocated string from any *_json or get_state_json function */
void cyoa_free_string(char* s);
```

### Catalog handle (multi-story + tag filtering)

```c
typedef struct CyoaCatalog CyoaCatalog;

/* Lifecycle */
CyoaCatalog* cyoa_catalog_create(void);
void cyoa_catalog_destroy(CyoaCatalog* catalog);

/* Registration */
int cyoa_catalog_register(CyoaCatalog* catalog, const uint8_t* bytecode, size_t len);
int cyoa_catalog_story_count(const CyoaCatalog* catalog);

/* Discovery — char* = heap-allocated JSON, caller frees */
char* cyoa_catalog_list_stories_json(const CyoaCatalog* catalog);
char* cyoa_catalog_stories_with_tag_json(const CyoaCatalog* catalog, const char* tag);
char* cyoa_catalog_stories_with_all_tags_json(const CyoaCatalog* catalog, const char* tags_json);
char* cyoa_catalog_stories_with_any_tags_json(const CyoaCatalog* catalog, const char* tags_json);

/* Engine creation from catalog */
CyoaEngine* cyoa_catalog_create_engine(CyoaCatalog* catalog, int index);
CyoaEngine* cyoa_catalog_create_engine_by_name(CyoaCatalog* catalog, const char* name);
```

Catalog discovery functions return JSON arrays of `{"name":"string","tags":["..."]}`
objects. The `tags_json` argument accepts a JSON array of tag strings (e.g.
`["fantasy","exploration"]`).

### C# (Unity) wrapper

A C# wrapper class (`CyoaEngine.cs`) uses `DllImport` to call the native
functions and provides a rich, memory-safe API mirroring the WASM/JS API.
See [`bindings/csharp/`](../bindings/csharp/README.md) for the wrapper and a Unity demo
MonoBehaviour.

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

// Play the story
string text = engine.CurrentEventText;
string[] choices = engine.CurrentChoices;
engine.MakeChoice(0);

// Save / load
string saveJson = engine.GetStateJson();
engine.SetStateJson(saveJson);
```

### C# (Godot) wrapper

A C# wrapper (`bindings/godot/csharp/CyoaEngine.cs`) uses `DllImport` to call
the same native C-ABI functions. Since GDScript cannot call native C directly,
this C# class serves as the FFI bridge, with a thin GDScript wrapper
(`bindings/godot/gdscript/cyoa_engine.gd`) for scene-based usage.

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
Godot integration guide.

---

## Rust Runtime API

The `cyoa-runtime` crate provides the full engine API used by both
the WASM and native bindings. Writers building native Rust games can
use this directly without FFI.

### `Engine`

```rust
use cyoa_runtime::Engine;
use cyoa_bytecode::Bytecode;

let bytecode: Bytecode = ...;
let mut engine = Engine::new(bytecode);

// Current event
engine.current_event_id() -> String;
engine.current_event_text() -> Vec<String>;
engine.current_choices() -> Vec<String>;

// Make a choice
engine.make_choice(index: i32) -> Vec<String>;  // effect text

// History
engine.history() -> &[HistoryEntry];

// State (full: stats, flags, tags, current_event, choice_history)
engine.get_state_json() -> String;
engine.set_state_json(&json) -> Result<(), serde_json::Error>;

// Stats / flags / tags
engine.stats_json() -> String;  // JSON map of stat name → i64 value
engine.get_stat(&name) -> i64;
engine.list_flags() -> Vec<String>;
engine.list_tags() -> Vec<String>;
engine.list_story_tags() -> Vec<String>;

// Queries
engine.can_access_event(&id) -> bool;
engine.available_events() -> Vec<String>;
```

### `StoryCatalog`

```rust
use cyoa_runtime::{StoryCatalog, StoryMetadata};

let mut catalog = StoryCatalog::new();
catalog.register(bytecode);  // Bytecode

// Discovery
catalog.list_stories() -> Vec<StoryMetadata>;
catalog.stories_with_tag(tag) -> Vec<&StoryMetadata>;
catalog.stories_with_all_tags(&tags) -> Vec<&StoryMetadata>;
catalog.stories_with_any_tags(&tags) -> Vec<&StoryMetadata>;

// Engine creation
catalog.create_engine(index) -> Option<Engine>;
catalog.create_engine_by_name(name) -> Option<Engine>;
catalog.len() -> usize;
catalog.is_empty() -> bool;
```

### `StoryMetadata`

```rust
pub struct StoryMetadata {
    pub name: String,
    pub tags: Vec<String>,
}
```

---

## Bytecode Format

The `.cyoa.bc` file is a binary format serialized with `postcard` (binary,
no-std compatible, fast). It is designed for zero-copy mmap loading.

### Binary Layout

```
┌────────────────────┐
│ Header (24 bytes)  │  magic (4B) + version (4B) + section offsets
├────────────────────┤
│ String Table       │  all text, deduplicated, interned
├────────────────────┤
│ Event Index        │  event_id → offset mapping
├────────────────────┤
│ Events             │  array of fixed-size EventEntry
├────────────────────┤
│ Choices            │  choice arrays (indexed by event)
├────────────────────┤
│ Effect Blocks      │  reusable effects
├────────────────────┤
│ Instruction Stream │  bytecode for events + effects + conditions
└────────────────────┘
```

### Key Types

| Type | Purpose |
|------|---------|
| `BytecodeHeader` | Magic, version, section offsets |
| `StringRef` | `(offset, length)` into string data |
| `EventEntry` | `id`, `body_start`, `text_start`, `choice_start`, `requires`, `tag_start` |
| `ChoiceEntry` | `text`, `step_start`, `use_start`, `next`, `requires` |
| `EffectEntry` | `name`, `inst_start`, `inst_len` |
| `Instruction` | `opcode: u8`, `operand_a: u32`, `operand_b: u32` |
| `Opcode` | 13 variants: `GetText` through `Return` |

### Versioning

- `MAGIC = 0x43594F41` ("CYOA")
- `VERSION = 1`

### Serialization

```rust
let bytes: Vec<u8> = bytecode.to_bytes()?;    // serialize
let bc: Bytecode = Bytecode::from_bytes(&bytes)?;  // deserialize
```

---

## CLI Reference

The `cyoa-cli` binary provides three commands:

```
cyoa compile <story.cyoa>     # Compile .cyoa → .cyoa.bc
cyoa play <story.cyoa.bc>     # Interactive play mode
cyoa validate <story.cyoa>    # Validate without output
```

See [README.md](../README.md#quick-start) for usage examples.