//! Entry point for the `cyoa-lsp` binary.
//!
//! Runs the CYOA Language Server over stdio (JSON-RPC).
//! The server provides syntax diagnostics and hover info for `.cyoa` files.
//!
//! Implements proper LSP message framing: Content-Length headers on both
//! input and output, as required by the Language Server Protocol.

use std::io::{self, BufRead, Write};
use std::str;

use cyoa_lsp::Server;

/// Read a single LSP message from stdin using Content-Length framing.
/// Returns the parsed JSON message as a string, or None on EOF.
fn read_message<R: BufRead>(reader: &mut R) -> Option<String> {
    let mut content_length: Option<usize> = None;

    // Read headers until we hit a blank line
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => return None, // EOF
            Ok(_) => {}
            Err(_) => return None,
        }

        // LSP headers are terminated by \r\n, and the line includes the \n
        // Strip trailing \r\n
        let line_trimmed = line.trim_end_matches("\r\n").trim_end_matches('\n');

        if line_trimmed.is_empty() {
            // Blank line — end of headers
            break;
        }

        // Parse Content-Length header
        if line_trimmed
            .to_ascii_lowercase()
            .starts_with("content-length:")
        {
            let value = line_trimmed.split(':').nth(1)?.trim();
            content_length = Some(value.parse::<usize>().ok()?);
        }
        // Ignore other headers (Content-Type, etc.)
    }

    // Now read the message body
    let len = content_length?;
    if len == 0 {
        return None;
    }
    let mut buffer = vec![0u8; len];
    reader.read_exact(&mut buffer).ok()?;

    // Clean the body: strip BOM, null bytes, and trim whitespace.
    // Some clients (e.g. GoLand) may include trailing \r\n in Content-Length,
    // send a UTF-8 BOM, or include null bytes.
    let body = clean_message_body(&buffer);

    if body.is_empty() {
        return None;
    }

    Some(body)
}

/// Clean a raw message body: strip UTF-8 BOM, null bytes, and trim whitespace.
fn clean_message_body(buffer: &[u8]) -> String {
    // Strip UTF-8 BOM if present (0xEF 0xBB 0xBF)
    let bytes = if buffer.starts_with(&[0xEF, 0xBB, 0xBF]) {
        &buffer[3..]
    } else {
        buffer
    };

    // Decode as UTF-8
    let s = match str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };

    // Remove null bytes (some clients may include them)
    let cleaned: String = s.chars().filter(|c| *c != '\0').collect();

    // Trim whitespace (handles trailing \r\n included in Content-Length)
    cleaned.trim().to_string()
}

/// Attempt to parse just the first JSON value from a string that may have
/// trailing characters. This handles cases where Content-Length included
/// extra bytes (e.g. from the next message or trailing data).
fn try_recover_json(s: &str) -> Option<String> {
    let mut de = serde_json::Deserializer::from_str(s).into_iter::<serde_json::Value>();
    if let Some(Ok(value)) = de.next() {
        serde_json::to_string(&value).ok()
    } else {
        None
    }
}

/// Write an LSP response with proper Content-Length framing.
fn write_message<W: Write>(writer: &mut W, json: &str) -> io::Result<()> {
    let bytes = json.as_bytes();
    write!(writer, "Content-Length: {}\r\n\r\n", bytes.len())?;
    writer.write_all(bytes)?;
    writer.flush()
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut out = stdout.lock();

    let mut server = Server::new();

    while let Some(message) = read_message(&mut reader) {
        let responses = match serde_json::from_str::<cyoa_lsp::RawMessage>(&message) {
            Ok(msg) => server.handle(msg),
            Err(_) => {
                // Attempt recovery for "trailing characters" errors by parsing
                // just the first JSON value (handles Content-Length overcount)
                if let Some(recovered) = try_recover_json(&message) {
                    match serde_json::from_str::<cyoa_lsp::RawMessage>(&recovered) {
                        Ok(msg) => server.handle(msg),
                        Err(_) => continue,
                    }
                } else {
                    continue;
                }
            }
        };

        for resp in &responses {
            if let Ok(json) = serde_json::to_string(resp) {
                let _ = write_message(&mut out, &json);
            }
        }
    }
}
