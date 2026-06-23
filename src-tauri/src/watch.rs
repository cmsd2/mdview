//! File watching abstraction: watch a set of file paths and get a
//! `file-changed` event whenever any of them is created, modified, or removed —
//! **without caring whether the file or its parent directories exist yet**.
//!
//! Callers hand [`FileWatcher::watch`] a flat list of absolute file paths and
//! nothing else. Directory bookkeeping, not-yet-existing paths, and progressive
//! descent into folders that appear later are all handled internally.
//!
//! Two watchers cooperate, tuned to stay cheap even when a target lives in a
//! large project (e.g. a README at a monorepo root):
//! - an **OS event watcher** on the individual files that *exist*. Watching a
//!   file rather than its directory keeps macOS FSEvents from streaming the
//!   surrounding subtree to us, and gives instant updates for in-place edits.
//! - a **poll watcher** on directories — the nearest existing ancestor of each
//!   target. A non-recursive poll is a one-level scan, so its cost is bounded by
//!   a directory's direct children regardless of project size. It catches what
//!   file-watches can't: targets/folders that don't exist yet appearing, and
//!   replace-on-save on Linux (where an inode-based file watch goes stale).
//!
//! A background owner thread owns the watchers and re-reconciles whenever its
//! own events reveal that a target or an intermediate folder has come into
//! existence, so descent needs no help from the caller.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notify::{Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher as _};
use notify_debouncer_full::{
    new_debouncer, new_debouncer_opt, DebounceEventResult, Debouncer, FileIdMap,
};
use tauri::{AppHandle, Emitter};

const DEBOUNCE: Duration = Duration::from_millis(150);
/// How often directories are polled. A backstop for creation/replacement, not
/// the primary update path (in-place edits arrive through OS events instantly),
/// so a relaxed interval keeps idle cost low.
const POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The set of file paths whose changes should emit `file-changed`. Shared with
/// the watcher callbacks (read) and the owner thread (write).
struct Tracked {
    targets: Vec<PathBuf>,
}

impl Tracked {
    /// True if a change at `path` should trigger a reload. Matches each target
    /// directly; also matches a directory that is an *ancestor* of a target, so
    /// creating an intermediate folder prompts a reload — and a re-reconcile
    /// that descends into it.
    fn is_trigger(&self, path: &Path) -> bool {
        self.targets
            .iter()
            .any(|t| paths_match(path, t) || t.starts_with(path))
    }
}

/// Commands sent to the owner thread.
enum Cmd {
    SetTargets(Vec<PathBuf>),
    Reconcile,
    Stop,
}

/// A handle to a running watcher. Dropping it stops watching.
pub struct FileWatcher {
    tx: Sender<Cmd>,
    handle: Option<JoinHandle<()>>,
}

impl FileWatcher {
    /// Watch exactly `targets` (absolute file paths), replacing any previous
    /// set. Emits `file-changed` when any target is created, modified, or
    /// removed, whether or not it (or its parent directories) currently exists.
    pub fn watch(&self, targets: &[PathBuf]) {
        let _ = self.tx.send(Cmd::SetTargets(targets.to_vec()));
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        // Stop the owner thread (which then drops the watchers) and join it.
        let _ = self.tx.send(Cmd::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// Start a watcher initially watching `doc_path`. Add more targets (or drop it)
/// later with [`FileWatcher::watch`].
pub fn start(app: AppHandle, doc_path: PathBuf) -> Result<FileWatcher, String> {
    let doc = doc_path.canonicalize().unwrap_or(doc_path);
    let tracked = Arc::new(Mutex::new(Tracked {
        targets: Vec::new(),
    }));
    let (tx, rx) = mpsc::channel();

    // Both watchers share the trigger set and feed the same owner thread.
    let events = new_debouncer(DEBOUNCE, None, handler(&tracked, &app, &tx))
        .map_err(|e| e.to_string())?;
    let poll = new_debouncer_opt::<_, PollWatcher, FileIdMap>(
        DEBOUNCE,
        None,
        handler(&tracked, &app, &tx),
        FileIdMap::new(),
        Config::default().with_poll_interval(POLL_INTERVAL),
    )
    .map_err(|e| e.to_string())?;

    let handle = thread::Builder::new()
        .name("mdview-watch".into())
        .spawn(move || {
            let mut core = Core {
                events,
                poll,
                tracked,
                watched_files: HashSet::new(),
                polled_dirs: HashSet::new(),
            };
            core.set_targets(vec![doc]);
            while let Ok(cmd) = rx.recv() {
                match cmd {
                    Cmd::SetTargets(targets) => core.set_targets(targets),
                    Cmd::Reconcile => core.reconcile(),
                    Cmd::Stop => break,
                }
            }
            // `core` (and its watchers) drop here — watching stops.
        })
        .map_err(|e| e.to_string())?;

    Ok(FileWatcher {
        tx,
        handle: Some(handle),
    })
}

/// Build a debouncer callback that emits `file-changed` on a trigger and asks
/// the owner thread to re-reconcile (so newly created targets/folders get
/// watched and replaced files get re-armed).
fn handler(
    tracked: &Arc<Mutex<Tracked>>,
    app: &AppHandle,
    tx: &Sender<Cmd>,
) -> impl FnMut(DebounceEventResult) + Send + 'static {
    let tracked = Arc::clone(tracked);
    let app = app.clone();
    let tx = tx.clone();
    move |result| {
        let Ok(events) = result else { return };
        let triggered = {
            let tracked = tracked.lock().expect("watch state poisoned");
            events
                .iter()
                .any(|ev| ev.paths.iter().any(|p| tracked.is_trigger(p)))
        };
        if triggered {
            let _ = app.emit("file-changed", ());
            let _ = tx.send(Cmd::Reconcile);
        }
    }
}

/// Lives on the owner thread; owns the watchers and the registered sets.
struct Core {
    /// OS event watcher, registered on individual existing files.
    events: Debouncer<RecommendedWatcher, FileIdMap>,
    /// Poll watcher, registered on directories.
    poll: Debouncer<PollWatcher, FileIdMap>,
    tracked: Arc<Mutex<Tracked>>,
    watched_files: HashSet<PathBuf>,
    polled_dirs: HashSet<PathBuf>,
}

impl Core {
    fn set_targets(&mut self, targets: Vec<PathBuf>) {
        self.tracked.lock().expect("watch state poisoned").targets = targets;
        self.reconcile();
    }

    /// Re-evaluate which files exist and reconcile the watch registrations:
    /// event-watch the targets that exist, poll the nearest existing ancestor
    /// directory of every target (so missing ones are caught when created), and
    /// drop anything no longer wanted.
    fn reconcile(&mut self) {
        let targets = self
            .tracked
            .lock()
            .expect("watch state poisoned")
            .targets
            .clone();

        let mut want_files = HashSet::new();
        let mut want_dirs = HashSet::new();
        for target in &targets {
            if target.is_file() {
                want_files.insert(target.clone());
            }
            if let Some(dir) = nearest_existing_dir(target) {
                want_dirs.insert(dir);
            }
        }

        for file in stale(&self.watched_files, &want_files) {
            self.unwatch_file(&file);
        }
        for file in &want_files {
            self.watch_file(file);
        }
        for dir in stale(&self.polled_dirs, &want_dirs) {
            self.unpoll_dir(&dir);
        }
        for dir in &want_dirs {
            self.poll_dir(dir);
        }
    }

    /// Event-watch a single file (idempotent; the OS watcher rejects missing
    /// paths, which a poll on the parent directory covers instead).
    fn watch_file(&mut self, file: &Path) {
        if self.watched_files.contains(file) || !file.is_file() {
            return;
        }
        if self
            .events
            .watcher()
            .watch(file, RecursiveMode::NonRecursive)
            .is_ok()
        {
            self.events
                .cache()
                .add_root(file, RecursiveMode::NonRecursive);
            self.watched_files.insert(file.to_path_buf());
        }
    }

    fn unwatch_file(&mut self, file: &Path) {
        let _ = self.events.watcher().unwatch(file);
        self.events.cache().remove_root(file);
        self.watched_files.remove(file);
    }

    /// Poll-watch a directory non-recursively (idempotent).
    fn poll_dir(&mut self, dir: &Path) {
        if self.polled_dirs.contains(dir) {
            return;
        }
        if self
            .poll
            .watcher()
            .watch(dir, RecursiveMode::NonRecursive)
            .is_ok()
        {
            self.poll.cache().add_root(dir, RecursiveMode::NonRecursive);
            self.polled_dirs.insert(dir.to_path_buf());
        }
    }

    fn unpoll_dir(&mut self, dir: &Path) {
        let _ = self.poll.watcher().unwatch(dir);
        self.poll.cache().remove_root(dir);
        self.polled_dirs.remove(dir);
    }
}

/// Entries in `current` no longer present in `wanted`.
fn stale(current: &HashSet<PathBuf>, wanted: &HashSet<PathBuf>) -> Vec<PathBuf> {
    current.difference(wanted).cloned().collect()
}

/// The nearest existing ancestor directory of `path` (its parent, or the first
/// existing directory above it); `None` only if no ancestor exists.
fn nearest_existing_dir(path: &Path) -> Option<PathBuf> {
    let mut cur = path.parent();
    while let Some(dir) = cur {
        if dir.is_dir() {
            return Some(dir.to_path_buf());
        }
        cur = dir.parent();
    }
    None
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
