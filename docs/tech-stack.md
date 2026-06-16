# Tech stack

## Decision

| Layer | Choice | Why |
|-------|--------|-----|
| App shell | **Tauri 2.x** | Uses the OS-native WebView, so no Chromium ships in the binary. ~5–10 MB app, low idle RAM, real native window. |
| Backend language | **Rust** | Tauri's host language; gives us a fast single static binary and access to the crates below. |
| Markdown parsing | **comrak** | GFM-complete (tables, task lists, strikethrough, autolinks, footnotes), CommonMark-compliant, pure Rust, actively maintained. Has a built-in math extension. |
| HTML sanitization | **ammonia** | Whitelist-based cleaner. Markdown is treated as untrusted input; we strip scripts/handlers before it reaches the WebView. |
| File watching | **notify** | Cross-platform FS events (FSEvents / ReadDirectoryChangesW / inotify) with a debounced wrapper. |
| Math | **KaTeX** (client) | Fast, font-based, no network, far lighter than MathJax. comrak emits math markers; KaTeX renders them. |
| Syntax highlighting | **highlight.js** (client) | Kept on the client to keep the Rust binary lean (vs. bundling syntect's syntax/theme set). |
| Base styling | **github-markdown-css** | Battle-tested GitHub look; ships a light and a dark variant. |
| Config location | **directories** crate | Resolves the correct per-OS config dir (XDG on Linux, `Application Support` on macOS, `%APPDATA%` on Windows) without hand-rolling paths. |
| Config format | **toml** + **serde** | Human-readable, comment-friendly config file; serde for (de)serialization. |
| Frontend framework | **none** (vanilla JS) | The view is "swap one HTML blob, re-run KaTeX/hljs." A framework would be pure weight. |

## Footprint expectation

- Binary: **~5–10 MB** (vs. ~150 MB+ for an Electron equivalent).
- Idle RAM: dominated by the OS WebView, which is shared OS infrastructure rather than a private Chromium copy.
- Cold start: sub-second; the WebView is already present on the OS.

## Alternatives considered

| Option | Verdict |
|--------|---------|
| **Electron** | Rejected — bundles Chromium (~150 MB, high RAM). Violates "few/light deps + low resource use." |
| **Local HTTP server + your browser** | Genuinely the lightest (no GUI framework at all), but you wanted a dedicated app window. Kept as a possible future "headless" mode (see roadmap). |
| **Go + Wails** | Viable twin of this design (goldmark instead of comrak). Slightly larger binaries; Rust chosen for comrak's math extension and tighter binary size. |
| **Qt / QWebEngine** | QWebEngine *is* Chromium — same weight problem as Electron, plus heavier toolchain. |
| **Terminal viewer (e.g. glow-style)** | Can't cleanly do embedded images + real math. Fails requirement 3. |
| **Backend syntax highlighting (syntect)** | Possible, but bundles a large syntax/theme corpus into the binary. Deferred to client highlight.js. |

## Toolchain prerequisites

- **Rust** (stable) + Tauri CLI.
- Node only for the small frontend bundle step (or skip bundling entirely and ship static assets — KaTeX/hljs/CSS are vendored as static files).
- Per-OS WebView: WebView2 runtime on Windows (auto-installable), WebKitGTK on Linux, system WebKit on macOS.
