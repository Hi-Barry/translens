mod commands;
mod config;
mod translator;

use config::AppConfig;
use std::sync::Mutex;
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    image::Image,
};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut, ShortcutState};

/// Shared application state
pub struct AppState {
    pub config: Mutex<AppConfig>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // Initialize config
            let config = AppConfig::load();
            app.manage(AppState {
                config: Mutex::new(config),
            });

            // Setup system tray
            setup_tray(app.handle())?;

            // Register global hotkey: Alt+Shift+T
            register_hotkey(app.handle())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::translate_text,
            commands::show_translator_window,
            commands::hide_translator_window,
            commands::open_settings_window,
            commands::get_config,
            commands::save_config,
            commands::save_window_position,
        ])
        .run(tauri::generate_context!())
        .expect("error while running translens");
}

fn register_hotkey(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let shortcut = Shortcut::new(Some(Modifiers::ALT | Modifiers::SHIFT), Code::KeyT);

    app.global_shortcut().on_shortcut(shortcut, move |handle, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            log::info!("Hotkey Alt+Shift+T pressed — triggering translation");

            // Read clipboard text
            let text = handle
                .clipboard()
                .read_text()
                .ok()
                .unwrap_or_default();

            let text = text.trim().to_string();

            if text.is_empty() {
                log::debug!("Clipboard is empty or not text — ignoring hotkey");
                return;
            }

            // Show translator window and send text
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let _ = commands::show_translator_window_inner(handle, text, 0, 0).await;
            });
        }
    })?;

    log::info!("Global hotkey Alt+Shift+T registered");
    Ok(())
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
                    // Read clipboard and translate
                    tauri::async_runtime::spawn(async move {
                        let text = handle
                            .clipboard()
                            .read_text()
                            .ok()
                            .unwrap_or_default();
                        let text = text.trim().to_string();
                        if text.is_empty() {
                            return;
                        }
                        let _ = commands::show_translator_window_inner(handle, text, 0, 0).await;
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
                    let text = handle
                        .clipboard()
                        .read_text()
                        .ok()
                        .unwrap_or_default();
                    let text = text.trim().to_string();
                    if text.is_empty() {
                        return;
                    }
                    let _ = commands::show_translator_window_inner(handle, text, 0, 0).await;
                });
            }
        })
        .build(app)?;

    Ok(())
}
