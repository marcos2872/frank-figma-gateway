mod commands;
mod logging;
mod supervisor;
mod tray;

use tauri::{Manager, WindowEvent};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(logging::plugin())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::preview_rewrite,
            commands::service_status,
            commands::start_services,
            commands::stop_services,
            commands::install_figma_integration,
            commands::remove_figma_integration,
        ])
        .setup(|app| {
            tray::build(app)?;
            if !std::env::args().any(|arg| arg == "--hidden") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run the Tauri application");
}
