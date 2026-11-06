//! IPC commands for the onboarding state.

use crate::events;
use crate::state::AppState;
use ffg_core::error::{ErrorCode, UiError};
use ffg_core::setup::{ResetScope, SetupError, SetupState, StepId, StepOutcome};
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub fn setup_get_state(state: State<'_, AppState>) -> Result<SetupState, UiError> {
    let store = state
        .setup
        .lock()
        .map_err(|_| UiError::internal("estado do app indisponível"))?;
    log::debug!("carregando estado de {}", state.paths.root().display());
    let result = store.load().map_err(UiError::from)?;
    log::debug!("origem do estado: {:?}", result.source);
    if let Some(warning) = result.warning {
        log::warn!("{}", warning.message);
    }
    let mut setup = result.state;
    if reconcile_runtime(&state, &mut setup) {
        store.save(&mut setup).map_err(UiError::from)?;
    }
    Ok(setup)
}

/// `active.json` é a fonte de verdade do runtime. Se o `setup.json` ficou para
/// trás (app antigo, crash no meio), a leitura corrige antes de devolver à UI.
fn reconcile_runtime(state: &AppState, setup: &mut SetupState) -> bool {
    let Ok(active) = state.runtime.active() else {
        return false;
    };
    let mut changed = false;

    if let Some(litellm) = active.litellm {
        if setup.runtime.active_litellm.as_deref() != Some(litellm.version.as_str()) {
            setup.runtime.litellm_version = Some(litellm.version.clone());
            setup.runtime.active_litellm = Some(litellm.version);
            changed = true;
        }
    }
    if let Some(pgsql) = active.pgsql {
        if setup.runtime.active_pgsql.as_deref() != Some(pgsql.version.as_str()) {
            setup.runtime.pgsql_version = Some(pgsql.version.clone());
            setup.runtime.active_pgsql = Some(pgsql.version);
            changed = true;
        }
    }

    changed
}

#[tauri::command]
pub fn setup_begin_step(
    step: StepId,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<SetupState, UiError> {
    let store = state
        .setup
        .lock()
        .map_err(|_| UiError::internal("estado do app indisponível"))?;
    let mut setup = store.load().map_err(UiError::from)?.state;
    setup.begin(step).map_err(setup_error_to_ui)?;
    store.save(&mut setup).map_err(UiError::from)?;
    emit_state(&app, &setup);
    Ok(setup)
}

#[tauri::command]
pub fn setup_complete_step(
    step: StepId,
    outcome: StepOutcome,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<SetupState, UiError> {
    let store = state
        .setup
        .lock()
        .map_err(|_| UiError::internal("estado do app indisponível"))?;
    let mut setup = store.load().map_err(UiError::from)?.state;
    setup
        .complete(step, outcome, Some(&now_utc()))
        .map_err(setup_error_to_ui)?;
    store.save(&mut setup).map_err(UiError::from)?;
    emit_state(&app, &setup);
    Ok(setup)
}

#[tauri::command]
pub fn setup_set_options(
    offline_mode: Option<bool>,
    start_with_windows: Option<bool>,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<SetupState, UiError> {
    let store = state
        .setup
        .lock()
        .map_err(|_| UiError::internal("estado do app indisponível"))?;
    let mut setup = store.load().map_err(UiError::from)?.state;
    setup.apply_options(offline_mode, start_with_windows);
    store.save(&mut setup).map_err(UiError::from)?;
    emit_state(&app, &setup);
    Ok(setup)
}

#[tauri::command]
pub fn setup_reset(
    scope: ResetScope,
    state: State<'_, AppState>,
    app: AppHandle,
) -> Result<SetupState, UiError> {
    let store = state
        .setup
        .lock()
        .map_err(|_| UiError::internal("estado do app indisponível"))?;
    let current = store.load().map_err(UiError::from)?.state;
    let mut next = current.reset(scope);
    store.save(&mut next).map_err(UiError::from)?;
    emit_state(&app, &next);
    Ok(next)
}

fn setup_error_to_ui(error: SetupError) -> UiError {
    let code = match &error {
        SetupError::StepOutOfOrder { .. } => ErrorCode::StepOrder,
        SetupError::StepNotStarted(_) => ErrorCode::StepNotStarted,
        SetupError::StepCannotSkip(_) | SetupError::SkipReasonRequired(_) => {
            ErrorCode::StepCannotSkip
        }
        SetupError::UnknownStep(_) => ErrorCode::Internal,
    };
    UiError::new(code, error.to_string())
}

fn emit_state(app: &AppHandle, state: &SetupState) {
    if let Err(error) = app.emit(events::SETUP_STATE, state.clone()) {
        log::warn!("falha ao emitir {}: {error}", events::SETUP_STATE);
    }
}

fn now_utc() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
