//! Stable error contract shared by the Rust backend and the UI.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable, machine-readable error codes. The UI maps these to actions and
/// localized messages without parsing free-form text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorCode {
    UnsupportedPlatform,
    DataDirNotWritable,
    DiskSpace,
    Network,
    Timeout,
    Cancelled,
    ManifestSignature,
    ManifestSchema,
    AppTooOld,
    ComponentNotAllowed,
    HashMismatch,
    SizeMismatch,
    RangeInvalid,
    ZipUnsafe,
    Extraction,
    VersionProbe,
    VersionMismatch,
    Activation,
    Rollback,
    PortInUse,
    ProcessSpawn,
    ProcessExited,
    HealthTimeout,
    ProviderAuth,
    ProviderForbidden,
    ProviderModelNotFound,
    ProviderRateLimited,
    ProviderBadRequest,
    ProviderUnavailable,
    CopilotAuthPending,
    CopilotAuthExpired,
    CopilotPlanMissing,
    CopilotDatabase,
    CaGeneration,
    CaTrust,
    CaRemove,
    EnvRead,
    EnvWrite,
    EnvRestore,
    FigmaNotFound,
    FigmaRunning,
    FigmaRestart,
    ProxyTls,
    ProxyRewrite,
    ProxyUpstream,
    ConfigInvalid,
    ConfigApply,
    StateCorrupt,
    StateSchema,
    StepOrder,
    StepNotStarted,
    StepCannotSkip,
    Internal,
}

/// Error returned by Tauri commands. It must never contain secrets, prompts or
/// response bodies; `detail` is meant for sanitized diagnostics only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UiError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default)]
    pub retryable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

impl UiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            detail: None,
            retryable: false,
            action: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn retryable(mut self) -> Self {
        self.retryable = true;
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }
}

impl fmt::Display for UiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for UiError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_camel_case_contract() {
        let error = UiError::new(ErrorCode::HashMismatch, "hash inválido")
            .with_detail("manifesto rejeitado")
            .with_action("retry_download")
            .retryable();

        let json = serde_json::to_value(&error).expect("serializable");
        assert_eq!(json["code"], "hashMismatch");
        assert_eq!(json["message"], "hash inválido");
        assert_eq!(json["detail"], "manifesto rejeitado");
        assert_eq!(json["action"], "retry_download");
        assert_eq!(json["retryable"], true);
    }

    #[test]
    fn omits_optional_fields_when_absent() {
        let json = serde_json::to_value(UiError::internal("falhou")).expect("serializable");
        assert!(json.get("detail").is_none());
        assert!(json.get("action").is_none());
    }

    #[test]
    fn display_uses_the_user_message() {
        let error = UiError::new(ErrorCode::Network, "sem internet");
        assert_eq!(error.to_string(), "sem internet");
    }
}
