//! Transactional store for `config.yaml` with history.

use crate::error::LiteLlmError;
use ffg_core::litellm_config::generate_litellm_config;
use ffg_core::setup::ModelSettings;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppliedConfig {
    pub revision: String,
    pub path: PathBuf,
    pub yaml: String,
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    dir: PathBuf,
}

impl ConfigStore {
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self, LiteLlmError> {
        let dir = dir.into();
        fs::create_dir_all(dir.join("candidates"))?;
        fs::create_dir_all(dir.join("history"))?;
        Ok(Self { dir })
    }

    pub fn current_path(&self) -> PathBuf {
        self.dir.join("config.yaml")
    }

    pub fn candidates_dir(&self) -> PathBuf {
        self.dir.join("candidates")
    }

    pub fn history_dir(&self) -> PathBuf {
        self.dir.join("history")
    }

    pub fn apply(&self, settings: &ModelSettings) -> Result<AppliedConfig, LiteLlmError> {
        let yaml = generate_litellm_config(settings)
            .map_err(|error| LiteLlmError::Config(error.to_string()))?;
        let current_revision = revision(&yaml);
        let history_name = current_revision.trim_start_matches("sha256:").to_owned();
        let candidate = self.candidates_dir().join(format!("{history_name}.yaml"));
        write_atomic(&candidate, &yaml)?;

        let current = self.current_path();
        if current.exists() {
            let previous = fs::read_to_string(&current)?;
            let previous_name = revision(&previous).trim_start_matches("sha256:").to_owned();
            let history = self.history_dir().join(format!("{previous_name}.yaml"));
            if !history.exists() {
                write_atomic(&history, &previous)?;
            }
        }

        write_atomic(&current, &yaml)?;
        Ok(AppliedConfig {
            revision: current_revision,
            path: current,
            yaml,
        })
    }

    pub fn load_current(&self) -> Result<Option<String>, LiteLlmError> {
        match fs::read_to_string(self.current_path()) {
            Ok(value) => Ok(Some(value)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(LiteLlmError::Io(error)),
        }
    }

    pub fn history_revisions(&self) -> Result<Vec<String>, LiteLlmError> {
        let mut revisions = Vec::new();
        for entry in fs::read_dir(self.history_dir())? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                if let Some(stem) = name.strip_suffix(".yaml") {
                    revisions.push(format!("sha256:{stem}"));
                }
            }
        }
        revisions.sort();
        Ok(revisions)
    }
}

pub fn revision(yaml: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(yaml.as_bytes())))
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), LiteLlmError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("yaml.tmp");
    fs::write(&temp, contents)?;
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ffg_core::setup::{ModelRoute, ModelSettings};
    use std::collections::BTreeMap;

    fn settings(public_name: &str) -> ModelSettings {
        ModelSettings {
            routes: vec![ModelRoute {
                public_name: public_name.to_owned(),
                provider_model: "openrouter/anthropic/claude-haiku-4.5".to_owned(),
                credential_name: None,
                api_key_env: Some("OPENROUTER_API_KEY".to_owned()),
                api_base: None,
                api_version: None,
                extra: BTreeMap::new(),
            }],
            aliases: BTreeMap::new(),
            fallbacks: BTreeMap::new(),
            applied_at: None,
            config_revision: None,
        }
    }

    #[test]
    fn applies_and_keeps_history() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ConfigStore::new(dir.path()).expect("store");

        let first = store.apply(&settings("claude-opus-4-8")).expect("first");
        assert!(first.path.is_file());
        assert!(store.load_current().expect("current").is_some());
        assert!(store.history_revisions().expect("history").is_empty());

        let second = store.apply(&settings("claude-haiku-4-5")).expect("second");
        assert_ne!(first.revision, second.revision);
        let history = store.history_revisions().expect("history");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0], first.revision);
    }

    #[test]
    fn rejects_invalid_config_without_overwriting_current() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = ConfigStore::new(dir.path()).expect("store");
        store.apply(&settings("claude-opus-4-8")).expect("valid");
        let before = store.load_current().expect("current").expect("content");

        let mut invalid = settings("gpt-5");
        invalid.routes[0].public_name = "gpt-5".to_owned();
        let error = store.apply(&invalid).expect_err("must fail");
        assert!(matches!(error, LiteLlmError::Config(_)));
        assert_eq!(
            store.load_current().expect("current").expect("content"),
            before
        );
    }

    #[test]
    fn revision_is_deterministic() {
        assert_eq!(revision("a"), revision("a"));
        assert_ne!(revision("a"), revision("b"));
        assert!(revision("a").starts_with("sha256:"));
    }
}
