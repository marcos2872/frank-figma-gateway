mod commands;
mod events;
mod logging;
mod paths;
mod secrets;
mod setup_store;
mod state;
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
            commands::app::app_info,
            commands::app::preview_rewrite,
            commands::services::service_status,
            commands::services::start_services,
            commands::services::stop_services,
            commands::services::restart_services,
            commands::services::service_logs,
            commands::services::install_figma_integration,
            commands::services::remove_figma_integration,
            commands::setup::setup_get_state,
            commands::setup::setup_begin_step,
            commands::setup::setup_complete_step,
            commands::setup::setup_set_options,
            commands::setup::setup_reset,
            commands::runtime::runtime_status,
            commands::runtime::runtime_fetch_manifest,
            commands::runtime::runtime_install,
            commands::runtime::runtime_cancel,
            commands::providers::providers_catalog,
            commands::providers::models_get,
            commands::providers::models_apply,
            commands::providers::provider_save,
            commands::providers::provider_remove,
            commands::providers::provider_secret_status,
            commands::providers::provider_test,
            commands::providers::litellm_health,
        ])
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()?;
            let paths = paths::RuntimePaths::new(data_dir);
            paths.ensure()?;
            app.manage(state::AppState::new(paths)?);

            let supervisor = app.state::<state::AppState>().supervisor.clone();
            tauri::async_runtime::spawn(async move {
                supervisor
                    .monitor(tokio_util::sync::CancellationToken::new())
                    .await;
            });

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
