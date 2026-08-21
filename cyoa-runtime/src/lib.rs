//! Runtime VM for CYOA stories.
//!
//! The VM is a register-based stack machine that interprets `.cyoa.bc`
//! bytecode. It owns `PlayerState` and `StoryCursor`, and returns zero-copy
//! `&str` into the loaded bytecode string table where possible.

use cyoa_bytecode::{Bytecode, Opcode};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Global game state — one per story/engine instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerState {
    pub stats: BTreeMap<String, i64>,
    pub flags: BTreeSet<String>,
    pub tags: BTreeSet<String>,
}

/// A single choice entry in the history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryEntry {
    pub event_id: String,
    pub choice_index: i32,
    pub choice_text: String,
}

/// Cursor tracking story position + history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryCursor {
    pub current_event: u32,
    pub choice_history: Vec<HistoryEntry>,
    /// Set to true when a terminal choice is made (choice with no `next`).
    pub complete: bool,
}

/// The CYOA engine — one per story.
///
/// Owns the bytecode. Text is returned via pointer+length into the
/// bytecode's string table for WASM zero-copy delivery.
pub struct Engine {
    bytecode: Bytecode,
    state: PlayerState,
    cursor: StoryCursor,
}

impl Engine {
    /// Create a new engine from bytecode.
    /// Initializes player state from the bytecode's stat/flag declarations.
    pub fn new(bytecode: Bytecode) -> Self {
        let mut stats = BTreeMap::new();
        for (name_idx, default) in &bytecode.stats {
            let name = bytecode.string_at(*name_idx).to_string();
            stats.insert(name, *default);
        }

        let mut engine = Self {
            bytecode,
            state: PlayerState {
                stats,
                flags: BTreeSet::new(),
                tags: BTreeSet::new(),
            },
            cursor: StoryCursor {
                current_event: 0,
                choice_history: Vec::new(),
                complete: false,
            },
        };
        // Execute the start event's body (inline effects)
        engine.execute_current_event_body();
        engine
    }

    /// Execute the body (inline effects) of the current event.
    /// Returns text produced by the body's effect instructions.
    fn execute_current_event_body(&mut self) -> Vec<String> {
        let event_idx = self.cursor.current_event as usize;
        if let Some(event) = self.bytecode.events.get(event_idx) {
            self.execute_effect_range(event.body_start, event.body_len)
        } else {
            Vec::new()
        }
    }

    /// Get the current event's text paragraphs (templates rendered).
    pub fn current_event_text(&self) -> Vec<String> {
        let event = match self.bytecode.events.get(self.cursor.current_event as usize) {
            Some(e) => e,
            None => return vec![],
        };
        self.execute_text_range(event.text_start, event.text_len)
    }

    /// Execute a range of text instructions, collecting rendered text.
    fn execute_text_range(&self, start: u32, len: u32) -> Vec<String> {
        let mut texts = Vec::new();
        let instr_len = Opcode::instr_byte_len();
        let start_idx = start as usize / instr_len;
        let end_idx = (start as usize + len as usize) / instr_len;

        for i in start_idx..end_idx {
            let Some(instr) = self.bytecode.instruction_at(i) else {
                break;
            };
            match instr.opcode {
                Opcode::GetText => {
                    let s = self.bytecode.string_at(instr.operand_a);
                    texts.push(s.to_string());
                }
                Opcode::RenderTemplate => {
                    let template = self.bytecode.string_at(instr.operand_a);
                    let rendered = self.render_template(template);
                    texts.push(rendered);
                }
                Opcode::Return => break,
                _ => {}
            }
        }
        texts
    }

    /// Get the current event's available choices (templates rendered).
    /// Choices with unmet prerequisites are excluded.
    pub fn current_choices(&self) -> Vec<String> {
        let event = match self.bytecode.events.get(self.cursor.current_event as usize) {
            Some(e) => e,
            None => return vec![],
        };

        let mut result = Vec::new();
        for i in 0..event.choice_len {
            let choice_idx = (event.choice_start + i) as usize;
            let Some(choice) = self.bytecode.choices.get(choice_idx) else {
                continue;
            };

            // Check prerequisites
            if choice.requires != 0 {
                let cond_str = self.bytecode.string_at(choice.requires);
                if !self.evaluate_condition(cond_str) {
                    continue;
                }
            }

            let raw_text = self.bytecode.string_at(choice.text);
            let rendered = self.render_template(raw_text);
            result.push(rendered);
        }
        result
    }

    /// Make a choice at the given index.
    /// Executes the choice's inline effects, applies referenced effects,
    /// records history, and advances to the next event.
    /// Returns text produced by executed effects (for display to the player).
    pub fn make_choice(&mut self, choice_index: i32) -> Vec<String> {
        let mut effect_texts = Vec::new();
        let event_idx = self.cursor.current_event as usize;
        let Some(event) = self.bytecode.events.get(event_idx) else {
            return effect_texts;
        };

        // Collect accessible choices (filtering by prerequisites)
        let mut accessible: Vec<usize> = Vec::new();
        for i in 0..event.choice_len {
            let choice_idx = (event.choice_start + i) as usize;
            let Some(choice) = self.bytecode.choices.get(choice_idx) else {
                continue;
            };

            if choice.requires != 0 {
                let cond_str = self.bytecode.string_at(choice.requires);
                if !self.evaluate_condition(cond_str) {
                    continue;
                }
            }
            accessible.push(choice_idx);
        }

        let Some(&actual_idx) = accessible.get(choice_index as usize) else {
            return effect_texts;
        };
        let Some(choice) = self.bytecode.choices.get(actual_idx).cloned() else {
            return effect_texts;
        };

        // Record history
        let choice_text = self.render_template(self.bytecode.string_at(choice.text));
        let event_id = self.bytecode.string_at(event.id).to_string();
        self.cursor.choice_history.push(HistoryEntry {
            event_id,
            choice_index,
            choice_text,
        });

        // Execute inline effect steps (collect text)
        effect_texts.extend(self.execute_effect_range(choice.step_start, choice.step_len));

        // Apply referenced effects (`uses`)
        if choice.use_len > 0 {
            let uses_str = self.bytecode.string_at(choice.use_start).to_string();
            for name in uses_str.split(',') {
                let name = name.trim();
                if name.is_empty() {
                    continue;
                }
                // Find effect and extract its ranges (clone to avoid borrow)
                let effect_range = self
                    .bytecode
                    .effects
                    .iter()
                    .find(|e| self.bytecode.string_at(e.name) == name)
                    .map(|e| (e.inst_start, e.inst_len));
                if let Some((start, len)) = effect_range {
                    effect_texts.extend(self.execute_effect_range(start, len));
                }
            }
        }

        // Advance to next event
        if choice.next != 0 {
            let next_name = self.bytecode.string_at(choice.next).to_string();
            if let Some(event_idx) = self.find_event_by_name(&next_name) {
                self.cursor.current_event = event_idx as u32;
                // Execute the new event's body (inline effects like set flag)
                effect_texts.extend(self.execute_current_event_body());
            }
        } else {
            // Terminal choice — story is complete
            self.cursor.complete = true;
        }

        effect_texts
    }

    /// Returns true if the story has ended (a terminal choice was made).
    /// A terminal choice is one that has no `next` event specified.
    pub fn is_story_complete(&self) -> bool {
        self.cursor.complete
    }

    /// Execute a range of effect instructions (stat/flag/tag changes).
    /// Collects and returns text produced by GetText/RenderTemplate instructions.
    fn execute_effect_range(&mut self, start: u32, len: u32) -> Vec<String> {
        let mut texts = Vec::new();
        let instr_len = Opcode::instr_byte_len();
        let start_idx = start as usize / instr_len;
        let end_idx = (start as usize + len as usize) / instr_len;

        for i in start_idx..end_idx {
            let Some(instr) = self.bytecode.instruction_at(i) else {
                break;
            };
            match instr.opcode {
                Opcode::GetText => {
                    let s = self.bytecode.string_at(instr.operand_a);
                    texts.push(s.to_string());
                }
                Opcode::RenderTemplate => {
                    let template = self.bytecode.string_at(instr.operand_a);
                    let rendered = self.render_template(template);
                    texts.push(rendered);
                }
                Opcode::ChangeStat => {
                    let stat_name = self.bytecode.string_at(instr.operand_a);
                    // operand_b is u32; cast through i32 to get sign extension
                    let delta = (instr.operand_b as i32) as i64;
                    let current = self.state.stats.get(stat_name).copied().unwrap_or(0);
                    self.state
                        .stats
                        .insert(stat_name.to_string(), current + delta);
                }
                Opcode::SetFlag => {
                    let flag_name = self.bytecode.string_at(instr.operand_a);
                    self.state.flags.insert(flag_name.to_string());
                }
                Opcode::ClearFlag => {
                    let flag_name = self.bytecode.string_at(instr.operand_a);
                    self.state.flags.remove(flag_name);
                }
                Opcode::AddTag => {
                    let tag_name = self.bytecode.string_at(instr.operand_a);
                    self.state.tags.insert(tag_name.to_string());
                }
                Opcode::Return => break,
                _ => {}
            }
        }
        texts
    }

    fn find_event_by_name(&self, name: &str) -> Option<usize> {
        self.bytecode
            .event_index
            .iter()
            .find(|(idx, _)| self.bytecode.string_at(*idx) == name)
            .map(|(_, event_idx)| *event_idx as usize)
    }

    /// Render a template string, replacing `{{stat}}` with current values.
    fn render_template(&self, template: &str) -> String {
        let mut result = String::new();
        let mut rest = template;

        while let Some(start) = rest.find("{{") {
            result.push_str(&rest[..start]);
            rest = &rest[start + 2..];

            if let Some(end) = rest.find("}}") {
                let stat_name = rest[..end].trim();
                if stat_name.is_empty() {
                    result.push_str("{{}}");
                } else {
                    let value = self.state.stats.get(stat_name).copied().unwrap_or(0);
                    result.push_str(&value.to_string());
                }
                rest = &rest[end + 2..];
            } else {
                // No closing — treat as literal
                result.push_str("{{");
                result.push_str(rest);
                break;
            }
        }
        result.push_str(rest);
        result
    }

    /// Evaluate a condition expression string against current state.
    fn evaluate_condition(&self, expr: &str) -> bool {
        let mut evaluator = CondEvaluator {
            input: expr.trim(),
            pos: 0,
            stats: &self.state.stats,
            flags: &self.state.flags,
        };
        evaluator.parse_or()
    }

    /// Get the choice history.
    pub fn history(&self) -> &[HistoryEntry] {
        &self.cursor.choice_history
    }

    /// Check if an event is reachable by name (exists in event index).
    pub fn can_access_event(&self, id: &str) -> bool {
        self.bytecode
            .event_index
            .iter()
            .any(|(name_idx, _)| self.bytecode.string_at(*name_idx) == id)
    }

    /// Get stats as a JSON object (name → current value).
    pub fn stats_json(&self) -> String {
        serde_json::to_string(&self.state.stats).unwrap_or_else(|_| "{}".to_string())
    }

    /// Set state from a JSON string (for save/load).
    ///
    /// Accepts the comprehensive state JSON format produced by
    /// [`get_state_json`](Self::get_state_json), which includes stats,
    /// flags, tags, current_event, choice_history, and the `complete` flag.
    /// This enables full save/load between sessions — the engine resumes
    /// exactly where it was saved, including story position and choice history.
    pub fn set_state_json(&mut self, json: &str) -> Result<(), serde_json::Error> {
        let value: serde_json::Value = serde_json::from_str(json)?;

        if let Some(stats) = value.get("stats") {
            self.state.stats = serde_json::from_value(stats.clone())?;
        }
        if let Some(flags) = value.get("flags") {
            self.state.flags = serde_json::from_value(flags.clone())?;
        }
        if let Some(tags) = value.get("tags") {
            self.state.tags = serde_json::from_value(tags.clone())?;
        }
        if let Some(current_event) = value.get("current_event") {
            self.cursor.current_event = current_event.as_u64().unwrap_or(0) as u32;
        }
        if let Some(history) = value.get("choice_history") {
            self.cursor.choice_history = serde_json::from_value(history.clone())?;
        }
        if let Some(complete) = value.get("complete") {
            self.cursor.complete = complete.as_bool().unwrap_or(false);
        }

        Ok(())
    }

    /// Get a stat value.
    pub fn get_stat(&self, name: &str) -> i64 {
        self.state.stats.get(name).copied().unwrap_or(0)
    }

    /// List all flags currently set.
    pub fn list_flags(&self) -> Vec<String> {
        self.state.flags.iter().cloned().collect()
    }

    /// List all tags currently set.
    pub fn list_tags(&self) -> Vec<String> {
        self.state.tags.iter().cloned().collect()
    }

    /// List all story-level tags declared in the story (not runtime-applied).
    pub fn list_story_tags(&self) -> Vec<String> {
        self.bytecode
            .story_tags
            .iter()
            .map(|idx| self.bytecode.string_at(*idx).to_string())
            .collect()
    }

    /// Get the current event's internal ID (name).
    pub fn current_event_id(&self) -> String {
        let event_idx = self.cursor.current_event as usize;
        self.bytecode
            .events
            .get(event_idx)
            .map(|e| self.bytecode.string_at(e.id).to_string())
            .unwrap_or_default()
    }

    /// List all event IDs in the story (useful for quest logs).
    pub fn available_events(&self) -> Vec<String> {
        self.bytecode
            .event_index
            .iter()
            .map(|(idx, _)| self.bytecode.string_at(*idx).to_string())
            .collect()
    }

    /// Get serialized state as a JSON string (for save/load).
    ///
    /// The JSON includes stats, flags, runtime tags, current event position,
    /// choice history, and the story-complete flag — enabling full save/load
    /// between sessions. The engine can be restored to the exact same state
    /// using [`set_state_json`](Self::set_state_json).
    ///
    /// ```json
    /// {
    ///   "stats": { "hp": 50, "gold": 10 },
    ///   "flags": ["visited_cave"],
    ///   "tags": ["combat"],
    ///   "current_event": 3,
    ///   "choice_history": [
    ///     {"eventId": "start", "choiceIndex": 0, "choiceText": "Enter"}
    ///   ],
    ///   "complete": false
    /// }
    /// ```
    ///
    /// Note: story-level tags are static metadata not included here — use
    /// [`list_story_tags`](Self::list_story_tags) to query them.
    pub fn get_state_json(&self) -> String {
        serde_json::json!({
            "stats": &self.state.stats,
            "flags": &self.state.flags,
            "tags": &self.state.tags,
            "current_event": self.cursor.current_event,
            "choice_history": &self.cursor.choice_history,
            "complete": self.cursor.complete,
        })
        .to_string()
    }
}

/// Metadata about a registered story, extracted from bytecode at registration time.
#[derive(Debug, Clone, Serialize)]
pub struct StoryMetadata {
    pub name: String,
    pub tags: Vec<String>,
}

/// A registry of compiled stories with tag-based filtering.
///
/// Game engines (especially random event systems) use this to discover
/// and filter stories by tags before creating an `Engine` to play them.
pub struct StoryCatalog {
    entries: Vec<(StoryMetadata, Bytecode)>,
}

impl StoryCatalog {
    /// Create an empty catalog.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a compiled story. Extracts story name and tags from the bytecode.
    pub fn register(&mut self, bytecode: Bytecode) {
        let name = bytecode.string_at(bytecode.story_name).to_string();
        let tags: Vec<String> = bytecode
            .story_tags
            .iter()
            .map(|idx| bytecode.string_at(*idx).to_string())
            .collect();
        self.entries.push((StoryMetadata { name, tags }, bytecode));
    }

    /// Get metadata for all registered stories.
    pub fn list_stories(&self) -> Vec<StoryMetadata> {
        self.entries.iter().map(|(meta, _)| meta.clone()).collect()
    }

    /// Find all stories that have a specific tag.
    pub fn stories_with_tag(&self, tag: &str) -> Vec<&StoryMetadata> {
        self.entries
            .iter()
            .filter(|(meta, _)| meta.tags.iter().any(|t| t == tag))
            .map(|(meta, _)| meta)
            .collect()
    }

    /// Find all stories that have ALL of the specified tags.
    pub fn stories_with_all_tags(&self, tags: &[&str]) -> Vec<&StoryMetadata> {
        self.entries
            .iter()
            .filter(|(meta, _)| tags.iter().all(|t| meta.tags.iter().any(|mt| mt == *t)))
            .map(|(meta, _)| meta)
            .collect()
    }

    /// Find all stories that have ANY of the specified tags.
    pub fn stories_with_any_tags(&self, tags: &[&str]) -> Vec<&StoryMetadata> {
        self.entries
            .iter()
            .filter(|(meta, _)| tags.iter().any(|t| meta.tags.iter().any(|mt| mt == *t)))
            .map(|(meta, _)| meta)
            .collect()
    }

    /// Create an `Engine` for the story at the given index.
    /// The bytecode is cloned for the engine.
    pub fn create_engine(&self, index: usize) -> Option<Engine> {
        self.entries
            .get(index)
            .map(|(_, bc)| Engine::new(bc.clone()))
    }

    /// Find a story by name and create an `Engine` for it.
    pub fn create_engine_by_name(&self, name: &str) -> Option<Engine> {
        self.entries
            .iter()
            .find(|(meta, _)| meta.name == name)
            .map(|(_, bc)| Engine::new(bc.clone()))
    }

    /// Number of registered stories.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if catalog is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for StoryCatalog {
    fn default() -> Self {
        Self::new()
    }
}

/// Condition expression evaluator.
struct CondEvaluator<'a> {
    input: &'a str,
    pos: usize,
    stats: &'a BTreeMap<String, i64>,
    flags: &'a BTreeSet<String>,
}

impl<'a> CondEvaluator<'a> {
    fn skip_ws(&mut self) {
        while self.pos < self.input.len() && self.input[self.pos..].starts_with(char::is_whitespace)
        {
            self.pos += 1;
        }
    }

    fn try_match(&mut self, s: &str) -> bool {
        let rest = &self.input[self.pos..];
        if rest.starts_with(s)
            && (s.len() == rest.len() || !rest[s.len()..].chars().next().unwrap().is_alphanumeric())
        {
            self.pos += s.len();
            true
        } else {
            false
        }
    }

    fn parse_or(&mut self) -> bool {
        let mut result = self.parse_and();
        self.skip_ws();
        while self.try_match("OR") {
            let rhs = self.parse_and();
            result = result || rhs;
            self.skip_ws();
        }
        result
    }

    fn parse_and(&mut self) -> bool {
        let mut result = self.parse_not();
        self.skip_ws();
        while self.try_match("AND") {
            let rhs = self.parse_not();
            result = result && rhs;
            self.skip_ws();
        }
        result
    }

    fn parse_not(&mut self) -> bool {
        self.skip_ws();
        if self.try_match("NOT") {
            self.skip_ws();
            let inner = self.parse_not();
            !inner
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> bool {
        self.skip_ws();
        // Use direct char matching for parens (not try_match, which enforces word boundaries)
        if self.pos < self.input.len() && self.input[self.pos..].starts_with('(') {
            self.pos += 1;
            let result = self.parse_or();
            self.skip_ws();
            if self.pos >= self.input.len() || !self.input[self.pos..].starts_with(')') {
                return false;
            }
            self.pos += 1;
            result
        } else {
            self.parse_comparison()
        }
    }

    fn parse_comparison(&mut self) -> bool {
        self.skip_ws();

        // Parse identifier
        let start = self.pos;
        while self.pos < self.input.len() {
            let c = self.input[self.pos..].chars().next().unwrap();
            if c.is_alphanumeric() || c == '_' || c == '$' {
                self.pos += c.len_utf8();
            } else {
                break;
            }
        }
        let name = &self.input[start..self.pos];

        if name.is_empty() {
            return false;
        }

        self.skip_ws();

        // Check for comparison operator
        let rest = &self.input[self.pos..];
        let op_len = match_operator_len(rest);
        if op_len > 0 {
            let op_str = &rest[..op_len];
            self.pos += op_len;
            self.skip_ws();

            // Parse number (may be negative)
            let num_start = self.pos;
            if self.input[self.pos..].starts_with('-') {
                self.pos += 1;
            }
            while self.pos < self.input.len() {
                let c = self.input[self.pos..].chars().next().unwrap();
                if c.is_ascii_digit() {
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let num_str = &self.input[num_start..self.pos];
            let value: i64 = num_str.trim().parse().unwrap_or(0);

            let stat_val = self.stats.get(name).copied().unwrap_or(0);
            compare_values(stat_val, op_str, value)
        } else {
            // Bare flag check
            self.flags.contains(name)
        }
    }
}

fn match_operator_len(s: &str) -> usize {
    for op in &["<=", ">=", "==", "!=", "<", ">"] {
        if s.starts_with(op) {
            return op.len();
        }
    }
    0
}

fn compare_values(lhs: i64, op: &str, rhs: i64) -> bool {
    match op {
        "<=" => lhs <= rhs,
        ">=" => lhs >= rhs,
        "==" => lhs == rhs,
        "!=" => lhs != rhs,
        "<" => lhs < rhs,
        ">" => lhs > rhs,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
