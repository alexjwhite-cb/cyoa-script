//! Convert parse errors into LSP diagnostics.

use crate::protocol::{Diagnostic, DiagnosticSeverity, Position, Range};
use cyoa_compiler::ParseError;

/// Extract the line and column from a `ParseError`.
fn extract_line_col(err: &ParseError) -> (usize, usize) {
    (err.line, err.col)
}

/// Convert a single `ParseError` into an LSP `Diagnostic`.
pub fn diagnostics_from_parse_error(err: &ParseError) -> Diagnostic {
    let (line, col) = extract_line_col(err);

    Diagnostic {
        range: Some(Range {
            start: Position {
                line: (line.saturating_sub(1)) as u32,
                character: (col.saturating_sub(1)) as u32,
            },
            end: Position {
                line: (line.saturating_sub(1)) as u32,
                character: (col.saturating_sub(1) + 10) as u32, // highlight ~10 chars
            },
        }),
        severity: Some(DiagnosticSeverity::Error),
        code: None,
        source: Some("cyoa-lsp".to_string()),
        message: err.message.clone(),
    }
}

/// Convert the message text from a parse error into a diagnostic message
/// that includes line/col info.
pub fn format_error_message(err: &ParseError) -> String {
    format!("{} (line {}, col {})", err.message, err.line, err.col)
}
