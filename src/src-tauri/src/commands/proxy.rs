//! Proxy lifecycle and self-test commands.

use crate::secrets::MASTER_KEY;
use crate::state::AppState;
use ffg_core::error::{ErrorCode, UiError};
use ffg_core::secrets::SecretKey;
use ffg_core::setup::SetupState;
use ffg_proxy::proxy::{self, ProxyConfig, ProxySelfTest, ProxyStatus};
use ffg_supervisor::allocate_port;
use tauri::State;

#[tauri::command]
pub async fn proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, UiError> {
    if let Some(handle) = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .as_ref()
    {
        return Ok(handle.status());
    }
    let setup = load_setup(&state)?;
    Ok(ProxyStatus {
        running: false,
        port: setup.services.proxy.unwrap_or(0),
        connections: 0,
        ai_requests: 0,
        failures: 0,
        first_ai_request_at: None,
        last_error: None,
    })
}

#[tauri::command]
pub async fn proxy_start(state: State<'_, AppState>) -> Result<ProxyStatus, UiError> {
    ensure_proxy(&state).await
}

pub async fn ensure_proxy(state: &AppState) -> Result<ProxyStatus, UiError> {
    if let Some(handle) = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .as_ref()
    {
        return Ok(handle.status());
    }

    let mut setup = load_setup(state)?;
    let proxy_port = match setup.services.proxy {
        Some(port) => port,
        None => {
            let port = allocate_port()
                .map_err(|error| UiError::internal(format!("porta do proxy: {error}")))?;
            setup.services.proxy = Some(port);
            save_setup(state, &mut setup)?;
            port
        }
    };
    let litellm_port = setup
        .services
        .litellm
        .ok_or_else(|| UiError::internal("LiteLLM ainda não tem uma porta atribuída."))?;
    let master = state
        .secrets
        .get(&SecretKey::new(MASTER_KEY))
        .map_err(|error| {
            UiError::internal("cofre de credenciais indisponível").with_detail(error.to_string())
        })?
        .ok_or_else(|| {
            UiError::new(
                ErrorCode::ProviderAuth,
                "A chave mestre do LiteLLM ainda não foi criada.",
            )
        })?;
    let ca = state.ca.authority().map_err(|error| error.to_ui_error())?;

    let handle = proxy::start(ProxyConfig {
        port: proxy_port,
        ca,
        lite_llm_url: format!("http://127.0.0.1:{litellm_port}"),
        master_key: master.expose().to_owned(),
        figma_upstream: "https://api.figma.com".to_owned(),
        mitm_hosts: vec!["api.figma.com".to_owned()],
    })
    .await
    .map_err(proxy_error)?;

    let status = handle.status();
    *state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))? = Some(handle);
    Ok(status)
}

#[tauri::command]
pub async fn proxy_stop(state: State<'_, AppState>) -> Result<ProxyStatus, UiError> {
    match stop_proxy(&state)? {
        Some(status) => Ok(status),
        None => proxy_status(state).await,
    }
}

pub fn stop_proxy(state: &AppState) -> Result<Option<ProxyStatus>, UiError> {
    let handle = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .take();
    Ok(handle.map(|handle| {
        handle.stop();
        handle.status()
    }))
}

#[tauri::command]
pub async fn proxy_self_test(state: State<'_, AppState>) -> Result<ProxySelfTest, UiError> {
    let port = state
        .proxy
        .lock()
        .map_err(|_| UiError::internal("proxy lock envenenado"))?
        .as_ref()
        .map(|handle| handle.port())
        .ok_or_else(|| UiError::internal("O proxy não está rodando."))?;
    let ca = state.ca.authority().map_err(|error| error.to_ui_error())?;
    proxy::self_test(port, &ca).await.map_err(proxy_error)
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

fn proxy_error(error: ffg_proxy::proxy::ProxyError) -> UiError {
    use ffg_proxy::proxy::ProxyError;
    match error {
        ProxyError::Ca(_) => UiError::new(
            ErrorCode::CaGeneration,
            "Não foi possível preparar a CA do proxy.",
        )
        .with_detail(error.to_string()),
        ProxyError::Bind(_) => UiError::new(
            ErrorCode::ProxyUpstream,
            "Não foi possível abrir a porta do proxy.",
        )
        .with_detail(error.to_string()),
        ProxyError::SelfTest(_) => {
            UiError::new(ErrorCode::ProxyTls, "O self-test do proxy falhou.")
                .with_detail(error.to_string())
        }
    }
}
