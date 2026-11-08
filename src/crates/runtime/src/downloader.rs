//! HTTP downloader with resume, mirror rotation, retries and hash verification.
//!
//! The stream is written straight to `cache/downloads/<component>-<version>.zip.part`
//! together with a `.meta.json` sidecar, so closing the app never loses progress.
//! Nothing is extracted here: the caller verifies the hash again before install.

use ffg_core::download::{
    plan_download_action, ComponentId, DownloadAction, DownloadPlan, ResumeState,
};
use ffg_core::manifest::ComponentManifest;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use crate::error::RuntimeError;

/// Progress callback used by the Tauri layer to emit `download://progress`.
pub type ProgressCallback = Arc<dyn Fn(DownloadProgress) + Send + Sync>;

const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DownloadPhase {
    Downloading,
    Verifying,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub component: ComponentId,
    pub downloaded: u64,
    pub total: u64,
    pub attempt: u32,
    pub phase: DownloadPhase,
}

/// Metadata persisted next to the `.part` file to allow `If-Range` resumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadMeta {
    downloaded: u64,
    total: u64,
    #[serde(default)]
    etag: Option<String>,
    #[serde(default)]
    last_modified: Option<String>,
}

impl DownloadMeta {
    fn resume_state(&self) -> ResumeState {
        ResumeState {
            downloaded: self.downloaded,
            total: self.total,
            etag: self.etag.clone(),
            last_modified: self.last_modified.clone(),
        }
    }
}

pub struct Downloader {
    client: reqwest::Client,
    downloads_dir: PathBuf,
}

impl Downloader {
    pub fn new(downloads_dir: PathBuf) -> Self {
        let client = reqwest::Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self {
            client,
            downloads_dir,
        }
    }

    pub fn part_path(&self, component: ComponentId, version: &str) -> PathBuf {
        self.downloads_dir
            .join(format!("{}-{version}.zip.part", component.as_str()))
    }

    pub fn zip_path(&self, component: ComponentId, version: &str) -> PathBuf {
        self.downloads_dir
            .join(format!("{}-{version}.zip", component.as_str()))
    }

    fn meta_path(&self, component: ComponentId, version: &str) -> PathBuf {
        self.downloads_dir
            .join(format!("{}-{version}.zip.meta.json", component.as_str()))
    }

    /// Downloads (or resumes) the component archive and returns the verified
    /// `.zip` path. A cached archive with a matching hash is reused.
    pub async fn download(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        mirrors: Vec<String>,
        progress: Option<ProgressCallback>,
        cancel: Option<CancellationToken>,
    ) -> Result<PathBuf, RuntimeError> {
        let plan = DownloadPlan::new(component, manifest, mirrors, None)?;
        fs::create_dir_all(&self.downloads_dir).await?;

        let zip_path = self.zip_path(component, &manifest.version);
        let part_path = self.part_path(component, &manifest.version);
        let meta_path = self.meta_path(component, &manifest.version);

        if zip_path.is_file() {
            if file_matches(&zip_path, &manifest.sha256).await {
                emit(
                    &progress,
                    component,
                    manifest.size,
                    manifest.size,
                    0,
                    DownloadPhase::Complete,
                );
                return Ok(zip_path);
            }
            fs::remove_file(&zip_path).await?;
        }

        let mut last_error = RuntimeError::Network("nenhuma tentativa executada".to_owned());
        for attempt in 1..=plan.mirrors.total_attempts() {
            if is_cancelled(&cancel) {
                return Err(RuntimeError::Cancelled);
            }
            let Some(mirror) = plan.mirrors.for_attempt(attempt) else {
                break;
            };
            let result = self
                .attempt(
                    component, manifest, mirror, &part_path, &meta_path, attempt, &progress,
                    &cancel,
                )
                .await;

            match result {
                Ok(()) => {
                    return self
                        .finalize(
                            component, manifest, &part_path, &meta_path, &zip_path, &progress,
                        )
                        .await
                }
                Err(error @ (RuntimeError::Cancelled | RuntimeError::HashMismatch { .. })) => {
                    return Err(error)
                }
                Err(error) => {
                    last_error = error;
                    let delay = plan.retry.delay_ms(attempt, u64::from(attempt) * 7);
                    if delay > 0 {
                        tokio::time::sleep(Duration::from_millis(delay)).await;
                    }
                }
            }
        }

        Err(last_error)
    }

    #[allow(clippy::too_many_arguments)]
    async fn attempt(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        mirror: &str,
        part_path: &Path,
        meta_path: &Path,
        attempt: u32,
        progress: &Option<ProgressCallback>,
        cancel: &Option<CancellationToken>,
    ) -> Result<(), RuntimeError> {
        let expected = manifest.size;
        let resume = self.resume_state(part_path, meta_path, expected).await;
        let action = plan_download_action(resume.as_ref(), expected);

        if matches!(action, DownloadAction::Complete) {
            if file_matches(part_path, &manifest.sha256).await {
                return Ok(());
            }
            let actual = hash_file(part_path).await.unwrap_or_default();
            let _ = fs::remove_file(part_path).await;
            let _ = fs::remove_file(meta_path).await;
            return Err(RuntimeError::HashMismatch {
                expected: manifest.sha256.clone(),
                actual,
            });
        }

        let mut request = self
            .client
            .get(mirror)
            .header("accept-encoding", "identity");
        let mut skip = 0u64;
        if let DownloadAction::Resume { start, if_range } = &action {
            request = request.header("range", format!("bytes={start}-"));
            if let Some(value) = if_range {
                request = request.header("if-range", value.clone());
            }
            skip = *start;
        }

        let response = request
            .send()
            .await
            .map_err(|error| RuntimeError::Network(error.to_string()))?;
        let status = response.status();

        if status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            let _ = fs::remove_file(part_path).await;
            let _ = fs::remove_file(meta_path).await;
            return Err(RuntimeError::RangeInvalid(
                "servidor recusou o intervalo (416); recomeçando".to_owned(),
            ));
        }

        if status.is_client_error() || status.is_server_error() {
            return Err(RuntimeError::Http(status.as_u16()));
        }

        // A 200 while resuming means the server ignored the range: restart.
        let restart = skip > 0 && status == reqwest::StatusCode::OK;
        if skip > 0 && !restart {
            if status != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(RuntimeError::RangeInvalid(format!(
                    "esperado 206, recebido {}",
                    status.as_u16()
                )));
            }
            let header = response
                .headers()
                .get("content-range")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| {
                    RuntimeError::RangeInvalid("resposta 206 sem Content-Range".to_owned())
                })?;
            let (start, _end, total) = parse_content_range(header).ok_or_else(|| {
                RuntimeError::RangeInvalid(format!("Content-Range ilegível: {header}"))
            })?;
            if start != skip {
                return Err(RuntimeError::RangeInvalid(format!(
                    "início {start} diferente do solicitado {skip}"
                )));
            }
            if let Some(total) = total {
                if total != expected {
                    return Err(RuntimeError::RangeInvalid(format!(
                        "tamanho total {total} diferente do manifesto {expected}"
                    )));
                }
            }
        }

        if restart {
            skip = 0;
        }

        if skip == 0 {
            if let Some(length) = response.content_length() {
                if length != expected {
                    return Err(RuntimeError::SizeMismatch {
                        expected,
                        actual: length,
                    });
                }
            }
        }

        let etag = header_string(response.headers(), "etag");
        let last_modified = header_string(response.headers(), "last-modified");

        let mut file = if skip == 0 {
            fs::File::create(part_path).await?
        } else {
            fs::OpenOptions::new().append(true).open(part_path).await?
        };

        let mut hasher = Sha256::new();
        if skip > 0 {
            hash_prefix(part_path, skip, &mut hasher).await?;
        }

        let mut downloaded = skip;
        let mut last_emit = Instant::now();
        emit(
            progress,
            component,
            downloaded,
            expected,
            attempt,
            DownloadPhase::Downloading,
        );

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if is_cancelled(cancel) {
                file.flush().await?;
                self.write_meta(meta_path, downloaded, expected, &etag, &last_modified)
                    .await;
                return Err(RuntimeError::Cancelled);
            }
            let chunk = chunk.map_err(|error| RuntimeError::Network(error.to_string()))?;
            file.write_all(&chunk).await?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;
            if downloaded > expected {
                drop(file);
                let _ = fs::remove_file(part_path).await;
                let _ = fs::remove_file(meta_path).await;
                return Err(RuntimeError::SizeMismatch {
                    expected,
                    actual: downloaded,
                });
            }
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                emit(
                    progress,
                    component,
                    downloaded,
                    expected,
                    attempt,
                    DownloadPhase::Downloading,
                );
                self.write_meta(meta_path, downloaded, expected, &etag, &last_modified)
                    .await;
                last_emit = Instant::now();
            }
        }

        file.flush().await?;
        drop(file);

        if downloaded != expected {
            let _ = fs::remove_file(part_path).await;
            let _ = fs::remove_file(meta_path).await;
            return Err(RuntimeError::SizeMismatch {
                expected,
                actual: downloaded,
            });
        }

        let actual = hex::encode(hasher.finalize());
        if !actual.eq_ignore_ascii_case(manifest.sha256.trim()) {
            let _ = fs::remove_file(part_path).await;
            let _ = fs::remove_file(meta_path).await;
            return Err(RuntimeError::HashMismatch {
                expected: manifest.sha256.clone(),
                actual,
            });
        }

        self.write_meta(meta_path, downloaded, expected, &etag, &last_modified)
            .await;
        Ok(())
    }

    async fn finalize(
        &self,
        component: ComponentId,
        manifest: &ComponentManifest,
        part_path: &Path,
        meta_path: &Path,
        zip_path: &Path,
        progress: &Option<ProgressCallback>,
    ) -> Result<PathBuf, RuntimeError> {
        emit(
            progress,
            component,
            manifest.size,
            manifest.size,
            0,
            DownloadPhase::Verifying,
        );

        if !file_matches(part_path, &manifest.sha256).await {
            let actual = hash_file(part_path).await.unwrap_or_default();
            let _ = fs::remove_file(part_path).await;
            let _ = fs::remove_file(meta_path).await;
            return Err(RuntimeError::HashMismatch {
                expected: manifest.sha256.clone(),
                actual,
            });
        }

        if zip_path.exists() {
            fs::remove_file(zip_path).await?;
        }
        fs::rename(part_path, zip_path).await?;
        let _ = fs::remove_file(meta_path).await;

        emit(
            progress,
            component,
            manifest.size,
            manifest.size,
            0,
            DownloadPhase::Complete,
        );
        Ok(zip_path.to_path_buf())
    }

    async fn resume_state(
        &self,
        part_path: &Path,
        meta_path: &Path,
        expected: u64,
    ) -> Option<ResumeState> {
        let downloaded = fs::metadata(part_path).await.ok()?.len();
        if downloaded == 0 {
            return None;
        }
        let meta = self.read_meta(meta_path).await;
        let mut state = meta.map(|meta| meta.resume_state()).unwrap_or(ResumeState {
            downloaded,
            total: expected,
            etag: None,
            last_modified: None,
        });
        state.downloaded = downloaded;
        state.total = expected;
        Some(state)
    }

    async fn read_meta(&self, meta_path: &Path) -> Option<DownloadMeta> {
        let raw = fs::read(meta_path).await.ok()?;
        serde_json::from_slice(&raw).ok()
    }

    async fn write_meta(
        &self,
        meta_path: &Path,
        downloaded: u64,
        total: u64,
        etag: &Option<String>,
        last_modified: &Option<String>,
    ) {
        let meta = DownloadMeta {
            downloaded,
            total,
            etag: etag.clone(),
            last_modified: last_modified.clone(),
        };
        if let Ok(raw) = serde_json::to_vec(&meta) {
            let _ = fs::write(meta_path, raw).await;
        }
    }
}

/// Hashes a file without loading it into memory.
async fn hash_file(path: &Path) -> Result<String, RuntimeError> {
    let mut file = fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

async fn file_matches(path: &Path, expected_sha256: &str) -> bool {
    match hash_file(path).await {
        Ok(actual) => actual.eq_ignore_ascii_case(expected_sha256.trim()),
        Err(_) => false,
    }
}

/// Feeds exactly `length` leading bytes of `path` into `hasher` (resume case).
async fn hash_prefix(path: &Path, length: u64, hasher: &mut Sha256) -> Result<(), RuntimeError> {
    let mut file = fs::File::open(path).await?;
    let mut remaining = length;
    let mut buffer = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let want = remaining.min(buffer.len() as u64) as usize;
        let read = file.read(&mut buffer[..want]).await?;
        if read == 0 {
            return Err(RuntimeError::RangeInvalid(
                "arquivo parcial mudou de tamanho".to_owned(),
            ));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    Ok(())
}

fn header_string(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// Parses `bytes <start>-<end>/<total>`; `total` is `None` for `*`.
fn parse_content_range(value: &str) -> Option<(u64, u64, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let (start, end) = range.split_once('-')?;
    let start = start.trim().parse::<u64>().ok()?;
    let end = end.trim().parse::<u64>().ok()?;
    let total = match total.trim() {
        "*" => None,
        value => Some(value.parse::<u64>().ok()?),
    };
    if end < start {
        return None;
    }
    Some((start, end, total))
}

fn is_cancelled(cancel: &Option<CancellationToken>) -> bool {
    cancel
        .as_ref()
        .map(CancellationToken::is_cancelled)
        .unwrap_or(false)
}

fn emit(
    progress: &Option<ProgressCallback>,
    component: ComponentId,
    downloaded: u64,
    total: u64,
    attempt: u32,
    phase: DownloadPhase,
) {
    if let Some(callback) = progress {
        callback(DownloadProgress {
            component,
            downloaded,
            total,
            attempt,
            phase,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const BODY: &[u8] = b"litellm-runtime-payload";

    fn sha256(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn manifest(url: String, body: &[u8]) -> ComponentManifest {
        ComponentManifest {
            version: "1.108.1".to_owned(),
            url,
            sha256: sha256(body),
            size: body.len() as u64,
            installed_size: None,
            entrypoint: Some("litellm.exe".to_owned()),
            version_args: vec![],
            min_app_version: None,
        }
    }

    fn downloader() -> (tempfile::TempDir, Downloader) {
        let dir = tempfile::tempdir().expect("tempdir");
        let downloader = Downloader::new(dir.path().to_path_buf());
        (dir, downloader)
    }

    #[tokio::test]
    async fn downloads_and_verifies_the_archive() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/litellm.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .mount(&server)
            .await;

        let (_dir, downloader) = downloader();
        let manifest = manifest(format!("{}/litellm.zip", server.uri()), BODY);
        let phases = Arc::new(Mutex::new(Vec::new()));
        let sink = phases.clone();
        let progress: ProgressCallback = Arc::new(move |event| {
            sink.lock().expect("lock").push(event.phase);
        });

        let zip = downloader
            .download(
                ComponentId::Litellm,
                &manifest,
                vec![],
                Some(progress),
                None,
            )
            .await
            .expect("download");

        assert_eq!(fs::read(&zip).await.expect("read"), BODY);
        let phases = phases.lock().expect("lock");
        assert!(phases.contains(&DownloadPhase::Downloading));
        assert_eq!(phases.last(), Some(&DownloadPhase::Complete));
    }

    #[tokio::test]
    async fn reuses_a_cached_archive_with_matching_hash() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .expect(0)
            .mount(&server)
            .await;

        let (_dir, downloader) = downloader();
        let manifest = manifest(format!("{}/litellm.zip", server.uri()), BODY);
        fs::create_dir_all(&downloader.downloads_dir)
            .await
            .expect("dir");
        fs::write(
            downloader.zip_path(ComponentId::Litellm, &manifest.version),
            BODY,
        )
        .await
        .expect("seed");

        let zip = downloader
            .download(ComponentId::Litellm, &manifest, vec![], None, None)
            .await
            .expect("cached");
        assert!(zip.is_file());
    }

    #[tokio::test]
    async fn resumes_a_partial_download_with_range() {
        let server = MockServer::start().await;
        let split = 7usize;
        Mock::given(method("GET"))
            .and(path("/litellm.zip"))
            .and(header("range", format!("bytes={split}-")))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header(
                        "content-range",
                        format!("bytes {split}-{}/{}", BODY.len() - 1, BODY.len()),
                    )
                    .set_body_bytes(&BODY[split..]),
            )
            .mount(&server)
            .await;

        let (_dir, downloader) = downloader();
        let manifest = manifest(format!("{}/litellm.zip", server.uri()), BODY);
        fs::create_dir_all(&downloader.downloads_dir)
            .await
            .expect("dir");
        fs::write(
            downloader.part_path(ComponentId::Litellm, &manifest.version),
            &BODY[..split],
        )
        .await
        .expect("partial");

        let zip = downloader
            .download(ComponentId::Litellm, &manifest, vec![], None, None)
            .await
            .expect("resume");
        assert_eq!(fs::read(&zip).await.expect("read"), BODY);
    }

    #[tokio::test]
    async fn restarts_when_the_server_ignores_the_range() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/litellm.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .mount(&server)
            .await;

        let (_dir, downloader) = downloader();
        let manifest = manifest(format!("{}/litellm.zip", server.uri()), BODY);
        fs::create_dir_all(&downloader.downloads_dir)
            .await
            .expect("dir");
        fs::write(
            downloader.part_path(ComponentId::Litellm, &manifest.version),
            &BODY[..3],
        )
        .await
        .expect("partial");

        let zip = downloader
            .download(ComponentId::Litellm, &manifest, vec![], None, None)
            .await
            .expect("restart");
        assert_eq!(fs::read(&zip).await.expect("read"), BODY);
    }

    #[tokio::test]
    async fn rejects_a_hash_mismatch_and_never_keeps_the_archive() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/litellm.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"litellm-runtime-payloaX"))
            .mount(&server)
            .await;

        let (_dir, downloader) = downloader();
        let manifest = manifest(format!("{}/litellm.zip", server.uri()), BODY);
        let error = downloader
            .download(ComponentId::Litellm, &manifest, vec![], None, None)
            .await
            .expect_err("must fail");
        assert!(matches!(error, RuntimeError::HashMismatch { .. }));
        assert!(!downloader
            .zip_path(ComponentId::Litellm, &manifest.version)
            .exists());
    }

    #[tokio::test]
    async fn rotates_to_the_next_mirror_after_an_http_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/missing.zip"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/litellm.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .mount(&server)
            .await;

        let (_dir, downloader) = downloader();
        let mut manifest = manifest(format!("{}/missing.zip", server.uri()), BODY);
        manifest.url = format!("{}/missing.zip", server.uri());
        let zip = downloader
            .download(
                ComponentId::Litellm,
                &manifest,
                vec![format!("{}/litellm.zip", server.uri())],
                None,
                None,
            )
            .await
            .expect("mirror");
        assert_eq!(fs::read(&zip).await.expect("read"), BODY);
    }

    #[tokio::test]
    async fn cancellation_keeps_the_partial_file() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/litellm.zip"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(BODY))
            .mount(&server)
            .await;

        let (_dir, downloader) = downloader();
        let manifest = manifest(format!("{}/litellm.zip", server.uri()), BODY);
        let token = CancellationToken::new();
        token.cancel();

        let error = downloader
            .download(ComponentId::Litellm, &manifest, vec![], None, Some(token))
            .await
            .expect_err("cancelled");
        assert!(matches!(error, RuntimeError::Cancelled));
    }

    #[test]
    fn parses_content_range_headers() {
        assert_eq!(
            parse_content_range("bytes 0-99/100"),
            Some((0, 99, Some(100)))
        );
        assert_eq!(parse_content_range("bytes 40-99/*"), Some((40, 99, None)));
        assert_eq!(parse_content_range("items 0-1/2"), None);
        assert_eq!(parse_content_range("bytes 9-1/2"), None);
    }

    #[test]
    fn progress_payload_is_camel_case() {
        let json = serde_json::to_value(DownloadProgress {
            component: ComponentId::Litellm,
            downloaded: 1,
            total: 2,
            attempt: 1,
            phase: DownloadPhase::Verifying,
        })
        .expect("serializable");
        assert_eq!(json["phase"], "verifying");
        assert_eq!(json["total"], 2);
    }
}
