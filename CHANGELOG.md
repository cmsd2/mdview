# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Live reload now also watches locally included assets: when the document
  references images or media (e.g. `![](images/diagram.png)`), edits to those
  files re-render the view too. A referenced file (or whole folder) that
  doesn't exist yet is also tracked, so creating it later triggers a reload.
  Files the document no longer references are dropped from the watcher.
- File watching is tuned to stay cheap for a document inside a large project
  (e.g. a README at a monorepo root): individual files are watched via OS
  events, while only the relevant directories are polled (a bounded, one-level
  scan), avoiding whole-subtree event streams.
- **View > Reload** menu item (`Ctrl`/`Cmd`-`R`) to re-render the current
  document on demand.
- macOS: **mdview > Install 'mdview' Shell Command** symlinks the app into
  `~/.local/bin` so it can be run from the terminal after being installed to
  `/Applications` (no `sudo`, no `/usr/local/bin`).

### Fixed

- Reload now reflects images that were edited, moved, or deleted. Asset
  responses are served with `Cache-Control: no-store` and asset URLs are
  cache-busted per render, so the WebView no longer shows a stale cached copy.

## [0.1.0] - 2026-06-16

Initial release: a lightweight, read-only Markdown viewer with live
file-watching.

### Added

- Native desktop app built with Tauri 2, using the OS-native WebView (no
  bundled Chromium) for a small binary and low resource use.
- GitHub-Flavoured Markdown rendering via `comrak` (tables, task lists,
  strikethrough, autolinks, footnotes, header anchors), sanitized with
  `ammonia` before reaching the WebView.
- Math rendering with KaTeX (`$inline$` and `$$display$$`).
- Syntax highlighting with highlight.js, themed to match light/dark.
- Embedded images and common media (video, audio, SVG, PDF) served from the
  document's directory through a sandboxed `mdview://` asset protocol with a
  path-traversal guard.
- Live reload: the open file is watched (debounced) and the view re-renders on
  save, preserving scroll position.
- In-page find (`Ctrl`/`Cmd`-`F`): highlights all matches with next/previous
  navigation and a match count; re-runs after live reload. Uses the CSS Custom
  Highlight API.
- Light / dark / system theme: defaults to the OS hint, falls back to light
  when none is detected. A non-system choice is persisted to a config file in
  the per-OS location (XDG on Linux, `Application Support` on macOS, `%APPDATA%`
  on Windows).
- Command-line interface via `clap` with an optional positional file path; a
  welcome screen is shown when launched without a file.
- GPL-3.0-or-later license.

[Unreleased]: https://github.com/cmsd2/mdview/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/cmsd2/mdview/releases/tag/v0.1.0
