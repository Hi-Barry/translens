//! Windows text selection detection
//!
//! Uses direct FFI to Win32 APIs to avoid `windows` crate compatibility issues.
//! Clipboard: user32!OpenClipboard/GetClipboardData/etc.
//! Keyboard simulation: user32!keybd_event
//! Memory: kernel32!GlobalAlloc/GlobalLock/GlobalUnlock

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::AppHandle;

// --- Win32 FFI declarations ---

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hwnd: *const std::ffi::c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(uformat: u32) -> isize;
    fn SetClipboardData(uformat: u32, hmem: isize) -> isize;
    fn keybd_event(bVk: u8, bScan: u8, dwFlags: u32, dwExtraInfo: usize);
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> isize;
    fn GlobalLock(hMem: isize) -> *mut std::ffi::c_void;
    fn GlobalUnlock(hMem: isize) -> i32;
}

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const VK_CONTROL: u8 = 0x11; // VK_CONTROL virtual key code

/// Start the Windows detection background thread
pub fn start_impl(_handle: AppHandle, running: Arc<AtomicBool>) {
    log::info!("Windows detection module started");
    while running.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Capture currently selected text: save clipboard → Ctrl+C → read → restore
pub fn capture_selected_text(app: &AppHandle) -> Option<String> {
    unsafe {
        let saved = save_clipboard_ffi();
        simulate_ctrl_c();
        std::thread::sleep(std::time::Duration::from_millis(150));
        let result = read_clipboard_text(app);
        if let Some(ref prev) = saved {
            restore_clipboard_ffi(prev.as_ptr(), prev.len());
        }
        result
    }
}

/// Save clipboard content (raw UTF-16) before Ctrl+C overwrites it
unsafe fn save_clipboard_ffi() -> Option<Vec<u16>> {
    if OpenClipboard(std::ptr::null()) == 0 {
        return None;
    }
    let handle = GetClipboardData(CF_UNICODETEXT);
    if handle == 0 {
        CloseClipboard();
        return None;
    }
    let ptr = GlobalLock(handle) as *const u16;
    if ptr.is_null() {
        GlobalUnlock(handle);
        CloseClipboard();
        return None;
    }
    let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
    let mut buf = Vec::with_capacity(len + 1);
    for i in 0..=len {
        buf.push(*ptr.add(i));
    }
    GlobalUnlock(handle);
    CloseClipboard();
    Some(buf)
}

/// Restore clipboard content after capture
unsafe fn restore_clipboard_ffi(data: *const u16, len: usize) {
    if OpenClipboard(std::ptr::null()) == 0 || len == 0 {
        return;
    }
    EmptyClipboard();
    let handle = GlobalAlloc(GMEM_MOVEABLE, len * 2);
    if handle != 0 {
        let ptr = GlobalLock(handle) as *mut u16;
        if !ptr.is_null() {
            std::ptr::copy_nonoverlapping(data, ptr, len);
            GlobalUnlock(handle);
            SetClipboardData(CF_UNICODETEXT, handle);
        }
    }
    CloseClipboard();
}

/// Read clipboard text without save/restore
pub fn read_clipboard_text(_app: &AppHandle) -> Option<String> {
    unsafe {
        if OpenClipboard(std::ptr::null()) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle == 0 {
            CloseClipboard();
            return None;
        }
        let ptr = GlobalLock(handle) as *const u16;
        if ptr.is_null() {
            GlobalUnlock(handle);
            CloseClipboard();
            return None;
        }
        let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
        let slice = std::slice::from_raw_parts(ptr, len);
        let s = String::from_utf16_lossy(slice);
        GlobalUnlock(handle);
        CloseClipboard();
        Some(s)
    }
}

/// Simulate Ctrl+C via keybd_event
fn simulate_ctrl_c() {
    unsafe {
        keybd_event(VK_CONTROL, 0, 0, 0);
        keybd_event(0x43, 0, 0, 0); // 'C'
        std::thread::sleep(std::time::Duration::from_millis(20));
        keybd_event(0x43, 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
}
