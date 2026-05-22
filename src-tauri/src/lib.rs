mod commands;
mod config;
mod detection;
mod overlay;
mod translator;

use std::sync::Mutex;
use config::AppConfig;
use tauri::{
    AppHandle, Emitter, Listener, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    image::Image,
};

/// Shared application state
pub struct AppState {
    pub config: Mutex<AppConfig>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            // Initialize config
            let config = AppConfig::load();
            app.manage(AppState {
                config: Mutex::new(config),
            });

            // Setup system tray
            setup_tray(app.handle())?;

            // Start detection thread (Windows only)
            #[cfg(target_os = "windows")]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    detection::start_detection(handle);
                });
            }

            // Listen for selection events from UIA hook → show/hide overlay button
            let handle = app.handle().clone();
            app.listen("selection-detected", move |event| {
                use crate::overlay::show_button;
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    if let (Some(text), Some(cx), Some(cy)) = (
                        data["text"].as_str(),
                        data["cx"].as_i64(),
                        data["cy"].as_i64(),
                    ) {
                        // Button appears to the right and slightly above cursor
                        let btn_x = cx as i32 + 12;
                        let btn_y = cy as i32 - 20;
                        show_button(handle.clone(), btn_x, btn_y, text.to_string());
                    }
                }
            });

            let handle = app.handle().clone();
            app.listen("selection-cleared", move |_event| {
                crate::overlay::hide_button(&handle);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate_text,
            commands::capture_and_translate,
            commands::show_translator_window,
            commands::hide_translator_window,
            commands::open_settings_window,
            commands::get_config,
            commands::save_config,
            commands::save_window_position,
            commands::show_overlay_button,
            commands::hide_overlay_button,
            commands::overlay_click,
        ])
        .run(tauri::generate_context!())
        .expect("error while running translens");
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let translate = MenuItem::with_id(app, "translate", "翻译选中文本", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&translate, &settings, &separator, &quit])?;

    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
        .unwrap_or_else(|_| Image::new(&[0; 32], 32, 32));

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("TransLens - AI 翻译")
        .on_menu_event(move |app, event| {
            let handle = app.clone();
            match event.id().as_ref() {
                "translate" => {
                    // Direct translate (uses clipboard fallback)
                    tauri::async_runtime::spawn(async move {
                        let _ = commands::capture_and_translate(handle).await;
                    });
                }
                "settings" => {
                    tauri::async_runtime::spawn(async move {
                        let _ = commands::open_settings_window(handle).await;
                    });
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(move |tray, event| {
            use tauri::tray::TrayIconEvent;
            if let TrayIconEvent::DoubleClick { .. } = event {
                let handle = tray.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = commands::capture_and_translate(handle).await;
                });
            }
        })
        .build(app)?;

    Ok(())
}
