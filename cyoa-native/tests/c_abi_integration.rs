//! Integration tests for the C-ABI (`cyoa-native`).
//!
//! These tests simulate a foreign caller (C/C#/etc.) by going through the
//! `extern "C"` functions: compiling a story, serializing to bytes, passing
//! them through the C-ABI boundary, and verifying the returned values.
//!
//! This is the only test file that exercises the handle-based API end-to-end.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use cyoa_bytecode::Bytecode;
use cyoa_compiler::{compile_story, parse_story};
use cyoa_native::{
    cyoa_available_events_json, cyoa_can_access_event, cyoa_catalog_create,
    cyoa_catalog_create_engine, cyoa_catalog_create_engine_by_name, cyoa_catalog_destroy,
    cyoa_catalog_list_stories_json, cyoa_catalog_register, cyoa_catalog_stories_with_all_tags_json,
    cyoa_catalog_stories_with_any_tags_json, cyoa_catalog_stories_with_tag_json,
    cyoa_catalog_story_count, cyoa_choice_text, cyoa_create, cyoa_current_choice_count,
    cyoa_current_event_id, cyoa_current_event_text, cyoa_destroy, cyoa_get_stat,
    cyoa_get_state_json, cyoa_history_entry, cyoa_history_length, cyoa_last_effect_text,
    cyoa_list_flags_json, cyoa_list_stats_json, cyoa_list_story_tags_json, cyoa_list_tags_json,
    cyoa_make_choice, cyoa_set_state_json,
};

/// Compile a source string into serialized bytecode bytes (Vec<u8>).
fn compile_to_bytes(source: &str) -> Vec<u8> {
    let story = parse_story(source).expect("should parse");
    let bc: Bytecode = compile_story(&story).expect("should compile");
    bc.to_bytes().expect("should serialize")
}

/// Convert a `*const c_char` (engine-owned) to a Rust `String`.
/// Does NOT free — the pointer belongs to the engine.
fn cstr_to_string(ptr: *const c_char) -> String {
    assert!(!ptr.is_null(), "null pointer returned");
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap().to_string()
}

/// Convert a `*mut c_char` (heap-allocated JSON) to a Rust `String`,
/// then free the native allocation.
fn move_json_out_mut(ptr: *mut c_char) -> String {
    assert!(!ptr.is_null(), "null JSON pointer returned");
    unsafe { CString::from_raw(ptr) }
        .to_str()
        .unwrap()
        .to_string()
}

/// A multi-event story with stats, flags, tags, and effects.
const STORY_SOURCE: &str = r#"
story TestAdventure:
  tags: fantasy, exploration

  stat hp = 50
  stat courage = 0
  stat gold = 0

  effect found_mushroom:
    + courage by 1
    text "You find a glowing mushroom."

  event start:
    "You stand at the entrance of a cave."
    "The darkness yawns before you."

    choice "Enter the cave":
      set visited_cave to true
      next deep_in_cave

    choice "Walk away":
      next gave_up

  event deep_in_cave:
    requires: courage >= 0
    "It's dark inside. You hear a growl."

    choice "Fight the growl" uses found_mushroom:
      + courage by 2
      - hp by 5
      "You draw your sword."
      next victory

    choice "Flee":
      - courage by 1
      next start

  event gave_up:
    "You turn back. The adventure ends."

  event victory:
    "You emerge victorious!"
"#;

// ═══════════════════════════════════════════════════════════════════════════
// Engine lifecycle tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cyoa_create_null_returns_null() {
    let result = cyoa_create(ptr::null(), 0);
    assert!(result.is_null(), "should return NULL for null bytecode");
}

#[test]
fn test_cyoa_create_and_destroy() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null(), "engine should be created");
    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_create_invalid_bytecode_returns_null() {
    let bad_bytes = b"this is not valid bytecode";
    let engine = cyoa_create(bad_bytes.as_ptr(), bad_bytes.len());
    assert!(engine.is_null(), "should return NULL for invalid bytecode");
}

// ═══════════════════════════════════════════════════════════════════════════
// Event query tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cyoa_current_event_id() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let id = cstr_to_string(cyoa_current_event_id(engine));
    assert_eq!(id, "start");

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_current_event_text() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let text = cstr_to_string(cyoa_current_event_text(engine));
    // Two paragraphs joined by newline
    assert!(text.contains("You stand at the entrance of a cave."));
    assert!(text.contains("The darkness yawns before you."));
    assert!(text.contains('\n'));

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_current_choice_count() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    assert_eq!(cyoa_current_choice_count(engine), 2);

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_choice_text() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let choice0 = cstr_to_string(cyoa_choice_text(engine, 0));
    assert_eq!(choice0, "Enter the cave");

    let choice1 = cstr_to_string(cyoa_choice_text(engine, 1));
    assert_eq!(choice1, "Walk away");

    // Out of bounds returns NULL
    let oob = cyoa_choice_text(engine, 5);
    assert!(oob.is_null());

    cyoa_destroy(engine);
}

// ═══════════════════════════════════════════════════════════════════════════
// Choice + effect text tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cyoa_make_choice_and_effect_text() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    // Before any choice, effect text should be empty
    let empty = cstr_to_string(cyoa_last_effect_text(engine));
    assert_eq!(empty, "");

    // Make first choice ("Enter the cave") → advances to deep_in_cave
    cyoa_make_choice(engine, 0);

    // After advancing, the current event should be deep_in_cave
    let id = cstr_to_string(cyoa_current_event_id(engine));
    assert_eq!(id, "deep_in_cave");

    // Make second choice ("Fight the growl" uses found_mushroom)
    cyoa_make_choice(engine, 0);

    // The found_mushroom effect should have produced "glowing mushroom" text
    let effects2 = cstr_to_string(cyoa_last_effect_text(engine));
    assert!(
        effects2.contains("glowing mushroom"),
        "should contain effect text from uses, got: {}",
        effects2
    );

    // After advancing, current event should be victory
    let id2 = cstr_to_string(cyoa_current_event_id(engine));
    assert_eq!(id2, "victory");

    cyoa_destroy(engine);
}

// ═══════════════════════════════════════════════════════════════════════════
// State JSON tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cyoa_get_state_json() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let json = move_json_out_mut(cyoa_get_state_json(engine));

    // Verify it has the expected structure
    assert!(json.contains("\"stats\""));
    assert!(json.contains("\"hp\""));
    assert!(json.contains("\"courage\""));
    assert!(json.contains("\"gold\""));
    assert!(json.contains("\"flags\""));
    assert!(json.contains("\"tags\""));

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_set_state_json_roundtrip() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    // Make a choice to change state
    cyoa_make_choice(engine, 0); // Enter the cave → visited_cave flag set

    // Get state
    let save_json = move_json_out_mut(cyoa_get_state_json(engine));
    assert!(save_json.contains("visited_cave"));

    // Restore to a fresh engine
    let engine2 = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine2.is_null());

    // Verify it doesn't have the flag yet
    let state_before = move_json_out_mut(cyoa_get_state_json(engine2));
    assert!(!state_before.contains("visited_cave"));

    // Set state
    let c_json = CString::new(save_json.as_str()).unwrap();
    cyoa_set_state_json(engine2, c_json.as_ptr());

    // Verify flag is now set
    let state_after = move_json_out_mut(cyoa_get_state_json(engine2));
    assert!(state_after.contains("visited_cave"));

    cyoa_destroy(engine);
    cyoa_destroy(engine2);
}

// ═══════════════════════════════════════════════════════════════════════════
// Stats, tags, flags tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cyoa_get_stat() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    // hp starts at 50
    assert_eq!(cyoa_get_stat(engine, c"hp".as_ptr()), 50);
    // courage starts at 0
    assert_eq!(cyoa_get_stat(engine, c"courage".as_ptr()), 0);
    // gold starts at 0
    assert_eq!(cyoa_get_stat(engine, c"gold".as_ptr()), 0);
    // Unknown stat returns 0
    assert_eq!(cyoa_get_stat(engine, c"nonexistent".as_ptr()), 0);

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_list_stats_json() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let json = move_json_out_mut(cyoa_list_stats_json(engine));
    assert!(json.contains("\"hp\":50"));
    assert!(json.contains("\"courage\":0"));
    assert!(json.contains("\"gold\":0"));

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_list_story_tags() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let json = move_json_out_mut(cyoa_list_story_tags_json(engine));
    assert!(json.contains("fantasy"));
    assert!(json.contains("exploration"));

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_list_tags_and_flags_empty_at_start() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let tags = move_json_out_mut(cyoa_list_tags_json(engine));
    assert_eq!(tags, "[]");

    let flags = move_json_out_mut(cyoa_list_flags_json(engine));
    assert_eq!(flags, "[]");

    cyoa_destroy(engine);
}

// ═══════════════════════════════════════════════════════════════════════════
// History tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cyoa_history_roundtrip() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    // No history at start
    assert_eq!(cyoa_history_length(engine), 0);

    // Make a choice
    cyoa_make_choice(engine, 0);

    // Now we have one history entry
    assert_eq!(cyoa_history_length(engine), 1);

    let entry = cstr_to_string(cyoa_history_entry(engine, 0));
    // Should be JSON with eventId, choiceIndex, choiceText
    assert!(entry.contains("\"eventId\":\"start\""));
    assert!(entry.contains("\"choiceIndex\":0"));

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_history_entry_oob_returns_null() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    assert!(cyoa_history_entry(engine, 0).is_null());
    assert!(cyoa_history_entry(engine, -1).is_null());

    cyoa_destroy(engine);
}

// ═══════════════════════════════════════════════════════════════════════════
// Query tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_cyoa_available_events_json() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let json = move_json_out_mut(cyoa_available_events_json(engine));
    assert!(json.contains("start"));
    assert!(json.contains("deep_in_cave"));
    assert!(json.contains("victory"));
    assert!(json.contains("gave_up"));

    cyoa_destroy(engine);
}

#[test]
fn test_cyoa_can_access_event() {
    let bytes = compile_to_bytes(STORY_SOURCE);
    let engine = cyoa_create(bytes.as_ptr(), bytes.len());
    assert!(!engine.is_null());

    let id = CString::new("deep_in_cave").unwrap();
    assert_eq!(cyoa_can_access_event(engine, id.as_ptr()), 1);

    let missing = CString::new("nonexistent_event").unwrap();
    assert_eq!(cyoa_can_access_event(engine, missing.as_ptr()), 0);

    cyoa_destroy(engine);
}

// ═══════════════════════════════════════════════════════════════════════════
// Catalog tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_catalog_lifecycle() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());
    assert_eq!(cyoa_catalog_story_count(catalog), 0);

    let bytes = compile_to_bytes(STORY_SOURCE);
    let result = cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());
    assert_eq!(result, 1, "registration should succeed");

    assert_eq!(cyoa_catalog_story_count(catalog), 1);

    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_register_null_rejected() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let result = cyoa_catalog_register(catalog, ptr::null(), 0);
    assert_eq!(result, 0, "should fail on null bytecode");

    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_list_stories_json() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let bytes = compile_to_bytes(STORY_SOURCE);
    cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());

    let json = move_json_out_mut(cyoa_catalog_list_stories_json(catalog));
    assert!(json.contains("TestAdventure"));
    assert!(json.contains("fantasy"));
    assert!(json.contains("exploration"));

    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_stories_with_tag() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let bytes = compile_to_bytes(STORY_SOURCE);
    cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());

    // Stories with tag "fantasy"
    let tag = CString::new("fantasy").unwrap();
    let json = move_json_out_mut(cyoa_catalog_stories_with_tag_json(catalog, tag.as_ptr()));
    assert!(json.contains("TestAdventure"));

    // Stories with a nonexistent tag
    let missing = CString::new("nonexistent").unwrap();
    let json_empty = move_json_out_mut(cyoa_catalog_stories_with_tag_json(
        catalog,
        missing.as_ptr(),
    ));
    assert_eq!(json_empty, "[]");

    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_stories_with_all_tags() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let bytes = compile_to_bytes(STORY_SOURCE);
    cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());

    // Stories with ALL tags: fantasy + exploration
    let tags_json = CString::new(r#"["fantasy","exploration"]"#).unwrap();
    let json = move_json_out_mut(cyoa_catalog_stories_with_all_tags_json(
        catalog,
        tags_json.as_ptr(),
    ));
    assert!(json.contains("TestAdventure"));

    // Stories with ALL tags: fantasy + combat (combat not present → empty)
    let tags_no_match = CString::new(r#"["fantasy","combat"]"#).unwrap();
    let json_empty = move_json_out_mut(cyoa_catalog_stories_with_all_tags_json(
        catalog,
        tags_no_match.as_ptr(),
    ));
    assert_eq!(json_empty, "[]");

    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_stories_with_any_tags() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let bytes = compile_to_bytes(STORY_SOURCE);
    cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());

    // Stories with ANY tag: fantasy + horror (fantasy matches)
    let tags_json = CString::new(r#"["fantasy","horror"]"#).unwrap();
    let json = move_json_out_mut(cyoa_catalog_stories_with_any_tags_json(
        catalog,
        tags_json.as_ptr(),
    ));
    assert!(json.contains("TestAdventure"));

    // Stories with ANY tag: politics + economics (no match → empty)
    let tags_no_match = CString::new(r#"["politics","economics"]"#).unwrap();
    let json_empty = move_json_out_mut(cyoa_catalog_stories_with_any_tags_json(
        catalog,
        tags_no_match.as_ptr(),
    ));
    assert_eq!(json_empty, "[]");

    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_create_engine() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let bytes = compile_to_bytes(STORY_SOURCE);
    cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());

    // Create engine by index
    let engine = cyoa_catalog_create_engine(catalog, 0);
    assert!(!engine.is_null(), "should create engine at index 0");

    // Verify the engine is functional
    let id = cstr_to_string(cyoa_current_event_id(engine));
    assert_eq!(id, "start");

    cyoa_destroy(engine);
    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_create_engine_by_name() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let bytes = compile_to_bytes(STORY_SOURCE);
    cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());

    // Create engine by name
    let name = CString::new("TestAdventure").unwrap();
    let engine = cyoa_catalog_create_engine_by_name(catalog, name.as_ptr());
    assert!(!engine.is_null(), "should create engine by name");

    let id = cstr_to_string(cyoa_current_event_id(engine));
    assert_eq!(id, "start");

    cyoa_destroy(engine);
    cyoa_catalog_destroy(catalog);
}

#[test]
fn test_catalog_create_engine_out_of_bounds() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    let bytes = compile_to_bytes(STORY_SOURCE);
    cyoa_catalog_register(catalog, bytes.as_ptr(), bytes.len());

    // Index out of bounds
    let engine = cyoa_catalog_create_engine(catalog, 5);
    assert!(engine.is_null(), "should return NULL for OOB index");

    // Name not found
    let name = CString::new("Nonexistent").unwrap();
    let engine2 = cyoa_catalog_create_engine_by_name(catalog, name.as_ptr());
    assert!(engine2.is_null(), "should return NULL for unknown name");

    cyoa_catalog_destroy(catalog);
}

// ═══════════════════════════════════════════════════════════════════════════
// Multi-story catalog test
// ═══════════════════════════════════════════════════════════════════════════

const STORY_B_SOURCE: &str = r#"
story SideQuest:
  tags: combat, short

  stat hp = 30

  event start:
    "A goblin appears!"

    choice "Attack":
      next fight
    choice "Run":
      next escape

  event fight:
    "You win!"

  event escape:
    "You ran away."
"#;

#[test]
fn test_catalog_multi_story_tag_filtering() {
    let catalog = cyoa_catalog_create();
    assert!(!catalog.is_null());

    // Register both stories
    let bytes_a = compile_to_bytes(STORY_SOURCE);
    let bytes_b = compile_to_bytes(STORY_B_SOURCE);
    assert_eq!(
        cyoa_catalog_register(catalog, bytes_a.as_ptr(), bytes_a.len()),
        1
    );
    assert_eq!(
        cyoa_catalog_register(catalog, bytes_b.as_ptr(), bytes_b.len()),
        1
    );

    assert_eq!(cyoa_catalog_story_count(catalog), 2);

    // Filter: stories with tag "fantasy" → only TestAdventure
    let tag = CString::new("fantasy").unwrap();
    let json = move_json_out_mut(cyoa_catalog_stories_with_tag_json(catalog, tag.as_ptr()));
    assert!(json.contains("TestAdventure"));
    assert!(!json.contains("SideQuest"));

    // Filter: stories with tag "combat" → only SideQuest
    let tag2 = CString::new("combat").unwrap();
    let json2 = move_json_out_mut(cyoa_catalog_stories_with_tag_json(catalog, tag2.as_ptr()));
    assert!(json2.contains("SideQuest"));
    assert!(!json2.contains("TestAdventure"));

    // Filter: stories with ALL tags ["fantasy","exploration"] → TestAdventure
    let all_tags = CString::new(r#"["fantasy","exploration"]"#).unwrap();
    let json3 = move_json_out_mut(cyoa_catalog_stories_with_all_tags_json(
        catalog,
        all_tags.as_ptr(),
    ));
    assert!(json3.contains("TestAdventure"));
    assert!(!json3.contains("SideQuest"));

    // Filter: stories with ANY tags ["fantasy","combat"] → both stories
    let any_tags = CString::new(r#"["fantasy","combat"]"#).unwrap();
    let json4 = move_json_out_mut(cyoa_catalog_stories_with_any_tags_json(
        catalog,
        any_tags.as_ptr(),
    ));
    assert!(json4.contains("TestAdventure"));
    assert!(json4.contains("SideQuest"));

    cyoa_catalog_destroy(catalog);
}
