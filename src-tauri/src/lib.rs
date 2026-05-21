mod commands;
mod config;
mod detection;
mod overlay;
mod translator;

use std::sync::Mutex;
use config::AppConfig;
use tauri::{
    AppHandle, Emitter, Manager,
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

            // Create overlay handler
            #[cfg(target_os = "windows")]
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    detection::start_detection(handle);
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate_text,
            commands::show_translator_window,
            commands::hide_translator_window,
            commands::get_config,
            commands::save_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running translens");
}

fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Create tray menu
    let translate = MenuItem::with_id(app, "translate", "翻译选中文本", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&translate, &settings, &separator, &quit])?;

    // Build tray icon
    // Use embedded icon or default Tauri icon
    let icon = Image::from_bytes(include_bytes!("../icons/icon.png"))
        .unwrap_or_else(|_| Image::new(&[0; 32], 32, 32));

    let _tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("TransLens - AI 翻译")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "translate" => {
                // Trigger translation of current clipboard
                let _ = app.emit("translate-clipboard", ());
            }
            "settings" => {
                let _ = app.emit("show-settings", ());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::TrayIconEvent;
            if let TrayIconEvent::DoubleClick { .. } = event {
                let app = tray.app_handle();
                let _ = app.emit("translate-clipboard", ());
            }
        })
        .build(app)?;

    Ok(())
}
