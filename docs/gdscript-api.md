# GDScript API Reference

The GDScript wrapper (`cyoa_engine.gd`) provides a Godot-friendly API that
delegates to the C# FFI layer. Since GDScript cannot call native C functions
directly, the C# `CyoaEngine` / `CyoaStoryCatalog` class acts as the
bridge — this script is a thin wrapper around it.

## Setup

```
YourGodotProject/
├── addons/
│   └── cyoa/
│       ├── cyoa_engine.gd        # GDScript wrapper (this file)
│       ├── scripts/
│       │   └── CyoaEngine.cs     # C# FFI bridge
│       └── native/
│           ├── libcyoa_native.so # (or .dll / .dylib)
│           └── forest_adventure.cyoa.bc
```

Load the script:

```gdscript
var engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
```

## Two Usage Patterns

The `CyoaEngineGD` class supports **catalog mode** and **engine mode** in a
single class. Catalog mode is used first to discover and filter stories;
once an engine is created, the instance automatically switches to engine mode.

### Catalog mode

```gdscript
# Register multiple stories and filter by tags
var catalog = preload("res://addons/cyoa/cyoa_engine.gd").new()
catalog.register_story("res://addons/cyoa/native/forest_adventure.cyoa.bc")
catalog.register_story("res://addons/cyoa/native/tavern_tales.cyoa.bc")

# Discover stories
var fantasy_stories = catalog.stories_with_tag("fantasy")

# Create an engine — returns a new CyoaEngineGD instance in engine mode
var engine = catalog.create_engine_by_name("ForestAdventure")
```

### Engine mode

```gdscript
var engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
engine.load_from_path("res://addons/cyoa/native/forest_adventure.cyoa.bc")
```

---

## Catalog API

### `register_story(path: String) -> void`

Register a compiled story from a `.cyoa.bc` file path. Switches the instance
to catalog mode.

```gdscript
catalog.register_story("res://addons/cyoa/native/forest_adventure.cyoa.bc")
```

### `register_story_from_bytes(bytes: PackedVectorByteArray) -> void`

Register a compiled story from bytecode bytes. Switches to catalog mode.

```gdscript
var bytes = FileAccess.get_file_as_bytes("res://addons/cyoa/native/forest_adventure.cyoa.bc")
catalog.register_story_from_bytes(bytes)
```

### `story_count() -> int`

Number of registered stories in catalog mode.

```gdscript
print(catalog.story_count())  # → 2
```

### `list_stories() -> Array`

List all registered stories as an array of `{ name, tags }` dictionaries.

```gdscript
for story in catalog.list_stories():
    print(story.name, story.tags)
```

### `stories_with_tag(tag: String) -> Array`

Find all stories that have a specific tag.

```gdscript
var fantasy = catalog.stories_with_tag("fantasy")
```

### `stories_with_all_tags(tags: Array) -> Array`

Find all stories that have **ALL** of the specified tags.

```gdscript
var stories = catalog.stories_with_all_tags(["fantasy", "exploration"])
```

### `stories_with_any_tags(tags: Array) -> Array`

Find all stories that have **ANY** of the specified tags.

```gdscript
var stories = catalog.stories_with_any_tags(["combat", "social"])
```

### `create_engine(index: int) -> Object`

Create a `CyoaEngineGD` instance (in engine mode) for the story at the
given catalog index. Returns `null` if out of bounds.

```gdscript
var engine = catalog.create_engine(0)
```

### `create_engine_by_name(name: String) -> Object`

Find a story by name and create a `CyoaEngineGD` instance for it.
Returns `null` if not found.

```gdscript
var engine = catalog.create_engine_by_name("ForestAdventure")
```

### `_exit_tree() -> void`

Cleanup method — frees the C# engine or catalog instance. Call
`queue_free()` on the `CyoaEngineGD` instance rather than calling directly.

---

## Engine API

All engine-mode methods return empty/zero values if the engine hasn't been
loaded or if `_engine` is `null`.

### `load_from_path(path: String) -> Error`

Load a story from a `.cyoa.bc` file path. Returns `OK` on success,
`ERR_FILE_NOT_FOUND` if the file doesn't exist.

```gdscript
var err = engine.load_from_path("res://addons/cyoa/native/forest_adventure.cyoa.bc")
assert(err == OK)
```

### `load_from_bytes(bytecode: PackedVectorByteArray) -> Error`

Load a story from bytecode bytes. Returns `OK` on success,
`ERR_INVALID_INPUT` if bytes are empty.

```gdscript
var bytes = FileAccess.get_file_as_bytes(path)
var err = engine.load_from_bytes(bytes)
```

### `get_current_event_id() -> String`

Internal event ID (name). Returns `""` if no engine loaded.

```gdscript
print(engine.get_current_event_id())  # → "old_ruins"
```

### `get_current_event_text() -> String`

Current event text paragraphs joined by `\n`. Returns `""` if no engine loaded.

```gdscript
print(engine.get_current_event_text())
```

### `get_choice_count() -> int`

Number of currently available choices.

```gdscript
var count = engine.get_choice_count()
for i in range(count):
    print(engine.get_choice_text(i))
```

### `get_choice_text(index: int) -> String`

Text of the choice at the given index. Returns `""` if out of bounds.

### `get_current_choices() -> PackedStringArray`

All current choice texts as an array.

```gdscript
var choices = engine.get_current_choices()
for choice in choices:
    print(choice)
```

### `make_choice(index: int) -> void`

Apply the player's choice at the given 0-based index. After this call, the
engine has advanced to the next event.

```gdscript
engine.make_choice(0)
print(engine.get_current_event_text())
```

### `get_last_effect_text() -> String`

Effect text from the most recent `make_choice()` call. Multiple effect
fragments are joined by `\n`. Returns `""` if no engine loaded.

```gdscript
engine.make_choice(0)
print(engine.get_last_effect_text())
```

---

## History API

### `get_history_length() -> int`

Number of entries in the choice history.

### `get_history_entry(index: int) -> Dictionary`

Get a single history entry as `{ event_id, choice_index, choice_text }`.
Returns `{}` if out of bounds.

```gdscript
for i in range(engine.get_history_length()):
    var entry = engine.get_history_entry(i)
    print(entry.event_id, entry.choice_index, entry.choice_text)
```

### `get_all_history() -> Array`

All history entries as an array of dictionaries.

```gdscript
for entry in engine.get_all_history():
    print(entry.event_id, entry.choice_index)
```

---

## State API

### `get_state_json() -> String`

Full player state as JSON (stats, flags, tags, current_event, choice_history).
Use for save files.

```gdscript
var save_json = engine.get_state_json()
# Save to file, database, etc.
```

### `set_state_json(json: String) -> void`

Restore state from a JSON string produced by `get_state_json()`.

```gdscript
engine.set_state_json(save_json)
```

---

## Queries

### `get_available_events() -> PackedStringArray`

All event IDs in the story. Useful for quest logs.

### `can_access_event(id: String) -> bool`

Whether an event by ID exists in the story's event index.

```gdscript
if engine.can_access_event("wolf_fight"):
    print("Wolf fight is reachable.")
```

---

## Stats / Tags / Flags

### `get_stats() -> Dictionary`

Stat name → current value as a `Dictionary`.

```gdscript
var stats = engine.get_stats()
print(stats["hp"])  # → 50
```

### `get_stat(name: String) -> int`

Individual stat value. Returns `0` if the stat doesn't exist.

### `get_story_tags() -> PackedStringArray`

Story-level tags (static metadata from the `.cyoa` file).

### `get_tags() -> PackedStringArray`

Runtime tags currently applied during play.

### `get_flags() -> PackedStringArray`

Runtime flags currently set.

---

## Full Example

```gdscript
# Load the engine wrapper
var engine = preload("res://addons/cyoa/cyoa_engine.gd").new()
engine.load_from_path("res://addons/cyoa/native/forest_adventure.cyoa.bc")

# Show the current event
print("Event: %s" % engine.get_current_event_id())
print(engine.get_current_event_text())

# Show choices
var choices = engine.get_current_choices()
for i in range(choices.size()):
    print("  [%d] %s" % [i, choices[i]])

# Make a choice
engine.make_choice(0)

# Save state
var save_json = engine.get_state_json()
# → {"stats":{...},"flags":[...],"tags":[...],"current_event":3,"choice_history":[...]}

# Later — restore state
engine.set_state_json(save_json)

# History
for entry in engine.get_all_history():
    print("  %s → %s" % [entry.event_id, entry.choice_text])

# Cleanup (if not using auto-free via _exit_tree)
engine._exit_tree()
```

---

See [`bindings/godot/README.md`](../bindings/godot/README.md) for the full
Godot integration guide, including C# wrapper API and mobile setup.