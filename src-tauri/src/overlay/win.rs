//! Windows floating overlay button
//!
//! Creates and manages a small transparent WebView window with a translate icon.
//! The button appears near selected text; clicking it opens the translator window
//! with the pre-captured text.
//!
//! Uses a dedicated Tauri WebviewWindow with transparent background,
//! always-on-top, and no decorations.

use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter, Manager};

/// Global reference to the overlay button's window label.
/// We reuse the same window rather than creating/destroying on each show.
static OVERLAY_LABEL: &str = "overlay-button";

/// Global state: text associated with the currently visible overlay button.
/// Stored so clicking the button knows which text to translate.
static OVERLAY_TEXT: OnceLock<Mutex<String>> = OnceLock::new();

fn overlay_text() -> &'static Mutex<String> {
    OVERLAY_TEXT.get_or_init(|| Mutex::new(String::new()))
}

// ── Public API ──────────────────────────────────────────────────────

pub fn show_button_impl(handle: AppHandle, x: i32, y: i32, text: String) {
    // Store text for later use when button is clicked
    if let Ok(mut t) = overlay_text().lock() {
        *t = text.clone();
    }

    // Try to show existing window, or create one
    if let Some(window) = handle.get_webview_window(OVERLAY_LABEL) {
        // Window exists — just reposition and show
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        // Create the overlay button window
        match tauri::WebviewWindowBuilder::new(
            &handle,
            OVERLAY_LABEL,
            tauri::WebviewUrl::App("overlay.html".into()),
        )
        .title("")
        .inner_size(36.0, 36.0)
        .min_inner_size(36.0, 36.0)
        .max_inner_size(36.0, 36.0)
        .position(x as f64, y as f64)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .visible(true)
        .resizable(false)
        .shadow(false)
        .skip_taskbar(true)
        .build()
        {
            Ok(_) => {}
            Err(e) => {
                log::error!("Failed to create overlay button: {}", e);
                return;
            }
        }
    }

    // Auto-hide after timeout (configurable via config)
    let handle_clone = handle.clone();
    let timeout = crate::config::AppConfig::load().overlay_timeout_ms;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(timeout));
        hide_button_impl(&handle_clone);
    });
}

pub fn hide_button_impl(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window(OVERLAY_LABEL) {
        let _ = window.hide();
    }
}

/// Called when the overlay button is clicked via IPC — opens translator.
pub fn on_overlay_click(handle: &AppHandle) {
    // Hide the button immediately
    hide_button_impl(handle);

    // Get the stored text
    let text = overlay_text()
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();

    if text.is_empty() {
        return;
    }

    // Get cursor position for the translator window
    let (cx, cy) = get_cursor_pos();

    // Show translator with the text
    let handle = handle.clone();
    tauri::async_runtime::spawn(async move {
        let _ = crate::commands::show_translator_window_inner(
            handle,
            text,
            cx,
            cy,
        )
        .await;
    });
}

/// Simpler variant: show translator directly (no overlay button).
pub fn on_direct_translate(handle: &AppHandle, text: String) {
    let handle = handle.clone();
    let (cx, cy) = get_cursor_pos();
    tauri::async_runtime::spawn(async move {
        let _ = crate::commands::show_translator_window_inner(
            handle,
            text,
            cx,
            cy,
        )
        .await;
    });
}

/// Called when the overlay button is clicked — opens translator with the stored text.
pub fn on_overlay_click_impl(handle: &AppHandle) {
    on_overlay_click(handle);
}

/// Show translator directly (skip overlay), positioned at cursor.
pub fn on_direct_translate_impl(handle: &AppHandle, text: String) {
    on_direct_translate(handle, text);
}

fn get_cursor_pos() -> (i32, i32) {
    #[cfg(target_os = "windows")]
    {
        #[link(name = "user32")]
        extern "system" {
            fn GetCursorPos(lpPoint: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            let mut pt: [i32; 2] = [0, 0];
            GetCursorPos(pt.as_mut_ptr() as *mut std::ffi::c_void);
            return (pt[0], pt[1]);
        }
    }
    #[cfg(not(target_os = "windows"))]
    (0, 0)
}
