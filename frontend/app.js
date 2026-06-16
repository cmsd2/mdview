// mdview frontend. Pure vanilla JS using Tauri's global API (withGlobalTauri).
// Responsibilities: fetch rendered HTML, render math + code, rewrite local
// asset URLs to the mdview:// scheme, intercept external links, manage theme.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

let assetBase = "mdview://localhost/";
let activeChoice = "system";

const content = document.getElementById("content");
const docTitle = document.getElementById("doc-title");

// ---- Theme -----------------------------------------------------------------

const MD_LIGHT = "vendor/github-markdown-light.css";
const MD_DARK = "vendor/github-markdown-dark.css";
const HLJS_LIGHT = "vendor/highlight/github.min.css";
const HLJS_DARK = "vendor/highlight/github-dark.min.css";

function applyTheme(resolved) {
  document.documentElement.dataset.theme = resolved;
  document.getElementById("md-theme").href = resolved === "dark" ? MD_DARK : MD_LIGHT;
  document.getElementById("hljs-theme").href =
    resolved === "dark" ? HLJS_DARK : HLJS_LIGHT;
}

function markActiveChoice(choice) {
  activeChoice = choice;
  for (const btn of document.querySelectorAll("[data-theme-choice]")) {
    btn.classList.toggle("active", btn.dataset.themeChoice === choice);
  }
}

async function chooseTheme(choice) {
  const resolved = await invoke("set_theme", { theme: choice });
  applyTheme(resolved);
  markActiveChoice(choice);
}

// ---- Asset URL rewriting ---------------------------------------------------

function isAbsolute(url) {
  return (
    /^[a-z][a-z0-9+.-]*:/i.test(url) || // has a scheme (http:, data:, mdview:, ...)
    url.startsWith("//") ||
    url.startsWith("#")
  );
}

function toAssetUrl(rel) {
  // Drop a leading "./" and percent-encode, preserving path separators.
  const cleaned = rel.replace(/^\.\//, "");
  return assetBase + cleaned.split("/").map(encodeURIComponent).join("/");
}

function rewriteAssets(root) {
  for (const el of root.querySelectorAll("[src]")) {
    const src = el.getAttribute("src");
    if (src && !isAbsolute(src)) el.setAttribute("src", toAssetUrl(src));
  }
}

// ---- External links --------------------------------------------------------

function interceptLinks(root) {
  for (const a of root.querySelectorAll("a[href]")) {
    const href = a.getAttribute("href");
    if (/^https?:\/\//i.test(href)) {
      a.addEventListener("click", (e) => {
        e.preventDefault();
        invoke("open_external", { url: href }).catch((err) =>
          console.error("open_external failed", err)
        );
      });
    }
  }
}

// ---- Math + code -----------------------------------------------------------

function renderMath(root) {
  if (!window.katex) return;
  for (const el of root.querySelectorAll("span[data-math-style]")) {
    const displayMode = el.getAttribute("data-math-style") === "display";
    try {
      window.katex.render(el.textContent, el, { displayMode, throwOnError: false });
    } catch (err) {
      console.warn("katex error", err);
    }
  }
}

function highlightCode(root) {
  if (!window.hljs) return;
  for (const block of root.querySelectorAll("pre code")) {
    window.hljs.highlightElement(block);
  }
}

// ---- Render cycle ----------------------------------------------------------

async function load() {
  const scroller = document.scrollingElement || document.documentElement;
  const ratio =
    scroller.scrollHeight > scroller.clientHeight
      ? scroller.scrollTop / (scroller.scrollHeight - scroller.clientHeight)
      : 0;

  try {
    const { html, title } = await invoke("render");
    content.innerHTML = html;
    rewriteAssets(content);
    renderMath(content);
    highlightCode(content);
    interceptLinks(content);

    docTitle.textContent = title;
    document.title = title;
    getCurrentWindow()
      .setTitle(`${title} — mdview`)
      .catch(() => {});

    // Restore reading position after layout settles.
    requestAnimationFrame(() => {
      scroller.scrollTop = ratio * (scroller.scrollHeight - scroller.clientHeight);
    });
  } catch (err) {
    content.innerHTML = `<div class="error"><strong>Could not render:</strong><br>${String(
      err
    )}</div>`;
  }
}

// ---- Bootstrap -------------------------------------------------------------

async function init() {
  try {
    assetBase = await invoke("asset_base");
  } catch (_) {
    /* keep default */
  }

  const resolved = await invoke("get_theme");
  applyTheme(resolved);
  markActiveChoice(resolved); // best-effort until the user picks explicitly

  for (const btn of document.querySelectorAll("[data-theme-choice]")) {
    btn.addEventListener("click", () => chooseTheme(btn.dataset.themeChoice));
  }

  await load();
  await listen("file-changed", load);
  await listen("watch-error", (e) =>
    console.warn("file-watching disabled:", e.payload)
  );
}

init();
