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
const TT_KEYWORD: u32 = 0;
const TT_STRING: u32 = 1;
const TT_COMMENT: u32 = 2;
const TT_TYPE: u32 = 3;
const TT_PARAMETER: u32 = 4;
const TT_NUMBER: u32 = 5;
const TT_OPERATOR: u32 = 6;

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
///
/// Per the LSP specification, a position inside a multibyte character is
/// interpreted as the start of that character.
fn byte_to_utf16_idx(line: &str, byte_pos: usize) -> u32 {
    let mut utf16_count = 0u32;
    for (byte_idx, ch) in line.char_indices() {
        if byte_idx >= byte_pos {
            return utf16_count;
        }
        let char_end = byte_idx + ch.len_utf8();
        if byte_pos < char_end {
            // byte_pos falls inside this multibyte character;
            // snap to its UTF-16 start position per LSP spec.
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
        let id = msg.id.clone();
        let Some(req) = Request::from_raw(msg) else {
            return vec![];
        };

        match req {
            Request::Initialize => self.handle_initialize(id),
            Request::Shutdown => self.handle_shutdown(id),
            Request::DidOpen { uri, text } => self.handle_document_update(uri, text),
            Request::DidChange { uri, text } => self.handle_document_update(uri, text),
            Request::DidClose { uri } => self.handle_did_close(uri),
            Request::Hover {
                uri,
                line,
                character,
            } => self.handle_hover(&uri, line, character, id),
            Request::Completion {
                uri,
                line,
                character,
            } => self.handle_completion(&uri, line, character, id),
            Request::Definition {
                uri,
                line,
                character,
            } => self.handle_definition(&uri, line, character, id),
            Request::OnTypeFormatting {
                uri,
                line,
                character,
                ch,
                options,
            } => self.handle_on_type_formatting(&uri, line, character, &ch, &options, id),
            Request::SemanticTokens { uri } => self.handle_semantic_tokens(&uri, id),
            Request::SemanticTokensRange { uri, range } => {
                self.handle_semantic_tokens_range(&uri, &range, id)
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
                    "tokenTypes": ["keyword", "string", "comment", "type", "parameter", "number", "operator"],
                    "tokenModifiers": []
                },
                "full": true,
                "range": true,
                "formats": ["relative"]
            }
        });

        self.json_response(
            id,
            serde_json::json!({
                "capabilities": capabilities
            }),
        )
    }

    fn handle_shutdown(&self, id: Option<RequestId>) -> Vec<Response> {
        self.empty_response(id)
    }

    // ── Document handlers ────────────────────────────────────────────────────

    fn handle_document_update(&mut self, uri: String, text: String) -> Vec<Response> {
        let diagnostics = self.parse_and_store(uri.clone(), text);
        self.publish_diagnostics(&uri, diagnostics)
    }

    fn handle_did_close(&mut self, uri: String) -> Vec<Response> {
        self.documents.remove(&uri);
        // Clear diagnostics on close
        self.publish_diagnostics(&uri, vec![])
    }

    fn handle_hover(
        &self,
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
            return self.json_response(
                id,
                serde_json::json!({
                    "contents": {
                        "kind": "markdown",
                        "value": hover_content,
                    }
                }),
            );
        }

        // Build story-level metadata (shown when cursor is not over a specific symbol)
        let story_metadata = if let Some(story) = doc.story.as_ref() {
            let mut content = format!("**Story**: `{}`\n\n", story.name);

            if !story.tags.is_empty() {
                content.push_str(&format!("**Tags**: {}\n\n", story.tags.join(", ")));
            }

            // Helper to collect names of a specific StoryItem variant.
            let collect_names = |variant: fn(&StoryItem) -> Option<&str>| -> Vec<&str> {
                story.items.iter().filter_map(variant).collect()
            };

            // List all stat definitions
            let stats = collect_names(|item| match item {
                StoryItem::StatDef(s) => Some(s.name.as_str()),
                _ => None,
            });
            if !stats.is_empty() {
                content.push_str(&format!("**Stats**: {}\n\n", stats.join(", ")));
            }

            // List all flag definitions
            let flags = collect_names(|item| match item {
                StoryItem::FlagDef(f) => Some(f.name.as_str()),
                _ => None,
            });
            if !flags.is_empty() {
                content.push_str(&format!("**Flags**: {}\n\n", flags.join(", ")));
            }

            // List all effect definitions
            let effects = collect_names(|item| match item {
                StoryItem::EffectDef(e) => Some(e.name.as_str()),
                _ => None,
            });
            if !effects.is_empty() {
                content.push_str(&format!("**Effects**: {}\n\n", effects.join(", ")));
            }

            // List all event ids
            let events = collect_names(|item| match item {
                StoryItem::EventDef(e) => Some(e.id.as_str()),
                _ => None,
            });
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

        self.json_response(
            id,
            serde_json::json!({
                "contents": {
                    "kind": "markdown",
                    "value": hover_content
                }
            }),
        )
    }

    fn handle_completion(
        &self,
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

        // Collect completions in a single pass over story items.
        for item in &story.items {
            match item {
                StoryItem::EventDef(e) => items.push(CompletionItem {
                    label: e.id.clone(),
                    kind: Some(CompletionItemKind::Class),
                    detail: Some("event".to_string()),
                    documentation: None,
                }),
                StoryItem::StatDef(s) => items.push(CompletionItem {
                    label: s.name.clone(),
                    kind: Some(CompletionItemKind::Variable),
                    detail: Some(format!("stat = {}", s.default)),
                    documentation: None,
                }),
                StoryItem::FlagDef(f) => items.push(CompletionItem {
                    label: f.name.clone(),
                    kind: Some(CompletionItemKind::Field),
                    detail: Some(format!("flag = {}", f.default)),
                    documentation: None,
                }),
                StoryItem::EffectDef(e) => items.push(CompletionItem {
                    label: e.name.clone(),
                    kind: Some(CompletionItemKind::Function),
                    detail: Some("effect".to_string()),
                    documentation: None,
                }),
                _ => {}
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

        self.json_response(
            id,
            serde_json::json!({
                "isIncomplete": false,
                "items": items
            }),
        )
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn parse_and_store(&mut self, uri: String, text: String) -> Vec<Diagnostic> {
        let result = parse_story(&text);
        let (story, error) = match result {
            Ok(s) => (Some(s), None),
            Err(e) => (None, Some(e)),
        };

        // Compute diagnostics before moving error into DocumentState.
        let diagnostics = match &error {
            Some(err) => vec![diagnostics_from_parse_error(err)],
            None => vec![],
        };

        // If the story has imports, resolve them so that go-to-definition,
        // hover, and completion work for imported symbols (effects, stats,
        // flags, events from std/ or local files).
        let (story, imported_files) = self.resolve_story_imports(&uri, story);

        self.documents.insert(
            uri,
            DocumentState {
                text,
                story,
                error,
                imported_files,
            },
        );

        diagnostics
    }

    /// Resolve imports for a story if it has any, falling back to the
    /// unmerged story on error (so main-file definitions still work).
    /// Returns the (possibly merged) story and a list of imported file
    /// paths + their text contents (for go-to-definition support).
    fn resolve_story_imports(
        &self,
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
            Err(_) => (Some(story.clone()), Vec::new()),
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
        self.json_response(id, serde_json::json!({
            "uri": loc.uri,
            "range": {
                "start": { "line": loc.range.start.line, "character": loc.range.start.character },
                "end":   { "line": loc.range.end.line,   "character": loc.range.end.character },
            }
        }))
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
            return self.json_response(
                id,
                serde_json::json!([{
                    "range": {
                        "start": { "line": line, "character": start_utf16 },
                        "end":   { "line": line, "character": character }
                    },
                    "newText": spaces
                }]),
            );
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
        let token_refs: Vec<&SemanticToken> = tokens.iter().collect();
        // For full document requests, line deltas are relative to line 0
        self.do_semantic_tokens(&token_refs, &lines, 0, id)
    }

    fn handle_semantic_tokens_range(
        &self,
        uri: &str,
        range: &Range,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return self.empty_response(id),
        };

        let tokens = tokenize_semantic(&doc.text);
        let lines: Vec<&str> = doc.text.lines().collect();

        // Filter tokens to those whose line falls within the requested range.
        // LSP ranges are half-open: [start.line, end.line).
        let filtered: Vec<&SemanticToken> = tokens
            .iter()
            .filter(|t| t.line >= range.start.line && t.line < range.end.line)
            .collect();

        // For range requests, the LSP spec requires the first token's line delta
        // to be relative to range.start.line, not 0. This is critical for GoLand
        // which sends semanticTokens/range requests for viewport highlighting.
        self.do_semantic_tokens(&filtered, &lines, range.start.line, id)
    }

    /// Shared helper that encodes tokens and wraps them in a JSON-RPC response.
    fn do_semantic_tokens(
        &self,
        tokens: &[&SemanticToken],
        lines: &[&str],
        start_line: u32,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        let data = encode_semantic_tokens(tokens, lines, start_line);
        self.json_response(id, serde_json::json!({ "data": data }))
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

    /// Create a JSON-RPC success response with the given result.
    fn json_response(&self, id: Option<RequestId>, result: serde_json::Value) -> Vec<Response> {
        vec![Response::Response {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
        }]
    }

    fn empty_response(&self, id: Option<RequestId>) -> Vec<Response> {
        self.json_response(id, serde_json::Value::Null)
    }

    fn empty_completion(&self, id: Option<RequestId>) -> Vec<Response> {
        self.json_response(
            id,
            serde_json::json!({
                "isIncomplete": false,
                "items": []
            }),
        )
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
            let keyword_boundary_ok =
                after.is_none() || after.unwrap().is_whitespace() || after.unwrap() == ':';
            if !keyword_boundary_ok {
                continue;
            }
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
                    let (start_utf16, end_utf16) = byte_range_to_utf16(line, name_start, name_end);
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
            let full_no_ext = std_base.join(rel);
            if full_no_ext.exists() {
                return Some(full_no_ext);
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

/// Encode a list of `SemanticToken`s into the LSP relative number-array format.
///
/// - `start_line`: the baseline line for delta computation. For `semanticTokens/full`
///   requests this is `0` (LSP spec: first token's line is relative to the document start).
///   For `semanticTokens/range` requests this is `range.start.line` (LSP spec: first
///   token's line is relative to the range start).
/// - `line` (encoded): delta from previous token's line (or from `start_line` for
///   the first token).
/// - `startChar` (encoded): if same line as prev, offset from prev token's end;
///   if different line, absolute char position on that line.
///
/// Byte offsets from `tokenize_semantic` are converted to UTF-16 code unit
/// offsets as required by the LSP specification.
fn encode_semantic_tokens(
    tokens: &[&SemanticToken],
    lines: &[&str],
    start_line: u32,
) -> Vec<serde_json::Value> {
    let mut data: Vec<serde_json::Value> = Vec::new();
    let mut prev_line: u32 = start_line;
    let mut prev_end_utf16: u32 = 0;

    for t in tokens {
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

    data
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
            // Skip leading whitespace on continuation lines — the string token
            // should start at the first non-whitespace character, consistent
            // with single-line string tokens that begin at the opening `"`.
            let mut start = 0usize;
            while start < bytes.len() && bytes[start].is_ascii_whitespace() {
                start += 1;
            }
            let mut end = start;

            i = start;
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

            if end > start {
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
            Response::Response { result, .. } => {
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
    fn test_hover_at_various_positions() {
        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // (label, line, character, expected_substrings)
        let cases: &[(&str, u32, u32, &[&str])] = &[
            ("keyword", 0, 0, &["keyword", "story"]),
            ("stat", 2, 8, &["stat", "hp", "50"]),
            (
                "story_metadata",
                0,
                7,
                &[
                    "TestStory",
                    "fantasy",
                    "hp",
                    "visited_cave",
                    "found_item",
                    "start",
                    "cave",
                ],
            ),
        ];

        for (label, line, character, expected) in cases {
            let hover_msg = request_msg(
                "textDocument/hover",
                serde_json::json!({
                    "textDocument": {"uri": "file:///test.cyoa"},
                    "position": {"line": line, "character": character}
                }),
            );

            let responses = server.handle(hover_msg);
            assert_eq!(
                responses.len(),
                1,
                "hover for {}: expected 1 response",
                label
            );
            match &responses[0] {
                Response::Response { result, .. } => {
                    let json = result.as_ref().unwrap();
                    let value = json["contents"]["value"].as_str().unwrap();
                    for substr in *expected {
                        assert!(
                            value.contains(substr),
                            "hover for {}: expected '{}' in '{}'",
                            label,
                            substr,
                            value
                        );
                    }
                }
                _ => panic!("expected Response for {}", label),
            }
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
            Response::Response { result, .. } => {
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
    fn test_definition_for_various_symbols() {
        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", VALID_STORY);
        server.handle(open_msg);

        // (label, line, character, expected_start_line, expected_end_line)
        let cases: &[(&str, u32, u32, u32, u32)] = &[
            ("stat", 2, 9, 2, 2),     // "stat hp = 50" — jump to stat definition
            ("effect", 10, 12, 4, 4), // "uses found_item" — jump to effect definition
        ];

        for (label, line, character, expected_start_line, expected_end_line) in cases {
            let def_msg = request_msg(
                "textDocument/definition",
                serde_json::json!({
                    "textDocument": {"uri": "file:///test.cyoa"},
                    "position": {"line": line, "character": character}
                }),
            );

            let responses = server.handle(def_msg);
            assert_eq!(
                responses.len(),
                1,
                "definition for {}: expected 1 response",
                label
            );
            match &responses[0] {
                Response::Response { result, .. } => {
                    let json = result.as_ref().unwrap();
                    assert!(
                        json["uri"].is_string(),
                        "definition for {}: expected uri",
                        label
                    );
                    let range = &json["range"];
                    let start = &range["start"];
                    assert_eq!(
                        start["line"].as_u64().unwrap(),
                        *expected_start_line as u64,
                        "definition for {}: start line",
                        label
                    );
                    let end = &range["end"];
                    assert_eq!(
                        end["line"].as_u64().unwrap(),
                        *expected_end_line as u64,
                        "definition for {}: end line",
                        label
                    );
                    assert!(
                        end["character"].as_u64().unwrap() > start["character"].as_u64().unwrap(),
                        "definition for {}: range should be non-empty",
                        label
                    );
                }
                _ => panic!("expected Response for {}", label),
            }
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
                let has_keyword = (0..data.len())
                    .step_by(5)
                    .any(|i| data[i + 3].as_u64().unwrap() == TT_KEYWORD as u64);
                assert!(has_keyword, "expected at least one keyword token");
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_semantic_tokens_capabilities() {
        // The legend (tokenTypes), formats, full, and range are all
        // advertised in the initialize response.
        let mut server = Server::new();
        let init_msg = request_msg("initialize", serde_json::json!({}));

        let responses = server.handle(init_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let json = result.as_ref().unwrap();
                let opts = &json["capabilities"]["semanticTokensOptions"];

                // Check legend tokenTypes
                let legend = opts["legend"]["tokenTypes"].as_array().unwrap();
                let legend_str: Vec<&str> = legend.iter().map(|v| v.as_str().unwrap()).collect();
                assert!(legend_str.contains(&"keyword"));
                assert!(legend_str.contains(&"string"));
                assert!(legend_str.contains(&"comment"));

                // Check formats
                let formats = opts["formats"].as_array().unwrap();
                let fmt_str: Vec<&str> = formats.iter().map(|v| v.as_str().unwrap()).collect();
                assert!(
                    fmt_str.contains(&"relative"),
                    "semanticTokensOptions should include \"formats\": [\"relative\"]"
                );

                // Check full and range support
                assert_eq!(opts["full"], true);
                assert_eq!(opts["range"], true);
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

                // Token type 1 = string (TT_STRING)
                // Flat array: each token is 5 consecutive numbers
                let string_tokens: Vec<_> = (0..data.len())
                    .step_by(5)
                    .filter(|&i| data[i + 3].as_u64().unwrap() == TT_STRING as u64)
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
    fn test_utf16_byte_conversion_with_multibyte() {
        // "a\u{2014}b" = 5 bytes (a=1, em-dash=3, b=1), 3 UTF-16 code units
        let line = "a\u{2014}b";
        // Shared table: (utf16_pos, byte_pos) pairs
        let cases = [(0, 0), (1, 1), (2, 4), (3, 5)];
        for (utf16_pos, byte_pos) in cases {
            assert_eq!(
                utf16_to_byte_idx(line, utf16_pos),
                byte_pos,
                "utf16→byte at UTF-16 pos {}",
                utf16_pos
            );
            assert_eq!(
                byte_to_utf16_idx(line, byte_pos),
                utf16_pos,
                "byte→utf16 at byte pos {}",
                byte_pos
            );
        }
        // byte_to_utf16: positions inside a multibyte char snap to its start
        // (per LSP spec: a position inside a character is interpreted as the
        // start of that character)
        assert_eq!(byte_to_utf16_idx(line, 2), 1); // byte within '—' → start of '—'
        assert_eq!(byte_to_utf16_idx(line, 3), 1); // last byte of '—' → start of '—'
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
                    .filter(|&i| data[i + 3].as_u64().unwrap() == TT_STRING as u64)
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
    fn test_semantic_tokens_range_filters_and_uses_range_relative_deltas() {
        // LSP spec: for semanticTokens/range requests:
        // 1. Line deltas must be relative to range.start.line, not 0
        // 2. Tokens must be filtered to only those within the requested range
        // This is critical for GoLand which sends range requests for viewport
        // highlighting. Without the delta fix, GoLand receives tokens with
        // deltas relative to 0, interprets them as out-of-range, and drops
        // all semantic tokens — falling back to default lexer highlighting
        // which only colors the bare `"` character.
        let story = "story TestStory:\n  event town_square:\n    \"First line of string.\n    Second line of string.\"\n    choice \"Tavern\":\n      next tavern\n";

        let mut server = Server::new();
        let open_msg = did_open_msg("file:///test.cyoa", story);
        server.handle(open_msg);

        // Request range covering lines 2-4 (0-indexed), which includes both
        // lines of the multi-line string.
        let range_msg = request_msg(
            "textDocument/semanticTokens/range",
            serde_json::json!({
                "textDocument": {"uri": "file:///test.cyoa"},
                "range": {
                    "start": {"line": 2, "character": 0},
                    "end": {"line": 4, "character": 100},
                }
            }),
        );
        let responses = server.handle(range_msg);
        assert_eq!(responses.len(), 1);
        match &responses[0] {
            Response::Response { result, .. } => {
                let data = result.as_ref().unwrap()["data"].as_array().unwrap();

                // Decode the first token's line delta
                let first_line_delta = data[0].as_u64().unwrap() as u32;
                // LSP spec: line delta is relative to range.start.line (line 2)
                let first_line = 2 + first_line_delta;
                assert_eq!(
                    first_line, 2,
                    "first token line should be relative to range.start.line (2), got delta={} → line={}",
                    first_line_delta, first_line
                );

                // Decode all line numbers from the relative encoding and verify
                // no tokens fall outside the requested range (lines 2-4).
                // First delta is relative to range.start.line (2); subsequent
                // deltas are relative to the previous token's line.
                let mut current_line: u32 = 2;
                for i in (0..data.len()).step_by(5) {
                    let line_delta = data[i].as_u64().unwrap() as u32;
                    current_line += line_delta;
                    assert!(
                        current_line >= 2 && current_line <= 4,
                        "token on line {} is outside requested range [2, 4]",
                        current_line
                    );
                }

                // Should have at least one string token (type=TT_STRING)
                let string_tokens: Vec<_> = (0..data.len())
                    .step_by(5)
                    .filter(|&i| data[i + 3].as_u64().unwrap() == TT_STRING as u64)
                    .collect();
                assert!(
                    !string_tokens.is_empty(),
                    "expected at least one string token in range covering both lines"
                );
            }
            _ => panic!("expected Response"),
        }
    }

    #[test]
    fn test_multiline_string_continuation_starts_at_first_non_whitespace() {
        // Multi-line string: the continuation line should start at the first
        // non-whitespace character, not at position 0.
        // Line 1: `  "hello` — opening quote at byte offset 3
        // Line 2: `  world"` — continuation, should start at byte offset 2 (start of "world")
        let story = "story TestStory:\n  \"hello\n  world\"\n";

        let tokens = tokenize_semantic(story);
        let lines: Vec<&str> = story.lines().collect();

        // Find string tokens on line 2 (the continuation)
        let line2_string: Vec<_> = tokens
            .iter()
            .filter(|t| t.line == 2 && t.token_type == TT_STRING)
            .collect();
        assert_eq!(
            line2_string.len(),
            1,
            "expected exactly one string token on continuation line"
        );

        let t = line2_string[0];
        // The token should start at position 2 (after leading whitespace), not 0
        assert_eq!(
            t.start_char, 2,
            "continuation string token should start at first non-whitespace char (2), not 0"
        );
        // The token length should cover from position 2 to the closing quote
        let line_str = &lines[2];
        let token_end_byte = (t.start_char + t.length) as usize;
        let token_content = &line_str[token_end_byte - 1..token_end_byte]; // last char should be "
        assert_eq!(token_content, "\"", "token should end at the closing quote");
    }
}
