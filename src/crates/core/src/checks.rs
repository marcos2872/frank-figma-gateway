//! Pure environment checks used by the onboarding welcome step.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckId {
    Platform,
    DataDir,
    Disk,
    Internet,
    Figma,
    WebView2,
    Ports,
    ExistingProxy,
    Admin,
    Clock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentCheck {
    pub id: CheckId,
    pub status: CheckStatus,
    pub message: String,
    #[serde(default)]
    pub detail: Option<String>,
}

impl EnvironmentCheck {
    pub fn new(id: CheckId, status: CheckStatus, message: impl Into<String>) -> Self {
        Self {
            id,
            status,
            message: message.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FigmaDetection {
    pub found: bool,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProxyEnvironment {
    #[serde(default)]
    pub https_proxy: Option<String>,
    #[serde(default)]
    pub http_proxy: Option<String>,
    #[serde(default)]
    pub no_proxy: Option<String>,
}

impl ProxyEnvironment {
    pub fn is_empty(&self) -> bool {
        self.https_proxy.is_none() && self.http_proxy.is_none() && self.no_proxy.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortStatus {
    pub port: u16,
    pub available: bool,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentReport {
    pub os: String,
    pub arch: String,
    pub supported: bool,
    pub data_dir: String,
    pub free_disk_bytes: u64,
    pub required_disk_bytes: u64,
    pub checks: Vec<EnvironmentCheck>,
    pub figma: FigmaDetection,
    pub existing_proxy: ProxyEnvironment,
    pub ports: Vec<PortStatus>,
    pub blocking: bool,
    pub warnings: Vec<String>,
}

/// Windows x64 is the only supported target for the full app.
pub fn is_supported_platform(os: &str, arch: &str) -> bool {
    os.eq_ignore_ascii_case("windows")
        && (arch.eq_ignore_ascii_case("x86_64")
            || arch.eq_ignore_ascii_case("amd64")
            || arch.eq_ignore_ascii_case("x64"))
}

/// Adds a 20% safety margin to the expected runtime size.
pub fn disk_requirement_with_margin(expected_bytes: u64) -> u64 {
    expected_bytes.saturating_add(expected_bytes / 5)
}

pub fn has_enough_disk(free_bytes: u64, required_bytes: u64) -> bool {
    free_bytes >= disk_requirement_with_margin(required_bytes)
}

/// Returns whether the report is blocking and the warning messages that should
/// be shown to the user.
pub fn summarize(checks: &[EnvironmentCheck]) -> (bool, Vec<String>) {
    let blocking = checks.iter().any(|check| check.status == CheckStatus::Fail);
    let warnings = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Warn)
        .map(|check| check.message.clone())
        .collect();
    (blocking, warnings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_only_windows_x64() {
        assert!(is_supported_platform("Windows", "x86_64"));
        assert!(is_supported_platform("windows", "AMD64"));
        assert!(!is_supported_platform("Linux", "x86_64"));
        assert!(!is_supported_platform("Windows", "aarch64"));
    }

    #[test]
    fn applies_twenty_percent_margin() {
        assert_eq!(disk_requirement_with_margin(100), 120);
        assert_eq!(disk_requirement_with_margin(0), 0);
        assert!(has_enough_disk(120, 100));
        assert!(!has_enough_disk(119, 100));
    }

    #[test]
    fn disk_math_does_not_overflow() {
        assert_eq!(disk_requirement_with_margin(u64::MAX), u64::MAX);
    }

    #[test]
    fn summarizes_blockers_and_warnings() {
        let checks = vec![
            EnvironmentCheck::new(CheckId::Platform, CheckStatus::Pass, "ok"),
            EnvironmentCheck::new(CheckId::Figma, CheckStatus::Warn, "Figma não encontrado"),
            EnvironmentCheck::new(CheckId::Disk, CheckStatus::Fail, "disco insuficiente"),
        ];
        let (blocking, warnings) = summarize(&checks);
        assert!(blocking);
        assert_eq!(warnings, vec!["Figma não encontrado"]);
    }

    #[test]
    fn detects_existing_proxy_environment() {
        assert!(ProxyEnvironment::default().is_empty());
        let proxy = ProxyEnvironment {
            https_proxy: Some("http://127.0.0.1:8080".to_owned()),
            ..ProxyEnvironment::default()
        };
        assert!(!proxy.is_empty());
    }
}
