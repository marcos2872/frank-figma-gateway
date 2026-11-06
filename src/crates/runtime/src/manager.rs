//! Facade used by the Tauri layer: manifest, download and install.

use ffg_core::download::ComponentId;
use ffg_core::install::{ActiveRuntime, InstallPlan};
use ffg_core::manifest::{
    verify_manifest, ComponentManifest, ManifestSignature, RuntimeManifest, RuntimePolicy,
    TrustedKey,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

use crate::downloader::{Downloader, ProgressCallback};
use crate::error::RuntimeError;
use crate::install::{Installation, Installer};

const MANIFEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct RuntimeManager {
    root: PathBuf,
    keys: Vec<TrustedKey>,
    policy: RuntimePolicy,
    app_version: String,
    downloader: Downloader,
    installer: Installer,
    client: reqwest::Client,
}

impl RuntimeManager {
    pub fn new(
        root: PathBuf,
        keys: Vec<TrustedKey>,
        policy: RuntimePolicy,
        app_version: String,
    ) -> Result<Self, RuntimeError> {
        let downloader = Downloader::new(root.join("cache").join("downloads"));
        let installer = Installer::new(root.clone());
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|error| RuntimeError::Network(error.to_string()))?;
        Ok(Self {
            root,
            keys,
            policy,
            app_version,
            downloader,
            installer,
            client,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn active(&self) -> Result<ActiveRuntime, RuntimeError> {
        self.installer.active()
    }

    /// Fetches and verifies `manifest.json` + `manifest.json.sig`.
    ///
    /// Both sources may be `https://`, `http://` (loopback only) or a local file
    /// path, which keeps the offline mode working without a separate code path.
    pub async fn fetch_manifest(
        &self,
        manifest_url: &str,
        signature_url: &str,
    ) -> Result<RuntimeManifest, RuntimeError> {
        let raw = self.read_source(manifest_url).await?;
        let signature_raw = self.read_source(signature_url).await?;
        let signature: ManifestSignature =
            serde_json::from_slice(&signature_raw).map_err(|error| {
                RuntimeError::Manifest(ffg_core::manifest::ManifestError::SignatureEncoding(
                    error.to_string(),
                ))
            })?;
        self.verify_manifest_bytes(&raw, &signature)
    }

    /// Verifies already-loaded manifest bytes (offline import path).
    pub fn verify_manifest_bytes(
        &self,
        raw: &[u8],
        signature: &ManifestSignature,
    ) -> Result<RuntimeManifest, RuntimeError> {
        let manifest = verify_manifest(
            raw,
            signature,
            &self.keys,
            &self.app_version,
            &self.policy,
            now_epoch_seconds(),
        )?;
        Ok(manifest)
    }

    pub fn component<'a>(
        &self,
        manifest: &'a RuntimeManifest,
        component: ComponentId,
    ) -> Result<&'a ComponentManifest, RuntimeError> {
        match component {
            ComponentId::Litellm => Ok(&manifest.litellm),
            ComponentId::Pgsql => manifest
                .pgsql
                .as_ref()
                .ok_or(RuntimeError::ComponentMissing(component)),
        }
    }

    pub async fn download(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        mirrors: Vec<String>,
        progress: Option<ProgressCallback>,
        cancel: Option<CancellationToken>,
    ) -> Result<PathBuf, RuntimeError> {
        self.downloader
            .download(component, manifest, mirrors, progress, cancel)
            .await
    }

    pub async fn install(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        zip_path: &Path,
    ) -> Result<Installation, RuntimeError> {
        self.installer.install(component, manifest, zip_path).await
    }

    /// Plan that would be used for an installation; exposed for diagnostics.
    pub fn plan(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        staging_id: &str,
    ) -> Result<InstallPlan, RuntimeError> {
        Ok(InstallPlan::new(component, manifest, staging_id)?)
    }

    async fn read_source(&self, source: &str) -> Result<Vec<u8>, RuntimeError> {
        if source.starts_with("https://") || source.starts_with("http://") {
            let response = tokio::time::timeout(MANIFEST_TIMEOUT, self.client.get(source).send())
                .await
                .map_err(|_| RuntimeError::Timeout(source.to_owned()))?
                .map_err(|error| RuntimeError::Network(error.to_string()))?;
            if !response.status().is_success() {
                return Err(RuntimeError::Http(response.status().as_u16()));
            }
            let bytes = tokio::time::timeout(MANIFEST_TIMEOUT, response.bytes())
                .await
                .map_err(|_| RuntimeError::Timeout(source.to_owned()))?
                .map_err(|error| RuntimeError::Network(error.to_string()))?;
            Ok(bytes.to_vec())
        } else {
            let path = source.strip_prefix("file://").unwrap_or(source);
            tokio::fs::read(path).await.map_err(RuntimeError::Io)
        }
    }
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use base64::Engine as _;
    use ed25519_dalek::{Signer, SigningKey};
    use ffg_core::manifest::{ComponentManifest, ManifestError, LITELLM_VERSION};
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn manifest() -> RuntimeManifest {
        RuntimeManifest {
            schema_version: 1,
            generated_at: Some("2026-11-06T12:00:00Z".to_owned()),
            min_app_version: Some("0.1.0".to_owned()),
            litellm: ComponentManifest {
                version: LITELLM_VERSION.to_owned(),
                url: "https://example.invalid/litellm.zip".to_owned(),
                sha256: EMPTY_SHA256.to_owned(),
                size: 10,
                installed_size: Some(20),
                entrypoint: Some("litellm.exe".to_owned()),
                version_args: vec!["--version".to_owned()],
                min_app_version: None,
            },
            pgsql: None,
        }
    }

    fn signed() -> (Vec<u8>, Vec<u8>, Vec<TrustedKey>) {
        let signing_key = SigningKey::from_bytes(&[3u8; 32]);
        let raw = serde_json::to_vec(&manifest()).expect("serialize");
        let signature = signing_key.sign(&raw);
        let signature_json = serde_json::to_vec(&json!({
            "keyId": "test",
            "algorithm": "ed25519",
            "signature": STANDARD.encode(signature.to_bytes()),
        }))
        .expect("signature json");
        let keys = vec![TrustedKey {
            key_id: "test".to_owned(),
            algorithm: "ed25519".to_owned(),
            public_key: STANDARD.encode(signing_key.verifying_key().to_bytes()),
            status: ffg_core::manifest::KeyStatus::Active,
            not_before: None,
            not_after: None,
        }];
        (raw, signature_json, keys)
    }

    fn manager(root: PathBuf, keys: Vec<TrustedKey>) -> RuntimeManager {
        RuntimeManager::new(root, keys, RuntimePolicy::default(), "0.1.0".to_owned())
            .expect("manager")
    }

    #[tokio::test]
    async fn fetches_and_verifies_a_remote_manifest() {
        let server = MockServer::start().await;
        let (raw, signature, keys) = signed();
        Mock::given(method("GET"))
            .and(path("/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(raw))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/manifest.json.sig"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(signature))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager(dir.path().to_path_buf(), keys);
        let manifest = manager
            .fetch_manifest(
                &format!("{}/manifest.json", server.uri()),
                &format!("{}/manifest.json.sig", server.uri()),
            )
            .await
            .expect("manifest");
        assert_eq!(manifest.litellm.version, LITELLM_VERSION);
    }

    #[tokio::test]
    async fn fetches_a_local_manifest_for_offline_mode() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (raw, signature, keys) = signed();
        let manifest_path = dir.path().join("manifest.json");
        let signature_path = dir.path().join("manifest.json.sig");
        std::fs::write(&manifest_path, &raw).expect("write manifest");
        std::fs::write(&signature_path, &signature).expect("write signature");

        let manager = manager(dir.path().to_path_buf(), keys);
        let manifest = manager
            .fetch_manifest(
                manifest_path.to_str().expect("path"),
                signature_path.to_str().expect("path"),
            )
            .await
            .expect("manifest");
        assert_eq!(manifest.litellm.version, LITELLM_VERSION);
    }

    #[tokio::test]
    async fn rejects_a_tampered_manifest() {
        let server = MockServer::start().await;
        let (_raw, signature, keys) = signed();
        Mock::given(method("GET"))
            .and(path("/manifest.json"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"{}".to_vec()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/manifest.json.sig"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(signature))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager(dir.path().to_path_buf(), keys);
        let error = manager
            .fetch_manifest(
                &format!("{}/manifest.json", server.uri()),
                &format!("{}/manifest.json.sig", server.uri()),
            )
            .await
            .expect_err("must fail");
        assert!(matches!(
            error,
            RuntimeError::Manifest(ManifestError::SignatureInvalid)
        ));
    }

    #[tokio::test]
    async fn reports_http_failures_for_the_manifest() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/manifest.json"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager(dir.path().to_path_buf(), vec![]);
        let error = manager
            .fetch_manifest(
                &format!("{}/manifest.json", server.uri()),
                &format!("{}/manifest.json.sig", server.uri()),
            )
            .await
            .expect_err("must fail");
        assert!(matches!(error, RuntimeError::Http(404)));
    }

    #[test]
    fn resolves_the_component_from_the_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager(dir.path().to_path_buf(), vec![]);
        let manifest = manifest();
        assert_eq!(
            manager
                .component(&manifest, ComponentId::Litellm)
                .expect("litellm")
                .version,
            LITELLM_VERSION
        );
        assert!(matches!(
            manager.component(&manifest, ComponentId::Pgsql),
            Err(RuntimeError::ComponentMissing(ComponentId::Pgsql))
        ));
    }

    #[test]
    fn builds_an_install_plan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manager = manager(dir.path().to_path_buf(), vec![]);
        let manifest = manifest();
        let plan = manager
            .plan(ComponentId::Litellm, &manifest.litellm, "abc123")
            .expect("plan");
        assert_eq!(plan.target_path, "runtime/litellm/1.108.1");
    }
}
