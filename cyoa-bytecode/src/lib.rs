//! Binary bytecode format for CYOA stories.
//!
//! The `.cyoa.bc` file is a single flat binary designed for zero-copy
//! mmap loading. The string table is at the front, enabling the VM to
//! return `&str` pointers directly into loaded bytecode memory.

use serde::{Deserialize, Serialize};

/// Magic number: "CYOA" (0x43594F41)
pub const MAGIC: u32 = 0x43594F41;
pub const VERSION: u32 = 1;

/// Bytecode file header.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BytecodeHeader {
    pub magic: u32,
    pub version: u32,
    pub string_table_offset: u64,
    pub string_table_len: u64,
    pub event_index_offset: u64,
    pub event_index_len: u64,
    pub events_offset: u64,
    pub events_len: u64,
    pub choices_offset: u64,
    pub choices_len: u64,
    pub effects_offset: u64,
    pub effects_len: u64,
    pub conditions_offset: u64,
    pub conditions_len: u64,
}

/// A string table entry: (offset, length) into the string data section.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct StringRef {
    pub offset: u32,
    pub length: u32,
}

/// Event entry in the event array (fixed-size).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEntry {
    /// String table index for the event's internal name.
    pub id: u32,
    /// Range into instructions for the event's body (inline effects on entry).
    pub body_start: u32,
    pub body_len: u32,
    /// Range into text instructions for the event's prose paragraphs.
    pub text_start: u32,
    pub text_len: u32,
    /// Range into choices array for this event's choices.
    pub choice_start: u32,
    pub choice_len: u32,
    /// String table index for the condition (or 0 if none).
    pub requires: u32,
    /// Range into tags array.
    pub tag_start: u32,
    pub tag_len: u32,
}

/// Choice entry (fixed-size).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceEntry {
    /// String table index for choice text (may contain template segments).
    pub text: u32,
    /// Range into effect-steps array for inline effects.
    pub step_start: u32,
    pub step_len: u32,
    /// Range into `uses` array (references to effect blocks).
    pub use_start: u32,
    pub use_len: u32,
    /// String table index for the `next` target (or 0 if terminal).
    pub next: u32,
    /// String table index for condition (or 0 if none).
    pub requires: u32,
}

/// A single VM instruction within an instruction stream.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Instruction {
    pub opcode: Opcode,
    pub operand_a: u32,
    pub operand_b: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Opcode {
    GetText = 0,
    RenderTemplate = 1,
    GetChoice = 2,
    ApplyEffect = 3,
    ChangeStat = 4,
    SetFlag = 5,
    ClearFlag = 6,
    AddTag = 7,
    CheckCondition = 8,
    RecordHistory = 9,
    BranchIfTrue = 10,
    Goto = 11,
    Return = 12,
}

impl Opcode {
    pub fn from_u8(n: u8) -> Option<Self> {
        Some(match n {
            0 => Opcode::GetText,
            1 => Opcode::RenderTemplate,
            2 => Opcode::GetChoice,
            3 => Opcode::ApplyEffect,
            4 => Opcode::ChangeStat,
            5 => Opcode::SetFlag,
            6 => Opcode::ClearFlag,
            7 => Opcode::AddTag,
            8 => Opcode::CheckCondition,
            9 => Opcode::RecordHistory,
            10 => Opcode::BranchIfTrue,
            11 => Opcode::Goto,
            12 => Opcode::Return,
            _ => return None,
        })
    }

    pub fn instr_byte_len() -> usize {
        9 // 1 byte opcode + 4 bytes operand_a + 4 bytes operand_b
    }
}

/// A serialized bytecode module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bytecode {
    pub header: BytecodeHeader,
    /// All strings in the story — the VM reads &str directly from here.
    pub string_data: Vec<u8>,
    /// String table: index → (offset, length) into string_data.
    pub string_table: Vec<StringRef>,
    /// Event index: event name string_idx → event array index.
    pub event_index: Vec<(u32, u32)>,
    /// Events array.
    pub events: Vec<EventEntry>,
    /// Choices array.
    pub choices: Vec<ChoiceEntry>,
    /// Effect blocks: name → instruction range.
    pub effects: Vec<EffectEntry>,
    /// Conditions bytecode (prerequisite trees).
    pub conditions: Vec<u8>,
    /// Global stat declarations: (string_idx, default_value).
    pub stats: Vec<(u32, i64)>,
    /// Global flag declarations.
    pub flags: Vec<u32>,
    /// String table index of the story name.
    pub story_name: u32,
    /// Story-level tags: string table indices (one per tag).
    pub story_tags: Vec<u32>,
    /// Instruction stream (union of all events + effects).
    pub instructions: Vec<u8>,
}

/// Effect block entry: name → instruction range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectEntry {
    pub name: u32, // string table idx
    pub inst_start: u32,
    pub inst_len: u32,
}

impl Bytecode {
    /// Get a string by index into the string table.
    pub fn string_at(&self, idx: u32) -> &str {
        let sref = &self.string_table[idx as usize];
        let start = sref.offset as usize;
        let end = start + sref.length as usize;
        std::str::from_utf8(&self.string_data[start..end])
            .expect("bytecode string data is valid UTF-8")
    }

    /// Serialize to binary (postcard format).
    pub fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_stdvec(self)
    }

    /// Deserialize from binary.
    pub fn from_bytes(data: &[u8]) -> Result<Self, postcard::Error> {
        postcard::from_bytes::<Self>(data)
    }

    /// Read an instruction at the given instruction index.
    pub fn instruction_at(&self, idx: usize) -> Option<Instruction> {
        let base = idx * Opcode::instr_byte_len();
        if base + Opcode::instr_byte_len() > self.instructions.len() {
            return None;
        }
        let opcode_byte = self.instructions[base];
        let opcode = Opcode::from_u8(opcode_byte)?;
        let operand_a = u32::from_le_bytes(
            self.instructions[base + 1..base + 5]
                .try_into()
                .expect("slice len is correct"),
        );
        let operand_b = u32::from_le_bytes(
            self.instructions[base + 5..base + 9]
                .try_into()
                .expect("slice len is correct"),
        );
        Some(Instruction {
            opcode,
            operand_a,
            operand_b,
        })
    }

    /// Read a range of instructions as raw bytes (for effect/choice body execution).
    pub fn instruction_bytes(&self, start: usize, len: usize) -> &[u8] {
        let end = start + len * Opcode::instr_byte_len();
        &self.instructions[start..end.min(self.instructions.len())]
    }
}
