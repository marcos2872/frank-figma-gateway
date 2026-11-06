use ffg_core::error::UiError;
use serde::Serialize;

/// States emitted by the scaffold. The real supervisor will transition through
/// these values once process supervision and health checks are wired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // reserved for the real supervisor (F4)
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Degraded,
    Failed,
    Stopping,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // reserved for health checks (F4)
pub enum HealthState {
    Unknown,
    Ok,
    Timeout,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub name: String,
    pub state: ServiceState,
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    pub health: HealthState,
    pub restart_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<UiError>,
}

/// Placeholder until the real supervisor spawns Postgres/LiteLLM.
pub fn placeholder_status() -> Vec<ServiceStatus> {
    vec![
        ServiceStatus {
            name: "postgres".to_owned(),
            state: ServiceState::Stopped,
            detail: Some("necessário no perfil Copilot".to_owned()),
            port: None,
            health: HealthState::Unknown,
            restart_count: 0,
            last_error: None,
        },
        ServiceStatus {
            name: "litellm".to_owned(),
            state: ServiceState::Stopped,
            detail: None,
            port: None,
            health: HealthState::Unknown,
            restart_count: 0,
            last_error: None,
        },
        ServiceStatus {
            name: "proxy".to_owned(),
            state: ServiceState::Stopped,
            detail: None,
            port: None,
            health: HealthState::Unknown,
            restart_count: 0,
            last_error: None,
        },
    ]
}
