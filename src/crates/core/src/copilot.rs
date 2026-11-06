//! Pure GitHub Copilot device-flow and error mapping.
//!
//! The HTTP calls and the LiteLLM management API live in `src-tauri`; this
//! module only decides when to poll and how to interpret statuses.

use crate::setup::{AuthStatus, CopilotState};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MIN_POLL_INTERVAL_SECONDS: u64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFlow {
    pub verification_uri: String,
    pub user_code: String,
    pub expires_in_seconds: u64,
    pub interval_seconds: u64,
    pub started_at_epoch_seconds: u64,
}

impl DeviceFlow {
    pub fn new(
        verification_uri: impl Into<String>,
        user_code: impl Into<String>,
        expires_in_seconds: u64,
        interval_seconds: u64,
        started_at_epoch_seconds: u64,
    ) -> Result<Self, CopilotError> {
        let verification_uri = verification_uri.into();
        if !verification_uri.starts_with("https://") {
            return Err(CopilotError::InvalidDeviceFlow(
                "verification_uri deve usar HTTPS".to_owned(),
            ));
        }
        let user_code = user_code.into();
        if user_code.trim().is_empty() {
            return Err(CopilotError::InvalidDeviceFlow(
                "user_code vazio".to_owned(),
            ));
        }
        if expires_in_seconds == 0 {
            return Err(CopilotError::InvalidDeviceFlow(
                "expiração deve ser maior que zero".to_owned(),
            ));
        }
        if interval_seconds < MIN_POLL_INTERVAL_SECONDS {
            return Err(CopilotError::InvalidDeviceFlow(format!(
                "intervalo mínimo é {MIN_POLL_INTERVAL_SECONDS}s"
            )));
        }

        Ok(Self {
            verification_uri,
            user_code,
            expires_in_seconds,
            interval_seconds,
            started_at_epoch_seconds,
        })
    }

    pub fn expires_at_epoch_seconds(&self) -> u64 {
        self.started_at_epoch_seconds
            .saturating_add(self.expires_in_seconds)
    }

    pub fn is_expired(&self, now_epoch_seconds: u64) -> bool {
        now_epoch_seconds >= self.expires_at_epoch_seconds()
    }

    pub fn poll_decision(
        &self,
        now_epoch_seconds: u64,
        last_poll_epoch_seconds: Option<u64>,
    ) -> PollDecision {
        if self.is_expired(now_epoch_seconds) {
            return PollDecision::Expired;
        }

        match last_poll_epoch_seconds {
            Some(last_poll)
                if now_epoch_seconds < last_poll.saturating_add(self.interval_seconds) =>
            {
                PollDecision::Wait {
                    retry_at_epoch_seconds: last_poll.saturating_add(self.interval_seconds),
                }
            }
            _ => PollDecision::Ready,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum PollDecision {
    Ready,
    Wait { retry_at_epoch_seconds: u64 },
    Expired,
}

pub fn apply_auth_status(
    copilot: &mut CopilotState,
    status: AuthStatus,
    github_login: Option<String>,
) -> Result<(), CopilotError> {
    match status {
        AuthStatus::Connected => {
            let login = github_login
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    CopilotError::InvalidDeviceFlow("login do GitHub ausente".to_owned())
                })?;
            copilot.connected = true;
            copilot.github_login = Some(login.trim().to_owned());
        }
        AuthStatus::NotStarted
        | AuthStatus::WaitingForUser
        | AuthStatus::Expired
        | AuthStatus::Failed => {
            copilot.connected = false;
            if status != AuthStatus::WaitingForUser {
                copilot.github_login = None;
            }
        }
    }
    copilot.auth_status = status;
    Ok(())
}

pub fn map_http_error(status: u16) -> CopilotError {
    match status {
        401 | 403 => CopilotError::AuthExpired,
        404 => CopilotError::PlanMissing,
        429 => CopilotError::RateLimited,
        500..=599 => CopilotError::Transient,
        _ => CopilotError::Fatal(format!("status HTTP {status}")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopilotError {
    AuthExpired,
    PlanMissing,
    RateLimited,
    Transient,
    Fatal(String),
    InvalidDeviceFlow(String),
}

impl fmt::Display for CopilotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CopilotError::AuthExpired => {
                f.write_str("autorização do GitHub expirou; conecte novamente")
            }
            CopilotError::PlanMissing => {
                f.write_str("o modelo não está disponível no seu plano do Copilot")
            }
            CopilotError::RateLimited => f.write_str("limite de requisições do Copilot atingido"),
            CopilotError::Transient => f.write_str("falha temporária no Copilot; tente de novo"),
            CopilotError::Fatal(detail) => write!(f, "falha no Copilot: {detail}"),
            CopilotError::InvalidDeviceFlow(detail) => {
                write!(f, "device flow inválido: {detail}")
            }
        }
    }
}

impl std::error::Error for CopilotError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow() -> DeviceFlow {
        DeviceFlow::new(
            "https://github.com/login/device",
            "ABCD-1234",
            600,
            5,
            1_000,
        )
        .expect("flow")
    }

    #[test]
    fn validates_device_flow_input() {
        assert!(DeviceFlow::new("http://github.com/login/device", "CODE", 600, 5, 0).is_err());
        assert!(DeviceFlow::new("https://github.com/login/device", " ", 600, 5, 0).is_err());
        assert!(DeviceFlow::new("https://github.com/login/device", "CODE", 0, 5, 0).is_err());
        assert!(DeviceFlow::new("https://github.com/login/device", "CODE", 600, 1, 0).is_err());
    }

    #[test]
    fn polls_immediately_then_waits_for_the_interval() {
        let flow = flow();
        assert_eq!(flow.poll_decision(1_000, None), PollDecision::Ready);
        assert_eq!(
            flow.poll_decision(1_003, Some(1_000)),
            PollDecision::Wait {
                retry_at_epoch_seconds: 1_005
            }
        );
        assert_eq!(flow.poll_decision(1_005, Some(1_000)), PollDecision::Ready);
    }

    #[test]
    fn expires_at_the_deadline() {
        let flow = flow();
        assert!(!flow.is_expired(1_599));
        assert!(flow.is_expired(1_600));
        assert_eq!(flow.poll_decision(1_600, None), PollDecision::Expired);
    }

    #[test]
    fn applies_connected_and_waiting_states() {
        let mut copilot = CopilotState::default();
        apply_auth_status(
            &mut copilot,
            AuthStatus::Connected,
            Some("octocat".to_owned()),
        )
        .expect("connected");
        assert!(copilot.connected);
        assert_eq!(copilot.github_login.as_deref(), Some("octocat"));
        assert_eq!(copilot.auth_status, AuthStatus::Connected);

        apply_auth_status(&mut copilot, AuthStatus::WaitingForUser, None).expect("waiting");
        assert!(!copilot.connected);
        assert_eq!(copilot.github_login.as_deref(), Some("octocat"));

        apply_auth_status(&mut copilot, AuthStatus::Expired, None).expect("expired");
        assert!(!copilot.connected);
        assert!(copilot.github_login.is_none());
    }

    #[test]
    fn connected_requires_a_login() {
        let mut copilot = CopilotState::default();
        assert!(matches!(
            apply_auth_status(&mut copilot, AuthStatus::Connected, None),
            Err(CopilotError::InvalidDeviceFlow(_))
        ));
    }

    #[test]
    fn maps_http_errors_to_actionable_states() {
        assert_eq!(map_http_error(401), CopilotError::AuthExpired);
        assert_eq!(map_http_error(403), CopilotError::AuthExpired);
        assert_eq!(map_http_error(404), CopilotError::PlanMissing);
        assert_eq!(map_http_error(429), CopilotError::RateLimited);
        assert_eq!(map_http_error(503), CopilotError::Transient);
        assert!(matches!(map_http_error(400), CopilotError::Fatal(_)));
    }
}
