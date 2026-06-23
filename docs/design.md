# Architecture & design

## Overview

```
                          mdview process (Tauri)
  ┌──────────────────────────────────────────────────────────────┐
  │  Rust core                                                     │
  │  ┌──────────┐   ┌──────────┐   ┌───────────┐   ┌───────────┐  │
  │  │   CLI    │   │  Render  │   │  Watcher  │   │   Asset   │  │
  │  │  parse   │──▶│ comrak + │   │  notify + │   │  protocol │  │
  │  │  path    │   │ ammonia  │   │ debounce  │   │  handler  │  │
  │  └──────────┘   └────┬─────┘   └─────┬─────┘   └─────┬─────┘  │
  │                      │ html          │ event         │ bytes  │
  └──────────────────────┼───────────────┼───────────────┼────────┘
        invoke("render") │   emit         │  mdview://    │
                         ▼   "file-changed"▼               ▼
  ┌──────────────────────────────────────────────────────────────┐
  │  OS WebView (frontend: index.html + app.js + vendored assets) │
  │   • inject sanitized HTML                                      │
  │   • KaTeX renders math markers                                 │
  │   • highlight.js colours code blocks                           │
  │   • preserve scroll position across reloads                    │
  └──────────────────────────────────────────────────────────────┘
```

## Components

### 1. CLI / entry (`src/main.rs`)
- Parses one positional arg: the Markdown file path (resolved to absolute).
- Validates the file exists and is readable; errors go to stderr with a non-zero exit.
- Stores `document_path` and its parent `base_dir` in Tauri-managed state.
- Launches the window.

### 2. Render module (`src/render.rs`)
- Single function: `render(path) -> Result<RenderedDoc>` where
  `RenderedDoc { html: String, title: String }`.
- Reads the file, runs comrak with GFM + math extensions enabled, then
  passes output through ammonia with a whitelist that keeps math/code markup
  and standard media tags but strips `<script>`, event handlers, and
  `javascript:` URLs.
- `title` is the first H1 (fallback: filename) — used for the window title.

### 3. Watcher module (`src/watch.rs`)
A path-watching abstraction: `FileWatcher::watch(&[PathBuf])` takes a flat list
of absolute file paths and emits a `file-changed` event whenever any is
created, modified, or removed — **without the caller caring whether the files
or their parent directories exist yet**. Internally:
- An **OS event watcher** is registered on the individual files that *exist*.
  Watching a file (not its directory) keeps macOS FSEvents from streaming the
  surrounding subtree to us, so cost stays low even for a README at a large
  project root, and in-place edits are instant.
- A **poll watcher** (debounced, ~1 s) is registered on directories — the
  nearest existing ancestor of each target. A non-recursive poll is a one-level
  scan, bounded by a directory's direct children regardless of project size. It
  catches what file-watches can't: not-yet-existing targets/folders appearing,
  and replace-on-save on Linux (where an inode-based file watch goes stale).
- A background **owner thread** owns both watchers and re-reconciles whenever
  its own events reveal a target or intermediate folder has appeared, so
  descent into folders that didn't exist yet needs no help from the caller.
- Both watchers are debounced (~150 ms) so a burst of writes yields one update.

The caller side: after each render, `render::local_asset_paths` resolves the
relative `src`/`poster` URLs against `base_dir` (lexically, so missing files
still yield a path; sandboxed like the `mdview://` handler), and the `render`
command calls `watch` with the document plus those asset paths. The **View >
Reload** menu item emits `file-changed` directly to force a re-render.

### 4. Asset protocol handler (`src/assets.rs`)
- Registers a custom URI scheme `mdview://` whose handler resolves a request
  path **relative to `base_dir`**, canonicalizes it, and **refuses anything
  that escapes `base_dir`** (path-traversal guard).
- Returns the file bytes with a guessed MIME type (`mime_guess`) so the
  WebView renders images/video/audio/PDF/etc. natively.

### 5. Config & theme module (`src/config.rs`)
- Resolves the config file path via the `directories` crate
  (`ProjectDirs::from("", "", "mdview")`):
  - Linux: `${XDG_CONFIG_HOME:-~/.config}/mdview/config.toml`
  - macOS: `~/Library/Application Support/mdview/config.toml`
  - Windows: `%APPDATA%\mdview\config.toml`
- Loads on startup; **a missing/invalid file is not an error** — it just means
  "no stored preference." The file is created lazily, only when the user first
  overrides the theme.
- Schema (TOML):
  ```toml
  # "light" | "dark" | omitted/"system" = follow the OS hint
  theme = "dark"
  ```
- **Theme resolution precedence** (first match wins):
  1. Stored override in `config.toml` (`light` or `dark`).
  2. OS hint — Tauri reports the window's `theme()` (Dark/Light) from the
     platform; the WebView's `prefers-color-scheme` is the same signal.
  3. **Fallback: light** (when no override and no detectable hint).
- Writing is atomic (write temp file + rename) and creates the parent dir.

### 6. Frontend (`frontend/`)
- `index.html` — shell with a `<article class="markdown-body">` mount point and
  vendored `katex`, `highlight.js`, and `github-markdown-css`.
- `app.js`:
  - On load and on every `file-changed` event → `invoke("render")`.
  - Capture current scroll ratio, swap `innerHTML`, run KaTeX + hljs over the
    new DOM, then restore scroll ratio.
  - Rewrite relative `src`/`href` of local resources to the `mdview://` scheme;
    external `http(s)` links open in the user's default browser (Tauri shell
    open), not inside the WebView.
  - On startup, apply the resolved theme by setting a `data-theme` attribute on
    `<html>`; the light/dark `github-markdown-css` variant and the matching
    hljs/KaTeX theme key off that attribute.
  - A theme toggle in the UI calls `invoke("set_theme", { theme })`, which
    persists the choice to `config.toml` and flips `data-theme` immediately.
    Selecting "system" clears the override and reverts to the OS hint.

## IPC contract

| Direction | Name | Payload | Notes |
|-----------|------|---------|-------|
| JS → Rust | `invoke("render")` | none (path is in managed state) | Returns `{ html, title }`. |
| JS → Rust | `invoke("get_theme")` | none | Returns the resolved theme `"light" \| "dark"` (override → OS hint → light). |
| JS → Rust | `invoke("set_theme")` | `{ theme: "light" \| "dark" \| "system" }` | Persists/clears the override in `config.toml`; returns the new resolved theme. |
| Rust → JS | `emit("file-changed")` | `{ path }` | Frontend re-invokes `render`. |
| JS → Rust | custom scheme `mdview://<relpath>` | — | Returns asset bytes + MIME. |

Keeping the path in Rust state (rather than passing it from JS) means the
frontend can never request an arbitrary file — it can only re-render *the*
document.

## Live-update flow

1. User saves the `.md` file in their editor.
2. `notify` fires; debouncer coalesces; watcher emits `file-changed`.
3. Frontend records scroll ratio → `invoke("render")` → receives fresh HTML.
4. DOM swapped; KaTeX + hljs re-run; scroll restored. No flicker, no reload.

## Security model

Markdown is treated as **untrusted**:
- comrak raw-HTML passthrough is **only** enabled together with ammonia
  sanitization downstream (never raw HTML straight into the WebView).
- Tauri CSP forbids inline/remote script execution beyond the vendored bundle.
- The `mdview://` handler is sandboxed to `base_dir` (no `../` escape, no
  absolute-path reads outside the tree).
- External links open out-of-process in the system browser.

## Error handling

- File missing/unreadable at startup → stderr message + non-zero exit.
- File deleted while open → render shows a non-fatal "file unavailable" banner;
  the watcher keeps waiting for it to reappear.
- Render/parse errors → shown inline in the view rather than crashing.
