//! Markdown -> sanitized HTML rendering.
//!
//! comrak produces GFM-complete HTML (with raw-HTML passthrough enabled), then
//! every byte passes through an ammonia whitelist before it can reach the
//! WebView. Math is left as `<span data-math-style>` markers for KaTeX to
//! render client-side; code fences keep their `language-*` class for hljs.

use serde::Serialize;
use std::path::Path;
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
    fn title_from_first_h1() {
        assert_eq!(extract_title("intro\n# Real Title\n").as_deref(), Some("Real Title"));
        assert_eq!(extract_title("no heading here"), None);
    }
}
