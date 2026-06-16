# Rendering details

How each content type asked for is handled.

## GitHub-Flavoured Markdown

comrak is configured CommonMark + GFM. Extensions enabled:

| Feature | comrak option |
|---------|---------------|
| Tables | `extension.table` |
| Task list items `- [ ]` | `extension.tasklist` |
| Strikethrough `~~x~~` | `extension.strikethrough` |
| Autolinks (bare URLs) | `extension.autolink` |
| Footnotes | `extension.footnote` |
| Header anchors | `extension.header_ids` (for in-doc `#` links) |
| Raw HTML passthrough | `render.unsafe = true` **+ ammonia sanitization** |

Visual parity with GitHub comes from **github-markdown-css** applied to the
`.markdown-body` container. The light/dark variant is chosen by the resolved
theme (see [design.md](./design.md#5-config--theme-module-srcconfigrs)) and
applied via a `data-theme` attribute on `<html>`, so a stored override wins
over the OS hint rather than `prefers-color-scheme` deciding unconditionally.

## Math

- comrak's math extension (`extension.math_dollars` for `$…$` / `$$…$$`, and
  `extension.math_code` for ```` ```math ```` fenced blocks) emits semantic
  markers rather than rendered output.
- **KaTeX** runs client-side over those markers (inline + display modes).
  Chosen over MathJax for speed, smaller payload, and no network dependency
  (KaTeX fonts are vendored locally).

## Code & syntax highlighting

- comrak emits fenced blocks as `<pre><code class="language-xxx">`.
- **highlight.js** colours them on the client after each render. The hljs
  theme is paired to light/dark so it matches the document background.
- Kept client-side deliberately: bundling backend highlighting (syntect) would
  inflate the binary with a large syntax/theme corpus for little gain.

## Images & embedded media

- **Relative paths** (`![](diagram.png)`, `<img src="pics/a.jpg">`) are
  rewritten to the `mdview://` scheme and served from the document's directory
  by the asset handler (with a path-traversal guard).
- **Absolute/remote** `http(s)` images load directly through the WebView
  (subject to CSP).
- **Data URIs** (`data:image/png;base64,…`) render as-is.
- SVG renders natively (sanitized — no embedded scripts).

## Other common MIME types

The asset handler attaches a guessed MIME type, so the WebView can render any
type it natively supports when referenced via standard HTML/Markdown:

| Type | How it shows |
|------|--------------|
| PNG/JPEG/GIF/WebP/SVG | `<img>` inline |
| MP4 / WebM video, MP3 / WAV audio | `<video>` / `<audio>` (raw-HTML tags, sanitized) |
| PDF | inline via `<embed>`/`<iframe>` where the WebView supports it, else a click-to-open link |
| Anything else | rendered as a download/open-externally link |

## Reload behaviour

On every render swap, `app.js`:
1. records `scrollTop / scrollHeight` before the swap,
2. replaces `innerHTML`,
3. re-runs KaTeX then highlight.js over the new subtree,
4. restores the scroll ratio.

This keeps the reading position stable across live updates instead of jumping
to the top on each save.
