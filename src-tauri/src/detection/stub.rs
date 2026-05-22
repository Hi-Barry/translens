//! Stub detection for non-Windows platforms
//! On Linux/macOS, text selection detection isn't implemented.
//! Translation can be triggered via:
//! - System tray menu ("Translate clipboard")
//! - Global hotkey (not yet implemented)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

/// Stub - does nothing but stay alive
pub fn start_impl(_handle: AppHandle, running: Arc<AtomicBool>) {
    log::info!("Detection not available on this platform");
    while running.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_secs(10));
    }
}

/// Stub - always returns None on non-Windows
pub fn capture_selected_text(_handle: &AppHandle) -> Option<String> {
    None
}
