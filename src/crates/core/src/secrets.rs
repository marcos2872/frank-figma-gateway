//! Secret storage contract and an in-memory implementation for tests.
//!
//! The real Windows adapter (Credential Manager/DPAPI) is introduced when
//! provider credentials start being persisted.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SecretKey(String);

impl SecretKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.trim().is_empty()
    }
}

impl fmt::Display for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SecretValue(String);

impl SecretValue {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretError(pub String);

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SecretError {}

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretError>;
    fn set(&self, key: &SecretKey, value: &SecretValue) -> Result<(), SecretError>;
    fn delete(&self, key: &SecretKey) -> Result<(), SecretError>;
}

#[derive(Debug, Default)]
pub struct MemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

impl SecretStore for MemorySecretStore {
    fn get(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretError> {
        let values = self
            .values
            .lock()
            .map_err(|_| SecretError("secret store lock poisoned".to_owned()))?;
        Ok(values
            .get(key.as_str())
            .map(|value| SecretValue::new(value.clone())))
    }

    fn set(&self, key: &SecretKey, value: &SecretValue) -> Result<(), SecretError> {
        if key.is_empty() {
            return Err(SecretError("secret key must not be empty".to_owned()));
        }
        let mut values = self
            .values
            .lock()
            .map_err(|_| SecretError("secret store lock poisoned".to_owned()))?;
        values.insert(key.as_str().to_owned(), value.expose().to_owned());
        Ok(())
    }

    fn delete(&self, key: &SecretKey) -> Result<(), SecretError> {
        let mut values = self
            .values
            .lock()
            .map_err(|_| SecretError("secret store lock poisoned".to_owned()))?;
        values.remove(key.as_str());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trip() {
        let store = MemorySecretStore::default();
        let key = SecretKey::new("ffg/provider/openrouter");
        let value = SecretValue::new("sk-test");

        assert!(store.get(&key).expect("get").is_none());
        store.set(&key, &value).expect("set");
        assert_eq!(
            store
                .get(&key)
                .expect("get")
                .map(|value| value.expose().to_owned()),
            Some("sk-test".to_owned())
        );
        store.delete(&key).expect("delete");
        assert!(store.get(&key).expect("get").is_none());
    }

    #[test]
    fn secret_value_never_leaks_in_debug_or_display() {
        let value = SecretValue::new("sk-live-super-secret");
        assert_eq!(format!("{value:?}"), "[REDACTED]");
        assert_eq!(format!("{value}"), "[REDACTED]");
        assert!(!format!("{value:?}").contains("sk-live"));
    }

    #[test]
    fn empty_key_is_rejected() {
        let store = MemorySecretStore::default();
        let error = store
            .set(&SecretKey::new("  "), &SecretValue::new("value"))
            .expect_err("must fail");
        assert_eq!(error.0, "secret key must not be empty");
    }

    #[test]
    fn key_exposes_its_name_but_not_a_value() {
        let key = SecretKey::new("ffg/provider/openrouter");
        assert_eq!(key.as_str(), "ffg/provider/openrouter");
        assert_eq!(key.to_string(), "ffg/provider/openrouter");
        assert!(!key.is_empty());
    }
}
