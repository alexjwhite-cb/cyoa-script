//! CYOA CLI — compile, play, and validate stories.
//!
//! Usage:
//!   cyoa compile <story.cyoa>      Compile .cyoa → .cyoa.bc
//!   cyoa play <story.cyoa.bc>      Play in interactive mode
//!   cyoa validate <story.cyoa>     Validate without outputting bytecode
//!   cyoa version                   Print the CLI version

use std::io::{self, Write};

use clap::Parser;

#[derive(Parser)]
#[command(name = "cyoa", version, about = "CYOA DSL compiler and runner")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Compile a .cyoa story to .cyoa.bc bytecode
    Compile {
        /// Input .cyoa file path
        input: String,
    },
    /// Play a compiled story interactively
    Play {
        /// Input .cyoa.bc file path
        input: String,
    },
    /// Validate a .cyoa story without compiling
    Validate {
        /// Input .cyoa file path
        input: String,
    },
    /// Print the version of the cyoa CLI
    Version,
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile { input } => cmd_compile(&input),
        Commands::Play { input } => cmd_play(&input),
        Commands::Validate { input } => cmd_validate(&input),
        Commands::Version => println!("{}", env!("CARGO_PKG_VERSION")),
    }
}

fn cmd_compile(input: &str) {
    eprintln!("Compiling {}...", input);

    let source = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: failed to read input file: {}", e);
            std::process::exit(1);
        }
    };

    let story = match cyoa_compiler::parse_story(&source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error:\n{}", e);
            std::process::exit(1);
        }
    };

    let bytecode = match cyoa_compiler::compile_story(&story) {
        Ok(bc) => bc,
        Err(e) => {
            eprintln!("Compile error: {}", e);
            std::process::exit(1);
        }
    };

    let output_path = format!("{}.bc", input);
    let bytes = match bytecode.to_bytes() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: failed to serialize bytecode: {}", e);
            std::process::exit(1);
        }
    };

    match std::fs::write(&output_path, &bytes) {
        Ok(_) => eprintln!("✓ Compiled to {} ({} bytes)", output_path, bytes.len()),
        Err(e) => {
            eprintln!("Error: failed to write output: {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_play(input: &str) {
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Error: failed to read input file: {}", e);
            std::process::exit(1);
        }
    };

    let bytecode = match cyoa_bytecode::Bytecode::from_bytes(&bytes) {
        Ok(bc) => bc,
        Err(e) => {
            eprintln!("Error: failed to decode bytecode: {}", e);
            std::process::exit(1);
        }
    };

    let mut engine = cyoa_runtime::Engine::new(bytecode);

    println!("{}", "| CYOA Interactive".bold());

    loop {
        // Print current event text (paragraphs separated by blank lines)
        let text = engine.current_event_text();
        for (i, line) in text.iter().enumerate() {
            if i > 0 {
                println!();
            }
            println!("{}", line);
        }

        // Get and display choices
        let choices = engine.current_choices();
        if choices.is_empty() {
            println!("\n[End of story]");
            break;
        }

        for (i, choice) in choices.iter().enumerate() {
            println!("\n  [{}] {}", i, choice);
        }

        // Prompt for input
        print!("\n> ");
        io::stdout().flush().ok();

        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();

        let trimmed = input.trim();
        match trimmed.parse::<usize>() {
            Ok(idx) if idx < choices.len() => {
                let effects = engine.make_choice(idx as i32);
                for line in &effects {
                    println!("{}", line);
                }
                if engine.is_story_complete() {
                    println!("\n[End of story]");
                    break;
                }
            }
            _ => {
                println!("Invalid choice. Enter a number.");
            }
        }
    }
}

fn cmd_validate(input: &str) {
    let source = match std::fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error: failed to read input file: {}", e);
            std::process::exit(1);
        }
    };

    let story = match cyoa_compiler::parse_story(&source) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error:\n{}", e);
            std::process::exit(1);
        }
    };

    // Resolve imports so that symbols from std/ and local imports are recognized
    let input_path = std::path::Path::new(input);
    let base_dir = input_path.parent().unwrap_or(std::path::Path::new("."));
    let std_paths = find_std_dirs(base_dir);
    let story = match cyoa_compiler::resolve_imports(&story, base_dir, &std_paths) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Import error: {}", e);
            std::process::exit(1);
        }
    };

    // Validate that all references (next, uses, stats, flags) are defined
    let ref_errors = cyoa_compiler::validate_references(&story, &source);
    if !ref_errors.is_empty() {
        eprintln!("Validation errors:");
        for err in &ref_errors {
            eprintln!("  line {} col {}: {}", err.line, err.col, err.message);
        }
        std::process::exit(1);
    }

    match cyoa_compiler::compile_story(&story) {
        Ok(_) => println!("✓ Valid: {} compiles successfully", input),
        Err(e) => {
            eprintln!("Compile error: {}", e);
            std::process::exit(1);
        }
    }
}

/// Walk up the directory tree from `base` looking for `std/` directories
/// to use as search roots for `std/` imports.
fn find_std_dirs(base: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    let mut current: Option<&std::path::Path> = Some(base);
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

// ── ANSI styling (no extra dependency needed) ──────────────────────────

trait StyleExt {
    fn bold(&self) -> String;
}

impl StyleExt for str {
    fn bold(&self) -> String {
        format!("\x1b[1m{}\x1b[0m", self)
    }
}
