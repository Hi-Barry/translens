use crate::config::AppConfig;
use crate::translator::deepseek::DeepSeekTranslator;
use crate::AppState;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Serialize, Deserialize)]
pub struct TranslateRequest {
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
}

/// Main IPC command: translate text via DeepSeek streaming API
#[tauri::command]
pub async fn translate_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    source_lang: String,
    target_lang: String,
) -> Result<(), String> {
    let (api_key, base_url, model, temperature) = {
        let config = state.config.lock().map_err(|e| e.to_string())?;
        (
            config.deepseek_api_key.clone(),
            config.deepseek_base_url.clone(),
            config.deepseek_model.clone(),
            config.deepseek_temperature,
        )
    };

    if api_key.is_empty() {
        return Err("请在设置中配置 DeepSeek API Key".to_string());
    }

    let translator = DeepSeekTranslator::new(&api_key, &base_url, &model, temperature);

    let mut stream = translator
        .translate(&text, &source_lang, &target_lang)
        .await
        .map_err(|e| format!("翻译请求失败: {}", e))?;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(content) => {
                // Skip empty chunks (SSE heartbeats, keepalive)
                if !content.is_empty() {
                    app.emit("translation-chunk", &content)
                        .map_err(|e| format!("发送事件失败: {}", e))?;
                }
            }
            Err(e) => {
                return Err(format!("翻译流错误: {}", e));
            }
        }
    }

    app.emit("translation-done", ())
        .map_err(|e| format!("发送完成事件失败: {}", e))?;

    Ok(())
}

/// Capture selected text from clipboard + open translator window
#[tauri::command]
pub async fn capture_and_translate(app: AppHandle) -> Result<(), String> {
    // Try to capture selected text (Windows only, returns None on Linux)
    let text = crate::detection::capture_selected_text(&app);

    let text = match text {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            // If no text captured via Ctrl+C, try direct clipboard read
            #[cfg(target_os = "windows")]
            {
                let clip_text = crate::detection::read_clipboard_text(&app).unwrap_or_default();
                if !clip_text.trim().is_empty() {
                    clip_text
                } else {
                    return Err("未检测到选中文本，请先选择文本后重试".to_string());
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                return Err("文本捕获仅在 Windows 平台可用".to_string());
            }
        }
    };

    // Show the translator window
    show_translator_window_inner(app, text, 0, 0).await
}

/// Show the translation popup window
#[tauri::command]
pub async fn show_translator_window(
    app: AppHandle,
    text: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    show_translator_window_inner(app, text, x, y).await
}

async fn show_translator_window_inner(
    app: AppHandle,
    text: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("translator") {
        // Determine position priority: explicit > saved > center
        if x != 0 || y != 0 {
            window
                .set_position(tauri::PhysicalPosition::new(x, y))
                .map_err(|e| e.to_string())?;
        } else {
            // Check if we have a saved position from a previous session
            let saved_pos = app.state::<AppState>().config.lock().ok().map(|c| (c.window_x, c.window_y));
            match saved_pos {
                Some((sx, sy)) if sx >= 0 && sy >= 0 => {
                    window
                        .set_position(tauri::PhysicalPosition::new(sx, sy))
                        .map_err(|e| e.to_string())?;
                }
                _ => {
                    let _ = window.center();
                }
            }
        }
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
        app.emit("translate-text", &text)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Hide the translation popup window
#[tauri::command]
pub async fn hide_translator_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("translator") {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Open the settings window
#[tauri::command]
pub async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("settings") {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        // Create settings window on demand
        use tauri::WebviewWindowBuilder;
        let _win = WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .title("TransLens 设置")
        .inner_size(480.0, 540.0)
        .resizable(false)
        .center()
        .visible(true)
        .build()
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Get current config (for settings UI)
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.config.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

/// Save window position (called from frontend after drag)
#[tauri::command]
pub fn save_window_position(
    state: State<'_, AppState>,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|e| e.to_string())?;
    config.window_x = x;
    config.window_y = y;
    config.save();
    Ok(())
}

/// Save config
#[tauri::command]
pub fn save_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<(), String> {
    config.save();
    let mut current = state.config.lock().map_err(|e| e.to_string())?;
    *current = config;
    Ok(())
}
