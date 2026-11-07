//! Persisted user preferences.
//!
//! We persist a theme override plus the reading preferences in [`Prefs`]
//! (font face, display and print text size). A missing or invalid file simply
//! means "defaults" — never an error. The file is created lazily, the first
//! time the user changes a setting.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Display text-size steps (pt) for View > Zoom In / Zoom Out.
pub const TEXT_SIZES: &[f64] = &[9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0];
/// Fixed print text sizes (pt) offered in View > Print Text Size.
pub const PRINT_SIZES: &[f64] = &[9.0, 10.0, 11.0, 12.0, 14.0];
/// Font faces the frontend knows how to apply (`data-font` on `<html>`).
pub const FONTS: &[&str] = &["sans", "serif", "cm"];

/// 12pt = 16px, github-markdown-css's own body size.
pub const DEFAULT_TEXT_SIZE: f64 = 12.0;
const DEFAULT_PRINT_SIZE: f64 = 11.0;

/// Reading preferences, sent as-is to the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// "sans" | "serif" | "cm" (Computer Modern).
    pub font: String,
    /// On-screen body text size, in pt.
    pub text_size: f64,
    /// Printed body text size, in pt. Ignored while `link_print_size` is set.
    pub print_size: f64,
    /// Print at `text_size` instead of `print_size`.
    pub link_print_size: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            font: "sans".to_string(),
            text_size: DEFAULT_TEXT_SIZE,
            print_size: DEFAULT_PRINT_SIZE,
            link_print_size: false,
        }
    }
}

impl Prefs {
    /// Replace unknown faces and out-of-range sizes (from a hand-edited file)
    /// with defaults, so the frontend only ever sees values it can apply.
    fn sanitized(mut self) -> Self {
        let defaults = Prefs::default();
        if !FONTS.contains(&self.font.as_str()) {
            self.font = defaults.font;
        }
        if !(6.0..=48.0).contains(&self.text_size) {
            self.text_size = defaults.text_size;
        }
        if !(6.0..=48.0).contains(&self.print_size) {
            self.print_size = defaults.print_size;
        }
        self
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ConfigFile {
    /// "light" | "dark" (or absent) = follow the OS hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
    #[serde(flatten)]
    prefs: Prefs,
}

/// Per-OS config file path:
/// - Linux:   `${XDG_CONFIG_HOME:-~/.config}/mdview/config.toml`
/// - macOS:   `~/Library/Application Support/mdview/config.toml`
/// - Windows: `%APPDATA%\mdview\config.toml`
fn config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "mdview")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

/// The stored config, or defaults for a missing/unreadable/invalid file.
fn load() -> ConfigFile {
    config_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Write the whole config. Writes are atomic (temp file + rename).
fn save(cfg: &ConfigFile) -> Result<(), String> {
    let path = config_path().ok_or("could not determine config directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let body = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, body).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}

/// A stored theme override, but only if it is a concrete theme.
/// Returns `None` for "system", absent, or any unreadable/invalid file.
pub fn load_override() -> Option<String> {
    match load().theme.as_deref() {
        Some("light") => Some("light".to_string()),
        Some("dark") => Some("dark".to_string()),
        _ => None,
    }
}

/// Persist a theme choice. "system" (or anything that isn't light/dark) clears
/// the override, reverting to the OS hint; other settings are kept.
pub fn save_override(theme: &str) -> Result<(), String> {
    let mut cfg = load();
    cfg.theme = matches!(theme, "light" | "dark").then(|| theme.to_string());
    save(&cfg)
}

/// The stored reading preferences (defaults for anything missing or invalid).
pub fn load_prefs() -> Prefs {
    load().prefs.sanitized()
}

/// Persist reading preferences, keeping the theme override.
pub fn save_prefs(prefs: &Prefs) -> Result<(), String> {
    let mut cfg = load();
    cfg.prefs = prefs.clone();
    save(&cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_file_fills_in_defaults() {
        let cfg: ConfigFile = toml::from_str("theme = \"dark\"\nfont = \"cm\"\n").unwrap();
        assert_eq!(cfg.theme.as_deref(), Some("dark"));
        assert_eq!(cfg.prefs.font, "cm");
        assert_eq!(cfg.prefs.text_size, DEFAULT_TEXT_SIZE);
        assert!(!cfg.prefs.link_print_size);
    }

    #[test]
    fn invalid_values_are_sanitized() {
        let cfg: ConfigFile =
            toml::from_str("font = \"comic\"\ntext_size = 200\nprint_size = 10\n").unwrap();
        let prefs = cfg.prefs.sanitized();
        assert_eq!(prefs.font, "sans");
        assert_eq!(prefs.text_size, DEFAULT_TEXT_SIZE);
        assert_eq!(prefs.print_size, 10.0);
    }

    #[test]
    fn round_trips_through_toml() {
        let cfg = ConfigFile {
            theme: None,
            prefs: Prefs { font: "serif".into(), text_size: 14.0, print_size: 10.0, link_print_size: true },
        };
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: ConfigFile = toml::from_str(&text).unwrap();
        assert_eq!(back.prefs, cfg.prefs);
        assert!(back.theme.is_none());
    }
}
