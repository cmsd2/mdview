//! Native printing for File > Print… and the toolbar button.
//!
//! On macOS we run our own `NSPrintOperation` over the `WKWebView` so the page
//! margins are real AppKit margins, reserved on every page. Tauri's
//! `print()` goes through wry, which zeroes the `NSPrintInfo` margins, and
//! WebKit doesn't reliably apply CSS `@page` margins on that path (the bottom
//! margin went missing). `app.js` cancels the `@page` margins on macOS so they
//! aren't applied twice. Elsewhere the platform WebView's own print UI honours
//! `@page`, so we use Tauri's `print()` unchanged.

use tauri::WebviewWindow;

pub fn print(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::print(window)
    }
    #[cfg(not(target_os = "macos"))]
    {
        window.print().map_err(|e| e.to_string())
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::runtime::NSObjectProtocol;
    use objc2::sel;
    use objc2_app_kit::NSPrintInfo;
    use objc2_web_kit::WKWebView;
    use tauri::WebviewWindow;

    /// Points per millimetre (AppKit margins are in points, 1pt = 1/72in).
    const MM: f64 = 72.0 / 25.4;
    /// Match the `@page` margins in `style.css`: 25mm sides (the usual
    /// word-processor default), 18mm top and bottom.
    const TOP_BOTTOM: f64 = 18.0 * MM;
    const SIDES: f64 = 25.0 * MM;

    pub fn print(window: &WebviewWindow) -> Result<(), String> {
        // with_webview runs the closure on the main thread, as AppKit requires.
        window
            .with_webview(|webview| unsafe {
                // Safety: on macOS `inner()` is the window's live WKWebView.
                let wk: &WKWebView = &*webview.inner().cast();
                // printOperationWithPrintInfo: is macOS 11+.
                if !wk.respondsToSelector(sel!(printOperationWithPrintInfo:)) {
                    eprintln!("mdview: printing needs macOS 11 or later");
                    return;
                }
                let Some(ns_window) = wk.window() else { return };

                // The shared print info carries the user's last paper size and
                // printer between prints; we only take over the margins.
                let info = NSPrintInfo::sharedPrintInfo();
                info.setTopMargin(TOP_BOTTOM);
                info.setBottomMargin(TOP_BOTTOM);
                info.setLeftMargin(SIDES);
                info.setRightMargin(SIDES);

                let op = wk.printOperationWithPrintInfo(&info);
                op.setCanSpawnSeparateThread(true);
                op.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
                    &ns_window,
                    None,
                    None,
                    std::ptr::null_mut(),
                );
            })
            .map_err(|e| e.to_string())
    }
}
