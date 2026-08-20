//! C-ABI bindings for the CYOA engine.
//!
//! Exports `extern "C"` functions for native desktop/mobile integration.
//! Each story is a separate engine instance identified by an opaque handle,
//! and a catalog handle manages multiple tagged stories.
//!
//! ## Memory rules
//!
//! - **`*const char` returns** point into engine-owned memory, valid until
//!   the next API call on the same engine/catalog handle. The caller must
//!   copy the string if it needs to persist.
//! - **`*mut char` returns** (from `*_json` and `*_state` functions) are
//!   heap-allocated. The caller must free them with [`cyoa_free_string`].
//! - Handles created by `cyoa_create` / `cyoa_catalog_create` must be freed
//!   with `cyoa_destroy` / `cyoa_catalog_destroy`.

#![allow(dead_code)]
#![allow(non_snake_case)]
// FFI functions are `extern "C"` — callers from C/C# are responsible for
// pointer validity. This lint does not apply to FFI boundaries.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uchar};
use std::ptr;

use cyoa_bytecode::Bytecode;
use cyoa_runtime::{Engine, StoryCatalog, StoryMetadata};

// ──────────────────────────────────────────────────────────────────────────
// Engine handle
// ──────────────────────────────────────────────────────────────────────────

/// Opaque handle to an engine instance — one per story.
pub struct CyoaEngine {
    engine: Engine,
    /// Single-slot string buffer for engine-owned returns.
    /// Overwritten on each call to a function returning `*const c_char`.
    str_buf: CString,
    /// Effect text fragments from the most recent `make_choice` call.
    last_effects: Vec<String>,
}

impl CyoaEngine {
    /// Store a Rust string into the engine's scratch buffer and return a
    /// `*const c_char` view into it (valid until the next call that touches
    /// `str_buf`).
    fn store_str(&mut self, s: &str) -> *const c_char {
        self.str_buf = CString::new(s).unwrap_or_else(|_| CString::new("").unwrap());
        self.str_buf.as_ptr()
    }
}

// ── Lifecycle ─────────────────────────────────────────────────────────────

/// Create an engine from compiled bytecode bytes.
///
/// `bytecode` must point to `len` bytes of a valid `.cyoa.bc` file.
/// Returns a handle, or NULL on failure.
#[no_mangle]
pub extern "C" fn cyoa_create(bytecode: *const c_uchar, len: usize) -> *mut CyoaEngine {
    if bytecode.is_null() {
        return ptr::null_mut();
    }

    let bytes = unsafe { std::slice::from_raw_parts(bytecode, len) };
    match Bytecode::from_bytes(bytes) {
        Ok(bc) => {
            let engine = CyoaEngine {
                engine: Engine::new(bc),
                str_buf: CString::new("").unwrap(),
                last_effects: Vec::new(),
            };
            Box::into_raw(Box::new(engine))
        }
        Err(_) => ptr::null_mut(),
    }
}

/// Destroy an engine created by [`cyoa_create`].
/// Passing NULL is a no-op.
#[no_mangle]
pub extern "C" fn cyoa_destroy(engine: *mut CyoaEngine) {
    if !engine.is_null() {
        unsafe {
            let _ = Box::from_raw(engine);
        }
    }
}

// ── Event queries ──────────────────────────────────────────────────────────
// Returned `*const char` values are engine-owned, valid until the next call.

/// Get the current event's internal ID (name).
#[no_mangle]
pub extern "C" fn cyoa_current_event_id(engine: *mut CyoaEngine) -> *const c_char {
    let eng = unsafe { &mut *engine };
    let id = eng.engine.current_event_id();
    eng.store_str(&id)
}

/// Get the current event's text as a single newline-joined string.
/// Multiple paragraphs are joined with `\n`.
#[no_mangle]
pub extern "C" fn cyoa_current_event_text(engine: *mut CyoaEngine) -> *const c_char {
    let eng = unsafe { &mut *engine };
    let paragraphs = eng.engine.current_event_text();
    let combined = paragraphs.join("\n");
    eng.store_str(&combined)
}

/// Number of currently available choices.
#[no_mangle]
pub extern "C" fn cyoa_current_choice_count(engine: *mut CyoaEngine) -> c_int {
    let eng = unsafe { &*engine };
    eng.engine.current_choices().len() as c_int
}

/// Text of the choice at `index`, or NULL if out of bounds.
#[no_mangle]
pub extern "C" fn cyoa_choice_text(engine: *mut CyoaEngine, index: c_int) -> *const c_char {
    let eng = unsafe { &mut *engine };
    let choices = eng.engine.current_choices();
    if index >= 0 && (index as usize) < choices.len() {
        eng.store_str(&choices[index as usize])
    } else {
        ptr::null()
    }
}

// ── Make a choice ──────────────────────────────────────────────────────────

/// Apply the player's choice at `index`.
/// After this call, the engine has advanced to the next event.
/// Use [`cyoa_last_effect_text`] to retrieve effect text from this choice.
#[no_mangle]
pub extern "C" fn cyoa_make_choice(engine: *mut CyoaEngine, index: c_int) {
    let eng = unsafe { &mut *engine };
    eng.last_effects = eng.engine.make_choice(index);
}

/// Get the effect text from the most recent [`cyoa_make_choice`] call.
/// Multiple effect texts are joined with `\n`.
/// Engine-owned, valid until the next call.
#[no_mangle]
pub extern "C" fn cyoa_last_effect_text(engine: *mut CyoaEngine) -> *const c_char {
    let eng = unsafe { &mut *engine };
    let combined = eng.last_effects.join("\n");
    eng.store_str(&combined)
}

// ── Choice history ────────────────────────────────────────────────────────

/// Number of entries in the choice history.
#[no_mangle]
pub extern "C" fn cyoa_history_length(engine: *mut CyoaEngine) -> c_int {
    let eng = unsafe { &*engine };
    eng.engine.history().len() as c_int
}

/// Get a history entry as a JSON string.
/// Format: `{"eventId":"...","choiceIndex":N,"choiceText":"..."}`
/// Returns NULL if `index` is out of bounds.
/// Engine-owned, valid until the next call.
#[no_mangle]
pub extern "C" fn cyoa_history_entry(engine: *mut CyoaEngine, index: c_int) -> *const c_char {
    let eng = unsafe { &mut *engine };
    let history = eng.engine.history();
    if index >= 0 && (index as usize) < history.len() {
        let entry = &history[index as usize];
        let json = serde_json::json!({
            "eventId": entry.event_id,
            "choiceIndex": entry.choice_index,
            "choiceText": entry.choice_text,
        });
        let json_str = serde_json::to_string(&json).unwrap();
        eng.store_str(&json_str)
    } else {
        ptr::null()
    }
}

// ── State management ──────────────────────────────────────────────────────

/// Serialize the full player state (stats, flags, tags, history) as a JSON
/// string.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_get_state_json(engine: *mut CyoaEngine) -> *mut c_char {
    let eng = unsafe { &*engine };
    let json = eng.engine.get_state_json();
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// Restore player state from a JSON string produced by [`cyoa_get_state_json`].
#[no_mangle]
pub extern "C" fn cyoa_set_state_json(engine: *mut CyoaEngine, json: *const c_char) {
    if json.is_null() {
        return;
    }
    let eng = unsafe { &mut *engine };
    let json_str = unsafe { CStr::from_ptr(json) }.to_str().unwrap_or("");
    let _ = eng.engine.set_state_json(json_str);
}

/// Free a string returned by [`cyoa_get_state_json`], [`cyoa_list_stats_json`],
/// [`cyoa_list_story_tags_json`], [`cyoa_list_tags_json`],
/// [`cyoa_list_flags_json`], [`cyoa_available_events_json`], or any of the
/// `cyoa_catalog_*` functions.
///
/// Passing NULL is a no-op.
#[no_mangle]
pub extern "C" fn cyoa_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

// ── Queries ────────────────────────────────────────────────────────────────

/// List all event IDs in the story as a JSON array string: `["id1","id2",...]`.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_available_events_json(engine: *mut CyoaEngine) -> *mut c_char {
    let eng = unsafe { &*engine };
    let events = eng.engine.available_events();
    let json = serde_json::to_string(&events).unwrap_or_else(|_| "[]".to_string());
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// Check whether an event by ID is reachable from the current story graph.
/// Returns 1 (true) or 0 (false).
#[no_mangle]
pub extern "C" fn cyoa_can_access_event(engine: *mut CyoaEngine, id: *const c_char) -> c_int {
    if id.is_null() {
        return 0;
    }
    let eng = unsafe { &*engine };
    let id_str = unsafe { CStr::from_ptr(id) }.to_str().unwrap_or("");
    if eng.engine.can_access_event(id_str) {
        1
    } else {
        0
    }
}

// ── Stats / tags / flags ───────────────────────────────────────────────────

/// Get all stats as a JSON string: `{"statName": value, ...}`.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_list_stats_json(engine: *mut CyoaEngine) -> *mut c_char {
    let eng = unsafe { &*engine };
    let json = eng.engine.stats_json();
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// List all story-level tags (static metadata declared in the story) as a
/// JSON array string: `["fantasy","exploration"]`.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_list_story_tags_json(engine: *mut CyoaEngine) -> *mut c_char {
    let eng = unsafe { &*engine };
    let tags = eng.engine.list_story_tags();
    let json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// List runtime tags currently applied during play as a JSON array string.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_list_tags_json(engine: *mut CyoaEngine) -> *mut c_char {
    let eng = unsafe { &*engine };
    let tags = eng.engine.list_tags();
    let json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string());
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// List all runtime flags currently set as a JSON array string.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_list_flags_json(engine: *mut CyoaEngine) -> *mut c_char {
    let eng = unsafe { &*engine };
    let flags = eng.engine.list_flags();
    let json = serde_json::to_string(&flags).unwrap_or_else(|_| "[]".to_string());
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// Get a stat value by name. Returns 0 if the stat doesn't exist.
#[no_mangle]
pub extern "C" fn cyoa_get_stat(engine: *mut CyoaEngine, name: *const c_char) -> c_int {
    if name.is_null() {
        return 0;
    }
    let eng = unsafe { &*engine };
    let name_str = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
    eng.engine.get_stat(name_str) as c_int
}

// ──────────────────────────────────────────────────────────────────────────
// Story catalog handle (multi-story tag filtering)
// ──────────────────────────────────────────────────────────────────────────

/// Opaque handle to a story catalog — manages multiple registered stories
/// and provides tag-based filtering (mirrors `WasmStoryCatalog` in cyoa-wasm).
pub struct CyoaCatalog {
    catalog: StoryCatalog,
}

/// Create an empty story catalog.
#[no_mangle]
pub extern "C" fn cyoa_catalog_create() -> *mut CyoaCatalog {
    let catalog = CyoaCatalog {
        catalog: StoryCatalog::new(),
    };
    Box::into_raw(Box::new(catalog))
}

/// Destroy a catalog created by [`cyoa_catalog_create`].
/// Passing NULL is a no-op.
#[no_mangle]
pub extern "C" fn cyoa_catalog_destroy(catalog: *mut CyoaCatalog) {
    if !catalog.is_null() {
        unsafe {
            let _ = Box::from_raw(catalog);
        }
    }
}

/// Register a compiled story from bytecode bytes.
///
/// `bytecode` must point to `len` bytes of a valid `.cyoa.bc` file.
/// Returns 1 on success, 0 on failure.
#[no_mangle]
pub extern "C" fn cyoa_catalog_register(
    catalog: *mut CyoaCatalog,
    bytecode: *const c_uchar,
    len: usize,
) -> c_int {
    if catalog.is_null() || bytecode.is_null() {
        return 0;
    }

    let cat = unsafe { &mut *catalog };
    let bytes = unsafe { std::slice::from_raw_parts(bytecode, len) };
    match Bytecode::from_bytes(bytes) {
        Ok(bc) => {
            cat.catalog.register(bc);
            1
        }
        Err(_) => 0,
    }
}

/// Number of registered stories in the catalog.
#[no_mangle]
pub extern "C" fn cyoa_catalog_story_count(catalog: *const CyoaCatalog) -> c_int {
    if catalog.is_null() {
        return 0;
    }
    let cat = unsafe { &*catalog };
    cat.catalog.len() as c_int
}

/// Serialize story metadata as a JSON array string.
/// Each entry: `{"name":"StoryName","tags":["tag1","tag2"]}`.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
fn stories_to_json(stories: &[&StoryMetadata]) -> *mut c_char {
    let arr: Vec<serde_json::Value> = stories
        .iter()
        .map(|m| serde_json::json!({ "name": m.name, "tags": &m.tags }))
        .collect();
    let json = serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string());
    CString::new(json)
        .map(|c| c.into_raw())
        .unwrap_or(ptr::null_mut())
}

/// List all registered stories as a JSON array string.
///
/// Returns a **heap-allocated** string that the caller **must** free with
/// [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_catalog_list_stories_json(catalog: *const CyoaCatalog) -> *mut c_char {
    if catalog.is_null() {
        return ptr::null_mut();
    }
    let cat = unsafe { &*catalog };
    let stories = cat.catalog.list_stories();
    let refs: Vec<&StoryMetadata> = stories.iter().collect();
    stories_to_json(&refs)
}

/// Find all stories that have a specific tag.
/// `tag` is a NUL-terminated C string.
///
/// Returns a **heap-allocated** JSON array string that the caller **must** free
/// with [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_catalog_stories_with_tag_json(
    catalog: *const CyoaCatalog,
    tag: *const c_char,
) -> *mut c_char {
    if catalog.is_null() || tag.is_null() {
        return ptr::null_mut();
    }
    let cat = unsafe { &*catalog };
    let tag_str = unsafe { CStr::from_ptr(tag) }.to_str().unwrap_or("");
    let stories = cat.catalog.stories_with_tag(tag_str);
    stories_to_json(&stories)
}

/// Find all stories that have ALL of the specified tags.
/// `tagsJson` is a JSON array of tag strings, e.g. `["fantasy","exploration"]`.
///
/// Returns a **heap-allocated** JSON array string that the caller **must** free
/// with [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_catalog_stories_with_all_tags_json(
    catalog: *const CyoaCatalog,
    tags_json: *const c_char,
) -> *mut c_char {
    if catalog.is_null() || tags_json.is_null() {
        return ptr::null_mut();
    }
    let cat = unsafe { &*catalog };
    let tags_str = unsafe { CStr::from_ptr(tags_json) }.to_str().unwrap_or("");
    let tags: Vec<String> = serde_json::from_str(tags_str).unwrap_or_default();
    let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
    let stories = cat.catalog.stories_with_all_tags(&tag_refs);
    stories_to_json(&stories)
}

/// Find all stories that have ANY of the specified tags.
/// `tagsJson` is a JSON array of tag strings, e.g. `["fantasy","combat"]`.
///
/// Returns a **heap-allocated** JSON array string that the caller **must** free
/// with [`cyoa_free_string`].
#[no_mangle]
pub extern "C" fn cyoa_catalog_stories_with_any_tags_json(
    catalog: *const CyoaCatalog,
    tags_json: *const c_char,
) -> *mut c_char {
    if catalog.is_null() || tags_json.is_null() {
        return ptr::null_mut();
    }
    let cat = unsafe { &*catalog };
    let tags_str = unsafe { CStr::from_ptr(tags_json) }.to_str().unwrap_or("");
    let tags: Vec<String> = serde_json::from_str(tags_str).unwrap_or_default();
    let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
    let stories = cat.catalog.stories_with_any_tags(&tag_refs);
    stories_to_json(&stories)
}

/// Create an engine for the story at the given catalog index.
/// Returns NULL if the index is out of bounds.
#[no_mangle]
pub extern "C" fn cyoa_catalog_create_engine(
    catalog: *mut CyoaCatalog,
    index: c_int,
) -> *mut CyoaEngine {
    if catalog.is_null() {
        return ptr::null_mut();
    }
    let cat = unsafe { &*catalog };
    cat.catalog
        .create_engine(index as usize)
        .map(|engine| {
            Box::into_raw(Box::new(CyoaEngine {
                engine,
                str_buf: CString::new("").unwrap(),
                last_effects: Vec::new(),
            }))
        })
        .unwrap_or(ptr::null_mut())
}

/// Find a story by name and create an engine for it.
/// Returns NULL if no story with that name is registered.
#[no_mangle]
pub extern "C" fn cyoa_catalog_create_engine_by_name(
    catalog: *mut CyoaCatalog,
    name: *const c_char,
) -> *mut CyoaEngine {
    if catalog.is_null() || name.is_null() {
        return ptr::null_mut();
    }
    let cat = unsafe { &*catalog };
    let name_str = unsafe { CStr::from_ptr(name) }.to_str().unwrap_or("");
    cat.catalog
        .create_engine_by_name(name_str)
        .map(|engine| {
            Box::into_raw(Box::new(CyoaEngine {
                engine,
                str_buf: CString::new("").unwrap(),
                last_effects: Vec::new(),
            }))
        })
        .unwrap_or(ptr::null_mut())
}
