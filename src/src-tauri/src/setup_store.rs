//! Atomic persistence for `setup.json`.
//!
//! The file never contains secrets. A failed/corrupt primary is recovered from
//! the previous backup; both are quarantined when recovery is impossible.

use crate::paths::RuntimePaths;
use ffg_core::error::{ErrorCode, UiError};
use ffg_core::setup::{
    parse_setup_state, SetupMigrationError, SetupState, SetupStatus, StepId, StepStatus,
    SETUP_SCHEMA_VERSION,
};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum SetupStoreError {
    Io(io::Error),
    Serialize(serde_json::Error),
    Corrupt(String),
    SchemaTooNew { found: u32, supported: u32 },
}

impl fmt::Display for SetupStoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupStoreError::Io(error) => write!(f, "falha de I/O no estado: {error}"),
            SetupStoreError::Serialize(error) => write!(f, "falha ao serializar o estado: {error}"),
            SetupStoreError::Corrupt(detail) => write!(f, "estado corrompido: {detail}"),
            SetupStoreError::SchemaTooNew { found, supported } => {
                write!(f, "schema {found} não suportado (máximo {supported})")
            }
        }
    }
}

impl std::error::Error for SetupStoreError {}

impl From<io::Error> for SetupStoreError {
    fn from(error: io::Error) -> Self {
        SetupStoreError::Io(error)
    }
}

impl From<serde_json::Error> for SetupStoreError {
    fn from(error: serde_json::Error) -> Self {
        SetupStoreError::Serialize(error)
    }
}

impl From<SetupStoreError> for UiError {
    fn from(error: SetupStoreError) -> Self {
        match error {
            SetupStoreError::SchemaTooNew { found, supported } => UiError::new(
                ErrorCode::StateSchema,
                "O estado da configuração foi criado por uma versão mais nova do app.",
            )
            .with_detail(format!("schema {found}; suportado até {supported}")),
            SetupStoreError::Corrupt(detail) => UiError::new(
                ErrorCode::StateCorrupt,
                "Não foi possível ler o estado da configuração.",
            )
            .with_detail(detail),
            SetupStoreError::Io(error) => UiError::new(
                ErrorCode::Internal,
                "Não foi possível acessar o estado da configuração.",
            )
            .with_detail(error.to_string())
            .retryable(),
            SetupStoreError::Serialize(error) => UiError::new(
                ErrorCode::Internal,
                "Não foi possível salvar o estado da configuração.",
            )
            .with_detail(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadSource {
    New,
    Primary,
    Backup,
    Quarantined,
}

#[derive(Debug)]
pub struct LoadResult {
    pub state: SetupState,
    pub source: LoadSource,
    pub warning: Option<UiError>,
}

#[derive(Debug, Clone)]
pub struct SetupStore {
    paths: RuntimePaths,
}

impl SetupStore {
    pub fn new(paths: RuntimePaths) -> Self {
        Self { paths }
    }

    pub fn load(&self) -> Result<LoadResult, SetupStoreError> {
        let primary = self.paths.setup_file();
        let backup = self.paths.setup_backup_file();

        if !primary.exists() {
            if backup.exists() {
                if let Ok(state) = self.read_state(&backup) {
                    return Ok(LoadResult {
                        state,
                        source: LoadSource::Backup,
                        warning: Some(
                            UiError::new(
                                ErrorCode::StateCorrupt,
                                "O estado atual não existia; o backup foi recuperado.",
                            )
                            .with_detail(backup.display().to_string()),
                        ),
                    });
                }
            }
            return Ok(LoadResult {
                state: SetupState::new(),
                source: LoadSource::New,
                warning: None,
            });
        }

        match self.read_state(&primary) {
            Ok(state) => Ok(LoadResult {
                state,
                source: LoadSource::Primary,
                warning: None,
            }),
            Err(SetupStoreError::SchemaTooNew { .. }) => Err(self.read_primary_error(&primary)),
            Err(primary_error) => {
                if backup.exists() {
                    match self.read_state(&backup) {
                        Ok(state) => {
                            let _ = self.quarantine(&primary);
                            Ok(LoadResult {
                                state,
                                source: LoadSource::Backup,
                                warning: Some(
                                    UiError::new(
                                        ErrorCode::StateCorrupt,
                                        "O estado principal estava inválido; o backup foi recuperado.",
                                    )
                                    .with_detail(primary_error.to_string()),
                                ),
                            })
                        }
                        Err(backup_error) => {
                            let _ = self.quarantine(&primary);
                            let _ = self.quarantine(&backup);
                            Ok(LoadResult {
                                state: self.attention_state(format!(
                                    "principal: {primary_error}; backup: {backup_error}"
                                )),
                                source: LoadSource::Quarantined,
                                warning: Some(
                                    UiError::new(
                                        ErrorCode::StateCorrupt,
                                        "O estado da configuração foi perdido e será recriado.",
                                    )
                                    .with_detail("os arquivos inválidos foram isolados"),
                                ),
                            })
                        }
                    }
                } else {
                    let _ = self.quarantine(&primary);
                    Ok(LoadResult {
                        state: self.attention_state(primary_error.to_string()),
                        source: LoadSource::Quarantined,
                        warning: Some(
                            UiError::new(
                                ErrorCode::StateCorrupt,
                                "O estado da configuração estava inválido e será recriado.",
                            )
                            .with_detail("o arquivo inválido foi isolado"),
                        ),
                    })
                }
            }
        }
    }

    pub fn save(&self, state: &mut SetupState) -> Result<(), SetupStoreError> {
        if state.schema_version > SETUP_SCHEMA_VERSION {
            return Err(SetupStoreError::SchemaTooNew {
                found: state.schema_version,
                supported: SETUP_SCHEMA_VERSION,
            });
        }

        state.normalize();
        state.schema_version = SETUP_SCHEMA_VERSION;
        state.revision = state.revision.saturating_add(1);
        state.updated_at = Some(now_utc());

        let bytes = serde_json::to_vec_pretty(state)?;
        let primary = self.paths.setup_file();
        let backup = self.paths.setup_backup_file();
        let temp = self.paths.setup_temp_file();

        if let Some(parent) = primary.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&temp, bytes)?;

        if primary.exists() {
            fs::copy(&primary, &backup)?;
        }

        if let Err(error) = replace_file(&temp, &primary) {
            let _ = fs::remove_file(&temp);
            return Err(SetupStoreError::Io(error));
        }

        Ok(())
    }

    fn read_primary_error(&self, primary: &Path) -> SetupStoreError {
        match self.read_state(primary) {
            Ok(_) => SetupStoreError::Corrupt("estado mudou durante a leitura".to_owned()),
            Err(error) => error,
        }
    }

    fn read_state(&self, path: &Path) -> Result<SetupState, SetupStoreError> {
        let raw = fs::read_to_string(path)?;
        parse_setup_state(&raw).map_err(|error| match error {
            SetupMigrationError::InvalidJson(error) => SetupStoreError::Corrupt(error.to_string()),
            SetupMigrationError::SchemaTooNew { found, supported } => {
                SetupStoreError::SchemaTooNew { found, supported }
            }
        })
    }

    fn attention_state(&self, detail: String) -> SetupState {
        let mut state = SetupState::new();
        state.status = SetupStatus::NeedsAttention;
        if let Some(step) = state.step_mut(StepId::Welcome) {
            step.status = StepStatus::Failed;
            step.last_error = Some(
                UiError::new(
                    ErrorCode::StateCorrupt,
                    "Não foi possível recuperar o estado da configuração.",
                )
                .with_detail(detail),
            );
        }
        state
    }

    fn quarantine(&self, path: &Path) -> io::Result<Option<PathBuf>> {
        if !path.exists() {
            return Ok(None);
        }

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("setup.json");
        let target = path.with_file_name(format!("{file_name}.corrupt-{stamp}"));
        fs::rename(path, &target)?;
        Ok(Some(target))
    }
}

#[cfg(not(windows))]
fn replace_file(temp: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temp, destination)
}

#[cfg(windows)]
fn replace_file(temp: &Path, destination: &Path) -> io::Result<()> {
    // TODO(F0 Windows validation): use ReplaceFileW for a truly atomic swap.
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(temp, destination)
}

fn now_utc() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ffg_core::setup::{StepOutcome, StepStatus};

    fn temp_store() -> (RuntimePaths, SetupStore, PathBuf) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let root = std::env::temp_dir().join(format!("ffg-store-{}-{stamp}", std::process::id()));
        let paths = RuntimePaths::new(&root);
        paths.ensure().expect("create dirs");
        let store = SetupStore::new(paths.clone());
        (paths, store, root)
    }

    #[test]
    fn missing_state_starts_fresh() {
        let (_, store, root) = temp_store();
        let result = store.load().expect("load");
        assert_eq!(result.source, LoadSource::New);
        assert_eq!(result.state.current_step, StepId::Welcome);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_and_load_round_trip() {
        let (paths, store, root) = temp_store();
        let mut state = SetupState::new();
        state.begin(StepId::Welcome).expect("begin");
        state
            .complete(
                StepId::Welcome,
                StepOutcome::Done,
                Some("2026-11-06T10:00:00Z"),
            )
            .expect("complete");

        store.save(&mut state).expect("save");
        assert!(paths.setup_file().is_file());
        assert_eq!(state.revision, 1);
        assert!(state.updated_at.is_some());

        let result = store.load().expect("load");
        assert_eq!(result.source, LoadSource::Primary);
        assert_eq!(result.state.current_step, StepId::Runtime);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_primary_falls_back_to_backup() {
        let (paths, store, root) = temp_store();
        let mut state = SetupState::new();
        store.save(&mut state).expect("first save");
        store.save(&mut state).expect("second save");
        assert!(paths.setup_backup_file().is_file());

        fs::write(paths.setup_file(), "{not-json").expect("corrupt");
        let result = store.load().expect("load");
        assert_eq!(result.source, LoadSource::Backup);
        assert!(result.warning.is_some());
        assert_eq!(result.state.schema_version, SETUP_SCHEMA_VERSION);
        assert!(!paths.setup_file().exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn corrupt_primary_and_backup_start_with_attention() {
        let (paths, store, root) = temp_store();
        fs::create_dir_all(paths.state_dir()).expect("state dir");
        fs::write(paths.setup_file(), "{not-json").expect("primary");
        fs::write(paths.setup_backup_file(), "{also-not-json").expect("backup");

        let result = store.load().expect("load");
        assert_eq!(result.source, LoadSource::Quarantined);
        assert_eq!(result.state.status, SetupStatus::NeedsAttention);
        assert_eq!(
            result.state.step(StepId::Welcome).map(|step| step.status),
            Some(StepStatus::Failed)
        );
        assert!(result.warning.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn newer_schema_is_rejected() {
        let (paths, store, root) = temp_store();
        fs::write(
            paths.setup_file(),
            r#"{"schemaVersion":99,"status":"notStarted","currentStep":"welcome","steps":[]}"#,
        )
        .expect("write");
        let error = store.load().expect_err("must fail");
        assert!(matches!(
            error,
            SetupStoreError::SchemaTooNew {
                found: 99,
                supported: SETUP_SCHEMA_VERSION
            }
        ));
        let _ = fs::remove_dir_all(root);
    }
}
