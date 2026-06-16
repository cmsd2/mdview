//! Debounced file watching. Watches the document's *parent directory* (so it
//! survives editors that replace-on-save) and emits a `file-changed` event to
//! the frontend when the document itself changes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, FileIdMap};
use tauri::{AppHandle, Emitter};

pub type FileWatcher = Debouncer<notify::RecommendedWatcher, FileIdMap>;

/// Start watching `doc_path`. The returned watcher must be kept alive for the
/// lifetime of the app (drop it and watching stops).
pub fn start(app: AppHandle, doc_path: PathBuf) -> Result<FileWatcher, String> {
    let watch_target = doc_path
        .canonicalize()
        .unwrap_or_else(|_| doc_path.clone());
    let parent = watch_target
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| watch_target.clone());

    let mut debouncer = new_debouncer(
        Duration::from_millis(150),
        None,
        move |result: DebounceEventResult| {
            let Ok(events) = result else { return };
            let touched = events
                .iter()
                .any(|ev| ev.paths.iter().any(|p| paths_match(p, &watch_target)));
            if touched {
                let _ = app.emit("file-changed", ());
            }
        },
    )
    .map_err(|e| e.to_string())?;

    use notify::Watcher as _;
    debouncer
        .watcher()
        .watch(&parent, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;
    debouncer
        .cache()
        .add_root(&parent, RecursiveMode::NonRecursive);

    Ok(debouncer)
}

/// Compare paths, tolerating that some FS events report non-canonical paths.
fn paths_match(event_path: &Path, target: &Path) -> bool {
    if event_path == target {
        return true;
    }
    match event_path.canonicalize() {
        Ok(c) => c == target,
        Err(_) => event_path.file_name() == target.file_name(),
    }
}
