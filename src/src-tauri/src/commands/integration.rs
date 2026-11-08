//! Figma integration: CA, proxy, per-process Figma environment and reversible journal.

use crate::commands::proxy::{ensure_proxy, stop_proxy};
use crate::figma::{self, FigmaInfo};
use crate::platform::SystemProxySnapshot;
use crate::state::AppState;
use ffg_core::env_plan::{
    EnvMutation, EnvSnapshot, ProxyEnvPlan, HTTPS_PROXY, HTTP_PROXY, LOOPBACK_NO_PROXY, NO_PROXY,
};
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
        // Reabrir a integração também limpa resíduos globais de versões antigas.
        ensure_proxy(state).await?;
        cleanup_legacy_env(state)?;
        restore_legacy_system_proxy(state);
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

    // O app não altera o proxy global: o Figma é aberto com o proxy apenas
    // no processo dele. Limpa resíduos de versões antigas.
    log::info!("integração: limpando configurações globais de versões antigas");
    cleanup_legacy_env(state)?;
    restore_legacy_system_proxy(state);

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
    } else {
        log::info!("integração: abra o Figma pelo app para injetar o proxy no processo");
    }

    let journal = load_journal(state)?;
    view(state, &journal)
}

async fn remove(state: &AppState) -> Result<IntegrationView, UiError> {
    let mut journal = load_journal(state)?;
    journal.begin_removal();

    cleanup_legacy_env(state)?;
    restore_legacy_system_proxy(state);

    stop_proxy(state)?;
    journal.mark_done(IntegrationStep::ProxyStop, Some(&now_utc()));

    state.ca.remove().map_err(|error| error.to_ui_error())?;
    journal.mark_done(IntegrationStep::CaRemove, Some(&now_utc()));
    journal.mark_removed(Some(&now_utc()));
    save_journal(state, &journal)?;

    view(state, &journal)
}

/// Removes proxy variables written globally by older versions.
///
/// The integration no longer changes `HKCU\Environment`: the proxy is injected
/// only into the Figma process so CLI tools keep their own network settings.
/// A legacy snapshot with a corporate proxy is restored; values that point at
/// our own proxy are removed.
pub(crate) fn cleanup_legacy_env(state: &AppState) -> Result<(), UiError> {
    let Some(port) = known_proxy_port(state) else {
        let _ = fs::remove_file(env_plan_path(state));
        return Ok(());
    };
    let mutations = match load_env_plan(state)? {
        Some(plan) => legacy_restore_mutations(&plan, port),
        None => {
            let snapshot = state
                .env
                .snapshot()
                .map_err(|error| platform_error(error.to_string()))?;
            legacy_env_mutations(&snapshot, port)
        }
    };
    if !mutations.is_empty() {
        state
            .env
            .apply(&mutations)
            .map_err(|error| platform_error(error.to_string()))?;
        log::info!("integração: variáveis de proxy globais de versões antigas ajustadas");
    }
    let _ = fs::remove_file(env_plan_path(state));
    Ok(())
}

fn known_proxy_port(state: &AppState) -> Option<u16> {
    current_proxy_port(state).or_else(|| {
        state
            .setup
            .lock()
            .ok()
            .and_then(|store| store.load().ok())
            .and_then(|loaded| loaded.state.services.proxy)
    })
}

fn legacy_restore_mutations(plan: &ProxyEnvPlan, port: u16) -> Vec<EnvMutation> {
    plan.restore_mutations()
        .into_iter()
        .map(|mutation| match mutation {
            EnvMutation::Set { name, value }
                if (name == HTTP_PROXY || name == HTTPS_PROXY) && points_to_us(&value, port) =>
            {
                EnvMutation::Remove { name }
            }
            EnvMutation::Set { name, value } if name == NO_PROXY && is_loopback_only(&value) => {
                EnvMutation::Remove { name }
            }
            other => other,
        })
        .collect()
}

fn legacy_env_mutations(snapshot: &EnvSnapshot, port: u16) -> Vec<EnvMutation> {
    let mut mutations = Vec::new();
    if snapshot
        .https_proxy
        .as_deref()
        .is_some_and(|value| points_to_us(value, port))
    {
        mutations.push(EnvMutation::Remove {
            name: HTTPS_PROXY.to_owned(),
        });
    }
    if snapshot
        .http_proxy
        .as_deref()
        .is_some_and(|value| points_to_us(value, port))
    {
        mutations.push(EnvMutation::Remove {
            name: HTTP_PROXY.to_owned(),
        });
    }
    if snapshot.no_proxy.as_deref().is_some_and(is_loopback_only) {
        mutations.push(EnvMutation::Remove {
            name: NO_PROXY.to_owned(),
        });
    }
    mutations
}

fn points_to_us(value: &str, port: u16) -> bool {
    let ours = format!("http://127.0.0.1:{port}");
    value.trim_end_matches('/').eq_ignore_ascii_case(&ours)
}

fn is_loopback_only(value: &str) -> bool {
    let entries: Vec<String> = value
        .split(',')
        .map(|entry| entry.trim().to_ascii_lowercase())
        .filter(|entry| !entry.is_empty())
        .collect();
    entries.len() == LOOPBACK_NO_PROXY.len()
        && LOOPBACK_NO_PROXY
            .iter()
            .all(|expected| entries.iter().any(|entry| entry.as_str() == *expected))
}

/// Restores the WinINET proxy left behind by older versions.
///
/// The current integration never changes the system proxy: the Figma process
/// receives `--proxy-server`. This cleanup runs on boot, install, removal and
/// exit only to undo legacy state.
pub(crate) fn restore_legacy_system_proxy(state: &AppState) {
    match load_system_proxy_snapshot(state) {
        Ok(Some(snapshot)) => match state.system_proxy.restore(&snapshot) {
            Ok(()) => {
                let _ = fs::remove_file(system_proxy_path(state));
                log::info!("integração: proxy do sistema legado restaurado");
            }
            Err(error) => {
                log::warn!("falha ao restaurar o proxy do sistema legado: {error}");
            }
        },
        Ok(None) => {
            let port = load_journal(state)
                .ok()
                .and_then(|journal| journal.proxy_port);
            let Some(port) = port else {
                return;
            };
            let Ok(current) = state.system_proxy.snapshot() else {
                return;
            };
            if !is_our_proxy(&current, port) {
                return;
            }
            let mut disabled = current;
            disabled.enabled = Some(0);
            if let Err(error) = state.system_proxy.restore(&disabled) {
                log::warn!("falha ao desligar o proxy do sistema legado: {error}");
            } else {
                log::info!("integração: proxy do sistema legado desligado");
            }
        }
        Err(error) => {
            log::warn!(
                "falha ao ler o snapshot do proxy do sistema legado: {}",
                error.message
            );
        }
    }
}

fn is_our_proxy(snapshot: &SystemProxySnapshot, port: u16) -> bool {
    let expected = format!("127.0.0.1:{port}");
    snapshot.enabled == Some(1) && snapshot.server.as_deref() == Some(expected.as_str())
}

async fn validate(state: &AppState) -> Result<IntegrationValidation, UiError> {
    let ca = state.ca.status().map_err(|error| error.to_ui_error())?;
    let proxy = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .as_ref()
        .map(|handle| handle.status());
    let proxy_running = proxy.as_ref().is_some_and(|status| status.running);
    let proxy_port = proxy.as_ref().map(|status| status.port);

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
    let proxy_running = proxy.as_ref().is_some_and(|status| status.running);
    let proxy_port = proxy
        .as_ref()
        .map(|status| status.port)
        .or(journal.proxy_port);

    Ok(IntegrationView {
        status: journal.status,
        ca_installed: ca.installed,
        ca_thumbprint: ca.thumbprint,
        proxy_running,
        proxy_port,
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

fn system_proxy_path(state: &AppState) -> std::path::PathBuf {
    state.paths.state_dir().join("integration.proxy.dpapi")
}

fn load_system_proxy_snapshot(state: &AppState) -> Result<Option<SystemProxySnapshot>, UiError> {
    let path = system_proxy_path(state);
    if !path.is_file() {
        return Ok(None);
    }
    let protected = fs::read(path).map_err(io_error)?;
    let raw = state
        .ca
        .protector()
        .unprotect(&protected)
        .map_err(|error| platform_error(error.to_string()))?;
    let snapshot = serde_json::from_slice(&raw).map_err(|error| {
        UiError::new(
            ErrorCode::StateCorrupt,
            "Snapshot do proxy do sistema inválido.",
        )
        .with_detail(error.to_string())
    })?;
    Ok(Some(snapshot))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_when_the_system_proxy_is_ours() {
        let ours = SystemProxySnapshot {
            enabled: Some(1),
            server: Some("127.0.0.1:57995".to_owned()),
            override_list: Some("<local>".to_owned()),
            auto_config_url: None,
        };
        assert!(is_our_proxy(&ours, 57995));
        assert!(!is_our_proxy(&ours, 57996));
    }

    #[test]
    fn ignores_disabled_or_other_servers() {
        let disabled = SystemProxySnapshot {
            enabled: Some(0),
            server: Some("127.0.0.1:57995".to_owned()),
            ..SystemProxySnapshot::default()
        };
        let corporate = SystemProxySnapshot {
            enabled: Some(1),
            server: Some("proxy.corp:8080".to_owned()),
            ..SystemProxySnapshot::default()
        };
        assert!(!is_our_proxy(&disabled, 57995));
        assert!(!is_our_proxy(&corporate, 57995));
    }

    fn snapshot_with(proxy: Option<&str>, no_proxy: Option<&str>) -> EnvSnapshot {
        EnvSnapshot {
            https_proxy: proxy.map(str::to_owned),
            http_proxy: proxy.map(str::to_owned),
            no_proxy: no_proxy.map(str::to_owned),
            all_proxy: None,
        }
    }

    #[test]
    fn removes_legacy_env_values_that_point_at_us() {
        let snapshot = snapshot_with(
            Some("http://127.0.0.1:57995"),
            Some("localhost,127.0.0.1,::1"),
        );
        assert_eq!(legacy_env_mutations(&snapshot, 57995).len(), 3);
    }

    #[test]
    fn keeps_corporate_proxies_and_extra_no_proxy_entries() {
        let snapshot = snapshot_with(
            Some("http://proxy.corp:8080"),
            Some("corp.local,localhost,127.0.0.1,::1"),
        );
        assert!(legacy_env_mutations(&snapshot, 57995).is_empty());
    }

    #[test]
    fn matches_legacy_values_with_a_trailing_slash() {
        let snapshot = snapshot_with(Some("http://127.0.0.1:57995/"), None);
        assert_eq!(legacy_env_mutations(&snapshot, 57995).len(), 2);
    }

    #[test]
    fn keeps_a_loopback_proxy_from_another_port() {
        let snapshot = snapshot_with(Some("http://127.0.0.1:57996"), None);
        assert!(legacy_env_mutations(&snapshot, 57995).is_empty());
    }

    fn plan_with(snapshot_proxy: Option<&str>, snapshot_no_proxy: Option<&str>) -> ProxyEnvPlan {
        use ffg_core::env_plan::{plan_proxy_env, ProxyDesired};

        let snapshot = snapshot_with(snapshot_proxy, snapshot_no_proxy);
        plan_proxy_env(
            &snapshot,
            &ProxyDesired {
                proxy_url: "http://127.0.0.1:57995".to_owned(),
                no_proxy: Vec::new(),
            },
        )
        .expect("plan")
    }

    #[test]
    fn restores_a_corporate_proxy_from_a_legacy_plan() {
        let plan = plan_with(Some("http://proxy.corp:8080"), Some("corp.local"));
        let mutations = legacy_restore_mutations(&plan, 57995);

        assert!(mutations.iter().any(|mutation| matches!(
            mutation,
            EnvMutation::Set { name, value }
                if name == HTTPS_PROXY && value == "http://proxy.corp:8080"
        )));
        assert!(mutations.iter().any(|mutation| matches!(
            mutation,
            EnvMutation::Set { name, value }
                if name == NO_PROXY && value == "corp.local"
        )));
    }

    #[test]
    fn discards_a_poisoned_snapshot_from_a_legacy_plan() {
        let plan = plan_with(
            Some("http://127.0.0.1:57995"),
            Some("localhost,127.0.0.1,::1"),
        );
        let mutations = legacy_restore_mutations(&plan, 57995);

        assert_eq!(mutations.len(), 3);
        assert!(mutations
            .iter()
            .all(|mutation| matches!(mutation, EnvMutation::Remove { .. })));
    }
}
