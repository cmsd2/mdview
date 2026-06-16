# Roadmap

## v1 — the brief

- [ ] `mdview <file.md>` opens a native Tauri window.
- [ ] GFM rendering via comrak (tables, task lists, strikethrough, autolinks,
      footnotes, anchors).
- [ ] Math via KaTeX; syntax highlighting via highlight.js.
- [ ] Relative images + common MIME types via the scoped `mdview://` handler.
- [ ] File-watching with debounce → live update, scroll position preserved.
- [ ] HTML sanitization (ammonia) + path-traversal-safe asset serving.
- [ ] Light/dark theme: default to OS hint, fall back to **light** if none.
- [ ] Manual theme toggle persisted to a config file in the per-OS config dir
      (XDG / `Application Support` / `%APPDATA%`); "system" clears the override.
- [ ] Clean startup/error handling for missing/deleted files (and missing/
      invalid config treated as "no preference," not an error).

## v1.1 — quality of life

- [ ] In-page find (Ctrl/Cmd-F) and a generated table-of-contents sidebar.
- [ ] Remember window size/position per document.
- [ ] "Open another file…" without relaunching.

## Later — optional, only if wanted

- [ ] **Headless mode**: `mdview --serve` runs just the renderer + watcher as a
      local server and opens your browser instead of a window (the lightest of
      all footprints; reuses the same render path).
- [ ] Mermaid / diagram blocks.
- [ ] Print / export-to-HTML.
- [ ] Multi-tab or directory-browsing mode.

## Explicit non-goals

- Editing, autosave, or any write-back to the file.
- Cloud sync / document management.
- A plugin system.
