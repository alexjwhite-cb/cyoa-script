//! The CYOA Language Server core logic.
//!
//! Tracks open documents (URI → source text + parsed AST) and responds to
//! LSP requests: diagnostics on open/change, hover info, and basic completion.

use std::collections::{HashMap, HashSet};

use crate::diagnostics::{diagnostics_from_parse_error, format_error_message};
use crate::protocol::*;
use cyoa_ast::{EffectStep, Story, StoryItem, TextSegment};
use cyoa_compiler::parse_story;

/// Keyword set for semantic token classification.
const KEYWORDS: &[&str] = &[
    "story", "import", "stat", "flag", "effect", "event", "choice", "requires", "tags", "uses",
    "next", "text", "set", "add", "by", "to", "AND", "OR", "NOT", "true", "false", "as",
];

/// Token type indices — must match the semanticTokensOptions legend.
#[allow(dead_code)]
const TT_KEYWORD: u32 = 0;
#[allow(dead_code)]
const TT_FUNCTION: u32 = 1;
#[allow(dead_code)]
const TT_VARIABLE: u32 = 2;
const TT_STRING: u32 = 3;
const TT_COMMENT: u32 = 4;
const TT_TYPE: u32 = 5;
const TT_PARAMETER: u32 = 6;
const TT_NUMBER: u32 = 7;
const TT_OPERATOR: u32 = 8;

/// A semantic token: position + length + type index.
#[derive(Debug, Clone)]
struct SemanticToken {
    line: u32,
    start_char: u32,
    length: u32,
    token_type: u32,
}

pub struct Server {
    /// Open documents: URI → (source text, parsed story, parse error if any)
    documents: HashMap<String, DocumentState>,
}

struct DocumentState {
    text: String,
    story: Option<Story>,
    error: Option<cyoa_compiler::ParseError>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            documents: HashMap::new(),
        }
    }

    /// Handle an incoming LSP request/notification.
    /// Returns:
    ///   - `Some(Response::Response { ... })` for request replies
    ///   - `Some(Response::PublishDiagnostics { ... })` for diagnostic notifications
    ///   - `None` for notifications that produce no output
    pub fn handle(&mut self, msg: RawMessage) -> Vec<Response> {
        let Some(req) = Request::from_raw(msg.clone()) else {
            return vec![];
        };

        match req {
            Request::Initialize => self.handle_initialize(msg.id),
            Request::Shutdown => self.handle_shutdown(msg.id),
            Request::DidOpen { uri, text } => self.handle_did_open(uri, text),
            Request::DidChange { uri, text } => self.handle_did_change(uri, text),
            Request::DidClose { uri } => self.handle_did_close(uri),
            Request::Hover {
                uri,
                line,
                character,
            } => self.handle_hover(&uri, line, character, msg.id),
            Request::Completion {
                uri,
                line,
                character,
            } => self.handle_completion(&uri, line, character, msg.id),
            Request::Definition {
                uri,
                line,
                character,
            } => self.handle_definition(&uri, line, character, msg.id),
            Request::OnTypeFormatting {
                uri,
                line,
                character,
                ch,
                options,
            } => self.handle_on_type_formatting(&uri, line, character, &ch, &options, msg.id),
            Request::SemanticTokens { uri } => self.handle_semantic_tokens(&uri, msg.id),
            Request::SemanticTokensRange { uri, range } => {
                self.handle_semantic_tokens_range(&uri, &range, msg.id)
            }
        }
    }

    // ── Request handlers ─────────────────────────────────────────────────────

    fn handle_initialize(&self, id: Option<RequestId>) -> Vec<Response> {
        let capabilities = serde_json::json!({
            "textDocumentSync": {
                "openClose": true,
                "change": 1  // TextDocumentSyncKind.Full
            },
            "hoverProvider": true,
            "completionProvider": {
                "triggerCharacters": ["\"", "#", "@", "-", "+", "{", "(", ":"]
            },
            "definitionProvider": true,
            "documentOnTypeFormattingProvider": {
                "moreTriggerChars": ["\t"]
            },
            "semanticTokensOptions": {
                "legend": {
                    "tokenTypes": ["keyword", "function", "variable", "string", "comment", "type", "parameter", "number", "operator"],
                    "tokenModifiers": []
                },
                "full": true,
                "range": true
            }
        });

        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "capabilities": capabilities
            })),
            error: None,
        }]
    }

    fn handle_shutdown(&self, id: Option<RequestId>) -> Vec<Response> {
        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::Value::Null),
            error: None,
        }]
    }

    // ── Document handlers ────────────────────────────────────────────────────

    fn handle_did_open(&mut self, uri: String, text: String) -> Vec<Response> {
        let diagnostics = self.parse_and_store(uri.clone(), text);
        self.publish_diagnostics(&uri, diagnostics)
    }

    fn handle_did_change(&mut self, uri: String, text: String) -> Vec<Response> {
        let diagnostics = self.parse_and_store(uri.clone(), text);
        self.publish_diagnostics(&uri, diagnostics)
    }

    fn handle_did_close(&mut self, uri: String) -> Vec<Response> {
        self.documents.remove(&uri);
        // Clear diagnostics on close
        self.publish_diagnostics(&uri, vec![])
    }

    fn handle_hover(
        &mut self,
        uri: &str,
        line: u32,
        character: u32,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return self.empty_response(id),
        };

        // Try to find the word at the cursor position
        let word = self.word_at_position(uri, line, character);

        // If there's a parse error, show that regardless of position
        if let Some(err) = &doc.error {
            let hover_content = format_error_message(err);
            return vec![Response::Response {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!({
                    "contents": {
                        "kind": "markdown",
                        "value": hover_content,
                    }
                })),
                error: None,
            }];
        }

        // Build story-level metadata (shown when cursor is not over a specific symbol)
        let story_metadata = if let Some(story) = doc.story.as_ref() {
            let mut content = format!("**Story**: `{}`\n\n", story.name);

            if !story.tags.is_empty() {
                content.push_str(&format!("**Tags**: {}\n\n", story.tags.join(", ")));
            }

            // List all stat definitions
            let stats: Vec<_> = story
                .items
                .iter()
                .filter_map(|item| match item {
                    StoryItem::StatDef(s) => Some(s.name.as_str()),
                    _ => None,
                })
                .collect();
            if !stats.is_empty() {
                content.push_str(&format!("**Stats**: {}\n\n", stats.join(", ")));
            }

            // List all flag definitions
            let flags: Vec<_> = story
                .items
                .iter()
                .filter_map(|item| match item {
                    StoryItem::FlagDef(f) => Some(f.name.as_str()),
                    _ => None,
                })
                .collect();
            if !flags.is_empty() {
                content.push_str(&format!("**Flags**: {}\n\n", flags.join(", ")));
            }

            // List all effect definitions
            let effects: Vec<_> = story
                .items
                .iter()
                .filter_map(|item| match item {
                    StoryItem::EffectDef(e) => Some(e.name.as_str()),
                    _ => None,
                })
                .collect();
            if !effects.is_empty() {
                content.push_str(&format!("**Effects**: {}\n\n", effects.join(", ")));
            }

            // List all event ids
            let events: Vec<_> = story
                .items
                .iter()
                .filter_map(|item| match item {
                    StoryItem::EventDef(e) => Some(e.id.as_str()),
                    _ => None,
                })
                .collect();
            if !events.is_empty() {
                content.push_str(&format!("**Events**: {}", events.join(", ")));
            }

            Some(content)
        } else {
            None
        };

        // Determine hover content: prefer symbol-specific info, fall back to story metadata
        let hover_content = if let Some(story) = doc.story.as_ref() {
            if let Some(word) = &word {
                // If the cursor is over a specific symbol, return its info;
                // otherwise fall back to story-level metadata
                self.symbol_hover_info(story, word)
                    .or(story_metadata.clone())
            } else {
                // Cursor is not over a symbol — show story-level metadata
                story_metadata
            }
        } else {
            None
        };

        let hover_content = hover_content.unwrap_or_else(|| "No information available".to_string());

        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "contents": {
                    "kind": "markdown",
                    "value": hover_content
                }
            })),
            error: None,
        }]
    }

    fn handle_completion(
        &mut self,
        uri: &str,
        _line: u32,
        _character: u32,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        let doc = match self.documents.get(uri) {
            Some(d) if d.story.is_some() => d,
            _ => return self.empty_completion(id),
        };

        let story = doc.story.as_ref().unwrap();
        let mut items = Vec::new();

        // Event IDs
        for item in &story.items {
            if let StoryItem::EventDef(e) = item {
                items.push(CompletionItem {
                    label: e.id.clone(),
                    kind: Some(CompletionItemKind::Class),
                    detail: Some("event".to_string()),
                    documentation: None,
                });
            }
        }

        // Stat names
        for item in &story.items {
            if let StoryItem::StatDef(s) = item {
                items.push(CompletionItem {
                    label: s.name.clone(),
                    kind: Some(CompletionItemKind::Variable),
                    detail: Some(format!("stat = {}", s.default)),
                    documentation: None,
                });
            }
        }

        // Flag names
        for item in &story.items {
            if let StoryItem::FlagDef(f) = item {
                items.push(CompletionItem {
                    label: f.name.clone(),
                    kind: Some(CompletionItemKind::Field),
                    detail: Some(format!("flag = {}", f.default)),
                    documentation: None,
                });
            }
        }

        // Effect names
        for item in &story.items {
            if let StoryItem::EffectDef(e) = item {
                items.push(CompletionItem {
                    label: e.name.clone(),
                    kind: Some(CompletionItemKind::Function),
                    detail: Some("effect".to_string()),
                    documentation: None,
                });
            }
        }

        // DSL keywords
        for kw in &[
            "event", "choice", "effect", "stat", "flag", "tags", "requires", "next", "uses", "add",
            "set",
        ] {
            items.push(CompletionItem {
                label: kw.to_string(),
                kind: Some(CompletionItemKind::Keyword),
                detail: Some("keyword".to_string()),
                documentation: None,
            });
        }

        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "isIncomplete": false,
                "items": items
            })),
            error: None,
        }]
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn parse_and_store(&mut self, uri: String, text: String) -> Vec<Diagnostic> {
        let result = parse_story(&text);
        let (story, error) = match result {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        };

        self.documents.insert(
            uri,
            DocumentState {
                text,
                story,
                error: error.clone(),
            },
        );

        match &error {
            Some(err) => vec![diagnostics_from_parse_error(err)],
            None => vec![],
        }
    }

    fn publish_diagnostics(&self, uri: &str, diagnostics: Vec<Diagnostic>) -> Vec<Response> {
        vec![Response::PublishDiagnostics {
            method: "textDocument/publishDiagnostics".to_string(),
            params: PublishDiagnosticsParams {
                uri: uri.to_string(),
                diagnostics,
            },
        }]
    }

    // ── Go-to-definition ─────────────────────────────────────────────────────

    fn handle_definition(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        let word = match self.word_at_position(uri, line, character) {
            Some(w) => w,
            None => return self.empty_response(id),
        };

        let loc = match self.find_definition(uri, &word) {
            Some(loc) => loc,
            None => return self.empty_response(id),
        };

        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "uri": loc.uri,
                "range": {
                    "start": { "line": loc.range.start.line, "character": loc.range.start.character },
                    "end":   { "line": loc.range.end.line,   "character": loc.range.end.character },
                }
            })),
            error: None,
        }]
    }

    // ── On-type formatting (tab → spaces) ────────────────────────────────────

    fn handle_on_type_formatting(
        &self,
        uri: &str,
        line: u32,
        character: u32,
        ch: &str,
        options: &serde_json::Value,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        // Only respond to tab characters
        if ch != "\t" {
            return self.empty_response(id);
        }

        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return self.empty_response(id),
        };

        let lines: Vec<&str> = doc.text.lines().collect();
        let line_idx = line as usize;
        if line_idx >= lines.len() {
            return self.empty_response(id);
        }

        let target_line = lines[line_idx];
        let char_pos = character as usize;

        // The tab should be at character position - 1 (cursor is after the tab)
        let tab_pos = char_pos.saturating_sub(1);

        // Get the line as bytes (LSP uses UTF-16 code units; for ASCII text this is equivalent)
        let bytes = target_line.as_bytes();

        if tab_pos < bytes.len() && bytes[tab_pos] == b'\t' {
            // Use tabSize from options, or default to 2
            let tab_size: u32 = options
                .get("tabSize")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(2);

            let spaces = " ".repeat(tab_size as usize);

            return vec![Response::Response {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!([{
                    "range": {
                        "start": { "line": line, "character": tab_pos as u32 },
                        "end":   { "line": line, "character": char_pos as u32 }
                    },
                    "newText": spaces
                }])),
                error: None,
            }];
        }

        self.empty_response(id)
    }

    // ── Semantic tokens (syntax highlighting) ────────────────────────────────

    fn handle_semantic_tokens(&self, uri: &str, id: Option<RequestId>) -> Vec<Response> {
        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return self.empty_response(id),
        };

        let tokens = tokenize_semantic(&doc.text);

        // Build the data array: each token is [line, startChar, length, tokenType, tokenModifiers]
        // tokenModifiers is 0 (no modifiers)
        let data: Vec<serde_json::Value> = tokens
            .iter()
            .map(|t| serde_json::json!([t.line, t.start_char, t.length, t.token_type, 0]))
            .collect();

        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({ "data": data })),
            error: None,
        }]
    }

    fn handle_semantic_tokens_range(
        &self,
        uri: &str,
        _range: &Range,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        // For simplicity, treat range tokens the same as full tokens.
        // A more sophisticated implementation would filter tokens to the range.
        self.handle_semantic_tokens(uri, id)
    }

    // ── Helper methods ─────────────────────────────────────────────────────────

    /// Extract the word at the given cursor position from the source text.
    /// Returns `Some(word)` if the cursor is on an identifier character.
    fn word_at_position(&self, uri: &str, line: u32, character: u32) -> Option<String> {
        let doc = self.documents.get(uri)?;
        let lines: Vec<&str> = doc.text.lines().collect();
        let line_idx = line as usize;
        if line_idx >= lines.len() {
            return None;
        }
        let target_line = lines[line_idx];
        let bytes = target_line.as_bytes();
        let char_pos = character as usize;

        if char_pos > bytes.len() {
            return None;
        }

        // Find start of word (walk backwards)
        let mut start = char_pos;
        while start > 0 {
            let c = bytes[start - 1] as char;
            if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                start -= 1;
            } else {
                break;
            }
        }

        // Find end of word (walk forwards)
        let mut end = char_pos;
        while end < bytes.len() {
            let c = bytes[end] as char;
            if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                end += 1;
            } else {
                break;
            }
        }

        if start == end {
            return None;
        }

        std::str::from_utf8(&bytes[start..end])
            .ok()
            .map(|s| s.to_string())
    }

    /// Find the definition location (line, start char, end char) of a word
    /// by searching the story's item definitions and source text.
    fn find_definition(&self, uri: &str, word: &str) -> Option<Location> {
        let doc = self.documents.get(uri)?;
        let story = doc.story.as_ref()?;

        for item in &story.items {
            let (keyword, name) = match item {
                StoryItem::StatDef(s) if s.name == word => ("stat", s.name.as_str()),
                StoryItem::FlagDef(f) if f.name == word => ("flag", f.name.as_str()),
                StoryItem::EffectDef(e) if e.name == word => ("effect", e.name.as_str()),
                StoryItem::EventDef(e) if e.id == word => ("event", e.id.as_str()),
                _ => continue,
            };

            // Search the source text for the definition line
            for (line_idx, line) in doc.text.lines().enumerate() {
                let trimmed = line.trim_start();
                if let Some(rest) = trimmed.strip_prefix(keyword) {
                    // Ensure word boundary after keyword
                    let after = rest.chars().next();
                    if after.is_none() || after.unwrap().is_whitespace() || after.unwrap() == ':' {
                        let after_kw = rest.trim_start();
                        if let Some(tail) = after_kw.strip_prefix(name) {
                            // Verify word boundary after the name
                            let after_name = tail.chars().next();
                            let boundary_ok = after_name.is_none()
                                || after_name.unwrap().is_whitespace()
                                || after_name.unwrap() == ':';
                            if boundary_ok {
                                let line_start = line.len() - trimmed.len();
                                let kw_end = keyword.len();
                                let ws_after_kw = rest.len() - rest.trim_start().len();
                                let name_start = line_start + kw_end + ws_after_kw;
                                let name_end = name_start + name.len();
                                return Some(Location {
                                    uri: uri.to_string(),
                                    range: Range {
                                        start: Position {
                                            line: line_idx as u32,
                                            character: name_start as u32,
                                        },
                                        end: Position {
                                            line: line_idx as u32,
                                            character: name_end as u32,
                                        },
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Build hover content for a specific symbol name.
    fn symbol_hover_info(&self, story: &Story, word: &str) -> Option<String> {
        let keyword_set: HashSet<&str> = KEYWORDS.iter().copied().collect();
        if keyword_set.contains(word) {
            return Some(format!("**Keyword**: `{}`\n\nCYOA DSL keyword.", word));
        }

        for item in &story.items {
            match item {
                StoryItem::StatDef(s) if s.name == word => {
                    return Some(format!(
                        "`stat` **{}** = {}\n\nDefault value: `{}`",
                        s.name, s.default, s.default
                    ));
                }
                StoryItem::FlagDef(f) if f.name == word => {
                    return Some(format!(
                        "`flag` **{}** (default: `{}`)\n\nA boolean story flag.",
                        f.name, f.default
                    ));
                }
                StoryItem::EffectDef(e) if e.name == word => {
                    let mut content = format!("`effect` **{}**\n\n**Body:**\n\n", e.name);
                    for step in &e.body {
                        match step {
                            EffectStep::ChangeStat { stat, delta } => {
                                let sign = if *delta >= 0 { "+" } else { "-" };
                                content.push_str(&format!(
                                    "- `{}` {} by {}\n",
                                    stat,
                                    sign,
                                    delta.abs()
                                ));
                            }
                            EffectStep::SetFlag { flag, value } => {
                                content.push_str(&format!("- `set {} to {}`\n", flag, value));
                            }
                            EffectStep::AddTag { tag } => {
                                content.push_str(&format!("- `add {}`\n", tag));
                            }
                            EffectStep::Text(t) => {
                                let txt = format_text_segments(&t.segments);
                                content.push_str(&format!("- \"{}\"\n", txt));
                            }
                        }
                    }
                    return Some(content);
                }
                StoryItem::EventDef(e) if e.id == word => {
                    let mut content = format!("`event` **{}**\n\n", e.id);
                    for text_content in &e.text {
                        let txt = format_text_segments(&text_content.segments);
                        content.push_str(&format!("> {}\n\n", txt));
                    }
                    if !e.choices.is_empty() {
                        content.push_str("**Choices:**\n\n");
                        for (i, choice) in e.choices.iter().enumerate() {
                            let txt = format_text_segments(&choice.text.segments);
                            content.push_str(&format!("{}. {}\n", i + 1, txt));
                        }
                    }
                    return Some(content);
                }
                _ => {}
            }
        }
        None
    }

    fn empty_response(&self, id: Option<RequestId>) -> Vec<Response> {
        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::Value::Null),
            error: None,
        }]
    }

    fn empty_completion(&self, id: Option<RequestId>) -> Vec<Response> {
        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(serde_json::json!({
                "isIncomplete": false,
                "items": []
            })),
            error: None,
        }]
    }
}

/// Format text segments into a single string for display.
fn format_text_segments(segments: &[TextSegment]) -> String {
    let mut result = String::new();
    for seg in segments {
        match seg {
            TextSegment::Literal(lit) => result.push_str(lit),
            TextSegment::StatRef(var) => result.push_str(&format!("{{{{{}}}}}", var)),
        }
    }
    result
}

/// Tokenize source text into semantic tokens for syntax highlighting.
/// Tracks multi-line string state (quoted strings spanning multiple source lines).
fn tokenize_semantic(text: &str) -> Vec<SemanticToken> {
    let keywords: HashSet<&str> = KEYWORDS.iter().copied().collect();
    let mut tokens = Vec::new();
    let mut in_string = false;

    for (line_idx, line) in text.lines().enumerate() {
        let line_num = line_idx as u32;
        let bytes = line.as_bytes();
        let mut i = 0usize;

        // If we were inside a string at the end of the previous line,
        // scan this line for the closing quote.
        if in_string {
            let start = 0usize;
            let mut end = 0usize;

            while i < bytes.len() {
                let c = bytes[i] as char;
                if c == '\\' {
                    i += 2;
                    end = i;
                    continue;
                }
                if c == '"' {
                    in_string = false;
                    i += 1;
                    end = i;
                    break;
                }
                i += 1;
                end = i;
            }

            if end > 0 {
                tokens.push(SemanticToken {
                    line: line_num,
                    start_char: start as u32,
                    length: (end - start) as u32,
                    token_type: TT_STRING,
                });
            }

            if in_string {
                // String continues to next line — skip normal tokenization
                // for the content we already scanned
                continue;
            }
        }

        // Normal tokenization
        while i < bytes.len() {
            let c = bytes[i] as char;

            if c.is_whitespace() {
                i += 1;
                continue;
            }

            // Comments: # to end of line
            if c == '#' {
                tokens.push(SemanticToken {
                    line: line_num,
                    start_char: i as u32,
                    length: (bytes.len() - i) as u32,
                    token_type: TT_COMMENT,
                });
                break; // rest of line is comment
            }

            // Identifiers and keywords
            if c.is_ascii_alphabetic() || c == '_' || c == '$' {
                let start = i;
                while i < bytes.len() {
                    let c = bytes[i] as char;
                    if c.is_ascii_alphanumeric() || c == '_' || c == '$' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let word = std::str::from_utf8(&bytes[start..i]).unwrap();
                let token_type = if keywords.contains(word) {
                    TT_KEYWORD
                } else {
                    TT_TYPE
                };
                tokens.push(SemanticToken {
                    line: line_num,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type,
                });
                continue;
            }

            // Template variables {{...}}
            if i + 1 < bytes.len() && bytes[i] == b'{' && bytes[i + 1] == b'{' {
                let start = i;
                i += 2;
                while i + 1 < bytes.len() {
                    if bytes[i] == b'}' && bytes[i + 1] == b'}' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                if i + 1 >= bytes.len() && i < bytes.len() {
                    // Incomplete {{ — treat the rest as parameter
                    i = bytes.len();
                }
                tokens.push(SemanticToken {
                    line: line_num,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type: TT_PARAMETER,
                });
                continue;
            }

            // Quoted strings
            if c == '"' {
                let start = i;
                i += 1; // skip opening quote
                let mut found_close = false;
                while i < bytes.len() {
                    let c = bytes[i] as char;
                    if c == '\\' {
                        i += 2;
                        continue;
                    }
                    if c == '"' {
                        i += 1;
                        found_close = true;
                        break;
                    }
                    i += 1;
                }

                if !found_close {
                    in_string = true;
                }

                tokens.push(SemanticToken {
                    line: line_num,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type: TT_STRING,
                });
                continue;
            }

            // Numbers (including negative)
            if c.is_ascii_digit()
                || (c == '-' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit())
            {
                let start = i;
                if c == '-' {
                    i += 1;
                }
                while i < bytes.len() && (bytes[i] as char).is_ascii_digit() {
                    i += 1;
                }
                tokens.push(SemanticToken {
                    line: line_num,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type: TT_NUMBER,
                });
                continue;
            }

            // Operators and punctuation
            if c == '='
                || c == '+'
                || c == ':'
                || c == '>'
                || c == '<'
                || c == '!'
                || c == '('
                || c == ')'
                || c == '['
                || c == ']'
                || c == ','
                || c == '.'
            {
                let start = i;
                // Check for multi-char operators (>=, <=, ==, !=)
                if i + 1 < bytes.len() {
                    let two_char = std::str::from_utf8(&bytes[i..i + 2]).unwrap();
                    if two_char == ">=" || two_char == "<=" || two_char == "==" || two_char == "!="
                    {
                        i += 2;
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
                tokens.push(SemanticToken {
                    line: line_num,
                    start_char: start as u32,
                    length: (i - start) as u32,
                    token_type: TT_OPERATOR,
                });
                continue;
            }

            // Skip unknown characters
            i += 1;
        }
    }

    tokens
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::RawMessage;

    const VALID_STORY: &str = r#"story TestStory:
  tags: fantasy, test
  stat hp = 50
  flag visited_cave
  effect found_item:
    + hp by 10
    text "You found a potion!"
  event start:
    "You begin your journey."
    choice "Go to cave":
      uses found_item
      next cave
  event cave:
    "You enter the cave."
"#;

    const INVALID_STORY: &str = r#"story BadStory:
  this is not valid syntax
"#;

    /// Build a JSON-RPC `didOpen` notification message.
    fn did_open_msg(uri: &str, text: &str) -> RawMessage {
        serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "text": text
                }
            }
        }))
        .unwrap()
    }

    /// Build a JSON-RPC request message (e.g. initialize, hover, completion).
    fn request_msg_id(
        method: &str,
        id: serde_json::Value,
        params: serde_json::Value,
    ) -> RawMessage {
        serde_json::from_value(serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }))
        .unwrap()
    }

    /// Build a JSON-RPC request message with numeric id 1.
    fn request_msg(method: &str, params: serde_json::Value) -> RawMessage {
        request_msg_id(
            method,
            serde_json::Value::Number(serde_json::Number::from(1i64)),
            params,
        )
    }

    #[test]
    fn test_initialize_returns_capabilities() {
        let mut server = Server::new();
        let msg = request_msg("initialize", serde_json::json!({}));

        let responses = server.handle(msg);
        assert_eq!(responses.len(), 1);

        match &responses[0] {
            Response::Response { result, error, .. } => {
                assert!(error.is_none());
                let json = result.as_ref().unwrap();
                assert!(json.get("capabilities").is_some());
                let caps = &json["capabilities"];
                assert_eq!(caps["hoverProvider"], true);
                assert_eq!(caps["textDocumentSync"]["openClose"], true);
            }
            _ => panic!("expected Response variant"),
        }
    }

    #[test]
    fn test_did_open_valid_story_no_diagnostics() {
        let mut server = Server::new();
        let msg = did_open_msg("file:///test.cyoa", VALID_STORY);

        let responses = server.handle(msg);
        // didOpen always returns a publishDiagnostics notification
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::PublishDiagnostics { params, .. } => {
                assert!(params.diagnostics.is_empty());
                assert_eq!(params.uri, "file:///test.cyoa");
            }
            _ => panic!("expected PublishDiagnostics"),
        }
    }

    #[test]
    fn test_did_open_invalid_story_has_diagnostics() {
        let mut server = Server::new();
        let msg = did_open_msg("file:///bad.cyoa", INVALID_STORY);

        let responses = server.handle(msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::PublishDiagnostics { params, .. } => {
                assert_eq!(params.diagnostics.len(), 1);
                assert_eq!(
                    params.diagnostics[0].severity,
                    Some(DiagnosticSeverity::Error)
                );
                assert!(params.diagnostics[0].message.contains("line"));
            }
            _ => panic!("expected PublishDiagnostics"),
        }
    }

    #[test]
    fn test_hover_returns_story_metadata() {
        let mut server = Server::new();

        // Open a doc first
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // Hover over "TestStory" (the story name, which is not a keyword or
        // symbol) — the server falls back to story-level metadata.
        let hover_msg = request_msg(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 0, "character": 7}
            }),
        );

        let responses = server.handle(hover_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let contents = &json["contents"]["value"];
                let value = contents.as_str().unwrap();
                assert!(value.contains("TestStory"));
                assert!(value.contains("fantasy"));
                assert!(value.contains("hp"));
                assert!(value.contains("visited_cave"));
                assert!(value.contains("found_item"));
                assert!(value.contains("start"));
                assert!(value.contains("cave"));
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_hover_over_keyword_returns_keyword_info() {
        let mut server = Server::new();

        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // Hover over the "story" keyword at position (0, 0)
        let hover_msg = request_msg(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 0, "character": 0}
            }),
        );

        let responses = server.handle(hover_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let contents = &json["contents"]["value"];
                let value = contents.as_str().unwrap();
                assert!(value.contains("keyword"));
                assert!(value.contains("story"));
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_hover_over_stat_returns_stat_info() {
        let mut server = Server::new();

        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // "stat hp = 50" — hover over "hp"
        let hover_msg = request_msg(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 2, "character": 8}
            }),
        );

        let responses = server.handle(hover_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let value = &json["contents"]["value"];
                let value = value.as_str().unwrap();
                assert!(value.contains("stat"));
                assert!(value.contains("hp"));
                assert!(value.contains("50"));
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_hover_unknown_document_returns_null() {
        let mut server = Server::new();

        let hover_msg = request_msg(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": {"uri": "file:///unknown.cyoa"},
                "position": {"line": 0, "character": 0}
            }),
        );

        let responses = server.handle(hover_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                assert_eq!(result.as_ref().unwrap(), &serde_json::Value::Null);
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_completion_returns_story_elements() {
        let mut server = Server::new();

        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        let completion_msg = request_msg(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 10, "character": 0}
            }),
        );

        let responses = server.handle(completion_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let items = json["items"].as_array().unwrap();
                // Should contain at least: start, cave, hp, visited_cave, found_item, keywords
                let labels: Vec<&str> =
                    items.iter().map(|i| i["label"].as_str().unwrap()).collect();
                assert!(labels.contains(&"start"));
                assert!(labels.contains(&"cave"));
                assert!(labels.contains(&"hp"));
                assert!(labels.contains(&"visited_cave"));
                assert!(labels.contains(&"found_item"));
                assert!(labels.contains(&"event"));
                assert!(labels.contains(&"choice"));
                assert!(labels.contains(&"stat"));
                assert!(labels.contains(&"flag"));
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_completion_unknown_document_returns_empty() {
        let mut server = Server::new();

        let completion_msg = request_msg(
            "textDocument/completion",
            serde_json::json!({
                "textDocument": {"uri": "file:///unknown.cyoa"},
                "position": {"line": 0, "character": 0}
            }),
        );

        let responses = server.handle(completion_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                assert_eq!(json["items"].as_array().unwrap().len(), 0);
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_did_change_after_error_produces_diagnostics() {
        let mut server = Server::new();

        // Open with invalid story
        let open_msg = did_open_msg("file:///test.cyoa", INVALID_STORY);
        let responses = server.handle(open_msg);
        match &responses[0] {
            Response::PublishDiagnostics { params, .. } => {
                assert_eq!(params.diagnostics.len(), 1);
            }
            _ => panic!("expected PublishDiagnostics"),
        }

        // Change to valid story
        let change_msg = request_msg(
            "textDocument/didChange",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "contentChanges": [{"text": VALID_STORY}]
            }),
        );

        let responses = server.handle(change_msg);
        match &responses[0] {
            Response::PublishDiagnostics { params, .. } => {
                assert!(params.diagnostics.is_empty());
            }
            _ => panic!("expected PublishDiagnostics"),
        }
    }

    #[test]
    fn test_did_close_clears_diagnostics() {
        let mut server = Server::new();

        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        let close_msg = request_msg(
            "textDocument/didClose",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"}
            }),
        );

        let responses = server.handle(close_msg);
        match &responses[0] {
            Response::PublishDiagnostics { params, .. } => {
                assert!(params.diagnostics.is_empty());
            }
            _ => panic!("expected PublishDiagnostics"),
        }
    }

    #[test]
    fn test_shutdown_returns_null() {
        let mut server = Server::new();
        let msg = request_msg("shutdown", serde_json::json!({}));

        let responses = server.handle(msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, error, .. } => {
                assert!(error.is_none());
                assert_eq!(result.as_ref().unwrap(), &serde_json::Value::Null);
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_unknown_method_returns_no_response() {
        let mut server = Server::new();
        let msg: RawMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"method":"workspace/something","params":{}}"#,
        )
        .unwrap();

        let responses = server.handle(msg);
        assert!(responses.is_empty());
    }

    #[test]
    fn test_hover_error_story_returns_error_message() {
        let mut server = Server::new();

        let open_msg = did_open_msg("file:///bad.cyoa", INVALID_STORY);
        server.handle(open_msg);

        let hover_msg = request_msg(
            "textDocument/hover",
            serde_json::json!({
                "textDocument": {"uri": "file:///bad.cyoa"},
                "position": {"line": 0, "character": 0}
            }),
        );

        let responses = server.handle(hover_msg);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let value = json["contents"]["value"].as_str().unwrap();
                assert!(value.contains("line"));
                assert!(value.contains("col"));
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_definition_for_stat() {
        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // "stat hp = 50" — "hp" starts at character 8 on line 2
        let def_msg = request_msg(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 2, "character": 9}
            }),
        );

        let responses = server.handle(def_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let uri = &json["uri"];
                assert!(uri.is_string());
                let range = &json["range"];
                let start = &range["start"];
                assert_eq!(start["line"], 2);
                let end = &range["end"];
                assert_eq!(end["line"], 2);
                assert!(end["character"].as_u64().unwrap() > start["character"].as_u64().unwrap());
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_definition_for_effect() {
        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // In the "start" event, "uses found_item" is on line 10
        // Line 10 is: "      uses found_item"
        let def_msg = request_msg(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 10, "character": 12}
            }),
        );

        let responses = server.handle(def_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let range = &json["range"];
                let start = &range["start"];
                let end = &range["end"];
                assert_eq!(start["line"], 4); // "effect found_item:" is at line 4
                assert_eq!(end["line"], 4);
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_definition_returns_null_for_unknown_word() {
        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // Hover over "TestStory" (line 0, character 8) — not a defined symbol
        let def_msg = request_msg(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 0, "character": 8}
            }),
        );

        let responses = server.handle(def_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                assert!(result.as_ref().unwrap().is_null());
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_on_type_formatting_replaces_tab_with_spaces() {
        // Story with a literal tab character inside a string literal (parses fine)
        let story_with_tab = "story TestStory:\n  text \"has\ttab\"\n";

        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", story_with_tab);
        server.handle(open_msg);

        // Line 1 is: `  text "has	ab"` — the tab is at character 11
        // Cursor at character 12 means tab_pos = 11
        let fmt_msg = request_msg(
            "textDocument/onTypeFormatting",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 1, "character": 12},
                "ch": "\t",
                "options": {
                    "tabSize": 2,
                    "insertSpaces": true
                }
            }),
        );

        let responses = server.handle(fmt_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let edits = json.as_array().unwrap();
                assert!(edits.len() >= 1);
                let range = &edits[0]["range"];
                assert_eq!(range["start"]["line"], 1);
                assert_eq!(range["start"]["character"], 11);
                let new_text = &edits[0]["newText"];
                assert_eq!(new_text, "  "); // 2 spaces
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_on_type_formatting_ignores_non_tab_char() {
        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // Sending a non-tab character should return null (no edit)
        let fmt_msg = request_msg(
            "textDocument/onTypeFormatting",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "position": {"line": 1, "character": 2},
                "ch": "a",
                "options": {"tabSize": 2, "insertSpaces": true}
            }),
        );

        let responses = server.handle(fmt_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                assert!(result.as_ref().unwrap().is_null());
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_semantic_tokens_present() {
        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        let tokens_msg = request_msg(
            "textDocument/semanticTokens",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
            }),
        );

        let responses = server.handle(tokens_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let data = json["data"].as_array().unwrap();
                assert!(!data.is_empty());

                // Each token is [line, startChar, length, tokenType, tokenModifiers]
                // Token type 0 = keyword (from TT_KEYWORD constant)
                let has_keyword = data.iter().any(|t| {
                    t[3].as_u64().unwrap() == 0 // TT_KEYWORD = 0
                });
                assert!(has_keyword, "expected at least one keyword token");
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_semantic_tokens_legend_in_capabilities() {
        // The legend (tokenTypes) is advertised in the initialize response
        let mut server = Server::new();
        let init_msg = request_msg("initialize", serde_json::json!({}));

        let responses = server.handle(init_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let legend = &json["capabilities"]["semanticTokensOptions"]["legend"]["tokenTypes"];
                let legend = legend.as_array().unwrap();
                let legend_str: Vec<&str> = legend.iter().map(|v| v.as_str().unwrap()).collect();
                assert!(legend_str.contains(&"keyword"));
                assert!(legend_str.contains(&"string"));
                assert!(legend_str.contains(&"comment"));
            }
            _ => panic!("expected Response"),
        }
    }
}
