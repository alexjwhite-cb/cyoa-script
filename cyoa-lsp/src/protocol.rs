//! Minimal LSP/JSON-RPC protocol types.
//!
//! We only need a subset of the LSP protocol — enough for diagnostics,
//! hover, and basic completion. These lightweight types are sufficient
//! for the current feature set.

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

// ===== Request parameter types =====

/// Parameters for `textDocument/definition` (go-to-definition).
#[derive(Debug, Clone, Deserialize)]
struct DefinitionParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    position: Position,
}

/// Parameters for `textDocument/onTypeFormatting`.
#[derive(Debug, Clone, Deserialize)]
struct OnTypeFormattingParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    position: Position,
    ch: String,
    #[serde(default)]
    options: serde_json::Value,
}

/// Parameters for `textDocument/semanticTokens` / `semanticTokens/full`.
#[derive(Debug, Clone, Deserialize)]
struct SemanticTokensParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
}

/// Parameters for `textDocument/semanticTokens/range`.
#[derive(Debug, Clone, Deserialize)]
struct SemanticTokensRangeParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    Num(i64),
    Str(String),
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

/// Parameters for `textDocument/didClose`.
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
    _range: Option<Range>,
}

/// Parameters for `textDocument/hover`.
#[derive(Debug, Clone, Deserialize)]
struct HoverParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    position: Position,
}

/// Parameters for `textDocument/completion`.
#[derive(Debug, Clone, Deserialize)]
struct CompletionParams {
    #[serde(rename = "textDocument")]
    text_document: TextDocumentIdentifier,
    position: Position,
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
    Definition {
        uri: String,
        line: u32,
        character: u32,
    },
    OnTypeFormatting {
        uri: String,
        line: u32,
        character: u32,
        ch: String,
        options: serde_json::Value,
    },
    SemanticTokens {
        uri: String,
    },
    SemanticTokensRange {
        uri: String,
        range: Range,
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
            "textDocument/definition" | "textDocument/declaration" => {
                let p: DefinitionParams = serde_json::from_value(params).ok()?;
                Some(Request::Definition {
                    uri: p.text_document.uri,
                    line: p.position.line,
                    character: p.position.character,
                })
            }
            "textDocument/onTypeFormatting" => {
                let p: OnTypeFormattingParams = serde_json::from_value(params).ok()?;
                Some(Request::OnTypeFormatting {
                    uri: p.text_document.uri,
                    line: p.position.line,
                    character: p.position.character,
                    ch: p.ch,
                    options: p.options,
                })
            }
            "textDocument/semanticTokens" | "textDocument/semanticTokens/full" => {
                let p: SemanticTokensParams = serde_json::from_value(params).ok()?;
                Some(Request::SemanticTokens {
                    uri: p.text_document.uri,
                })
            }
            "textDocument/semanticTokens/range" => {
                let p: SemanticTokensRangeParams = serde_json::from_value(params).ok()?;
                Some(Request::SemanticTokensRange {
                    uri: p.text_document.uri,
                    range: p.range,
                })
            }
            _ => None,
        }
    }
}

// ===== Core LSP types =====

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

/// A location inside a document.
#[derive(Debug, Clone, Serialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

/// A completion item in the completion list.
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
    Function = 3,
    Field = 5,
    Variable = 6,
    Class = 7,
    Keyword = 14,
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
    },
    /// `textDocument/publishDiagnostics` notification (server → client).
    PublishDiagnostics {
        method: String,
        #[serde(rename = "params")]
        params: PublishDiagnosticsParams,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct PublishDiagnosticsParams {
    #[serde(rename = "uri")]
    pub uri: String,
    pub diagnostics: Vec<Diagnostic>,
}
