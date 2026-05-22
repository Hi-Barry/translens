//! Windows text selection detection
//!
//! Clipboard read/write is delegated to `tauri-plugin-clipboard-manager` (cleaner API).
//! Only the Ctrl+C keyboard simulation uses raw Win32 `keybd_event`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;

use windows::Win32::UI::Input::KeyboardAndMouse::*;

/// Start the Windows detection background thread
pub fn start_impl(_handle: AppHandle, running: Arc<AtomicBool>) {
    log::info!("Windows detection module started");
    while running.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Capture the currently selected text via clipboard
///
/// 1. Save current clipboard content
/// 2. Simulate Ctrl+C to copy selected text
/// 3. Read clipboard via Tauri plugin
/// 4. Restore original clipboard content
/// 5. Return the captured text
pub fn capture_selected_text(app: &AppHandle) -> Option<String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;

    // Step 1: Save current clipboard
    let saved = save_clipboard(app);

    // Step 2: Simulate Ctrl+C
    simulate_ctrl_c();
    std::thread::sleep(std::time::Duration::from_millis(150));

    // Step 3: Read captured text
    let result = read_clipboard_text(app);

    // Step 4: Restore original clipboard content
    if let Some(prev) = saved {
        restore_clipboard(app, &prev);
    }

    result
}

fn save_clipboard(app: &AppHandle) -> Option<String> {
    app.clipboard()
        .read_text()
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

fn restore_clipboard(app: &AppHandle, text: &str) {
    let _ = app.clipboard().write_text(text.to_string());
}

pub fn read_clipboard_text(app: &AppHandle) -> Option<String> {
    app.clipboard()
        .read_text()
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

/// Simulate Ctrl+C keyboard shortcut to copy selected text
fn simulate_ctrl_c() {
    unsafe {
        keybd_event(VK_CONTROL.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(0x43, 0, KEYBD_EVENT_FLAGS(0), 0);
        std::thread::sleep(std::time::Duration::from_millis(20));
        keybd_event(0x43, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}
