//! Unit tests for the CYOA compiler.
//!
//! Tests the full pipeline: parse → resolve imports → compile to bytecode.
//! Also tests individual stages: parser, codegen, import resolver.

use crate::*;

// ===== Parser tests =====

#[test]
fn test_parse_simple_story() {
    let source = r#"
story HelloWorld:

  stat hp = 100

  event start:
    "You wake up in a forest."
    choice "Go north":
      next north_path
    choice "Go south":
      next south_path
"#;
    let story = parse_story(source).expect("should parse");
    assert_eq!(story.name, "HelloWorld");
    assert_eq!(story.items.len(), 2); // stat + event
}

#[test]
fn test_parse_stat_with_default() {
    let source = r#"
story Test:
  stat hp = 50
"#;
    let story = parse_story(source).unwrap();
    let stat = match &story.items[0] {
        StoryItem::StatDef(s) => s,
        _ => panic!("expected StatDef"),
    };
    assert_eq!(stat.name, "hp");
    assert_eq!(stat.default, 50);
}

#[test]
fn test_parse_flag() {
    let source = r#"
story Test:
  flag visited_cave
"#;
    let story = parse_story(source).unwrap();
    let flag = match &story.items[0] {
        StoryItem::FlagDef(f) => f,
        _ => panic!("expected FlagDef"),
    };
    assert_eq!(flag.name, "visited_cave");
    assert!(!flag.default);
}

#[test]
fn test_parse_flag_with_default() {
    let source = r#"
story Test:
  flag has_key = true
"#;
    let story = parse_story(source).unwrap();
    let flag = match &story.items[0] {
        StoryItem::FlagDef(f) => f,
        _ => panic!("expected FlagDef"),
    };
    assert!(flag.default);
}

#[test]
fn test_parse_effect_block() {
    let source = r#"
story Test:
  effect heal:
    + hp by 20
    text "You feel healthier."
"#;
    let story = parse_story(source).unwrap();
    let eff = match &story.items[0] {
        StoryItem::EffectDef(e) => e,
        _ => panic!("expected EffectDef"),
    };
    assert_eq!(eff.name, "heal");
}

#[test]
fn test_parse_effect_steps() {
    let source = r#"
story Test:
  stat hp = 0
  effect damage:
    - hp by 10
    text "You take damage."
  effect heal:
    + hp by 20
    set wounded to false
"#;
    let story = parse_story(source).unwrap();
    // stat, damage, heal
    assert_eq!(story.items.len(), 3);

    let damage = match &story.items[1] {
        StoryItem::EffectDef(e) => e,
        _ => panic!("expected EffectDef"),
    };
    assert_eq!(damage.body.len(), 2);
    assert_eq!(
        damage.body[0],
        EffectStep::ChangeStat {
            stat: "hp".into(),
            delta: -10
        }
    );

    let heal = match &story.items[2] {
        StoryItem::EffectDef(e) => e,
        _ => panic!("expected EffectDef"),
    };
    assert_eq!(heal.body.len(), 2);
    assert_eq!(
        heal.body[1],
        EffectStep::SetFlag {
            flag: "wounded".into(),
            value: false
        }
    );
}

#[test]
fn test_parse_add_tag() {
    let source = r#"
story Test:
  effect mark:
    add explored
"#;
    let story = parse_story(source).unwrap();
    let eff = match &story.items[0] {
        StoryItem::EffectDef(e) => e,
        _ => panic!("expected EffectDef"),
    };
    assert_eq!(
        eff.body[0],
        EffectStep::AddTag {
            tag: "explored".into()
        }
    );
}

#[test]
fn test_parse_event_with_text_and_choices() {
    let source = r#"
story Test:
  event start:
    "Welcome to the game."
    choice "Begin":
      next chapter_1
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[0] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert_eq!(ev.id, "start");
    assert_eq!(ev.text.len(), 1);
    assert_eq!(ev.choices.len(), 1);
    assert_eq!(ev.choices[0].next, Some("chapter_1".into()));
}

#[test]
fn test_parse_choice_with_uses() {
    let source = r#"
story Test:
  effect heal:
    + hp by 10
  event start:
    "You are wounded."
    choice "Use potion":
      uses heal
      next after
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[1] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert_eq!(ev.choices[0].uses, vec!["heal"]);
}

#[test]
fn test_parse_choice_with_inline_modifier() {
    let source = r#"
story Test:
  effect heal:
    + hp by 10
  event start:
    "You are wounded."
    choice "Use potion" uses heal:
      next after
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[1] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert_eq!(ev.choices[0].uses, vec!["heal"]);
}

#[test]
fn test_parse_event_requires_condition() {
    let source = r#"
story Test:
  stat courage = 0
  event gated:
    requires: courage >= 5 AND hp > 0
    "A dragon appears."
    choice "Fight":
      next dragon_fight
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[1] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert!(ev.requires.is_some());
}

#[test]
fn test_parse_event_tags() {
    let source = r#"
story Test:
  event start:
    tags: combat, evening
    "Combat time."
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[0] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert_eq!(ev.tags, vec!["combat", "evening"]);
}

#[test]
fn test_parse_story_tags() {
    let source = r#"
story ForestAdventure:
  tags: fantasy, exploration

  stat hp = 50

  event start:
    "You find yourself at the edge of a dark forest."
"#;
    let story = parse_story(source).unwrap();
    assert_eq!(story.tags, vec!["fantasy", "exploration"]);
    assert_eq!(story.name, "ForestAdventure");
}

#[test]
fn test_parse_story_tags_empty() {
    let source = r#"
story Test:

  stat hp = 50
"#;
    let story = parse_story(source).unwrap();
    assert!(story.tags.is_empty());
}

#[test]
fn test_parse_story_tags_single() {
    let source = r#"
story Test:
  tags: combat

  event start:
    "Fight!"
"#;
    let story = parse_story(source).unwrap();
    assert_eq!(story.tags, vec!["combat"]);
}

#[test]
fn test_parse_choice_requires() {
    let source = r#"
story Test:
  stat gold = 10
  event tavern:
    "The barkeep eyes your coin."
    choice "Buy ale (5 gold)":
      requires: gold >= 5
      next tavern
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[1] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert!(ev.choices[0].requires.is_some());
}

#[test]
fn test_parse_template_in_text() {
    let source = r#"
story Test:
  stat gold = 0
  event tavern:
    "You have {{gold}} gold pieces."
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[1] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    let text = &ev.text[0];
    assert!(text
        .segments
        .iter()
        .any(|s| matches!(s, TextSegment::StatRef(_))));
}

#[test]
fn test_parse_import() {
    let source = r#"
import "std/healing"
import "./local_stuff"

story Test:
  stat hp = 50
"#;
    let story = parse_story(source).unwrap();
    assert_eq!(story.imports.len(), 2);
    assert_eq!(story.imports[0].path, "std/healing");
    assert_eq!(story.imports[1].path, "./local_stuff");
}

#[test]
fn test_parse_comments_stripped() {
    let source = r#"# This is a comment
story Test:
  # comment inside story
  stat hp = 50  # inline comment
"#;
    let story = parse_story(source).unwrap();
    assert_eq!(story.name, "Test");
    let stat = match &story.items[0] {
        StoryItem::StatDef(s) => s,
        _ => panic!("expected StatDef"),
    };
    assert_eq!(stat.name, "hp");
}

#[test]
fn test_parse_condition_and() {
    let source = r#"
story Test:
  stat courage = 0
  stat hp = 0
  event gated:
    requires: courage >= 5 AND hp > 0
    "Text"
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[2] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    match ev.requires.as_ref().unwrap() {
        ConditionExpr::And(_, _) => {} // expected
        other => panic!("expected And, got {:?}", other),
    }
}

#[test]
fn test_parse_condition_not() {
    let source = r#"
story Test:
  event gated:
    requires: NOT visited_cave
    "Text"
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[0] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    match ev.requires.as_ref().unwrap() {
        ConditionExpr::Not(_) => {}
        other => panic!("expected Not, got {:?}", other),
    }
}

#[test]
fn test_parse_condition_parens() {
    let source = r#"
story Test:
  stat courage = 0
  stat hp = 0
  event gated:
    requires: (courage >= 5 OR courage >= 10) AND hp > 0
    "Text"
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[2] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    match ev.requires.as_ref().unwrap() {
        ConditionExpr::And(_, _) => {}
        other => panic!("expected And, got {:?}", other),
    }
}

// ===== Codegen tests =====

#[test]
fn test_compile_basic_story() {
    let source = r#"
story TestStory:
  stat hp = 100
  stat gold = 0

  event start:
    "Welcome to the test."
    choice "Go east":
      next east
    choice "Go west":
      next west

  event east:
    "You went east."
"#;
    let story = parse_story(source).unwrap();
    let bytecode = compile_story(&story).expect("compilation should succeed");

    // Check magic and version
    assert_eq!(bytecode.header.magic, cyoa_bytecode::MAGIC);
    assert_eq!(bytecode.header.version, cyoa_bytecode::VERSION);

    // Check stats were compiled
    assert_eq!(bytecode.stats.len(), 2);
    assert_eq!(bytecode.events.len(), 2); // start + east
    assert_eq!(bytecode.choices.len(), 2); // both choices in start
}

#[test]
fn test_compile_with_effects() {
    let source = r#"
story TestStory:
  stat hp = 50

  effect heal:
    + hp by 20

  effect damage:
    - hp by 10
    text "Ouch!"

  event start:
    "You begin your journey."
    choice "Explore":
      uses heal
      next explore
    choice "Rest":
      uses heal
      next rest

  event explore:
    "You explore."

  event rest:
    "You rest."
"#;
    let story = parse_story(source).unwrap();
    let bytecode = compile_story(&story).expect("compilation should succeed");

    // Should have 3 effects: none, but 2 effects
    assert_eq!(bytecode.effects.len(), 2);
    // 3 events
    assert_eq!(bytecode.events.len(), 3);
    // 2 choices in start
    assert_eq!(bytecode.choices.len(), 2);
}

#[test]
fn test_compile_with_conditions() {
    let source = r#"
story TestStory:
  stat courage = 0

  event start:
    "Begin."
    choice "Fight":
      requires: courage >= 5
      next fight
    choice "Run":
      next run

  event fight:
    "You fight."

  event run:
    "You run."
"#;
    let story = parse_story(source).unwrap();
    let bytecode = compile_story(&story).expect("compilation should succeed");

    // The choice with requires should have a non-zero requires field
    let start_event = &bytecode.events[0];
    let choice_start = start_event.choice_start as usize;
    let gated_choice = &bytecode.choices[choice_start]; // First choice
    assert_ne!(gated_choice.requires, 0); // Should have a condition

    let free_choice = &bytecode.choices[choice_start + 1]; // Second choice
    assert_eq!(free_choice.requires, 0); // No condition
}

#[test]
fn test_compile_template_text() {
    let source = r#"
story TestStory:
  stat gold = 0
  event start:
    "You have {{gold}} gold."
"#;
    let story = parse_story(source).unwrap();
    let bytecode = compile_story(&story).expect("compilation should succeed");

    // The text should be rendered as a template string in the string table
    let _event = &bytecode.events[0];
    let text_instr = bytecode
        .instruction_at(0)
        .expect("should have an instruction");
    assert_eq!(text_instr.opcode, cyoa_bytecode::Opcode::RenderTemplate);
}

#[test]
fn test_compile_simple_text() {
    let source = r#"
story TestStory:
  event start:
    "Simple text here."
"#;
    let story = parse_story(source).unwrap();
    let bytecode = compile_story(&story).expect("compilation should succeed");

    let _event = &bytecode.events[0];
    let text_instr = bytecode
        .instruction_at(0)
        .expect("should have an instruction");
    assert_eq!(text_instr.opcode, cyoa_bytecode::Opcode::GetText);
}

// ===== Import resolution tests =====

#[test]
fn test_resolve_imports_std() {
    use std::path::Path;
    let tmp_dir = std::env::temp_dir();
    let std_dir = tmp_dir.join("cyoa_test_std");
    let std_subdir = std_dir.join("std");
    std::fs::create_dir_all(&std_subdir).unwrap();

    // std/healing.cyoa — a library file without a story block
    std::fs::write(
        std_subdir.join("healing.cyoa"),
        "effect heal:\n  + hp by 10\n",
    )
    .unwrap();

    let source = r#"
import "std/healing"

story Test:
  stat hp = 0
  event start:
    "Hello"
    choice "Use heal":
      uses heal
      next start
"#;
    let story = parse_story(source).unwrap();
    let base_dir = Path::new(".");
    let result = resolve_imports(&story, base_dir, std::slice::from_ref(&std_subdir))
        .expect("should resolve");

    // Should have merged: heal effect + stat + event
    assert!(result.items.len() >= 3);
}

#[test]
fn test_resolve_imports_local() {
    let tmp_dir = std::env::temp_dir();
    let test_dir = tmp_dir.join("cyoa_test_local");
    std::fs::create_dir_all(&test_dir).unwrap();

    let local_file = test_dir.join("extra.cyoa");
    std::fs::write(
        &local_file,
        r#"
effect extra:
  + hp by 5
"#,
    )
    .unwrap();

    let source = r#"
import "./extra"

story Test:
  stat hp = 0
  event start:
    "Hello"
    choice "Go":
      uses extra
      next start
"#;
    let story = parse_story(source).unwrap();
    let result = resolve_imports(&story, &test_dir, &[]).expect("should resolve");
    assert!(result
        .items
        .iter()
        .any(|i| matches!(i, StoryItem::EffectDef(_))));
}

#[test]
fn test_resolve_imports_circular() {
    let tmp_dir = std::env::temp_dir();
    let test_dir = tmp_dir.join("cyoa_test_circular");
    std::fs::create_dir_all(&test_dir).unwrap();

    std::fs::write(
        test_dir.join("a.cyoa"),
        r#"import "./b"
story A:
  event x:
    "a"
"#,
    )
    .unwrap();

    std::fs::write(
        test_dir.join("b.cyoa"),
        r#"import "./a"
story B:
  event y:
    "b"
"#,
    )
    .unwrap();

    let source = r#"
story Main:
  import "./a"
  stat hp = 0
"#;
    let story = parse_story(source).unwrap();
    let result = resolve_imports(&story, &test_dir, &[]);
    assert!(result.is_err());
}

#[test]
fn test_resolve_imports_name_collision() {
    let tmp_dir = std::env::temp_dir();
    let test_dir = tmp_dir.join("cyoa_test_collision");
    std::fs::create_dir_all(&test_dir).unwrap();

    std::fs::write(
        test_dir.join("a.cyoa"),
        r#"
effect shared:
  + hp by 5
"#,
    )
    .unwrap();

    std::fs::write(
        test_dir.join("b.cyoa"),
        r#"
effect shared:
  + hp by 10
"#,
    )
    .unwrap();

    let source = r#"
import "./a"
import "./b"

story Main:
  stat hp = 0
"#;
    let story = parse_story(source).unwrap();
    let result = resolve_imports(&story, &test_dir, &[]);
    assert!(result.is_err());
}

// ===== End-to-end compile tests =====

#[test]
fn test_compile_forest_adventure() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(
        manifest_dir
            .join("..")
            .join("examples")
            .join("forest_adventure.cyoa"),
    )
    .expect("should find example file");
    let story = parse_story(&source).expect("should parse");

    // The example imports std/healing and std/combat — resolve from the repo root
    let repo_root = manifest_dir.join("..");
    let resolved = resolve_imports(&story, &repo_root.join("std"), &[repo_root.join("std")])
        .expect("imports should resolve");

    let bytecode = compile_story(&resolved).expect("should compile");
    assert!(!bytecode.events.is_empty());
    assert!(!bytecode.instructions.is_empty());
}

#[test]
fn test_compile_empty_story() {
    let source = r#"
story Empty:
"#;
    let story = parse_story(source).unwrap();
    let bytecode = compile_story(&story).expect("should compile");
    assert!(bytecode.events.is_empty());
    assert!(bytecode.stats.is_empty());
}

// ===== Multi-line string tests =====

#[test]
fn test_parse_multiline_quoted_string() {
    let source = r#"
story Test:
  stat hp = 0
  event start:
    "This is the first paragraph.
    This is the second paragraph.
    They are in the same string."
    choice "Continue":
      next end
  event end:
    "The end."
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[1] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    // The multi-line string should be a single TextContent item
    assert_eq!(ev.text.len(), 1);
    let segments = &ev.text[0].segments;
    // Should be a single literal segment containing newlines
    assert_eq!(segments.len(), 1);
    match &segments[0] {
        TextSegment::Literal(s) => {
            assert!(s.contains('\n'), "multi-line string should contain newline");
            assert!(s.contains("first paragraph"));
            assert!(s.contains("second paragraph"));
        }
        other => panic!("expected Literal, got {:?}", other),
    }
}

#[test]
fn test_parse_multiline_string_with_template() {
    let source = r#"
story Test:
  stat gold = 42
  event start:
    "You see {{gold}} gold
    pieces scattered on the floor."
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[1] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    // Should be a single text segment with a stat reference and literal
    assert_eq!(ev.text.len(), 1);
    let segments = &ev.text[0].segments;
    // Should contain a stat ref for "gold" and literal text
    let has_stat_ref = segments
        .iter()
        .any(|s| matches!(s, TextSegment::StatRef(ref name) if name == "gold"));
    assert!(has_stat_ref, "should contain {{gold}} stat ref");
}

#[test]
fn test_parse_multiline_string_with_comment_inside() {
    let source = r#"
story Test:
  event start:
    "Hello world
    # this is not a comment, it is inside the string
    Goodbye"
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[0] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert_eq!(ev.text.len(), 1);
    let segments = &ev.text[0].segments;
    match &segments[0] {
        TextSegment::Literal(s) => {
            assert!(s.contains("# this is not a comment"));
            assert!(s.contains("Hello world"));
            assert!(s.contains("Goodbye"));
        }
        other => panic!("expected Literal, got {:?}", other),
    }
}

#[test]
fn test_parse_single_line_string_still_works() {
    let source = r#"
story Test:
  event start:
    "Single line string."
    choice "Go":
      next end
  event end:
    "End."
"#;
    let story = parse_story(source).unwrap();
    let ev = match &story.items[0] {
        StoryItem::EventDef(e) => e,
        _ => panic!("expected EventDef"),
    };
    assert_eq!(ev.text.len(), 1);
}

#[test]
fn test_compile_multiline_string() {
    let source = r#"
story TestStory:
  stat hp = 0
  event start:
    "Line one.
    Line two.
    Line three."
    choice "Continue":
      next end
  event end:
    "The end."
"#;
    let story = parse_story(source).unwrap();
    let bytecode = compile_story(&story).expect("should compile");
    assert!(!bytecode.events.is_empty());
    assert!(!bytecode.instructions.is_empty());
}
