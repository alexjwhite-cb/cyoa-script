/**
 * cyoa.h — C-ABI header for the CYOA story engine.
 *
 * Declare this header or include the generated static/shared library
 * (`libcyoa_native.a` / `cyoa_native.dll` / `libcyoa_native.so`) from any
 * language with C FFI: C, C++, C#, Rust, Zig, etc.
 *
 * Build the library:
 *   cargo build -p cyoa-native
 *
 * Memory model:
 *   - Functions returning `const char *`  return engine-owned buffers that
 *     remain valid until the next API call on the same handle. Copy the
 *     string immediately if you need it to persist.
 *   - Functions returning `char *`        return heap-allocated strings that
 *     the caller MUST free with `cyoa_free_string()`.
 *   - `CyoaEngine` and `CyoaCatalog`     are opaque handles. Free them with
 *     `cyoa_destroy` and `cyoa_catalog_destroy` respectively.
 */

#ifndef CYOA_H
#define CYOA_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* ── Opaque handles ──────────────────────────────────────────────────────── */

typedef struct CyoaEngine CyoaEngine;
typedef struct CyoaCatalog CyoaCatalog;

/* ── Engine lifecycle ────────────────────────────────────────────────────── */

/**
 * Create an engine from compiled bytecode (`.cyoa.bc`).
 *
 *   bytecode  pointer to the raw bytes of a compiled story
 *   len       number of bytes at `bytecode`
 *
 * Returns an opaque handle, or NULL on failure.
 */
CyoaEngine *cyoa_create(const uint8_t *bytecode, size_t len);

/**
 * Destroy an engine handle created by `cyoa_create`.
 * Passing NULL is a no-op.
 */
void cyoa_destroy(CyoaEngine *engine);

/* ── Event queries (const char* = engine-owned, valid until next call) ─────── */

/** Current event ID (name). */
const char *cyoa_current_event_id(CyoaEngine *engine);

/** Current event text — paragraphs joined by '\n'. */
const char *cyoa_current_event_text(CyoaEngine *engine);

/** Number of currently available choices. */
int cyoa_current_choice_count(CyoaEngine *engine);

/** Text of the choice at `index`, or NULL if out of bounds. */
const char *cyoa_choice_text(CyoaEngine *engine, int index);

/* ── Make a choice ────────────────────────────────────────────────────────── */

/**
 * Apply the player's choice at `index`.
 * After this call the engine has advanced to the next event.
 * Use `cyoa_last_effect_text` to retrieve effect text from this choice.
 */
void cyoa_make_choice(CyoaEngine *engine, int index);

/**
 * Effect text from the most recent `cyoa_make_choice` call.
 * Paragraphs joined by '\n'. Engine-owned, valid until next call.
 */
const char *cyoa_last_effect_text(CyoaEngine *engine);

/* ── Choice history ──────────────────────────────────────────────────────── */

/** Number of entries in the choice history. */
int cyoa_history_length(CyoaEngine *engine);

/**
 * History entry at `index` as a JSON string:
 *   {"eventId":"...","choiceIndex":N,"choiceText":"..."}
 * Returns NULL if `index` is out of bounds.
 * Engine-owned, valid until next call.
 */
const char *cyoa_history_entry(CyoaEngine *engine, int index);

/* ── State management ────────────────────────────────────────────────────── */

/**
 * Serialize the full player state as a JSON string.
 * Returns a heap-allocated string the caller MUST free with `cyoa_free_string`.
 */
char *cyoa_get_state_json(CyoaEngine *engine);

/** Restore player state from a JSON string produced by `cyoa_get_state_json`. */
void cyoa_set_state_json(CyoaEngine *engine, const char *json);

/**
 * Free a string returned by `cyoa_get_state_json`, `cyoa_list_stats_json`,
 * `cyoa_list_story_tags_json`, `cyoa_list_tags_json`,
 * `cyoa_list_flags_json`, `cyoa_available_events_json`, or any
 * `cyoa_catalog_*` function that returns a `char *`.
 *
 * Passing NULL is a no-op.
 */
void cyoa_free_string(char *s);

/* ── Queries ───────────────────────────────────────────────────────────────── */

/**
 * All event IDs in the story as a JSON array string: ["id1","id2",...].
 * Heap-allocated, caller must free with `cyoa_free_string`.
 */
char *cyoa_available_events_json(CyoaEngine *engine);

/**
 * Check whether an event by ID is reachable from the current story graph.
 * Returns 1 (true) or 0 (false).
 */
int cyoa_can_access_event(CyoaEngine *engine, const char *id);

/* ── Stats / tags / flags ─────────────────────────────────────────────────── */

/**
 * All stats as a JSON string: {"statName": value, ...}.
 * Heap-allocated, caller must free with `cyoa_free_string`.
 */
char *cyoa_list_stats_json(CyoaEngine *engine);

/**
 * Story-level tags (static metadata) as a JSON array: ["tag1","tag2"].
 * Heap-allocated, caller must free with `cyoa_free_string`.
 */
char *cyoa_list_story_tags_json(CyoaEngine *engine);

/**
 * Runtime tags currently applied during play as a JSON array.
 * Heap-allocated, caller must free with `cyoa_free_string`.
 */
char *cyoa_list_tags_json(CyoaEngine *engine);

/**
 * Runtime flags currently set as a JSON array.
 * Heap-allocated, caller must free with `cyoa_free_string`.
 */
char *cyoa_list_flags_json(CyoaEngine *engine);

/**
 * Get a stat value by name. Returns 0 if the stat doesn't exist.
 */
int cyoa_get_stat(CyoaEngine *engine, const char *name);

/* ── Story catalog (multi-story, tag filtering) ───────────────────────────── */

/** Create an empty story catalog. */
CyoaCatalog *cyoa_catalog_create(void);

/** Destroy a catalog created by `cyoa_catalog_create`. NULL is a no-op. */
void cyoa_catalog_destroy(CyoaCatalog *catalog);

/**
 * Register a compiled story from bytecode bytes.
 * Returns 1 on success, 0 on failure.
 */
int cyoa_catalog_register(CyoaCatalog *catalog, const uint8_t *bytecode, size_t len);

/** Number of registered stories in the catalog. */
int cyoa_catalog_story_count(const CyoaCatalog *catalog);

/**
 * List all registered stories as a JSON array:
 *   [{"name":"StoryName","tags":["tag1","tag2"]}, ...]
 * Heap-allocated, caller must free with `cyoa_free_string`.
 */
char *cyoa_catalog_list_stories_json(const CyoaCatalog *catalog);

/**
 * Find all stories that have a specific tag.
 * Heap-allocated JSON array, caller must free with `cyoa_free_string`.
 */
char *cyoa_catalog_stories_with_tag_json(const CyoaCatalog *catalog, const char *tag);

/**
 * Find all stories that have ALL of the specified tags.
 * `tags_json` is a JSON array string: ["fantasy","exploration"].
 * Heap-allocated JSON array, caller must free with `cyoa_free_string`.
 */
char *cyoa_catalog_stories_with_all_tags_json(const CyoaCatalog *catalog, const char *tags_json);

/**
 * Find all stories that have ANY of the specified tags.
 * `tags_json` is a JSON array string: ["fantasy","combat"].
 * Heap-allocated JSON array, caller must free with `cyoa_free_string`.
 */
char *cyoa_catalog_stories_with_any_tags_json(const CyoaCatalog *catalog, const char *tags_json);

/**
 * Create an engine for the story at the given catalog index.
 * Returns NULL if the index is out of bounds.
 */
CyoaEngine *cyoa_catalog_create_engine(CyoaCatalog *catalog, int index);

/**
 * Find a story by name and create an engine for it.
 * Returns NULL if no story with that name is registered.
 */
CyoaEngine *cyoa_catalog_create_engine_by_name(CyoaCatalog *catalog, const char *name);

#ifdef __cplusplus
}
#endif

#endif /* CYOA_H */
