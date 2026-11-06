//! Pure download planning: resume, retry and mirror selection.
//!
//! The actual HTTP stream and file writes live in `ffg-runtime`; this module
//! only decides what to request and how to react to failures.

use crate::manifest::{is_valid_sha256, ComponentManifest};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ComponentId {
    Litellm,
    Pgsql,
}

impl ComponentId {
    pub fn as_str(self) -> &'static str {
        match self {
            ComponentId::Litellm => "litellm",
            ComponentId::Pgsql => "pgsql",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeState {
    pub downloaded: u64,
    pub total: u64,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DownloadAction {
    Start,
    Resume {
        start: u64,
        if_range: Option<String>,
    },
    Restart,
    Complete,
}

/// Decides whether a download starts, resumes with a Range request, restarts
/// from zero or is already complete.
pub fn plan_download_action(resume: Option<&ResumeState>, expected_size: u64) -> DownloadAction {
    let Some(resume) = resume else {
        return DownloadAction::Start;
    };

    if resume.total != expected_size || resume.downloaded > expected_size {
        return DownloadAction::Restart;
    }
    if resume.downloaded == expected_size {
        return DownloadAction::Complete;
    }
    if resume.downloaded == 0 {
        return DownloadAction::Start;
    }

    DownloadAction::Resume {
        start: resume.downloaded,
        if_range: resume.etag.clone().or_else(|| resume.last_modified.clone()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    /// Deterministic jitter percentage (0–50) used by tests and callers.
    pub jitter_percent: u8,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 500,
            max_delay_ms: 5_000,
            jitter_percent: 20,
        }
    }
}

impl RetryPolicy {
    pub fn can_retry(&self, attempt: u32) -> bool {
        attempt < self.max_attempts
    }

    /// Exponential backoff with deterministic jitter. `attempt` is 1-based.
    pub fn delay_ms(&self, attempt: u32, jitter_seed: u64) -> u64 {
        if attempt == 0 {
            return 0;
        }

        let exponent = attempt.saturating_sub(1).min(20);
        let base = self
            .base_delay_ms
            .saturating_mul(1u64.checked_shl(exponent).unwrap_or(u64::MAX));
        let capped = base.min(self.max_delay_ms);
        if self.jitter_percent == 0 || capped == 0 {
            return capped;
        }

        let range = capped.saturating_mul(u64::from(self.jitter_percent)) / 100;
        let jitter = if range == 0 {
            0
        } else {
            jitter_seed % (range + 1)
        };
        capped.saturating_add(jitter).min(self.max_delay_ms)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorPlan {
    pub mirrors: Vec<String>,
    pub attempts_per_mirror: u32,
}

impl MirrorPlan {
    pub fn new(
        primary: &str,
        mirrors: Vec<String>,
        attempts_per_mirror: u32,
    ) -> Result<Self, DownloadError> {
        if attempts_per_mirror == 0 {
            return Err(DownloadError::InvalidAttempts);
        }

        let mut urls = Vec::new();
        for url in std::iter::once(primary.to_owned()).chain(mirrors) {
            let url = url.trim();
            if url.is_empty() || urls.iter().any(|existing| existing == url) {
                continue;
            }
            if !url.starts_with("https://") {
                return Err(DownloadError::InvalidUrl(url.to_owned()));
            }
            urls.push(url.to_owned());
        }

        if urls.is_empty() {
            return Err(DownloadError::EmptyMirrors);
        }

        Ok(Self {
            mirrors: urls,
            attempts_per_mirror,
        })
    }

    /// Returns the mirror for a 1-based attempt, or `None` when out of range.
    pub fn for_attempt(&self, attempt: u32) -> Option<&str> {
        if attempt == 0 || self.mirrors.is_empty() {
            return None;
        }
        let position = (attempt - 1) / self.attempts_per_mirror;
        self.mirrors
            .get(position as usize % self.mirrors.len())
            .map(String::as_str)
    }

    pub fn total_attempts(&self) -> u32 {
        (self.mirrors.len() as u32).saturating_mul(self.attempts_per_mirror)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPlan {
    pub component: ComponentId,
    pub version: String,
    pub expected_size: u64,
    pub expected_sha256: String,
    pub mirrors: MirrorPlan,
    pub action: DownloadAction,
    pub retry: RetryPolicy,
}

impl DownloadPlan {
    pub fn new(
        component: ComponentId,
        manifest: &ComponentManifest,
        mirrors: Vec<String>,
        resume: Option<&ResumeState>,
    ) -> Result<Self, DownloadError> {
        if manifest.size == 0 {
            return Err(DownloadError::InvalidSize);
        }
        if !is_valid_sha256(&manifest.sha256) {
            return Err(DownloadError::InvalidHash);
        }

        Ok(Self {
            component,
            version: manifest.version.clone(),
            expected_size: manifest.size,
            expected_sha256: manifest.sha256.clone(),
            mirrors: MirrorPlan::new(&manifest.url, mirrors, 2)?,
            action: plan_download_action(resume, manifest.size),
            retry: RetryPolicy::default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    EmptyMirrors,
    InvalidUrl(String),
    InvalidAttempts,
    InvalidSize,
    InvalidHash,
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DownloadError::EmptyMirrors => f.write_str("nenhum espelho de download configurado"),
            DownloadError::InvalidUrl(url) => write!(f, "URL de download inválida: {url}"),
            DownloadError::InvalidAttempts => f.write_str("tentativas por espelho deve ser > 0"),
            DownloadError::InvalidSize => f.write_str("tamanho do artefato deve ser > 0"),
            DownloadError::InvalidHash => f.write_str("sha256 do artefato inválido"),
        }
    }
}

impl std::error::Error for DownloadError {}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn manifest() -> ComponentManifest {
        ComponentManifest {
            version: "1.108.1".to_owned(),
            url: "https://example.invalid/litellm.zip".to_owned(),
            sha256: SHA256.to_owned(),
            size: 100,
            installed_size: Some(200),
            entrypoint: Some("litellm.exe".to_owned()),
            version_args: vec!["--version".to_owned()],
            min_app_version: None,
        }
    }

    #[test]
    fn starts_without_resume_state() {
        assert_eq!(plan_download_action(None, 100), DownloadAction::Start);
    }

    #[test]
    fn resumes_partial_download_with_etag() {
        let resume = ResumeState {
            downloaded: 40,
            total: 100,
            etag: Some("\"abc\"".to_owned()),
            last_modified: Some("yesterday".to_owned()),
        };
        assert_eq!(
            plan_download_action(Some(&resume), 100),
            DownloadAction::Resume {
                start: 40,
                if_range: Some("\"abc\"".to_owned())
            }
        );
    }

    #[test]
    fn falls_back_to_last_modified_for_if_range() {
        let resume = ResumeState {
            downloaded: 40,
            total: 100,
            etag: None,
            last_modified: Some("yesterday".to_owned()),
        };
        assert_eq!(
            plan_download_action(Some(&resume), 100),
            DownloadAction::Resume {
                start: 40,
                if_range: Some("yesterday".to_owned())
            }
        );
    }

    #[test]
    fn restarts_on_mismatched_or_oversized_resume() {
        let mismatched = ResumeState {
            downloaded: 40,
            total: 90,
            etag: None,
            last_modified: None,
        };
        assert_eq!(
            plan_download_action(Some(&mismatched), 100),
            DownloadAction::Restart
        );

        let oversized = ResumeState {
            downloaded: 120,
            total: 100,
            etag: None,
            last_modified: None,
        };
        assert_eq!(
            plan_download_action(Some(&oversized), 100),
            DownloadAction::Restart
        );
    }

    #[test]
    fn detects_complete_download() {
        let complete = ResumeState {
            downloaded: 100,
            total: 100,
            etag: None,
            last_modified: None,
        };
        assert_eq!(
            plan_download_action(Some(&complete), 100),
            DownloadAction::Complete
        );
    }

    #[test]
    fn retry_backoff_is_capped_and_deterministic() {
        let policy = RetryPolicy {
            max_attempts: 4,
            base_delay_ms: 100,
            max_delay_ms: 350,
            jitter_percent: 0,
        };
        assert_eq!(policy.delay_ms(0, 0), 0);
        assert_eq!(policy.delay_ms(1, 0), 100);
        assert_eq!(policy.delay_ms(2, 0), 200);
        assert_eq!(policy.delay_ms(3, 0), 350);
        assert_eq!(policy.delay_ms(4, 0), 350);
        assert!(policy.can_retry(3));
        assert!(!policy.can_retry(4));
    }

    #[test]
    fn retry_jitter_is_bounded() {
        let policy = RetryPolicy {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 1_000,
            jitter_percent: 20,
        };
        let first = policy.delay_ms(1, 7);
        let second = policy.delay_ms(1, 7);
        assert_eq!(first, second);
        assert!((100..=120).contains(&first));
    }

    #[test]
    fn mirror_plan_rotates_and_deduplicates() {
        let plan = MirrorPlan::new(
            "https://primary.invalid/a.zip",
            vec![
                "https://mirror.invalid/a.zip".to_owned(),
                "https://primary.invalid/a.zip".to_owned(),
            ],
            2,
        )
        .expect("valid mirrors");
        assert_eq!(plan.mirrors.len(), 2);
        assert_eq!(plan.for_attempt(1), Some("https://primary.invalid/a.zip"));
        assert_eq!(plan.for_attempt(2), Some("https://primary.invalid/a.zip"));
        assert_eq!(plan.for_attempt(3), Some("https://mirror.invalid/a.zip"));
        assert_eq!(plan.for_attempt(4), Some("https://mirror.invalid/a.zip"));
        assert_eq!(plan.total_attempts(), 4);
    }

    #[test]
    fn mirror_plan_rejects_invalid_input() {
        assert_eq!(
            MirrorPlan::new("", vec![], 1).expect_err("must fail"),
            DownloadError::EmptyMirrors
        );
        assert_eq!(
            MirrorPlan::new("http://example.invalid/a.zip", vec![], 1).expect_err("must fail"),
            DownloadError::InvalidUrl("http://example.invalid/a.zip".to_owned())
        );
        assert_eq!(
            MirrorPlan::new("https://example.invalid/a.zip", vec![], 0).expect_err("must fail"),
            DownloadError::InvalidAttempts
        );
    }

    #[test]
    fn download_plan_validates_manifest_fields() {
        let mut manifest = manifest();
        assert!(DownloadPlan::new(ComponentId::Litellm, &manifest, vec![], None).is_ok());

        manifest.size = 0;
        assert_eq!(
            DownloadPlan::new(ComponentId::Litellm, &manifest, vec![], None)
                .expect_err("must fail"),
            DownloadError::InvalidSize
        );

        manifest.size = 100;
        manifest.sha256 = "abc".to_owned();
        assert_eq!(
            DownloadPlan::new(ComponentId::Litellm, &manifest, vec![], None)
                .expect_err("must fail"),
            DownloadError::InvalidHash
        );
    }

    #[test]
    fn component_id_is_stable() {
        assert_eq!(ComponentId::Litellm.as_str(), "litellm");
        assert_eq!(ComponentId::Pgsql.as_str(), "pgsql");
    }
}
