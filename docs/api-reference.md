# CYOA Engine API Reference

> Version 0.5.0 — Full API reference for all language bindings.

This directory contains per-language API references. Each page describes
all available functions in that language with examples and memory contracts.

## Available APIs

| Document | Languages | Use Cases |
|----------|-----------|-----------|
| [WASM / JavaScript API](wasm-api.md) | JavaScript / TypeScript / Web | Web apps, browser games, web-based story editors |
| [C-ABI API](c-abi-api.md) | C / C++ / C# | Native apps, Unity, any language with FFI |
| [C# API](c-abi-api.md#csharp-unity-wrapper) | C# (Unity) | Unity game engine integration |
| [Rust Runtime API](rust-api.md) | Rust | Native Rust games, engine development |
| [GDScript API](gdscript-api.md) | GDScript (Godot) | Godot game engine integration |

---

## Architecture Overview

The CYOA engine follows a handle-based architecture. Each story is an
independent `Engine` instance. A `StoryCatalog` manages multiple registered
stories and provides tag-based filtering for game engines that need to discover
stories at runtime (e.g., random event systems).

```
┌─────────────┐    register    ┌──────────────────┐
│ Game Engine │ ─────────────→│  StoryCatalog    │
│  (JS/C#/     │               │  (multi-story)   │
│   GDScript)  │               └────────┬─────────┘
└─────────────┘                        │ create_engine(i)
                                       ▼
                              ┌──────────────────┐
                              │    Engine        │
                              │  (per story)     │
                              └────────┬─────────┘
                                       │
                              ┌────────▼──────────┐
                              │  State & Cursor   │
                              │  (owned by VM)    │
                              └───────────────────┘
```

### Key design principles

- **One `Engine` per story** — state is never shared between stories. The
  game engine handles cross-story coordination (e.g., passing gold between quests).
- **Zero-copy text** — the VM reads text directly from bytecode memory; no
  allocation on the hot path (WASM returns `Uint8Array` views, native returns
  `const char*` into engine-owned buffers).
- **Declarative DSL** — writers describe "what exists" (events, choices,
  effects) rather than imperative control flow.

---

## Common Concepts (all languages)

### State JSON format

Every API provides `getStateJson()` / `set_state_json()` for save/load between
sessions. The JSON format is identical across all languages:

```json
{
  "stats": { "hp": 50, "courage": 3, "gold": 10 },
  "flags": ["visited_old_ruins", "wolf_friend"],
  "tags": ["combat"],
  "current_event": 3,
  "choice_history": [
    { "event_id": "old_ruins", "choice_index": 0, "choice_text": "Enter the ruins" }
  ],
  "complete": false
}
```

- **`stats`**: Stat name → current `i64` value (note: JS returns these as `bigint`)
- **`flags`**: Set of flag names currently set
- **`tags`**: Runtime tags applied during play (not story-level tags)
- **`current_event`**: Event table index for cursor restoration
- **`choice_history`**: All choices made, for replay/analytics
- **`complete`**: `true` if a terminal choice was made (story has ended)

> **Important**: `tags` in state are **runtime-applied** tags (added during play
> via `add tag`). **Story-level tags** are static metadata in the bytecode and
> are queried via `listStoryTags()` / `list_story_tags()` — they are not part
> the state JSON.

### Naming conventions

| Layer | Convention | Example |
|-------|-----------|---------|
| Rust | `snake_case` | `current_event_id()`, `stories_with_any_tags()` |
| WASM (JS) | `camelCase` | `currentEventId()`, `storiesWithAnyTags()` |
| C-ABI | `snake_case` + `cyoa_` prefix | `cyoa_current_event_id()`, `cyoa_catalog_stories_with_any_tags_json()` |
| C# | `PascalCase` | `CurrentEventId`, `StoriesWithAnyTags()` |
| GDScript | `snake_case` | `get_current_event_id()`, `stories_with_any_tags()` |

## Documentation Sync

Updated per the [Documentation Sync Protocol](../CLAUDE.md#documentation-sync-protocol):

1. `SPEC.md` — language spec, bytecode format, VM instructions
2. `CLAUDE.md` — build/test commands, project structure
3. `README.md` — user-facing quick start and examples
4. `docs/syntax.md` — extended syntax reference
5. `docs/api-reference.md` + language-specific pages — engine API reference
6. `docs/mobile.md` — mobile cross-compilation
7. `bindings/*/README.md` — language-specific integration guides