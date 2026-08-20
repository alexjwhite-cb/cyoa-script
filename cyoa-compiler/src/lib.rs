//! CYOA DSL compiler.
//!
//! Pipeline: parse → merge imports → bytecode codegen.
//!
//! 1. **Parse pass**: Parse `.cyoa` source files into `Story` ASTs (line-based)
//! 2. **Merge pass**: Resolve imports, detect cycles, merge into single AST
//! 3. **Codegen pass**: Emit `.cyoa.bc` bytecode from the merged AST
//!
//! Grammar reference: see `grammar.pest` for the official pest grammar.
//! The Rust parser in `parser.rs` implements equivalent logic with better
//! error messages for writers.

mod codegen;
mod parser;
mod resolver;

pub use codegen::compile_story;
pub use codegen::CodegenError;
pub use parser::parse_story;
pub use parser::ParseError;
pub use resolver::resolve_imports;
pub use resolver::ImportError;

/// Re-export key types for convenience.
pub use cyoa_ast::*;

/// Compile a `.cyoa` source string to bytecode.
///
/// This is a convenience function that parses, resolves imports, and compiles
/// in one step. For import resolution, `base_dir` is used to resolve relative
/// import paths. The `std/` directory is resolved from the `std_paths` map.
pub fn compile(
    source: &str,
    base_dir: &std::path::Path,
    std_paths: &[std::path::PathBuf],
) -> Result<cyoa_bytecode::Bytecode, CompilationError> {
    let story = parse_story(source)?;
    let merged = resolve_imports(&story, base_dir, std_paths)?;
    let bytecode = compile_story(&merged)?;
    Ok(bytecode)
}

/// Combined compilation error.
#[derive(Debug)]
pub enum CompilationError {
    Parse(ParseError),
    Import(ImportError),
    Codegen(CodegenError),
}

impl std::fmt::Display for CompilationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilationError::Parse(e) => write!(f, "parse error: {}", e),
            CompilationError::Import(e) => write!(f, "import error: {}", e),
            CompilationError::Codegen(e) => write!(f, "codegen error: {}", e),
        }
    }
}

impl std::error::Error for CompilationError {}

impl From<ParseError> for CompilationError {
    fn from(e: ParseError) -> Self {
        CompilationError::Parse(e)
    }
}

impl From<ImportError> for CompilationError {
    fn from(e: ImportError) -> Self {
        CompilationError::Import(e)
    }
}

impl From<CodegenError> for CompilationError {
    fn from(e: CodegenError) -> Self {
        CompilationError::Codegen(e)
    }
}

#[cfg(test)]
mod tests;
