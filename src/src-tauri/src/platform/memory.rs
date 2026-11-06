//! In-memory fallback used by tests and non-Windows builds.

use super::{CertStore, FileProtector, PlatformError};
use std::collections::HashSet;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct MemoryCertStore {
    installed: Mutex<HashSet<String>>,
}

impl CertStore for MemoryCertStore {
    fn is_installed(&self, thumbprint: &str) -> Result<bool, PlatformError> {
        let installed = self
            .installed
            .lock()
            .map_err(|_| PlatformError("cert store lock poisoned".to_owned()))?;
        Ok(installed.contains(thumbprint))
    }

    fn install(&self, thumbprint: &str, _der: &[u8]) -> Result<(), PlatformError> {
        let mut installed = self
            .installed
            .lock()
            .map_err(|_| PlatformError("cert store lock poisoned".to_owned()))?;
        installed.insert(thumbprint.to_owned());
        Ok(())
    }

    fn remove(&self, thumbprint: &str) -> Result<bool, PlatformError> {
        let mut installed = self
            .installed
            .lock()
            .map_err(|_| PlatformError("cert store lock poisoned".to_owned()))?;
        Ok(installed.remove(thumbprint))
    }
}

#[derive(Debug, Default)]
pub struct PlainProtector;

impl FileProtector for PlainProtector {
    fn protect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError> {
        Ok(bytes.to_vec())
    }

    fn unprotect(&self, bytes: &[u8]) -> Result<Vec<u8>, PlatformError> {
        Ok(bytes.to_vec())
    }
}
