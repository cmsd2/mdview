//! mdview — a lightweight, read-only Markdown viewer with live file-watching.

mod assets;
mod config;
mod render;
mod watch;

use std::path::PathBuf;
use std::sync::Mutex;

use clap::Parser;
use tauri::{Emitter, Manager, Theme, WebviewWindow};

/// Command-line interface.
#[derive(Parser, Debug)]
#[command(name = "mdview", version, about = "A lightweight, read-only Markdown viewer")]
struct Cli {
    /// Path to the Markdown file to open. If omitted, a welcome screen is shown.
    file: Option<PathBuf>,
}

/// Shared app state: the document being viewed and its directory.
/// Both are `None` when launched without a file (welcome screen).
pub struct AppState {
    pub doc_path: Option<PathBuf>,
    pub base_dir: Option<PathBuf>,
}

#[tauri::command]
fn render(state: tauri::State<'_, AppState>) -> Result<render::RenderedDoc, String> {
    match &state.doc_path {
        Some(path) => render::render(path),
        None => Ok(render::welcome()),
    }
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

pub fn run() {
    let cli = Cli::parse();

    let doc_path = match cli.file {
        Some(file) => {
            let resolved = match file.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    eprintln!("mdview: cannot open '{}': no such file", file.display());
                    std::process::exit(1);
                }
            };
            if !resolved.is_file() {
                eprintln!("mdview: '{}' is not a file", resolved.display());
                std::process::exit(1);
            }
            Some(resolved)
        }
        None => None,
    };
    let base_dir = doc_path
        .as_ref()
        .map(|p| p.parent().map(PathBuf::from).unwrap_or_else(|| p.clone()));

    let state = AppState { doc_path: doc_path.clone(), base_dir };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
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
            // Give the window a sensible title before the first render.
            if let Some(win) = app.get_webview_window("main") {
                let name = doc_path
                    .as_ref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .map(|n| format!("{n} — mdview"))
                    .unwrap_or_else(|| "mdview".to_string());
                let _ = win.set_title(&name);
            }

            // Start the file watcher (only when a file is open); keep it alive
            // in managed state.
            if let Some(path) = doc_path.clone() {
                let handle = app.handle().clone();
                match watch::start(handle, path) {
                    Ok(watcher) => {
                        app.manage(Mutex::new(watcher));
                    }
                    Err(e) => {
                        eprintln!("mdview: file-watching disabled: {e}");
                        let _ = app.emit("watch-error", e);
                    }
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running mdview");
}
