//! Runtime component manifest (LiteLLM/Postgres), signature and policy checks.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SUPPORTED_MANIFEST_SCHEMA: u32 = 1;
pub const LITELLM_VERSION: &str = "1.108.1";
pub const PGSQL_MAJOR: u16 = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComponentManifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub installed_size: Option<u64>,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(default)]
    pub version_args: Vec<String>,
    #[serde(default)]
    pub min_app_version: Option<String>,
}

impl ComponentManifest {
    pub fn effective_installed_size(&self) -> u64 {
        self.installed_size.unwrap_or(self.size)
    }

    pub fn entrypoint(&self) -> Option<&str> {
        self.entrypoint
            .as_deref()
            .filter(|value| !value.trim().is_empty())
    }

    pub fn entrypoint_args(&self) -> Vec<String> {
        if self.version_args.is_empty() {
            vec!["--version".to_owned()]
        } else {
            self.version_args.clone()
        }
    }

    pub fn validate(
        &self,
        component: &'static str,
        app_version: &Version,
    ) -> Result<(), ManifestError> {
        if self.version.trim().is_empty() {
            return Err(ManifestError::InvalidComponent {
                component,
                reason: "versão vazia".to_owned(),
            });
        }
        if !self.url.starts_with("https://") {
            return Err(ManifestError::InvalidComponent {
                component,
                reason: "URL deve usar HTTPS".to_owned(),
            });
        }
        if !is_valid_sha256(&self.sha256) {
            return Err(ManifestError::InvalidComponent {
                component,
                reason: "sha256 inválido".to_owned(),
            });
        }
        if self.size == 0 {
            return Err(ManifestError::InvalidComponent {
                component,
                reason: "tamanho deve ser maior que zero".to_owned(),
            });
        }
        if let Some(installed_size) = self.installed_size {
            if installed_size == 0 {
                return Err(ManifestError::InvalidComponent {
                    component,
                    reason: "installedSize deve ser maior que zero".to_owned(),
                });
            }
        }
        if self.entrypoint().is_none() {
            return Err(ManifestError::InvalidComponent {
                component,
                reason: "entrypoint ausente".to_owned(),
            });
        }
        if let Some(min_app_version) = &self.min_app_version {
            ensure_app_version(app_version, min_app_version)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeManifest {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: Option<String>,
    #[serde(default)]
    pub min_app_version: Option<String>,
    pub litellm: ComponentManifest,
    #[serde(default)]
    pub pgsql: Option<ComponentManifest>,
}

fn default_schema_version() -> u32 {
    SUPPORTED_MANIFEST_SCHEMA
}

impl RuntimeManifest {
    pub fn validate(&self, app_version: &str, policy: &RuntimePolicy) -> Result<(), ManifestError> {
        if self.schema_version != SUPPORTED_MANIFEST_SCHEMA {
            return Err(ManifestError::UnsupportedSchema(self.schema_version));
        }

        let app_version = Version::parse(app_version)
            .map_err(|_| ManifestError::InvalidVersion(app_version.to_owned()))?;

        if let Some(required) = &self.min_app_version {
            ensure_app_version(&app_version, required)?;
        }

        self.litellm.validate("litellm", &app_version)?;
        if let Some(pgsql) = &self.pgsql {
            pgsql.validate("pgsql", &app_version)?;
            let major =
                version_major(&pgsql.version).ok_or_else(|| ManifestError::InvalidComponent {
                    component: "pgsql",
                    reason: "versão sem major reconhecível".to_owned(),
                })?;
            if major != policy.pgsql_major {
                return Err(ManifestError::ComponentNotAllowed {
                    component: "pgsql",
                    version: pgsql.version.clone(),
                });
            }
        }

        if self.litellm.version != policy.litellm_version {
            return Err(ManifestError::ComponentNotAllowed {
                component: "litellm",
                version: self.litellm.version.clone(),
            });
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSignature {
    pub key_id: String,
    pub algorithm: String,
    pub signature: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum KeyStatus {
    #[default]
    Active,
    Retired,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrustedKey {
    pub key_id: String,
    #[serde(default = "default_algorithm")]
    pub algorithm: String,
    pub public_key: String,
    #[serde(default)]
    pub status: KeyStatus,
    #[serde(default)]
    pub not_before: Option<u64>,
    #[serde(default)]
    pub not_after: Option<u64>,
}

impl TrustedKey {
    /// Validity window in Unix epoch seconds. Keys without bounds are valid
    /// while their status is `Active`.
    pub fn is_valid_at(&self, now_epoch_seconds: u64) -> bool {
        if let Some(not_before) = self.not_before {
            if now_epoch_seconds < not_before {
                return false;
            }
        }
        if let Some(not_after) = self.not_after {
            if now_epoch_seconds > not_after {
                return false;
            }
        }
        true
    }
}

fn default_algorithm() -> String {
    "ed25519".to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePolicy {
    pub litellm_version: String,
    pub pgsql_major: u16,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            litellm_version: LITELLM_VERSION.to_owned(),
            pgsql_major: PGSQL_MAJOR,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    SignatureInvalid,
    SignatureEncoding(String),
    UnknownKey(String),
    KeyNotActive(String),
    KeyExpired(String),
    KeyNotYetValid(String),
    UnsupportedAlgorithm(String),
    InvalidManifest(String),
    UnsupportedSchema(u32),
    InvalidVersion(String),
    AppTooOld {
        app: String,
        required: String,
    },
    InvalidComponent {
        component: &'static str,
        reason: String,
    },
    ComponentNotAllowed {
        component: &'static str,
        version: String,
    },
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::SignatureInvalid => f.write_str("assinatura do manifesto inválida"),
            ManifestError::SignatureEncoding(detail) => {
                write!(f, "assinatura do manifesto malformada: {detail}")
            }
            ManifestError::UnknownKey(key_id) => write!(f, "chave desconhecida: {key_id}"),
            ManifestError::KeyNotActive(key_id) => write!(f, "chave não está ativa: {key_id}"),
            ManifestError::KeyExpired(key_id) => write!(f, "chave expirada: {key_id}"),
            ManifestError::KeyNotYetValid(key_id) => {
                write!(f, "chave ainda não é válida: {key_id}")
            }
            ManifestError::UnsupportedAlgorithm(algorithm) => {
                write!(f, "algoritmo de assinatura não suportado: {algorithm}")
            }
            ManifestError::InvalidManifest(detail) => write!(f, "manifesto inválido: {detail}"),
            ManifestError::UnsupportedSchema(schema) => {
                write!(f, "schema de manifesto não suportado: {schema}")
            }
            ManifestError::InvalidVersion(version) => write!(f, "versão inválida: {version}"),
            ManifestError::AppTooOld { app, required } => {
                write!(f, "app {app} é mais antigo que o exigido {required}")
            }
            ManifestError::InvalidComponent { component, reason } => {
                write!(f, "componente {component} inválido: {reason}")
            }
            ManifestError::ComponentNotAllowed { component, version } => {
                write!(
                    f,
                    "componente {component} na versão {version} não é permitido"
                )
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// Verifies a detached Ed25519 signature over the exact manifest bytes.
pub fn verify_signature(
    raw_manifest: &[u8],
    signature: &ManifestSignature,
    trusted_keys: &[TrustedKey],
    now_epoch_seconds: u64,
) -> Result<(), ManifestError> {
    if !signature.algorithm.eq_ignore_ascii_case("ed25519") {
        return Err(ManifestError::UnsupportedAlgorithm(
            signature.algorithm.clone(),
        ));
    }

    let key = trusted_keys
        .iter()
        .find(|candidate| candidate.key_id == signature.key_id)
        .ok_or_else(|| ManifestError::UnknownKey(signature.key_id.clone()))?;

    if key.status != KeyStatus::Active {
        return Err(ManifestError::KeyNotActive(key.key_id.clone()));
    }
    if let Some(not_before) = key.not_before {
        if now_epoch_seconds < not_before {
            return Err(ManifestError::KeyNotYetValid(key.key_id.clone()));
        }
    }
    if let Some(not_after) = key.not_after {
        if now_epoch_seconds > not_after {
            return Err(ManifestError::KeyExpired(key.key_id.clone()));
        }
    }
    if !key.algorithm.eq_ignore_ascii_case("ed25519") {
        return Err(ManifestError::UnsupportedAlgorithm(key.algorithm.clone()));
    }

    let key_bytes = STANDARD
        .decode(key.public_key.trim())
        .map_err(|error| ManifestError::SignatureEncoding(error.to_string()))?;
    let key_array: [u8; 32] = key_bytes.as_slice().try_into().map_err(|_| {
        ManifestError::SignatureEncoding("chave pública deve ter 32 bytes".to_owned())
    })?;
    let verifying_key = VerifyingKey::from_bytes(&key_array)
        .map_err(|error| ManifestError::SignatureEncoding(error.to_string()))?;

    let signature_bytes = STANDARD
        .decode(signature.signature.trim())
        .map_err(|error| ManifestError::SignatureEncoding(error.to_string()))?;
    let signature = Signature::from_slice(&signature_bytes)
        .map_err(|error| ManifestError::SignatureEncoding(error.to_string()))?;

    verifying_key
        .verify(raw_manifest, &signature)
        .map_err(|_| ManifestError::SignatureInvalid)
}

/// Verifies signature, parses the manifest and applies the runtime policy.
pub fn verify_manifest(
    raw_manifest: &[u8],
    signature: &ManifestSignature,
    trusted_keys: &[TrustedKey],
    app_version: &str,
    policy: &RuntimePolicy,
    now_epoch_seconds: u64,
) -> Result<RuntimeManifest, ManifestError> {
    verify_signature(raw_manifest, signature, trusted_keys, now_epoch_seconds)?;
    let manifest: RuntimeManifest = serde_json::from_slice(raw_manifest)
        .map_err(|error| ManifestError::InvalidManifest(error.to_string()))?;
    manifest.validate(app_version, policy)?;
    Ok(manifest)
}

/// Verifies a SHA-256 hex digest in constant-ish time for the given bytes.
pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
    let actual = hex::encode(Sha256::digest(bytes));
    actual.eq_ignore_ascii_case(expected_hex.trim())
}

pub fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64 && hex::decode(value).is_ok()
}

fn ensure_app_version(app_version: &Version, required: &str) -> Result<(), ManifestError> {
    let required =
        Version::parse(required).map_err(|_| ManifestError::InvalidVersion(required.to_owned()))?;
    if app_version < &required {
        return Err(ManifestError::AppTooOld {
            app: app_version.to_string(),
            required: required.to_string(),
        });
    }
    Ok(())
}

fn version_major(version: &str) -> Option<u16> {
    version
        .split('.')
        .next()
        .and_then(|major| major.parse::<u16>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    const NOW: u64 = 1_700_000_000;
    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn test_manifest() -> RuntimeManifest {
        RuntimeManifest {
            schema_version: SUPPORTED_MANIFEST_SCHEMA,
            generated_at: Some("2026-11-06T12:00:00Z".to_owned()),
            min_app_version: Some("0.1.0".to_owned()),
            litellm: ComponentManifest {
                version: LITELLM_VERSION.to_owned(),
                url: "https://example.invalid/litellm-runtime-1.108.1-win-x64.zip".to_owned(),
                sha256: EMPTY_SHA256.to_owned(),
                size: 10,
                installed_size: Some(20),
                entrypoint: Some("litellm.exe".to_owned()),
                version_args: vec!["--version".to_owned()],
                min_app_version: None,
            },
            pgsql: Some(ComponentManifest {
                version: "16.9".to_owned(),
                url: "https://example.invalid/pgsql-16.9-win-x64.zip".to_owned(),
                sha256: EMPTY_SHA256.to_owned(),
                size: 10,
                installed_size: Some(20),
                entrypoint: Some("bin/postgres.exe".to_owned()),
                version_args: vec!["--version".to_owned()],
                min_app_version: None,
            }),
        }
    }

    fn signed(manifest: &RuntimeManifest) -> (Vec<u8>, ManifestSignature, Vec<TrustedKey>) {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let raw = serde_json::to_vec(manifest).expect("serialize");
        let signature = signing_key.sign(&raw);
        let trusted = vec![TrustedKey {
            key_id: "test-2026".to_owned(),
            algorithm: "ed25519".to_owned(),
            public_key: STANDARD.encode(signing_key.verifying_key().to_bytes()),
            status: KeyStatus::Active,
            not_before: None,
            not_after: None,
        }];
        let sig = ManifestSignature {
            key_id: "test-2026".to_owned(),
            algorithm: "ed25519".to_owned(),
            signature: STANDARD.encode(signature.to_bytes()),
        };
        (raw, sig, trusted)
    }

    #[test]
    fn accepts_matching_hash() {
        assert!(verify_sha256(b"", EMPTY_SHA256));
        assert!(verify_sha256(b"", &EMPTY_SHA256.to_uppercase()));
    }

    #[test]
    fn rejects_mismatching_hash() {
        assert!(!verify_sha256(b"tampered", EMPTY_SHA256));
        assert!(!is_valid_sha256("abc"));
    }

    #[test]
    fn parses_manifest_without_new_fields() {
        let json = r#"{
            "litellm": {
                "version": "1.108.1",
                "url": "https://example.invalid/litellm.zip",
                "sha256": "abc",
                "size": 1
            }
        }"#;
        let manifest: RuntimeManifest = serde_json::from_str(json).expect("valid manifest");
        assert_eq!(manifest.litellm.version, "1.108.1");
        assert!(manifest.pgsql.is_none());
        assert_eq!(manifest.litellm.entrypoint_args(), vec!["--version"]);
    }

    #[test]
    fn verifies_a_valid_signature_and_manifest() {
        let manifest = test_manifest();
        let (raw, signature, keys) = signed(&manifest);
        let parsed = verify_manifest(
            &raw,
            &signature,
            &keys,
            "0.1.0",
            &RuntimePolicy::default(),
            NOW,
        )
        .expect("valid");
        assert_eq!(parsed.litellm.version, LITELLM_VERSION);
        assert_eq!(parsed.litellm.entrypoint.as_deref(), Some("litellm.exe"));
    }

    #[test]
    fn rejects_tampered_bytes() {
        let manifest = test_manifest();
        let (mut raw, signature, keys) = signed(&manifest);
        raw[0] ^= 0xff;
        let error = verify_manifest(
            &raw,
            &signature,
            &keys,
            "0.1.0",
            &RuntimePolicy::default(),
            NOW,
        )
        .expect_err("must fail");
        assert_eq!(error, ManifestError::SignatureInvalid);
    }

    #[test]
    fn rejects_unknown_and_revoked_keys() {
        let manifest = test_manifest();
        let (raw, signature, keys) = signed(&manifest);
        let unknown = verify_signature(&raw, &signature, &[], NOW).expect_err("must fail");
        assert_eq!(unknown, ManifestError::UnknownKey("test-2026".to_owned()));

        let mut revoked_key = keys;
        revoked_key[0].status = KeyStatus::Revoked;
        let revoked = verify_signature(&raw, &signature, &revoked_key, NOW).expect_err("must fail");
        assert_eq!(revoked, ManifestError::KeyNotActive("test-2026".to_owned()));
    }

    #[test]
    fn rejects_keys_outside_the_validity_window() {
        let manifest = test_manifest();
        let (raw, signature, keys) = signed(&manifest);

        let mut expired = keys.clone();
        expired[0].not_after = Some(NOW - 1);
        assert_eq!(
            verify_signature(&raw, &signature, &expired, NOW).expect_err("must fail"),
            ManifestError::KeyExpired("test-2026".to_owned())
        );

        let mut future = keys;
        future[0].not_before = Some(NOW + 1);
        assert_eq!(
            verify_signature(&raw, &signature, &future, NOW).expect_err("must fail"),
            ManifestError::KeyNotYetValid("test-2026".to_owned())
        );
    }

    #[test]
    fn rejects_unsupported_algorithm() {
        let manifest = test_manifest();
        let (raw, mut signature, keys) = signed(&manifest);
        signature.algorithm = "rsa".to_owned();
        let error = verify_signature(&raw, &signature, &keys, NOW).expect_err("must fail");
        assert_eq!(error, ManifestError::UnsupportedAlgorithm("rsa".to_owned()));
    }

    #[test]
    fn rejects_newer_schema() {
        let mut manifest = test_manifest();
        manifest.schema_version = 99;
        let (raw, signature, keys) = signed(&manifest);
        let error = verify_manifest(
            &raw,
            &signature,
            &keys,
            "0.1.0",
            &RuntimePolicy::default(),
            NOW,
        )
        .expect_err("must fail");
        assert_eq!(error, ManifestError::UnsupportedSchema(99));
    }

    #[test]
    fn rejects_app_older_than_required() {
        let manifest = test_manifest();
        let (raw, signature, keys) = signed(&manifest);
        let error = verify_manifest(
            &raw,
            &signature,
            &keys,
            "0.0.9",
            &RuntimePolicy::default(),
            NOW,
        )
        .expect_err("must fail");
        assert!(matches!(error, ManifestError::AppTooOld { .. }));
    }

    #[test]
    fn rejects_litellm_version_outside_policy() {
        let mut manifest = test_manifest();
        manifest.litellm.version = "1.107.0".to_owned();
        let (raw, signature, keys) = signed(&manifest);
        let error = verify_manifest(
            &raw,
            &signature,
            &keys,
            "0.1.0",
            &RuntimePolicy::default(),
            NOW,
        )
        .expect_err("must fail");
        assert_eq!(
            error,
            ManifestError::ComponentNotAllowed {
                component: "litellm",
                version: "1.107.0".to_owned()
            }
        );
    }

    #[test]
    fn rejects_pgsql_major_outside_policy() {
        let mut manifest = test_manifest();
        manifest.pgsql = Some(ComponentManifest {
            version: "17.1".to_owned(),
            ..manifest.litellm.clone()
        });
        let (raw, signature, keys) = signed(&manifest);
        let error = verify_manifest(
            &raw,
            &signature,
            &keys,
            "0.1.0",
            &RuntimePolicy::default(),
            NOW,
        )
        .expect_err("must fail");
        assert_eq!(
            error,
            ManifestError::ComponentNotAllowed {
                component: "pgsql",
                version: "17.1".to_owned()
            }
        );
    }

    #[test]
    fn validates_component_fields() {
        let mut manifest = test_manifest();
        manifest.litellm.url = "http://example.invalid/file.zip".to_owned();
        let error = manifest
            .validate("0.1.0", &RuntimePolicy::default())
            .expect_err("must fail");
        assert!(matches!(
            error,
            ManifestError::InvalidComponent {
                component: "litellm",
                ..
            }
        ));

        let mut manifest = test_manifest();
        manifest.litellm.sha256 = "abc".to_owned();
        let error = manifest
            .validate("0.1.0", &RuntimePolicy::default())
            .expect_err("must fail");
        assert!(matches!(
            error,
            ManifestError::InvalidComponent {
                component: "litellm",
                ..
            }
        ));

        let mut manifest = test_manifest();
        manifest.litellm.size = 0;
        let error = manifest
            .validate("0.1.0", &RuntimePolicy::default())
            .expect_err("must fail");
        assert!(matches!(
            error,
            ManifestError::InvalidComponent {
                component: "litellm",
                ..
            }
        ));

        let mut manifest = test_manifest();
        manifest.litellm.entrypoint = None;
        let error = manifest
            .validate("0.1.0", &RuntimePolicy::default())
            .expect_err("must fail");
        assert!(matches!(
            error,
            ManifestError::InvalidComponent {
                component: "litellm",
                ..
            }
        ));
    }

    #[test]
    fn parses_the_ci_fixture() {
        let raw = include_str!("../tests/fixtures/manifest.json");
        let manifest: RuntimeManifest = serde_json::from_str(raw).expect("fixture");
        assert_eq!(manifest.schema_version, SUPPORTED_MANIFEST_SCHEMA);
        assert_eq!(manifest.litellm.version, LITELLM_VERSION);
        assert!(manifest.pgsql.is_some());
    }
}
