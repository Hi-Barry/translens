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
                app.emit("translation-chunk", &content)
                    .map_err(|e| format!("发送事件失败: {}", e))?;
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

/// Show the translation popup window
#[tauri::command]
pub async fn show_translator_window(
    app: AppHandle,
    text: String,
    x: i32,
    y: i32,
) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("translator") {
        window
            .set_position(tauri::PhysicalPosition::new(x, y))
            .map_err(|e| e.to_string())?;
        window.show().map_err(|e| e.to_string())?;
        window
            .set_focus()
            .map_err(|e| e.to_string())?;
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

/// Get current config (for settings UI)
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    state.config.lock().map(|c| c.clone()).map_err(|e| e.to_string())
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
