//! Floating overlay button
//! On Windows: creates a thin Layered Window next to selected text
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
