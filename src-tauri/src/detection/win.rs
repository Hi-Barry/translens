//! Windows text selection detection
//! Two methods:
//!   1. Auto-detection via mouse hook (WH_MOUSE_LL) — future
//!   2. Manual capture via Ctrl+C simulation (tray menu / hotkey)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

// CF_UNICODETEXT = 13 — not exposed in windows crate v0.60 DataExchange module
const CF_UNICODETEXT: u32 = 13u32;

/// Start the Windows detection background thread
pub fn start_impl(_handle: AppHandle, running: Arc<AtomicBool>) {
    log::info!("Windows detection module started");
    // Keep thread alive (future: mouse hook)
    while running.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Capture the currently selected text via clipboard
///
/// 1. Save current clipboard content
/// 2. Simulate Ctrl+C
/// 3. Read clipboard text
/// 4. Restore original clipboard content
/// 5. Return the captured text
pub fn capture_selected_text() -> Option<String> {
    unsafe {
        // Step 1: Save current clipboard
        let saved = save_clipboard();

        // Step 2: Simulate Ctrl+C
        simulate_ctrl_c();

        // Small wait for clipboard to update
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Step 3: Read clipboard text
        let result = read_clipboard_text();

        // Step 4: Restore original clipboard content
        restore_clipboard(&saved);

        result
    }
}

/// Save clipboard data for later restoration
struct SavedClipboard {
    data: Vec<u16>,
    _format: u32,
}

fn save_clipboard() -> Option<SavedClipboard> {
    unsafe {
        let opened = OpenClipboard(None).is_ok();
        if opened {
            if let Ok(handle) = GetClipboardData(CF_UNICODETEXT) {
                if !handle.is_invalid() {
                    let ptr = GlobalLock(handle) as *const u16;
                    if !ptr.is_null() {
                        let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
                        let mut data = Vec::with_capacity(len + 1);
                        for i in 0..=len {
                            data.push(*ptr.add(i));
                        }
                        let _ = GlobalUnlock(handle);
                        let _ = CloseClipboard();
                        return Some(SavedClipboard {
                            data,
                            _format: CF_UNICODETEXT,
                        });
                    }
                    let _ = GlobalUnlock(handle);
                }
            }
            let _ = CloseClipboard();
        }
    }
    None
}

fn restore_clipboard(saved: &Option<SavedClipboard>) {
    if let Some(saved) = saved {
        unsafe {
            if OpenClipboard(None).is_ok() {
                let _ = EmptyClipboard();
                if !saved.data.is_empty() {
                    let handle = GlobalAlloc(GMEM_MOVEABLE, saved.data.len() * 2);
                    if let Ok(handle) = handle {
                        if !handle.is_invalid() {
                            let ptr = GlobalLock(handle) as *mut u16;
                            if !ptr.is_null() {
                                std::ptr::copy_nonoverlapping(
                                    saved.data.as_ptr(),
                                    ptr,
                                    saved.data.len(),
                                );
                                let _ = GlobalUnlock(handle);
                                let _ = SetClipboardData(CF_UNICODETEXT, handle);
                            }
                        }
                    }
                }
                let _ = CloseClipboard();
            }
        }
    }
}

pub fn read_clipboard_text() -> Option<String> {
    unsafe {
        let opened = OpenClipboard(None).is_ok();
        if opened {
            if let Ok(handle) = GetClipboardData(CF_UNICODETEXT) {
                if !handle.is_invalid() {
                    let ptr = GlobalLock(handle) as *const u16;
                    if !ptr.is_null() {
                        let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
                        let slice = std::slice::from_raw_parts(ptr, len);
                        let s = String::from_utf16_lossy(slice);
                        let _ = GlobalUnlock(handle);
                        let _ = CloseClipboard();
                        return Some(s);
                    }
                    let _ = GlobalUnlock(handle);
                }
            }
            let _ = CloseClipboard();
        }
    }
    None
}

fn simulate_ctrl_c() {
    unsafe {
        // Press Ctrl
        keybd_event(VK_CONTROL.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
        // Press C
        keybd_event(0x43, 0, KEYBD_EVENT_FLAGS(0), 0); // Virtual key for 'C'
        std::thread::sleep(std::time::Duration::from_millis(20));
        // Release C
        keybd_event(0x43, 0, KEYEVENTF_KEYUP, 0);
        // Release Ctrl
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}
