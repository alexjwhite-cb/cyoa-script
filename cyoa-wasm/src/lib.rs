//! WASM bindings for the CYOA engine.
//!
//! This crate compiles to a `.wasm` module that can be loaded from
//! JavaScript, Unity, and Godot. The API is handle-based: each story
//! is an independent Engine instance.
//!
//! ## Zero-copy text access
//!
//! Two tiers of API are provided:
//!
//! - **High-level** (`getCurrentEvent`, `listStats`, etc.): returns JS objects
//!   via `serde_wasm_bindgen`. Convenient and ergonomic.
//! - **Zero-copy** (`currentEventTextBytes`, `currentChoicesBytes`): returns
//!   `Uint8Array` views directly into WASM linear memory. The JS caller decodes
//!   with `TextDecoder`. These views are valid until the next method call on the
//!   same engine.
//!
//! ## Multi-story support
//!
//! [`WasmStoryCatalog`] registers multiple compiled stories and filters by
//! tags. Each story becomes an independent [`WasmEngine`].

use wasm_bindgen::prelude::*;

use cyoa_bytecode::Bytecode;
use cyoa_runtime::{Engine, StoryCatalog};

// ────────────────────────────────────────────────────────────────────────────
// Serializable return types for the WASM API
//
// NOTE: serde_wasm_bindgen 0.6.x does not correctly serialize
// serde_json::Value to JsValue (produces empty objects). Using plain
// serializable Rust structs works correctly.
// ────────────────────────────────────────────────────────────────────────────

/// A serializable story metadata entry returned to JS as a plain object.
/// Fields: `name`, `tags`
#[derive(serde::Serialize)]
struct StoryInfo {
    name: String,
    tags: Vec<String>,
}

/// Serializable current event info.
/// Fields: `id`, `text`, `choices`
#[derive(serde::Serialize)]
struct EventInfo {
    id: String,
    text: Vec<String>,
    choices: Vec<String>,
}

/// Serializable choice result.
/// Fields: `effectText`, `gameStateChanged`
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ChoiceResult {
    effect_text: Vec<String>,
    game_state_changed: bool,
}

/// Serializable choice history entry.
/// Fields: `eventId`, `choiceIndex`, `choiceText`
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ChoiceHistoryEntry {
    event_id: String,
    choice_index: i32,
    choice_text: String,
}

// ────────────────────────────────────────────────────────────────────────────
// WasmStoryCatalog
// ────────────────────────────────────────────────────────────────────────────

/// A registry of compiled stories with tag-based filtering.
///
/// Game engines (especially random event systems) use this to discover
/// and filter stories by tags before creating a `WasmEngine` to play them.
#[wasm_bindgen]
pub struct WasmStoryCatalog {
    inner: StoryCatalog,
}

#[wasm_bindgen]
impl WasmStoryCatalog {
    /// Create an empty catalog.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: StoryCatalog::new(),
        }
    }

    /// Register a compiled story from bytecode bytes.
    ///
    /// The bytecode must have been produced by `cyoa compile`.
    /// Story name and tags are extracted from the bytecode at registration.
    pub fn register(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let bc = Bytecode::from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&format!("failed to decode bytecode: {}", e)))?;
        self.inner.register(bc);
        Ok(())
    }

    /// Number of registered stories.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the catalog is empty.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// List all registered stories as an array of `{ name, tags }` objects.
    ///
    /// ```json
    /// [{"name": "ForestAdventure", "tags": ["fantasy", "exploration"]}, ...]
    /// ```
    #[wasm_bindgen(js_name = listStories)]
    pub fn list_stories(&self) -> Result<JsValue, JsValue> {
        let stories: Vec<StoryInfo> = self
            .inner
            .list_stories()
            .iter()
            .map(|m| StoryInfo {
                name: m.name.clone(),
                tags: m.tags.clone(),
            })
            .collect();
        serde_wasm_bindgen::to_value(&stories).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Find all stories that have a specific tag.
    #[wasm_bindgen(js_name = storiesWithTag)]
    pub fn stories_with_tag(&self, tag: &str) -> Result<JsValue, JsValue> {
        let stories: Vec<StoryInfo> = self
            .inner
            .stories_with_tag(tag)
            .iter()
            .map(|m| StoryInfo {
                name: m.name.clone(),
                tags: m.tags.clone(),
            })
            .collect();
        serde_wasm_bindgen::to_value(&stories).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Find all stories that have ALL of the specified tags.
    ///
    /// `tagsJson` is a JSON array of tag strings, e.g. `"[\"fantasy\", \"exploration\"]"`.
    #[wasm_bindgen(js_name = storiesWithAllTags)]
    pub fn stories_with_all_tags(&self, tags_json: &str) -> Result<JsValue, JsValue> {
        let tags: Vec<String> = serde_json::from_str(tags_json)
            .map_err(|e| JsValue::from_str(&format!("invalid JSON for tags: {}", e)))?;
        let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
        let stories: Vec<StoryInfo> = self
            .inner
            .stories_with_all_tags(&tag_refs)
            .iter()
            .map(|m| StoryInfo {
                name: m.name.clone(),
                tags: m.tags.clone(),
            })
            .collect();
        serde_wasm_bindgen::to_value(&stories).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Find all stories that have ANY of the specified tags.
    ///
    /// `tagsJson` is a JSON array of tag strings, e.g. `"[\"fantasy\", \"combat\"]"`.
    #[wasm_bindgen(js_name = storiesWithAnyTags)]
    pub fn stories_with_any_tags(&self, tags_json: &str) -> Result<JsValue, JsValue> {
        let tags: Vec<String> = serde_json::from_str(tags_json)
            .map_err(|e| JsValue::from_str(&format!("invalid JSON for tags: {}", e)))?;
        let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
        let stories: Vec<StoryInfo> = self
            .inner
            .stories_with_any_tags(&tag_refs)
            .iter()
            .map(|m| StoryInfo {
                name: m.name.clone(),
                tags: m.tags.clone(),
            })
            .collect();
        serde_wasm_bindgen::to_value(&stories).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Create an `Engine` for the story at the given index.
    /// Returns `null` if the index is out of bounds.
    #[wasm_bindgen(js_name = createEngine)]
    pub fn create_engine(&self, index: usize) -> Option<WasmEngine> {
        self.inner.create_engine(index).map(WasmEngine::from_engine)
    }

    /// Find a story by name and create an `Engine` for it.
    /// Returns `null` if no story with that name is registered.
    #[wasm_bindgen(js_name = createEngineByName)]
    pub fn create_engine_by_name(&self, name: &str) -> Option<WasmEngine> {
        self.inner
            .create_engine_by_name(name)
            .map(WasmEngine::from_engine)
    }
}

impl Default for WasmStoryCatalog {
    fn default() -> Self {
        Self::new()
    }
}

// ────────────────────────────────────────────────────────────────────────────
// WasmEngine
// ────────────────────────────────────────────────────────────────────────────

/// A CYOA story engine instance.
///
/// Each engine is independent — state is never shared between instances.
/// Create one per story (main quest, side quests, etc.).
#[wasm_bindgen]
pub struct WasmEngine {
    engine: Engine,
    /// Scratch pool for zero-copy text access — keeps rendered strings alive
    /// in WASM linear memory so JS can read them via Uint8Array views.
    text_pool: Vec<String>,
}

impl WasmEngine {
    fn from_engine(engine: Engine) -> Self {
        Self {
            engine,
            text_pool: Vec::new(),
        }
    }
}

#[wasm_bindgen]
impl WasmEngine {
    /// Create an engine directly from compiled bytecode bytes.
    ///
    /// Alternatively, use [`WasmStoryCatalog::createEngine`] to create engines
    /// from a catalog of tagged stories.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<WasmEngine, JsValue> {
        let bc = Bytecode::from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&format!("failed to decode bytecode: {}", e)))?;
        Ok(Self::from_engine(Engine::new(bc)))
    }

    // ── High-level API ──────────────────────────────────────────────────

    /// Get the current event's internal ID (name).
    #[wasm_bindgen(js_name = currentEventId)]
    pub fn current_event_id(&self) -> String {
        self.engine.current_event_id()
    }

    /// Get the current event as a JS object:
    /// ```json
    /// { "id": "old_ruins", "text": ["...", "..."], "choices": ["...", "..."] }
    /// ```
    #[wasm_bindgen(js_name = getCurrentEvent)]
    pub fn get_current_event(&self) -> Result<JsValue, JsValue> {
        let result = EventInfo {
            id: self.engine.current_event_id(),
            text: self.engine.current_event_text(),
            choices: self.engine.current_choices(),
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Make a choice at the given index.
    /// Returns `{ effectText: string[], gameStateChanged: boolean }`.
    ///
    /// `choiceIndex` is 0-based, referencing only choices visible to the player
    /// (prerequisites are already filtered).
    #[wasm_bindgen(js_name = makeChoice)]
    pub fn make_choice(&mut self, choice_index: i32) -> Result<JsValue, JsValue> {
        let effects = self.engine.make_choice(choice_index);
        let result = ChoiceResult {
            effect_text: effects,
            game_state_changed: true,
        };
        serde_wasm_bindgen::to_value(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get the choice history as an array of `{ eventId, choiceIndex, choiceText }`.
    #[wasm_bindgen(js_name = getHistory)]
    pub fn get_history(&self) -> Result<JsValue, JsValue> {
        let history: Vec<ChoiceHistoryEntry> = self
            .engine
            .history()
            .iter()
            .map(|h| ChoiceHistoryEntry {
                event_id: h.event_id.clone(),
                choice_index: h.choice_index,
                choice_text: h.choice_text.clone(),
            })
            .collect();
        serde_wasm_bindgen::to_value(&history).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get the full player state as a JSON string.
    /// Use this for save files. The JSON contains `stats`, `flags`, and `tags`.
    ///
    /// The returned string is valid until the next method call on this engine.
    #[wasm_bindgen(js_name = getStateJson)]
    pub fn get_state_json(&self) -> String {
        self.engine.get_state_json()
    }

    /// Restore state from a JSON string produced by [`getStateJson`](Self::get_state_json).
    #[wasm_bindgen(js_name = setStateJson)]
    pub fn set_state_json(&mut self, json: &str) -> Result<(), JsValue> {
        self.engine
            .set_state_json(json)
            .map_err(|e| JsValue::from_str(&format!("failed to parse state JSON: {}", e)))
    }

    /// Get stats as a JS object mapping stat names to current values.
    #[wasm_bindgen(js_name = listStats)]
    pub fn list_stats(&self) -> Result<JsValue, JsValue> {
        // Use js_sys::JSON::parse to avoid serde_json::Value serialization issues
        // with serde_wasm_bindgen (which drops object properties).
        let json_str = self.engine.stats_json();
        js_sys::JSON::parse(&json_str)
            .map_err(|e| JsValue::from_str(&format!("failed to parse stats JSON: {:?}", e)))
    }

    /// Get a stat value by name. Returns 0 if the stat doesn't exist.
    #[wasm_bindgen(js_name = getStat)]
    pub fn get_stat(&self, name: &str) -> i64 {
        self.engine.get_stat(name)
    }

    /// List story-level tags declared in the story (static metadata).
    /// These are distinct from runtime tags (see [`listTags`](Self::listTags)).
    #[wasm_bindgen(js_name = listStoryTags)]
    pub fn list_story_tags(&self) -> Vec<String> {
        self.engine.list_story_tags()
    }

    /// List runtime tags currently applied during play.
    #[wasm_bindgen(js_name = listTags)]
    pub fn list_tags(&self) -> Vec<String> {
        self.engine.list_tags()
    }

    /// List all runtime flags currently set.
    #[wasm_bindgen(js_name = listFlags)]
    pub fn list_flags(&self) -> Vec<String> {
        self.engine.list_flags()
    }

    /// Check if an event by ID is reachable from the current story graph.
    #[wasm_bindgen(js_name = canAccessEvent)]
    pub fn can_access_event(&self, id: &str) -> bool {
        self.engine.can_access_event(id)
    }

    /// List all event IDs in the story (useful for quest logs).
    #[wasm_bindgen(js_name = availableEvents)]
    pub fn available_events(&self) -> Vec<String> {
        self.engine.available_events()
    }

    /// Returns true if the story has ended (a terminal choice was made).
    /// A terminal choice is one that has no `next` event specified.
    #[wasm_bindgen(js_name = isStoryComplete)]
    pub fn is_story_complete(&self) -> bool {
        self.engine.is_story_complete()
    }

    // ── Zero-copy text API ──────────────────────────────────────────────

    /// Zero-copy: returns a `Uint8Array` view of all current event text
    /// paragraphs, newline-separated.
    ///
    /// The bytes live in WASM linear memory and are valid until the next
    /// method call on this engine. Use `TextDecoder` to decode:
    ///
    /// ```js
    /// const bytes = engine.currentEventTextBytes();
    /// const text = new TextDecoder().decode(bytes);
    /// ```
    #[wasm_bindgen(js_name = currentEventTextBytes)]
    pub fn current_event_text_bytes(&mut self) -> js_sys::Uint8Array {
        let texts = self.engine.current_event_text();
        self.text_pool = texts;
        let combined = self.text_pool.join("\n");
        self.text_pool.clear();
        self.text_pool.push(combined);
        // SAFETY: text_pool[0] lives in WASM linear memory and stays alive
        // until the next method call on this engine (documented contract).
        unsafe { js_sys::Uint8Array::view(self.text_pool[0].as_bytes()) }
    }

    /// Zero-copy: returns a `Uint8Array` view of all current choice texts,
    /// newline-separated.
    ///
    /// The bytes live in WASM linear memory and are valid until the next
    /// method call on this engine.
    #[wasm_bindgen(js_name = currentChoicesBytes)]
    pub fn current_choices_bytes(&mut self) -> js_sys::Uint8Array {
        let choices = self.engine.current_choices();
        self.text_pool = choices;
        let combined = self.text_pool.join("\n");
        self.text_pool.clear();
        self.text_pool.push(combined);
        // SAFETY: text_pool[0] lives in WASM linear memory and stays alive
        // until the next method call on this engine (documented contract).
        unsafe { js_sys::Uint8Array::view(self.text_pool[0].as_bytes()) }
    }
}

// Re-export for convenience
pub use cyoa_runtime::{PlayerState, StoryMetadata};
