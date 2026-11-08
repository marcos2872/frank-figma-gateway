//! Safe ZIP extraction.
//!
//! All path/size/symlink rules live in `ffg_core::install`; this module only
//! applies them to a real archive and writes files under the staging directory.

use ffg_core::install::{
    validate_archive_path, validate_zip_entries, ZipEntry, MAX_UNCOMPRESSED_BYTES,
};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::error::RuntimeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractionReport {
    pub files: u64,
    pub bytes: u64,
}

/// Extracts `zip_path` into `destination`, rejecting unsafe archives.
pub fn extract_zip(
    zip_path: &Path,
    destination: &Path,
    required_entrypoint: Option<&str>,
) -> Result<ExtractionReport, RuntimeError> {
    let file = File::open(zip_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|error| RuntimeError::Zip(error.to_string()))?;

    let entries = collect_entries(&mut archive)?;
    validate_zip_entries(&entries, required_entrypoint)?;

    fs::create_dir_all(destination)?;

    let mut files = 0u64;
    let mut written = 0u64;
    for index in 0..entries.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| RuntimeError::Zip(error.to_string()))?;
        let name = entry.name().to_owned();
        validate_archive_path(&name)?;

        let relative = sanitize_relative(&name)
            .ok_or_else(|| RuntimeError::Extraction(format!("nome inválido no ZIP: {name}")))?;
        let target = destination.join(&relative);

        if entry.is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&target)?;
        let copied = io::copy(&mut entry, &mut output)
            .map_err(|error| RuntimeError::Extraction(format!("{name}: {error}")))?;
        written = written.saturating_add(copied);
        if written > MAX_UNCOMPRESSED_BYTES {
            return Err(RuntimeError::Extraction(
                "conteúdo extraído excede o limite permitido".to_owned(),
            ));
        }
        files += 1;
    }

    Ok(ExtractionReport {
        files,
        bytes: written,
    })
}

fn collect_entries<R: Read + io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Vec<ZipEntry>, RuntimeError> {
    let mut entries = Vec::with_capacity(archive.len());
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| RuntimeError::Zip(error.to_string()))?;
        let is_symlink = entry
            .unix_mode()
            .map(|mode| mode & 0o170_000 == 0o120_000)
            .unwrap_or(false);
        entries.push(ZipEntry {
            name: entry.name().to_owned(),
            is_dir: entry.is_dir(),
            is_symlink,
            compressed_size: entry.compressed_size(),
            uncompressed_size: entry.size(),
        });
    }
    Ok(entries)
}

/// Converts a validated archive name into a relative path without `..`.
fn sanitize_relative(name: &str) -> Option<PathBuf> {
    let normalized = name.replace('\\', "/");
    let mut path = PathBuf::new();
    for segment in normalized.trim_end_matches('/').split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        path.push(segment);
    }
    if path.as_os_str().is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Writes a small ZIP fixture used by the installer tests.
#[cfg(test)]
pub(crate) fn write_zip(
    path: &Path,
    entries: &[(&str, &[u8])],
) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    let file = File::create(path)?;
    let mut writer = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer.start_file(*name, options)?;
        writer.write_all(bytes)?;
    }
    writer.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_a_safe_archive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("runtime.zip");
        write_zip(
            &zip_path,
            &[("litellm.exe", b"binary"), ("bin/data.json", b"{}")],
        )
        .expect("zip");

        let destination = dir.path().join("staging");
        let report = extract_zip(&zip_path, &destination, Some("litellm.exe")).expect("extract");
        assert_eq!(report.files, 2);
        assert!(destination.join("litellm.exe").is_file());
        assert!(destination.join("bin").join("data.json").is_file());
    }

    #[test]
    fn rejects_a_missing_entrypoint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("runtime.zip");
        write_zip(&zip_path, &[("other.exe", b"x")]).expect("zip");

        let error = extract_zip(&zip_path, &dir.path().join("out"), Some("litellm.exe"))
            .expect_err("must fail");
        assert!(matches!(error, RuntimeError::Install(_)));
    }

    #[test]
    fn rejects_path_traversal_entries() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("runtime.zip");
        write_zip(&zip_path, &[("../escape.exe", b"x")]).expect("zip");

        let error = extract_zip(&zip_path, &dir.path().join("out"), None).expect_err("must fail");
        assert!(matches!(error, RuntimeError::Install(_)));
    }

    #[test]
    fn rejects_a_corrupt_archive() {
        let dir = tempfile::tempdir().expect("tempdir");
        let zip_path = dir.path().join("runtime.zip");
        fs::write(&zip_path, b"not a zip").expect("write");

        let error = extract_zip(&zip_path, &dir.path().join("out"), None).expect_err("must fail");
        assert!(matches!(error, RuntimeError::Zip(_)));
    }

    #[test]
    fn sanitizes_relative_names() {
        assert_eq!(
            sanitize_relative("bin/postgres.exe"),
            Some(PathBuf::from("bin/postgres.exe"))
        );
        assert_eq!(
            sanitize_relative("bin\\postgres.exe"),
            Some(PathBuf::from("bin/postgres.exe"))
        );
        assert_eq!(sanitize_relative("../escape"), None);
        assert_eq!(sanitize_relative("./file"), None);
        assert_eq!(sanitize_relative(""), None);
    }
}
