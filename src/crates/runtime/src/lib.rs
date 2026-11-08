//! Runtime pipeline: manifest verification, download, extraction, probe and
//! atomic installation of the LiteLLM/Postgres artifacts.
//!
//! Pure decisions live in `ffg-core`; this crate owns the I/O.

pub mod downloader;
pub mod error;
pub mod extract;
pub mod install;
pub mod manager;
pub mod probe;

pub use downloader::{DownloadPhase, DownloadProgress, ProgressCallback};
pub use error::RuntimeError;
pub use install::Installation;
pub use manager::RuntimeManager;
