use crate::supervisor::{placeholder_status, ServiceState, ServiceStatus};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    pub ok: bool,
    pub message: String,
}

#[tauri::command]
pub fn service_status() -> Vec<ServiceStatus> {
    placeholder_status()
}

#[tauri::command]
pub fn start_services() -> Vec<ServiceStatus> {
    with_state(ServiceState::Starting)
}

#[tauri::command]
pub fn stop_services() -> Vec<ServiceStatus> {
    with_state(ServiceState::Stopped)
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

fn with_state(state: ServiceState) -> Vec<ServiceStatus> {
    placeholder_status()
        .into_iter()
        .map(|mut service| {
            service.state = state;
            service
        })
        .collect()
}
