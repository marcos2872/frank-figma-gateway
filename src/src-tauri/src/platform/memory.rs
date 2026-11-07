//! In-memory fallback used by tests and non-Windows builds.

use super::{
    CertStore, EnvStore, FileProtector, PlatformError, SystemProxySnapshot, SystemProxyStore,
};
use ffg_core::env_plan::{EnvMutation, EnvSnapshot};
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

#[derive(Debug, Default)]
pub struct MemoryEnvStore {
    snapshot: Mutex<EnvSnapshot>,
}

impl EnvStore for MemoryEnvStore {
    fn snapshot(&self) -> Result<EnvSnapshot, PlatformError> {
        self.snapshot
            .lock()
            .map_err(|_| PlatformError("env store lock poisoned".to_owned()))
            .map(|value| value.clone())
    }

    fn apply(&self, mutations: &[EnvMutation]) -> Result<(), PlatformError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| PlatformError("env store lock poisoned".to_owned()))?;
        for mutation in mutations {
            let (name, value) = match mutation {
                EnvMutation::Set { name, value } => (name.as_str(), Some(value.clone())),
                EnvMutation::Remove { name } => (name.as_str(), None),
            };
            match name {
                "HTTPS_PROXY" => snapshot.https_proxy = value,
                "HTTP_PROXY" => snapshot.http_proxy = value,
                "NO_PROXY" => snapshot.no_proxy = value,
                "ALL_PROXY" => snapshot.all_proxy = value,
                _ => {}
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct MemorySystemProxy {
    current: Mutex<SystemProxySnapshot>,
}

impl SystemProxyStore for MemorySystemProxy {
    fn snapshot(&self) -> Result<SystemProxySnapshot, PlatformError> {
        self.current
            .lock()
            .map_err(|_| PlatformError("system proxy lock poisoned".to_owned()))
            .map(|value| value.clone())
    }

    fn apply(&self, port: u16) -> Result<(), PlatformError> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| PlatformError("system proxy lock poisoned".to_owned()))?;
        current.enabled = Some(1);
        current.server = Some(format!("127.0.0.1:{port}"));
        Ok(())
    }

    fn restore(&self, snapshot: &SystemProxySnapshot) -> Result<(), PlatformError> {
        let mut current = self
            .current
            .lock()
            .map_err(|_| PlatformError("system proxy lock poisoned".to_owned()))?;
        *current = snapshot.clone();
        Ok(())
    }
}
