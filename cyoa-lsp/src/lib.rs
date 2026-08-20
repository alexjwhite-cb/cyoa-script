//! Language Server Protocol for the CYOA DSL.
//!
//! Provides syntax diagnostics and hover info for editors (VS Code, Neovim)
//! editing `.cyoa` files.
//!
//! The server runs over stdio as a JSON-RPC server:
//! - Receives `textDocument/didOpen` / `didChange` → parses and validates
//! - Publishes diagnostics (parse errors) back to the client
//! - Responds to `textDocument/hover` with element info
//! - Responds to `textDocument/completion` with stat/flag/event names

mod diagnostics;
mod protocol;
mod server;

pub use cyoa_compiler::{parse_story, ParseError};
pub use diagnostics::diagnostics_from_parse_error;
pub use protocol::{RawMessage, Request, Response};
pub use server::Server;
