//! Journal for the reversible Figma integration.
//!
//! Every side effect is recorded before/after execution so a crash or partial
//! failure can be rolled back in reverse order: env → proxy → CA.

use serde::{Deserialize, Serialize};

pub const JOURNAL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationStatus {
    Inactive,
    Active,
    Partial,
    Removing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IntegrationStep {
    CaInstall,
    ProxyStart,
    EnvWrite,
    Validate,
    EnvRestore,
    ProxyStop,
    CaRemove,
}

impl IntegrationStep {
    /// Steps required for a new install.
    ///
    /// `EnvWrite`/`EnvRestore` remain in the enum only to read journals written
    /// by older versions; the current integration never touches
    /// `HKCU\Environment`.
    pub const INSTALL_STEPS: [IntegrationStep; 3] = [
        IntegrationStep::CaInstall,
        IntegrationStep::ProxyStart,
        IntegrationStep::Validate,
    ];

    pub const REMOVAL_STEPS: [IntegrationStep; 2] =
        [IntegrationStep::ProxyStop, IntegrationStep::CaRemove];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JournalStepStatus {
    Pending,
    Done,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepRecord {
    pub step: IntegrationStep,
    pub status: JournalStepStatus,
    #[serde(default)]
    pub at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationJournal {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub status: IntegrationStatus,
    #[serde(default)]
    pub steps: Vec<StepRecord>,
    #[serde(default)]
    pub ca_thumbprint: Option<String>,
    #[serde(default)]
    pub env_snapshot_ref: Option<String>,
    #[serde(default)]
    pub proxy_port: Option<u16>,
}

fn default_schema_version() -> u32 {
    JOURNAL_SCHEMA_VERSION
}

impl Default for IntegrationJournal {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrationJournal {
    pub fn new() -> Self {
        Self {
            schema_version: JOURNAL_SCHEMA_VERSION,
            status: IntegrationStatus::Inactive,
            steps: Vec::new(),
            ca_thumbprint: None,
            env_snapshot_ref: None,
            proxy_port: None,
        }
    }

    pub fn record(&mut self, step: IntegrationStep, status: JournalStepStatus, at: Option<&str>) {
        if let Some(existing) = self.steps.iter_mut().find(|record| record.step == step) {
            existing.status = status;
            existing.at = at.map(str::to_owned);
        } else {
            self.steps.push(StepRecord {
                step,
                status,
                at: at.map(str::to_owned),
            });
        }
        self.refresh_status();
    }

    pub fn mark_done(&mut self, step: IntegrationStep, at: Option<&str>) {
        self.record(step, JournalStepStatus::Done, at);
    }

    pub fn mark_failed(&mut self, step: IntegrationStep, at: Option<&str>) {
        self.record(step, JournalStepStatus::Failed, at);
    }

    pub fn is_done(&self, step: IntegrationStep) -> bool {
        self.steps
            .iter()
            .any(|record| record.step == step && record.status == JournalStepStatus::Done)
    }

    pub fn has_failure(&self) -> bool {
        self.steps
            .iter()
            .any(|record| record.status == JournalStepStatus::Failed)
    }

    pub fn refresh_status(&mut self) {
        if self.status == IntegrationStatus::Removing {
            if IntegrationStep::REMOVAL_STEPS
                .iter()
                .all(|step| self.is_done(*step))
            {
                self.status = IntegrationStatus::Inactive;
            }
            return;
        }

        if self.has_failure() {
            self.status = IntegrationStatus::Partial;
        } else if IntegrationStep::INSTALL_STEPS
            .iter()
            .all(|step| self.is_done(*step))
        {
            self.status = IntegrationStatus::Active;
        } else {
            self.status = IntegrationStatus::Inactive;
        }
    }

    pub fn begin_removal(&mut self) {
        self.status = IntegrationStatus::Removing;
    }

    /// Reverse order of the completed side effects.
    pub fn rollback_plan(&self) -> Vec<IntegrationStep> {
        let mut plan = Vec::new();
        // Legacy env writes are cleaned unconditionally by the app, not here.
        if self.is_done(IntegrationStep::ProxyStart) && !self.is_done(IntegrationStep::ProxyStop) {
            plan.push(IntegrationStep::ProxyStop);
        }
        if self.is_done(IntegrationStep::CaInstall) && !self.is_done(IntegrationStep::CaRemove) {
            plan.push(IntegrationStep::CaRemove);
        }
        plan
    }

    pub fn mark_removed(&mut self, at: Option<&str>) {
        self.status = IntegrationStatus::Removing;
        for step in IntegrationStep::REMOVAL_STEPS {
            self.record(step, JournalStepStatus::Done, at);
        }
        self.ca_thumbprint = None;
        self.env_snapshot_ref = None;
        self.proxy_port = None;
        self.status = IntegrationStatus::Inactive;
    }

    pub fn is_active(&self) -> bool {
        self.status == IntegrationStatus::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_journal() -> IntegrationJournal {
        let mut journal = IntegrationJournal::new();
        for step in IntegrationStep::INSTALL_STEPS {
            journal.mark_done(step, Some("2026-11-06T10:00:00Z"));
        }
        journal.ca_thumbprint = Some("thumb".to_owned());
        journal.env_snapshot_ref = Some("ffg/integration/env-snapshot".to_owned());
        journal.proxy_port = Some(8787);
        journal
    }

    #[test]
    fn becomes_active_after_all_install_steps() {
        let journal = active_journal();
        assert_eq!(journal.status, IntegrationStatus::Active);
        assert!(journal.is_active());
    }

    #[test]
    fn a_failure_moves_the_journal_to_partial() {
        let mut journal = active_journal();
        journal.mark_failed(IntegrationStep::Validate, Some("2026-11-06T10:01:00Z"));
        assert_eq!(journal.status, IntegrationStatus::Partial);
        assert!(journal.has_failure());
    }

    #[test]
    fn rollback_runs_proxy_then_ca() {
        let journal = active_journal();
        assert_eq!(
            journal.rollback_plan(),
            vec![IntegrationStep::ProxyStop, IntegrationStep::CaRemove]
        );
    }

    #[test]
    fn legacy_journal_with_env_steps_is_still_active() {
        let mut journal = IntegrationJournal::new();
        for step in [
            IntegrationStep::CaInstall,
            IntegrationStep::ProxyStart,
            IntegrationStep::EnvWrite,
            IntegrationStep::Validate,
        ] {
            journal.mark_done(step, None);
        }
        assert_eq!(journal.status, IntegrationStatus::Active);
    }

    #[test]
    fn rollback_skips_steps_that_were_not_executed() {
        let mut journal = IntegrationJournal::new();
        journal.mark_done(IntegrationStep::CaInstall, None);
        assert_eq!(journal.rollback_plan(), vec![IntegrationStep::CaRemove]);
    }

    #[test]
    fn rollback_is_idempotent() {
        let mut journal = active_journal();
        journal.mark_done(IntegrationStep::ProxyStop, None);
        assert_eq!(journal.rollback_plan(), vec![IntegrationStep::CaRemove]);
        journal.mark_done(IntegrationStep::CaRemove, None);
        assert!(journal.rollback_plan().is_empty());
    }

    #[test]
    fn removal_clears_side_effect_metadata() {
        let mut journal = active_journal();
        journal.begin_removal();
        assert_eq!(journal.status, IntegrationStatus::Removing);
        journal.mark_removed(Some("2026-11-06T10:02:00Z"));
        assert_eq!(journal.status, IntegrationStatus::Inactive);
        assert!(journal.ca_thumbprint.is_none());
        assert!(journal.env_snapshot_ref.is_none());
        assert!(journal.proxy_port.is_none());
        assert!(journal.rollback_plan().is_empty());
    }

    #[test]
    fn repeated_records_do_not_duplicate_steps() {
        let mut journal = IntegrationJournal::new();
        journal.mark_done(IntegrationStep::CaInstall, None);
        journal.mark_done(IntegrationStep::CaInstall, None);
        assert_eq!(journal.steps.len(), 1);
    }

    #[test]
    fn serializes_in_camel_case() {
        let journal = active_journal();
        let json = serde_json::to_value(&journal).expect("serializable");
        assert_eq!(json["schemaVersion"], JOURNAL_SCHEMA_VERSION);
        assert_eq!(json["status"], "active");
        assert_eq!(json["proxyPort"], 8787);
        assert_eq!(json["steps"][0]["step"], "caInstall");
    }
}
