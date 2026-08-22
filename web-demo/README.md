# CYOA WASM Web Demo

A working demo of the CYOA engine compiled to WebAssembly. Loads two stories
(`forest_adventure.cyoa` and `tavern_tales.cyoa`) into a `StoryCatalog`,
filters them by tags, and plays them with zero-copy text rendering.

A live version is deployed to
[GitHub Pages](https://cyoa-script.github.io/web-demo/).

## Prerequisites

The demo requires pre-built WASM artifacts. From the project root:

```bash
wasm-pack build cyoa-wasm --target web --out-dir cyoa-wasm/pkg
```

This produces `cyoa-wasm/pkg/cyoa_wasm.js` and `cyoa_wasm_bg.wasm`, which the
demo loads at runtime.

## Run locally

### Option 1: Node.js server (recommended)

```bash
node web-demo/server.js [port]
# Default port: 8080
# Open http://localhost:8080/web-demo/
```

The server (`web-demo/server.js`) serves the entire project root so the demo
can load the WASM module from `../cyoa-wasm/pkg/` and story bytecode from
`../examples/`.

### Option 2: Python static server

```bash
python3 -m http.server 8080 --directory .
# Open http://localhost:8080/web-demo/
```

### Option 3: Any static file server

Serve the project root and open `/web-demo/`. Ensure your server:
- Serves `.wasm` files with MIME type `application/wasm`
- Allows cross-origin requests from the same origin (or use a local server)

## Demo features

- **Story catalog**: tag-based story discovery and filtering
- **Zero-copy benchmark**: live comparison of serde (high-level) vs
  `Uint8Array` (zero-copy) text access — both read from WASM linear memory
- **Stats panel**: current stats, story tags (static metadata), and runtime tags
- **Full gameplay**: make choices, see effect text, track stats in real time

## Files

| File | Purpose |
|------|---------|
| [`index.html`](index.html) | Demo UI + WASM loading + gameplay logic |
| [`server.js`](server.js) | Optional Node.js static file server |
| [`cyoa.d.ts`](cyoa.d.ts) | TypeScript declarations for the WASM API |

WASM API reference: [docs/wasm-api.md](../docs/wasm-api.md)
