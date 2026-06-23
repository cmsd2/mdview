# mdview

A lightweight, read-only **Markdown viewer** with live file-watching.

- **Cross-platform** — macOS, Windows, Linux (Tauri + the OS-native WebView, **no bundled Chromium**).
- **Lightweight** — small binary, low memory, sub-second start.
- **GitHub-flavoured Markdown** — tables, task lists, footnotes, autolinks, and anchors via [comrak](https://github.com/kivikakk/comrak).
- **Math** ([KaTeX](https://katex.org/)), **syntax highlighting** ([highlight.js](https://highlightjs.org/)), **embedded images** and common media (video/audio/SVG/PDF).
- **Live reload** — edits to the file update the view instantly, preserving scroll position.
- **Find** — `Ctrl`/`Cmd`-`F` highlights all matches with next/prev navigation and a match count.
- **Light / dark / system** theme, with the choice remembered in a config file.

> No editing. This is a viewer.

## Usage

```sh
mdview path/to/file.md   # open a file (watched; re-renders on save)
mdview                   # no file -> welcome screen
```

### Running from the terminal on macOS

When you drag `mdview.app` into `/Applications`, its binary lives at
`/Applications/mdview.app/Contents/MacOS/mdview`, which isn't on your `$PATH`.
To get a `mdview` command (no `sudo`, no `/usr/local/bin`):

- **From the app:** **mdview ▸ Install 'mdview' Shell Command**. This symlinks
  the app into `~/.local/bin`. If that directory isn't on your `PATH` yet, add
  this to your shell profile (e.g. `~/.zshrc`) and open a new terminal:

  ```sh
  export PATH="$HOME/.local/bin:$PATH"
  ```

- **By hand**, equivalently:

  ```sh
  mkdir -p ~/.local/bin
  ln -sf /Applications/mdview.app/Contents/MacOS/mdview ~/.local/bin/mdview
  ```

Run the binary directly (as above) rather than `open -a mdview` — `open`
detaches the process and discards the working directory, so relative paths like
`mdview ./notes.md` wouldn't resolve. One consequence of running it directly:
the app shares the terminal session's lifetime (close the terminal and the
window closes too) unless you background it.

During development you can run it without building a bundle (the frontend is
static, so no dev server is needed):

```sh
cargo run --manifest-path src-tauri/Cargo.toml -- path/to/file.md
```

If you prefer `tauri dev` (Rust hot-reload), note that arguments to the app
need a **doubled** `--` (one for npm→tauri, one for tauri→binary):

```sh
npm run tauri dev -- -- path/to/file.md
```

## Theme

A toolbar toggle switches between **light**, **dark**, and **system** (follow
the OS hint; falls back to light if none is detected). A non-system choice is
persisted to:

| OS | Path |
|----|------|
| Linux | `${XDG_CONFIG_HOME:-~/.config}/mdview/config.toml` |
| macOS | `~/Library/Application Support/mdview/config.toml` |
| Windows | `%APPDATA%\mdview\config.toml` |

## Development

```sh
npm install        # installs the Tauri CLI + vendors frontend assets
npm run dev        # run the app (cargo + tauri dev)
npm run build      # produce a release bundle
cargo test --manifest-path src-tauri/Cargo.toml   # backend tests
```

See [`docs/`](./docs) for the full design: [tech stack](./docs/tech-stack.md),
[architecture](./docs/design.md), [rendering](./docs/rendering.md), and the
[roadmap](./docs/roadmap.md).

## Tech stack

Tauri 2 (Rust) · comrak + ammonia · notify · KaTeX · highlight.js ·
github-markdown-css. Rationale and alternatives in
[docs/tech-stack.md](./docs/tech-stack.md).

## License

[GPL-3.0-or-later](./LICENSE).
