// Copies the vendored frontend assets out of node_modules into
// frontend/vendor/ so the app ships them locally (no CDN, no network).
// Run automatically on `npm install` (postinstall) or via `npm run vendor`.

import { existsSync, mkdirSync, copyFileSync, cpSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url)) + "/..";
const nm = join(root, "node_modules");
const out = join(root, "frontend", "vendor");

function ensureDir(p) {
  mkdirSync(p, { recursive: true });
}

function file(from, to) {
  const src = join(nm, from);
  const dst = join(out, to);
  if (!existsSync(src)) {
    console.error(`[vendor] missing: ${from} — did you run npm install?`);
    process.exitCode = 1;
    return;
  }
  ensureDir(dirname(dst));
  copyFileSync(src, dst);
  console.log(`[vendor] ${to}`);
}

function tree(from, to) {
  const src = join(nm, from);
  const dst = join(out, to);
  if (!existsSync(src)) {
    console.error(`[vendor] missing dir: ${from}`);
    process.exitCode = 1;
    return;
  }
  ensureDir(dirname(dst));
  cpSync(src, dst, { recursive: true });
  console.log(`[vendor] ${to}/`);
}

// Start clean so removed upstream files don't linger.
if (existsSync(out)) rmSync(out, { recursive: true, force: true });
ensureDir(out);

// GitHub Markdown CSS (light + dark variants).
file("github-markdown-css/github-markdown-light.css", "github-markdown-light.css");
file("github-markdown-css/github-markdown-dark.css", "github-markdown-dark.css");

// KaTeX: js, css, and the fonts the css references via url(fonts/...).
file("katex/dist/katex.min.js", "katex/katex.min.js");
file("katex/dist/katex.min.css", "katex/katex.min.css");
tree("katex/dist/fonts", "katex/fonts");

// highlight.js browser bundle + light/dark themes.
file("@highlightjs/cdn-assets/highlight.min.js", "highlight/highlight.min.js");
file("@highlightjs/cdn-assets/styles/github.min.css", "highlight/github.min.css");
file("@highlightjs/cdn-assets/styles/github-dark.min.css", "highlight/github-dark.min.css");

console.log("[vendor] done");
