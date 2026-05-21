//! Windows floating overlay button
//! Creates a small layered window with the translate icon near selected text
//! Clicking the button triggers the translation popup

use tauri::AppHandle;

/// Show overlay button on Windows
/// For Tauri v2, we use a small transparent WebView window as the button
/// rather than raw Win32 (simpler to develop, more flexible)
pub fn show_button_impl(handle: AppHandle, x: i32, y: i32, text: String) {
    // For MVP, we directly show the translation window instead of the overlay button
    // The overlay button will be implemented in the next iteration
    //
    // Approach for the overlay button (future):
    // 1. Create a small Tauri window (32x32) with transparent background
    // 2. Use CSS to render a circular button with translate icon
    // 3. Position it at (x, y) relative to selected text
    // 4. On click, emit event to show translation popup
    // 5. Auto-close after timeout or click outside

    use tauri::{Emitter, Manager};
    use tauri::WebviewWindowBuilder;

    // Position near selection (slightly offset)
    let btn_x = x + 5;
    let btn_y = y - 36; // Above the selection

    // For now: directly open translation popup
    // (no overlay button in MVP - shortcut: tray or planned hotkey)
    let _ = handle.emit("translate-text", &text);

    // Future: create overlay button window
}
