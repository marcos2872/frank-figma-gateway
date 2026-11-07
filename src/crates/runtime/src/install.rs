//! Atomic installation: staging → probe → versioned directory → active pointer.

use ffg_core::download::ComponentId;
use ffg_core::install::{ActiveComponent, ActiveRuntime, InstallPlan};
use ffg_core::manifest::ComponentManifest;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::RuntimeError;
use crate::extract::extract_zip;
use crate::probe::probe_default;

/// Result of an installation, mirrored by the UI contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installation {
    pub component: ComponentId,
    pub version: String,
    pub path: String,
    #[serde(default)]
    pub previous: Option<ActiveComponent>,
}

pub struct Installer {
    root: PathBuf,
}

impl Installer {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn active_file(&self) -> PathBuf {
        self.root.join("runtime").join("active.json")
    }

    /// Reads the active pointer; a missing file means "nothing installed".
    pub fn active(&self) -> Result<ActiveRuntime, RuntimeError> {
        let path = self.active_file();
        if !path.is_file() {
            return Ok(ActiveRuntime::default());
        }
        let raw = fs::read_to_string(&path)?;
        serde_json::from_str(&raw)
            .map_err(|error| RuntimeError::State(format!("active.json inválido: {error}")))
    }

    /// Installs a verified archive and activates it. On any failure the
    /// previous pointer and the staging directory are left intact.
    pub async fn install(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        zip_path: &Path,
    ) -> Result<Installation, RuntimeError> {
        let staging_id = uuid::Uuid::new_v4().simple().to_string();
        let plan = InstallPlan::new(component, manifest, &staging_id)?;
        let staging = self.root.join(&plan.staging_path);

        if staging.exists() {
            fs::remove_dir_all(&staging)?;
        }
        fs::create_dir_all(&staging)?;

        let result = self
            .install_staged(component, manifest, &plan, &staging, zip_path)
            .await;
        if result.is_err() {
            let _ = fs::remove_dir_all(&staging);
        }
        result
    }

    async fn install_staged(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        plan: &InstallPlan,
        staging: &Path,
        zip_path: &Path,
    ) -> Result<Installation, RuntimeError> {
        extract_zip(zip_path, staging, manifest.entrypoint())?;

        let entrypoint = staging.join(&plan.entrypoint);
        probe_default(&entrypoint, &plan.version_args, &manifest.version).await?;

        let target = self.root.join(&plan.target_path);
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(staging, &target).map_err(|error| {
            RuntimeError::Activation(format!("não foi possível mover o staging: {error}"))
        })?;

        let mut active = self.active()?;
        let previous = active.get(component).cloned();
        active.activate(
            component,
            ActiveComponent {
                version: manifest.version.clone(),
                path: plan.target_path.clone(),
            },
        );

        if let Err(error) = self.write_active(&active) {
            let mut rollback = active.clone();
            rollback.rollback(component, previous.clone());
            let _ = self.write_active(&rollback);
            let _ = fs::remove_dir_all(&target);
            return Err(error);
        }

        Ok(Installation {
            component,
            version: manifest.version.clone(),
            path: plan.target_path.clone(),
            previous,
        })
    }

    fn write_active(&self, active: &ActiveRuntime) -> Result<(), RuntimeError> {
        let path = self.active_file();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(active)
            .map_err(|error| RuntimeError::State(error.to_string()))?;
        let temp = path.with_extension("json.tmp");
        fs::write(&temp, bytes)?;
        if path.exists() {
            fs::remove_file(&path)?;
        }
        fs::rename(&temp, &path).map_err(|error| {
            RuntimeError::Activation(format!("não foi possível gravar active.json: {error}"))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::write_zip;

    fn manifest() -> ComponentManifest {
        ComponentManifest {
            version: "1.108.1".to_owned(),
            url: "https://example.invalid/litellm.zip".to_owned(),
            sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
            size: 10,
            installed_size: None,
            entrypoint: Some("litellm.exe".to_owned()),
            version_args: vec!["--version".to_owned()],
            min_app_version: None,
        }
    }

    #[test]
    fn missing_pointer_means_nothing_installed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let installer = Installer::new(dir.path().to_path_buf());
        let active = installer.active().expect("active");
        assert!(active.litellm.is_none());
        assert!(active.pgsql.is_none());
    }

    #[test]
    fn writes_and_reads_the_active_pointer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let installer = Installer::new(dir.path().to_path_buf());
        let active = ActiveRuntime {
            litellm: Some(ActiveComponent {
                version: "1.108.1".to_owned(),
                path: "runtime/litellm/1.108.1".to_owned(),
            }),
            pgsql: None,
        };
        installer.write_active(&active).expect("write");
        assert_eq!(installer.active().expect("read"), active);
        assert!(installer.active_file().is_file());
    }

    #[test]
    fn rejects_a_corrupt_pointer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let installer = Installer::new(dir.path().to_path_buf());
        fs::create_dir_all(installer.active_file().parent().expect("parent")).expect("dirs");
        fs::write(installer.active_file(), "{not-json").expect("write");
        let error = installer.active().expect_err("must fail");
        assert!(matches!(error, RuntimeError::State(_)));
    }

    #[tokio::test]
    async fn install_requires_a_working_entrypoint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("runtime.zip");
        write_zip(&zip_path, &[("litellm.exe", b"not-a-real-binary")]).expect("zip");

        let installer = Installer::new(dir.path().to_path_buf());
        let error = installer
            .install(ComponentId::Litellm, &manifest(), &zip_path)
            .await
            .expect_err("probe must fail");

        assert!(matches!(
            error,
            RuntimeError::VersionProbe(_) | RuntimeError::Timeout(_)
        ));
        assert!(!dir.path().join("runtime/litellm/1.108.1").exists());
        assert!(installer.active().expect("active").litellm.is_none());
    }

    #[tokio::test]
    async fn failed_install_leaves_no_staging_behind() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("runtime.zip");
        write_zip(&zip_path, &[("litellm.exe", b"x")]).expect("zip");

        let installer = Installer::new(dir.path().to_path_buf());
        let _ = installer
            .install(ComponentId::Litellm, &manifest(), &zip_path)
            .await;

        let staging = dir.path().join("runtime").join("staging");
        let leftovers = fs::read_dir(&staging)
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert_eq!(leftovers, 0);
    }
}
