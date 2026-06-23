//! mdview — a lightweight, read-only Markdown viewer with live file-watching.

mod assets;
mod config;
mod render;
mod watch;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use clap::Parser;
use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
#[cfg(target_os = "macos")]
use tauri::menu::AboutMetadata;
use tauri::{AppHandle, Emitter, Manager, RunEvent, Theme, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

/// Command-line interface.
#[derive(Parser, Debug)]
#[command(name = "mdview", version, about = "A lightweight, read-only Markdown viewer")]
struct Cli {
    /// Path to the Markdown file to open. If omitted, a welcome screen is shown.
    file: Option<PathBuf>,
}

/// Shared app state: the document being viewed and its directory. Wrapped in a
/// `Mutex` because the open document can change at runtime (File > Open…, drag
/// onto the dock icon, or the macOS "Open Document" Apple Event from Finder).
pub struct AppState {
    pub doc: Mutex<DocState>,
}

#[derive(Default, Clone)]
pub struct DocState {
    pub doc_path: Option<PathBuf>,
    pub base_dir: Option<PathBuf>,
}

impl AppState {
    pub fn snapshot(&self) -> DocState {
        self.doc.lock().expect("doc state poisoned").clone()
    }
}

/// Holds the currently-active file watcher so dropping it (when the doc
/// changes) stops watching the old file.
struct WatcherState(Mutex<Option<watch::FileWatcher>>);

#[tauri::command]
fn render(
    state: tauri::State<'_, AppState>,
    watcher: tauri::State<'_, WatcherState>,
) -> Result<render::RenderedDoc, String> {
    let snap = state.snapshot();
    let Some(path) = snap.doc_path else {
        return Ok(render::welcome());
    };

    let doc = render::render(&path)?;

    // Keep the watcher's target set in sync with what this render references:
    // the document plus its included images/media. Files the document no longer
    // references drop off; ones it newly references (even not-yet-created) are
    // picked up.
    let mut targets = vec![path];
    if let Some(base) = snap.base_dir.as_deref() {
        targets.extend(render::local_asset_paths(base, &doc.html));
    }
    if let Some(w) = watcher.0.lock().expect("watcher state poisoned").as_ref() {
        w.watch(&targets);
    }

    Ok(doc)
}

#[tauri::command]
fn get_theme(window: WebviewWindow) -> String {
    resolve_theme(&window)
}

#[tauri::command]
fn set_theme(window: WebviewWindow, theme: String) -> Result<String, String> {
    config::save_override(&theme)?;
    Ok(resolve_theme(&window))
}

/// Platform-correct base URL for the `mdview://` asset scheme.
#[tauri::command]
fn asset_base() -> String {
    if cfg!(windows) {
        "http://mdview.localhost/".to_string()
    } else {
        "mdview://localhost/".to_string()
    }
}

#[tauri::command]
fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

/// Resolve the effective theme: stored override → OS hint → light fallback.
fn resolve_theme(window: &WebviewWindow) -> String {
    if let Some(stored) = config::load_override() {
        return stored;
    }
    match window.theme() {
        Ok(Theme::Dark) => "dark".to_string(),
        _ => "light".to_string(),
    }
}

/// Swap the currently-open document. Validates the path, restarts the watcher,
/// retitles the window, and asks the frontend to re-render. Used by the menu,
/// the macOS Open-Document event, and the initial CLI-arg load.
fn open_document(app: &AppHandle, requested: &Path) -> Result<(), String> {
    let resolved = requested
        .canonicalize()
        .map_err(|e| format!("cannot open '{}': {e}", requested.display()))?;
    if !resolved.is_file() {
        return Err(format!("'{}' is not a file", resolved.display()));
    }
    let base_dir = resolved
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| resolved.clone());

    {
        let state = app.state::<AppState>();
        let mut doc = state.doc.lock().expect("doc state poisoned");
        doc.doc_path = Some(resolved.clone());
        doc.base_dir = Some(base_dir);
    }

    let new_watcher = watch::start(app.clone(), resolved.clone())?;
    {
        let watcher_state = app.state::<WatcherState>();
        let mut w = watcher_state.0.lock().expect("watcher state poisoned");
        // Swap the new watcher in, then drop the old one *after* releasing the
        // lock — dropping it joins its owner thread, which would otherwise block
        // any concurrent `render` waiting on this same lock.
        let old = w.replace(new_watcher);
        drop(w);
        drop(old);
    }

    if let Some(win) = app.get_webview_window("main") {
        let name = resolved
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| format!("{n} — mdview"))
            .unwrap_or_else(|| "mdview".to_string());
        let _ = win.set_title(&name);
    }

    let _ = app.emit("file-changed", ());
    Ok(())
}

/// Install a `mdview` symlink into `~/.local/bin` pointing at the running
/// binary, then report the outcome in a dialog. macOS-only: when the app is
/// dragged into `/Applications` its executable isn't on `$PATH`, so this gives
/// terminal users a `mdview` command without needing `sudo` or `/usr/local/bin`.
#[cfg(target_os = "macos")]
fn install_cli(app: &AppHandle) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

    let (title, body, kind) = match link_cli() {
        Ok(link) => (
            "Command installed",
            format!(
                "Installed the `mdview` command at:\n{}\n\nIf your terminal can't \
                 find it, add this line to your shell profile (e.g. ~/.zshrc) and \
                 open a new terminal:\n\n    export PATH=\"$HOME/.local/bin:$PATH\"",
                link.display()
            ),
            MessageDialogKind::Info,
        ),
        Err(e) => ("Could not install command", e, MessageDialogKind::Error),
    };
    app.dialog().message(body).title(title).kind(kind).show(|_| {});
}

/// Create (or refresh) the `~/.local/bin/mdview` symlink to this executable.
/// Returns the link path on success.
#[cfg(target_os = "macos")]
fn link_cli() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate the app binary: {e}"))?;
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    let dir = PathBuf::from(home).join(".local/bin");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let link = dir.join("mdview");

    // Replace a symlink left by a previous install (or a dead link), but never
    // clobber a real file the user put there themselves.
    match std::fs::symlink_metadata(&link) {
        Ok(meta) if meta.file_type().is_symlink() => std::fs::remove_file(&link)
            .map_err(|e| format!("cannot replace {}: {e}", link.display()))?,
        Ok(_) => {
            return Err(format!(
                "{} already exists and isn't a symlink — remove it first.",
                link.display()
            ))
        }
        Err(_) => {} // nothing there yet
    }

    std::os::unix::fs::symlink(&exe, &link)
        .map_err(|e| format!("cannot create symlink {}: {e}", link.display()))?;
    Ok(link)
}

/// Show the native open-file dialog and load whatever the user picks.
fn prompt_open(app: &AppHandle) {
    let app = app.clone();
    app.clone()
        .dialog()
        .file()
        .add_filter("Markdown", &["md", "markdown", "mdown", "mkd", "mkdn"])
        .pick_file(move |chosen| {
            let Some(file) = chosen else { return };
            if let Ok(path) = file.into_path() {
                if let Err(e) = open_document(&app, &path) {
                    eprintln!("mdview: {e}");
                }
            }
        });
}

/// Build the application menu. On macOS we include the standard App / Edit /
/// Window submenus so the menubar stays usable; everywhere else just File +
/// Edit is enough.
fn install_menu(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItemBuilder::with_id("open", "Open…")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let reload = MenuItemBuilder::with_id("reload", "Reload")
        .accelerator("CmdOrCtrl+R")
        .build(app)?;

    let mut menu_builder = MenuBuilder::new(app);

    #[cfg(target_os = "macos")]
    {
        let install_cli = MenuItemBuilder::with_id("install-cli", "Install 'mdview' Shell Command")
            .build(app)?;
        let app_submenu = SubmenuBuilder::new(app, "mdview")
            .about(Some(AboutMetadata::default()))
            .separator()
            .item(&install_cli)
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?;
        menu_builder = menu_builder.item(&app_submenu);
    }

    let file_submenu = SubmenuBuilder::new(app, "File")
        .item(&open)
        .separator()
        .close_window()
        .build()?;

    let edit_submenu = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let view_submenu = SubmenuBuilder::new(app, "View").item(&reload).build()?;

    menu_builder = menu_builder
        .item(&file_submenu)
        .item(&edit_submenu)
        .item(&view_submenu);

    #[cfg(target_os = "macos")]
    {
        let window_submenu = SubmenuBuilder::new(app, "Window")
            .minimize()
            .maximize()
            .separator()
            .fullscreen()
            .build()?;
        menu_builder = menu_builder.item(&window_submenu);
    }

    let menu = menu_builder.build()?;
    app.set_menu(menu)?;

    let handle = app.clone();
    app.on_menu_event(move |_, event| match event.id().0.as_str() {
        "open" => prompt_open(&handle),
        "reload" => {
            let _ = handle.emit("file-changed", ());
        }
        #[cfg(target_os = "macos")]
        "install-cli" => install_cli(&handle),
        _ => {}
    });

    Ok(())
}

pub fn run() {
    let cli = Cli::parse();

    // CLI arg is validated up-front so terminal users still get a clear error
    // before the GUI launches. Finder/Open-Document arrivals come in later via
    // RunEvent::Opened and go through open_document().
    let cli_doc = cli.file.as_ref().map(|file| match file.canonicalize() {
        Ok(p) if p.is_file() => p,
        Ok(p) => {
            eprintln!("mdview: '{}' is not a file", p.display());
            std::process::exit(1);
        }
        Err(_) => {
            eprintln!("mdview: cannot open '{}': no such file", file.display());
            std::process::exit(1);
        }
    });

    let initial_doc = DocState {
        base_dir: cli_doc
            .as_ref()
            .map(|p| p.parent().map(PathBuf::from).unwrap_or_else(|| p.clone())),
        doc_path: cli_doc.clone(),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState { doc: Mutex::new(initial_doc) })
        .manage(WatcherState(Mutex::new(None)))
        .register_uri_scheme_protocol("mdview", |ctx, request| {
            let state = ctx.app_handle().state::<AppState>();
            assets::handle(&state, &request)
        })
        .invoke_handler(tauri::generate_handler![
            render,
            get_theme,
            set_theme,
            asset_base,
            open_external
        ])
        .setup(move |app| {
            install_menu(app.handle())?;

            // Give the window a sensible title before the first render.
            if let Some(win) = app.get_webview_window("main") {
                let name = cli_doc
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|n| format!("{n} — mdview"))
                    .unwrap_or_else(|| "mdview".to_string());
                let _ = win.set_title(&name);
            }

            if let Some(path) = cli_doc.clone() {
                match watch::start(app.handle().clone(), path) {
                    Ok(watcher) => {
                        let state = app.state::<WatcherState>();
                        *state.0.lock().expect("watcher state poisoned") = Some(watcher);
                    }
                    Err(e) => {
                        eprintln!("mdview: file-watching disabled: {e}");
                        let _ = app.emit("watch-error", e);
                    }
                }
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building mdview");

    app.run(|app_handle, event| {
        // macOS "Open Document" Apple Event — fires when the user opens a .md
        // file via Finder, drags onto the dock, or `open -a mdview file.md`.
        if let RunEvent::Opened { urls } = event {
            for url in urls {
                if let Ok(path) = url.to_file_path() {
                    if let Err(e) = open_document(app_handle, &path) {
                        eprintln!("mdview: {e}");
                    }
                }
            }
        }
    });
}
