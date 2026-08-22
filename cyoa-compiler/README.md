# cyoa-compiler

Pest grammar, parser, and bytecode codegen for the CYOA language. All
`.cyoa` imports are resolved at compile time into a single self-contained
`.cyoa.bc` file.

## Build

```bash
cargo build -p cyoa-compiler
```

## Compile a story

```bash
# From the project root:
cargo run -p cyoa-cli -- compile examples/forest_adventure.cyoa
# → produces examples/forest_adventure.cyoa.bc
```

## Use as a Rust dependency

```toml
[dependencies]
cyoa-compiler = { path = "cyoa-compiler" }
cyoa-bytecode = { path = "cyoa-bytecode" }
```

```rust
use cyoa_compiler::Compiler;

let mut compiler = Compiler::new();
let bytecode = compiler.compile_file("forest_adventure.cyoa")?;
std::fs::write("forest_adventure.cyoa.bc", bytecode.to_bytes()?)?;
```

Independent of `cyoa-runtime` — compilation is a build-time step, execution
is separate.

Grammar: [SPEC.md](../SPEC.md) ·
Compiler API: [docs/rust-api.md](../docs/rust-api.md)
