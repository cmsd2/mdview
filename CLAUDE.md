# CLAUDE.md

Guidance for AI agents working in this repository.

## What this is

`mdview` is a **lightweight, read-only Markdown viewer** with live
file-watching. It is a **Tauri 2** desktop app: a **Rust** backend plus a small
**vanilla-JS** frontend rendered in the **OS-native WebView** (WebKit /
WebView2 / WebKitGTK) — there is **no bundled Chromium** and **no JS
framework/bundler**.

Design priorities, in order: cross-platform · few/light dependencies ·
good-looking rendering · low resource use. Weigh changes against these. The
full design lives in [`docs/`](./docs) — read it before non-trivial work.

**Non-goals:** editing, file management, plugins. Keep it a viewer.

## Layout

```
src-tauri/            Rust backend (the app crate; lib + thin main)
  src/lib.rs          CLI (clap), Tauri builder, commands, setup
  src/render.rs       comrak -> ammonia sanitize; welcome screen; title
  src/config.rs       per-OS theme config (directories + toml)
  src/watch.rs        debounced file watching (notify) -> "file-changed" event
  src/assets.rs       mdview:// custom protocol (sandboxed to the doc's dir)
  tauri.conf.json     window, CSP, bundle, withGlobalTauri
  capabilities/       Tauri permission grants
frontend/             static files served as frontendDist (no build step)
  index.html app.js style.css
  vendor/             KaTeX, highlight.js, github-markdown-css (generated)
scripts/              copy-vendor.mjs (vendoring), gen-icon.mjs (icon source)
docs/                 design docs (start here)
```

## Build / run / test

```sh
npm install        # installs Tauri CLI + runs copy-vendor (postinstall)
npm run vendor     # re-copy frontend/vendor from node_modules

# Quick run (no dev server needed — frontend is static):
cargo run --manifest-path src-tauri/Cargo.toml -- path/to/file.md
cargo run --manifest-path src-tauri/Cargo.toml --          # welcome screen

cargo test --manifest-path src-tauri/Cargo.toml            # backend tests
node --check frontend/app.js                               # JS sanity
npm run build      # release bundle (slow)
```

- **`tauri dev` arg gotcha:** to pass a file you need a **doubled** `--`
  (`npm run tauri dev -- -- file.md`) — one separator for npm→tauri, one for
  tauri→binary. For most testing, prefer `cargo run` above.
- `frontend/vendor/` and `src-tauri/target/` are gitignored. After a fresh
  clone you must `npm install` (or `npm run vendor`) before the frontend works.

## How it fits together (data flow)

1. `lib.rs` parses the CLI (`clap`), resolves the file, stores
   `AppState { doc_path, base_dir }` (both `Option` — `None` = welcome screen).
2. Frontend (`app.js`) calls `invoke("render")` → `render.rs` runs comrak
   (GFM + math, raw-HTML passthrough) then **ammonia** sanitization, returning
   `{ html, title }`.
3. `app.js` injects the HTML, then client-side: rewrites relative asset URLs to
   `mdview://`, renders math (KaTeX over `span[data-math-style]`), highlights
   code (highlight.js), intercepts external links, re-applies find highlights.
4. `watch.rs` emits `file-changed`; the frontend re-invokes `render` and swaps
   the DOM, preserving scroll position.
5. `assets.rs` serves `mdview://` requests from `base_dir` only.

## Invariants — do not break these

- **All rendered HTML is sanitized.** comrak runs with `render.unsafe_ = true`
  (raw HTML passes through) **only because** ammonia cleans it afterward
  (`render::sanitizer`). Never send comrak output to the WebView without that
  pass. If you enable a comrak feature whose markup ammonia strips (new tag or
  attribute), extend the allowlist in `render.rs` accordingly.
- **The `mdview://` handler must stay sandboxed.** `assets::resolve`
  canonicalizes and checks `starts_with(base_dir)`. Keep that guard.
- **Theme precedence:** stored override (`config.toml`) → OS hint
  (`window.theme()`) → **light** fallback. See `resolve_theme` in `lib.rs`.
- **No new heavy dependencies** without a strong reason — it's an explicit
  project priority. Prefer the existing crates/libs.
- **Frontend stays framework-free and bundler-free.** Use the Tauri global API
  (`window.__TAURI__.*`, enabled by `withGlobalTauri`). New vendored assets go
  through `scripts/copy-vendor.mjs`, not a CDN (CSP forbids remote scripts).

## Gotchas worth knowing

- **comrak math markup** is `<span data-math-style="inline|display">TeX</span>`;
  `app.js` and the ammonia allowlist both depend on that exact shape.
- **Asset URL prefix is platform-specific:** `asset_base` returns
  `mdview://localhost/` on macOS/Linux but `http://mdview.localhost/` on
  Windows. Build asset URLs via that command, never hardcode.
- **Tauri commands** live in `lib.rs` and are registered in two places:
  `tauri::generate_handler!` and (for permissions) the JS just invokes by name.
  Adding a command = add the `#[tauri::command]` fn + list it in the handler.
- **Icons** are generated from `app-icon.png` (itself produced by
  `scripts/gen-icon.mjs`) via `npx tauri icon`; the source PNG is gitignored.

## Conventions

- Match the surrounding style; keep comments at the existing density (modules
  have a `//!` header explaining their role).
- Update [`CHANGELOG.md`](./CHANGELOG.md) (Keep a Changelog format) under
  `[Unreleased]` for user-facing changes, and the relevant `docs/` file when
  behavior or architecture changes.
- License is **GPL-3.0-or-later** (declared in `Cargo.toml` and `package.json`).
```
