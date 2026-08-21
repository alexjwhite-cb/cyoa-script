#!/usr/bin/env node
/**
 * Build the CYOA documentation site.
 *
 * Reads markdown files (README.md + docs/*.md) and wraps each in the
 * docs-site/layout.html template, producing HTML pages in _site/docs/.
 *
 * Requires the `marked` npm package: `npm install -g marked`
 *
 * Usage:
 *   node docs-site/build-site.js [--output-dir _site/docs]
 */

const fs = require('fs');
const path = require('path');

// --- CLI args ---
const outDir = process.argv.includes('--output-dir')
  ? process.argv[process.argv.indexOf('--output-dir') + 1]
  : '_site/docs';

// --- Resolve paths ---
const projectRoot = path.resolve(__dirname, '..');
const docsDir = path.join(projectRoot, 'docs');
const readmePath = path.join(projectRoot, 'README.md');
const layoutPath = path.join(__dirname, 'layout.html');

// --- marked setup (GFM) ---
const { marked } = require('marked');
marked.setOptions({
  gfm: true,
  breaks: false,
  langPrefix: 'language-',
});

function escapeHtml(str) {
  return str.replace(/[&<>"']/g, c => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
  }[c]));
}

// --- Read layout template ---
let layoutTemplate = fs.readFileSync(layoutPath, 'utf8');

/**
 * Convert markdown links to appropriate HTML links.
 *
 * - docs/X.md or ./docs/X.md → /docs/X.html (same-site docs page)
 * - README.md → /docs/index.html (homepage)
 * - SPEC.md, CLAUDE.md, ../SPEC.md (root-level files) → GitHub source link
 * - Other .md references → GitHub source link
 */
function fixLinks(html) {
  return html.replace(/href="([^"]+)\.md(\#[^"]*)?"/g, (_, href, fragment) => {
    const frag = fragment || '';
    const normalized = href.replace(/^\.\//, '');

    // Root-level docs files referenced via ../
    const rootFiles = ['SPEC.md', 'CLAUDE.md', 'README.md'];
    for (const rf of rootFiles) {
      if (normalized === rf || normalized === '../' + rf) {
        if (rf === 'README.md') return `href="/docs/index.html${frag}"`;
        return `href="https://github.com/alexjwhite/cyoa-script/blob/main/${rf}${frag}" target="_blank"`;
      }
    }

    // docs/X.md or bare X.md (relative to docs dir) → /docs/X.html
    if (normalized.startsWith('docs/') || !normalized.includes('/')) {
      const basename = path.basename(normalized, '.md');
      return `href="/docs/${basename}.html${frag}"`;
    }

    // Any other .md with a path → GitHub source
    return `href="https://github.com/alexjwhite/cyoa-script/blob/main/${normalized}${frag}" target="_blank"`;
  });
}

/**
 * Wrap rendered markdown content inside the HTML layout template.
 */
function wrapInLayout(title, contentHtml) {
  return layoutTemplate
    .replace(/\{\{TITLE\}\}/g, title)
    .replace(/\{\{CONTENT\}\}/g, contentHtml);
}

/**
 * Process a single markdown file into an HTML page.
 */
function processFile(mdPath, htmlFilename, title) {
  const mdContent = fs.readFileSync(mdPath, 'utf8');

  // Determine the page title from the first H1 if not provided
  if (!title) {
    const match = mdContent.match(/^#\s+(.+)$/m);
    title = match ? match[1].trim() : htmlFilename.replace('.html', '');
  }

  const rendered = marked.parse(mdContent);
  const fixed = fixLinks(rendered);
  const page = wrapInLayout(title, fixed);

  const outputPath = path.join(outDir, htmlFilename);
  fs.writeFileSync(outputPath, page, 'utf8');
  console.log(`  ✓ ${htmlFilename} (${mdPath.replace(projectRoot + '/', '')})`);
}

// --- Main ---
console.log('Building CYOA documentation site...');

// Ensure output directory exists
fs.mkdirSync(outDir, { recursive: true });

// 1. Homepage: README.md → index.html
processFile(readmePath, 'index.html', 'CYOA Engine — Declarative Language for Choose-Your-Own-Adventure Games');

// 2. Documentation pages from docs/*.md
const docFiles = fs.readdirSync(docsDir)
  .filter(f => f.endsWith('.md'))
  .sort();

for (const filename of docFiles) {
  const mdPath = path.join(docsDir, filename);
  const htmlFilename = filename.replace('.md', '.html');
  processFile(mdPath, htmlFilename);
}

// 3. Copy favicon to output directory
const faviconSrc = path.join(__dirname, 'favicon.svg');
if (fs.existsSync(faviconSrc)) {
  fs.copyFileSync(faviconSrc, path.join(outDir, 'favicon.svg'));
}

console.log(`\nDone! Built ${docFiles.length + 1} pages → ${outDir}/`);
