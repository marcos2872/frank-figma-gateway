//! Platform-specific certificate store, private-key protection, env and
//! system proxy (WinINET) access.

use ffg_core::env_plan::{EnvMutation, EnvSnapshot};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformError(pub String);

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlatformError {}

pub trait CertStore: Send + Sync {
    fn is_installed(&self, thumbprint: &str) -> Result<bool, PlatformError>;
    fn install(&self, thumbprint: &str, der: &[u8]) -> Result<(), PlatformError>;
    fn remove(&self, thumbprint: &str) -> Result<bool, PlatformError>;
}

pub trait EnvStore: Send + Sync {
    fn snapshot(&self) -> Result<EnvSnapshot, PlatformError>;
    fn apply(&self, mutations: &[EnvMutation]) -> Result<(), PlatformError>;
}

pub trait FileProtector: Send + Sync {
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError>;
    fn unprotect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError>;
}

/// Valores de `Internet Settings` antes da integração. O `server` pode conter
/// `user:pass@host` de um proxy corporativo, então o snapshot é protegido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SystemProxySnapshot {
    pub enabled: Option<u32>,
    pub server: Option<String>,
    pub override_list: Option<String>,
    pub auto_config_url: Option<String>,
}

/// Legacy WinINET proxy state, kept for cleanup of older installs.
pub trait SystemProxyStore: Send + Sync {
    fn snapshot(&self) -> Result<SystemProxySnapshot, PlatformError>;
    fn restore(&self, snapshot: &SystemProxySnapshot) -> Result<(), PlatformError>;
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_env;
#[cfg(windows)]
mod windows_proxy;
#[cfg(windows)]
pub use windows::{WindowsCertStore as PlatformCertStore, WindowsDpapi as PlatformFileProtector};
pub use windows_env::WindowsEnvStore as PlatformEnvStore;
#[cfg(windows)]
pub use windows_proxy::WindowsSystemProxy as PlatformSystemProxyStore;

#[cfg(not(windows))]
mod memory;
#[cfg(not(windows))]
pub use memory::{MemoryCertStore as PlatformCertStore, PlainProtector as PlatformFileProtector};
#[cfg(not(windows))]
pub use memory::{
    MemoryEnvStore as PlatformEnvStore, MemorySystemProxy as PlatformSystemProxyStore,
};
