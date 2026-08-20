//! Unit tests for the CYOA runtime VM.
//!
//! Tests compile stories from embedded source and exercise the VM.

use crate::*;
use cyoa_bytecode::Bytecode;
use cyoa_compiler::compile_story;

/// Helper: compile a source string to bytecode (no imports).
fn compile(source: &str) -> Bytecode {
    let story = cyoa_compiler::parse_story(source).expect("should parse");
    compile_story(&story).expect("should compile")
}

// ===== Template rendering tests =====

#[test]
fn test_render_template_no_placeholders() {
    let bc = compile(
        r#"
story T:
  event start:
    "Hello world"
"#,
    );
    let engine = Engine::new(bc.clone());
    assert_eq!(engine.render_template("Hello world"), "Hello world");
}

#[test]
fn test_render_template_with_stat() {
    let bc = compile(
        r#"
story T:
  stat gold = 0
  event start:
    "You have {{gold}} gold."
"#,
    );
    let mut engine = Engine::new(bc);

    // gold = 0
    let text = engine.current_event_text();
    assert_eq!(text.len(), 1);
    assert_eq!(text[0], "You have 0 gold.");

    // Change gold and check again
    engine.state.stats.insert("gold".into(), 42);
    let text = engine.current_event_text();
    assert_eq!(text[0], "You have 42 gold.");
}

#[test]
fn test_render_template_multiple_placeholders() {
    let bc = compile(
        r#"
story T:
  stat hp = 0
  stat gold = 0
  event start:
    "HP: {{hp}}, Gold: {{gold}}"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.state.stats.insert("hp".into(), 10);
    engine.state.stats.insert("gold".into(), 99);
    let text = engine.current_event_text();
    assert_eq!(text[0], "HP: 10, Gold: 99");
}

#[test]
fn test_render_template_missing_stat_renders_zero() {
    let bc = compile(
        r#"
story T:
  event start:
    "Value: {{nonexistent}}"
"#,
    );
    let engine = Engine::new(bc);
    // {{nonexistent}} should render as 0
    let text = engine.current_event_text();
    assert_eq!(text[0], "Value: 0");
}

// ===== Condition evaluation tests =====

#[test]
fn test_condition_bare_flag_true() {
    let bc = compile(
        r#"
story T:
  flag visited
  event start:
    "Hello"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.state.flags.insert("visited".into());
    assert!(engine.evaluate_condition("visited"));
    engine.state.flags.remove("visited");
    assert!(!engine.evaluate_condition("visited"));
}

#[test]
fn test_condition_stat_compare() {
    let bc = compile(
        r#"
story T:
  stat courage = 0
  event start:
    "Hello"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.state.stats.insert("courage".into(), 7);

    assert!(engine.evaluate_condition("courage >= 5"));
    assert!(!engine.evaluate_condition("courage >= 10"));
    assert!(engine.evaluate_condition("courage > 5"));
    assert!(engine.evaluate_condition("courage <= 7"));
    assert!(engine.evaluate_condition("courage < 10"));
    assert!(engine.evaluate_condition("courage == 7"));
    assert!(!engine.evaluate_condition("courage == 8"));
    assert!(engine.evaluate_condition("courage != 8"));
}

#[test]
fn test_condition_and() {
    let bc = compile(
        r#"
story T:
  stat courage = 0
  flag visited
  event start:
    "Hello"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.state.stats.insert("courage".into(), 10);
    engine.state.flags.insert("visited".into());

    assert!(engine.evaluate_condition("visited AND courage >= 5"));
    assert!(!engine.evaluate_condition("visited AND courage >= 20"));
    engine.state.flags.remove("visited");
    assert!(!engine.evaluate_condition("visited AND courage >= 5"));
}

#[test]
fn test_condition_or() {
    let bc = compile(
        r#"
story T:
  stat courage = 0
  stat hp = 0
  event start:
    "Hello"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.state.stats.insert("courage".into(), 2);
    engine.state.stats.insert("hp".into(), 3);

    assert!(engine.evaluate_condition("courage >= 5 OR hp > 0"));
    assert!(!engine.evaluate_condition("courage >= 5 OR hp > 100"));
}

#[test]
fn test_condition_not() {
    let bc = compile(
        r#"
story T:
  flag visited
  event start:
    "Hello"
"#,
    );
    let mut engine = Engine::new(bc);

    assert!(engine.evaluate_condition("NOT visited"));
    engine.state.flags.insert("visited".into());
    assert!(!engine.evaluate_condition("NOT visited"));
}

#[test]
fn test_condition_parens() {
    let bc = compile(
        r#"
story T:
  stat courage = 0
  stat hp = 0
  flag visited
  event start:
    "Hello"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.state.stats.insert("courage".into(), 10);
    engine.state.stats.insert("hp".into(), 2);
    engine.state.flags.insert("visited".into());

    assert!(engine.evaluate_condition("(courage >= 5 OR hp > 100) AND visited"));
    assert!(!engine.evaluate_condition("(courage >= 5 OR hp > 100) AND NOT visited"));

    engine.state.stats.insert("courage".into(), 2);
    assert!(!engine.evaluate_condition("(courage >= 5 OR hp > 100) AND visited"));
}

// ===== Full VM flow tests =====

#[test]
fn test_engine_current_event_text() {
    let bc = compile(
        r#"
story Test:
  event start:
    "First paragraph."
    "Second paragraph."
    choice "Continue":
      next second
  event second:
    "The end."
"#,
    );
    let engine = Engine::new(bc);
    let text = engine.current_event_text();
    assert_eq!(text.len(), 2);
    assert_eq!(text[0], "First paragraph.");
    assert_eq!(text[1], "Second paragraph.");
}

#[test]
fn test_engine_current_choices() {
    let bc = compile(
        r#"
story Test:
  event start:
    "Choose:"
    choice "Left":
      next left
    choice "Right":
      next right
  event left:
    "Left path"
  event right:
    "Right path"
"#,
    );
    let engine = Engine::new(bc);
    let choices = engine.current_choices();
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0], "Left");
    assert_eq!(choices[1], "Right");
}

#[test]
fn test_engine_choice_with_stat_change() {
    let bc = compile(
        r#"
story Test:
  stat hp = 50
  event start:
    "You drink a potion."
    choice "Drink":
      + hp by 20
      next end
  event end:
    "You feel better."
"#,
    );
    let mut engine = Engine::new(bc);
    assert_eq!(engine.get_stat("hp"), 50);

    engine.make_choice(0);
    assert_eq!(engine.get_stat("hp"), 70);
}

#[test]
fn test_engine_choice_negative_stat() {
    let bc = compile(
        r#"
story Test:
  stat hp = 50
  event start:
    "You take damage."
    choice "Ouch":
      - hp by 15
      next end
  event end:
    "You are hurt."
"#,
    );
    let mut engine = Engine::new(bc);
    assert_eq!(engine.get_stat("hp"), 50);

    engine.make_choice(0);
    // delta is -15, but operand_b is u32; the codegen stores it as *delta as u32
    // For negative numbers, this wraps. Let me check what the codegen does...
    // Actually, codegen does: self.emit_instruction(Opcode::ChangeStat, stat_idx, *delta as u32);
    // So delta = -15 becomes u32::MAX - 14 = 4294967281
    // In the VM: (instr.operand_b as i32) as i64 = -15
    // So it should work!
    assert_eq!(engine.get_stat("hp"), 35);
}

#[test]
fn test_engine_choice_sets_flag() {
    let bc = compile(
        r#"
story Test:
  event start:
    "You explore."
    choice "Search":
      set found_key to true
      next end
  event end:
    "You found the key."
"#,
    );
    let mut engine = Engine::new(bc);
    assert!(!engine.list_flags().contains(&"found_key".to_string()));

    engine.make_choice(0);
    assert!(engine.list_flags().contains(&"found_key".to_string()));
}

#[test]
fn test_engine_choice_adds_tag() {
    let bc = compile(
        r#"
story Test:
  event start:
    "Combat!"
    choice "Attack":
      add combat_started
      next end
  event end:
    "Done."
"#,
    );
    let mut engine = Engine::new(bc);
    assert!(!engine.list_tags().contains(&"combat_started".to_string()));

    engine.make_choice(0);
    assert!(engine.list_tags().contains(&"combat_started".to_string()));
}

#[test]
fn test_engine_uses_effect_block() {
    let bc = compile(
        r#"
story Test:
  stat hp = 0
  effect heal:
    + hp by 30
    text "You feel restored."

  event start:
    "You are wounded."
    choice "Heal":
      uses heal
      next end
  event end:
    "Done."
"#,
    );
    let mut engine = Engine::new(bc);
    assert_eq!(engine.get_stat("hp"), 0);

    engine.make_choice(0);
    assert_eq!(engine.get_stat("hp"), 30);
}

#[test]
fn test_engine_advances_to_next_event() {
    let bc = compile(
        r#"
story Test:
  event start:
    "First"
    choice "Go":
      next second
  event second:
    "Second"
    choice "Go":
      next third
  event third:
    "Third"
"#,
    );
    let mut engine = Engine::new(bc);

    assert_eq!(engine.current_event_text(), vec!["First".to_string()]);
    engine.make_choice(0);
    assert_eq!(engine.current_event_text(), vec!["Second".to_string()]);
    engine.make_choice(0);
    assert_eq!(engine.current_event_text(), vec!["Third".to_string()]);
}

#[test]
fn test_engine_choice_history_recorded() {
    let bc = compile(
        r#"
story Test:
  event start:
    "First"
    choice "A":
      next second
  event second:
    "Second"
    choice "B":
      next end
  event end:
    "Done."
"#,
    );
    let mut engine = Engine::new(bc);

    engine.make_choice(0);
    assert_eq!(engine.history().len(), 1);
    assert_eq!(engine.history()[0].event_id, "start");
    assert_eq!(engine.history()[0].choice_index, 0);
    assert_eq!(engine.history()[0].choice_text, "A");

    engine.make_choice(0);
    assert_eq!(engine.history().len(), 2);
    assert_eq!(engine.history()[1].event_id, "second");
}

#[test]
fn test_engine_prerequisite_hidden_choice() {
    let bc = compile(
        r#"
story Test:
  stat courage = 0
  event start:
    "Choose:"
    choice "Fight":
      requires: courage >= 10
      + courage by 5
      next end
    choice "Run":
      next end
  event end:
    "Done."
"#,
    );
    let mut engine = Engine::new(bc);

    // courage = 0, so "Fight" choice should be hidden
    let choices = engine.current_choices();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0], "Run");

    // Now make the "Run" choice (index 0 since Fight is hidden)
    engine.make_choice(0);
    assert_eq!(engine.history().len(), 1);
    assert_eq!(engine.history()[0].choice_text, "Run");
}

#[test]
fn test_engine_template_in_choice_text() {
    let bc = compile(
        r#"
story T:
  stat gold = 0
  event start:
    "Start"
    choice "Spend {{gold}} gold":
      next end
  event end:
    "End"
"#,
    );
    let engine = Engine::new(bc);
    let choices = engine.current_choices();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0], "Spend 0 gold");
}

#[test]
fn test_engine_state_json_serialization() {
    let bc = compile(
        r#"
story T:
  stat hp = 50
  flag found_key
  event start:
    "Start"
    choice "Done":
      + hp by 10
      set found_key to true
      next end
  event end:
  "End"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.make_choice(0);

    let json = engine.get_state_json();
    assert!(json.contains("60")); // 50 + 10 = 60
    assert!(json.contains("\"found_key\""));
    // Comprehensive state JSON includes cursor info
    assert!(json.contains("current_event"));
    assert!(json.contains("choice_history"));
}

#[test]
fn test_engine_state_json_roundtrip() {
    let bc = compile(
        r#"
story T:
  stat hp = 50
  stat gold = 0
  flag found_key
  event start:
    "Start"
    choice "Act":
      + hp by 10
      - gold by 5
      set found_key to true
      next end
  event end:
    "End"
"#,
    );
    let mut engine = Engine::new(bc.clone());
    engine.make_choice(0);

    // Get state
    let save_json = engine.get_state_json();

    // Restore to a fresh engine
    let mut engine2 = Engine::new(bc);
    engine2.set_state_json(&save_json).unwrap();

    // Verify state was restored
    let restored_json = engine2.get_state_json();
    assert!(restored_json.contains("\"hp\":60"));
    assert!(restored_json.contains("\"found_key\""));
    assert!(restored_json.contains("\"gold\":-5"));
}

#[test]
fn test_engine_multiple_effects_used() {
    let bc = compile(
        r#"
story T:
  stat hp = 0
  stat courage = 0
  effect heal:
    + hp by 20
  effect boost:
    + courage by 5

  event start:
    "Start"
    choice "Act":
      uses heal, boost
      next end
  event end:
    "End"
"#,
    );
    let mut engine = Engine::new(bc);
    engine.make_choice(0);
    assert_eq!(engine.get_stat("hp"), 20);
    assert_eq!(engine.get_stat("courage"), 5);
}

#[test]
fn test_engine_list_story_tags() {
    let bc = compile(
        r#"
story T:
  tags: fantasy, exploration

  stat hp = 50
  event start:
    "Hello"
"#,
    );
    let engine = Engine::new(bc);
    let tags = engine.list_story_tags();
    assert_eq!(tags, vec!["fantasy", "exploration"]);
}

#[test]
fn test_engine_list_story_tags_empty() {
    let bc = compile(
        r#"
story T:
  stat hp = 50
  event start:
    "Hello"
"#,
    );
    let engine = Engine::new(bc);
    assert!(engine.list_story_tags().is_empty());
}

#[test]
fn test_engine_story_tags_vs_runtime_tags() {
    let bc = compile(
        r#"
story T:
  tags: narrative

  event start:
    "Start"
    choice "Attack":
      add combat_started
      next end
  event end:
    "End"
"#,
    );
    let mut engine = Engine::new(bc);

    // Story tags are declared in the bytecode
    let story_tags = engine.list_story_tags();
    assert_eq!(story_tags, vec!["narrative"]);

    // Runtime tags are empty before making choices
    assert!(engine.list_tags().is_empty());

    // After making a choice that adds a tag, runtime tags change
    engine.make_choice(0);
    let runtime_tags = engine.list_tags();
    assert_eq!(runtime_tags, vec!["combat_started"]);

    // Story tags are unchanged — they're static metadata
    let story_tags_after = engine.list_story_tags();
    assert_eq!(story_tags_after, vec!["narrative"]);
}

#[test]
fn test_engine_can_access_event() {
    let bc = compile(
        r#"
story T:
  event start:
    "A"
    choice "Go":
      next end
  event end:
    "B"
"#,
    );
    let engine = Engine::new(bc);
    assert!(engine.can_access_event("start"));
    assert!(engine.can_access_event("end"));
    assert!(!engine.can_access_event("nonexistent"));
}

// ===== StoryCatalog tests =====

#[test]
fn test_story_catalog_register_and_list() {
    let bc1 = compile(
        r#"
story StoryA:
  tags: fantasy, exploration

  event start:
    "Story A"
"#,
    );
    let bc2 = compile(
        r#"
story StoryB:
  tags: combat

  event start:
    "Story B"
"#,
    );

    let mut catalog = StoryCatalog::new();
    assert!(catalog.is_empty());
    catalog.register(bc1);
    catalog.register(bc2);
    assert_eq!(catalog.len(), 2);

    let stories = catalog.list_stories();
    assert_eq!(stories.len(), 2);
    let names: Vec<&str> = stories.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"StoryA"));
    assert!(names.contains(&"StoryB"));
}

#[test]
fn test_story_catalog_stories_with_tag() {
    let bc1 = compile(
        r#"
story FantasyStory:
  tags: fantasy, exploration

  event start:
    "A"
"#,
    );
    let bc2 = compile(
        r#"
story CombatStory:
  tags: combat, dangerous

  event start:
    "B"
"#,
    );
    let bc3 = compile(
        r#"
story MixedStory:
  tags: fantasy, combat

  event start:
    "C"
"#,
    );

    let mut catalog = StoryCatalog::new();
    catalog.register(bc1);
    catalog.register(bc2);
    catalog.register(bc3);

    // Story A and Story C have "fantasy"
    let fantasy = catalog.stories_with_tag("fantasy");
    assert_eq!(fantasy.len(), 2);
    let fantasy_names: Vec<&str> = fantasy.iter().map(|s| s.name.as_str()).collect();
    assert!(fantasy_names.contains(&"FantasyStory"));
    assert!(fantasy_names.contains(&"MixedStory"));

    // Story B and Story C have "combat"
    let combat = catalog.stories_with_tag("combat");
    assert_eq!(combat.len(), 2);
    let combat_names: Vec<&str> = combat.iter().map(|s| s.name.as_str()).collect();
    assert!(combat_names.contains(&"CombatStory"));
    assert!(combat_names.contains(&"MixedStory"));

    // No story has "sci-fi"
    let scifi = catalog.stories_with_tag("sci-fi");
    assert!(scifi.is_empty());
}

#[test]
fn test_story_catalog_stories_with_all_tags() {
    let bc1 = compile(
        r#"
story StoryA:
  tags: fantasy, exploration, early_game

  event start:
    "A"
"#,
    );
    let bc2 = compile(
        r#"
story StoryB:
  tags: fantasy, combat

  event start:
    "B"
"#,
    );
    let bc3 = compile(
        r#"
story StoryC:
  tags: fantasy, exploration

  event start:
    "C"
"#,
    );

    let mut catalog = StoryCatalog::new();
    catalog.register(bc1);
    catalog.register(bc2);
    catalog.register(bc3);

    // Stories with BOTH "fantasy" AND "exploration"
    let both = catalog.stories_with_all_tags(&["fantasy", "exploration"]);
    assert_eq!(both.len(), 2);
    let names: Vec<&str> = both.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"StoryA"));
    assert!(names.contains(&"StoryC"));

    // Stories with "fantasy" AND "exploration" AND "early_game" — only StoryA
    let triple = catalog.stories_with_all_tags(&["fantasy", "exploration", "early_game"]);
    assert_eq!(triple.len(), 1);
    assert_eq!(triple[0].name, "StoryA");

    // No story has all three
    let none = catalog.stories_with_all_tags(&["fantasy", "combat", "exploration"]);
    assert!(none.is_empty());
}

#[test]
fn test_story_catalog_stories_with_any_tags() {
    let bc1 = compile(
        r#"
story StoryA:
  tags: fantasy

  event start:
    "A"
"#,
    );
    let bc2 = compile(
        r#"
story StoryB:
  tags: combat

  event start:
    "B"
"#,
    );
    let bc3 = compile(
        r#"
story StoryC:
  tags: romance

  event start:
    "C"
"#,
    );

    let mut catalog = StoryCatalog::new();
    catalog.register(bc1);
    catalog.register(bc2);
    catalog.register(bc3);

    // Any story with "fantasy" OR "romance"
    let matched = catalog.stories_with_any_tags(&["fantasy", "romance"]);
    assert_eq!(matched.len(), 2);
    let names: Vec<&str> = matched.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"StoryA"));
    assert!(names.contains(&"StoryC"));
}

#[test]
fn test_story_catalog_create_engine() {
    let bc = compile(
        r#"
story MyStory:
  tags: fantasy

  stat hp = 50
  event start:
    "Hello"
"#,
    );

    let mut catalog = StoryCatalog::new();
    catalog.register(bc);

    let engine = catalog.create_engine(0);
    assert!(engine.is_some());
    let engine = engine.unwrap();
    assert_eq!(engine.get_stat("hp"), 50);
    assert_eq!(engine.list_story_tags(), vec!["fantasy"]);
}

#[test]
fn test_story_catalog_create_engine_by_name() {
    let bc = compile(
        r#"
story NamedStory:
  tags: fantasy, adventure

  stat gold = 0
  event start:
    "Hello"
"#,
    );

    let mut catalog = StoryCatalog::new();
    catalog.register(bc);

    let engine = catalog.create_engine_by_name("NamedStory");
    assert!(engine.is_some());
    assert_eq!(
        engine.unwrap().list_story_tags(),
        vec!["fantasy", "adventure"]
    );

    // Non-existent story returns None
    let none = catalog.create_engine_by_name("DoesNotExist");
    assert!(none.is_none());
}

#[test]
fn test_story_catalog_empty_tags() {
    let bc = compile(
        r#"
story NoTags:
  stat hp = 10

  event start:
    "Hello"
"#,
    );

    let mut catalog = StoryCatalog::new();
    catalog.register(bc);

    // Story with no tags
    assert!(catalog.list_stories()[0].tags.is_empty());

    // Filtering by any tag returns empty
    let matched = catalog.stories_with_tag("anything");
    assert!(matched.is_empty());
    let matched = catalog.stories_with_all_tags(&["anything"]);
    assert!(matched.is_empty());
    let matched = catalog.stories_with_any_tags(&["anything"]);
    assert!(matched.is_empty());
}
