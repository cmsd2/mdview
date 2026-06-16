//! Persisted user preferences.
//!
//! The only thing we persist in v1 is a theme override. A missing or invalid
//! file simply means "no stored preference" — never an error. The file is
//! created lazily, the first time the user picks an explicit theme.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    /// "light" | "dark" | "system" (or absent) = follow the OS hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
}

/// Per-OS config file path:
/// - Linux:   `${XDG_CONFIG_HOME:-~/.config}/mdview/config.toml`
/// - macOS:   `~/Library/Application Support/mdview/config.toml`
/// - Windows: `%APPDATA%\mdview\config.toml`
fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "mdview")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// A stored theme override, but only if it is a concrete theme.
/// Returns `None` for "system", absent, or any unreadable/invalid file.
pub fn load_override() -> Option<String> {
    let path = config_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    let cfg: ConfigFile = toml::from_str(&text).ok()?;
    match cfg.theme.as_deref() {
        Some("light") => Some("light".to_string()),
        Some("dark") => Some("dark".to_string()),
        _ => None,
    }
}

/// Persist a theme choice. "system" (or anything that isn't light/dark) clears
/// the override by removing the file. Writes are atomic (temp file + rename).
pub fn save_override(theme: &str) -> Result<(), String> {
    let path = config_path().ok_or("could not determine config directory")?;

    if theme != "light" && theme != "dark" {
        // Revert to following the OS hint: drop the file entirely.
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let cfg = ConfigFile { theme: Some(theme.to_string()) };
    let body = toml::to_string_pretty(&cfg).map_err(|e| e.to_string())?;

    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}
