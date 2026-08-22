# cyoa-runtime

The CYOA virtual machine — a register-based stack machine with `PlayerState`
(stats/flags/tags) and `StoryCursor` (current event + choice history).
Independent of `cyoa-compiler`; the runtime only consumes compiled bytecode.

## Build

```bash
cargo build -p cyoa-runtime
```

## Use as a Rust dependency

```toml
[dependencies]
cyoa-runtime = { path = "cyoa-runtime" }
cyoa-bytecode = { path = "cyoa-bytecode" }
```

```rust
use cyoa_runtime::Engine;
use cyoa_bytecode::Bytecode;

let bytes = std::fs::read("forest_adventure.cyoa.bc")?;
let bc = Bytecode::from_bytes(&bytes)?;
let mut engine = Engine::new(bc);

// Read the current event
println!("Event: {}", engine.current_event_id());
for para in engine.current_event_text() {
    println!("{}", para);
}
let choices = engine.current_choices();
for (i, c) in choices.iter().enumerate() {
    println!("  [{}] {}", i, c);
}

// Make a choice
let effects = engine.make_choice(0);
for text in effects {
    println!("{}", text);
}

// Save / load
let save = engine.get_state_json();
let mut engine2 = Engine::new(bc);
engine2.set_state_json(&save)?;
```

Full API: [docs/rust-api.md](../docs/rust-api.md)
