//! Provider profiles, model settings and LiteLLM config commands.

use crate::secrets::MASTER_KEY;
use crate::state::AppState;
use ffg_core::error::UiError;
use ffg_core::providers::{catalog, ProviderDescriptor, ProviderProfileInput};
use ffg_core::secrets::{SecretError, SecretKey, SecretValue};
use ffg_core::setup::{ModelSettings, SetupState, StepId};
use ffg_litellm::{LiteLlmClient, ProviderTestResult};
use ffg_supervisor::ServiceName;
use tauri::State;

#[tauri::command]
pub fn providers_catalog() -> Vec<ProviderDescriptor> {
    catalog()
}

#[tauri::command]
pub fn models_get(state: State<'_, AppState>) -> Result<ModelSettings, UiError> {
    Ok(load_setup(&state)?.models)
}

#[tauri::command]
pub async fn models_apply(
    settings: ModelSettings,
    state: State<'_, AppState>,
) -> Result<ModelSettings, UiError> {
    let applied = state
        .litellm
        .apply(&settings)
        .map_err(|error| error.to_ui_error())?;

    let mut setup = load_setup(&state)?;
    setup.models = settings.clone();
    setup.models.applied_at = Some(now_utc());
    setup.models.config_revision = Some(applied.revision);
    setup.invalidate_downstream(StepId::Models);
    save_setup(&state, &mut setup)?;

    // If LiteLLM is already registered, reload it with the new config.
    let _ = state.supervisor.restart(ServiceName::Litellm).await;
    Ok(setup.models)
}

#[tauri::command]
pub fn provider_save(
    profile: ProviderProfileInput,
    state: State<'_, AppState>,
) -> Result<SetupState, UiError> {
    profile.validate().map_err(provider_error)?;
    let selection = profile.to_selection().map_err(provider_error)?;

    if let Some(secret_ref) = &selection.secret_ref {
        if let Some(api_key) = profile
            .api_key
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            state
                .secrets
                .set(&SecretKey::new(secret_ref), &SecretValue::new(api_key))
                .map_err(secret_error)?;
        }
    }

    let mut setup = load_setup(&state)?;
    setup.provider = Some(selection);
    setup.invalidate_downstream(StepId::Provider);
    save_setup(&state, &mut setup)?;
    Ok(setup)
}

#[tauri::command]
pub fn provider_remove(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<SetupState, UiError> {
    let key = format!("ffg/provider/{profile_id}");
    state
        .secrets
        .delete(&SecretKey::new(key))
        .map_err(secret_error)?;

    let mut setup = load_setup(&state)?;
    setup.provider = None;
    setup.invalidate_downstream(StepId::Provider);
    save_setup(&state, &mut setup)?;
    Ok(setup)
}

#[tauri::command]
pub fn provider_secret_status(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<bool, UiError> {
    let key = SecretKey::new(format!("ffg/provider/{profile_id}"));
    state
        .secrets
        .get(&key)
        .map(|value| value.is_some())
        .map_err(secret_error)
}

#[tauri::command]
pub async fn provider_test(
    model: String,
    state: State<'_, AppState>,
) -> Result<ProviderTestResult, UiError> {
    let setup = load_setup(&state)?;
    let port = setup
        .services
        .litellm
        .ok_or_else(|| UiError::internal("LiteLLM ainda não tem uma porta atribuída."))?;
    let master = state
        .secrets
        .get(&SecretKey::new(MASTER_KEY))
        .map_err(secret_error)?
        .ok_or_else(|| {
            UiError::new(
                ffg_core::error::ErrorCode::ProviderAuth,
                "A chave mestre do LiteLLM ainda não foi criada.",
            )
        })?;

    LiteLlmClient::new(format!("http://127.0.0.1:{port}"), master.expose())
        .test_message(&model)
        .await
        .map_err(|error| error.to_ui_error())
}

#[tauri::command]
pub async fn litellm_health(state: State<'_, AppState>) -> Result<(), UiError> {
    let setup = load_setup(&state)?;
    let port = setup
        .services
        .litellm
        .ok_or_else(|| UiError::internal("LiteLLM ainda não tem uma porta atribuída."))?;
    let master = state
        .secrets
        .get(&SecretKey::new(MASTER_KEY))
        .map_err(secret_error)?
        .ok_or_else(|| {
            UiError::new(
                ffg_core::error::ErrorCode::ProviderAuth,
                "A chave mestre do LiteLLM ainda não foi criada.",
            )
        })?;
    LiteLlmClient::new(format!("http://127.0.0.1:{port}"), master.expose())
        .health()
        .await
        .map_err(|error| error.to_ui_error())
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

fn provider_error(error: ffg_core::providers::ProviderError) -> UiError {
    UiError::new(
        ffg_core::error::ErrorCode::ProviderBadRequest,
        error.to_string(),
    )
}

fn secret_error(error: SecretError) -> UiError {
    UiError::internal("Não foi possível acessar o cofre de credenciais.")
        .with_detail(error.to_string())
}

fn now_utc() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}
