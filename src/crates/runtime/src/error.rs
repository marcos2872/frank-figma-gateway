//! Error type returned by the runtime pipeline.
//!
//! Every failure is mapped to a stable [`UiError`] code so the UI can offer the
//! right action without parsing free-form text. Details are sanitized: no URL
//! with userinfo, no secret and no archive content is ever included.

use ffg_core::download::{ComponentId, DownloadError};
use ffg_core::error::{ErrorCode, UiError};
use ffg_core::install::InstallError;
use ffg_core::manifest::ManifestError;
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum RuntimeError {
    Io(io::Error),
    Http(u16),
    Network(String),
    Timeout(String),
    Cancelled,
    Manifest(ManifestError),
    Download(DownloadError),
    Install(InstallError),
    ComponentMissing(ComponentId),
    HashMismatch { expected: String, actual: String },
    SizeMismatch { expected: u64, actual: u64 },
    RangeInvalid(String),
    Zip(String),
    Extraction(String),
    VersionProbe(String),
    VersionMismatch { expected: String, output: String },
    Activation(String),
    State(String),
}

impl RuntimeError {
    pub fn to_ui_error(&self) -> UiError {
        let (code, message) = match self {
            RuntimeError::Io(error) => (
                ErrorCode::Internal,
                format!("Falha de disco ao preparar o runtime: {error}"),
            ),
            RuntimeError::Http(status) => (
                ErrorCode::Network,
                format!("O servidor respondeu com HTTP {status} ao baixar o runtime."),
            ),
            RuntimeError::Network(detail) => (
                ErrorCode::Network,
                format!("Não foi possível baixar o runtime: {detail}"),
            ),
            RuntimeError::Timeout(detail) => (
                ErrorCode::Timeout,
                format!("O download do runtime excedeu o tempo limite: {detail}"),
            ),
            RuntimeError::Cancelled => (
                ErrorCode::Cancelled,
                "O download do runtime foi cancelado.".to_owned(),
            ),
            RuntimeError::Manifest(error) => {
                (manifest_code(error), format!("Manifesto recusado: {error}"))
            }
            RuntimeError::Download(error) => (
                ErrorCode::Network,
                format!("Plano de download inválido: {error}"),
            ),
            RuntimeError::Install(error) => (
                ErrorCode::ZipUnsafe,
                format!("Pacote de runtime recusado: {error}"),
            ),
            RuntimeError::ComponentMissing(component) => (
                ErrorCode::ComponentNotAllowed,
                format!("O manifesto não traz o componente {}.", component.as_str()),
            ),
            RuntimeError::HashMismatch { .. } => (
                ErrorCode::HashMismatch,
                "O SHA-256 do pacote baixado não confere com o manifesto.".to_owned(),
            ),
            RuntimeError::SizeMismatch { expected, actual } => (
                ErrorCode::SizeMismatch,
                format!("O pacote tem {actual} bytes; o manifesto espera {expected}."),
            ),
            RuntimeError::RangeInvalid(detail) => (
                ErrorCode::RangeInvalid,
                format!("Resposta de retomada inválida: {detail}"),
            ),
            RuntimeError::Zip(detail) => (
                ErrorCode::ZipUnsafe,
                format!("ZIP inválido ou corrompido: {detail}"),
            ),
            RuntimeError::Extraction(detail) => (
                ErrorCode::Extraction,
                format!("Falha ao extrair o pacote: {detail}"),
            ),
            RuntimeError::VersionProbe(detail) => (
                ErrorCode::VersionProbe,
                format!("Não foi possível executar o runtime instalado: {detail}"),
            ),
            RuntimeError::VersionMismatch { expected, output } => (
                ErrorCode::VersionMismatch,
                format!("O runtime instalado não reportou a versão {expected} (saída: {output})."),
            ),
            RuntimeError::Activation(detail) => (
                ErrorCode::Activation,
                format!("Não foi possível ativar o runtime: {detail}"),
            ),
            RuntimeError::State(detail) => (
                ErrorCode::Internal,
                format!("Estado do runtime inválido: {detail}"),
            ),
        };

        let mut ui = UiError::new(code, message);
        if self.is_retryable() {
            ui = ui.retryable();
        }
        match self.retry_action() {
            Some(action) => ui.with_action(action),
            None => ui,
        }
    }

    fn is_retryable(&self) -> bool {
        matches!(
            self,
            RuntimeError::Http(_)
                | RuntimeError::Network(_)
                | RuntimeError::Timeout(_)
                | RuntimeError::RangeInvalid(_)
                | RuntimeError::SizeMismatch { .. }
                | RuntimeError::Io(_)
        )
    }

    fn retry_action(&self) -> Option<&'static str> {
        match self {
            RuntimeError::HashMismatch { .. } => Some("retry_download"),
            RuntimeError::Http(_) | RuntimeError::Network(_) | RuntimeError::Timeout(_) => {
                Some("retry_download")
            }
            RuntimeError::VersionProbe(_) | RuntimeError::VersionMismatch { .. } => {
                Some("reinstall_runtime")
            }
            _ => None,
        }
    }
}

fn manifest_code(error: &ManifestError) -> ErrorCode {
    match error {
        ManifestError::SignatureInvalid
        | ManifestError::SignatureEncoding(_)
        | ManifestError::UnknownKey(_)
        | ManifestError::KeyNotActive(_)
        | ManifestError::KeyExpired(_)
        | ManifestError::KeyNotYetValid(_)
        | ManifestError::UnsupportedAlgorithm(_) => ErrorCode::ManifestSignature,
        ManifestError::AppTooOld { .. } => ErrorCode::AppTooOld,
        ManifestError::ComponentNotAllowed { .. } => ErrorCode::ComponentNotAllowed,
        ManifestError::InvalidManifest(_)
        | ManifestError::UnsupportedSchema(_)
        | ManifestError::InvalidVersion(_)
        | ManifestError::InvalidComponent { .. } => ErrorCode::ManifestSchema,
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::Io(error) => write!(f, "falha de I/O: {error}"),
            RuntimeError::Http(status) => write!(f, "HTTP {status}"),
            RuntimeError::Network(detail) => write!(f, "rede: {detail}"),
            RuntimeError::Timeout(detail) => write!(f, "tempo limite: {detail}"),
            RuntimeError::Cancelled => f.write_str("operação cancelada"),
            RuntimeError::Manifest(error) => write!(f, "manifesto: {error}"),
            RuntimeError::Download(error) => write!(f, "download: {error}"),
            RuntimeError::Install(error) => write!(f, "instalação: {error}"),
            RuntimeError::ComponentMissing(component) => {
                write!(f, "componente ausente no manifesto: {}", component.as_str())
            }
            RuntimeError::HashMismatch { expected, actual } => {
                write!(
                    f,
                    "sha256 divergente (esperado {expected}, obtido {actual})"
                )
            }
            RuntimeError::SizeMismatch { expected, actual } => {
                write!(
                    f,
                    "tamanho divergente (esperado {expected}, obtido {actual})"
                )
            }
            RuntimeError::RangeInvalid(detail) => write!(f, "range inválido: {detail}"),
            RuntimeError::Zip(detail) => write!(f, "zip: {detail}"),
            RuntimeError::Extraction(detail) => write!(f, "extração: {detail}"),
            RuntimeError::VersionProbe(detail) => write!(f, "probe de versão: {detail}"),
            RuntimeError::VersionMismatch { expected, output } => {
                write!(f, "versão divergente (esperado {expected}, saída {output})")
            }
            RuntimeError::Activation(detail) => write!(f, "ativação: {detail}"),
            RuntimeError::State(detail) => write!(f, "estado: {detail}"),
        }
    }
}

impl std::error::Error for RuntimeError {}

impl From<io::Error> for RuntimeError {
    fn from(error: io::Error) -> Self {
        RuntimeError::Io(error)
    }
}

impl From<ManifestError> for RuntimeError {
    fn from(error: ManifestError) -> Self {
        RuntimeError::Manifest(error)
    }
}

impl From<DownloadError> for RuntimeError {
    fn from(error: DownloadError) -> Self {
        RuntimeError::Download(error)
    }
}

impl From<InstallError> for RuntimeError {
    fn from(error: InstallError) -> Self {
        RuntimeError::Install(error)
    }
}

impl From<RuntimeError> for UiError {
    fn from(error: RuntimeError) -> Self {
        error.to_ui_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_manifest_errors_to_stable_codes() {
        let signature = RuntimeError::Manifest(ManifestError::SignatureInvalid);
        assert_eq!(signature.to_ui_error().code, ErrorCode::ManifestSignature);

        let schema = RuntimeError::Manifest(ManifestError::UnsupportedSchema(9));
        assert_eq!(schema.to_ui_error().code, ErrorCode::ManifestSchema);

        let old = RuntimeError::Manifest(ManifestError::AppTooOld {
            app: "0.0.1".to_owned(),
            required: "0.1.0".to_owned(),
        });
        assert_eq!(old.to_ui_error().code, ErrorCode::AppTooOld);
    }

    #[test]
    fn network_errors_are_retryable_with_an_action() {
        let ui = RuntimeError::Http(503).to_ui_error();
        assert_eq!(ui.code, ErrorCode::Network);
        assert!(ui.retryable);
        assert_eq!(ui.action.as_deref(), Some("retry_download"));
    }

    #[test]
    fn hash_mismatch_offers_a_retry_but_is_not_marked_retryable() {
        let ui = RuntimeError::HashMismatch {
            expected: "a".repeat(64),
            actual: "b".repeat(64),
        }
        .to_ui_error();
        assert_eq!(ui.code, ErrorCode::HashMismatch);
        assert!(!ui.retryable);
        assert_eq!(ui.action.as_deref(), Some("retry_download"));
    }

    #[test]
    fn cancellation_and_state_have_no_action() {
        assert_eq!(
            RuntimeError::Cancelled.to_ui_error().code,
            ErrorCode::Cancelled
        );
        let state = RuntimeError::State("active.json ilegível".to_owned()).to_ui_error();
        assert_eq!(state.code, ErrorCode::Internal);
        assert!(state.action.is_none());
    }

    #[test]
    fn display_never_includes_secrets() {
        let error = RuntimeError::Network("timeout ao conectar".to_owned());
        assert_eq!(error.to_string(), "rede: timeout ao conectar");
    }
}
