# CYOA Language Specification (SPEC.md)

> **SPEC.md is the canonical source of truth for the CYOA DSL language.**
> Update this file before `CLAUDE.md`, `README.md`, and `docs/*.md` on every
> language or API change. Follow the sync protocol in `CLAUDE.md`.

## 1. Overview

The CYOA DSL is a declarative, indentation-based language for writing
Choose-Your-Own-Adventure-style interactive stories. It targets writers and
game designers with **zero programming knowledge**. Stories are compiled ahead
of time (`.cyoa` → `.cyoa.bc`) into a compact bytecode format that a lightweight
register-based VM interprets at runtime.

---

## 2. File Format

- **Source**: `.cyoa` files (UTF-8 text, indentation-based)
- **Compiled**: `.cyoa.bc` files (binary, self-contained, mmap-loadable)

### 2.1 Import System

Stories may import definitions from other `.cyoa` files using Go-style paths:

```
import "std/healing"    # standard library package
import "./my_stats"      # relative path
import "./effects"       # relative path
```

All imports are resolved at **compile time** — the resulting `.cyoa.bc` is
self-contained (no runtime file access). The compiler uses 3 passes:

1. **Parse pass**: Parse main file + all transitively-referenced files into ASTs
2. **Merge pass**: Build dependency graph, detect cycles, resolve name collisions
3. **Codegen pass**: Emit unified bytecode from merged AST

**Phase 1 scope**: Only `std/...` and `./...` imports. Package registry imports
deferred to Phase 2+.

**Collision resolution**: Use `as` aliasing:
```
import "./effects" as eff
```

**Circular import detection**: Compile error. The compiler builds a dependency
graph and rejects cycles.

---

## 3. DSL Grammar (Pest)

### 3.1 Statement Types

Every top-level construct in a `.cyoa` file falls into one of these categories:

| Keyword | Description | Example |
|---------|-------------|---------|
| `story` | Top-level story container | `story ForestAdventure:` |
| `stat` | Numeric state variable (i64) | `stat hp = 50` |
| `effect` | Reusable consequence block | `effect found_mushroom:` |
| `event` | Story node with text + choices | `event old_ruins:` |
| `choice` | A player-selectable option | `choice "Enter the ruins":` |
| `import` | Import from another file | `import "std/healing"` |

### 3.2 Indentation Rules

The DSL is **indentation-based** (like Python). The base indentation unit is
2 spaces (enforced — no tabs).

```
story MyStory:                    # root — 0 indent
  stat hp = 50                    # story body — 2 spaces
  event the_cave:                 # event in story — 2 spaces
    "Dark cave entrance."         # event body — 4 spaces
    choice "Enter":               # choice in event — 4 spaces
      next deeper_in              # choice body — 6 spaces
```

### 3.3 Comments

Comments start with `#` and extend to the end of the line. Writers use `#`
because it's familiar and doesn't conflict with `{{}}` templating syntax.

```
# This is a comment
stat hp = 50  # inline comment also works
```

### 3.4 State Declarations

#### Stats (Numeric)

```
stat hp = 50          # initial value (i64)
stat courage = 0
stat gold = 0
```

- Type: `i64` (64-bit integer — wide range for game values)
- Scope: Global to the story
- Default if omitted: `0`

#### Flags (Boolean)

```
flag visited_cave        # defaults to false
flag obtained_key = false # explicit false
flag mushroom_collected = true  # starts true
```

- Type: `bool`
- Default: `false`

#### Tags (String Set)

Tags exist at two levels:

**Story-level tags** — declared once at the top of the story block,
describing the story as a whole. These are static metadata queryable via
the API (e.g. `list_story_tags()`) so game engines can filter stories
by tag:

```
story ForestAdventure:
  tags: fantasy, exploration

  stat hp = 50
  ...
```

Multi-line form (when `tags:` is on its own line, indented tags follow):
```
story ForestAdventure:
  tags:
    fantasy
    exploration

  stat hp = 50
  ...
```

**Event-level tags** — declared per-event, describing that event's content:

```
event dark_forest:
  tags: combat, dangerous
```

Multi-line form:
```
event dark_forest:
  tags:
    combat
    dangerous
```

### 3.5 Effect Blocks

Reusable consequence blocks. Defined once, referenced by `uses` in choices:

```
effect wolf_scare:
  - hp by 10
  text "The wolf's claws scratch you. You take 10 damage."
```

**Effect body syntax** (inside an effect block):
- `+ stat by N` — increase stat
- `- stat by N` — decrease stat
- `set flag to true` / `set flag to false` — set boolean
- `add tag` — add a tag to the current event context
- `text "string"` or `text "string" with {{templating}}` — text output

### 3.6 Events

An event is a story node with optional prerequisites, tags, text, and choices:

```
event old_ruins:
  requires: visited_old_ruins AND courage >= 5    # prerequisites
  tags: exploration, early_game                    # event tags

  "You stand before ancient stone ruins, half-swallowed by ivy."
  "A cold wind whispers from within."

  choice "Enter the ruins":
    set visited_old_ruins to true
    next river_crossing

  choice "Circle around":
    uses found_mushroom
    next dense_forest
```

**Event fields** (in any order within the event block):
- `requires:` — AND/OR condition expression (optional, inline or multi-line)
- `tags:` — comma-separated list (optional, inline or multi-line)
- Text lines — quoted or unquoted prose (rendered to player)
- `choice:` — one or more choices

**Multi-line `tags:` and `requires:`** — when the keyword is on its own line,
the content follows on indented lines:

```
event old_ruins:
  tags:
    exploration
    early_game
  requires:
    visited_old_ruins AND courage >= 5
  "You stand before ancient stone ruins."
```

Both forms may be mixed freely within the same story.

### 3.7 Choices

```
choice "Attack the wolf":
  + courage by 2
  uses wolf_scare
  "You charge at the wolf."
  next wolf_fight
```

**Choice fields** (in any order):
- Effect inline: `+ stat by N`, `- stat by N`, `set flag to bool`
- `uses effect_name` — append a reusable effect block
- `text "string"` — text output from the choice
- Plain quoted text — rendered text from the choice
- `requires:` — local prerequisite (optional)
- `next event_id` — goto target (required for non-terminal choices)

**Choice text can be templated**:
```
choice "Buy ale (cost: {{gold}} gold)":
```

### 3.8 Prerequisites (Conditions)

The `requires:` field supports AND/OR logic and stat/flag comparisons:

```
requires: courage >= 5 AND hp > 0 AND visited_old_ruins
requires: gold >= 5 OR reputation >= 10
requires: NOT defeated_dragon
requires: (courage >= 5 OR gold >= 100) AND visited_old_ruins
```

**Operators**:
- `>=`, `<=`, `>`, `<`, `==`, `!=` — numeric comparison (stats only)
- `AND`, `OR`, `NOT` — logical combinators
- `flag_name` (bare identifier) — true if flag is set
- Parentheses `( )` — grouping

### 3.9 Text Templating

Writers interpolate current state values into prose using `{{var}}`:

```
"You have {{gold}} gold pieces."
"The barkeep eyes your coin purse. You have {{gold}} gold."
```

- **Only stats are interpolated** (numeric `i64`).
- Flags (boolean) are NOT interpolated.
- If a variable hasn't been declared, the placeholder renders literally
  (e.g., `{{gold}}` remains as-is — so writers notice the missing declaration).
- At **compile time**, template text is parsed into segments:
  literal text + stat lookup. At **runtime**, the VM renders templates
  atomically using `RenderTemplate` instruction.

### 3.10 Story Block

The `story` block wraps all definitions:

```
story ForestAdventure:
  # imports, stats, effects, events all live here
  ...
```

Only one `story` block per file. The story name is used as an identifier.

---

## 4. Bytecode Format (.cyoa.bc)

### 4.1 Design Goals

1. **Zero-copy**: String table at front; VM returns `&str` into loaded memory
2. **Self-contained**: All imports merged; no runtime file access
3. **Versioned**: Magic number + version field for forward compatibility

### 4.2 Binary Layout

```
┌──────────────────┐
│ Header (24 bytes) │  magic (4B) + version (4B) + 4× offset/length pairs
├──────────────────┤
│ String Table      │  All text, deduplicated, interned
│  [offset, len]    │  All text references are (offset, length) pairs
├──────────────────┤
│ Event Index       │  event_id_str → bytecode offset mapping
├──────────────────┤
│ Event Array       │  Fixed-size event headers (id, text range, choice range)
├──────────────────┤
│ Choice Array      │  (indexed by event's choice range)
├──────────────────┤
│ Effect Blocks     │  Reusable effects
├──────────────────┤
│ Instruction Stream│  Bytecode for events + effects + conditions
└──────────────────┘
```

### 4.3 String Table

All text (event text, choice text, stat names, flag names, tags, template
segments) is stored in the string table. Every text reference in the bytecode
points to the string table via `(offset, length)` — this enables zero-copy
`&str` returns from the VM.

### 4.4 Serialization

Serialized using `postcard` (binary, no-std compatible, fast) with `serde`
derive on all format types.

---

## 5. VM Instruction Set

Register-based stack machine. The VM holds references to the loaded bytecode
and returns `&str` into it (zero-copy).

| Instruction | Operand(s) | Description |
|------------|------------|-------------|
| `GetText` | `u32` (string table idx) | Push text into return buffer |
| `RenderTemplate` | `u32` (template idx) | Render `{{stat}}` → push result |
| `GetChoice` | `i32` (choice index) | Return choice at runtime cursor |
| `ApplyEffect` | `u32` (effect block idx) | Execute reusable effect block |
| `ChangeStat` | `i32` delta | Add delta to current stat |
| `SetFlag` | `u32` (flag idx) | Set flag to true |
| `ClearFlag` | `u32` (flag idx) | Set flag to false |
| `AddTag` | `u32` (tag idx) | Add tag to current event context |
| `CheckCondition` | `u32` (condition idx) | Evaluate prereq → push bool |
| `RecordHistory` | `u32` event_id, `i32` choice_idx | Record choice in history |
| `BranchIfTrue` | `u32` addr, `u32` addr | Pop bool; jump if true |
| `Goto` | `u32` addr | Unconditional jump |
| `Return` | — | Return from effect block |

---

## 6. State Model

### 6.1 PlayerState

```rust
pub struct PlayerState {
    stats: BTreeMap<String, i64>,    // hp, gold, courage, ...
    flags: BTreeSet<String>,           // visited_cave, obtained_key, ...
    tags: BTreeSet<String>,            // combat, dangerous (current event tags)
}
```

- One `PlayerState` per engine instance (one per story).
- Serializable to JSON via serde for save/load.
- Game engines never access `PlayerState` directly — they use API calls.

> **Note**: `PlayerState.tags` holds **runtime-applied** tags (added via the `AddTag`
> opcode during play). **Story-level tags** (declared via `tags:` at the story block
> scope) are static metadata stored in the `Bytecode` struct and accessed via
> `Engine::list_story_tags()` — they are distinct from runtime tags and never change
> during play.

### 6.2 StoryCursor

```rust
pub struct StoryCursor {
    current_event: u32,                // ID of the current event
    choice_history: Vec<HistoryEntry>, // choice history
    complete: bool,                    // true after a terminal choice
}

pub struct HistoryEntry {
    event_id: String,
    choice_index: i32,
    choice_text: String,
}
```

- History is recorded via `RecordHistory` instruction.
- Serialized as part of `getState()`/`setState()` for save/load.

### 6.3 Engine

```rust
pub struct Engine {
    bytecode: &'static [u8],    // zero-copy: points into mmap'd bytecode
    state: PlayerState,
    cursor: StoryCursor,
}
```

Each `Engine` instance is independent (multi-story support).

---

## 7. Runtime APIs

### 7.1 WASM / JavaScript API

The WASM bindings expose two classes: `WasmStoryCatalog` for story discovery
and `WasmEngine` for playing a single story. Both are compiled to a `.wasm`
module loadable via ES module `import`.

#### Multi-Story Catalog

```typescript
// Register stories and filter by tags
const catalog = new WasmStoryCatalog();
catalog.register(forestAdventureBytecode);  // Uint8Array
catalog.register(tavernTalesBytecode);

// Discover stories by tag
const fantasyStories = catalog.storiesWithTag("fantasy");
const multiTag = catalog.storiesWithAllTags('["fantasy","exploration"]');

// Create an engine for a story
const engine = catalog.createEngine(0);       // by index
// or
const engine2 = catalog.createEngineByName("ForestAdventure");
```

#### Engine API (high-level — serde-backed, returns JS objects)

```typescript
engine.currentEventId(): string;

engine.getCurrentEvent(): {
  id: string;               // event internal name
  text: string[];           // all paragraphs, templates rendered
  choices: string[];        // visible choice texts (prerequisites filtered)
};

engine.makeChoice(choiceIndex: number): {
  effectText: string[];      // text from effects triggered
  gameStateChanged: boolean;
};

engine.getHistory(): { eventId: string; choiceIndex: number; choiceText: string }[];

engine.getStateJson(): string;      // full state as JSON (for save files)
engine.setStateJson(json: string): void;

engine.listStats(): { [key: string]: number };  // stat name → current value
engine.getStat(name: string): bigint;            // individual stat value

engine.listStoryTags(): string[];   // static story-level tags
engine.listTags(): string[];        // runtime tags applied during play
engine.listFlags(): string[];       // runtime flags currently set

engine.canAccessEvent(id: string): boolean;
engine.availableEvents(): string[];  // all event IDs in the story
engine.isStoryComplete(): boolean;   // true if a terminal choice was made
```

#### Engine API (zero-copy — Uint8Array views into WASM memory)

For maximum performance, the engine exposes raw byte views into WASM linear
memory. These avoid the `TextDecoder` overhead of wasm-bindgen's automatic
`String` conversion. The views are valid until the next method call on the
same engine — call, read, then proceed.

```typescript
// Returns a Uint8Array view of all event text paragraphs, \n-separated
const textBytes: Uint8Array = engine.currentEventTextBytes();
const text: string = new TextDecoder().decode(textBytes);

// Returns a Uint8Array view of all choice texts, \n-separated
const choiceBytes: Uint8Array = engine.currentChoicesBytes();
```

### 7.2 C-ABI API (Native)

The `cyoa-native` crate compiles to a native C-ABI shared library
(`libcyoa_native.so` / `cyoa_native.dll` / `libcyoa_native.dylib`).
The API is handle-based: a `CyoaEngine` handle represents one story instance,
and a `CyoaCatalog` handle manages multiple registered stories with tag filtering.

#### Memory model

| Return type | Lifetime | Free with |
|---|---|---|
| `const char *` | Engine-owned; valid until next call on same handle | — (copy immediately) |
| `char *` | Heap-allocated | `cyoa_free_string()` |

#### Engine API

```c
typedef struct CyoaEngine CyoaEngine;

/* Lifecycle */
CyoaEngine* cyoa_create(const uint8_t* bytecode, size_t len);
void cyoa_destroy(CyoaEngine* engine);

/* Event queries (const char* = engine-owned, valid until next call) */
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

/* State management (char* = heap-allocated, caller must free) */
char* cyoa_get_state_json(CyoaEngine* engine);
void  cyoa_set_state_json(CyoaEngine* engine, const char* json);

/* Queries */
char* cyoa_available_events_json(CyoaEngine* engine);  /* JSON array, caller frees */
int   cyoa_can_access_event(CyoaEngine* engine, const char* id);

/* Stats / tags / flags (char* = heap-allocated, caller must free) */
char* cyoa_list_stats_json(CyoaEngine* engine);
char* cyoa_list_story_tags_json(CyoaEngine* engine);
char* cyoa_list_tags_json(CyoaEngine* engine);
char* cyoa_list_flags_json(CyoaEngine* engine);
int   cyoa_get_stat(CyoaEngine* engine, const char* name);

/* Free a heap-allocated string returned by any *func*_json or get_state_json */
void cyoa_free_string(char* s);
```

#### Catalog API (multi-story + tag filtering)

```c
typedef struct CyoaCatalog CyoaCatalog;

/* Lifecycle */
CyoaCatalog* cyoa_catalog_create(void);
void cyoa_catalog_destroy(CyoaCatalog* catalog);

/* Registration */
int cyoa_catalog_register(CyoaCatalog* catalog, const uint8_t* bytecode, size_t len);
int cyoa_catalog_story_count(const CyoaCatalog* catalog);

/* Discovery (char* = heap-allocated JSON, caller must free) */
char* cyoa_catalog_list_stories_json(const CyoaCatalog* catalog);
char* cyoa_catalog_stories_with_tag_json(const CyoaCatalog* catalog, const char* tag);
char* cyoa_catalog_stories_with_all_tags_json(const CyoaCatalog* catalog, const char* tags_json);
char* cyoa_catalog_stories_with_any_tags_json(const CyoaCatalog* catalog, const char* tags_json);

/* Engine creation from catalog */
CyoaEngine* cyoa_catalog_create_engine(CyoaCatalog* catalog, int index);
CyoaEngine* cyoa_catalog_create_engine_by_name(CyoaCatalog* catalog, const char* name);
```

**Catalog discovery returns** JSON arrays of `{"name":"string","tags":["..."]}` objects.
`tags_json` arguments accept a JSON array of tag strings (e.g. `["fantasy","exploration"]`).

---

## 8. Standard Library (std/)

Shipped with the compiler. Imported via `import "std/..."`:

| Package | Contents |
|---------|----------|
| `std/healing` | `healing_potion`, `minor_heal`, `full_restore` effects |
| `std/combat` | `basic_attack`, `critical_hit`, `dodge` effects |

Phase 1 packages are minimal — they grow as features are developed.

---

## 9. Implementation Phases

| Phase | Goal | Timeline |
|-------|------|----------|
| Phase 1 | Core engine: parser → bytecode → VM (native Rust) | 4–6 weeks |
| Phase 2 | WASM + Web demo (zero-copy string table) | 2–3 weeks |
| Phase 3 | Native C-ABI + Unity C# wrapper | 3–4 weeks |
| Phase 4 | Mobile + Godot + LSP + tooling | 3–5 weeks |
| Phase 4.1 | Godot (GDScript + C#/.NET) bindings | 1 week |
| Phase 4.2 | Language Server Protocol (LSP) server | 1 week |
| Phase 4.3 | Mobile cross-compilation docs (Android/iOS) | 3 days |
| Phase 5 | Articy:Draft import | Future |

---

## 10. Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | 2026-08-18 | Initial spec — Phase 1 foundation |
| 0.2.0 | 2026-08-19 | Phase 2 — WASM bindings: WasmStoryCatalog, WasmEngine, zero-copy text, web demo |
| 0.3.0 | 2026-08-19 | Phase 3 — C-ABI native bindings: CyoaEngine, CyoaCatalog, JSON-based state/tags/flags, C# Unity wrapper |
| 0.4.0 | 2026-08-20 | Phase 4 — Godot integration: C# wrapper (bindings/godot/csharp/), GDScript wrapper (bindings/godot/gdscript/), bindings/ directory reorg |
| 0.5.0 | 2026-08-20 | Phase 4 complete — LSP server (cyoa-lsp crate), mobile cross-compilation guide (docs/mobile.md), Android (cargo-ndk) + iOS (cargo-lipo) instructions |
