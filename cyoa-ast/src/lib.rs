//! AST types for the CYOA DSL.
//!
//! This is the foundational crate — it defines the data structures that
//! represent a parsed CYOA story before compilation to bytecode.
//! No dependencies on other cyoa crates.

/// A complete story file.
#[derive(Debug, Clone, PartialEq)]
pub struct Story {
    pub name: String,
    pub imports: Vec<Import>,
    pub tags: Vec<String>,
    pub items: Vec<StoryItem>,
}

/// An import statement (Go-style path).
#[derive(Debug, Clone, PartialEq)]
pub struct Import {
    pub path: String,
    pub alias: Option<String>,
}

/// Top-level item in a story file (after merge, imports are resolved).
#[derive(Debug, Clone, PartialEq)]
pub enum StoryItem {
    Import(Import),
    StatDef(StatDef),
    FlagDef(FlagDef),
    EffectDef(EffectDef),
    EventDef(EventDef),
}

/// Stat definition: `stat hp = 50`
#[derive(Debug, Clone, PartialEq)]
pub struct StatDef {
    pub name: String,
    pub default: i64,
}

/// Flag definition: `flag visited_cave`
#[derive(Debug, Clone, PartialEq)]
pub struct FlagDef {
    pub name: String,
    pub default: bool,
}

/// Effect block: reusable consequences.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectDef {
    pub name: String,
    pub body: Vec<EffectStep>,
}

/// A step within an effect or choice body.
#[derive(Debug, Clone, PartialEq)]
pub enum EffectStep {
    ChangeStat { stat: String, delta: i64 },
    SetFlag { flag: String, value: bool },
    AddTag { tag: String },
    Text(TextContent),
}

/// Text content that may contain template segments.
#[derive(Debug, Clone, PartialEq)]
pub struct TextContent {
    pub segments: Vec<TextSegment>,
}

/// A segment of text — either literal or a stat reference.
#[derive(Debug, Clone, PartialEq)]
pub enum TextSegment {
    Literal(String),
    StatRef(String),
}

/// An event (story node).
#[derive(Debug, Clone, PartialEq)]
pub struct EventDef {
    pub id: String,
    pub requires: Option<ConditionExpr>,
    pub tags: Vec<String>,
    /// Inline effect steps executed when the event is entered.
    pub body: Vec<EffectStep>,
    pub text: Vec<TextContent>,
    pub choices: Vec<ChoiceDef>,
}

/// A player-selectable choice.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceDef {
    pub text: TextContent,
    pub requires: Option<ConditionExpr>,
    pub steps: Vec<EffectStep>,
    pub uses: Vec<String>,    // references to effect block names
    pub next: Option<String>, // goto event id
}

/// Boolean condition expression (AND/OR/NOT/threshold).
#[derive(Debug, Clone, PartialEq)]
pub enum ConditionExpr {
    /// A bare flag reference: `visited_cave`
    Flag(String),
    /// A stat comparison: `courage >= 5`
    StatCompare {
        stat: String,
        op: CompareOp,
        value: i64,
    },
    /// Logical AND
    And(Box<ConditionExpr>, Box<ConditionExpr>),
    /// Logical OR
    Or(Box<ConditionExpr>, Box<ConditionExpr>),
    /// Logical NOT
    Not(Box<ConditionExpr>),
}

/// Comparison operators for stat conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareOp {
    Gte, // >=
    Lte, // <=
    Gt,  // >
    Lt,  // <
    Eq,  // ==
    Ne,  // !=
}
