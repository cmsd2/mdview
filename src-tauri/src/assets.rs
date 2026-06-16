//! `mdview://` custom URI scheme handler.
//!
//! Serves files relative to the open document's directory, with a
//! path-traversal guard so a malicious link cannot read outside that tree.

use percent_encoding::percent_decode_str;
use std::path::{Path, PathBuf};
use tauri::http::{Request, Response};

use crate::AppState;

/// Resolve and serve a single asset request. Always returns a `Response`
/// (404/403 with an empty body on failure) so the protocol never panics.
pub fn handle(state: &AppState, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    let Some(base_dir) = state.base_dir.as_deref() else {
        return not_found(); // no document open -> nothing to serve
    };
    match resolve(base_dir, request.uri().path()) {
        Some(path) => match std::fs::read(&path) {
            Ok(bytes) => {
                let mime = mime_guess::from_path(&path).first_or_octet_stream();
                Response::builder()
                    .status(200)
                    .header("Content-Type", mime.as_ref())
                    .header("Access-Control-Allow-Origin", "*")
                    .body(bytes)
                    .unwrap_or_else(|_| not_found())
            }
            Err(_) => not_found(),
        },
        None => forbidden(),
    }
}

/// Map a request path to a real file inside `base_dir`, or `None` if it would
/// escape the directory (or doesn't resolve).
fn resolve(base_dir: &Path, uri_path: &str) -> Option<PathBuf> {
    let rel = percent_decode_str(uri_path.trim_start_matches('/'))
        .decode_utf8()
        .ok()?
        .into_owned();

    let candidate = base_dir.join(rel);
    let canonical = candidate.canonicalize().ok()?;
    let base = base_dir.canonicalize().ok()?;

    if canonical.starts_with(&base) {
        Some(canonical)
    } else {
        None // path traversal attempt — refuse
    }
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder().status(404).body(Vec::new()).unwrap()
}

fn forbidden() -> Response<Vec<u8>> {
    Response::builder().status(403).body(Vec::new()).unwrap()
}
