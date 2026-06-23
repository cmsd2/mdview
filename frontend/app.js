// mdview frontend. Pure vanilla JS using Tauri's global API (withGlobalTauri).
// Responsibilities: fetch rendered HTML, render math + code, rewrite local
// asset URLs to the mdview:// scheme, intercept external links, manage theme.

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

let assetBase = "mdview://localhost/";
let activeChoice = "system";
// Bumped each render so asset URLs change, forcing the WebView to re-fetch
// (and thus reflect images that were edited, moved, or deleted) instead of
// serving stale bytes from its cache.
let assetVersion = 0;

const content = document.getElementById("content");
const docTitle = document.getElementById("doc-title");
const findbar = document.getElementById("findbar");
const findInput = document.getElementById("find-input");
const findCount = document.getElementById("find-count");

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
  const url = assetBase + cleaned.split("/").map(encodeURIComponent).join("/");
  // Cache-bust per render (see assetVersion) so reload reflects the file on disk.
  return url + (url.includes("?") ? "&" : "?") + "v=" + assetVersion;
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

// ---- Find (Ctrl/Cmd-F) -----------------------------------------------------
// Highlights matches with the CSS Custom Highlight API (no DOM mutation, so it
// composes cleanly with live re-renders). Degrades to count + scroll if the
// API is unavailable.

const supportsHighlight = typeof CSS !== "undefined" && !!CSS.highlights;
let findOpen = false;
let matches = [];
let current = -1;

function collectMatches(query) {
  matches = [];
  if (!query) return;
  const needle = query.toLowerCase();
  const walker = document.createTreeWalker(content, NodeFilter.SHOW_TEXT);
  let node;
  while ((node = walker.nextNode())) {
    const hay = node.nodeValue.toLowerCase();
    let from = 0;
    let idx = hay.indexOf(needle, from);
    while (idx !== -1) {
      const range = document.createRange();
      range.setStart(node, idx);
      range.setEnd(node, idx + needle.length);
      matches.push(range);
      from = idx + needle.length;
      idx = hay.indexOf(needle, from);
    }
  }
}

function paintHighlights() {
  if (!supportsHighlight) return;
  const others = matches.filter((_, i) => i !== current);
  CSS.highlights.set("find-match", new Highlight(...others));
  const cur = current >= 0 && matches[current] ? [matches[current]] : [];
  CSS.highlights.set("find-current", new Highlight(...cur));
}

function clearHighlights() {
  if (supportsHighlight) {
    CSS.highlights.delete("find-match");
    CSS.highlights.delete("find-current");
  }
  matches = [];
  current = -1;
}

function updateCount() {
  if (!findInput.value) {
    findCount.textContent = "0/0";
    findCount.classList.remove("empty");
  } else if (matches.length === 0) {
    findCount.textContent = "0/0";
    findCount.classList.add("empty");
  } else {
    findCount.textContent = `${current + 1}/${matches.length}`;
    findCount.classList.remove("empty");
  }
}

function scrollToCurrent() {
  const range = matches[current];
  if (!range) return;
  const rect = range.getBoundingClientRect();
  const scroller = document.scrollingElement || document.documentElement;
  if (rect.width === 0 && rect.height === 0) {
    range.startContainer.parentElement?.scrollIntoView({ block: "center" });
    return;
  }
  const target = rect.top + scroller.scrollTop - window.innerHeight / 2;
  scroller.scrollTo({ top: Math.max(0, target), behavior: "smooth" });
}

function runSearch(resetIndex) {
  collectMatches(findInput.value);
  if (matches.length === 0) current = -1;
  else if (resetIndex || current < 0) current = 0;
  else current = Math.min(current, matches.length - 1);
  paintHighlights();
  updateCount();
  if (current >= 0) scrollToCurrent();
}

function step(delta) {
  if (matches.length === 0) return;
  current = (current + delta + matches.length) % matches.length;
  paintHighlights();
  updateCount();
  scrollToCurrent();
}

function openFind() {
  findbar.hidden = false;
  findOpen = true;
  findInput.focus();
  findInput.select();
  if (findInput.value) runSearch(false);
}

function closeFind() {
  findbar.hidden = true;
  findOpen = false;
  clearHighlights();
  updateCount();
}

// Re-run the active search after the document is re-rendered (live reload).
function refreshFind() {
  if (findOpen && findInput.value) runSearch(false);
}

function wireFind() {
  document.getElementById("find-open").addEventListener("click", openFind);
  document.getElementById("find-close").addEventListener("click", closeFind);
  document.getElementById("find-next").addEventListener("click", () => step(1));
  document.getElementById("find-prev").addEventListener("click", () => step(-1));

  findInput.addEventListener("input", () => runSearch(true));
  findInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      step(e.shiftKey ? -1 : 1);
    } else if (e.key === "Escape") {
      e.preventDefault();
      closeFind();
    }
  });

  document.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && !e.altKey && (e.key === "f" || e.key === "F")) {
      e.preventDefault();
      openFind();
    } else if (e.key === "Escape" && findOpen) {
      e.preventDefault();
      closeFind();
    }
  });
}

// ---- Render cycle ----------------------------------------------------------

async function load() {
  assetVersion++;
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
    refreshFind();

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

  wireFind();

  await load();
  await listen("file-changed", load);
  await listen("watch-error", (e) =>
    console.warn("file-watching disabled:", e.payload)
  );
}

init();
