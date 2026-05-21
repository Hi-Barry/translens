//! Text selection detection
//! On Windows: UIA events + mouse hooks
//! On other platforms: clipboard polling stub

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
