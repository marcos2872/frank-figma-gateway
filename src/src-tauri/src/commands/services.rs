use crate::secrets::{ensure_secret, MASTER_KEY, SALT_KEY};
use crate::state::AppState;
use ffg_core::error::UiError;
use ffg_core::secrets::SecretError;
use ffg_supervisor::{
    allocate_port, HealthCheck, LogLine, ServiceName, ServiceSpec, ServiceStatus,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub async fn service_status(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    Ok(state.supervisor.status_all().await)
}

#[tauri::command]
pub async fn start_services(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    register_services(&state).await?;
    state
        .supervisor
        .start_all()
        .await
        .map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub async fn stop_services(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    state
        .supervisor
        .stop_all()
        .await
        .map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub async fn restart_services(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    let _ = state.supervisor.stop_all().await;
    register_services(&state).await?;
    state
        .supervisor
        .start_all()
        .await
        .map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub async fn service_logs(
    service: ServiceName,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<LogLine>, UiError> {
    state
        .supervisor
        .logs(service, limit.unwrap_or(200))
        .await
        .map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub fn install_figma_integration() -> SetupResult {
    SetupResult {
        ok: false,
        message: "TODO: instalar a CA local e configurar o proxy HTTPS.".to_owned(),
    }
}

#[tauri::command]
pub fn remove_figma_integration() -> SetupResult {
    SetupResult {
        ok: false,
        message: "TODO: remover a CA local e restaurar as variáveis de ambiente.".to_owned(),
    }
}

async fn register_services(state: &AppState) -> Result<(), UiError> {
    let active = state
        .runtime
        .active()
        .map_err(|error| error.to_ui_error())?;

    let mut setup = {
        let store = state
            .setup
            .lock()
            .map_err(|_| UiError::internal("estado do app indisponível"))?;
        store.load().map_err(UiError::from)?.state
    };

    let mut changed = false;
    let litellm_port = match setup.services.litellm {
        Some(port) => port,
        None => {
            let port = allocate_port()
                .map_err(|error| UiError::internal(format!("porta indisponível: {error}")))?;
            setup.services.litellm = Some(port);
            changed = true;
            port
        }
    };
    let postgres_port = match setup.services.postgres {
        Some(port) => port,
        None => {
            let port = allocate_port()
                .map_err(|error| UiError::internal(format!("porta indisponível: {error}")))?;
            setup.services.postgres = Some(port);
            changed = true;
            port
        }
    };

    if changed {
        let store = state
            .setup
            .lock()
            .map_err(|_| UiError::internal("estado do app indisponível"))?;
        let mut setup = setup;
        store.save(&mut setup).map_err(UiError::from)?;
    }

    if let Some(litellm) = &active.litellm {
        let cwd = state.paths.root().join(&litellm.path);
        let program = cwd.join("litellm.exe");
        let config = state.paths.litellm_config_file();
        if program.is_file() && config.is_file() {
            let master = ensure_secret(state.secrets.as_ref(), MASTER_KEY).map_err(secret_error)?;
            let salt = ensure_secret(state.secrets.as_ref(), SALT_KEY).map_err(secret_error)?;
            let mut env = BTreeMap::new();
            env.insert("LITELLM_MASTER_KEY".to_owned(), master.expose().to_owned());
            env.insert("LITELLM_SALT_KEY".to_owned(), salt.expose().to_owned());
            state
                .supervisor
                .register(ServiceSpec {
                    name: ServiceName::Litellm,
                    program,
                    args: vec![
                        "--config".to_owned(),
                        config.to_string_lossy().into_owned(),
                        "--port".to_owned(),
                        litellm_port.to_string(),
                    ],
                    env,
                    cwd: Some(cwd),
                    port: Some(litellm_port),
                    health: HealthCheck::Http {
                        url: format!("http://127.0.0.1:{litellm_port}/health/readiness"),
                        timeout: Duration::from_secs(5),
                    },
                })
                .await;
        }
    }

    if let Some(pgsql) = &active.pgsql {
        let cwd = state.paths.root().join(&pgsql.path);
        let program = cwd.join("bin").join("postgres.exe");
        let data = state.paths.postgres_data_dir();
        if program.is_file() && data.is_dir() {
            state
                .supervisor
                .register(ServiceSpec {
                    name: ServiceName::Postgres,
                    program,
                    args: vec![
                        "-D".to_owned(),
                        data.to_string_lossy().into_owned(),
                        "-p".to_owned(),
                        postgres_port.to_string(),
                        "-c".to_owned(),
                        "listen_addresses=127.0.0.1".to_owned(),
                    ],
                    env: BTreeMap::new(),
                    cwd: Some(cwd),
                    port: Some(postgres_port),
                    health: HealthCheck::Tcp {
                        port: postgres_port,
                        timeout: Duration::from_secs(2),
                    },
                })
                .await;
        }
    }

    Ok(())
}

fn secret_error(error: SecretError) -> UiError {
    UiError::internal("Não foi possível acessar o cofre de credenciais.")
        .with_detail(error.to_string())
}
