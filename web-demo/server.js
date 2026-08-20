#!/usr/bin/env node
/**
 * Simple static file server for the CYOA WASM demo.
 * Usage: node server.js [port]
 */
const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = process.argv[2] || 8080;
const ROOT = path.resolve(__dirname, '..');

const MIME_TYPES = {
  '.html': 'text/html',
  '.js': 'application/javascript',
  '.mjs': 'application/javascript',
  '.css': 'text/css',
  '.wasm': 'application/wasm',
  '.bc': 'application/octet-stream',
  '.json': 'application/json',
  '.d.ts': 'application/typescript',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
};

const server = http.createServer((req, res) => {
  let urlPath = req.url.split('?')[0];
  if (urlPath === '/') urlPath = '/web-demo/index.html';

  const filePath = path.join(ROOT, urlPath);

  // Prevent directory traversal
  if (!filePath.startsWith(ROOT)) {
    res.writeHead(403);
    res.end('Forbidden');
    return;
  }

  fs.readFile(filePath, (err, data) => {
    if (err) {
      res.writeHead(404);
      res.end('Not Found');
      return;
    }

    const ext = path.extname(filePath);
    const mime = MIME_TYPES[ext] || 'application/octet-stream';
    res.writeHead(200, { [mime.includes('javascript') || mime === 'application/wasm' ? 'Content-Type' : 'Content-Type']: mime });
    res.end(data);
  });
});

server.listen(PORT, () => {
  console.log(`CYOA WASM demo server running at http://localhost:${PORT}/`);
  console.log(`  - Web demo: http://localhost:${PORT}/web-demo/`);
  console.log(`  - WASM module: http://localhost:${PORT}/cyoa-wasm/pkg/cyoa_wasm.js`);
  console.log(`  - Bytecode: http://localhost:${PORT}/examples/forest_adventure.cyoa.bc`);
});
