//! GitHub Copilot device flow.
//!
//! Mesmo protocolo que o LiteLLM usa (`litellm.llms.github_copilot.authenticator`):
//! o app obtém o `access_token` do GitHub e o grava no diretório que o LiteLLM
//! lê (`GITHUB_COPILOT_TOKEN_DIR/access-token`), sem tocar na API de management.

use ffg_core::error::{ErrorCode, UiError};
use serde::Serialize;
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
pub const DEFAULT_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
pub const DEFAULT_ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
pub const DEFAULT_USER_URL: &str = "https://api.github.com/user";
const SCOPE: &str = "read:user";

#[derive(Debug, Error)]
pub enum CopilotError {
    #[error("falha de rede: {0}")]
    Network(String),
    #[error("timeout do GitHub")]
    Timeout,
    #[error("HTTP {0} do GitHub")]
    Http(u16),
    #[error("resposta inesperada do GitHub: {0}")]
    InvalidResponse(String),
    #[error("o código de dispositivo expirou")]
    Expired,
    #[error("autorização recusada pelo usuário")]
    Denied,
    #[error("o GitHub pediu para desacelerar o polling")]
    SlowDown,
}

impl CopilotError {
    pub fn to_ui_error(&self) -> UiError {
        match self {
            CopilotError::Expired => UiError::new(
                ErrorCode::CopilotAuthExpired,
                "O código de autorização expirou. Gere um novo.",
            ),
            CopilotError::Denied => UiError::new(
                ErrorCode::CopilotAuthPending,
                "A autorização do GitHub foi recusada.",
            ),
            CopilotError::SlowDown => UiError::new(
                ErrorCode::CopilotAuthPending,
                "O GitHub pediu para aguardar antes de tentar de novo.",
            )
            .retryable(),
            CopilotError::Network(detail) => {
                UiError::new(ErrorCode::Network, "Não foi possível falar com o GitHub.")
                    .with_detail(detail.clone())
                    .retryable()
            }
            CopilotError::Timeout => {
                UiError::new(ErrorCode::Timeout, "O GitHub não respondeu a tempo.").retryable()
            }
            CopilotError::Http(status) => UiError::new(
                ErrorCode::ProviderUnavailable,
                "O GitHub recusou a requisição de autenticação.",
            )
            .with_detail(format!("HTTP {status}"))
            .retryable(),
            CopilotError::InvalidResponse(detail) => UiError::new(
                ErrorCode::CopilotAuthPending,
                "Resposta inesperada do GitHub.",
            )
            .with_detail(detail.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFlow {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollOutcome {
    Pending,
    SlowDown,
    Token(String),
}

pub struct CopilotAuth {
    client: reqwest::Client,
    client_id: String,
    device_code_url: String,
    token_url: String,
    user_url: String,
}

impl CopilotAuth {
    pub fn from_env() -> Self {
        Self {
            client: reqwest::Client::builder()
                .no_proxy()
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap_or_default(),
            client_id: env_or("GITHUB_COPILOT_CLIENT_ID", DEFAULT_CLIENT_ID),
            device_code_url: env_or("GITHUB_COPILOT_DEVICE_CODE_URL", DEFAULT_DEVICE_CODE_URL),
            token_url: env_or("GITHUB_COPILOT_ACCESS_TOKEN_URL", DEFAULT_ACCESS_TOKEN_URL),
            user_url: env_or("GITHUB_COPILOT_USER_URL", DEFAULT_USER_URL),
        }
    }

    pub async fn begin(&self) -> Result<DeviceFlow, CopilotError> {
        let response = self
            .client
            .post(&self.device_code_url)
            .header("accept", "application/json")
            .json(&serde_json::json!({
                "client_id": self.client_id,
                "scope": SCOPE,
            }))
            .send()
            .await
            .map_err(network)?;
        let status = response.status();
        if !status.is_success() {
            return Err(CopilotError::Http(status.as_u16()));
        }
        let body: serde_json::Value = response.json().await.map_err(invalid)?;

        let device_code = string_field(&body, "device_code")?;
        let user_code = string_field(&body, "user_code")?;
        let verification_uri = string_field(&body, "verification_uri")?;

        Ok(DeviceFlow {
            device_code,
            user_code,
            verification_uri,
            expires_in: body
                .get("expires_in")
                .and_then(|value| value.as_u64())
                .unwrap_or(900),
            interval: body
                .get("interval")
                .and_then(|value| value.as_u64())
                .unwrap_or(5),
        })
    }

    pub async fn poll(&self, device_code: &str) -> Result<PollOutcome, CopilotError> {
        let response = self
            .client
            .post(&self.token_url)
            .header("accept", "application/json")
            .json(&serde_json::json!({
                "client_id": self.client_id,
                "device_code": device_code,
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            }))
            .send()
            .await
            .map_err(network)?;
        let status = response.status();
        if !status.is_success() {
            return Err(CopilotError::Http(status.as_u16()));
        }
        let body: serde_json::Value = response.json().await.map_err(invalid)?;

        if let Some(token) = body.get("access_token").and_then(|value| value.as_str()) {
            return Ok(PollOutcome::Token(token.to_owned()));
        }

        match body.get("error").and_then(|value| value.as_str()) {
            Some("authorization_pending") => Ok(PollOutcome::Pending),
            Some("slow_down") => Ok(PollOutcome::SlowDown),
            Some("expired_token") => Err(CopilotError::Expired),
            Some("access_denied") => Err(CopilotError::Denied),
            Some(other) => Err(CopilotError::InvalidResponse(other.to_owned())),
            None => Err(CopilotError::InvalidResponse(
                "resposta sem access_token nem error".to_owned(),
            )),
        }
    }

    /// Login do GitHub associado ao token (para exibir na UI). Falha não é fatal.
    pub async fn login(&self, token: &str) -> Result<Option<String>, CopilotError> {
        let response = self
            .client
            .get(&self.user_url)
            .header("accept", "application/json")
            .header("authorization", format!("Bearer {token}"))
            .send()
            .await
            .map_err(network)?;
        if !response.status().is_success() {
            return Ok(None);
        }
        let body: serde_json::Value = response.json().await.map_err(invalid)?;
        Ok(body
            .get("login")
            .and_then(|value| value.as_str())
            .map(str::to_owned))
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn string_field(body: &serde_json::Value, field: &str) -> Result<String, CopilotError> {
    body.get(field)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
        .ok_or_else(|| CopilotError::InvalidResponse(format!("campo {field} ausente")))
}

fn network(error: reqwest::Error) -> CopilotError {
    if error.is_timeout() {
        CopilotError::Timeout
    } else {
        CopilotError::Network(error.to_string())
    }
}

fn invalid(error: reqwest::Error) -> CopilotError {
    CopilotError::InvalidResponse(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn auth(server: &MockServer) -> CopilotAuth {
        CopilotAuth {
            client: reqwest::Client::new(),
            client_id: "test-client".to_owned(),
            device_code_url: format!("{}/device", server.uri()),
            token_url: format!("{}/token", server.uri()),
            user_url: format!("{}/user", server.uri()),
        }
    }

    #[tokio::test]
    async fn begins_a_device_flow() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/device"))
            .and(header("accept", "application/json"))
            .and(body_partial_json(serde_json::json!({
                "client_id": "test-client",
                "scope": "read:user"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "device_code": "dev-1",
                "user_code": "ABCD-1234",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 900,
                "interval": 5
            })))
            .mount(&server)
            .await;

        let flow = auth(&server).begin().await.expect("device code");
        assert_eq!(flow.user_code, "ABCD-1234");
        assert_eq!(flow.device_code, "dev-1");
        assert_eq!(flow.interval, 5);
    }

    #[tokio::test]
    async fn polls_pending_slow_down_and_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_partial_json(serde_json::json!({
                "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
                "device_code": "dev-1"
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "authorization_pending"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "slow_down"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "access_token": "gho_token"
            })))
            .mount(&server)
            .await;

        let auth = auth(&server);
        assert_eq!(
            auth.poll("dev-1").await.expect("pending"),
            PollOutcome::Pending
        );
        assert_eq!(
            auth.poll("dev-1").await.expect("slow down"),
            PollOutcome::SlowDown
        );
        assert_eq!(
            auth.poll("dev-1").await.expect("token"),
            PollOutcome::Token("gho_token".to_owned())
        );
    }

    #[tokio::test]
    async fn reports_expired_and_denied_codes() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "expired_token"
            })))
            .up_to_n_times(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "error": "access_denied"
            })))
            .mount(&server)
            .await;

        let auth = auth(&server);
        assert!(matches!(
            auth.poll("dev-1").await.expect_err("expired"),
            CopilotError::Expired
        ));
        assert!(matches!(
            auth.poll("dev-1").await.expect_err("denied"),
            CopilotError::Denied
        ));
    }

    #[tokio::test]
    async fn reads_the_github_login() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .and(header("authorization", "Bearer gho_token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "login": "octocat"
            })))
            .mount(&server)
            .await;

        assert_eq!(
            auth(&server).login("gho_token").await.expect("login"),
            Some("octocat".to_owned())
        );
    }
}
