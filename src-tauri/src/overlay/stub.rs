//! Stub overlay for non-Windows platforms
use tauri::AppHandle;

pub fn show_button_impl(_handle: AppHandle, _x: i32, _y: i32, _text: String) {}
pub fn hide_button_impl(_handle: &AppHandle) {}
pub fn on_overlay_click_impl(_handle: &AppHandle) {}
pub fn on_direct_translate_impl(_handle: &AppHandle, _text: String) {}
