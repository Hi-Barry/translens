//! Windows text selection detection
//! Two methods:
//!   1. Auto-detection via mouse hook (WH_MOUSE_LL) — future
//!   2. Manual capture via Ctrl+C simulation (tray menu / hotkey)
//!
//! Uses raw Win32 clipboard APIs directly (via `windows` crate).
//! In v0.60, functions like `GetClipboardData` return `Result<HANDLE>`
//! while memory functions expect `HGLOBAL` — explicit conversions needed.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;
use windows::Win32::System::DataExchange::*;
use windows::Win32::System::Memory::*;
use windows::Win32::UI::Input::KeyboardAndMouse::*;

// CF_UNICODETEXT = 13 — not exposed as const in windows crate v0.60 DataExchange
const CF_UNICODETEXT: u32 = 13u32;

/// Convert HANDLE to HGLOBAL (both are `*mut c_void` newtypes).
/// The win32 clipboard API returns HANDLEs that are really HGLOBALs.
unsafe fn handle_to_hglobal(h: HANDLE) -> HGLOBAL {
    HGLOBAL(h.0)
}

/// Open clipboard safely, returns true if successful.
unsafe fn open_clipboard() -> bool {
    OpenClipboard(None).is_ok()
}

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
/// 2. Simulate Ctrl+C
/// 3. Read clipboard text
/// 4. Restore original clipboard content
/// 5. Return the captured text
pub fn capture_selected_text() -> Option<String> {
    unsafe {
        let saved = save_clipboard();
        simulate_ctrl_c();
        std::thread::sleep(std::time::Duration::from_millis(100));
        let result = read_clipboard_text();
        restore_clipboard(&saved);
        result
    }
}

struct SavedClipboard {
    data: Vec<u16>,
}

fn save_clipboard() -> Option<SavedClipboard> {
    unsafe {
        if !open_clipboard() {
            return None;
        }
        let handle = match GetClipboardData(CF_UNICODETEXT) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return None;
            }
        };
        if handle.is_invalid() {
            let _ = CloseClipboard();
            return None;
        }
        let hglobal = handle_to_hglobal(handle);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            let _ = GlobalUnlock(hglobal);
            let _ = CloseClipboard();
            return None;
        }
        let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
        let mut data = Vec::with_capacity(len + 1);
        for i in 0..=len {
            data.push(*ptr.add(i));
        }
        let _ = GlobalUnlock(hglobal);
        let _ = CloseClipboard();
        Some(SavedClipboard { data })
    }
}

fn restore_clipboard(saved: &Option<SavedClipboard>) {
    let Some(saved) = saved else { return };
    unsafe {
        if !open_clipboard() {
            return;
        }
        let _ = EmptyClipboard();
        if !saved.data.is_empty() {
            let result = GlobalAlloc(GMEM_MOVEABLE, saved.data.len() * 2);
            if let Ok(handle) = result {
                if !handle.is_invalid() {
                    let ptr = GlobalLock(handle) as *mut u16;
                    if !ptr.is_null() {
                        std::ptr::copy_nonoverlapping(
                            saved.data.as_ptr(),
                            ptr,
                            saved.data.len(),
                        );
                        let _ = GlobalUnlock(handle);
                        // SetClipboardData takes HANDLE; convert HGLOBAL
                        let _ = SetClipboardData(CF_UNICODETEXT, HANDLE(handle.0));
                    }
                }
            }
        }
        let _ = CloseClipboard();
    }
}

pub fn read_clipboard_text() -> Option<String> {
    unsafe {
        if !open_clipboard() {
            return None;
        }
        let handle = match GetClipboardData(CF_UNICODETEXT) {
            Ok(h) => h,
            Err(_) => {
                let _ = CloseClipboard();
                return None;
            }
        };
        if handle.is_invalid() {
            let _ = CloseClipboard();
            return None;
        }
        let hglobal = handle_to_hglobal(handle);
        let ptr = GlobalLock(hglobal) as *const u16;
        if ptr.is_null() {
            let _ = GlobalUnlock(hglobal);
            let _ = CloseClipboard();
            return None;
        }
        let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf16_lossy(slice);
        let _ = GlobalUnlock(hglobal);
        let _ = CloseClipboard();
        Some(s)
    }
}

fn simulate_ctrl_c() {
    unsafe {
        keybd_event(VK_CONTROL.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(0x43, 0, KEYBD_EVENT_FLAGS(0), 0); // 'C'
        std::thread::sleep(std::time::Duration::from_millis(20));
        keybd_event(0x43, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}
