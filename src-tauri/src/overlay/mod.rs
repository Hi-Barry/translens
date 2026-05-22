//! Floating overlay button
//! On Windows: creates a small transparent WebView window next to selected text
//! On other platforms: NOP (no overlay)

#[cfg(target_os = "windows")]
#[path = "win.rs"]
mod platform;

#[cfg(not(target_os = "windows"))]
#[path = "stub.rs"]
mod platform;

use tauri::AppHandle;

/// Show a small floating button at the given screen position
pub fn show_button(handle: AppHandle, x: i32, y: i32, text: String) {
    platform::show_button_impl(handle, x, y, text);
}

/// Hide the floating overlay button if it's visible
pub fn hide_button(handle: &AppHandle) {
    platform::hide_button_impl(handle);
}

/// Handle overlay button click: close button, open translator with stored text
pub fn on_overlay_click(handle: &AppHandle) {
    platform::on_overlay_click_impl(handle);
}

/// Show translator directly (skip overlay button), positioned at cursor
pub fn on_direct_translate(handle: &AppHandle, text: String) {
    platform::on_direct_translate_impl(handle, text);
}
