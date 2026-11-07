//! Figma integration: CA, proxy, HKCU environment and reversible journal.

use crate::commands::proxy::{ensure_proxy, stop_proxy};
use crate::figma::{self, FigmaInfo};
use crate::state::AppState;
use ffg_core::env_plan::{plan_proxy_env, ProxyDesired, ProxyEnvPlan};
use ffg_core::error::{ErrorCode, UiError};
use ffg_core::integration::{IntegrationJournal, IntegrationStatus, IntegrationStep};
use ffg_proxy::proxy;
use serde::Serialize;
use std::fs;
use tauri::State;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationView {
    pub status: IntegrationStatus,
    pub ca_installed: bool,
    pub ca_thumbprint: Option<String>,
    pub proxy_running: bool,
    pub proxy_port: Option<u16>,
    pub env_installed: bool,
    pub figma: FigmaInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationCheck {
    pub id: String,
    pub ok: bool,
    pub blocking: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationValidation {
    pub ok: bool,
    pub checks: Vec<IntegrationCheck>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub async fn integration_status(state: State<'_, AppState>) -> Result<IntegrationView, UiError> {
    let journal = load_journal(&state)?;
    view(&state, &journal)
}

#[tauri::command]
pub async fn figma_environment() -> FigmaInfo {
    figma::detect()
}

#[tauri::command]
pub async fn figma_validate(state: State<'_, AppState>) -> Result<IntegrationValidation, UiError> {
    validate(&state).await
}

#[tauri::command]
pub async fn figma_install(
    restart_figma: Option<bool>,
    state: State<'_, AppState>,
) -> Result<IntegrationView, UiError> {
    install(&state, restart_figma.unwrap_or(false)).await
}

#[tauri::command]
pub async fn figma_remove(state: State<'_, AppState>) -> Result<IntegrationView, UiError> {
    remove(&state).await
}

#[tauri::command]
pub async fn figma_restart(state: State<'_, AppState>) -> Result<FigmaInfo, UiError> {
    figma::restart_with_proxy(current_proxy_port(&state)).map_err(|error| {
        UiError::new(
            ErrorCode::FigmaRestart,
            "Não foi possível reiniciar o Figma.",
        )
        .with_detail(error.to_string())
    })
}

/// Porta do proxy em execução ou a persistida no journal da integração.
fn current_proxy_port(state: &AppState) -> Option<u16> {
    if let Ok(guard) = state.proxy.lock() {
        if let Some(handle) = guard.as_ref() {
            return Some(handle.port());
        }
    }
    load_journal(state)
        .ok()
        .and_then(|journal| journal.proxy_port)
}

#[tauri::command]
pub async fn install_figma_integration(state: State<'_, AppState>) -> Result<SetupResult, UiError> {
    let view = install(&state, false).await?;
    Ok(SetupResult {
        ok: true,
        message: format!(
            "Integração ativa na porta {}.",
            view.proxy_port.unwrap_or(0)
        ),
    })
}

#[tauri::command]
pub async fn remove_figma_integration(state: State<'_, AppState>) -> Result<SetupResult, UiError> {
    remove(&state).await?;
    Ok(SetupResult {
        ok: true,
        message: "Integração removida e ambiente restaurado.".to_owned(),
    })
}

async fn install(state: &AppState, restart_figma: bool) -> Result<IntegrationView, UiError> {
    let journal = load_journal(state)?;
    if journal.status == IntegrationStatus::Active {
        return view(state, &journal);
    }

    let result = install_steps(state, restart_figma).await;
    if result.is_err() {
        let _ = remove(state).await;
    }
    result
}

async fn install_steps(state: &AppState, restart_figma: bool) -> Result<IntegrationView, UiError> {
    let mut journal = load_journal(state)?;

    log::info!("integração: instalando a CA em CurrentUser\\Root");
    let ca = state.ca.install().map_err(|error| error.to_ui_error())?;
    log::info!("integração: CA instalada={}", ca.installed);
    if !ca.installed {
        return Err(UiError::new(
            ErrorCode::CaTrust,
            "A CA não ficou instalada no trust store.",
        ));
    }
    journal.mark_done(IntegrationStep::CaInstall, Some(&now_utc()));
    journal.ca_thumbprint = ca.thumbprint.clone();
    save_journal(state, &journal)?;

    log::info!("integração: subindo o proxy");
    let proxy = ensure_proxy(state).await?;
    log::info!("integração: proxy na porta {}", proxy.port);
    journal.mark_done(IntegrationStep::ProxyStart, Some(&now_utc()));
    journal.proxy_port = Some(proxy.port);
    save_journal(state, &journal)?;

    log::info!("integração: gravando HKCU\\Environment");
    let snapshot = state
        .env
        .snapshot()
        .map_err(|error| platform_error(error.to_string()))?;
    let desired = ProxyDesired {
        proxy_url: format!("http://127.0.0.1:{}", proxy.port),
        no_proxy: Vec::new(),
    };
    let plan = plan_proxy_env(&snapshot, &desired)
        .map_err(|error| UiError::new(ErrorCode::EnvWrite, error.to_string()))?;
    save_env_plan(state, &plan)?;
    state
        .env
        .apply(&plan.mutations)
        .map_err(|error| platform_error(error.to_string()))?;
    journal.mark_done(IntegrationStep::EnvWrite, Some(&now_utc()));
    journal.env_snapshot_ref = Some(env_plan_path(state).display().to_string());
    save_journal(state, &journal)?;

    log::info!("integração: validando TLS/rewrite");
    let validation = validate(state).await?;
    log::info!("integração: validação ok={}", validation.ok);
    if !validation.ok {
        journal.mark_failed(IntegrationStep::Validate, Some(&now_utc()));
        save_journal(state, &journal)?;
        return Err(
            UiError::new(ErrorCode::ProxyTls, "A validação da integração falhou.").with_detail(
                validation
                    .checks
                    .iter()
                    .filter(|check| check.blocking && !check.ok)
                    .map(|check| check.message.clone())
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
        );
    }
    journal.mark_done(IntegrationStep::Validate, Some(&now_utc()));
    save_journal(state, &journal)?;

    if restart_figma {
        let _ = figma::restart_with_proxy(Some(proxy.port));
    }

    let journal = load_journal(state)?;
    view(state, &journal)
}

async fn remove(state: &AppState) -> Result<IntegrationView, UiError> {
    let mut journal = load_journal(state)?;
    journal.begin_removal();

    if let Some(plan) = load_env_plan(state)? {
        state
            .env
            .apply(&plan.restore_mutations())
            .map_err(|error| platform_error(error.to_string()))?;
        let _ = fs::remove_file(env_plan_path(state));
        journal.mark_done(IntegrationStep::EnvRestore, Some(&now_utc()));
    }

    stop_proxy(state)?;
    journal.mark_done(IntegrationStep::ProxyStop, Some(&now_utc()));

    state.ca.remove().map_err(|error| error.to_ui_error())?;
    journal.mark_done(IntegrationStep::CaRemove, Some(&now_utc()));
    journal.mark_removed(Some(&now_utc()));
    save_journal(state, &journal)?;

    view(state, &journal)
}

async fn validate(state: &AppState) -> Result<IntegrationValidation, UiError> {
    let ca = state.ca.status().map_err(|error| error.to_ui_error())?;
    let proxy = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .as_ref()
        .map(|handle| handle.status());
    let snapshot = state
        .env
        .snapshot()
        .map_err(|error| platform_error(error.to_string()))?;

    let proxy_running = proxy.as_ref().is_some_and(|status| status.running);
    let proxy_port = proxy.as_ref().map(|status| status.port);
    let expected_proxy = proxy_port.map(|port| format!("http://127.0.0.1:{port}"));
    let env_ok = expected_proxy.as_ref().is_some_and(|expected| {
        snapshot.https_proxy.as_deref() == Some(expected.as_str())
            && snapshot.http_proxy.as_deref() == Some(expected.as_str())
            && snapshot
                .no_proxy
                .as_deref()
                .is_some_and(|value| value.contains("127.0.0.1") && value.contains("localhost"))
    });

    let self_test = if let (true, Some(port)) = (proxy_running, proxy_port) {
        let ca = state.ca.authority().map_err(|error| error.to_ui_error())?;
        proxy::self_test(port, &ca).await.ok()
    } else {
        None
    };

    let figma = figma::detect();
    let mut checks = vec![
        check(
            "ca",
            ca.installed,
            true,
            "CA instalada em CurrentUser\\Root",
        ),
        check("proxy", proxy_running, true, "proxy rodando em loopback"),
        check(
            "env",
            env_ok,
            true,
            "HTTPS_PROXY/HTTP_PROXY/NO_PROXY configurados",
        ),
        check(
            "rewrite",
            self_test.as_ref().is_some_and(|result| result.ok),
            true,
            "TLS + rewrite + LiteLLM respondendo",
        ),
        check(
            "figma",
            figma.detected,
            false,
            "Figma instalado e detectado",
        ),
    ];
    checks.retain(|check| check.blocking || !check.ok);
    Ok(IntegrationValidation {
        ok: checks
            .iter()
            .filter(|check| check.blocking)
            .all(|check| check.ok),
        checks,
    })
}

fn view(state: &AppState, journal: &IntegrationJournal) -> Result<IntegrationView, UiError> {
    let ca = state.ca.status().map_err(|error| error.to_ui_error())?;
    let proxy = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .as_ref()
        .map(|handle| handle.status());
    let snapshot = state
        .env
        .snapshot()
        .map_err(|error| platform_error(error.to_string()))?;
    let proxy_running = proxy.as_ref().is_some_and(|status| status.running);
    let proxy_port = proxy
        .as_ref()
        .map(|status| status.port)
        .or(journal.proxy_port);
    let env_installed = proxy_port.is_some_and(|port| {
        let expected = format!("http://127.0.0.1:{port}");
        snapshot.https_proxy.as_deref() == Some(expected.as_str())
            && snapshot.http_proxy.as_deref() == Some(expected.as_str())
    });

    Ok(IntegrationView {
        status: journal.status,
        ca_installed: ca.installed,
        ca_thumbprint: ca.thumbprint,
        proxy_running,
        proxy_port,
        env_installed,
        figma: figma::detect(),
    })
}

fn check(id: &str, ok: bool, blocking: bool, message: &str) -> IntegrationCheck {
    IntegrationCheck {
        id: id.to_owned(),
        ok,
        blocking,
        message: message.to_owned(),
    }
}

fn load_journal(state: &AppState) -> Result<IntegrationJournal, UiError> {
    let path = state.paths.integration_file();
    match fs::read(&path) {
        Ok(raw) => serde_json::from_slice(&raw).map_err(|error| {
            UiError::new(ErrorCode::StateCorrupt, "Journal da integração inválido.")
                .with_detail(error.to_string())
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(IntegrationJournal::new()),
        Err(error) => Err(UiError::new(
            ErrorCode::Internal,
            "Não foi possível ler o journal da integração.",
        )
        .with_detail(error.to_string())),
    }
}

fn save_journal(state: &AppState, journal: &IntegrationJournal) -> Result<(), UiError> {
    let path = state.paths.integration_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    let raw = serde_json::to_vec_pretty(journal).map_err(|error| {
        UiError::internal("Não foi possível serializar o journal.").with_detail(error.to_string())
    })?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, raw).map_err(io_error)?;
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    fs::rename(temp, path).map_err(io_error)
}

fn env_plan_path(state: &AppState) -> std::path::PathBuf {
    state.paths.state_dir().join("integration.env.dpapi")
}

fn save_env_plan(state: &AppState, plan: &ProxyEnvPlan) -> Result<(), UiError> {
    let raw = serde_json::to_vec(plan).map_err(|error| {
        UiError::internal("Não foi possível serializar o snapshot de env.")
            .with_detail(error.to_string())
    })?;
    let protected = state
        .ca
        .protector()
        .protect(&raw)
        .map_err(|error| platform_error(error.to_string()))?;
    fs::write(env_plan_path(state), protected).map_err(io_error)
}

fn load_env_plan(state: &AppState) -> Result<Option<ProxyEnvPlan>, UiError> {
    let path = env_plan_path(state);
    if !path.is_file() {
        return Ok(None);
    }
    let protected = fs::read(path).map_err(io_error)?;
    let raw = state
        .ca
        .protector()
        .unprotect(&protected)
        .map_err(|error| platform_error(error.to_string()))?;
    let plan = serde_json::from_slice(&raw).map_err(|error| {
        UiError::new(ErrorCode::StateCorrupt, "Snapshot de env inválido.")
            .with_detail(error.to_string())
    })?;
    Ok(Some(plan))
}

fn platform_error(detail: String) -> UiError {
    UiError::new(
        ErrorCode::EnvWrite,
        "Falha ao manipular o ambiente do Windows.",
    )
    .with_detail(detail)
}

fn io_error(error: std::io::Error) -> UiError {
    UiError::internal("Falha de I/O na integração.").with_detail(error.to_string())
}

fn now_utc() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
