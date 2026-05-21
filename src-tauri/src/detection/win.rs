//! Windows text selection detection
//! Uses: SetWinEventHook (UIA) + WH_MOUSE_LL (low-level mouse hook)
//! Fallback: clipboard capture on mouse up

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

/// Start Windows-specific detection thread(s)
pub fn start_impl(_handle: AppHandle, _running: Arc<AtomicBool>) {
    // Windows detection will be implemented fully in the next iteration
    // This requires:
    // 1. UIA COM interface for getting selected text + bounding rect
    // 2. Low-level mouse hook (WH_MOUSE_LL) as fallback
    // 3. Clipboard save/restore for fallback text capture
    //
    // For now, the app works with manual triggers (tray menu, hotkeys).
    // Full automatic detection will be added when testing on actual Windows.
    log::info!("Windows detection module loaded (placeholder)");
}
