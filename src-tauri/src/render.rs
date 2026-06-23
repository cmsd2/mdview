//! Markdown -> sanitized HTML rendering.
//!
//! comrak produces GFM-complete HTML (with raw-HTML passthrough enabled), then
//! every byte passes through an ammonia whitelist before it can reach the
//! WebView. Math is left as `<span data-math-style>` markers for KaTeX to
//! render client-side; code fences keep their `language-*` class for hljs.

use percent_encoding::percent_decode_str;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

#[derive(Debug, Serialize)]
pub struct RenderedDoc {
    pub html: String,
    pub title: String,
}

/// Render a Markdown file to sanitized HTML. Returns a friendly error string
/// on failure so the frontend can show it inline rather than crashing.
pub fn render(path: &Path) -> Result<RenderedDoc, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Could not read {}: {e}", path.display()))?;

    let html = markdown_to_safe_html(&source);
    let title = extract_title(&source).unwrap_or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("mdview")
            .to_string()
    });

    Ok(RenderedDoc { html, title })
}

/// The welcome screen shown when mdview is launched without a file. Rendered
/// through the same Markdown pipeline as any document.
pub fn welcome() -> RenderedDoc {
    const WELCOME: &str = "\
# mdview

A lightweight, read-only Markdown viewer.

**Open a Markdown file to view it:**

```sh
mdview path/to/file.md
```

The file is watched and the view updates live as you edit it.
";
    RenderedDoc {
        html: markdown_to_safe_html(WELCOME),
        title: "mdview".to_string(),
    }
}

/// Parse Markdown to HTML (comrak) and sanitize it (ammonia).
pub fn markdown_to_safe_html(source: &str) -> String {
    let mut options = comrak::Options::default();
    let ext = &mut options.extension;
    ext.table = true;
    ext.strikethrough = true;
    ext.tasklist = true;
    ext.autolink = true;
    ext.footnotes = true;
    ext.tagfilter = true;
    ext.header_ids = Some(String::new()); // enable in-doc anchor links
    ext.math_dollars = true; // $inline$ and $$display$$
    ext.math_code = true; // ```math fenced blocks

    // Raw HTML passthrough — only safe because of the ammonia pass below.
    options.render.unsafe_ = true;

    let dirty = comrak::markdown_to_html(source, &options);
    sanitizer().clean(&dirty).to_string()
}

/// Absolute, sandboxed filesystem paths for the local assets referenced by the
/// rendered HTML, for the file-watcher to track. Each relative reference is
/// resolved lexically against `base_dir` (so a not-yet-created asset still
/// yields a path) and any that would escape `base_dir` are dropped.
pub fn local_asset_paths(base_dir: &Path, html: &str) -> Vec<PathBuf> {
    local_assets(html)
        .iter()
        .filter_map(|rel| sandboxed_join(base_dir, rel))
        .collect()
}

/// Join a relative asset path onto `base_dir` without requiring it to exist,
/// resolving `.`/`..` lexically and refusing anything that escapes `base_dir`
/// (returns `None`) — mirroring the `mdview://` protocol's traversal guard.
fn sandboxed_join(base_dir: &Path, rel: &str) -> Option<PathBuf> {
    use std::path::Component;
    let mut path = base_dir.to_path_buf();
    let mut depth = 0i32; // components pushed below `base_dir`
    for comp in Path::new(rel).components() {
        match comp {
            Component::Normal(c) => {
                path.push(c);
                depth += 1;
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return None; // would climb above base_dir
                }
                path.pop();
                depth -= 1;
            }
            // Absolute paths / drive prefixes aren't relative to base_dir.
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(path)
}

/// Local (relative) asset references (`src`/`poster`) in the rendered HTML,
/// decoded to filesystem-style paths. Absolute/scheme/anchor URLs are skipped —
/// they don't live in the document's directory. Mirrors the frontend's notion
/// of a "local" URL (see `isAbsolute` in `app.js`).
pub fn local_assets(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for attr in ["src=\"", "poster=\""] {
        let mut rest = html;
        while let Some(i) = rest.find(attr) {
            rest = &rest[i + attr.len()..];
            let Some(end) = rest.find('"') else { break };
            let raw = &rest[..end];
            rest = &rest[end + 1..];
            if is_local(raw) {
                out.push(percent_decode_str(raw).decode_utf8_lossy().into_owned());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// True for a relative URL that resolves to a file in the document's directory.
fn is_local(url: &str) -> bool {
    !(url.is_empty()
        || url.starts_with("//")
        || url.starts_with('#')
        || url.starts_with('/')
        || has_scheme(url))
}

/// Does `url` begin with an explicit scheme (`http:`, `data:`, `mdview:`, …)?
/// Matches the frontend regex `^[a-z][a-z0-9+.-]*:` (case-insensitive).
fn has_scheme(url: &str) -> bool {
    let bytes = url.as_bytes();
    if !bytes.first().is_some_and(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    for &b in &bytes[1..] {
        if b == b':' {
            return true;
        }
        if !(b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.') {
            return false;
        }
    }
    false
}

/// First-H1 (`# ...`) text, used for the window title.
fn extract_title(source: &str) -> Option<String> {
    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim().trim_end_matches('#').trim();
            if !title.is_empty() {
                return Some(title.to_string());
            }
        }
    }
    None
}

/// A reusable ammonia cleaner configured for comrak's GFM + math output.
fn sanitizer() -> &'static ammonia::Builder<'static> {
    static CLEANER: OnceLock<ammonia::Builder<'static>> = OnceLock::new();
    CLEANER.get_or_init(|| {
        let mut b = ammonia::Builder::default();

        // Tags ammonia doesn't allow by default but comrak/GFM/media need.
        b.add_tags([
            "video", "audio", "source", "input", "section", "details", "summary", "del", "ins",
            "kbd", "figure", "figcaption",
        ]);

        // `class` (language-*, hljs, footnotes) and `id` (header anchors,
        // footnote refs) are needed throughout; values can't execute.
        b.add_generic_attributes(["class", "id", "align"]);

        b.add_tag_attributes("img", ["width", "height", "loading"]);
        b.add_tag_attributes("span", ["data-math-style"]); // KaTeX markers
        b.add_tag_attributes("section", ["data-footnotes"]);
        b.add_tag_attributes("td", ["style"]); // comrak table cell alignment
        b.add_tag_attributes("th", ["style"]);
        b.add_tag_attributes("input", ["type", "checked", "disabled"]); // task lists
        b.add_tag_attributes(
            "video",
            ["src", "controls", "width", "height", "poster", "loop", "muted", "playsinline"],
        );
        b.add_tag_attributes("audio", ["src", "controls", "loop", "muted"]);
        b.add_tag_attributes("source", ["src", "type"]);

        // Keep relative URLs as-is; the frontend rewrites them to mdview://.
        b.url_relative(ammonia::UrlRelative::PassThrough);

        // Permit our asset scheme and data: URIs in addition to the defaults.
        b.add_url_schemes(["mdview", "data", "tel"]);

        b
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_scripts_but_keeps_structure() {
        let html = markdown_to_safe_html("# Hi\n\n<script>alert(1)</script>\n\nText");
        assert!(!html.contains("<script"));
        assert!(html.contains("Text"));
    }

    #[test]
    fn keeps_code_language_class() {
        let html = markdown_to_safe_html("```rust\nfn main() {}\n```\n");
        assert!(html.contains("language-rust"));
    }

    #[test]
    fn local_assets_finds_relative_includes_only() {
        let html = markdown_to_safe_html(
            "![a](images/a.png)\n\n![b](http://x/y.png)\n\n\
             <video poster=\"thumb.jpg\" src=\"clip.mp4\"></video>\n\n![c](sub/c%20d.png)",
        );
        let assets = local_assets(&html);
        assert!(assets.contains(&"images/a.png".to_string()));
        assert!(assets.contains(&"thumb.jpg".to_string()));
        assert!(assets.contains(&"clip.mp4".to_string()));
        assert!(assets.contains(&"sub/c d.png".to_string())); // percent-decoded
        assert!(!assets.iter().any(|a| a.contains("http"))); // absolute skipped
    }

    #[test]
    fn asset_paths_resolve_under_base_and_reject_escapes() {
        let base = Path::new("/docs");
        let html = markdown_to_safe_html(
            "![a](img/a.png)\n\n![b](../secret.png)\n\n![c](./b.png)\n\n![d](http://x/y.png)",
        );
        let paths = local_asset_paths(base, &html);
        assert!(paths.contains(&PathBuf::from("/docs/img/a.png")));
        assert!(paths.contains(&PathBuf::from("/docs/b.png"))); // "./" normalized
        assert!(!paths.iter().any(|p| p.ends_with("secret.png"))); // escape dropped
        assert!(!paths.iter().any(|p| p.to_string_lossy().contains("http"))); // absolute skipped
    }

    #[test]
    fn title_from_first_h1() {
        assert_eq!(extract_title("intro\n# Real Title\n").as_deref(), Some("Real Title"));
        assert_eq!(extract_title("no heading here"), None);
    }
}
