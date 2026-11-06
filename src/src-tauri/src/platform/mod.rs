//! Platform-specific certificate store and private-key protection.

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

pub trait FileProtector: Send + Sync {
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError>;
    fn unprotect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError>;
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{WindowsCertStore as PlatformCertStore, WindowsDpapi as PlatformFileProtector};

#[cfg(not(windows))]
mod memory;
#[cfg(not(windows))]
pub use memory::{MemoryCertStore as PlatformCertStore, PlainProtector as PlatformFileProtector};
