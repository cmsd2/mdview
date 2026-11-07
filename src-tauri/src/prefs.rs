//! Reading preferences at runtime: font face, display text size, and print
//! text size (fixed, or linked to the display size). Owns the View-menu items
//! that change them, keeps their check marks in sync, persists changes via
//! `config.rs`, and emits `prefs-changed` so `app.js` can apply them.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::config::{self, Prefs, DEFAULT_TEXT_SIZE, FONTS, PRINT_SIZES, TEXT_SIZES};

/// Menu labels for each entry in `config::FONTS`.
const FONT_LABELS: &[(&str, &str)] = &[
    ("sans", "Sans-serif"),
    ("serif", "Serif"),
    ("cm", "Computer Modern"),
];

pub struct PrefsState {
    prefs: Mutex<Prefs>,
    /// Filled in once the menu is built.
    checks: Mutex<Option<Checks>>,
}

/// Handles to the check-mark items, so they can mirror the current prefs.
struct Checks {
    fonts: Vec<(&'static str, CheckMenuItem<Wry>)>,
    print_linked: CheckMenuItem<Wry>,
    print_sizes: Vec<(f64, CheckMenuItem<Wry>)>,
}

impl PrefsState {
    pub fn load() -> Self {
        Self { prefs: Mutex::new(config::load_prefs()), checks: Mutex::new(None) }
    }

    pub fn get(&self) -> Prefs {
        self.prefs.lock().expect("prefs poisoned").clone()
    }

    /// Set every check mark from `prefs`. Called after each click too, because
    /// a check item toggles itself before we see the event.
    fn sync_checks(&self, prefs: &Prefs) {
        let checks = self.checks.lock().expect("prefs menu poisoned");
        let Some(checks) = checks.as_ref() else { return };
        for (font, item) in &checks.fonts {
            let _ = item.set_checked(*font == prefs.font);
        }
        let _ = checks.print_linked.set_checked(prefs.link_print_size);
        for (size, item) in &checks.print_sizes {
            let _ = item.set_checked(!prefs.link_print_size && *size == prefs.print_size);
        }
    }
}

#[tauri::command]
pub fn get_prefs(state: tauri::State<'_, PrefsState>) -> Prefs {
    state.get()
}

/// Append the zoom items and the Font / Print Text Size submenus to the View
/// menu.
pub fn add_view_items<'m>(
    app: &'m AppHandle,
    view: SubmenuBuilder<'m, Wry, AppHandle>,
) -> tauri::Result<SubmenuBuilder<'m, Wry, AppHandle>> {
    let zoom_in = MenuItemBuilder::with_id("zoom-in", "Zoom In")
        .accelerator("CmdOrCtrl+=")
        .build(app)?;
    let zoom_out = MenuItemBuilder::with_id("zoom-out", "Zoom Out")
        .accelerator("CmdOrCtrl+-")
        .build(app)?;
    let zoom_reset = MenuItemBuilder::with_id("zoom-reset", "Actual Size")
        .accelerator("CmdOrCtrl+0")
        .build(app)?;

    let mut font_menu = SubmenuBuilder::new(app, "Font");
    let mut fonts = Vec::new();
    for &(font, label) in FONT_LABELS {
        let item = CheckMenuItemBuilder::with_id(format!("font-{font}"), label).build(app)?;
        font_menu = font_menu.item(&item);
        fonts.push((font, item));
    }

    let print_linked =
        CheckMenuItemBuilder::with_id("print-linked", "Same as Display").build(app)?;
    let mut print_menu = SubmenuBuilder::new(app, "Print Text Size")
        .item(&print_linked)
        .separator();
    let mut print_sizes = Vec::new();
    for &size in PRINT_SIZES {
        let item =
            CheckMenuItemBuilder::with_id(format!("print-size-{size}"), format!("{size} pt"))
                .build(app)?;
        print_menu = print_menu.item(&item);
        print_sizes.push((size, item));
    }

    let state = app.state::<PrefsState>();
    *state.checks.lock().expect("prefs menu poisoned") =
        Some(Checks { fonts, print_linked, print_sizes });
    state.sync_checks(&state.get());

    Ok(view
        .separator()
        .item(&zoom_in)
        .item(&zoom_out)
        .item(&zoom_reset)
        .separator()
        .item(&font_menu.build()?)
        .item(&print_menu.build()?))
}

/// Apply a View-menu preference change. Returns `false` for ids that aren't
/// ours, so the caller can try its own.
pub fn handle_menu(app: &AppHandle, id: &str) -> bool {
    let state = app.state::<PrefsState>();
    let before = state.get();
    let mut prefs = before.clone();

    match id {
        "zoom-in" => prefs.text_size = step(prefs.text_size, true),
        "zoom-out" => prefs.text_size = step(prefs.text_size, false),
        "zoom-reset" => prefs.text_size = DEFAULT_TEXT_SIZE,
        "print-linked" => prefs.link_print_size = !prefs.link_print_size,
        _ => {
            if let Some(font) = id.strip_prefix("font-").filter(|f| FONTS.contains(f)) {
                prefs.font = font.to_string();
            } else if let Some(size) =
                id.strip_prefix("print-size-").and_then(|s| s.parse::<f64>().ok())
            {
                prefs.print_size = size;
                prefs.link_print_size = false;
            } else {
                return false;
            }
        }
    }

    *state.prefs.lock().expect("prefs poisoned") = prefs.clone();
    state.sync_checks(&prefs);
    if prefs != before {
        if let Err(e) = config::save_prefs(&prefs) {
            eprintln!("mdview: could not save preferences: {e}");
        }
        let _ = app.emit("prefs-changed", &prefs);
    }
    true
}

/// The next `TEXT_SIZES` step above (or below) `current`, staying put at the
/// ends. A hand-edited size between steps snaps to the neighbouring step.
fn step(current: f64, up: bool) -> f64 {
    let next = if up {
        TEXT_SIZES.iter().copied().find(|&s| s > current + 0.01)
    } else {
        TEXT_SIZES.iter().rev().copied().find(|&s| s < current - 0.01)
    };
    next.unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_through_sizes_and_stops_at_the_ends() {
        assert_eq!(step(12.0, true), 13.0);
        assert_eq!(step(12.0, false), 11.0);
        assert_eq!(step(28.0, true), 28.0);
        assert_eq!(step(9.0, false), 9.0);
        // Off-step values snap to the neighbour in the requested direction.
        assert_eq!(step(15.0, true), 16.0);
        assert_eq!(step(15.0, false), 14.0);
    }

    #[test]
    fn every_font_has_a_label() {
        for font in FONTS {
            assert!(FONT_LABELS.iter().any(|(f, _)| f == font), "no label for {font}");
        }
    }
}
