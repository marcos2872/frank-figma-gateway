//! Figma desktop detection and restart.

use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FigmaInfo {
    pub detected: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub running: bool,
}

#[cfg(windows)]
pub fn detect() -> FigmaInfo {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return missing();
    };
    let root = PathBuf::from(local_app_data).join("Figma");
    let direct = root.join("Figma.exe");
    let (path, version) = if direct.is_file() {
        (Some(direct), None)
    } else {
        let mut candidates = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if let Some(version) = name.strip_prefix("app-") {
                    let executable = entry.path().join("Figma.exe");
                    if executable.is_file() {
                        candidates.push((version.to_owned(), executable));
                    }
                }
            }
        }
        candidates.sort_by(|left, right| left.0.cmp(&right.0));
        candidates
            .pop()
            .map(|(version, path)| (Some(path), Some(version)))
            .unwrap_or((None, None))
    };

    FigmaInfo {
        detected: path.is_some(),
        path: path.map(|path| path.display().to_string()),
        version,
        running: is_running(),
    }
}

#[cfg(not(windows))]
pub fn detect() -> FigmaInfo {
    missing()
}

fn missing() -> FigmaInfo {
    FigmaInfo {
        detected: false,
        path: None,
        version: None,
        running: false,
    }
}

#[cfg(windows)]
fn is_running() -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Figma.exe", "/NH"])
        .output();
    match output {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .to_ascii_lowercase()
            .contains("figma.exe"),
        Err(_) => false,
    }
}

#[cfg(not(windows))]
fn is_running() -> bool {
    false
}

#[cfg(windows)]
pub fn restart() -> Result<FigmaInfo, FigmaError> {
    let info = detect();
    let Some(path) = info.path.clone() else {
        return Err(FigmaError::NotFound);
    };
    if info.running {
        let status = std::process::Command::new("taskkill")
            .args(["/IM", "Figma.exe", "/T", "/F"])
            .status()
            .map_err(|error| FigmaError::Process(error.to_string()))?;
        if !status.success() {
            return Err(FigmaError::Process("taskkill falhou".to_owned()));
        }
    }
    std::process::Command::new(&path)
        .spawn()
        .map_err(|error| FigmaError::Process(error.to_string()))?;
    Ok(detect())
}

#[cfg(not(windows))]
pub fn restart() -> Result<FigmaInfo, FigmaError> {
    Err(FigmaError::Unsupported)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FigmaError {
    NotFound,
    #[allow(dead_code)]
    Unsupported,
    Process(String),
}

impl std::fmt::Display for FigmaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FigmaError::NotFound => f.write_str("Figma não encontrado"),
            FigmaError::Unsupported => f.write_str("restart do Figma só no Windows"),
            FigmaError::Process(detail) => write!(f, "processo do Figma: {detail}"),
        }
    }
}

impl std::error::Error for FigmaError {}
