//! Runtime download/install commands.

use crate::state::AppState;
use ffg_core::download::ComponentId;
use ffg_core::install::ActiveRuntime;
use ffg_core::manifest::RuntimeManifest;
use ffg_runtime::{Installation, ProgressCallback};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

#[tauri::command]
pub fn runtime_status(
    state: State<'_, AppState>,
) -> Result<ActiveRuntime, ffg_core::error::UiError> {
    state.runtime.active().map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub async fn runtime_fetch_manifest(
    manifest_url: String,
    signature_url: String,
    state: State<'_, AppState>,
) -> Result<RuntimeManifest, ffg_core::error::UiError> {
    state
        .runtime
        .fetch_manifest(&manifest_url, &signature_url)
        .await
        .map_err(|error| error.to_ui_error())
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn runtime_install(
    component: ComponentId,
    manifest_url: String,
    signature_url: String,
    operation_id: String,
    mirrors: Option<Vec<String>>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<Installation, ffg_core::error::UiError> {
    let manifest = state
        .runtime
        .fetch_manifest(&manifest_url, &signature_url)
        .await
        .map_err(|error| error.to_ui_error())?;
    let component_manifest = state
        .runtime
        .component(&manifest, component)
        .map_err(|error| error.to_ui_error())?
        .clone();

    let cancel = CancellationToken::new();
    {
        let mut operations = state.operations.lock().map_err(|_| {
            ffg_core::error::UiError::internal("registro de operações indisponível")
        })?;
        operations.insert(operation_id.clone(), cancel.clone());
    }

    let progress_app = app.clone();
    let progress: ProgressCallback = Arc::new(move |event| {
        if let Err(error) = progress_app.emit("download://progress", event) {
            log::warn!("falha ao emitir download://progress: {error}");
        }
    });

    let result = async {
        let zip = state
            .runtime
            .download(
                component,
                &component_manifest,
                mirrors.unwrap_or_default(),
                Some(progress),
                Some(cancel),
            )
            .await?;
        state
            .runtime
            .install(component, &component_manifest, &zip)
            .await
    }
    .await;

    if let Ok(mut operations) = state.operations.lock() {
        operations.remove(&operation_id);
    }

    let installation = result.map_err(|error| error.to_ui_error())?;
    sync_runtime_state(&state, &app, component, &installation.version);
    Ok(installation)
}

/// Mantém `setup.json` alinhado com o `active.json` do runtime, para a UI saber
/// o que já está instalado (inclusive o Postgres usado pelo Copilot).
fn sync_runtime_state(state: &AppState, app: &AppHandle, component: ComponentId, version: &str) {
    let Ok(store) = state.setup.lock() else {
        return;
    };
    let Ok(mut setup) = store.load().map(|result| result.state) else {
        return;
    };
    match component {
        ComponentId::Litellm => setup.runtime.active_litellm = Some(version.to_owned()),
        ComponentId::Pgsql => setup.runtime.active_pgsql = Some(version.to_owned()),
    }
    if store.save(&mut setup).is_ok() {
        if let Err(error) = app.emit(crate::events::SETUP_STATE, setup) {
            log::warn!("falha ao emitir {}: {error}", crate::events::SETUP_STATE);
        }
    }
}

#[tauri::command]
pub fn runtime_cancel(
    operation_id: String,
    state: State<'_, AppState>,
) -> Result<(), ffg_core::error::UiError> {
    let operations = state
        .operations
        .lock()
        .map_err(|_| ffg_core::error::UiError::internal("registro de operações indisponível"))?;
    if let Some(token) = operations.get(&operation_id) {
        token.cancel();
    }
    Ok(())
}
