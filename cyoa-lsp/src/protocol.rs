//! Minimal LSP/JSON-RPC protocol types.
//!
//! We only need a subset of the LSP protocol — enough for diagnostics,
//! hover, and completion. Using `lsp-types` would pull in the full spec;
//! these lightweight types are sufficient for the current feature set.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// The JSON-RPC message envelope.
#[derive(Debug, Clone, Deserialize)]
pub struct RawMessage {
    #[serde(default)]
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Num(i64),
    Str(String),
}

/// An LSP URI — we store the `file://` form as a string.
#[derive(Debug, Clone, Deserialize)]
struct TextDocumentUri {
    uri: String,
}

/// Parameters for `textDocument/didOpen` / `textDocument/didChange`.
#[derive(Debug, Clone, Deserialize)]
struct DidOpenParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentItem,
}

/// A text document item with its URI and full text content.
#[derive(Debug, Clone, Deserialize)]
struct TextDocumentItem {
    uri: String,
    text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DidOpenTextDocumentParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
}

#[derive(Debug, Clone, Deserialize)]
struct TextDocumentIdentifier {
    uri: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DidChangeTextDocumentParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    #[serde(rename = "contentChanges")]
    content_changes: Vec<TextDocumentContentChangeEvent>,
}

#[derive(Debug, Clone, Deserialize)]
struct TextDocumentContentChangeEvent {
    text: Option<String>,
    #[serde(rename = "range")]
    _range: Option<lsp_range::Range>,
}

mod lsp_range {
    use serde::Deserialize;
    #[derive(Debug, Clone, Deserialize)]
    pub struct Range {
        pub start: Position,
        pub end: Position,
    }
    #[derive(Debug, Clone, Deserialize)]
    pub struct Position {
        pub line: u32,
        pub character: u32,
    }
}

/// Parameters for `textDocument/hover`.
#[derive(Debug, Clone, Deserialize)]
struct HoverParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    position: lsp_range::Position,
}

/// Parameters for `textDocument/completion`.
#[derive(Debug, Clone, Deserialize)]
struct CompletionParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    position: lsp_range::Position,
}

/// Parsed request from a raw JSON-RPC message.
#[derive(Debug)]
pub enum Request {
    Initialize,
    Shutdown,
    DidOpen {
        uri: String,
        text: String,
    },
    DidChange {
        uri: String,
        text: String,
    },
    DidClose {
        uri: String,
    },
    Hover {
        uri: String,
        line: u32,
        character: u32,
    },
    Completion {
        uri: String,
        line: u32,
        character: u32,
    },
}

impl Request {
    pub fn from_raw(msg: RawMessage) -> Option<Self> {
        let method = msg.method?;
        let params = msg.params?;

        match method.as_str() {
            "initialize" => Some(Request::Initialize),
            "shutdown" => Some(Request::Shutdown),
            "textDocument/didOpen" => {
                let p: DidOpenParams = serde_json::from_value(params).ok()?;
                Some(Request::DidOpen {
                    uri: p.text_document.uri,
                    text: p.text_document.text,
                })
            }
            "textDocument/didChange" => {
                let p: DidChangeTextDocumentParams = serde_json::from_value(params).ok()?;
                // Use the latest text content
                let text = p
                    .content_changes
                    .last()
                    .and_then(|c| c.text.clone())
                    .unwrap_or_default();
                Some(Request::DidChange {
                    uri: p.text_document.uri,
                    text,
                })
            }
            "textDocument/didClose" => {
                let p: DidOpenTextDocumentParams = serde_json::from_value(params).ok()?;
                Some(Request::DidClose {
                    uri: p.text_document.uri,
                })
            }
            "textDocument/hover" => {
                let p: HoverParams = serde_json::from_value(params).ok()?;
                Some(Request::Hover {
                    uri: p.text_document.uri,
                    line: p.position.line,
                    character: p.position.character,
                })
            }
            "textDocument/completion" => {
                let p: CompletionParams = serde_json::from_value(params).ok()?;
                Some(Request::Completion {
                    uri: p.text_document.uri,
                    line: p.position.line,
                    character: p.position.character,
                })
            }
            _ => None,
        }
    }
}

/// LSP diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u32)]
pub enum DiagnosticSeverity {
    Error = 1,
    Warning = 2,
    Information = 3,
    Hint = 4,
}

/// A diagnostic (error/warning) for a document.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    #[serde(rename = "severity", skip_serializing_if = "Option::is_none")]
    pub severity: Option<DiagnosticSeverity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// LSP Hover response.
#[derive(Debug, Clone, Serialize)]
pub struct Hover {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contents: Option<HoverContents>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum HoverContents {
    Markup(MarkupContent),
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkupContent {
    #[serde(rename = "kind")]
    pub kind: String, // "markdown" or "plaintext"
    pub value: String,
}

/// LSP Completion list.
#[derive(Debug, Clone, Serialize)]
pub struct CompletionList {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_incomplete: Option<bool>,
    pub items: Vec<CompletionItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletionItem {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<CompletionItemKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[repr(u32)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
    Folder = 19,
    EnumMember = 20,
    Constant = 21,
    Struct = 22,
    Event = 23,
    Operator = 24,
    TypeParameter = 25,
}

/// Response to an LSP request (or a notification to publish diagnostics).
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum Response {
    /// JSON-RPC response to a request.
    Response {
        jsonrpc: String,
        id: Option<RequestId>,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ResponseError>,
    },
    /// `textDocument/publishDiagnostics` notification (server → client).
    PublishDiagnostics {
        method: String,
        #[serde(rename = "params")]
        params: PublishDiagnosticsParams,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishDiagnosticsParams {
    #[serde(rename = "uri")]
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// LSP ServerCapabilities — sent in the `initialize` response.
#[derive(Debug, Clone, Serialize)]
pub struct ServerCapabilities {
    #[serde(rename = "textDocumentSync", skip_serializing_if = "Option::is_none")]
    pub text_document_sync: Option<TextDocumentSyncOptions>,
    #[serde(rename = "completionProvider", skip_serializing_if = "Option::is_none")]
    pub completion_provider: Option<CompletionOptions>,
    #[serde(rename = "hoverProvider", skip_serializing_if = "Option::is_none")]
    pub hover_provider: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TextDocumentSyncOptions {
    #[serde(rename = "openClose")]
    pub open_close: bool,
    #[serde(rename = "change")]
    pub change: u32, // 1 = full
}

#[derive(Debug, Clone, Serialize)]
pub struct CompletionOptions {
    #[serde(rename = "resolveProvider", skip_serializing_if = "Option::is_none")]
    pub resolve_provider: Option<bool>,
    #[serde(rename = "triggerCharacters", skip_serializing_if = "Option::is_none")]
    pub trigger_characters: Option<Vec<String>>,
}
