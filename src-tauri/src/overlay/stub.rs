//! Stub overlay for non-Windows platforms
use tauri::AppHandle;

pub fn show_button_impl(_handle: AppHandle, _x: i32, _y: i32, _text: String) {
    // No overlay on non-Windows platforms
}
