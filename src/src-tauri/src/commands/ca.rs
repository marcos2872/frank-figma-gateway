//! CA status, installation, removal and export commands.

use crate::ca::CaStatus;
use crate::state::AppState;
use ffg_core::error::UiError;
use std::path::Path;
use tauri::State;

#[tauri::command]
pub fn ca_status(state: State<'_, AppState>) -> Result<CaStatus, UiError> {
    state.ca.status().map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub fn ca_install(state: State<'_, AppState>) -> Result<CaStatus, UiError> {
    state.ca.install().map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub fn ca_remove(state: State<'_, AppState>) -> Result<CaStatus, UiError> {
    state.ca.remove().map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub fn ca_export(path: String, state: State<'_, AppState>) -> Result<String, UiError> {
    state
        .ca
        .export(Path::new(&path))
        .map(|path| path.display().to_string())
        .map_err(|error| error.to_ui_error())
}
