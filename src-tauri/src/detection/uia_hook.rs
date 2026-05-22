//! Windows UIA-based text selection detection
//!
//! Uses SetWinEventHook(EVENT_OBJECT_SELECTION) to detect when the user
//! selects text in any accessible application. On detection, queries
//! UI Automation for the selected text and screen position, then emits
//! a Tauri event for the overlay button to display.
//!
//! No clipboard interaction — reads selection directly via UIA TextPattern.
//! Uses raw COM FFI (consistent with existing win.rs pattern).

#![cfg(target_os = "windows")]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// ====================================================================
// Win32 FFI declarations (raw, matching existing win.rs pattern)
// ====================================================================

#[link(name = "user32")]
extern "system" {
    fn SetWinEventHook(
        eventMin: u32,
        eventMax: u32,
        hmodWinEventProc: *const std::ffi::c_void,
        lpfnWinEventProc: unsafe extern "system" fn(
            hWinEventHook: *const std::ffi::c_void,
            event: u32,
            hwnd: *const std::ffi::c_void,
            idObject: i32,
            idChild: i32,
            dwEventThread: u32,
            dwmsEventTime: u32,
        ),
        idProcess: u32,
        idThread: u32,
        dwFlags: u32,
    ) -> *const std::ffi::c_void;

    fn UnhookWinEvent(hWinEventHook: *const std::ffi::c_void) -> i32;
    fn GetMessageW(lpMsg: *mut std::ffi::c_void, hWnd: *const std::ffi::c_void, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const std::ffi::c_void) -> i32;
    fn DispatchMessageW(lpMsg: *const std::ffi::c_void) -> isize;
    fn GetCursorPos(lpPoint: *mut std::ffi::c_void) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(pvReserved: *const std::ffi::c_void, dwCoInit: u32) -> i32;
    fn CoCreateInstance(
        rclsid: *const std::ffi::c_void, pUnkOuter: *const std::ffi::c_void,
        dwClsContext: u32, riid: *const std::ffi::c_void,
        ppv: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn CoUninitialize();
}

#[link(name = "oleaut32")]
extern "system" {
    fn SafeArrayGetDim(psa: *const std::ffi::c_void) -> u32;
    fn SafeArrayGetUBound(psa: *const std::ffi::c_void, nDim: u32, plBound: *mut i32) -> i32;
    fn SafeArrayAccessData(psa: *const std::ffi::c_void, ppvData: *mut *mut std::ffi::c_void) -> i32;
    fn SafeArrayUnaccessData(psa: *const std::ffi::c_void) -> i32;
    fn SafeArrayDestroy(psa: *mut std::ffi::c_void) -> i32;
    fn SysFreeString(bstrString: *mut u16);
}

// ====================================================================
// Constants
// ====================================================================

const EVENT_OBJECT_SELECTION: u32 = 0x8006;
const WINEVENT_OUTOFCONTEXT: u32 = 0;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;
const COINIT_APARTMENTTHREADED: u32 = 0x2;
const CLSCTX_INPROC_SERVER: u32 = 1;
const S_OK: i32 = 0;
const UIA_TEXTPATTERN_ID: i32 = 10018;

// CLSID_CUIAutomation: {ff48dba4-60ef-4201-aa87-54103eef594e}
const CLSID_CUIAUTOMATION: [u8; 16] = [
    0xa4, 0xdb, 0x48, 0xff, 0xef, 0x60, 0x01, 0x42,
    0xaa, 0x87, 0x54, 0x10, 0x3e, 0xef, 0x59, 0x4e,
];

// IID_IUIAutomation: {30cbe57d-d9d0-452a-ab13-7ac5ac4825ee}
const IID_IUIAUTOMATION: [u8; 16] = [
    0x7d, 0xe5, 0xcb, 0x30, 0xd0, 0xd9, 0x2a, 0x45,
    0xab, 0x13, 0x7a, 0xc5, 0xac, 0x48, 0x25, 0xee,
];

// IID_IUIAutomationTextPattern: {92e1daa0-fe1f-4526-90e2-0db36f3c0f60}
const IID_IUIAUTOMATION_TEXTPATTERN: [u8; 16] = [
    0xa0, 0xda, 0xe1, 0x92, 0x1f, 0xfe, 0x26, 0x45,
    0x90, 0xe2, 0x0d, 0xb3, 0x6f, 0x3c, 0x0f, 0x60,
];

// IID_IUIAutomationTextRange: {a543cc6a-2796-476c-ba32-1c857c4405ed}
const IID_IUIAUTOMATION_TEXTRANGE: [u8; 16] = [
    0x6a, 0xcc, 0x43, 0xa5, 0x96, 0x27, 0x6c, 0x47,
    0xba, 0x32, 0x1c, 0x85, 0x7c, 0x44, 0x05, 0xed,
];

// ====================================================================
// COM vtable helpers
// ====================================================================

/// COM vtable cell: a function pointer stored in the vtable.
type VtblCell = *const std::ffi::c_void;

/// Read vtable entry at given index from a COM interface pointer.
unsafe fn vtbl_entry(obj: *const std::ffi::c_void, idx: usize) -> VtblCell {
    let vtable_ptr = *(obj as *const *const VtblCell);
    *vtable_ptr.add(idx)
}

// ── IUIAutomation helpers ───────────────────────────────────────────
// IUIAutomationIID vtable indices:
// 0=QI, 1=AddRef, 2=Release, 3=CompareElements, 4=CompareRuntimeIds,
// 5=GetRootElement, 6=ElementFromHandle, 7=ElementFromPoint,
// 8=GetFocusedElement, ...

/// IUIAutomation::GetFocusedElement → element pointer
unsafe fn uia_get_focused_element(
    uia: *const std::ffi::c_void,
) -> Option<*mut std::ffi::c_void> {
    let func: unsafe extern "system" fn(
        *const std::ffi::c_void, *mut *mut std::ffi::c_void,
    ) -> i32 = std::mem::transmute(vtbl_entry(uia, 8));

    let mut element = std::ptr::null_mut();
    let hr = func(uia, &mut element);
    if hr >= 0 && !element.is_null() { Some(element) } else { None }
}

// ── IUIAutomationElement helpers ────────────────────────────────────
// Indices: 0=QI, 1=AddRef, 2=Release, ...
// GetCurrentPatternAs is at a late index. Rather than hardcoding,
// we use QueryInterface (index 0) with the TextPattern IID.
// UIA allows this because IUIAutomationTextPattern is a standard interface
// that the element may QueryInterface for if it supports text selection.

/// Call IUIAutomationElement::QueryInterface(IID_TextPattern) → pattern ptr
unsafe fn element_query_text_pattern(
    element: *const std::ffi::c_void,
) -> Option<*mut std::ffi::c_void> {
    let func: unsafe extern "system" fn(
        *const std::ffi::c_void,
        *const std::ffi::c_void,
        *mut *mut std::ffi::c_void,
    ) -> i32 = std::mem::transmute(vtbl_entry(element, 0));

    let mut pattern = std::ptr::null_mut();
    let hr = func(
        element,
        IID_IUIAUTOMATION_TEXTPATTERN.as_ptr() as *const std::ffi::c_void,
        &mut pattern,
    );
    if hr >= 0 && !pattern.is_null() { Some(pattern) } else { None }
}

// ── IUIAutomationTextPattern helpers ────────────────────────────────
// vtable: 0=QI, 1=AddRef, 2=Release, 3=RangeFromPoint, 4=RangeFromChild,
// 5=GetSelection, 6=get_DocumentRange, 7=GetVisibleRanges, ...

/// TextPattern::GetSelection → SAFEARRAY of text ranges
unsafe fn text_pattern_get_selection(
    tp: *const std::ffi::c_void,
) -> Option<*mut std::ffi::c_void> {
    let func: unsafe extern "system" fn(
        *const std::ffi::c_void, *mut *mut std::ffi::c_void,
    ) -> i32 = std::mem::transmute(vtbl_entry(tp, 5));

    let mut psa = std::ptr::null_mut();
    let hr = func(tp, &mut psa);
    if hr >= 0 && !psa.is_null() { Some(psa) } else { None }
}

// ── IUIAutomationTextRange helpers ─────────────────────────────────
// vtable: 0=QI, 1=AddRef, 2=Release, 3=Clone, 4=Compare, 5=CompareEndpoints,
// 6=ExpandToEnclosingUnit, 7=FindAttribute, 8=FindText, 9=GetAttributeValue,
// 10=GetBoundingRectangles, 11=GetEnclosingElement, 12=GetText, ...

/// TextRange::GetText(maxLength) → String
unsafe fn text_range_get_text(range: *const std::ffi::c_void) -> Option<String> {
    // GetText signature: HRESULT GetText(int maxLength, BSTR* pRetVal)
    let func: unsafe extern "system" fn(
        *const std::ffi::c_void, i32, *mut *mut u16,
    ) -> i32 = std::mem::transmute(vtbl_entry(range, 12));

    let mut bstr = std::ptr::null_mut();
    let hr = func(range, 4096, &mut bstr);
    if hr >= 0 && !bstr.is_null() {
        let len = (0..).take_while(|&i| *bstr.add(i) != 0).count();
        let s = String::from_utf16_lossy(std::slice::from_raw_parts(bstr, len));
        SysFreeString(bstr);
        Some(s)
    } else {
        None
    }
}

/// TextRange::GetBoundingRectangles → (left, top, right, bottom)
unsafe fn text_range_get_bounding_rect(
    range: *const std::ffi::c_void,
) -> Option<(i32, i32, i32, i32)> {
    // GetBoundingRectangles signature: HRESULT GetBoundingRectangles(SAFEARRAY** pRetVal)
    let func: unsafe extern "system" fn(
        *const std::ffi::c_void, *mut *mut std::ffi::c_void,
    ) -> i32 = std::mem::transmute(vtbl_entry(range, 10));

    let mut psa = std::ptr::null_mut();
    let hr = func(range, &mut psa);
    if hr != S_OK || psa.is_null() {
        return None;
    }

    // SAFEARRAY contains doubles: [left, top, width, height, ...]
    let dims = SafeArrayGetDim(psa);
    if dims < 1 { return None; }

    let mut ubound: i32 = -1;
    if SafeArrayGetUBound(psa, 1, &mut ubound) != S_OK { return None; }

    let count = (ubound + 1) as usize;
    if count < 4 { return None; }

    let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
    if SafeArrayAccessData(psa, &mut data) != S_OK { return None; }

    let vals = std::slice::from_raw_parts(data as *const f64, 4);
    let result = (
        vals[0] as i32,
        vals[1] as i32,
        (vals[0] + vals[2]) as i32,
        (vals[1] + vals[3]) as i32,
    );

    SafeArrayUnaccessData(psa);
    SafeArrayDestroy(psa);
    Some(result)
}

// ── IUnknown::Release helper ────────────────────────────────────────

unsafe fn com_release(obj: *const std::ffi::c_void) {
    if obj.is_null() { return; }
    let func: unsafe extern "system" fn(*const std::ffi::c_void) -> u32 =
        std::mem::transmute(vtbl_entry(obj, 2));
    func(obj);
}

// ====================================================================
// Event flag + global handle
// ====================================================================

static SELECTION_EVENT: AtomicBool = AtomicBool::new(false);
static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

// ====================================================================
// WinEvent hook callback
// ====================================================================

unsafe extern "system" fn win_event_proc(
    _hhook: *const std::ffi::c_void,
    _event: u32,
    _hwnd: *const std::ffi::c_void,
    _id_object: i32,
    _id_child: i32,
    _dw_event_thread: u32,
    _dwms_event_time: u32,
) {
    SELECTION_EVENT.store(true, Ordering::Release);
}

// ====================================================================
// Selection query logic
// ====================================================================

struct SelectionInfo {
    text: String,
    cx: i32,
    cy: i32,
    rect_left: i32,
    rect_top: i32,
    rect_right: i32,
    rect_bottom: i32,
}

/// Query UIA for currently selected text and screen position.
/// Returns None if no text is selected or if UIA is unavailable.
unsafe fn query_uia_selection() -> Option<SelectionInfo> {
    // 1. Create UIA instance
    let mut uia: *mut std::ffi::c_void = std::ptr::null_mut();
    let hr = CoCreateInstance(
        CLSID_CUIAUTOMATION.as_ptr() as *const std::ffi::c_void,
        std::ptr::null(),
        CLSCTX_INPROC_SERVER,
        IID_IUIAUTOMATION.as_ptr() as *const std::ffi::c_void,
        &mut uia,
    );
    if hr != S_OK || uia.is_null() {
        log::debug!("CoCreateInstance IUIAutomation failed: HR=0x{:08X}", hr);
        return None;
    }

    // 2. Get focused element
    let element = match uia_get_focused_element(uia) {
        Some(e) => e,
        None => { com_release(uia); return None; }
    };

    // 3. QI for TextPattern
    let tp = match element_query_text_pattern(element) {
        Some(p) => p,
        None => { com_release(element); com_release(uia); return None; }
    };

    // 4. Get selection
    let selection_psa = match text_pattern_get_selection(tp) {
        Some(a) => a,
        None => { com_release(tp); com_release(element); com_release(uia); return None; }
    };

    // 5. Extract ranges from SAFEARRAY
    let mut ubound: i32 = -1;
    if SafeArrayGetUBound(selection_psa, 1, &mut ubound) != S_OK || ubound < 0 {
        SafeArrayDestroy(selection_psa);
        com_release(tp); com_release(element); com_release(uia);
        return None;
    }

    let count = (ubound + 1) as usize;
    if count == 0 {
        SafeArrayDestroy(selection_psa);
        com_release(tp); com_release(element); com_release(uia);
        return None;
    }

    let mut data: *mut std::ffi::c_void = std::ptr::null_mut();
    if SafeArrayAccessData(selection_psa, &mut data) != S_OK {
        SafeArrayDestroy(selection_psa);
        com_release(tp); com_release(element); com_release(uia);
        return None;
    }

    let range_ptrs = std::slice::from_raw_parts(
        data as *const *mut std::ffi::c_void, count,
    );

    // Find first non-null range pointer
    let range = match range_ptrs.iter().copied().find(|p| !p.is_null()) {
        Some(r) => r,
        None => {
            SafeArrayUnaccessData(selection_psa);
            SafeArrayDestroy(selection_psa);
            com_release(tp); com_release(element); com_release(uia);
            return None;
        }
    };

    // 6. Read text
    let text = text_range_get_text(range).unwrap_or_default();
    if text.trim().is_empty() {
        // Degenerate (caret only, no selection)
        SafeArrayUnaccessData(selection_psa);
        SafeArrayDestroy(selection_psa);
        com_release(tp); com_release(element); com_release(uia);
        return None;
    }

    // 7. Get bounding rectangle
    let (rl, rt, rr, rb) = text_range_get_bounding_rect(range).unwrap_or((0, 0, 0, 0));

    SafeArrayUnaccessData(selection_psa);
    SafeArrayDestroy(selection_psa);

    // 8. Get cursor position
    let mut cursor_pos: [i32; 2] = [0, 0];
    GetCursorPos(cursor_pos.as_mut_ptr() as *mut std::ffi::c_void);

    com_release(tp);
    com_release(element);
    com_release(uia);

    Some(SelectionInfo {
        text,
        cx: cursor_pos[0],
        cy: cursor_pos[1],
        rect_left: rl,
        rect_top: rt,
        rect_right: rr,
        rect_bottom: rb,
    })
}

fn process_selection() {
    let handle = match APP_HANDLE.get() {
        Some(h) => h.clone(),
        None => return,
    };

    match unsafe { query_uia_selection() } {
        Some(sel) if sel.text.len() >= 2 => {
            // Only emit for meaningful selections (2+ chars)
            let _ = handle.emit("selection-detected", serde_json::json!({
                "text": sel.text,
                "cx": sel.cx,
                "cy": sel.cy,
                "rect_left": sel.rect_left,
                "rect_top": sel.rect_top,
                "rect_right": sel.rect_right,
                "rect_bottom": sel.rect_bottom,
            }));
        }
        _ => {
            // No selection or too short, hide overlay
            let _ = handle.emit("selection-cleared", ());
        }
    }
}

// ====================================================================
// Public entry point
// ====================================================================

/// Start the UIA-based selection detection on a dedicated background thread.
///
/// The thread runs a Win32 message pump required by SetWinEventHook.
/// When text selection is detected, emits "selection-detected" Tauri event
/// with { text, cx, cy, rect_left, rect_top, rect_right, rect_bottom }.
pub fn start_uia_hook(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);

    thread::spawn(move || {
        // Initialize COM (apartment-threaded for UI automation)
        let com_hr = unsafe {
            CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED)
        };
        if com_hr < 0 && com_hr != -2147221007i32 /* RPC_E_CHANGED_MODE */ {
            log::error!("CoInitializeEx failed: HR=0x{:08X}", com_hr);
            return;
        }

        // Register WinEvent hook
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_SELECTION,
                EVENT_OBJECT_SELECTION,
                std::ptr::null(),
                win_event_proc,
                0, 0, // all processes, all threads
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };

        if hook.is_null() {
            log::error!("SetWinEventHook failed");
            unsafe { CoUninitialize() };
            return;
        }

        log::info!("UIA selection hook started");

        // Message pump + processing loop
        let mut last_process = Instant::now();
        const DEBOUNCE_MS: u64 = 300;

        unsafe {
            let mut msg: [isize; 6] = std::mem::zeroed();

            loop {
                let ret = GetMessageW(
                    msg.as_mut_ptr() as *mut std::ffi::c_void,
                    std::ptr::null(), 0, 0,
                );

                if ret == 0 { break; }         // WM_QUIT
                if ret == -1 {                  // Error
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }

                // Dispatch -> hook callbacks fire here
                TranslateMessage(msg.as_ptr() as *const std::ffi::c_void);
                DispatchMessageW(msg.as_ptr() as *const std::ffi::c_void);

                // Check if selection event was fired
                if SELECTION_EVENT.swap(false, Ordering::Acquire) {
                    let now = Instant::now();
                    if now.duration_since(last_process) >= Duration::from_millis(DEBOUNCE_MS) {
                        last_process = now;
                        process_selection();
                    }
                }
            }
        }

        // Cleanup
        if !hook.is_null() {
            unsafe { UnhookWinEvent(hook); }
        }
        unsafe { CoUninitialize(); }
        log::info!("UIA selection hook stopped");
    });
}
