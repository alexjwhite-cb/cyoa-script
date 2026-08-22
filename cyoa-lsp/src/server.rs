//! The CYOA Language Server core logic.
//!
//! Tracks open documents (URI → source text + parsed AST) and responds to
//! LSP requests: diagnostics on open/change, hover info, and basic completion.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::diagnostics::{diagnostics_from_parse_error, format_error_message};
use crate::protocol::*;
use cyoa_ast::{EffectStep, Story, StoryItem, TextSegment};
use cyoa_compiler::{parse_story, resolve_imports};

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
/// Note: `start_char` and `length` are **byte offsets** (not UTF-16 code units).
/// Conversion to UTF-16 happens in `handle_semantic_tokens`.
#[derive(Debug, Clone)]
struct SemanticToken {
    line: u32,
    start_char: u32,
    length: u32,
    token_type: u32,
}

/// Convert a UTF-16 code unit offset to a byte offset within a line.
/// The LSP spec uses UTF-16 code units for character positions; Rust strings
/// use byte offsets. This conversion is necessary whenever the client
/// (e.g. GoLand) sends a `character` position that includes characters
/// outside the BMP or multibyte UTF-8 sequences.
fn utf16_to_byte_idx(line: &str, utf16_pos: u32) -> usize {
    let mut utf16_count = 0u32;
    for (byte_idx, ch) in line.char_indices() {
        if utf16_count >= utf16_pos {
            return byte_idx;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    // Position is at or beyond the end of the string
    line.len()
}

/// Convert a byte offset to a UTF-16 code unit offset within a line.
fn byte_to_utf16_idx(line: &str, byte_pos: usize) -> u32 {
    let mut utf16_count = 0u32;
    for (byte_idx, ch) in line.char_indices() {
        if byte_idx >= byte_pos {
            return utf16_count;
        }
        utf16_count += ch.len_utf16() as u32;
    }
    utf16_count
}

/// Convert a byte range [start_byte, end_byte) to a UTF-16 code unit range.
fn byte_range_to_utf16(line: &str, start_byte: usize, end_byte: usize) -> (u32, u32) {
    (
        byte_to_utf16_idx(line, start_byte),
        byte_to_utf16_idx(line, end_byte),
    )
}

pub struct Server {
    /// Open documents: URI → (source text, parsed story, parse error if any)
    documents: HashMap<String, DocumentState>,
}

struct DocumentState {
    text: String,
    story: Option<Story>,
    error: Option<cyoa_compiler::ParseError>,
    /// Imported file paths + contents, for go-to-definition of symbols
    /// defined in imported files (e.g. `healing_potion` from `std/healing`).
    imported_files: Vec<(PathBuf, String)>,
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
                "triggerCharacters": ["\t"]
            },
            "semanticTokensOptions": {
                "legend": {
                    "tokenTypes": ["keyword", "function", "variable", "string", "comment", "type", "parameter", "number", "operator"],
                    "tokenModifiers": []
                },
                "full": true,
                "range": true,
                "formats": ["relative"]
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

        // If the story has imports, resolve them so that go-to-definition,
        // hover, and completion work for imported symbols (effects, stats,
        // flags, events from std/ or local files).
        let (story, imported_files) = self.resolve_story_imports(uri.as_str(), &uri, story);

        self.documents.insert(
            uri,
            DocumentState {
                text,
                story,
                error: error.clone(),
                imported_files,
            },
        );

        match &error {
            Some(err) => vec![diagnostics_from_parse_error(err)],
            None => vec![],
        }
    }

    /// Resolve imports for a story if it has any, falling back to the
    /// unmerged story on error (so main-file definitions still work).
    /// Returns the (possibly merged) story and a list of imported file
    /// paths + their text contents (for go-to-definition support).
    fn resolve_story_imports(
        &self,
        _uri: &str,
        uri: &str,
        story: Option<Story>,
    ) -> (Option<Story>, Vec<(PathBuf, String)>) {
        let story = match story.as_ref() {
            Some(s) => s,
            None => return (story, Vec::new()),
        };
        // Collect all import paths from both the imports list and StoryItem::Import items
        // (imports can appear either at top-level story.imports or as StoryItem::Import
        // inside the story body after parsing).
        let mut original_imports: Vec<String> =
            story.imports.iter().map(|i| i.path.clone()).collect();
        for item in &story.items {
            if let StoryItem::Import(import) = item {
                original_imports.push(import.path.clone());
            }
        }
        if original_imports.is_empty() {
            return (Some(story.clone()), Vec::new());
        }

        let path = match uri_to_path(uri) {
            Some(p) => p,
            None => return (Some(story.clone()), Vec::new()),
        };
        let base_dir = path.parent().unwrap_or(&path);
        let std_paths = find_std_dirs(base_dir);

        match resolve_imports(story, base_dir, &std_paths) {
            Ok(merged) => {
                // Re-read imported files to collect their text for go-to-definition
                let mut imported_files = Vec::new();
                for import_path in &original_imports {
                    if let Some(fp) = resolve_import_to_file(import_path, base_dir, &std_paths) {
                        if let Ok(text) = std::fs::read_to_string(&fp) {
                            imported_files.push((fp, text));
                        }
                    }
                }
                (Some(merged), imported_files)
            }
            Err(e) => {
                eprintln!("cyoa-lsp: import resolution failed: {}", e);
                (Some(story.clone()), Vec::new())
            }
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

        // LSP spec allows Location | Location[] | LocationLink[] | null.
        // Return as a single Location for maximum client compatibility
        // (some GoLand versions don't handle Location[] for definition requests).
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

        // Convert UTF-16 code unit offset to byte offset for text inspection
        let byte_pos = utf16_to_byte_idx(target_line, character);
        let bytes = target_line.as_bytes();

        // The tab should be at byte position - 1 (cursor is after the tab)
        let tab_byte_pos = byte_pos.saturating_sub(1);

        if tab_byte_pos < bytes.len() && bytes[tab_byte_pos] == b'\t' {
            // Use tabSize from options, or default to 2
            let tab_size: u32 = options
                .get("tabSize")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(2);

            let spaces = " ".repeat(tab_size as usize);

            // Return range in UTF-16 code unit positions (LSP requirement)
            let start_utf16 = character.saturating_sub(1);
            return vec![Response::Response {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::json!([{
                    "range": {
                        "start": { "line": line, "character": start_utf16 },
                        "end":   { "line": line, "character": character }
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
        let lines: Vec<&str> = doc.text.lines().collect();

        // LSP semantic token encoding uses relative values:
        // - line: delta from previous token's line (first token = absolute line)
        // - startChar: if same line as prev, offset from prev token's end;
        //   if different line, absolute char position on that line
        //
        // Additionally, LSP uses UTF-16 code unit offsets, but tokenize_semantic
        // produces byte offsets. We convert each token's byte offsets to UTF-16
        // code unit offsets before encoding.
        let mut data: Vec<serde_json::Value> = Vec::new();
        let mut prev_line: u32 = 0;
        let mut prev_end_utf16: u32 = 0;

        for t in &tokens {
            // Get the line text for this token
            let line_str = lines.get(t.line as usize).copied().unwrap_or("");
            let token_end_byte = (t.start_char + t.length) as usize;

            // Convert byte offsets to UTF-16 code unit offsets
            let start_utf16 = byte_to_utf16_idx(line_str, t.start_char as usize);
            let token_end_utf16 = byte_to_utf16_idx(line_str, token_end_byte);
            let utf16_len = token_end_utf16 - start_utf16;

            let line_delta = t.line.saturating_sub(prev_line);
            let start_char = if line_delta == 0 {
                // Same line: relative to end of previous token
                start_utf16.saturating_sub(prev_end_utf16)
            } else {
                // Different line: absolute character position
                start_utf16
            };

            // LSP spec: data must be a flat number[] array, where each
            // token is encoded as 5 consecutive numbers:
            // [line, startChar, length, tokenType, tokenModifiers]
            data.push(serde_json::json!(line_delta));
            data.push(serde_json::json!(start_char));
            data.push(serde_json::json!(utf16_len));
            data.push(serde_json::json!(t.token_type));
            data.push(serde_json::json!(0));

            prev_line = t.line;
            prev_end_utf16 = start_utf16 + utf16_len;
        }

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
    ///
    /// The `character` parameter is a UTF-16 code unit offset (per LSP spec),
    /// so we convert it to a byte offset before indexing into the line.
    fn word_at_position(&self, uri: &str, line: u32, character: u32) -> Option<String> {
        let doc = self.documents.get(uri)?;
        let lines: Vec<&str> = doc.text.lines().collect();
        let line_idx = line as usize;
        if line_idx >= lines.len() {
            return None;
        }
        let target_line = lines[line_idx];

        // Convert UTF-16 code unit offset to byte offset
        let char_pos = utf16_to_byte_idx(target_line, character);
        let bytes = target_line.as_bytes();

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
    ///
    /// Searches the main document text first. If the symbol is defined in an
    /// imported file (e.g. `healing_potion` from `std/healing`), falls back to
    /// searching the stored imported file texts and returns a `Location` with
    /// the imported file's URI.
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

            // Search the main document text for the definition
            if let Some(range) = find_def_in_text(&doc.text, keyword, name) {
                return Some(Location {
                    uri: uri.to_string(),
                    range,
                });
            }

            // Search imported file texts (for symbols from std/ or local imports)
            for (file_path, text) in &doc.imported_files {
                if let Some(range) = find_def_in_text(text, keyword, name) {
                    return Some(Location {
                        uri: path_to_uri(file_path),
                        range,
                    });
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

/// Search `text` for a line matching `keyword name` or `keyword name:` and
/// return the `Range` (in UTF-16 code unit offsets) of `name` on that line.
fn find_def_in_text(text: &str, keyword: &str, name: &str) -> Option<Range> {
    for (line_idx, line) in text.lines().enumerate() {
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
                        // Convert byte offsets to UTF-16 code unit offsets (LSP requirement)
                        let (start_utf16, end_utf16) =
                            byte_range_to_utf16(line, name_start, name_end);
                        return Some(Range {
                            start: Position {
                                line: line_idx as u32,
                                character: start_utf16,
                            },
                            end: Position {
                                line: line_idx as u32,
                                character: end_utf16,
                            },
                        });
                    }
                }
            }
        }
    }
    None
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

/// Resolve an import path (e.g. `"std/healing"` or `"./local.cyoa"`) to a
/// file path on disk. Mirrors the logic in `cyoa_compiler::resolver::ImportResolver::resolve_path`.
fn resolve_import_to_file(path: &str, base_dir: &Path, std_paths: &[PathBuf]) -> Option<PathBuf> {
    if let Some(rel_path) = path.strip_prefix("std/") {
        let rel = Path::new(rel_path);
        for std_base in std_paths {
            let full = std_base.join(rel).with_extension("cyoa");
            if full.exists() {
                return Some(full);
            }
        }
        for std_base in std_paths {
            let full = std_base.join(rel);
            if full.exists() {
                return Some(full);
            }
        }
        None
    } else if path.starts_with("./") || path.starts_with("../") {
        let rel = Path::new(&path[2..]); // Strip "./"
        let full = base_dir.join(rel).with_extension("cyoa");
        if full.exists() {
            Some(full)
        } else {
            None
        }
    } else {
        None
    }
}

/// Convert a filesystem `PathBuf` to a `file://` URI string.
fn path_to_uri(path: &Path) -> String {
    let path_str = path.display().to_string().replace('\\', "/");
    format!("file:///{}", path_str)
}

/// Convert a `file://` URI to a filesystem `PathBuf`.
///
/// Handles both POSIX paths (`/mnt/c/...`) and Windows drive-letter paths
/// (`file:///C:/path` — the leading `/` is stripped on Windows so that
/// `C:/path` is used instead of `/C:/path`, which `is_dir()` can't resolve).
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let path = uri.strip_prefix("file://")?;
    // Handle percent-encoded characters (e.g., %3A → :)
    let decoded = decode_percent(path);
    // On Windows, GoLand sends file:///C:/path — strip the leading /
    // so the path becomes C:/path (a valid absolute Windows path).
    #[cfg(windows)]
    let decoded = if decoded.len() >= 3 && decoded.starts_with('/') && decoded.as_bytes()[2] == b':'
    {
        decoded[1..].to_string()
    } else {
        decoded
    };
    Some(PathBuf::from(decoded))
}

/// Simple percent-decoding for URI paths (e.g., %20 → space, %3A → :).
fn decode_percent(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    result.push(b as char);
                    i += 3;
                    continue;
                }
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }
    result
}

/// Walk up the directory tree from `base` looking for `std/` directories
/// to use as search roots for `std/` imports. Falls back to searching from
/// the current working directory if no std/ is found relative to the file.
fn find_std_dirs(base: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut current: Option<&Path> = Some(base);
    while let Some(dir) = current {
        let std_dir = dir.join("std");
        if std_dir.is_dir() {
            dirs.push(std_dir);
        }
        current = dir.parent();
    }
    // Fallback: also check from the current working directory
    if dirs.is_empty() {
        if let Ok(cwd) = std::env::current_dir() {
            let cwd_std = cwd.join("std");
            if cwd_std.is_dir() {
                dirs.push(cwd_std);
            }
        }
    }
    dirs
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
                // Definition returns a single Location (not Location[])
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
                // Definition returns a single Location (not Location[])
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
    fn test_definition_for_imported_effect() {
        // Create a temporary project directory with its own std/ folder
        let tmp = std::env::temp_dir().join("cyoa_lsp_test_import_dir");
        let std_dir = tmp.join("std");
        std::fs::create_dir_all(&std_dir).unwrap_or(());

        // Write a healing.cyoa in the temp std/ directory
        let healing =
            "effect healing_potion:\n  + hp by 20\n  text \"You drink a healing potion.\"\n";
        std::fs::write(std_dir.join("healing.cyoa"), healing).unwrap();

        // Story with import and usage of healing_potion
        let story = "story ImportTestStory:\n  import \"std/healing\"\n  effect local_effect:\n    text \"A local effect.\"\n  event start:\n    \"You begin your journey.\"\n    choice \"Drink potion\":\n      uses healing_potion\n      next start\n";

        let story_path = tmp.join("test.cyoa");
        std::fs::write(&story_path, story).unwrap();

        let uri = format!(
            "file:///{}",
            story_path.display().to_string().replace('\\', "/")
        );

        let mut server = Server::new();
        let open_msg = did_open_msg(&uri, story);
        server.handle(open_msg);

        // Line 7 (0-indexed) is: `      uses healing_potion`
        // "healing_potion" starts at character 11 (after 6 spaces + "uses" + space)
        let def_msg = request_msg(
            "textDocument/definition",
            serde_json::json!({
                "textDocument": {"uri": uri},
                "position": {"line": 7, "character": 11}
            }),
        );

        let responses = server.handle(def_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                assert!(
                    !json.is_null(),
                    "definition should not be null for imported effect"
                );
                let loc_uri = json["uri"].as_str().unwrap();
                assert!(
                    loc_uri.contains("healing"),
                    "should navigate to healing file, got: {}",
                    loc_uri
                );
            }
            _ => panic!("expected Response"),
        }

        let _ = std::fs::remove_dir_all(&tmp);
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

                // LSP spec: data is a flat number[] — each token is 5 consecutive
                // numbers: [line, startChar, length, tokenType, tokenModifiers]
                // Token type 0 = keyword (from TT_KEYWORD constant)
                let has_keyword = (0..data.len()).step_by(5).any(|i| {
                    data[i + 3].as_u64().unwrap() == 0 // TT_KEYWORD = 0
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

    #[test]
    fn test_semantic_tokens_multiline_string() {
        // Story with a multiline string spanning two lines
        let story = "story TestStory:\n  text \"line one\n  line two\"\n";

        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", story);
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

                // Token type 3 = string (TT_STRING)
                // Flat array: each token is 5 consecutive numbers
                let string_tokens: Vec<_> = (0..data.len())
                    .step_by(5)
                    .filter(|&i| data[i + 3].as_u64().unwrap() == 3) // TT_STRING = 3
                    .collect();
                // Should have string tokens on BOTH lines (0-indexed: line 1 and line 2)
                assert!(
                    string_tokens.len() >= 2,
                    "expected string tokens on both lines, got {}",
                    string_tokens.len()
                );
            }
            _ => panic!("expected Response"),
        }
    }

    // ── UTF-16 / byte offset conversion tests ──────────────────────────────

    #[test]
    fn test_utf16_to_byte_idx_with_multibyte() {
        // "a\u{2014}b" = 5 bytes (a=1, em-dash=3, b=1), 3 UTF-16 code units
        let line = "a\u{2014}b";
        assert_eq!(utf16_to_byte_idx(line, 0), 0); // 'a'
        assert_eq!(utf16_to_byte_idx(line, 1), 1); // '—' (start)
        assert_eq!(utf16_to_byte_idx(line, 2), 4); // 'b'
        assert_eq!(utf16_to_byte_idx(line, 3), 5); // end of string (byte len = 5)
    }

    #[test]
    fn test_byte_to_utf16_idx_with_multibyte() {
        let line = "a\u{2014}b";
        assert_eq!(byte_to_utf16_idx(line, 0), 0); // 'a'
        assert_eq!(byte_to_utf16_idx(line, 1), 1); // start of '—'
        assert_eq!(byte_to_utf16_idx(line, 3), 2); // last byte of '—' → UTF-16 2 (next char 'b')
        assert_eq!(byte_to_utf16_idx(line, 4), 2); // 'b'
        assert_eq!(byte_to_utf16_idx(line, 5), 3); // end of string
    }

    #[test]
    fn test_semantic_tokens_formats_relative_in_capabilities() {
        let mut server = Server::new();
        let init_msg = request_msg("initialize", serde_json::json!({}));
        let responses = server.handle(init_msg);
        assert_eq!(responses.len(), 1);

        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let formats = &json["capabilities"]["semanticTokensOptions"]["formats"];
                let formats = formats.as_array().unwrap();
                let fmt_str: Vec<&str> = formats.iter().map(|v| v.as_str().unwrap()).collect();
                assert!(
                    fmt_str.contains(&"relative"),
                    "semanticTokensOptions should include \"formats\": [\"relative\"]"
                );
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_semantic_tokens_multiline_string_with_emdash() {
        // Multi-line string where the second line (continuation) contains an em-dash
        // and the closing quote — mirrors forest_adventure.cyoa lines 157-158.
        let story = "story TestStory:\n  text \"hello\n  world — end\"\n";

        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", story);
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

                // Token type 3 = string (TT_STRING)
                // Flat array: each token is 5 consecutive numbers
                let string_tokens: Vec<_> = (0..data.len())
                    .step_by(5)
                    .filter(|&i| data[i + 3].as_u64().unwrap() == 3) // TT_STRING = 3
                    .collect();
                assert!(
                    string_tokens.len() >= 2,
                    "expected string tokens on both lines (continuation with em-dash), got {}",
                    string_tokens.len()
                );
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_semantic_tokens_utf16_offsets_with_emdash() {
        // String with em-dash — byte offsets differ from UTF-16 offsets
        // Line 1: `  text "hello \u{2014} world"`
        // Bytes after em-dash are shifted by 2 (em-dash = 3 bytes, 1 UTF-16 unit)
        let story = "story TestStory:\n  text \"hello \u{2014} world\"\n";

        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", story);
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

                // Find the string token on line 1.
                // The string "hello — world" starts at char 7 (after `  text `).
                // UTF-16 length = 15 (1+5+1+1+1+5+1 = 15 chars in UTF-16)
                // Byte length  = 17 (em-dash is 3 bytes, others are 1)
                //
                // In relative encoding, the first token on line 1 has
                // line_delta > 0, so start_char is absolute (UTF-16).
                // Flat array: each token is 5 consecutive numbers
                // [line_delta, start_char, length, tokenType, tokenModifiers]
                let string_tokens: Vec<_> = (0..data.len())
                    .step_by(5)
                    .filter(|&i| data[i + 3].as_u64().unwrap() == 3) // TT_STRING = 3
                    .collect();
                assert!(
                    !string_tokens.is_empty(),
                    "expected at least one string token"
                );

                // The string token on line 1 should have startChar = 7 (UTF-16, absolute)
                // and length = 15 (UTF-16 code units, not 17 bytes)
                let line1_string: Vec<_> = string_tokens
                    .iter()
                    .filter(|&idx| {
                        // length field is at idx + 2 in the flat array
                        data[idx + 2].as_u64().unwrap() == 15 // length field
                    })
                    .collect();
                assert!(
                    !line1_string.is_empty(),
                    "expected string token with UTF-16 length 15 (not byte length 17) for em-dash line"
                );
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_word_at_position_after_multibyte_char() {
        let mut server = Server::new();
        // Line 1: `  "hello \u{2014} world"` — em-dash creates 2-byte offset
        let story = "story TestStory:\n  \"hello \u{2014} world\"\n";

        let open_msg = did_open_msg("file:///test.cyoa", story);
        server.handle(open_msg);

        // UTF-16 positions on line 1:
        // 0:' ', 1:' ', 2:'"', 3-7:'hello', 8:' ', 9:' — ', 10:' ', 11-15:'world', 16:'"'
        //
        // 'w' in "world" is at UTF-16 position 11, but byte position 13
        // (because — is 3 bytes but 1 UTF-16 code unit)
        //
        // Cursor at UTF-16 13 ('o' in "world"):
        let word = server.word_at_position("file:///test.cyoa", 1, 13);
        assert_eq!(word, Some("world".to_string()));

        // Cursor at UTF-16 12 ('w' in "world"):
        let word2 = server.word_at_position("file:///test.cyoa", 1, 12);
        assert_eq!(word2, Some("world".to_string()));
    }

    #[test]
    fn test_definition_with_multibyte_chars_in_word_line() {
        let mut server = Server::new();
        // Valid CYOA story: stat on ASCII-only line, reference inside event
        // with an em-dash on the same line as the reference.
        let story = "story TestStory:\n  stat hp = 50\n  event start:\n    \"hello \u{2014} ref: {{hp}}\"\n";
        let open_msg = did_open_msg("file:///test.cyoa", story);
        server.handle(open_msg);

        // Line 3: `    "hello \u{2014} ref: {{hp}}"`
        // UTF-16 positions:
        // 0-3: spaces, 4:'"', 5-9:'hello', 10:' ', 11:'\u{2014}', 12:' ', 13-15:'ref', 16:':',
        // 17:' ', 18-19:'{{', 20-21:'hp', 22-23:'}}'
        //
        // 'h' of "hp" (inside {{hp}}) is at UTF-16 position 20
        let word = server.word_at_position("file:///test.cyoa", 3, 21);
        assert_eq!(word, Some("hp".to_string()));

        // find_definition should return the location in UTF-16 offsets
        // The definition `stat hp = 50` is on line 1 (ASCII-only):
        // Byte: 0:' ', 1:' ', 2-5:'stat', 6:' ', 7-8:'hp', 9:' ',...
        // name_start = 7, name_end = 9 (same in UTF-16 since line is ASCII)
        let loc = server.find_definition("file:///test.cyoa", "hp");
        assert!(loc.is_some(), "expected definition for 'hp'");
        if let Some(loc) = loc {
            assert_eq!(loc.range.start.line, 1);
            assert_eq!(loc.range.start.character, 7); // UTF-16 offset of 'h'
            assert_eq!(loc.range.end.character, 9); // UTF-16 offset after 'p'
        }
    }

    #[test]
    fn test_find_definition_emits_utf16_offsets() {
        let mut server = Server::new();
        // Open with valid story so doc.story is Some.
        // We test byte→UTF-16 conversion on a separate line string directly.
        let valid = "story TestStory:\n  stat hp = 50\n  \"{{hp}}\"\n";
        let open_msg = did_open_msg("file:///test.cyoa", valid);
        server.handle(open_msg);

        // Override the document text with em-dash version but keep the story
        // We need to test the byte→UTF-16 conversion path
        // Line with " — stat hp": bytes: 2 spaces, — (3 bytes), space, stat...
        // name_start in bytes = 2 + 3 + 1 + 4 + 1 = 11
        // name_start in UTF-16 = 2 + 1 + 1 + 4 + 1 = 9

        // Directly test the helper on a line with em-dash before 'stat'
        let test_line = "  \u{2014} stat hp = 50";
        let byte_name_start = 2 + 3 + 1 + 4 + 1; // after "  — stat "
        let byte_name_end = byte_name_start + 2; // "hp"
        let (utf16_start, utf16_end) =
            byte_range_to_utf16(test_line, byte_name_start, byte_name_end);
        // In UTF-16: "  " (2) + "—" (1) + " " (1) + "stat" (4) + " " (1) = 9
        assert_eq!(utf16_start, 9);
        assert_eq!(utf16_end, 11); // 9 + 2 (hp is ASCII)
    }
}
