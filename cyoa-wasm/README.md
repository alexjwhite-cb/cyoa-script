# cyoa-wasm

WASM bindings for the CYOA engine, enabling use in web pages, browser games,
Unity WebGL, and Godot WebAssembly builds. Exported as an ES module via
`wasm-bindgen`.

## Build

```bash
# Raw .wasm (from project root)
cargo build -p cyoa-wasm --target wasm32-unknown-unknown

# JS bindings (requires wasm-pack)
wasm-pack build cyoa-wasm --target web --out-dir cyoa-wasm/pkg
```

Output: `cyoa_wasm.js` + `cyoa_wasm_bg.wasm` + TypeScript declarations.

## Usage (JavaScript / TypeScript)

```typescript
import init, { WasmStoryCatalog, WasmEngine } from './cyoa_wasm.js';
await init();

// Register stories from compiled .cyoa.bc bytecode
const catalog = new WasmStoryCatalog();
const resp = await fetch('./forest_adventure.cyoa.bc');
catalog.register(new Uint8Array(await resp.arrayBuffer()));

// Discover by tag
catalog.storiesWithTag("fantasy");          // [{ name, tags }]
catalog.storiesWithAllTags('["fantasy","exploration"]');

// Create and play
const engine = catalog.createEngineByName("ForestAdventure");
const event = engine.getCurrentEvent();
// { id, text: [...], choices: [...] }

engine.makeChoice(0);                       // { effectText, gameStateChanged }

// Zero-copy text (Uint8Array view into WASM linear memory)
const bytes = engine.currentEventTextBytes();
const text = new TextDecoder().decode(bytes);

// Save / load
engine.getStateJson();
engine.setStateJson(json);
```

Type declarations: [`cyoa.d.ts`](../web-demo/cyoa.d.ts) ·
Full API: [`docs/wasm-api.md`](../docs/wasm-api.md) ·
Web demo: [`web-demo/`](../web-demo/)
