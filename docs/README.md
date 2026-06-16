# mdview — design docs

A lightweight, read-only Markdown viewer with live file-watching.

**Goals (in priority order):**

1. **Cross-platform** — macOS, Windows, Linux from one codebase.
2. **Few/light dependencies** — no bundled browser engine, small binary, fast cold start.
3. **Good-looking rendering** — GitHub-flavoured Markdown, math, images, and common MIME types render cleanly.
4. **Low resource usage** — small RSS at idle, cheap re-render on file change.

**Non-goals:** editing, document management, syncing, plugins/extensions (v1).

## Documents

| File | What it covers |
|------|----------------|
| [tech-stack.md](./tech-stack.md) | Chosen stack, alternatives weighed, dependency list & footprint |
| [design.md](./design.md) | Architecture, components, data flow, IPC contract, security |
| [rendering.md](./rendering.md) | How GFM / math / images / other MIME types are handled |
| [roadmap.md](./roadmap.md) | Milestones and what ships in v1 vs later |

## One-paragraph summary

`mdview <file.md>` opens a native window (Tauri, using the OS's built-in
WebView — WebKit on macOS, WebView2 on Windows, WebKitGTK on Linux; **no
bundled Chromium**). A Rust core parses Markdown to HTML with
[`comrak`](https://github.com/kivikakk/comrak) (GFM-complete), sanitizes it,
and hands it to a minimal vanilla-JS frontend that does math (KaTeX) and
syntax highlighting (highlight.js) client-side. A `notify`-based file watcher
re-renders on save and live-updates the view while preserving scroll position.
Relative images and other assets are served through a scoped custom URI
protocol rooted at the document's directory.
