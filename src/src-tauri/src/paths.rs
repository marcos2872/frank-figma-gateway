//! Filesystem layout owned by the app.
//!
//! Everything lives under Tauri's app-local data directory; nothing is written
//! next to the executable and no path is hardcoded to a user name.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RuntimePaths {
    root: PathBuf,
}

impl RuntimePaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn operations_dir(&self) -> PathBuf {
        self.state_dir().join("operations")
    }

    pub fn setup_file(&self) -> PathBuf {
        self.state_dir().join("setup.json")
    }

    pub fn setup_backup_file(&self) -> PathBuf {
        self.state_dir().join("setup.json.bak")
    }

    pub fn setup_temp_file(&self) -> PathBuf {
        self.state_dir().join("setup.json.tmp")
    }

    #[allow(dead_code)] // Used by the Figma integration journal (F8).
    pub fn integration_file(&self) -> PathBuf {
        self.state_dir().join("integration.json")
    }

    pub fn runtime_dir(&self) -> PathBuf {
        self.root.join("runtime")
    }

    pub fn staging_dir(&self) -> PathBuf {
        self.runtime_dir().join("staging")
    }

    #[allow(dead_code)] // Used by the atomic runtime activation (F3).
    pub fn active_runtime_file(&self) -> PathBuf {
        self.runtime_dir().join("active.json")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn downloads_dir(&self) -> PathBuf {
        self.cache_dir().join("downloads")
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join("config")
    }

    pub fn litellm_config_dir(&self) -> PathBuf {
        self.config_dir().join("litellm")
    }

    #[allow(dead_code)] // Used by the LiteLLM config lifecycle (F5).
    pub fn litellm_config_file(&self) -> PathBuf {
        self.litellm_config_dir().join("config.yaml")
    }

    pub fn litellm_history_dir(&self) -> PathBuf {
        self.litellm_config_dir().join("history")
    }

    pub fn litellm_candidates_dir(&self) -> PathBuf {
        self.litellm_config_dir().join("candidates")
    }

    pub fn postgres_dir(&self) -> PathBuf {
        self.root.join("postgres")
    }

    pub fn postgres_data_dir(&self) -> PathBuf {
        self.postgres_dir().join("data")
    }

    pub fn ca_dir(&self) -> PathBuf {
        self.root.join("ca")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn diagnostics_dir(&self) -> PathBuf {
        self.root.join("diagnostics")
    }

    /// Creates the directory tree. Existing directories are left untouched.
    pub fn ensure(&self) -> io::Result<()> {
        for directory in [
            self.state_dir(),
            self.operations_dir(),
            self.runtime_dir(),
            self.staging_dir(),
            self.downloads_dir(),
            self.litellm_config_dir(),
            self.litellm_history_dir(),
            self.litellm_candidates_dir(),
            self.postgres_data_dir(),
            self.ca_dir(),
            self.logs_dir(),
            self.diagnostics_dir(),
        ] {
            fs::create_dir_all(directory)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_paths_under_the_root() {
        let paths = RuntimePaths::new(PathBuf::from("/data/ffg"));
        assert_eq!(
            paths.setup_file(),
            PathBuf::from("/data/ffg/state/setup.json")
        );
        assert_eq!(
            paths.litellm_config_file(),
            PathBuf::from("/data/ffg/config/litellm/config.yaml")
        );
        assert_eq!(
            paths.postgres_data_dir(),
            PathBuf::from("/data/ffg/postgres/data")
        );
        assert_eq!(paths.root(), Path::new("/data/ffg"));
    }

    #[test]
    fn creates_the_expected_tree() {
        let root = std::env::temp_dir().join(format!(
            "ffg-paths-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let paths = RuntimePaths::new(&root);
        paths.ensure().expect("create dirs");
        assert!(paths.state_dir().is_dir());
        assert!(paths.downloads_dir().is_dir());
        assert!(paths.ca_dir().is_dir());
        let _ = fs::remove_dir_all(root);
    }
}
