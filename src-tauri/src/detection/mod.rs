//! Text selection detection
//! On Windows: clipboard-based capture + mouse hooks
//! On other platforms: NOP stub

#[cfg(target_os = "windows")]
#[path = "win.rs"]
mod platform;

#[cfg(not(target_os = "windows"))]
#[path = "stub.rs"]
mod platform;

use tauri::AppHandle;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub fn start_detection(handle: AppHandle) {
    let running = Arc::new(AtomicBool::new(true));
    platform::start_impl(handle, running);
}

/// Capture currently selected text (Windows clipboard-based)
/// Returns None on non-Windows platforms or if no text is selected
#[cfg(target_os = "windows")]
pub fn capture_selected_text(handle: &AppHandle) -> Option<String> {
    platform::capture_selected_text(handle)
}

#[cfg(not(target_os = "windows"))]
pub fn capture_selected_text(_handle: &AppHandle) -> Option<String> {
    None
}

/// Read clipboard text directly (no save/restore, no Ctrl+C simulation)
#[cfg(target_os = "windows")]
pub fn read_clipboard_text(handle: &AppHandle) -> Option<String> {
    platform::read_clipboard_text(handle)
}

#[cfg(not(target_os = "windows"))]
pub fn read_clipboard_text(_handle: &AppHandle) -> Option<String> {
    None
}
