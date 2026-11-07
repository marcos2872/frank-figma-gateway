//! Errors from the LiteLLM config store and client.

use ffg_core::error::{ErrorCode, UiError};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LiteLlmError {
    #[error("config inválida: {0}")]
    Config(String),

    #[error("falha de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("resposta inválida do LiteLLM: {0}")]
    InvalidResponse(String),

    #[error("falha de rede")]
    Network,

    #[error("timeout")]
    Timeout,

    #[error("LiteLLM recusou a autenticação")]
    Unauthorized,

    #[error("modelo não encontrado")]
    ModelNotFound,

    #[error("rate limit do provider")]
    RateLimited,

    #[error("provider indisponível")]
    Unavailable,

    #[error("requisição inválida: {0}")]
    BadRequest(String),

    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
}

impl LiteLlmError {
    pub fn to_ui_error(&self) -> UiError {
        match self {
            LiteLlmError::Unauthorized => UiError::new(
                ErrorCode::ProviderAuth,
                "A credencial do provider foi recusada.",
            ),
            LiteLlmError::ModelNotFound => UiError::new(
                ErrorCode::ProviderModelNotFound,
                "O modelo não está disponível no provider.",
            ),
            LiteLlmError::RateLimited => UiError::new(
                ErrorCode::ProviderRateLimited,
                "O provider atingiu o limite de requisições.",
            )
            .retryable(),
            LiteLlmError::Unavailable | LiteLlmError::Network => UiError::new(
                ErrorCode::ProviderUnavailable,
                "O provider está indisponível.",
            )
            .retryable(),
            LiteLlmError::Timeout => {
                UiError::new(ErrorCode::Timeout, "O provider não respondeu a tempo.").retryable()
            }
            LiteLlmError::BadRequest(detail) => UiError::new(
                ErrorCode::ProviderBadRequest,
                "A requisição ao provider foi rejeitada.",
            )
            .with_detail(detail.clone()),
            LiteLlmError::Config(detail) => UiError::new(
                ErrorCode::ConfigInvalid,
                "A configuração do LiteLLM é inválida.",
            )
            .with_detail(detail.clone()),
            LiteLlmError::Io(error) => UiError::new(
                ErrorCode::ConfigApply,
                "Não foi possível gravar a configuração do LiteLLM.",
            )
            .with_detail(error.to_string()),
            LiteLlmError::InvalidResponse(detail)
            | LiteLlmError::Http {
                message: detail, ..
            } => UiError::new(
                ErrorCode::ProviderUnavailable,
                "Resposta inválida do provider.",
            )
            .with_detail(detail.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_provider_errors() {
        assert_eq!(
            LiteLlmError::Unauthorized.to_ui_error().code,
            ErrorCode::ProviderAuth
        );
        assert_eq!(
            LiteLlmError::ModelNotFound.to_ui_error().code,
            ErrorCode::ProviderModelNotFound
        );
        assert_eq!(
            LiteLlmError::RateLimited.to_ui_error().code,
            ErrorCode::ProviderRateLimited
        );
        assert!(LiteLlmError::Network.to_ui_error().retryable);
    }
}
