//! `--version` probe for a freshly extracted runtime.

use std::path::Path;
use std::time::Duration;

use tokio::process::Command;

use crate::error::RuntimeError;

const PROBE_TIMEOUT: Duration = Duration::from_secs(30);

/// Runs `program` with `args` and requires the reported version to match.
pub async fn probe_version(
    program: &Path,
    args: &[String],
    expected: &str,
    timeout: Duration,
) -> Result<String, RuntimeError> {
    if !program.is_file() {
        return Err(RuntimeError::VersionProbe(format!(
            "executável ausente: {}",
            program.display()
        )));
    }

    let mut command = Command::new(program);
    command
        .args(args)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null());
    hide_console_window(&mut command);

    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| RuntimeError::Timeout("probe de --version".to_owned()))?
        .map_err(|error| RuntimeError::VersionProbe(error.to_string()))?;

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let text = text.trim().to_owned();

    if !output.status.success() {
        return Err(RuntimeError::VersionProbe(format!(
            "o executável saiu com {}: {}",
            output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "sinal".to_owned()),
            truncate(&text)
        )));
    }

    if !version_matches(&text, expected) {
        return Err(RuntimeError::VersionMismatch {
            expected: expected.to_owned(),
            output: truncate(&text),
        });
    }

    Ok(text)
}

pub async fn probe_default(
    program: &Path,
    args: &[String],
    expected: &str,
) -> Result<String, RuntimeError> {
    probe_version(program, args, expected, PROBE_TIMEOUT).await
}

/// Matches the expected version as a whole token, so `1.108.1` does not match
/// `1.108.10`.
pub fn version_matches(output: &str, expected: &str) -> bool {
    let expected = expected.trim();
    if expected.is_empty() {
        return false;
    }
    output
        .split(|character: char| !(character.is_ascii_digit() || character == '.'))
        .any(|token| token == expected)
}

fn truncate(text: &str) -> String {
    const LIMIT: usize = 240;
    if text.chars().count() <= LIMIT {
        return text.to_owned();
    }
    let head: String = text.chars().take(LIMIT).collect();
    format!("{head}…")
}

#[cfg(windows)]
fn hide_console_window(command: &mut Command) {
    // CREATE_NO_WINDOW: the probe must not flash a console over the GUI.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_whole_version_tokens() {
        assert!(version_matches("litellm 1.108.1", "1.108.1"));
        assert!(version_matches("postgres (PostgreSQL) 16.9", "16.9"));
        assert!(version_matches("v1.108.1\n", "1.108.1"));
    }

    #[test]
    fn rejects_prefix_and_suffix_matches() {
        assert!(!version_matches("litellm 1.108.10", "1.108.1"));
        assert!(!version_matches("litellm 11.108.1", "1.108.1"));
        assert!(!version_matches("", "1.108.1"));
        assert!(!version_matches("litellm 1.108.1", ""));
    }

    #[tokio::test]
    async fn reports_a_missing_executable() {
        let error = probe_default(
            Path::new("definitely-missing-ffg-binary"),
            &["--version".to_owned()],
            "1.0.0",
        )
        .await
        .expect_err("must fail");
        assert!(matches!(error, RuntimeError::VersionProbe(_)));
    }

    #[test]
    fn truncates_long_output() {
        let long = "x".repeat(500);
        let truncated = truncate(&long);
        assert_eq!(truncated.chars().count(), 241);
        assert!(truncated.ends_with('…'));
    }
}
