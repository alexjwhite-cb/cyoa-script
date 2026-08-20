# Rust Runtime API Reference

The `cyoa-runtime` crate provides the full engine API used by both the WASM
and native bindings. Writers building native Rust games can use this directly
without FFI. Writers building game engines can also compile stories from
source using `cyoa-compiler`.

## Cargo dependency

```toml
[dependencies]
cyoa-runtime = { path = "cyoa-runtime" }
cyoa-bytecode = { path = "cyoa-bytecode" }
```

## Engine

### Construction

```rust
use cyoa_runtime::Engine;
use cyoa_bytecode::Bytecode;

let bytes: Vec<u8> = std::fs::read("forest_adventure.cyoa.bc")?;
let bc = Bytecode::from_bytes(&bytes)?;
let mut engine = Engine::new(bc);
```

### Current event

```rust
engine.current_event_id() -> String           // internal event ID (name)
engine.current_event_text() -> Vec<String>    // text paragraphs
engine.current_choices() -> Vec<String>       // visible choice texts
```

**Example**:
```rust
println!("Event: {}", engine.current_event_id());
for paragraph in engine.current_event_text() {
    println!("{}", paragraph);
}
let choices = engine.current_choices();
for (i, choice) in choices.iter().enumerate() {
    println!("  [{}] {}", i, choice);
}
```

### Making choices

```rust
engine.make_choice(index: i32) -> Vec<String>   // effect text from the choice
```

**Example**:
```rust
let effects = engine.make_choice(0);
for text in effects {
    println!("{}", text);
}
```

### History

```rust
engine.history() -> &[HistoryEntry]
```

`HistoryEntry` fields:
- `event_id: String` — the event where the choice was made
- `choice_index: i32` — 0-based index of the choice
- `choice_text: String` — text of the chosen choice

**Example**:
```rust
for entry in engine.history() {
    println!("{}: chose '{}' ({})", entry.event_id, entry.choice_text, entry.choice_index);
}
```

### State management

```rust
engine.get_state_json() -> String
engine.set_state_json(&json: &str) -> Result<(), serde_json::Error>
```

`get_state_json()` returns a comprehensive JSON string containing stats,
flags, tags, current_event index, and choice_history — enabling full save/load
between sessions.

`set_state_json()` parses and restores all fields, including restoring the
story cursor to the correct position.

**Example**:
```rust
let save = engine.get_state_json();
// ... save to disk / database / etc ...

// Later, or in a new session:
let mut engine2 = Engine::new(bytecode);
engine2.set_state_json(&save)?;
```

### Stats / tags / flags

```rust
engine.stats_json() -> String        // JSON: {"statName": value, ...}
engine.get_stat(name: &str) -> i64    // 0 if stat doesn't exist
engine.list_flags() -> Vec<String>   // runtime flags currently set
engine.list_tags() -> Vec<String>    // runtime tags applied during play
engine.list_story_tags() -> Vec<String>  // static story-level tags
```

### Queries

```rust
engine.can_access_event(id: &str) -> bool    // event reachable?
engine.available_events() -> Vec<String>     // all event IDs
```

---

## StoryCatalog

Multi-story registry with tag-based filtering. Each story becomes an
independent `Engine` instance.

```rust
use cyoa_runtime::{StoryCatalog, StoryMetadata};

let mut catalog = StoryCatalog::new();
catalog.register(bytecode1);
catalog.register(bytecode2);

// Discovery
catalog.list_stories() -> Vec<StoryMetadata>;
catalog.stories_with_tag(tag: &str) -> Vec<&StoryMetadata>;
catalog.stories_with_all_tags(tags: &[&str]) -> Vec<&StoryMetadata>;
catalog.stories_with_any_tags(tags: &[&str]) -> Vec<&StoryMetadata>;

// Engine creation
catalog.create_engine(index: usize) -> Option<Engine>;
catalog.create_engine_by_name(name: &str) -> Option<Engine>;

// Metadata
catalog.len() -> usize;
catalog.is_empty() -> bool;
```

**Example**:
```rust
let fantasy = catalog.stories_with_tag("fantasy");
if let Some(story) = fantasy.first() {
    let engine = catalog.create_engine_by_name(&story.name).unwrap();
    println!("Playing: {}", story.name);
}
```

### StoryMetadata

```rust
pub struct StoryMetadata {
    pub name: String,
    pub tags: Vec<String>,
}
```

---

## PlayerState

The `PlayerState` struct is serializable via serde and is what
`get_state_json()` / `set_state_json()` operate on.

```rust
pub struct PlayerState {
    pub stats: BTreeMap<String, i64>,    // hp, gold, courage, ...
    pub flags: BTreeSet<String>,           // visited_cave, obtained_key, ...
    pub tags: BTreeSet<String>,            // combat, dangerous (current event tags)
}
```

> **Note**: `PlayerState.tags` holds **runtime-applied** tags (added via the
> `AddTag` opcode during play). **Story-level tags** (declared via `tags:` at
> the story block scope) are static metadata stored in the `Bytecode` struct
> and accessed via `Engine::list_story_tags()` — they are distinct from runtime
> tags and never change during play.

## StoryCursor

```rust
pub struct StoryCursor {
    pub current_event: u32,                // index of the current event
    pub choice_history: Vec<HistoryEntry>, // choice history
}
```

## HistoryEntry

```rust
pub struct HistoryEntry {
    pub event_id: String,
    pub choice_index: i32,
    pub choice_text: String,
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
let bytes: Vec<u8> = bytecode.to_bytes()?;      // serialize
let bc: Bytecode = Bytecode::from_bytes(&bytes)?;  // deserialize
```

---

## CLI Reference

The `cyoa-cli` binary provides three commands:

```
cyoa compile <story.cyoa>      # Compile .cyoa → .cyoa.bc
cyoa play <story.cyoa.bc>       # Interactive play mode
cyoa validate <story.cyoa>       # Validate without output
```

**Examples**:
```bash
# Compile a story
cargo run -p cyoa-cli -- compile examples/forest_adventure.cyoa

# Play interactively
cargo run -p cyoa-cli -- play examples/forest_adventure.cyoa.bc

# Validate without output
cargo run -p cyoa-cli -- validate examples/forest_adventure.cyoa
```

### Compilation pipeline

1. **Parse pass**: parse main + all transitively-imported files
2. **Merge pass**: build dependency graph, detect cycles, resolve name collisions
3. **Codegen pass**: emit unified bytecode

See [README.md](../README.md#quick-start) for usage examples and [SPEC.md](../SPEC.md) for the full language specification.