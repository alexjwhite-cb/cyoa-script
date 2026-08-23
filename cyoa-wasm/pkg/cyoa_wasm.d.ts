/* tslint:disable */
/* eslint-disable */

/**
 * A CYOA story engine instance.
 *
 * Each engine is independent — state is never shared between instances.
 * Create one per story (main quest, side quests, etc.).
 */
export class WasmEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * List all event IDs in the story (useful for quest logs).
     */
    availableEvents(): string[];
    /**
     * Check if an event by ID is reachable from the current story graph.
     */
    canAccessEvent(id: string): boolean;
    /**
     * Zero-copy: returns a `Uint8Array` view of all current choice texts,
     * newline-separated.
     *
     * The bytes live in WASM linear memory and are valid until the next
     * method call on this engine.
     */
    currentChoicesBytes(): Uint8Array;
    /**
     * Get the current event's internal ID (name).
     */
    currentEventId(): string;
    /**
     * Zero-copy: returns a `Uint8Array` view of all current event text
     * paragraphs, newline-separated.
     *
     * The bytes live in WASM linear memory and are valid until the next
     * method call on this engine. Use `TextDecoder` to decode:
     *
     * ```js
     * const bytes = engine.currentEventTextBytes();
     * const text = new TextDecoder().decode(bytes);
     * ```
     */
    currentEventTextBytes(): Uint8Array;
    /**
     * Get the current event as a JS object:
     * ```json
     * { "id": "old_ruins", "text": ["...", "..."], "choices": ["...", "..."] }
     * ```
     */
    getCurrentEvent(): any;
    /**
     * Get the choice history as an array of `{ eventId, choiceIndex, choiceText }`.
     */
    getHistory(): any;
    /**
     * Get a stat value by name. Returns 0 if the stat doesn't exist.
     */
    getStat(name: string): bigint;
    /**
     * Get the full player state as a JSON string.
     * Use this for save files. The JSON contains `stats`, `flags`, and `tags`.
     *
     * The returned string is valid until the next method call on this engine.
     */
    getStateJson(): string;
    /**
     * List all runtime flags currently set.
     */
    listFlags(): string[];
    /**
     * Get stats as a JS object mapping stat names to current values.
     */
    listStats(): any;
    /**
     * List story-level tags declared in the story (static metadata).
     * These are distinct from runtime tags (see [`listTags`](Self::listTags)).
     */
    listStoryTags(): string[];
    /**
     * List runtime tags currently applied during play.
     */
    listTags(): string[];
    /**
     * Make a choice at the given index.
     * Returns `{ effectText: string[], gameStateChanged: boolean }`.
     *
     * `choiceIndex` is 0-based, referencing only choices visible to the player
     * (prerequisites are already filtered).
     */
    makeChoice(choice_index: number): any;
    /**
     * Create an engine directly from compiled bytecode bytes.
     *
     * Alternatively, use [`WasmStoryCatalog::createEngine`] to create engines
     * from a catalog of tagged stories.
     */
    constructor(bytes: Uint8Array);
    /**
     * Restore state from a JSON string produced by [`getStateJson`](Self::get_state_json).
     */
    setStateJson(json: string): void;
}

/**
 * A registry of compiled stories with tag-based filtering.
 *
 * Game engines (especially random event systems) use this to discover
 * and filter stories by tags before creating a `WasmEngine` to play them.
 */
export class WasmStoryCatalog {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Create an `Engine` for the story at the given index.
     * Returns `null` if the index is out of bounds.
     */
    createEngine(index: number): WasmEngine | undefined;
    /**
     * Find a story by name and create an `Engine` for it.
     * Returns `null` if no story with that name is registered.
     */
    createEngineByName(name: string): WasmEngine | undefined;
    /**
     * Whether the catalog is empty.
     */
    isEmpty(): boolean;
    /**
     * Number of registered stories.
     */
    len(): number;
    /**
     * List all registered stories as an array of `{ name, tags }` objects.
     *
     * ```json
     * [{"name": "ForestAdventure", "tags": ["fantasy", "exploration"]}, ...]
     * ```
     */
    listStories(): any;
    /**
     * Create an empty catalog.
     */
    constructor();
    /**
     * Register a compiled story from bytecode bytes.
     *
     * The bytecode must have been produced by `cyoa compile`.
     * Story name and tags are extracted from the bytecode at registration.
     */
    register(bytes: Uint8Array): void;
    /**
     * Find all stories that have ALL of the specified tags.
     *
     * `tagsJson` is a JSON array of tag strings, e.g. `"[\"fantasy\", \"exploration\"]"`.
     */
    storiesWithAllTags(tags_json: string): any;
    /**
     * Find all stories that have ANY of the specified tags.
     *
     * `tagsJson` is a JSON array of tag strings, e.g. `"[\"fantasy\", \"combat\"]"`.
     */
    storiesWithAnyTags(tags_json: string): any;
    /**
     * Find all stories that have a specific tag.
     */
    storiesWithTag(tag: string): any;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmengine_free: (a: number, b: number) => void;
    readonly __wbg_wasmstorycatalog_free: (a: number, b: number) => void;
    readonly wasmengine_availableEvents: (a: number) => [number, number];
    readonly wasmengine_canAccessEvent: (a: number, b: number, c: number) => number;
    readonly wasmengine_currentChoicesBytes: (a: number) => any;
    readonly wasmengine_currentEventId: (a: number) => [number, number];
    readonly wasmengine_currentEventTextBytes: (a: number) => any;
    readonly wasmengine_getCurrentEvent: (a: number) => [number, number, number];
    readonly wasmengine_getHistory: (a: number) => [number, number, number];
    readonly wasmengine_getStat: (a: number, b: number, c: number) => bigint;
    readonly wasmengine_getStateJson: (a: number) => [number, number];
    readonly wasmengine_listFlags: (a: number) => [number, number];
    readonly wasmengine_listStats: (a: number) => [number, number, number];
    readonly wasmengine_listStoryTags: (a: number) => [number, number];
    readonly wasmengine_listTags: (a: number) => [number, number];
    readonly wasmengine_makeChoice: (a: number, b: number) => [number, number, number];
    readonly wasmengine_new: (a: number, b: number) => [number, number, number];
    readonly wasmengine_setStateJson: (a: number, b: number, c: number) => [number, number];
    readonly wasmstorycatalog_createEngine: (a: number, b: number) => number;
    readonly wasmstorycatalog_createEngineByName: (a: number, b: number, c: number) => number;
    readonly wasmstorycatalog_isEmpty: (a: number) => number;
    readonly wasmstorycatalog_len: (a: number) => number;
    readonly wasmstorycatalog_listStories: (a: number) => [number, number, number];
    readonly wasmstorycatalog_new: () => number;
    readonly wasmstorycatalog_register: (a: number, b: number, c: number) => [number, number];
    readonly wasmstorycatalog_storiesWithAllTags: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmstorycatalog_storiesWithAnyTags: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmstorycatalog_storiesWithTag: (a: number, b: number, c: number) => [number, number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_drop_slice: (a: number, b: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
