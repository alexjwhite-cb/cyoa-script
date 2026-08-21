# WASM / JavaScript API Reference

The WASM module is built with `wasm-bindgen` and exported as an ES module.
It provides two classes: `WasmStoryCatalog` (multi-story discovery) and
`WasmEngine` (playback for a single story).

## Loading

```typescript
import init, { WasmStoryCatalog, WasmEngine } from './cyoa_wasm.js';

// Must be called once before using any exported class
await init();
```

---

## WasmStoryCatalog

A registry of compiled stories with tag-based filtering. Use this to discover
stories (especially for random event systems) before creating an engine to play
them.

### Constructor

#### `new WasmStoryCatalog()`

Creates an empty story catalog.

```typescript
const catalog = new WasmStoryCatalog();
```

### Methods

#### `register(bytes: Uint8Array): void`

Register a compiled story from bytecode bytes. The bytecode must have been
produced by `cyoa compile`.

```typescript
const response = await fetch('./forest_adventure.cyoa.bc');
const bytes = new Uint8Array(await response.arrayBuffer());
catalog.register(bytes);
```

**Throws** if the bytecode fails to decode.

#### `len(): number`

Number of registered stories.

```typescript
console.log(catalog.len());  // → 2
```

#### `isEmpty(): boolean`

Whether the catalog has zero registered stories.

```typescript
if (catalog.isEmpty()) {
  console.log("No stories loaded yet.");
}
```

#### `listStories(): { name: string; tags: string[] }[]`

List all registered stories with their names and tags.

```typescript
const stories = catalog.listStories();
// → [{ name: "ForestAdventure", tags: ["fantasy", "exploration"] }, ...]
```

#### `storiesWithTag(tag: string): { name: string; tags: string[] }[]`

Find all stories that have a specific tag.

```typescript
const fantasyStories = catalog.storiesWithTag("fantasy");
// → [{ name: "ForestAdventure", tags: ["fantasy", "exploration"] }]
```

#### `storiesWithAllTags(tagsJson: string): { name: string; tags: string[] }[]`

Find all stories that have **ALL** of the specified tags.

`tagsJson` is a JSON array of tag strings:

```typescript
const stories = catalog.storiesWithAllTags('["fantasy", "exploration"]');
```

#### `storiesWithAnyTags(tagsJson: string): { name: string; tags: string[] }[]`

Find all stories that have **ANY** of the specified tags.

```typescript
const stories = catalog.storiesWithAnyTags('["combat", "social"]');
```

#### `createEngine(index: number): WasmEngine \| null`

Create a `WasmEngine` for the story at the given index (0-based).

Returns `null` if the index is out of bounds.

```typescript
const engine = catalog.createEngine(0);
```

#### `createEngineByName(name: string): WasmEngine \| null`

Find a story by its declared name and create a `WasmEngine` for it.

Returns `null` if no story with that name is registered.

```typescript
const engine = catalog.createEngineByName("ForestAdventure");
```

---

## WasmEngine

A CYOA story engine instance. Each engine is independent — state is never
shared between instances. Create one per story (main quest, side quests, etc.).

### Constructor

#### `new WasmEngine(bytes: Uint8Array)`

Create an engine directly from compiled bytecode bytes. Alternatively, use
`WasmStoryCatalog.createEngine()` to create engines from a catalog.

```typescript
const engine = new WasmEngine(bytecodeBytes);
```

**Throws** if the bytecode fails to decode.

### High-Level API

#### `currentEventId(): string`

Get the current event's internal ID (name).

```typescript
console.log(engine.currentEventId());
// → "old_ruins"
```

#### `getCurrentEvent(): { id: string; text: string[]; choices: string[] }`

Get the current event as a JavaScript object. Templates are already rendered
(e.g., `{{gold}}` → `42`). Choices with unmet prerequisites are excluded
from the list.

```typescript
const event = engine.getCurrentEvent();
// → {
//     id: "old_ruins",
//     text: ["You stand before ancient stone ruins...", "A cold wind whispers..."],
//     choices: ["Enter the ruins", "Circle around"]
//   }
```

#### `makeChoice(choiceIndex: number): { effectText: string[]; gameStateChanged: boolean }`

Make a choice at the given 0-based index. The index references only choices
visible to the player (prerequisites are already filtered).

The returned `effectText` array contains any text produced by the choice's
effects (e.g., "You find a glowing mushroom.").

```typescript
const result = engine.makeChoice(0);
// → { effectText: ["You find a glowing mushroom. It hums softly."], gameStateChanged: true }
```

#### `getHistory(): { eventId: string; choiceIndex: number; choiceText: string }[]`

Get the choice history as an array of objects. Useful for UI display,
analytics, and save/load systems.

```typescript
const history = engine.getHistory();
// → [
//     { eventId: "start", choiceIndex: 0, choiceText: "Enter the forest" },
//     { eventId: "forest_path", choiceIndex: 1, choiceText: "Return to the entrance" },
//   ]
```

#### `getStateJson(): string`

Get the full player state as a JSON string (for save files). The JSON includes
`stats`, `flags`, `tags`, `current_event`, `choice_history`, and `complete`
— enabling full save/load between sessions.

```typescript
const saveJson = engine.getStateJson();
localStorage.setItem("my-save", saveJson);
```

**Memory contract**: The returned string is valid until the next method call
on this engine.

#### `setStateJson(json: string): void`

Restore state from a JSON string produced by `getStateJson()`.

```typescript
const saveJson = localStorage.getItem("my-save");
engine.setStateJson(saveJson);
```

**Throws** if the JSON is malformed or doesn't match the expected schema.

#### `listStats(): { [key: string]: number }`

Get stats as a JavaScript object mapping stat names to current values.

```typescript
const stats = engine.listStats();
// → { hp: 50, courage: 3, gold: 10 }
```

#### `getStat(name: string): bigint`

Get a single stat value by name. Returns `0` if the stat doesn't exist.

**Note**: The value is returned as a `bigint` in JavaScript (Rust's `i64`
maps to JS `bigint`). For typical game values (which fit in `i32`), convert
with `Number()`.

```typescript
const hp = Number(engine.getStat("hp"));
// → 50
```

#### `listStoryTags(): string[]`

List story-level tags — static metadata declared in the `.cyoa` file at the
story block level. These do **not** change during play.

```typescript
console.log(engine.listStoryTags());
// → ["fantasy", "exploration"]
```

#### `listTags(): string[]`

List runtime tags currently applied during play (added by `add tag` or the
`AddTag` opcode).

```typescript
console.log(engine.listTags());
// → ["combat"]
```

#### `listFlags(): string[]`

List all runtime flags currently set (boolean story progress markers).

```typescript
console.log(engine.listFlags());
// → ["visited_old_ruins", "wolf_friend"]
```

#### `canAccessEvent(id: string): boolean`

Check if an event by ID is reachable from the current story graph.

```typescript
if (engine.canAccessEvent("wolf_fight")) {
  console.log("The wolf fight event is reachable.");
}
```

#### `availableEvents(): string[]`

List all event IDs in the story. Useful for quest logs and event discovery.

```typescript
const events = engine.availableEvents();
// → ["start", "forest_path", "old_ruins", "forest_encounter", ...]
```

#### `isStoryComplete(): boolean`

Returns `true` if the story has ended — i.e., a terminal choice was made
(a choice with no `next` event). After this returns `true`, there are no
more choices to display.

```typescript
engine.makeChoice(0);
if (engine.isStoryComplete()) {
  console.log("The story is over.");
}
```

---

## Zero-Copy Text Access

For maximum performance when rendering large blocks of text, the engine
provides `Uint8Array` views directly into WASM linear memory. These avoid
the `TextDecoder` serialization overhead of the serde-based high-level API.

The views are valid **until the next method call on the same engine**.
Typical usage pattern: **call → decode → use → next call is safe**.

### `currentEventTextBytes(): Uint8Array`

Returns a `Uint8Array` view of all current event text paragraphs,
newline-separated (`\n`).

```typescript
const bytes = engine.currentEventTextBytes();
const text = new TextDecoder().decode(bytes);
```

### `currentChoicesBytes(): Uint8Array`

Returns a `Uint8Array` view of all current choice texts, newline-separated.

```typescript
const bytes = engine.currentChoicesBytes();
const choices = new TextDecoder().decode(bytes).split('\n');
```

**Performance note**: The zero-copy API avoids wasm-bindgen's `String`
allocation + `TextDecoder.decode()` overhead. Benchmarks show 2–5× speedup
depending on text length.

---

## TypeScript Declarations

```typescript
export class WasmStoryCatalog {
  constructor();
  register(bytes: Uint8Array): void;
  len(): number;
  isEmpty(): boolean;
  listStories(): Array<{ name: string; tags: string[] }>;
  storiesWithTag(tag: string): Array<{ name: string; tags: string[] }>;
  storiesWithAllTags(tagsJson: string): Array<{ name: string; tags: string[] }>;
  storiesWithAnyTags(tagsJson: string): Array<{ name: string; tags: string[] }>;
  createEngine(index: number): WasmEngine | undefined;
  createEngineByName(name: string): WasmEngine | undefined;
}

export class WasmEngine {
  constructor(bytes: Uint8Array);

  currentEventId(): string;
  getCurrentEvent(): { id: string; text: string[]; choices: string[] };
  makeChoice(choiceIndex: number): { effectText: string[]; gameStateChanged: boolean };
  getHistory(): Array<{ eventId: string; choiceIndex: number; choiceText: string }>;
  getStateJson(): string;
  setStateJson(json: string): void;
  listStats(): { [key: string]: number };
  getStat(name: string): bigint;
  listStoryTags(): string[];
  listTags(): string[];
  listFlags(): string[];
  canAccessEvent(id: string): boolean;
  availableEvents(): string[];
  isStoryComplete(): boolean;

  // Zero-copy text API
  currentEventTextBytes(): Uint8Array;
  currentChoicesBytes(): Uint8Array;
}

export default function init(): Promise<void>;
```

The full `.d.ts` file is in [`web-demo/cyoa.d.ts`](../web-demo/cyoa.d.ts).