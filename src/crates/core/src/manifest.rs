//! Runtime component manifest (LiteLLM/Postgres) and integrity checks.

use serde::Deserialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ComponentManifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub min_app_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct RuntimeManifest {
    pub litellm: ComponentManifest,
    #[serde(default)]
    pub pgsql: Option<ComponentManifest>,
}

/// Verifies a SHA-256 hex digest in constant-ish time for the given bytes.
pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> bool {
    let actual = hex::encode(Sha256::digest(bytes));
    actual.eq_ignore_ascii_case(expected_hex.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn accepts_matching_hash() {
        assert!(verify_sha256(b"", EMPTY_SHA256));
        assert!(verify_sha256(b"", &EMPTY_SHA256.to_uppercase()));
    }

    #[test]
    fn rejects_mismatching_hash() {
        assert!(!verify_sha256(b"tampered", EMPTY_SHA256));
    }

    #[test]
    fn parses_manifest() {
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
    }
}
