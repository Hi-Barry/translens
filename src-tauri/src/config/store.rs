use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_window_pos() -> i32 { -1 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub target_language: String,
    pub auto_detect_source: bool,
    pub close_on_esc: bool,
    pub close_on_lose_focus: bool,

    pub deepseek_api_key: String,
    pub deepseek_base_url: String,
    pub deepseek_model: String,
    pub deepseek_temperature: f32,

    pub theme: String,
    pub window_opacity: f64,
    pub font_size: u32,

    // Saved window position (-1 = not yet saved, use center)
    #[serde(default = "default_window_pos")]
    pub window_x: i32,
    #[serde(default = "default_window_pos")]
    pub window_y: i32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            target_language: "zh-CN".to_string(),
            auto_detect_source: true,
            close_on_esc: true,
            close_on_lose_focus: false,

            deepseek_api_key: String::new(),
            deepseek_base_url: "https://api.deepseek.com/v1".to_string(),
            deepseek_model: "deepseek-chat".to_string(),
            deepseek_temperature: 0.1,

            theme: "system".to_string(),
            window_opacity: 0.95,
            font_size: 14,

            window_x: -1,
            window_y: -1,
        }
    }
}

impl AppConfig {
    fn config_dir() -> PathBuf {
        let base = if cfg!(target_os = "windows") {
            std::env::var("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("."))
        } else {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".config"))
                .unwrap_or_else(|_| PathBuf::from("."))
        };
        base.join("translens")
    }

    pub fn load() -> Self {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("config.json");

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    serde_json::from_str(&content).unwrap_or_default()
                }
                Err(_) => Self::default(),
            }
        } else {
            let config = Self::default();
            config.save();
            config
        }
    }

    pub fn save(&self) {
        let dir = Self::config_dir();
        std::fs::create_dir_all(&dir).ok();
        let path = dir.join("config.json");
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(&path, content);
        }
    }
}
