//! AST → Bytecode code generator.
//!
//! Converts a merged `Story` AST into `cyoa_bytecode::Bytecode`.

use cyoa_ast::*;
use cyoa_bytecode::*;
use std::collections::BTreeMap;

/// Error during bytecode generation.
#[derive(Debug)]
pub enum CodegenError {
    UndefinedEffect(String),
    UndefinedEvent(String),
    UndefinedStat(String),
    UndefinedFlag(String),
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodegenError::UndefinedEffect(s) => {
                write!(f, "undefined effect referenced: {}", s)
            }
            CodegenError::UndefinedEvent(s) => {
                write!(f, "undefined event referenced: {}", s)
            }
            CodegenError::UndefinedStat(s) => {
                write!(f, "undefined stat referenced: {}", s)
            }
            CodegenError::UndefinedFlag(s) => {
                write!(f, "undefined flag referenced: {}", s)
            }
        }
    }
}

impl std::error::Error for CodegenError {}

/// A reference validation error with source position information.
#[derive(Debug, Clone)]
pub struct ReferenceError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

/// Validate that all references in the story point to defined symbols.
///
/// Checks:
/// - `next` targets in choices reference defined events
/// - `uses` references in choices reference defined effects
///
/// Note: stat and flag references are NOT validated at compile time. The
/// runtime treats undeclared stats as 0 and undeclared flags as false,
/// so stories are not required to pre-declare every stat/flag they use.
/// This is especially important for std library effects (e.g. `std/combat`
/// uses `courage`) that may reference stats the importing story doesn't
/// declare.
///
/// Returns a list of errors with line/col positions extracted from the
/// source text. This is used by the LSP to provide real-time diagnostics
/// and by `cyoa validate` to report errors.
pub fn validate_references(story: &Story, source: &str) -> Vec<ReferenceError> {
    let (_, _, defined_events, defined_effects) = collect_defined_symbols(story);
    let lines: Vec<&str> = source.lines().collect();

    let mut errors: Vec<ReferenceError> = Vec::new();

    for item in &story.items {
        if let StoryItem::EventDef(ev) = item {
            check_event_refs(ev, &defined_events, &defined_effects, &lines, &mut errors);
        }
    }

    errors
}

/// Check references within a single event and its choices, reporting
/// undefined `next` (event) and `uses` (effect) references.
fn check_event_refs(
    ev: &EventDef,
    defined_events: &std::collections::HashSet<&str>,
    defined_effects: &std::collections::HashSet<&str>,
    lines: &[&str],
    errors: &mut Vec<ReferenceError>,
) {
    for choice in &ev.choices {
        check_choice_refs(choice, defined_events, defined_effects, lines, errors);
    }
}

/// Collect the names of all defined symbols (stats, flags, events, effects).
fn collect_defined_symbols(
    story: &Story,
) -> (
    std::collections::HashSet<&str>,
    std::collections::HashSet<&str>,
    std::collections::HashSet<&str>,
    std::collections::HashSet<&str>,
) {
    story.items.iter().fold(
        (
            std::collections::HashSet::new(),
            std::collections::HashSet::new(),
            std::collections::HashSet::new(),
            std::collections::HashSet::new(),
        ),
        |(mut stats, mut flags, mut events, mut effects), item| {
            match item {
                StoryItem::StatDef(s) => {
                    stats.insert(s.name.as_str());
                }
                StoryItem::FlagDef(f) => {
                    flags.insert(f.name.as_str());
                }
                StoryItem::EventDef(e) => {
                    events.insert(e.id.as_str());
                }
                StoryItem::EffectDef(e) => {
                    effects.insert(e.name.as_str());
                }
                _ => {}
            }
            (stats, flags, events, effects)
        },
    )
}

/// Check references within a single choice, reporting undefined
/// `next` (event) and `uses` (effect) references.
fn check_choice_refs(
    choice: &ChoiceDef,
    defined_events: &std::collections::HashSet<&str>,
    defined_effects: &std::collections::HashSet<&str>,
    lines: &[&str],
    errors: &mut Vec<ReferenceError>,
) {
    // Choice next → event reference
    if let Some(target) = &choice.next {
        if !defined_events.contains(target.as_str()) {
            if let Some((line, col)) = find_text_position(lines, target) {
                errors.push(ReferenceError {
                    message: format!(
                        "undefined event '{}': referenced in choice 'next' but not defined",
                        target
                    ),
                    line,
                    col,
                });
            }
        }
    }
    // Choice uses → effect references
    for eff in &choice.uses {
        if !defined_effects.contains(eff.as_str()) {
            if let Some((line, col)) = find_text_position(lines, eff) {
                errors.push(ReferenceError {
                    message: format!(
                        "undefined effect '{}': referenced in 'uses' but not defined",
                        eff
                    ),
                    line,
                    col,
                });
            }
        }
    }
}

/// Find the first word-boundary occurrence of `needle` in the source lines
/// and return its (line, col) position (1-based, matching ParseError conventions).
///
/// Uses word-boundary matching to avoid false positives (e.g. matching "hp"
/// inside "chap" or "courage" inside "encourage"). Skips comment lines (where
/// the first non-whitespace character is `#`) so that errors point at actual
/// references, not at mentions in comments.
fn find_text_position(lines: &[&str], needle: &str) -> Option<(usize, usize)> {
    lines.iter().enumerate().find_map(|(line_idx, line)| {
        if is_comment_line(line) {
            return None;
        }
        find_word_in_line(line, needle).map(|col| (line_idx + 1, col + 1))
    })
}

/// Check if a line is a comment (first non-whitespace character is `#`).
fn is_comment_line(line: &str) -> bool {
    line.bytes()
        .find(|b| !b.is_ascii_whitespace())
        .is_some_and(|b| b == b'#')
}

/// Find the first word-boundary match of `word` in a single line.
/// Returns the byte offset of the match.
fn find_word_in_line(line: &str, word: &str) -> Option<usize> {
    if word.is_empty() {
        return None;
    }
    let bytes = line.as_bytes();
    let word_bytes = word.as_bytes();
    let mut start = 0;
    while start + word_bytes.len() <= bytes.len() {
        if let Some(rel_pos) = line[start..].find(word) {
            let abs_pos = start + rel_pos;
            // Check word boundary before
            let before_ok = abs_pos == 0 || !is_ident_char(bytes[abs_pos - 1]);
            // Check word boundary after
            let after_pos = abs_pos + word.len();
            let after_ok = after_pos >= bytes.len() || !is_ident_char(bytes[after_pos]);
            if before_ok && after_ok {
                return Some(abs_pos);
            }
            start = abs_pos + word_bytes.len();
        } else {
            break;
        }
    }
    None
}

/// Check if a byte is a valid identifier character.
fn is_ident_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'$'
}

/// Generate bytecode from a merged `Story` AST.
pub fn compile_story(story: &Story) -> Result<Bytecode, CodegenError> {
    let mut ctx = CodegenContext::new();
    ctx.compile(story)?;
    Ok(ctx.finish())
}

struct CodegenContext {
    /// String table: index → string
    strings: Vec<String>,
    /// String → index lookup
    string_index: BTreeMap<String, u32>,
    /// Stat declarations: (string_idx, default_value)
    stats: Vec<(u32, i64)>,
    /// Flag declarations: string_idx
    flags: Vec<u32>,
    /// Effect blocks
    effects: Vec<EffectEntry>,
    /// Events array
    events: Vec<EventEntry>,
    /// Event name → index mapping
    event_index: Vec<(u32, u32)>,
    /// Choices
    choices: Vec<ChoiceEntry>,
    /// Instructions (serialized as bytes)
    instructions: Vec<u8>,
    /// Conditions data
    conditions: Vec<u8>,
    /// Story-level tag string table indices
    story_tags: Vec<u32>,
    /// String table index of the story name
    story_name: u32,
}

impl CodegenContext {
    fn new() -> Self {
        Self {
            strings: Vec::new(),
            string_index: BTreeMap::new(),
            stats: Vec::new(),
            flags: Vec::new(),
            effects: Vec::new(),
            events: Vec::new(),
            event_index: Vec::new(),
            choices: Vec::new(),
            instructions: Vec::new(),
            conditions: Vec::new(),
            story_tags: Vec::new(),
            story_name: 0,
        }
    }

    /// Intern a string into the string table. Returns its index.
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&idx) = self.string_index.get(s) {
            return idx;
        }
        let idx = self.strings.len() as u32;
        self.strings.push(s.to_string());
        self.string_index.insert(s.to_string(), idx);
        idx
    }

    fn compile(&mut self, story: &Story) -> Result<(), CodegenError> {
        // Intern story name and tags (metadata, not instructions)
        self.story_name = self.intern(&story.name);

        // Intern story-level tags
        for tag in &story.tags {
            let idx = self.intern(tag);
            self.story_tags.push(idx);
        }

        // First pass: intern all stat/flag names
        for item in &story.items {
            match item {
                StoryItem::StatDef(s) => {
                    let idx = self.intern(&s.name);
                    self.stats.push((idx, s.default));
                }
                StoryItem::FlagDef(f) => {
                    let idx = self.intern(&f.name);
                    self.flags.push(idx);
                }
                _ => {}
            }
        }

        // Second pass: compile effects
        for item in &story.items {
            if let StoryItem::EffectDef(eff) = item {
                self.compile_effect(eff)?;
            }
        }

        // Third pass: compile events
        for item in &story.items {
            if let StoryItem::EventDef(ev) = item {
                self.compile_event(ev)?;
            }
        }

        Ok(())
    }

    fn compile_effect(&mut self, eff: &EffectDef) -> Result<(), CodegenError> {
        let name_idx = self.intern(&eff.name);
        let inst_start = self.instructions.len();

        for step in &eff.body {
            self.compile_effect_step(step)?;
        }
        // Always end effects with a Return instruction
        self.emit_instruction(Opcode::Return, 0, 0);

        let inst_len = (self.instructions.len() - inst_start) as u32;
        self.effects.push(EffectEntry {
            name: name_idx,
            inst_start: inst_start as u32,
            inst_len,
        });
        Ok(())
    }

    fn compile_effect_step(&mut self, step: &EffectStep) -> Result<(), CodegenError> {
        match step {
            EffectStep::ChangeStat { stat, delta } => {
                let stat_idx = self.intern(stat);
                // Store signed delta as u32 via casting
                self.emit_instruction(Opcode::ChangeStat, stat_idx, *delta as u32);
            }
            EffectStep::SetFlag { flag, value } => {
                let flag_idx = self.intern(flag);
                if *value {
                    self.emit_instruction(Opcode::SetFlag, flag_idx, 0);
                } else {
                    self.emit_instruction(Opcode::ClearFlag, flag_idx, 0);
                }
            }
            EffectStep::AddTag { tag } => {
                let tag_idx = self.intern(tag);
                self.emit_instruction(Opcode::AddTag, tag_idx, 0);
            }
            EffectStep::RemoveTag { tag } => {
                let tag_idx = self.intern(tag);
                self.emit_instruction(Opcode::RemoveTag, tag_idx, 0);
            }
            EffectStep::Text(text) => {
                let idx = self.compile_text(text);
                if text
                    .segments
                    .iter()
                    .any(|s| matches!(s, TextSegment::StatRef(_)))
                {
                    self.emit_instruction(Opcode::RenderTemplate, idx, 0);
                } else {
                    self.emit_instruction(Opcode::GetText, idx, 0);
                }
            }
        }
        Ok(())
    }

    fn compile_text(&mut self, text: &TextContent) -> u32 {
        // Serialize the text as a single string with {{stat}} markers
        // The VM will parse these at runtime.
        let mut rendered = String::new();
        for seg in &text.segments {
            match seg {
                TextSegment::Literal(s) => rendered.push_str(s),
                TextSegment::StatRef(name) => {
                    rendered.push_str("{{");
                    rendered.push_str(name);
                    rendered.push_str("}}");
                }
            }
        }
        self.intern(&rendered)
    }

    fn compile_event(&mut self, ev: &EventDef) -> Result<(), CodegenError> {
        let id_idx = self.intern(&ev.id);

        // Compile event body (inline effects executed on entry)
        let body_inst_start = self.instructions.len();
        for step in &ev.body {
            self.compile_effect_step(step)?;
        }
        if !ev.body.is_empty() {
            self.emit_instruction(Opcode::Return, 0, 0);
        }
        let body_inst_len = (self.instructions.len() - body_inst_start) as u32;

        // Compile event text into instructions
        let text_inst_start = self.instructions.len();
        for text in &ev.text {
            let idx = self.compile_text(text);
            if text
                .segments
                .iter()
                .any(|s| matches!(s, TextSegment::StatRef(_)))
            {
                self.emit_instruction(Opcode::RenderTemplate, idx, 0);
            } else {
                self.emit_instruction(Opcode::GetText, idx, 0);
            }
        }
        let text_inst_len = (self.instructions.len() - text_inst_start) as u32;

        // Compile choices
        let choice_start = self.choices.len() as u32;
        for choice in &ev.choices {
            self.compile_choice(choice)?;
        }
        let choice_len = (self.choices.len() - choice_start as usize) as u32;

        // Intern tags
        let tag_start = self.intern_tags(&ev.tags);
        let tag_len = ev.tags.len() as u32;

        // Serialize condition (if any) to string table
        let requires = ev
            .requires
            .as_ref()
            .map(|c| self.intern(&condition_to_string(c)))
            .unwrap_or(0);

        // Add to event index
        self.event_index.push((id_idx, self.events.len() as u32));

        self.events.push(EventEntry {
            id: id_idx,
            body_start: body_inst_start as u32,
            body_len: body_inst_len,
            text_start: text_inst_start as u32,
            text_len: text_inst_len,
            choice_start,
            choice_len,
            requires,
            tag_start,
            tag_len,
        });

        Ok(())
    }

    fn intern_tags(&mut self, tags: &[String]) -> u32 {
        // For now, tag storage is simplified — we intern a joined string
        if tags.is_empty() {
            return 0;
        }
        let joined = tags.join(",");
        self.intern(&joined)
    }

    fn compile_choice(&mut self, choice: &ChoiceDef) -> Result<(), CodegenError> {
        let text_idx = self.compile_text(&choice.text);

        // Compile inline effect steps
        let step_start = self.instructions.len();
        for step in &choice.steps {
            self.compile_effect_step(step)?;
        }
        let step_len = (self.instructions.len() - step_start) as u32;

        // Intern `uses` references — for now, store as string refs
        let uses_joined = choice.uses.join(",");
        let use_start = self.intern(&uses_joined);
        let use_len = choice.uses.len() as u32;

        // Intern `next` target
        let next_idx = choice.next.as_ref().map(|n| self.intern(n)).unwrap_or(0);

        // Serialize condition (if any) to string table
        let requires = choice
            .requires
            .as_ref()
            .map(|c| self.intern(&condition_to_string(c)))
            .unwrap_or(0);

        self.choices.push(ChoiceEntry {
            text: text_idx,
            step_start: step_start as u32,
            step_len,
            use_start,
            use_len,
            next: next_idx,
            requires,
        });

        Ok(())
    }

    fn emit_instruction(&mut self, opcode: Opcode, a: u32, b: u32) {
        // Each instruction: 1 byte opcode + 4 bytes operand_a + 4 bytes operand_b = 9 bytes
        self.instructions.push(opcode as u8);
        self.instructions.extend_from_slice(&a.to_le_bytes());
        self.instructions.extend_from_slice(&b.to_le_bytes());
    }

    fn finish(self) -> Bytecode {
        let string_data: Vec<u8> = self
            .strings
            .iter()
            .flat_map(|s| s.as_bytes().iter().copied())
            .collect();
        let string_table: Vec<StringRef> = {
            let mut table = Vec::new();
            let mut offset = 0u32;
            for s in &self.strings {
                table.push(StringRef {
                    offset,
                    length: s.len() as u32,
                });
                offset += s.len() as u32;
            }
            table
        };

        Bytecode {
            header: BytecodeHeader {
                magic: MAGIC,
                version: VERSION,
                string_table_offset: 0,
                string_table_len: string_table.len() as u64,
                event_index_offset: 0,
                event_index_len: self.event_index.len() as u64,
                events_offset: 0,
                events_len: self.events.len() as u64,
                choices_offset: 0,
                choices_len: self.choices.len() as u64,
                effects_offset: 0,
                effects_len: self.effects.len() as u64,
                conditions_offset: 0,
                conditions_len: self.conditions.len() as u64,
            },
            string_data,
            string_table,
            event_index: self.event_index,
            events: self.events,
            choices: self.choices,
            effects: self.effects,
            conditions: self.conditions,
            stats: self.stats,
            flags: self.flags,
            story_name: self.story_name,
            story_tags: self.story_tags,
            instructions: self.instructions,
        }
    }
}

/// Serialize a ConditionExpr to a string representation for the string table.
/// The VM parses these at runtime.
fn condition_to_string(expr: &ConditionExpr) -> String {
    match expr {
        ConditionExpr::Flag(name) => name.clone(),
        ConditionExpr::StatCompare { stat, op, value } => {
            format!("{} {} {}", stat, op_to_string(*op), value)
        }
        ConditionExpr::And(a, b) => {
            format!("{} AND {}", condition_to_string(a), condition_to_string(b))
        }
        ConditionExpr::Or(a, b) => {
            format!("{} OR {}", condition_to_string(a), condition_to_string(b))
        }
        ConditionExpr::Not(a) => {
            format!("NOT {}", condition_to_string(a))
        }
    }
}

fn op_to_string(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Gte => ">=",
        CompareOp::Lte => "<=",
        CompareOp::Gt => ">",
        CompareOp::Lt => "<",
        CompareOp::Eq => "==",
        CompareOp::Ne => "!=",
    }
}
