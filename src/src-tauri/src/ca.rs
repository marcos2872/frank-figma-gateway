//! Local CA lifecycle: generation, protected key storage and trust store.

use crate::paths::RuntimePaths;
use crate::platform::{CertStore, FileProtector, PlatformError};
use ffg_core::error::{ErrorCode, UiError};
use ffg_proxy::ca::{thumbprint_from_pem, CaError, CertificateAuthority};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaStatus {
    pub generated: bool,
    pub installed: bool,
    pub thumbprint: Option<String>,
    pub cert_path: Option<String>,
}

pub struct CaManager {
    paths: RuntimePaths,
    cert_store: Arc<dyn CertStore>,
    protector: Arc<dyn FileProtector>,
    cache: Mutex<Option<Arc<CertificateAuthority>>>,
}

impl CaManager {
    pub fn new(
        paths: RuntimePaths,
        cert_store: Arc<dyn CertStore>,
        protector: Arc<dyn FileProtector>,
    ) -> Self {
        Self {
            paths,
            cert_store,
            protector,
            cache: Mutex::new(None),
        }
    }

    pub fn cert_path(&self) -> PathBuf {
        self.paths.ca_dir().join("root.pem")
    }

    pub fn key_path(&self) -> PathBuf {
        self.paths.ca_dir().join("root.key.dpapi")
    }

    pub fn authority(&self) -> Result<Arc<CertificateAuthority>, CaManagerError> {
        if let Some(cached) = self
            .cache
            .lock()
            .map_err(|_| CaManagerError::State("cache da CA envenenado".to_owned()))?
            .clone()
        {
            return Ok(cached);
        }

        let authority = match self.load()? {
            Some(authority) => authority,
            None => {
                let generated = Arc::new(CertificateAuthority::generate()?);
                self.save(&generated)?;
                generated
            }
        };

        *self
            .cache
            .lock()
            .map_err(|_| CaManagerError::State("cache da CA envenenado".to_owned()))? =
            Some(authority.clone());
        Ok(authority)
    }

    pub fn status(&self) -> Result<CaStatus, CaManagerError> {
        let cert_path = self.cert_path();
        if !cert_path.is_file() {
            return Ok(CaStatus {
                generated: false,
                installed: false,
                thumbprint: None,
                cert_path: None,
            });
        }

        let thumbprint = match self.load()? {
            Some(authority) => authority.thumbprint(),
            None => {
                let pem = fs::read_to_string(&cert_path)?;
                thumbprint_from_pem(&pem)?
            }
        };
        let installed = self.cert_store.is_installed(&thumbprint)?;
        Ok(CaStatus {
            generated: true,
            installed,
            thumbprint: Some(thumbprint),
            cert_path: Some(cert_path.display().to_string()),
        })
    }

    pub fn install(&self) -> Result<CaStatus, CaManagerError> {
        let authority = self.authority()?;
        self.cert_store
            .install(&authority.thumbprint(), authority.cert_der())?;
        self.status()
    }

    pub fn remove(&self) -> Result<CaStatus, CaManagerError> {
        let cert_path = self.cert_path();
        if cert_path.is_file() {
            let pem = fs::read_to_string(&cert_path)?;
            let thumbprint = thumbprint_from_pem(&pem)?;
            self.cert_store.remove(&thumbprint)?;
        }
        self.status()
    }

    pub fn export(&self, path: &Path) -> Result<PathBuf, CaManagerError> {
        let authority = self.authority()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, authority.cert_pem())?;
        Ok(path.to_path_buf())
    }

    fn load(&self) -> Result<Option<Arc<CertificateAuthority>>, CaManagerError> {
        let cert_path = self.cert_path();
        let key_path = self.key_path();
        if !cert_path.is_file() || !key_path.is_file() {
            return Ok(None);
        }

        let cert_pem = fs::read_to_string(cert_path)?;
        let protected = fs::read(key_path)?;
        let key_bytes = self.protector.unprotect(&protected)?;
        let key_pem = String::from_utf8(key_bytes)?;
        Ok(Some(Arc::new(CertificateAuthority::load(
            &cert_pem, &key_pem,
        )?)))
    }

    fn save(&self, authority: &CertificateAuthority) -> Result<(), CaManagerError> {
        fs::create_dir_all(self.paths.ca_dir())?;
        fs::write(self.cert_path(), authority.cert_pem())?;
        let protected = self.protector.protect(authority.key_pem().as_bytes())?;
        fs::write(self.key_path(), protected)?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum CaManagerError {
    #[error("CA: {0}")]
    Ca(#[from] CaError),

    #[error("plataforma: {0}")]
    Platform(#[from] PlatformError),

    #[error("I/O: {0}")]
    Io(#[from] io::Error),

    #[error("UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("estado: {0}")]
    State(String),
}

impl CaManagerError {
    pub fn to_ui_error(&self) -> UiError {
        match self {
            CaManagerError::Platform(_) => UiError::new(
                ErrorCode::CaTrust,
                "Não foi possível instalar ou remover a CA no Windows.",
            )
            .with_detail(self.to_string()),
            CaManagerError::Ca(_) => UiError::new(
                ErrorCode::CaGeneration,
                "Não foi possível gerar a CA local.",
            )
            .with_detail(self.to_string()),
            CaManagerError::Io(_) | CaManagerError::Utf8(_) | CaManagerError::State(_) => {
                UiError::new(ErrorCode::CaGeneration, "Falha local ao processar a CA.")
                    .with_detail(self.to_string())
            }
        }
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use crate::platform::{PlatformCertStore, PlatformFileProtector};

    fn manager() -> (CaManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = RuntimePaths::new(dir.path());
        paths.ensure().expect("paths");
        (
            CaManager::new(
                paths,
                Arc::new(PlatformCertStore::default()),
                Arc::new(PlatformFileProtector::default()),
            ),
            dir,
        )
    }

    #[test]
    fn generates_installs_exports_and_removes() {
        let (manager, dir) = manager();
        let status = manager.status().expect("status");
        assert!(!status.generated);

        let status = manager.install().expect("install");
        assert!(status.generated);
        assert!(status.installed);
        assert!(manager.cert_path().is_file());
        assert!(manager.key_path().is_file());

        let exported = manager
            .export(&dir.path().join("exported.pem"))
            .expect("export");
        assert!(exported.is_file());

        let status = manager.remove().expect("remove");
        assert!(!status.installed);
        assert!(status.generated);
    }

    #[test]
    fn reloads_the_same_ca() {
        let (manager, _dir) = manager();
        let first = manager.authority().expect("first").thumbprint();
        let reloaded = CaManager::new(
            manager.paths.clone(),
            Arc::new(PlatformCertStore::default()),
            Arc::new(PlatformFileProtector::default()),
        );
        assert_eq!(reloaded.authority().expect("reload").thumbprint(), first);
    }
}
