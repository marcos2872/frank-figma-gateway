mod ca;
mod commands;
mod events;
mod figma;
mod logging;
mod paths;
mod platform;
mod secrets;
mod setup_store;
mod state;
mod tray;

use tauri::{Emitter, Manager, WindowEvent};

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
            commands::app::open_url,
            commands::services::service_status,
            commands::services::start_services,
            commands::services::stop_services,
            commands::services::restart_services,
            commands::services::service_logs,
            commands::integration::install_figma_integration,
            commands::integration::remove_figma_integration,
            commands::integration::integration_status,
            commands::integration::figma_environment,
            commands::integration::figma_install,
            commands::integration::figma_remove,
            commands::integration::figma_validate,
            commands::integration::figma_restart,
            commands::copilot::copilot_status,
            commands::copilot::copilot_prepare,
            commands::copilot::copilot_disable,
            commands::copilot::copilot_disconnect,
            commands::copilot::copilot_begin_auth,
            commands::copilot::copilot_poll_auth,
            commands::diagnostics::environment_report,
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
            commands::ca::ca_status,
            commands::ca::ca_install,
            commands::ca::ca_remove,
            commands::ca::ca_export,
            commands::proxy::proxy_status,
            commands::proxy::proxy_start,
            commands::proxy::proxy_stop,
            commands::proxy::proxy_self_test,
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

            // Se a integração com o Figma estava ativa, o proxy precisa voltar a
            // subir junto com o app (o env já aponta para ele).
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<state::AppState>();
                restore_integration(&state).await;
            });

            // A UI escuta `service://status`; sem emissão ela ficaria presa em
            // "Iniciando" mesmo depois de o health check passar.
            let events_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut last: std::collections::HashMap<String, String> =
                    std::collections::HashMap::new();
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    let state = events_handle.state::<state::AppState>();
                    let Ok(statuses) = commands::services::service_status(state).await else {
                        continue;
                    };
                    for status in statuses {
                        let key = format!("{:?}", status.name);
                        let signature =
                            format!("{:?}|{:?}|{:?}", status.state, status.health, status.detail);
                        if last
                            .get(&key)
                            .is_some_and(|previous| previous == &signature)
                        {
                            continue;
                        }
                        last.insert(key, signature);
                        if let Err(error) = events_handle.emit(events::SERVICE_STATUS, &status) {
                            log::warn!("falha ao emitir {}: {error}", events::SERVICE_STATUS);
                        }
                    }
                }
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
        .build(tauri::generate_context!())
        .expect("failed to build the Tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<state::AppState>();
                commands::integration::restore_system_proxy_on_exit(state.inner());
            }
        });
}

/// Restaura o proxy quando o journal da integração está `active`.
///
/// O env em `HKCU\Environment` continua apontando para o proxy depois de um
/// restart; sem subir o proxy, todo o tráfego do Figma falharia.
async fn restore_integration(state: &state::AppState) {
    let path = state.paths.integration_file();
    let Ok(raw) = std::fs::read(&path) else {
        return;
    };
    let Ok(journal) = serde_json::from_slice::<ffg_core::integration::IntegrationJournal>(&raw)
    else {
        log::warn!("journal da integração inválido; restauração ignorada");
        return;
    };
    if journal.status != ffg_core::integration::IntegrationStatus::Active {
        return;
    }

    // O proxy sozinho responde 502 sem o LiteLLM: a integração só volta a
    // funcionar quando os sidecars também sobem.
    if let Err(error) = commands::services::start_services_inner(state).await {
        log::warn!(
            "integração ativa, mas os serviços não subiram: {}",
            error.message
        );
    }

    match commands::proxy::ensure_proxy(state).await {
        Ok(status) => {
            log::info!(
                "integração ativa: proxy restaurado na porta {}",
                status.port
            );
            if let Err(error) = commands::integration::ensure_system_proxy(state, status.port) {
                log::warn!(
                    "integração ativa, mas o proxy do sistema não foi aplicado: {}",
                    error.message
                );
            }
        }
        Err(error) => log::warn!("integração ativa, mas o proxy não subiu: {}", error.message),
    }
}
