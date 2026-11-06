use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Degraded,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub name: String,
    pub state: ServiceState,
    pub detail: Option<String>,
}

/// Placeholder until the real supervisor spawns Postgres/LiteLLM.
pub fn placeholder_status() -> Vec<ServiceStatus> {
    vec![
        ServiceStatus {
            name: "postgres".to_owned(),
            state: ServiceState::Stopped,
            detail: Some("necessário no perfil Copilot".to_owned()),
        },
        ServiceStatus {
            name: "litellm".to_owned(),
            state: ServiceState::Stopped,
            detail: None,
        },
        ServiceStatus {
            name: "proxy".to_owned(),
            state: ServiceState::Stopped,
            detail: None,
        },
    ]
}
