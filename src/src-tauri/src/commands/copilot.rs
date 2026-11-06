//! Postgres profile and GitHub Copilot state commands.

use crate::secrets::{ensure_secret, POSTGRES_PASSWORD};
use crate::state::AppState;
use ffg_core::error::{ErrorCode, UiError};
use ffg_core::secrets::SecretError;
use ffg_core::setup::{AuthStatus, CopilotState, SetupState};
use ffg_litellm::{DeviceFlow, PollOutcome};
use ffg_postgres::{self as pg, CommandSpec, PostgresLayout};
use ffg_supervisor::{HealthCheck, ServiceName, ServiceSpec, ServiceState};
use serde::Serialize;
use std::collections::BTreeMap;
use std::time::Duration;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PostgresStatus {
    pub initialized: bool,
    pub running: bool,
    pub port: Option<u16>,
    pub data_dir: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotStatus {
    pub copilot: CopilotState,
    pub postgres: PostgresStatus,
}

#[tauri::command]
pub async fn copilot_status(state: State<'_, AppState>) -> Result<CopilotStatus, UiError> {
    let setup = load_setup(&state)?;
    let postgres = postgres_status(&state, &setup).await?;
    Ok(CopilotStatus {
        copilot: setup.copilot,
        postgres,
    })
}

#[tauri::command]
pub async fn copilot_prepare(state: State<'_, AppState>) -> Result<CopilotStatus, UiError> {
    prepare_postgres(&state).await?;
    let mut setup = load_setup(&state)?;
    setup.copilot.enabled = true;
    setup.copilot.postgres_ready = true;
    setup.copilot.database_name = "litellm".to_owned();
    setup.copilot.credential_name = "github-copilot".to_owned();
    setup.copilot.auth_status = AuthStatus::NotStarted;
    save_setup(&state, &mut setup)?;
    let postgres = postgres_status(&state, &setup).await?;
    Ok(CopilotStatus {
        copilot: setup.copilot,
        postgres,
    })
}

#[tauri::command]
pub async fn copilot_disable(
    delete_data: Option<bool>,
    state: State<'_, AppState>,
) -> Result<CopilotStatus, UiError> {
    let _ = state.supervisor.stop(ServiceName::Postgres).await;
    if delete_data.unwrap_or(false) {
        let data = state.paths.postgres_data_dir();
        if data.exists() {
            std::fs::remove_dir_all(&data).map_err(|error| {
                UiError::internal("Não foi possível apagar os dados do Postgres.")
                    .with_detail(error.to_string())
            })?;
        }
    }
    let mut setup = load_setup(&state)?;
    setup.copilot = CopilotState::default();
    save_setup(&state, &mut setup)?;
    let postgres = postgres_status(&state, &setup).await?;
    Ok(CopilotStatus {
        copilot: setup.copilot,
        postgres,
    })
}

#[tauri::command]
pub async fn copilot_disconnect(state: State<'_, AppState>) -> Result<CopilotStatus, UiError> {
    let mut setup = load_setup(&state)?;
    setup.copilot.connected = false;
    setup.copilot.github_login = None;
    setup.copilot.auth_status = AuthStatus::NotStarted;
    save_setup(&state, &mut setup)?;
    let postgres = postgres_status(&state, &setup).await?;
    Ok(CopilotStatus {
        copilot: setup.copilot,
        postgres,
    })
}

#[tauri::command]
pub async fn copilot_begin_auth(state: State<'_, AppState>) -> Result<DeviceFlow, UiError> {
    let flow = match state.copilot.begin().await {
        Ok(flow) => flow,
        Err(error) => {
            log::warn!("copilot: device flow falhou: {error}");
            return Err(error.to_ui_error());
        }
    };
    log::info!(
        "copilot: device flow iniciado (expira em {}s)",
        flow.expires_in
    );
    let mut setup = load_setup(&state)?;
    setup.copilot.enabled = true;
    setup.copilot.auth_status = AuthStatus::WaitingForUser;
    save_setup(&state, &mut setup)?;
    Ok(flow)
}

#[tauri::command]
pub async fn copilot_poll_auth(
    device_code: String,
    state: State<'_, AppState>,
) -> Result<CopilotStatus, UiError> {
    let outcome = match state.copilot.poll(&device_code).await {
        Ok(outcome) => outcome,
        Err(error) => {
            log::warn!("copilot: poll falhou: {error}");
            return Err(error.to_ui_error());
        }
    };

    let mut setup = load_setup(&state)?;
    match outcome {
        PollOutcome::Pending | PollOutcome::SlowDown => {
            log::debug!("copilot: aguardando autorização");
            setup.copilot.auth_status = AuthStatus::WaitingForUser;
        }
        PollOutcome::Token(token) => {
            log::info!("copilot: token recebido; gravando access-token");
            store_access_token(&state, &token)?;
            let login = state.copilot.login(&token).await.unwrap_or(None);
            setup.copilot.connected = true;
            setup.copilot.auth_status = AuthStatus::Connected;
            setup.copilot.github_login = login;
            setup.copilot.last_error = None;
        }
    }
    save_setup(&state, &mut setup)?;

    let postgres = postgres_status(&state, &setup).await?;
    Ok(CopilotStatus {
        copilot: setup.copilot,
        postgres,
    })
}

/// Grava o token no diretório que o LiteLLM lê (`GITHUB_COPILOT_TOKEN_DIR`).
fn store_access_token(state: &AppState, token: &str) -> Result<(), UiError> {
    let dir = state.paths.root().join("copilot").join("github-copilot");
    std::fs::create_dir_all(&dir).map_err(io_error)?;
    std::fs::write(dir.join("access-token"), token).map_err(io_error)?;
    Ok(())
}

async fn prepare_postgres(state: &AppState) -> Result<(), UiError> {
    let active = state
        .runtime
        .active()
        .map_err(|error| error.to_ui_error())?;
    let pgsql = active
        .pgsql
        .ok_or_else(|| UiError::internal("Runtime do Postgres não instalado."))?;
    let runtime_dir = state.paths.root().join(&pgsql.path);
    let data_dir = state.paths.postgres_data_dir();
    let layout = PostgresLayout::from_runtime(&runtime_dir, &data_dir);

    if !layout.is_initialized() {
        let _ = state.supervisor.stop(ServiceName::Postgres).await;
        std::fs::create_dir_all(&data_dir).map_err(io_error)?;
        let password =
            ensure_secret(state.secrets.as_ref(), POSTGRES_PASSWORD).map_err(secret_error)?;
        let password_file = state.paths.state_dir().join("pg-password.tmp");
        std::fs::write(&password_file, password.expose()).map_err(io_error)?;
        let result = run_command(pg::initdb(&layout, &password_file)).await;
        let _ = std::fs::remove_file(&password_file);
        result?;

        let port = ensure_postgres_port(state)?;
        state
            .supervisor
            .register(ServiceSpec {
                name: ServiceName::Postgres,
                program: layout.bin_dir.join("postgres.exe"),
                args: pg::server(&layout, port).args,
                env: BTreeMap::new(),
                cwd: Some(runtime_dir),
                port: Some(port),
                health: HealthCheck::Tcp {
                    port,
                    timeout: Duration::from_secs(2),
                },
            })
            .await;
        state
            .supervisor
            .start(ServiceName::Postgres)
            .await
            .map_err(|error| error.to_ui_error())?;

        for _ in 0..40 {
            let status = state
                .supervisor
                .status(ServiceName::Postgres)
                .await
                .map_err(|error| error.to_ui_error())?;
            if status.state == ServiceState::Running {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        run_command(pg::create_database(&layout, port, password.expose())).await?;
    }

    let mut setup = load_setup(state)?;
    setup.copilot.postgres_ready = true;
    setup.services.postgres = Some(ensure_postgres_port(state)?);
    save_setup(state, &mut setup)?;
    Ok(())
}

async fn postgres_status(state: &AppState, setup: &SetupState) -> Result<PostgresStatus, UiError> {
    let data_dir = state.paths.postgres_data_dir();
    let initialized = data_dir.join("PG_VERSION").is_file();
    let running = state
        .supervisor
        .status(ServiceName::Postgres)
        .await
        .map(|status| status.state == ServiceState::Running)
        .unwrap_or(false);
    Ok(PostgresStatus {
        initialized,
        running,
        port: setup.services.postgres,
        data_dir: data_dir.display().to_string(),
    })
}

fn ensure_postgres_port(state: &AppState) -> Result<u16, UiError> {
    let mut setup = load_setup(state)?;
    if let Some(port) = setup.services.postgres {
        return Ok(port);
    }
    let port = ffg_supervisor::allocate_port()
        .map_err(|error| UiError::internal(format!("porta do Postgres: {error}")))?;
    setup.services.postgres = Some(port);
    save_setup(state, &mut setup)?;
    Ok(port)
}

async fn run_command(spec: CommandSpec) -> Result<(), UiError> {
    let mut command = tokio::process::Command::new(&spec.program);
    command.args(&spec.args).envs(&spec.env);
    let output = tokio::time::timeout(Duration::from_secs(180), command.output())
        .await
        .map_err(|_| UiError::new(ErrorCode::CopilotDatabase, "initdb/psql excedeu o tempo."))?
        .map_err(io_error)?;
    if !output.status.success() {
        return Err(
            UiError::new(ErrorCode::CopilotDatabase, "Comando do Postgres falhou.").with_detail(
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(400)
                    .collect::<String>(),
            ),
        );
    }
    Ok(())
}

fn load_setup(state: &AppState) -> Result<SetupState, UiError> {
    let store = state
        .setup
        .lock()
        .map_err(|_| UiError::internal("estado do app indisponível"))?;
    Ok(store.load().map_err(UiError::from)?.state)
}

fn save_setup(state: &AppState, setup: &mut SetupState) -> Result<(), UiError> {
    let store = state
        .setup
        .lock()
        .map_err(|_| UiError::internal("estado do app indisponível"))?;
    store.save(setup).map_err(UiError::from)
}

fn secret_error(error: SecretError) -> UiError {
    UiError::internal("Cofre de credenciais indisponível.").with_detail(error.to_string())
}

fn io_error(error: std::io::Error) -> UiError {
    UiError::internal("Falha de I/O no Postgres.").with_detail(error.to_string())
}
