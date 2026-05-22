//! Text selection detection
//! On Windows: clipboard-based capture + UIA selection hook
//! On other platforms: NOP stub

#[cfg(target_os = "windows")]
#[path = "win.rs"]
mod platform;

#[cfg(not(target_os = "windows"))]
#[path = "stub.rs"]
mod platform;

#[cfg(target_os = "windows")]
mod uia_hook;

use tauri::AppHandle;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub fn start_detection(handle: AppHandle) {
    // Start the legacy clipboard-based detection (Ctrl+C simulation) in its own thread
    let running = Arc::new(AtomicBool::new(true));
    let h = handle.clone();
    std::thread::spawn(move || {
        platform::start_impl(h, running);
    });

    // Start the new UIA-based selection hook (also spawns its own thread)
    #[cfg(target_os = "windows")]
    uia_hook::start_uia_hook(handle);
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
