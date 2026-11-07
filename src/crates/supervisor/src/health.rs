//! Health checks for sidecars.

use serde::Serialize;
use std::time::Duration;
use tokio::net::TcpStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HealthState {
    Unknown,
    Ok,
    Timeout,
    Error,
}

#[derive(Debug, Clone)]
pub enum HealthCheck {
    None,
    Tcp { port: u16, timeout: Duration },
    Http { url: String, timeout: Duration },
}

impl HealthCheck {
    pub fn port(&self) -> Option<u16> {
        match self {
            HealthCheck::Tcp { port, .. } => Some(*port),
            _ => None,
        }
    }
}

pub async fn check_health(
    client: &reqwest::Client,
    check: &HealthCheck,
) -> (HealthState, Option<String>) {
    match check {
        HealthCheck::None => (HealthState::Ok, None),
        HealthCheck::Tcp { port, timeout } => {
            match tokio::time::timeout(*timeout, TcpStream::connect(("127.0.0.1", *port))).await {
                Ok(Ok(_)) => (HealthState::Ok, None),
                Ok(Err(error)) => (HealthState::Error, Some(error.to_string())),
                Err(_) => (HealthState::Timeout, Some("timeout de conexão".to_owned())),
            }
        }
        HealthCheck::Http { url, timeout } => {
            match client.get(url).timeout(*timeout).send().await {
                Ok(response) if response.status().is_success() => (HealthState::Ok, None),
                Ok(response) => (
                    HealthState::Error,
                    Some(format!("HTTP {}", response.status())),
                ),
                Err(error) if error.is_timeout() => {
                    (HealthState::Timeout, Some("timeout HTTP".to_owned()))
                }
                Err(error) => (HealthState::Error, Some(error.to_string())),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn none_is_always_ok() {
        let client = reqwest::Client::new();
        assert_eq!(
            check_health(&client, &HealthCheck::None).await.0,
            HealthState::Ok
        );
    }

    #[tokio::test]
    async fn tcp_detects_an_open_port() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("listener");
        let port = listener.local_addr().expect("addr").port();
        let client = reqwest::Client::new();
        let (state, _) = check_health(
            &client,
            &HealthCheck::Tcp {
                port,
                timeout: Duration::from_secs(2),
            },
        )
        .await;
        assert_eq!(state, HealthState::Ok);
    }

    #[tokio::test]
    async fn tcp_detects_a_closed_port() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("listener");
        let port = listener.local_addr().expect("addr").port();
        drop(listener);
        let client = reqwest::Client::new();
        let (state, _) = check_health(
            &client,
            &HealthCheck::Tcp {
                port,
                timeout: Duration::from_millis(500),
            },
        )
        .await;
        assert!(matches!(state, HealthState::Error | HealthState::Timeout));
    }
}
