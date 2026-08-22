# cyoa-ast

AST types for the CYOA language — the foundation crate with **no external
dependencies**. Defines the story model shared by the compiler and runtime:
`Story`, `Event`, `Choice`, `Effect`, `StatDef`, `Tag`, `Condition`, and
`Import`.

## Build

```bash
cargo build -p cyoa-ast
```

## Use

```rust
use cyoa_ast::*;
```

## Dependency graph

```
cyoa-ast        (no deps — foundational types)
  ↑
cyoa-bytecode   (deps: postcard, serde)
  ↑
cyoa-compiler   (deps: pest, cyoa-ast, cyoa-bytecode)
cyoa-runtime    (deps: cyoa-ast, cyoa-bytecode, serde_json)
```

`cyoa-ast` is the root of the dependency chain. Both `cyoa-compiler`
(build time) and `cyoa-runtime` (execution) depend on it but not on each
other.

Canonical spec: [SPEC.md](../SPEC.md) · Data model: [docs/rust-api.md](../docs/rust-api.md)
