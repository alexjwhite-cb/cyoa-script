//! Entry point for the `cyoa-lsp` binary.
//!
//! Runs the CYOA Language Server over stdio (JSON-RPC).
//! The server provides syntax diagnostics and hover info for `.cyoa` files.

use std::io::{self, BufRead, Write};

use cyoa_lsp::Server;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut server = Server::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.is_empty() {
            continue;
        }

        let responses = match serde_json::from_str::<cyoa_lsp::RawMessage>(&line) {
            Ok(msg) => server.handle(msg),
            Err(e) => {
                eprintln!("cyoa-lsp: failed to parse message: {}", e);
                continue;
            }
        };

        for resp in &responses {
            if let Ok(json) = serde_json::to_string(resp) {
                writeln!(out, "{}", json).ok();
            }
        }
        out.flush().ok();
    }
}
