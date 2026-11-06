//! Pure installation planning and archive safety checks.
//!
//! ZIP extraction itself belongs to `ffg-runtime`; this module validates the
//! archive layout and computes the atomic activation/rollback plan.

use crate::download::ComponentId;
use crate::manifest::ComponentManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

pub const MAX_ZIP_ENTRIES: usize = 10_000;
pub const MAX_UNCOMPRESSED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_COMPRESSION_RATIO: u64 = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipEntry {
    pub name: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
}

impl ZipEntry {
    pub fn file(name: impl Into<String>, compressed_size: u64, uncompressed_size: u64) -> Self {
        Self {
            name: name.into(),
            is_dir: false,
            is_symlink: false,
            compressed_size,
            uncompressed_size,
        }
    }

    pub fn directory(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_dir: true,
            is_symlink: false,
            compressed_size: 0,
            uncompressed_size: 0,
        }
    }

    pub fn symlink(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_dir: false,
            is_symlink: true,
            compressed_size: 1,
            uncompressed_size: 1,
        }
    }
}

/// Rejects absolute paths, drive letters, `..`, `.`, empty segments and NUL.
pub fn validate_archive_path(name: &str) -> Result<(), InstallError> {
    if name.trim().is_empty() || name.contains('\0') {
        return Err(InstallError::UnsafePath(name.to_owned()));
    }

    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        return Err(InstallError::UnsafePath(name.to_owned()));
    }

    let trimmed = normalized.trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(InstallError::UnsafePath(name.to_owned()));
    }

    for segment in trimmed.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(InstallError::UnsafePath(name.to_owned()));
        }
    }

    Ok(())
}

pub fn validate_zip_entries(
    entries: &[ZipEntry],
    required_entrypoint: Option<&str>,
) -> Result<(), InstallError> {
    if entries.len() > MAX_ZIP_ENTRIES {
        return Err(InstallError::TooManyEntries(entries.len()));
    }

    let mut names = HashSet::with_capacity(entries.len());
    let mut total_compressed = 0u64;
    let mut total_uncompressed = 0u64;

    for entry in entries {
        validate_archive_path(&entry.name)?;
        if entry.is_symlink {
            return Err(InstallError::Symlink(entry.name.clone()));
        }
        if !names.insert(entry.name.to_ascii_lowercase()) {
            return Err(InstallError::DuplicateEntry(entry.name.clone()));
        }
        total_compressed = total_compressed.saturating_add(entry.compressed_size);
        total_uncompressed = total_uncompressed.saturating_add(entry.uncompressed_size);
    }

    if total_uncompressed > MAX_UNCOMPRESSED_BYTES {
        return Err(InstallError::TooLarge(total_uncompressed));
    }
    if total_uncompressed > 0
        && (total_compressed == 0
            || total_uncompressed / total_compressed.max(1) > MAX_COMPRESSION_RATIO)
    {
        return Err(InstallError::SuspiciousCompression);
    }

    if let Some(required) = required_entrypoint {
        let present = entries
            .iter()
            .any(|entry| entry.name.replace('\\', "/").eq_ignore_ascii_case(required));
        if !present {
            return Err(InstallError::MissingEntrypoint(required.to_owned()));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallPlan {
    pub component: ComponentId,
    pub version: String,
    pub staging_path: String,
    pub target_path: String,
    pub active_pointer: String,
    pub entrypoint: String,
    pub version_args: Vec<String>,
    pub expected_installed_size: u64,
}

impl InstallPlan {
    pub fn new(
        component: ComponentId,
        manifest: &ComponentManifest,
        staging_id: &str,
    ) -> Result<Self, InstallError> {
        if !is_safe_id(staging_id) {
            return Err(InstallError::InvalidStagingId(staging_id.to_owned()));
        }

        let entrypoint = manifest.entrypoint().ok_or(InstallError::EmptyEntrypoint)?;
        validate_archive_path(entrypoint)?;

        Ok(Self {
            component,
            version: manifest.version.clone(),
            staging_path: format!("runtime/staging/{staging_id}"),
            target_path: format!("runtime/{}/{}", component.as_str(), manifest.version),
            active_pointer: "runtime/active.json".to_owned(),
            entrypoint: entrypoint.to_owned(),
            version_args: manifest.entrypoint_args(),
            expected_installed_size: manifest.effective_installed_size(),
        })
    }
}

fn is_safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveComponent {
    pub version: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRuntime {
    #[serde(default)]
    pub litellm: Option<ActiveComponent>,
    #[serde(default)]
    pub pgsql: Option<ActiveComponent>,
}

impl ActiveRuntime {
    pub fn get(&self, component: ComponentId) -> Option<&ActiveComponent> {
        match component {
            ComponentId::Litellm => self.litellm.as_ref(),
            ComponentId::Pgsql => self.pgsql.as_ref(),
        }
    }

    /// Activates a component and returns the previous version, which the caller
    /// keeps for rollback.
    pub fn activate(
        &mut self,
        component: ComponentId,
        next: ActiveComponent,
    ) -> Option<ActiveComponent> {
        let slot = match component {
            ComponentId::Litellm => &mut self.litellm,
            ComponentId::Pgsql => &mut self.pgsql,
        };
        slot.replace(next)
    }

    pub fn rollback(&mut self, component: ComponentId, previous: Option<ActiveComponent>) {
        let slot = match component {
            ComponentId::Litellm => &mut self.litellm,
            ComponentId::Pgsql => &mut self.pgsql,
        };
        *slot = previous;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    UnsafePath(String),
    Symlink(String),
    DuplicateEntry(String),
    TooManyEntries(usize),
    TooLarge(u64),
    SuspiciousCompression,
    MissingEntrypoint(String),
    EmptyEntrypoint,
    InvalidStagingId(String),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstallError::UnsafePath(path) => write!(f, "caminho inseguro no ZIP: {path}"),
            InstallError::Symlink(path) => write!(f, "symlink não permitido no ZIP: {path}"),
            InstallError::DuplicateEntry(path) => write!(f, "entrada duplicada no ZIP: {path}"),
            InstallError::TooManyEntries(count) => {
                write!(f, "ZIP com entradas demais: {count}")
            }
            InstallError::TooLarge(size) => write!(f, "ZIP grande demais: {size} bytes"),
            InstallError::SuspiciousCompression => {
                f.write_str("razão de compressão suspeita (provável zip bomb)")
            }
            InstallError::MissingEntrypoint(path) => {
                write!(f, "entrypoint ausente no ZIP: {path}")
            }
            InstallError::EmptyEntrypoint => f.write_str("entrypoint vazio no manifesto"),
            InstallError::InvalidStagingId(id) => write!(f, "id de staging inválido: {id}"),
        }
    }
}

impl std::error::Error for InstallError {}

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
            version_args: vec![],
            min_app_version: None,
        }
    }

    #[test]
    fn accepts_relative_archive_paths() {
        assert!(validate_archive_path("litellm.exe").is_ok());
        assert!(validate_archive_path("bin/postgres.exe").is_ok());
        assert!(validate_archive_path("bin\\postgres.exe").is_ok());
        assert!(validate_archive_path("runtime/").is_ok());
    }

    #[test]
    fn rejects_unsafe_archive_paths() {
        for path in [
            "",
            " ",
            "/etc/passwd",
            "\\windows\\system32",
            "../escape",
            "bin/../../escape",
            "./file",
            "C:/windows/system32",
            "folder//file",
            "nul\0byte",
        ] {
            assert!(validate_archive_path(path).is_err(), "{path}");
        }
    }

    #[test]
    fn rejects_symlinks_and_duplicates() {
        let symlink = vec![ZipEntry::symlink("link")];
        assert_eq!(
            validate_zip_entries(&symlink, None).expect_err("must fail"),
            InstallError::Symlink("link".to_owned())
        );

        let duplicate = vec![ZipEntry::file("A.TXT", 1, 1), ZipEntry::file("a.txt", 1, 1)];
        assert_eq!(
            validate_zip_entries(&duplicate, None).expect_err("must fail"),
            InstallError::DuplicateEntry("a.txt".to_owned())
        );
    }

    #[test]
    fn validates_size_and_compression_limits() {
        let too_large = vec![ZipEntry::file("big.bin", 1, MAX_UNCOMPRESSED_BYTES + 1)];
        assert!(matches!(
            validate_zip_entries(&too_large, None),
            Err(InstallError::TooLarge(_))
        ));

        let bomb = vec![ZipEntry::file("bomb.bin", 1, 10_000)];
        assert_eq!(
            validate_zip_entries(&bomb, None).expect_err("must fail"),
            InstallError::SuspiciousCompression
        );
    }

    #[test]
    fn requires_the_manifest_entrypoint() {
        let entries = vec![
            ZipEntry::file("litellm.exe", 1, 1),
            ZipEntry::directory("bin/"),
            ZipEntry::file("bin/postgres.exe", 1, 1),
        ];
        assert!(validate_zip_entries(&entries, Some("litellm.exe")).is_ok());
        assert!(validate_zip_entries(&entries, Some("missing.exe")).is_err());
    }

    #[test]
    fn builds_a_versioned_install_plan() {
        let plan =
            InstallPlan::new(ComponentId::Litellm, &manifest(), "abc-123").expect("valid plan");
        assert_eq!(plan.target_path, "runtime/litellm/1.108.1");
        assert_eq!(plan.staging_path, "runtime/staging/abc-123");
        assert_eq!(plan.active_pointer, "runtime/active.json");
        assert_eq!(plan.version_args, vec!["--version"]);
        assert_eq!(plan.expected_installed_size, 200);
    }

    #[test]
    fn rejects_bad_staging_ids_and_missing_entrypoints() {
        assert!(matches!(
            InstallPlan::new(ComponentId::Litellm, &manifest(), "../escape"),
            Err(InstallError::InvalidStagingId(_))
        ));

        let mut manifest = manifest();
        manifest.entrypoint = None;
        assert_eq!(
            InstallPlan::new(ComponentId::Litellm, &manifest, "ok").expect_err("must fail"),
            InstallError::EmptyEntrypoint
        );
    }

    #[test]
    fn activation_keeps_the_previous_version_for_rollback() {
        let mut active = ActiveRuntime::default();
        let first = ActiveComponent {
            version: "1.108.1".to_owned(),
            path: "runtime/litellm/1.108.1".to_owned(),
        };
        assert!(active
            .activate(ComponentId::Litellm, first.clone())
            .is_none());
        assert_eq!(active.get(ComponentId::Litellm), Some(&first));

        let second = ActiveComponent {
            version: "1.108.2".to_owned(),
            path: "runtime/litellm/1.108.2".to_owned(),
        };
        let previous = active.activate(ComponentId::Litellm, second.clone());
        assert_eq!(previous, Some(first.clone()));

        active.rollback(ComponentId::Litellm, previous);
        assert_eq!(active.get(ComponentId::Litellm), Some(&first));
    }

    #[test]
    fn serializes_active_runtime_in_camel_case() {
        let active = ActiveRuntime {
            litellm: Some(ActiveComponent {
                version: "1.108.1".to_owned(),
                path: "runtime/litellm/1.108.1".to_owned(),
            }),
            pgsql: None,
        };
        let json = serde_json::to_value(&active).expect("serializable");
        assert_eq!(json["litellm"]["version"], "1.108.1");
        assert!(json["pgsql"].is_null());
    }
}
