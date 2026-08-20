//! The CYOA Language Server core logic.
//!
//! Tracks open documents (URI → source text + parsed AST) and responds to
//! LSP requests: diagnostics on open/change, hover info, and basic completion.

use std::collections::HashMap;

use cyoa_ast::{Story, StoryItem};
use cyoa_compiler::parse_story;

use crate::diagnostics::{diagnostics_from_parse_error, format_error_message};
use crate::protocol::*;

pub struct Server {
    /// Open documents: URI → (source text, parsed story, parse error if any)
    documents: HashMap<String, DocumentState>,
}

struct DocumentState {
    #[allow(dead_code)]
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
        _line: u32,
        _character: u32,
        id: Option<RequestId>,
    ) -> Vec<Response> {
        let doc = match self.documents.get(uri) {
            Some(d) => d,
            None => return self.empty_response(id),
        };

        let hover_content = if let Some(story) = doc.story.as_ref() {
            let mut content = format!("**Story**: `{}`\n\n", story.name);

            // Story-level tags
            if !story.tags.is_empty() {
                content.push_str(&format!("**Tags**: {}\n\n", story.tags.join(", ")));
            }

            // Stats
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

            // Flags
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

            // Effects
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

            // Events
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

            content
        } else if let Some(err) = &doc.error {
            format_error_message(err)
        } else {
            "No information available".to_string()
        };

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

        // Now hover
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
}
