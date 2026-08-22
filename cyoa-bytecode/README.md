# cyoa-bytecode

Binary bytecode format and serialization for compiled `.cyoa.bc` files.
Uses `postcard` for compact, zero-copy deserialization.

## Binary layout

`.cyoa.bc` is a single flat binary structured for zero-copy loading:

```
┌──────────────┐
│ Header       │  magic, version, section offsets
├──────────────┤
│ String Table │  ← all text deduplicated, interned
├──────────────┤
│ Event Index  │  event_id → offset mapping
├──────────────┤
│ Events       │  array of fixed-size event headers
├──────────────┤
│ Choices      │  choice arrays (indexed by event)
├──────────────┤
│ Effect Blocks│  reusable effects
├──────────────┤
│ Conditions   │  prerequisite tree bytecode
└──────────────┘
```

The VM returns `&str` pointing directly into the loaded bytecode — no
allocation, no copy.

## Build

```bash
cargo build -p cyoa-bytecode
```

## Use

```rust
use cyoa_bytecode::Bytecode;

let bytes = std::fs::read("forest_adventure.cyoa.bc")?;
let bc = Bytecode::from_bytes(&bytes)?;

std::fs::write("story.cyoa.bc", bc.to_bytes()?)?;
```

Binary format details: [SPEC.md](../SPEC.md#bytecode-format) ·
API reference: [docs/rust-api.md](../docs/rust-api.md)
