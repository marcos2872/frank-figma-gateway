//! Onboarding state machine.
//!
//! The state is intentionally free of I/O, clocks and Tauri types: it can be
//! serialized to `setup.json`, tested on any platform and evolved with migrations.

use crate::error::UiError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

pub const SETUP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepId {
    Welcome,
    Runtime,
    Provider,
    Models,
    Figma,
    Finish,
}

impl StepId {
    pub const ALL: [StepId; 6] = [
        StepId::Welcome,
        StepId::Runtime,
        StepId::Provider,
        StepId::Models,
        StepId::Figma,
        StepId::Finish,
    ];

    pub fn index(self) -> usize {
        StepId::ALL
            .iter()
            .position(|candidate| *candidate == self)
            .unwrap_or(0)
    }

    pub fn next(self) -> Option<StepId> {
        StepId::ALL.get(self.index() + 1).copied()
    }

    pub fn previous(self) -> Option<StepId> {
        self.index()
            .checked_sub(1)
            .and_then(|index| StepId::ALL.get(index).copied())
    }

    /// Steps that may be skipped with an explicit warning. Runtime, models and
    /// the final test are mandatory.
    pub fn is_skippable(self) -> bool {
        matches!(self, StepId::Provider | StepId::Figma)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepStatus {
    Pending,
    InProgress,
    Done,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SetupStatus {
    NotStarted,
    InProgress,
    Completed,
    NeedsAttention,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepState {
    pub id: StepId,
    pub status: StepStatus,
    #[serde(default)]
    pub stale: bool,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub skipped_reason: Option<String>,
    #[serde(default)]
    pub last_error: Option<UiError>,
}

impl StepState {
    pub fn pending(id: StepId) -> Self {
        Self {
            id,
            status: StepStatus::Pending,
            stale: false,
            completed_at: None,
            skipped_reason: None,
            last_error: None,
        }
    }

    pub fn is_complete(&self) -> bool {
        !self.stale && matches!(self.status, StepStatus::Done | StepStatus::Skipped)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    #[serde(default)]
    pub litellm_version: Option<String>,
    #[serde(default)]
    pub active_litellm: Option<String>,
    #[serde(default)]
    pub pgsql_version: Option<String>,
    #[serde(default)]
    pub active_pgsql: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    #[serde(rename = "openRouter")]
    OpenRouter,
    #[serde(rename = "azureAi")]
    AzureAi,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "openAi")]
    OpenAI,
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "gitHubCopilot")]
    GitHubCopilot,
    #[serde(rename = "openAiCompatible")]
    OpenAICompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestStatus {
    NotTested,
    Ok,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSelection {
    pub profile_id: String,
    pub kind: ProviderKind,
    pub label: String,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub deployment: Option<String>,
    /// Credential Manager entry name. Never the secret itself.
    #[serde(default)]
    pub secret_ref: Option<String>,
    pub test_status: TestStatus,
    #[serde(default)]
    pub tested_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRoute {
    pub public_name: String,
    pub provider_model: String,
    #[serde(default)]
    pub credential_name: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub api_version: Option<String>,
    #[serde(default)]
    pub extra: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelSettings {
    #[serde(default)]
    pub routes: Vec<ModelRoute>,
    #[serde(default)]
    pub fallbacks: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub applied_at: Option<String>,
    #[serde(default)]
    pub config_revision: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthStatus {
    NotStarted,
    WaitingForUser,
    Connected,
    Expired,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotState {
    pub enabled: bool,
    pub postgres_ready: bool,
    pub database_name: String,
    pub credential_name: String,
    pub connected: bool,
    #[serde(default)]
    pub github_login: Option<String>,
    pub auth_status: AuthStatus,
    #[serde(default)]
    pub last_error: Option<UiError>,
}

impl Default for CopilotState {
    fn default() -> Self {
        Self {
            enabled: false,
            postgres_ready: false,
            database_name: "litellm".to_owned(),
            credential_name: "github-copilot".to_owned(),
            connected: false,
            github_login: None,
            auth_status: AuthStatus::NotStarted,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ServicePorts {
    #[serde(default)]
    pub litellm: Option<u16>,
    #[serde(default)]
    pub proxy: Option<u16>,
    #[serde(default)]
    pub postgres: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FigmaState {
    pub detected: bool,
    #[serde(default)]
    pub install_path: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    pub running: bool,
    pub integration_active: bool,
    #[serde(default)]
    pub ca_thumbprint: Option<String>,
    pub env_installed: bool,
    #[serde(default)]
    pub proxy_port: Option<u16>,
    #[serde(default)]
    pub last_validated_at: Option<String>,
    pub skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingState {
    pub offline_mode: bool,
    pub skipped_figma: bool,
    pub start_with_windows: bool,
    #[serde(default)]
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupState {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub revision: u64,
    pub status: SetupStatus,
    pub current_step: StepId,
    pub steps: Vec<StepState>,
    #[serde(default)]
    pub runtime: RuntimeState,
    #[serde(default)]
    pub provider: Option<ProviderSelection>,
    #[serde(default)]
    pub copilot: CopilotState,
    #[serde(default)]
    pub models: ModelSettings,
    #[serde(default)]
    pub services: ServicePorts,
    #[serde(default)]
    pub figma: FigmaState,
    #[serde(default)]
    pub onboarding: OnboardingState,
    #[serde(default)]
    pub updated_at: Option<String>,
}

fn default_schema_version() -> u32 {
    SETUP_SCHEMA_VERSION
}

impl Default for SetupState {
    fn default() -> Self {
        Self::new()
    }
}

impl SetupState {
    pub fn new() -> Self {
        Self {
            schema_version: SETUP_SCHEMA_VERSION,
            revision: 0,
            status: SetupStatus::NotStarted,
            current_step: StepId::Welcome,
            steps: StepId::ALL
                .into_iter()
                .map(StepState::pending)
                .collect::<Vec<_>>(),
            runtime: RuntimeState::default(),
            provider: None,
            copilot: CopilotState::default(),
            models: ModelSettings::default(),
            services: ServicePorts::default(),
            figma: FigmaState::default(),
            onboarding: OnboardingState::default(),
            updated_at: None,
        }
    }

    /// Fills missing steps, removes duplicates/unknown order and keeps the
    /// current step inside the canonical list.
    pub fn normalize(&mut self) {
        let mut normalized = Vec::with_capacity(StepId::ALL.len());
        for id in StepId::ALL {
            if let Some(position) = self.steps.iter().position(|step| step.id == id) {
                normalized.push(self.steps[position].clone());
            } else {
                normalized.push(StepState::pending(id));
            }
        }
        self.steps = normalized;

        if !self.steps.iter().any(|step| step.id == self.current_step) {
            self.current_step = self.first_pending_step();
        }
    }

    pub fn step(&self, id: StepId) -> Option<&StepState> {
        self.steps.iter().find(|step| step.id == id)
    }

    pub fn step_mut(&mut self, id: StepId) -> Option<&mut StepState> {
        self.steps.iter_mut().find(|step| step.id == id)
    }

    pub fn first_pending_step(&self) -> StepId {
        for id in StepId::ALL {
            match self.step(id) {
                Some(step)
                    if step.stale
                        || matches!(
                            step.status,
                            StepStatus::Pending | StepStatus::InProgress | StepStatus::Failed
                        ) =>
                {
                    return id;
                }
                None => return id,
                _ => {}
            }
        }
        StepId::Finish
    }

    pub fn can_begin(&self, step: StepId) -> Result<(), SetupError> {
        let index = step.index();
        for previous in StepId::ALL.iter().take(index) {
            if !self
                .step(*previous)
                .map(StepState::is_complete)
                .unwrap_or(false)
            {
                return Err(SetupError::StepOutOfOrder {
                    step,
                    pending: *previous,
                });
            }
        }
        Ok(())
    }

    pub fn begin(&mut self, step: StepId) -> Result<(), SetupError> {
        self.normalize();
        self.can_begin(step)?;

        let step_state = self.step_mut(step).ok_or(SetupError::UnknownStep(step))?;
        step_state.status = StepStatus::InProgress;
        step_state.stale = false;
        step_state.last_error = None;
        self.current_step = step;
        if matches!(
            self.status,
            SetupStatus::NotStarted | SetupStatus::Completed
        ) {
            self.status = SetupStatus::InProgress;
        }
        Ok(())
    }

    pub fn complete(
        &mut self,
        step: StepId,
        outcome: StepOutcome,
        completed_at: Option<&str>,
    ) -> Result<(), SetupError> {
        self.normalize();

        let step_state = self.step_mut(step).ok_or(SetupError::UnknownStep(step))?;
        if step_state.status != StepStatus::InProgress {
            return Err(SetupError::StepNotStarted(step));
        }

        match &outcome {
            StepOutcome::Done => {
                step_state.status = StepStatus::Done;
                step_state.skipped_reason = None;
            }
            StepOutcome::Skipped { reason } => {
                if !step.is_skippable() {
                    return Err(SetupError::StepCannotSkip(step));
                }
                let reason = reason.trim();
                if reason.is_empty() {
                    return Err(SetupError::SkipReasonRequired(step));
                }
                step_state.status = StepStatus::Skipped;
                step_state.skipped_reason = Some(reason.to_owned());
            }
        }

        step_state.stale = false;
        step_state.last_error = None;
        step_state.completed_at = completed_at.map(str::to_owned);

        if step == StepId::Finish && matches!(outcome, StepOutcome::Done) {
            self.status = SetupStatus::Completed;
            self.current_step = StepId::Finish;
            self.onboarding.completed_at = completed_at.map(str::to_owned);
        } else {
            self.current_step = self.first_pending_step();
            self.recompute_status();
        }
        Ok(())
    }

    /// Marks every completed step after `from` as stale. The steps themselves
    /// are kept so the user can see what needs to be redone.
    pub fn invalidate_downstream(&mut self, from: StepId) {
        self.normalize();
        let start = from.index() + 1;
        for step in self.steps.iter_mut().skip(start) {
            if step.is_complete() {
                step.stale = true;
            }
        }
        self.onboarding.completed_at = None;
        self.current_step = self.first_pending_step();
        self.recompute_status();
    }

    pub fn apply_options(&mut self, offline_mode: Option<bool>, start_with_windows: Option<bool>) {
        if let Some(value) = offline_mode {
            self.onboarding.offline_mode = value;
        }
        if let Some(value) = start_with_windows {
            self.onboarding.start_with_windows = value;
        }
    }

    pub fn reset(&self, scope: ResetScope) -> Self {
        match scope {
            ResetScope::All => Self::new(),
            ResetScope::Wizard => {
                let mut next = Self::new();
                next.revision = self.revision;
                next.runtime = self.runtime.clone();
                next.services = self.services;
                next.onboarding.offline_mode = self.onboarding.offline_mode;
                next.onboarding.start_with_windows = self.onboarding.start_with_windows;

                next.complete_step_without_running(StepId::Welcome);
                if next.runtime.active_litellm.is_some() {
                    next.complete_step_without_running(StepId::Runtime);
                    next.current_step = StepId::Provider;
                } else {
                    next.current_step = StepId::Runtime;
                }
                next.status = SetupStatus::InProgress;
                next
            }
            ResetScope::Figma => {
                let mut next = self.clone();
                next.figma.integration_active = false;
                next.figma.env_installed = false;
                next.figma.ca_thumbprint = None;
                next.figma.last_validated_at = None;
                next.figma.skipped = false;
                next.onboarding.skipped_figma = false;
                for id in [StepId::Figma, StepId::Finish] {
                    if let Some(step) = next.step_mut(id) {
                        *step = StepState::pending(id);
                    }
                }
                next.current_step = StepId::Figma;
                next.status = SetupStatus::InProgress;
                next
            }
        }
    }

    fn complete_step_without_running(&mut self, step: StepId) {
        if let Some(step_state) = self.step_mut(step) {
            step_state.status = StepStatus::Done;
            step_state.stale = false;
        }
    }

    fn recompute_status(&mut self) {
        if self
            .steps
            .iter()
            .any(|step| step.status == StepStatus::Failed)
        {
            self.status = SetupStatus::NeedsAttention;
        } else if self.steps.iter().all(StepState::is_complete) {
            self.status = SetupStatus::Completed;
        } else if self
            .steps
            .iter()
            .any(|step| step.status != StepStatus::Pending || step.stale)
        {
            self.status = SetupStatus::InProgress;
        } else {
            self.status = SetupStatus::NotStarted;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum StepOutcome {
    Done,
    Skipped { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetScope {
    Wizard,
    Figma,
    All,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupError {
    UnknownStep(StepId),
    StepOutOfOrder { step: StepId, pending: StepId },
    StepNotStarted(StepId),
    StepCannotSkip(StepId),
    SkipReasonRequired(StepId),
}

impl fmt::Display for SetupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupError::UnknownStep(step) => write!(f, "passo desconhecido: {step:?}"),
            SetupError::StepOutOfOrder { step, pending } => {
                write!(f, "passo {step:?} depende de {pending:?}")
            }
            SetupError::StepNotStarted(step) => {
                write!(f, "passo {step:?} não está em andamento")
            }
            SetupError::StepCannotSkip(step) => write!(f, "passo {step:?} não pode ser pulado"),
            SetupError::SkipReasonRequired(step) => {
                write!(f, "informe o motivo para pular o passo {step:?}")
            }
        }
    }
}

impl std::error::Error for SetupError {}

#[derive(Debug)]
pub enum SetupMigrationError {
    InvalidJson(serde_json::Error),
    SchemaTooNew { found: u32, supported: u32 },
}

impl fmt::Display for SetupMigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SetupMigrationError::InvalidJson(error) => write!(f, "setup.json inválido: {error}"),
            SetupMigrationError::SchemaTooNew { found, supported } => write!(
                f,
                "setup.json usa schema {found}, mas este app suporta até {supported}"
            ),
        }
    }
}

impl std::error::Error for SetupMigrationError {}

impl From<serde_json::Error> for SetupMigrationError {
    fn from(error: serde_json::Error) -> Self {
        SetupMigrationError::InvalidJson(error)
    }
}

/// Parses a persisted state, rejecting schemas created by a newer app.
pub fn parse_setup_state(raw: &str) -> Result<SetupState, SetupMigrationError> {
    let value: serde_json::Value = serde_json::from_str(raw)?;
    let schema = value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(u64::from(SETUP_SCHEMA_VERSION)) as u32;

    if schema > SETUP_SCHEMA_VERSION {
        return Err(SetupMigrationError::SchemaTooNew {
            found: schema,
            supported: SETUP_SCHEMA_VERSION,
        });
    }

    let mut state: SetupState = serde_json::from_value(value)?;
    state.schema_version = SETUP_SCHEMA_VERSION;
    state.normalize();
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(state: &mut SetupState, step: StepId) {
        state.begin(step).expect("begin");
        state
            .complete(step, StepOutcome::Done, Some("2026-11-06T10:00:00Z"))
            .expect("complete");
    }

    #[test]
    fn new_state_has_every_step_pending() {
        let state = SetupState::new();
        assert_eq!(state.status, SetupStatus::NotStarted);
        assert_eq!(state.current_step, StepId::Welcome);
        assert_eq!(state.steps.len(), StepId::ALL.len());
        assert!(state
            .steps
            .iter()
            .all(|step| step.status == StepStatus::Pending));
    }

    #[test]
    fn steps_advance_in_order() {
        let mut state = SetupState::new();
        done(&mut state, StepId::Welcome);
        assert_eq!(state.current_step, StepId::Runtime);
        done(&mut state, StepId::Runtime);
        assert_eq!(state.current_step, StepId::Provider);
        assert_eq!(state.status, SetupStatus::InProgress);
    }

    #[test]
    fn cannot_start_a_step_before_the_previous_one() {
        let mut state = SetupState::new();
        let error = state.begin(StepId::Runtime).expect_err("must fail");
        assert_eq!(
            error,
            SetupError::StepOutOfOrder {
                step: StepId::Runtime,
                pending: StepId::Welcome
            }
        );
    }

    #[test]
    fn provider_and_figma_can_be_skipped_with_reason() {
        let mut state = SetupState::new();
        done(&mut state, StepId::Welcome);
        done(&mut state, StepId::Runtime);

        state.begin(StepId::Provider).expect("begin");
        state
            .complete(
                StepId::Provider,
                StepOutcome::Skipped {
                    reason: "configuro depois".to_owned(),
                },
                Some("2026-11-06T10:00:00Z"),
            )
            .expect("skip");
        assert_eq!(
            state.step(StepId::Provider).map(|step| step.status),
            Some(StepStatus::Skipped)
        );
        assert_eq!(
            state
                .step(StepId::Provider)
                .and_then(|step| step.skipped_reason.as_deref()),
            Some("configuro depois")
        );
    }

    #[test]
    fn required_steps_cannot_be_skipped() {
        let mut state = SetupState::new();
        state.begin(StepId::Welcome).expect("begin");
        let error = state
            .complete(
                StepId::Welcome,
                StepOutcome::Skipped {
                    reason: "qualquer".to_owned(),
                },
                None,
            )
            .expect_err("must fail");
        assert_eq!(error, SetupError::StepCannotSkip(StepId::Welcome));
    }

    #[test]
    fn skip_requires_a_non_empty_reason() {
        let mut state = SetupState::new();
        done(&mut state, StepId::Welcome);
        done(&mut state, StepId::Runtime);
        state.begin(StepId::Provider).expect("begin");
        let error = state
            .complete(
                StepId::Provider,
                StepOutcome::Skipped {
                    reason: "   ".to_owned(),
                },
                None,
            )
            .expect_err("must fail");
        assert_eq!(error, SetupError::SkipReasonRequired(StepId::Provider));
    }

    #[test]
    fn finish_completes_the_setup() {
        let mut state = SetupState::new();
        for step in [
            StepId::Welcome,
            StepId::Runtime,
            StepId::Provider,
            StepId::Models,
        ] {
            done(&mut state, step);
        }
        state.begin(StepId::Figma).expect("begin");
        state
            .complete(
                StepId::Figma,
                StepOutcome::Skipped {
                    reason: "sem Figma".to_owned(),
                },
                Some("2026-11-06T10:00:00Z"),
            )
            .expect("skip");
        done(&mut state, StepId::Finish);

        assert_eq!(state.status, SetupStatus::Completed);
        assert_eq!(state.current_step, StepId::Finish);
        assert_eq!(
            state.onboarding.completed_at.as_deref(),
            Some("2026-11-06T10:00:00Z")
        );
    }

    #[test]
    fn invalidates_downstream_steps() {
        let mut state = SetupState::new();
        for step in [
            StepId::Welcome,
            StepId::Runtime,
            StepId::Provider,
            StepId::Models,
            StepId::Figma,
            StepId::Finish,
        ] {
            done(&mut state, step);
        }
        assert_eq!(state.status, SetupStatus::Completed);

        state.invalidate_downstream(StepId::Provider);
        assert_eq!(state.status, SetupStatus::InProgress);
        assert!(state.step(StepId::Models).expect("models").stale);
        assert!(state.step(StepId::Finish).expect("finish").stale);
        assert!(!state.step(StepId::Provider).expect("provider").stale);
        assert_eq!(state.current_step, StepId::Models);
    }

    #[test]
    fn reset_wizard_keeps_the_runtime() {
        let mut state = SetupState::new();
        for step in [
            StepId::Welcome,
            StepId::Runtime,
            StepId::Provider,
            StepId::Models,
        ] {
            done(&mut state, step);
        }
        state.runtime.active_litellm = Some("1.108.1".to_owned());

        let next = state.reset(ResetScope::Wizard);
        assert_eq!(next.current_step, StepId::Provider);
        assert_eq!(
            next.step(StepId::Runtime).map(|step| step.status),
            Some(StepStatus::Done)
        );
        assert_eq!(
            next.step(StepId::Provider).map(|step| step.status),
            Some(StepStatus::Pending)
        );
        assert_eq!(next.runtime.active_litellm.as_deref(), Some("1.108.1"));
    }

    #[test]
    fn reset_figma_clears_integration_flags() {
        let mut state = SetupState::new();
        state.figma.integration_active = true;
        state.figma.env_installed = true;
        state.figma.ca_thumbprint = Some("abc".to_owned());

        let next = state.reset(ResetScope::Figma);
        assert_eq!(next.current_step, StepId::Figma);
        assert!(!next.figma.integration_active);
        assert!(!next.figma.env_installed);
        assert!(next.figma.ca_thumbprint.is_none());
        assert_eq!(
            next.step(StepId::Finish).map(|step| step.status),
            Some(StepStatus::Pending)
        );
    }

    #[test]
    fn reset_all_starts_from_scratch() {
        let mut state = SetupState::new();
        state.revision = 4;
        let next = state.reset(ResetScope::All);
        assert_eq!(next.revision, 0);
        assert_eq!(next.status, SetupStatus::NotStarted);
    }

    #[test]
    fn normalize_fills_missing_steps() {
        let mut state = SetupState::new();
        state.steps.retain(|step| step.id != StepId::Models);
        state.normalize();
        assert_eq!(state.steps.len(), StepId::ALL.len());
        assert_eq!(
            state.step(StepId::Models).map(|step| step.status),
            Some(StepStatus::Pending)
        );
    }

    #[test]
    fn serializes_the_expected_camel_case_shape() {
        let mut state = SetupState::new();
        state.onboarding.offline_mode = true;
        let json = serde_json::to_value(&state).expect("serializable");
        assert_eq!(json["schemaVersion"], SETUP_SCHEMA_VERSION);
        assert_eq!(json["currentStep"], "welcome");
        assert_eq!(json["onboarding"]["offlineMode"], true);
        assert_eq!(json["steps"][0]["id"], "welcome");
        assert_eq!(json["copilot"]["credentialName"], "github-copilot");
    }

    #[test]
    fn parses_and_normalizes_a_persisted_state() {
        let mut state = SetupState::new();
        state.steps.remove(0);
        let raw = serde_json::to_string(&state).expect("serializable");
        let parsed = parse_setup_state(&raw).expect("parse");
        assert_eq!(parsed.steps.len(), StepId::ALL.len());
        assert_eq!(parsed.schema_version, SETUP_SCHEMA_VERSION);
    }

    #[test]
    fn rejects_a_newer_schema() {
        let raw =
            r#"{"schemaVersion":99,"status":"notStarted","currentStep":"welcome","steps":[]}"#;
        let error = parse_setup_state(raw).expect_err("must fail");
        assert!(matches!(
            error,
            SetupMigrationError::SchemaTooNew {
                found: 99,
                supported: SETUP_SCHEMA_VERSION
            }
        ));
    }

    #[test]
    fn step_navigation_is_bounded() {
        assert_eq!(StepId::Welcome.previous(), None);
        assert_eq!(StepId::Welcome.next(), Some(StepId::Runtime));
        assert_eq!(StepId::Finish.next(), None);
        assert_eq!(StepId::Finish.previous(), Some(StepId::Figma));
    }
}
