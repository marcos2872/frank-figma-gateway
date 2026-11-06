//! LiteLLM health, model discovery and provider test client.

use crate::error::LiteLlmError;
use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTestResult {
    pub ok: bool,
    pub status: u16,
    pub message: String,
    pub latency_ms: u64,
}

#[derive(Clone)]
pub struct LiteLlmClient {
    base_url: String,
    master_key: String,
    client: reqwest::Client,
}

impl LiteLlmClient {
    pub fn new(base_url: impl Into<String>, master_key: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            master_key: master_key.into(),
            client: reqwest::Client::builder()
                .user_agent("frank-figma-gateway")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    pub async fn health(&self) -> Result<(), LiteLlmError> {
        let response = self
            .client
            .get(format!("{}/health/readiness", self.base_url))
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(map_reqwest)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(LiteLlmError::Http {
                status: response.status().as_u16(),
                message: "health check falhou".to_owned(),
            })
        }
    }

    pub async fn test_message(&self, model: &str) -> Result<ProviderTestResult, LiteLlmError> {
        let started = Instant::now();
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .bearer_auth(&self.master_key)
            .header("anthropic-version", "2023-06-01")
            .json(&serde_json::json!({
                "model": model,
                "max_tokens": 8,
                "messages": [{"role": "user", "content": "ping"}]
            }))
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(map_reqwest)?;

        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        if (200..300).contains(&status) {
            Ok(ProviderTestResult {
                ok: true,
                status,
                message: "conexão e modelo OK".to_owned(),
                latency_ms: started.elapsed().as_millis() as u64,
            })
        } else {
            Err(map_status(status, &body))
        }
    }

    pub async fn models(&self) -> Result<Vec<String>, LiteLlmError> {
        let response = self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .bearer_auth(&self.master_key)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(map_status(
                response.status().as_u16(),
                &response.text().await.unwrap_or_default(),
            ));
        }
        let value: serde_json::Value = response
            .json()
            .await
            .map_err(|error| LiteLlmError::InvalidResponse(error.to_string()))?;
        Ok(value
            .get("data")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.get("id").and_then(serde_json::Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }
}

fn map_reqwest(error: reqwest::Error) -> LiteLlmError {
    if error.is_timeout() {
        LiteLlmError::Timeout
    } else {
        LiteLlmError::Network
    }
}

fn map_status(status: u16, body: &str) -> LiteLlmError {
    match status {
        401 | 403 => LiteLlmError::Unauthorized,
        404 => LiteLlmError::ModelNotFound,
        429 => LiteLlmError::RateLimited,
        400 => LiteLlmError::BadRequest(truncate(body)),
        500..=599 => LiteLlmError::Unavailable,
        _ => LiteLlmError::Http {
            status,
            message: truncate(body),
        },
    }
}

fn truncate(value: &str) -> String {
    value.chars().take(200).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn client(server: &MockServer) -> LiteLlmClient {
        LiteLlmClient::new(server.uri(), "master-key")
    }

    #[tokio::test]
    async fn reports_a_successful_message_test() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("authorization", "Bearer master-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "msg_1",
                "content": [{"type": "text", "text": "pong"}]
            })))
            .mount(&server)
            .await;

        let result = client(&server)
            .test_message("claude-opus-4-8")
            .await
            .expect("test");
        assert!(result.ok);
        assert_eq!(result.status, 200);
    }

    #[tokio::test]
    async fn maps_provider_statuses() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(401).set_body_string("unauthorized"))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).test_message("claude-opus-4-8").await,
            Err(LiteLlmError::Unauthorized)
        ));

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(404).set_body_string("model not found"))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).test_message("claude-opus-4-8").await,
            Err(LiteLlmError::ModelNotFound)
        ));

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
            .mount(&server)
            .await;
        assert!(matches!(
            client(&server).test_message("claude-opus-4-8").await,
            Err(LiteLlmError::RateLimited)
        ));
    }

    #[tokio::test]
    async fn discovers_models() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{"id": "claude-opus-4-8"}, {"id": "claude-haiku-4-5"}]
            })))
            .mount(&server)
            .await;
        let models = client(&server).models().await.expect("models");
        assert_eq!(models, vec!["claude-opus-4-8", "claude-haiku-4-5"]);
    }

    #[tokio::test]
    async fn checks_health() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health/readiness"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        client(&server).health().await.expect("health");
    }
}
