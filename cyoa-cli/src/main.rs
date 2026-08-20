//! CYOA CLI — compile, play, and validate stories.
//!
//! Usage:
//!   cyoa compile <story.cyoa>      Compile .cyoa → .cyoa.bc
//!   cyoa play <story.cyoa.bc>      Play in interactive mode
//!   cyoa validate <story.cyoa>     Validate without outputting bytecode

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
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Compile { input } => cmd_compile(&input),
        Commands::Play { input } => cmd_play(&input),
        Commands::Validate { input } => cmd_validate(&input),
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
        // Print current event text
        let text = engine.current_event_text();
        for line in &text {
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

    match cyoa_compiler::parse_story(&source) {
        Ok(story) => match cyoa_compiler::compile_story(&story) {
            Ok(_) => println!("✓ Valid: {} compiles successfully", input),
            Err(e) => {
                eprintln!("Compile error: {}", e);
                std::process::exit(1);
            }
        },
        Err(e) => {
            eprintln!("Parse error:\n{}", e);
            std::process::exit(1);
        }
    }
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
