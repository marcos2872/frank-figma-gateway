use crate::secrets::{ensure_secret, MASTER_KEY, SALT_KEY};
use crate::state::AppState;
use ffg_core::error::UiError;
use ffg_core::secrets::{SecretError, SecretKey};
use ffg_supervisor::{
    allocate_port, HealthCheck, HealthState, LogLine, ServiceName, ServiceSpec, ServiceState,
    ServiceStatus,
};
use std::collections::BTreeMap;
use std::time::Duration;
use tauri::State;

#[tauri::command]
pub async fn service_status(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    let mut statuses = state.supervisor.status_all().await;
    let proxy = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .as_ref()
        .map(|handle| handle.status());
    if let Some(proxy) = proxy {
        for status in &mut statuses {
            if status.name == ServiceName::Proxy {
                status.state = if proxy.running {
                    ServiceState::Running
                } else {
                    ServiceState::Stopped
                };
                status.port = Some(proxy.port);
                status.health = if proxy.running {
                    HealthState::Ok
                } else {
                    HealthState::Unknown
                };
                status.detail = proxy.last_error.clone();
            }
        }
    }
    Ok(statuses)
}

#[tauri::command]
pub async fn start_services(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    start_services_inner(&state).await?;
    // Com a integração ativa, o proxy também precisa voltar a subir.
    if integration_is_active(&state) {
        if let Err(error) = crate::commands::proxy::ensure_proxy(&state).await {
            log::warn!("proxy não subiu ao iniciar serviços: {}", error.message);
        }
    }
    service_status(state).await
}

/// Sobe os sidecars registrados. Usado pelo comando e pela restauração da
/// integração no startup, que precisa deixar o LiteLLM pronto antes de o
/// Figma voltar a falar com o proxy.
pub(crate) async fn start_services_inner(state: &AppState) -> Result<(), UiError> {
    register_services(state).await?;
    state
        .supervisor
        .start_all()
        .await
        .map_err(|error| error.to_ui_error())?;
    Ok(())
}

fn integration_is_active(state: &AppState) -> bool {
    let path = state.paths.integration_file();
    let Ok(raw) = std::fs::read(&path) else {
        return false;
    };
    serde_json::from_slice::<ffg_core::integration::IntegrationJournal>(&raw)
        .map(|journal| journal.status == ffg_core::integration::IntegrationStatus::Active)
        .unwrap_or(false)
}

#[tauri::command]
pub async fn stop_services(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    state
        .supervisor
        .stop_all()
        .await
        .map_err(|error| error.to_ui_error())?;
    service_status(state).await
}

#[tauri::command]
pub async fn restart_services(state: State<'_, AppState>) -> Result<Vec<ServiceStatus>, UiError> {
    let _ = state.supervisor.stop_all().await;
    register_services(&state).await?;
    state
        .supervisor
        .start_all()
        .await
        .map_err(|error| error.to_ui_error())?;
    service_status(state).await
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
            let copilot_token_dir = state.paths.root().join("copilot").join("github-copilot");
            if copilot_token_dir.join("access-token").is_file() {
                env.insert(
                    "GITHUB_COPILOT_TOKEN_DIR".to_owned(),
                    copilot_token_dir.display().to_string(),
                );
            }
            // Chaves de provider: o config referencia `os.environ/<VAR>`.
            if let Some(secret_ref) = setup
                .provider
                .as_ref()
                .and_then(|provider| provider.secret_ref.as_deref())
            {
                if let Some(secret) = state
                    .secrets
                    .get(&SecretKey::new(secret_ref))
                    .map_err(secret_error)?
                {
                    for route in &setup.models.routes {
                        if let Some(name) = &route.api_key_env {
                            env.insert(name.clone(), secret.expose().to_owned());
                        }
                    }
                }
            }
            // Sem `DATABASE_URL`: o runtime empacotado não inclui o Prisma, e o
            // Copilot autentica pelo token do device flow, não pelo banco.
            state
                .supervisor
                .register(ServiceSpec {
                    name: ServiceName::Litellm,
                    program,
                    args: vec![
                        "--config".to_owned(),
                        config.to_string_lossy().into_owned(),
                        "--host".to_owned(),
                        "127.0.0.1".to_owned(),
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
